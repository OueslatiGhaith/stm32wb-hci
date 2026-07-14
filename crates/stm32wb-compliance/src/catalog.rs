//! Versioned intermediate protocol catalog shared by source adapters.
//!
//! A family-specific adapter (currently STM32CubeWB C) owns parsing. It emits
//! this schema; coverage comparison, wire validation, JSON reporting, and a
//! future STM32CubeWBA adapter consume it. Keeping that boundary explicit
//! means parser changes do not silently change downstream assumptions.

use serde::{Deserialize, Deserializer, Serialize};

use crate::model::{CoverageEntry, CoverageOrigin, ProtocolCoverage, StandardHciCoverage};

/// Increment only for a deliberate, documented incompatible schema change.
pub const CATALOG_SCHEMA_VERSION: u16 = 9;

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

/// Wire envelope extracted from generated source evidence.
///
/// Zero-length requests and returns use `Known { minimum: 0, maximum: 0 }`;
/// there is no separate empty representation. The same type is used for
/// command requests, Command Complete returns, and vendor-event payloads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ExtractedEnvelope {
    Known {
        minimum: u32,
        maximum: u32,
    },
    /// Source expression which cannot yet become a stable wire envelope.
    Unresolved(String),
}

impl ExtractedEnvelope {
    pub const fn fixed(length: u32) -> Self {
        Self::Known {
            minimum: length,
            maximum: length,
        }
    }

    pub const fn known(minimum: u32, maximum: u32) -> Self {
        Self::Known { minimum, maximum }
    }

    pub const fn bounds(&self) -> Option<(u32, u32)> {
        match self {
            Self::Known { minimum, maximum } => Some((*minimum, *maximum)),
            Self::Unresolved(_) => None,
        }
    }
}

/// Generated command-completion behavior and its completion-specific data.
///
/// Owning the return envelope here makes it impossible to construct Command
/// Status or asynchronous-event completions with a Command Complete return,
/// or a Command Complete without one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub enum CatalogCompletion {
    CommandComplete {
        returns: ExtractedEnvelope,
    },
    CommandStatus {},
    Event {
        code: u8,
    },
    /// Source value which cannot yet become a stable completion claim.
    Unresolved {
        expression: String,
    },
}

/// Scope-specific event evidence. Vendor ACI events always carry a payload
/// envelope; standard HCI and LE Meta events are inventory-only.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "scope")]
pub enum CatalogEventKind {
    VendorAci { payload: ExtractedEnvelope },
    StandardHci,
    LeMeta,
}

/// Scope-specific command identity. Vendor ACI commands use their OCF
/// namespace; standard HCI commands use a full opcode, from which OGF and OCF
/// are derived.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "scope")]
pub enum CatalogCommandKind {
    VendorAci { ocf: u16 },
    StandardHci { opcode: u16 },
}

impl CatalogCommandKind {
    pub const fn scope(&self) -> CommandScope {
        match self {
            Self::VendorAci { .. } => CommandScope::VendorAci,
            Self::StandardHci { .. } => CommandScope::StandardHci,
        }
    }

    pub const fn code(&self) -> u16 {
        match self {
            Self::VendorAci { ocf } => *ocf,
            Self::StandardHci { opcode } => *opcode,
        }
    }

    pub const fn ocf(&self) -> u16 {
        match self {
            Self::VendorAci { ocf } => *ocf,
            Self::StandardHci { opcode } => *opcode & 0x03ff,
        }
    }

    pub const fn ogf(&self) -> Option<u8> {
        match self {
            Self::VendorAci { .. } => None,
            Self::StandardHci { opcode } => Some((*opcode >> 10) as u8),
        }
    }

    pub const fn opcode(&self) -> Option<u16> {
        match self {
            Self::VendorAci { .. } => None,
            Self::StandardHci { opcode } => Some(*opcode),
        }
    }
}

impl CatalogEventKind {
    pub const fn scope(&self) -> EventScope {
        match self {
            Self::VendorAci { .. } => EventScope::VendorAci,
            Self::StandardHci => EventScope::StandardHci,
            Self::LeMeta => EventScope::LeMeta,
        }
    }

    pub const fn vendor_payload(&self) -> Option<&ExtractedEnvelope> {
        match self {
            Self::VendorAci { payload } => Some(payload),
            Self::StandardHci | Self::LeMeta => None,
        }
    }
}

/// One generated command declaration normalized from a family source adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogCommand {
    #[serde(flatten)]
    pub kind: CatalogCommandKind,
    pub name: String,
    pub source_name: String,
    pub source_offset: u32,
    pub completion: CatalogCompletion,
    pub request: ExtractedEnvelope,
}

impl CatalogCommand {
    pub const fn scope(&self) -> CommandScope {
        self.kind.scope()
    }

    pub const fn code(&self) -> u16 {
        self.kind.code()
    }

    pub const fn ocf(&self) -> u16 {
        self.kind.ocf()
    }

    pub const fn ogf(&self) -> Option<u8> {
        self.kind.ogf()
    }

    pub const fn opcode(&self) -> Option<u16> {
        self.kind.opcode()
    }
}

/// One generated event-table entry normalized from a family source adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogEvent {
    #[serde(flatten)]
    pub kind: CatalogEventKind,
    pub code: u16,
    pub name: String,
    pub source_name: String,
    pub source_offset: u32,
}

impl CatalogEvent {
    pub const fn scope(&self) -> EventScope {
        self.kind.scope()
    }

