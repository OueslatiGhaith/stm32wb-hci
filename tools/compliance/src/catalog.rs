//! Versioned intermediate protocol catalog shared by source adapters.
//!
//! A family-specific adapter (currently STM32CubeWB C) owns parsing. It emits
//! this schema; coverage comparison, wire validation, JSON reporting, and a
//! future STM32CubeWBA adapter consume it. Keeping that boundary explicit
//! means parser changes do not silently change downstream assumptions.

use serde::{Deserialize, Deserializer, Serialize};

use crate::model::{CoverageEntry, CoverageOrigin, ProtocolCoverage, StandardHciCoverage};

/// Increment only for a deliberate, documented incompatible schema change.
pub const CATALOG_SCHEMA_VERSION: u16 = 6;

/// Firmware family whose generated catalog produced this schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogFamily {
    Stm32Wb,
}

/// Namespace for an HCI command record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandScope {
    VendorAci,
    StandardHci,
}

/// Namespace for an HCI event record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventScope {
    VendorAci,
    StandardHci,
    LeMeta,
}

/// Generated command-completion behavior.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum CompletionExpectation {
    CommandComplete,
    CommandStatus,
    Event(u8),
    /// Source value which cannot yet become a stable completion claim.
    Unresolved(String),
}

/// Shape of the generated request payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum RequestLayout {
    Empty,
    Fixed(u32),
    Variable {
        minimum: u32,
        maximum: u32,
    },
    /// Source expression which cannot yet become a stable wire envelope.
    Unresolved(String),
}

/// Shape of the generated command-complete payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ResponseLayout {
    None,
    Status,
    Fixed(u32),
    Variable {
        minimum: u32,
        maximum: u32,
    },
    /// Source expression which cannot yet become a stable wire envelope.
    Unresolved(String),
}

/// Shape of a generated event payload after the two-byte vendor event code.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum EventPayloadLayout {
    Fixed(u32),
    Variable {
        minimum: u32,
        maximum: u32,
    },
    /// Source evidence which cannot yet become a stable wire envelope.
    Unresolved(String),
}

/// One generated command declaration normalized from a family source adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogCommand {
    pub scope: CommandScope,
    pub name: String,
    pub source_name: String,
    pub source_offset: u32,
    pub ogf: Option<u8>,
    /// Vendor ACI OCF, or the low ten bits of a standard HCI opcode.
    pub ocf: u16,
    /// Full opcode for standard HCI commands. Vendor ACI keeps its OCF
    /// namespace and therefore has no opcode here.
    pub opcode: Option<u16>,
    pub completion: CompletionExpectation,
    pub request: RequestLayout,
    pub response: ResponseLayout,
}

/// One generated event-table entry normalized from a family source adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogEvent {
    pub scope: EventScope,
    pub code: u16,
    pub name: String,
    pub source_name: String,
    pub source_offset: u32,
    /// Vendor ACI payload envelope. Standard HCI and LE Meta events are
    /// inventory-only and intentionally carry no payload claim.
    pub payload: Option<EventPayloadLayout>,
}

/// Stable, normalized result of parsing one immutable firmware source tag.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogSchema {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u16,
    pub family: CatalogFamily,
    pub cube_tag: String,
    pub commands: Vec<CatalogCommand>,
    pub events: Vec<CatalogEvent>,
}

fn deserialize_schema_version<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u16::deserialize(deserializer)?;
    if version == CATALOG_SCHEMA_VERSION {
        Ok(version)
    } else {
        Err(serde::de::Error::custom(format!(
            "unsupported catalog schema version {version}; expected {CATALOG_SCHEMA_VERSION}"
        )))
    }
}

impl CatalogSchema {
    pub(crate) fn new(family: CatalogFamily, cube_tag: impl Into<String>) -> Self {
        Self {
            schema_version: CATALOG_SCHEMA_VERSION,
            family,
            cube_tag: cube_tag.into(),
            commands: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Normalize ordering once at the adapter boundary so a serialized catalog
    /// is deterministic and all downstream consumers see the same ordering.
    pub(crate) fn normalize(&mut self) {
        self.commands.sort_by_key(|command| {
            (
                command_scope_order(command.scope),
                command.ocf,
                command.name.clone(),
                command.source_name.clone(),
                command.source_offset,
            )
        });
        self.events.sort_by_key(|event| {
            (
                event_scope_order(event.scope),
                event.code,
                event.name.clone(),
                event.source_name.clone(),
                event.source_offset,
            )
        });
    }

    pub(crate) fn vendor_coverage(&self) -> ProtocolCoverage {
        ProtocolCoverage {
            commands: self
                .commands
                .iter()
                .filter(|command| command.scope == CommandScope::VendorAci)
                .map(|command| {
                    CoverageEntry::new(command.ocf, &command.name, CoverageOrigin::VendorAutoSource)
                })
                .collect(),
            events: self
                .events
                .iter()
                .filter(|event| event.scope == EventScope::VendorAci)
                .map(|event| {
                    CoverageEntry::new(event.code, &event.name, CoverageOrigin::VendorAutoSource)
                })
                .collect(),
        }
    }

    pub(crate) fn standard_hci_coverage(&self) -> StandardHciCoverage {
        StandardHciCoverage {
            commands: self
                .commands
                .iter()
                .filter(|command| command.scope == CommandScope::StandardHci)
                .filter_map(|command| {
                    command.opcode.map(|opcode| {
                        CoverageEntry::new(
                            opcode,
                            &command.name,
                            CoverageOrigin::StandardHciAutoSource,
                        )
                    })
                })
                .collect(),
            events: self
                .events
                .iter()
                .filter(|event| event.scope == EventScope::StandardHci)
                .map(|event| {
                    CoverageEntry::new(
                        event.code,
                        &event.name,
                        CoverageOrigin::StandardHciAutoSource,
                    )
                })
                .collect(),
            le_meta_events: self
                .events
                .iter()
                .filter(|event| event.scope == EventScope::LeMeta)
                .map(|event| {
                    CoverageEntry::new(
                        event.code,
                        &event.name,
                        CoverageOrigin::StandardHciAutoSource,
                    )
                })
                .collect(),
        }
    }
}

fn command_scope_order(scope: CommandScope) -> u8 {
    match scope {
        CommandScope::VendorAci => 0,
        CommandScope::StandardHci => 1,
    }
}

fn event_scope_order(scope: EventScope) -> u8 {
    match scope {
        EventScope::VendorAci => 0,
        EventScope::StandardHci => 1,
        EventScope::LeMeta => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_serialization_is_versioned_and_deterministic() {
        let mut schema = CatalogSchema::new(CatalogFamily::Stm32Wb, "v1.17.1");
        schema.commands.extend([
            CatalogCommand {
                scope: CommandScope::VendorAci,
                name: "z_last".to_owned(),
                source_name: "z.c".to_owned(),
                source_offset: 9,
                ogf: None,
                ocf: 2,
                opcode: None,
                completion: CompletionExpectation::CommandComplete,
                request: RequestLayout::Empty,
                response: ResponseLayout::Variable {
                    minimum: 2,
                    maximum: 252,
                },
            },
            CatalogCommand {
                scope: CommandScope::VendorAci,
                name: "a_first".to_owned(),
                source_name: "a.c".to_owned(),
                source_offset: 4,
                ogf: None,
                ocf: 1,
                opcode: None,
                completion: CompletionExpectation::CommandStatus,
                request: RequestLayout::Variable {
                    minimum: 3,
                    maximum: 255,
                },
                response: ResponseLayout::None,
            },
        ]);
        schema.events.push(CatalogEvent {
            scope: EventScope::VendorAci,
            code: 0x400,
            name: "gap_event".to_owned(),
            source_name: "ble_events.c".to_owned(),
            source_offset: 12,
            payload: Some(EventPayloadLayout::Fixed(0)),
        });
        schema.events.push(CatalogEvent {
            scope: EventScope::LeMeta,
            code: 0x01,
            name: "le_event".to_owned(),
            source_name: "ble_events.c".to_owned(),
            source_offset: 18,
            payload: None,
        });
        schema.normalize();

        let value = serde_json::to_value(&schema).unwrap();
        assert_eq!(value["schema_version"], CATALOG_SCHEMA_VERSION);
        assert_eq!(value["family"], "stm32_wb");
        assert_eq!(value["commands"][0]["name"], "a_first");
        assert!(value["events"][1]["payload"].is_null());
        assert_eq!(
            value["commands"][0]["request"],
            serde_json::json!({
                "kind": "variable",
                "value": {
                    "minimum": 3,
                    "maximum": 255,
                },
            })
        );
        assert_eq!(
            value["commands"][1]["response"],
            serde_json::json!({
                "kind": "variable",
                "value": {
                    "minimum": 2,
                    "maximum": 252,
                },
            })
        );
        assert_eq!(
            serde_json::from_value::<CatalogSchema>(value).unwrap(),
            schema
        );
        assert_eq!(
            serde_json::to_value(RequestLayout::Unresolved("computed_size".to_owned())).unwrap(),
            serde_json::json!({
                "kind": "unresolved",
                "value": "computed_size",
            })
        );

        let mut unsupported = serde_json::to_value(&schema).unwrap();
        unsupported["schema_version"] = serde_json::json!(CATALOG_SCHEMA_VERSION + 1);
        assert!(serde_json::from_value::<CatalogSchema>(unsupported).is_err());
    }
}