    pub const fn vendor_payload(&self) -> Option<&ExtractedEnvelope> {
        self.kind.vendor_payload()
    }
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
                command_scope_order(command.scope()),
                command.ocf(),
                command.name.clone(),
                command.source_name.clone(),
                command.source_offset,
            )
        });
        self.events.sort_by_key(|event| {
            (
                event_scope_order(event.scope()),
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
                .filter(|command| command.scope() == CommandScope::VendorAci)
                .map(|command| {
                    CoverageEntry::new(
                        command.ocf(),
                        &command.name,
                        CoverageOrigin::VendorAutoSource,
                    )
                })
                .collect(),
            events: self
                .events
                .iter()
                .filter(|event| event.scope() == EventScope::VendorAci)
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
                .filter(|command| command.scope() == CommandScope::StandardHci)
                .map(|command| {
                    CoverageEntry::new(
                        command.code(),
                        &command.name,
                        CoverageOrigin::StandardHciAutoSource,
                    )
                })
                .collect(),
            events: self
                .events
                .iter()
                .filter(|event| event.scope() == EventScope::StandardHci)
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
                .filter(|event| event.scope() == EventScope::LeMeta)
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
                kind: CatalogCommandKind::StandardHci { opcode: 0x2002 },
                name: "z_last".to_owned(),
                source_name: "z.c".to_owned(),
                source_offset: 9,
                completion: CatalogCompletion::CommandComplete {
                    returns: ExtractedEnvelope::Known {
                        minimum: 1,
                        maximum: 251,
                    },
                },
                request: ExtractedEnvelope::fixed(0),
            },
            CatalogCommand {
                kind: CatalogCommandKind::VendorAci { ocf: 1 },
                name: "a_first".to_owned(),
                source_name: "a.c".to_owned(),
                source_offset: 4,
                completion: CatalogCompletion::CommandStatus {},
                request: ExtractedEnvelope::Known {
                    minimum: 3,
                    maximum: 255,
                },
            },
        ]);
        schema.events.push(CatalogEvent {
            kind: CatalogEventKind::VendorAci {
                payload: ExtractedEnvelope::fixed(0),
            },
            code: 0x400,
            name: "gap_event".to_owned(),
            source_name: "ble_events.c".to_owned(),
            source_offset: 12,
        });
        schema.events.push(CatalogEvent {
            kind: CatalogEventKind::LeMeta,
            code: 0x01,
            name: "le_event".to_owned(),
            source_name: "ble_events.c".to_owned(),
            source_offset: 18,
        });
        schema.normalize();

        let value = serde_json::to_value(&schema).unwrap();
        assert_eq!(value["schema_version"], CATALOG_SCHEMA_VERSION);
        assert_eq!(value["family"], "stm32_wb");
        assert_eq!(value["commands"][0]["name"], "a_first");
        assert_eq!(value["commands"][0]["scope"], "vendor_aci");
        assert_eq!(value["commands"][0]["ocf"], 1);
        assert!(value["commands"][0].get("opcode").is_none());
        assert_eq!(value["commands"][1]["scope"], "standard_hci");
        assert_eq!(value["commands"][1]["opcode"], 0x2002);
        assert!(value["commands"][1].get("ocf").is_none());
        assert_eq!(schema.commands[1].ogf(), Some(8));
        assert_eq!(schema.commands[1].ocf(), 2);
        assert_eq!(value["events"][0]["scope"], "vendor_aci");
        assert_eq!(value["events"][1]["scope"], "le_meta");
        assert!(value["events"][1].get("payload").is_none());
        assert_eq!(
            value["commands"][0]["request"],
            serde_json::json!({
                "kind": "known",
                "value": {
                    "minimum": 3,
                    "maximum": 255,
                },
            })
        );
        assert_eq!(
            value["commands"][1]["completion"]["returns"],
            serde_json::json!({
                "kind": "known",
                "value": {
                    "minimum": 1,
                    "maximum": 251,
                },
            })
        );
        assert!(value["commands"][0].get("response").is_none());
        assert_eq!(value["commands"][0]["completion"]["kind"], "command_status");
        assert_eq!(
            value["commands"][1]["request"],
            serde_json::json!({
                "kind": "known",
                "value": {
                    "minimum": 0,
                    "maximum": 0,
                },
            })
        );
        assert_eq!(
            serde_json::from_value::<CatalogSchema>(value).unwrap(),
            schema
        );
        assert_eq!(
            serde_json::to_value(ExtractedEnvelope::Unresolved("computed_size".to_owned()))
                .unwrap(),
            serde_json::json!({
                "kind": "unresolved",
                "value": "computed_size",
            })
        );

        let mut unsupported = serde_json::to_value(&schema).unwrap();
        unsupported["schema_version"] = serde_json::json!(CATALOG_SCHEMA_VERSION + 1);
        assert!(serde_json::from_value::<CatalogSchema>(unsupported).is_err());
    }

    #[test]
    fn completion_deserialization_rejects_invalid_return_states() {
        let status_with_return = serde_json::json!({
            "kind": "command_status",
            "returns": {
                "kind": "known",
                "value": { "minimum": 0, "maximum": 0 },
            },
        });
        assert!(serde_json::from_value::<CatalogCompletion>(status_with_return).is_err());

        let complete_without_return = serde_json::json!({
            "kind": "command_complete",
        });
        assert!(serde_json::from_value::<CatalogCompletion>(complete_without_return).is_err());
    }
}
