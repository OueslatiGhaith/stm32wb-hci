//! Intermediate protocol catalog shared by source adapters.
//!
//! A family-specific adapter (currently STM32CubeWB C) owns parsing. It emits
//! this schema; coverage comparison, wire validation, JSON reporting, and a
//! future STM32CubeWBA adapter consume it. Keeping that boundary explicit
//! means parser changes do not silently change downstream assumptions.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::model::{CoverageEntry, CoverageOrigin, ProtocolCoverage, StandardHciCoverage};

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

/// A validated inclusive range of encoded wire lengths.
///
/// The private fields make an inverted envelope unrepresentable. Zero-length
/// requests and returns use `Envelope::fixed(0)`; there is no separate empty
/// representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct Envelope {
    minimum: u32,
    maximum: u32,
}

impl Envelope {
    pub const fn fixed(length: u32) -> Self {
        Self {
            minimum: length,
            maximum: length,
        }
    }

    pub const fn bounded(minimum: u32, maximum: u32) -> Self {
        assert!(minimum <= maximum, "wire envelope minimum exceeds maximum");
        Self { minimum, maximum }
    }

    pub const fn try_bounded(minimum: u32, maximum: u32) -> Option<Self> {
        if minimum <= maximum {
            Some(Self { minimum, maximum })
        } else {
            None
        }
    }

    pub const fn minimum(self) -> u32 {
        self.minimum
    }

    pub const fn maximum(self) -> u32 {
        self.maximum
    }

    pub const fn bounds(self) -> (u32, u32) {
        (self.minimum, self.maximum)
    }

    pub const fn is_fixed(self) -> bool {
        self.minimum == self.maximum
    }
}

impl<'de> Deserialize<'de> for Envelope {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawEnvelope {
            minimum: u32,
            maximum: u32,
        }

        let raw = RawEnvelope::deserialize(deserializer)?;
        Self::try_bounded(raw.minimum, raw.maximum).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "wire envelope minimum {} exceeds maximum {}",
                raw.minimum, raw.maximum
            ))
        })
    }
}

impl fmt::Display for Envelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_fixed() {
            write!(formatter, "{} bytes", self.maximum)
        } else {
            write!(formatter, "{}..={} bytes", self.minimum, self.maximum)
        }
    }
}

/// Evidence extracted from generated source.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum Evidence<T> {
    Known(T),
    /// Source expression which cannot yet become a stable wire envelope.
    Unresolved(String),
}

/// Extracted evidence for a validated wire envelope.
pub type WireLayoutEvidence = Evidence<WireLayout>;

/// One ordered component of an encoded payload.
///
/// Keeping fixed fields separate preserves field boundaries. Variable fields
/// retain their element width and valid cardinality instead of being flattened
/// into a single minimum/maximum byte count.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum WireSegment {
    Fixed {
        length: u32,
    },
    Variable {
        element_width: u32,
        minimum_elements: u32,
        maximum_elements: u32,
    },
}

impl WireSegment {
    pub const fn fixed(length: u32) -> Self {
        Self::Fixed { length }
    }

    pub const fn variable(
        element_width: u32,
        minimum_elements: u32,
        maximum_elements: u32,
    ) -> Self {
        assert!(
            element_width > 0,
            "variable wire elements must not be empty"
        );
        assert!(
            minimum_elements <= maximum_elements,
            "variable wire cardinality is inverted"
        );
        Self::Variable {
            element_width,
            minimum_elements,
            maximum_elements,
        }
    }

    const fn minimum_length(&self) -> Option<u32> {
        match self {
            Self::Fixed { length } => Some(*length),
            Self::Variable {
                element_width,
                minimum_elements,
                ..
            } => element_width.checked_mul(*minimum_elements),
        }
    }

    const fn maximum_length(&self) -> Option<u32> {
        match self {
            Self::Fixed { length } => Some(*length),
            Self::Variable {
                element_width,
                maximum_elements,
                ..
            } => element_width.checked_mul(*maximum_elements),
        }
    }
}

/// Complete ordered storage schema for one HCI parameter payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WireLayout {
    envelope: Envelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    segments: Option<Vec<WireSegment>>,
}

impl WireLayout {
    pub fn from_segments(segments: Vec<WireSegment>) -> Option<Self> {
        let segments = normalize_segments(segments)?;
        let minimum = segments.iter().try_fold(0_u32, |total, segment| {
            total.checked_add(segment.minimum_length()?)
        })?;
        let maximum = segments.iter().try_fold(0_u32, |total, segment| {
            total.checked_add(segment.maximum_length()?)
        })?;
        Some(Self {
            envelope: Envelope::bounded(minimum, maximum),
            segments: Some(segments),
        })
    }

    pub fn fixed(length: u32) -> Self {
        Self {
            envelope: Envelope::fixed(length),
            segments: None,
        }
    }

    pub fn with_envelope(envelope: Envelope, segments: Vec<WireSegment>) -> Option<Self> {
        let storage = Self::from_segments(segments)?;
        (storage.envelope.minimum() == envelope.minimum()
            && storage.envelope.maximum() >= envelope.maximum())
        .then_some(Self {
            envelope,
            segments: storage.segments,
        })
    }

    pub fn byte_capacity(minimum: u32, maximum: u32) -> Self {
        assert!(minimum <= maximum, "wire envelope minimum exceeds maximum");
        Self {
            envelope: Envelope::bounded(minimum, maximum),
            segments: None,
        }
    }

    pub const fn envelope(&self) -> Envelope {
        self.envelope
    }

    pub fn segments(&self) -> Option<&[WireSegment]> {
        self.segments.as_deref()
    }

    pub fn into_segments(self) -> Option<Vec<WireSegment>> {
        self.segments
    }

    fn validate(&self) -> bool {
        let Some(segments) = &self.segments else {
            return true;
        };
        if segments.iter().any(|segment| match segment {
            WireSegment::Fixed { .. } => false,
            WireSegment::Variable {
                element_width,
                minimum_elements,
                maximum_elements,
            } => *element_width == 0 || minimum_elements > maximum_elements,
        }) {
            return false;
        }
        Self::from_segments(segments.clone()).is_some_and(|storage| {
            storage.envelope.minimum() == self.envelope.minimum()
                && storage.envelope.maximum() >= self.envelope.maximum()
        })
    }
}

fn normalize_segments(segments: Vec<WireSegment>) -> Option<Vec<WireSegment>> {
    let mut normalized = Vec::<WireSegment>::new();
    for segment in segments {
        match (normalized.last_mut(), segment) {
            (Some(WireSegment::Fixed { length: previous }), WireSegment::Fixed { length }) => {
                *previous = previous.checked_add(length)?
            }
            (_, WireSegment::Fixed { length: 0 }) => {}
            (_, segment) => normalized.push(segment),
        }
    }
    Some(normalized)
}

impl PartialEq<Envelope> for WireLayout {
    fn eq(&self, other: &Envelope) -> bool {
        self.envelope == *other
    }
}

impl PartialEq<WireLayout> for Envelope {
    fn eq(&self, other: &WireLayout) -> bool {
        *self == other.envelope
    }
}

impl Evidence<WireLayout> {
    pub fn fixed(length: u32) -> Self {
        Self::Known(WireLayout::fixed(length))
    }

    pub fn known(minimum: u32, maximum: u32) -> Self {
        Self::Known(WireLayout::byte_capacity(minimum, maximum))
    }

    pub const fn bounds(&self) -> Option<(u32, u32)> {
        match self {
            Self::Known(layout) => Some(layout.envelope().bounds()),
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
        returns: WireLayoutEvidence,
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
    VendorAci { payload: WireLayoutEvidence },
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

    pub const fn vendor_payload(&self) -> Option<&WireLayoutEvidence> {
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
    pub request: WireLayoutEvidence,
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

    pub const fn vendor_payload(&self) -> Option<&WireLayoutEvidence> {
        self.kind.vendor_payload()
    }
}

/// Stable, normalized result of parsing one immutable firmware source tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSchema {
    pub family: CatalogFamily,
    pub cube_tag: String,
    pub commands: Vec<CatalogCommand>,
    pub events: Vec<CatalogEvent>,
}

impl Serialize for CatalogSchema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;

        #[derive(Serialize)]
        struct CatalogSchemaRef<'a> {
            family: CatalogFamily,
            cube_tag: &'a str,
            commands: &'a [CatalogCommand],
            events: &'a [CatalogEvent],
        }

        CatalogSchemaRef {
            family: self.family,
            cube_tag: &self.cube_tag,
            commands: &self.commands,
            events: &self.events,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CatalogSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawCatalogSchema {
            family: CatalogFamily,
            cube_tag: String,
            commands: Vec<CatalogCommand>,
            events: Vec<CatalogEvent>,
        }

        let raw = RawCatalogSchema::deserialize(deserializer)?;
        let schema = Self {
            family: raw.family,
            cube_tag: raw.cube_tag,
            commands: raw.commands,
            events: raw.events,
        };
        schema.validate().map_err(serde::de::Error::custom)?;
        Ok(schema)
    }
}

impl CatalogSchema {
    pub(crate) fn new(family: CatalogFamily, cube_tag: impl Into<String>) -> Self {
        Self {
            family,
            cube_tag: cube_tag.into(),
            commands: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Normalize ordering once at the adapter boundary so a serialized catalog
    /// is deterministic and all downstream consumers see the same ordering.
    pub(crate) fn normalize(&mut self) -> Result<(), String> {
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
        self.validate()
    }

    /// Validate all cross-entry invariants at the catalog boundary.
    pub fn validate(&self) -> Result<(), String> {
        let mut command_codes = BTreeMap::new();
        let mut command_names = BTreeMap::new();
        for command in &self.commands {
            if let CatalogCommandKind::VendorAci { ocf } = command.kind
                && ocf > 0x03ff
            {
                return Err(format!(
                    "vendor command {} has OCF 0x{ocf:X}, which exceeds ten bits",
                    command.name
                ));
            }
            validate_evidence(&command.request, "command request", &command.name)?;
            if let CatalogCompletion::CommandComplete { returns } = &command.completion {
                validate_evidence(returns, "command return", &command.name)?;
            }

            let key = (command.scope(), command.code());
            if let Some(previous) = command_codes.insert(key, command.name.as_str()) {
                return Err(format!(
                    "duplicate command ({:?}, 0x{:04X}): {previous} and {}",
                    command.scope(),
                    command.code(),
                    command.name
                ));
            }
            let name_key = (command.scope(), command.name.as_str());
            if let Some(previous) = command_names.insert(name_key, command.code()) {
                return Err(format!(
                    "command name {} is inconsistent in {:?}: 0x{previous:04X} and 0x{:04X}",
                    command.name,
                    command.scope(),
                    command.code()
                ));
            }
        }

        let mut event_codes = BTreeMap::new();
        let mut event_names = BTreeMap::new();
        for event in &self.events {
            if matches!(event.scope(), EventScope::StandardHci | EventScope::LeMeta)
                && event.code > u16::from(u8::MAX)
            {
                return Err(format!(
                    "standard event {} has code 0x{:X}, which exceeds eight bits",
                    event.name, event.code
                ));
            }
            if let Some(payload) = event.vendor_payload() {
                validate_evidence(payload, "event payload", &event.name)?;
            }

            let key = (event.scope(), event.code);
            if let Some(previous) = event_codes.insert(key, event.name.as_str()) {
                return Err(format!(
                    "duplicate event ({:?}, 0x{:04X}): {previous} and {}",
                    event.scope(),
                    event.code,
                    event.name
                ));
            }
            let name_key = (event.scope(), event.name.as_str());
            if let Some(previous) = event_names.insert(name_key, event.code) {
                return Err(format!(
                    "event name {} is inconsistent in {:?}: 0x{previous:04X} and 0x{:04X}",
                    event.name,
                    event.scope(),
                    event.code
                ));
            }
        }
        Ok(())
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

fn validate_evidence(evidence: &WireLayoutEvidence, label: &str, name: &str) -> Result<(), String> {
    if let Evidence::Known(layout) = evidence {
        if layout.envelope().minimum() > layout.envelope().maximum() {
            return Err(format!("inverted {label} envelope for {name}"));
        }
        if !layout.validate() {
            return Err(format!("inconsistent {label} wire schema for {name}"));
        }
    }
    Ok(())
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
    fn schema_serialization_is_validated_and_deterministic() {
        let mut schema = CatalogSchema::new(CatalogFamily::Stm32Wb, "v1.17.1");
        schema.commands.extend([
            CatalogCommand {
                kind: CatalogCommandKind::StandardHci { opcode: 0x2002 },
                name: "z_last".to_owned(),
                source_name: "z.c".to_owned(),
                source_offset: 9,
                completion: CatalogCompletion::CommandComplete {
                    returns: WireLayoutEvidence::known(1, 251),
                },
                request: WireLayoutEvidence::fixed(0),
            },
            CatalogCommand {
                kind: CatalogCommandKind::VendorAci { ocf: 1 },
                name: "a_first".to_owned(),
                source_name: "a.c".to_owned(),
                source_offset: 4,
                completion: CatalogCompletion::CommandStatus {},
                request: WireLayoutEvidence::known(3, 255),
            },
        ]);
        schema.events.push(CatalogEvent {
            kind: CatalogEventKind::VendorAci {
                payload: WireLayoutEvidence::fixed(0),
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
        schema.normalize().unwrap();

        let value = serde_json::to_value(&schema).unwrap();
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
                    "envelope": {
                        "minimum": 3,
                        "maximum": 255,
                    },
                },
            })
        );
        assert_eq!(
            value["commands"][1]["completion"]["returns"],
            serde_json::json!({
                "kind": "known",
                "value": {
                    "envelope": {
                        "minimum": 1,
                        "maximum": 251,
                    },
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
                    "envelope": {
                        "minimum": 0,
                        "maximum": 0,
                    },
                },
            })
        );
        assert_eq!(
            serde_json::from_value::<CatalogSchema>(value).unwrap(),
            schema
        );
        assert_eq!(
            serde_json::to_value(WireLayoutEvidence::Unresolved("computed_size".to_owned()))
                .unwrap(),
            serde_json::json!({
                "kind": "unresolved",
                "value": "computed_size",
            })
        );
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

    #[test]
    fn catalog_boundary_rejects_invalid_envelopes_and_identities() {
        let command = |ocf, name: &str| CatalogCommand {
            kind: CatalogCommandKind::VendorAci { ocf },
            name: name.to_owned(),
            source_name: "fixture.c".to_owned(),
            source_offset: 0,
            completion: CatalogCompletion::CommandComplete {
                returns: WireLayoutEvidence::fixed(0),
            },
            request: WireLayoutEvidence::fixed(0),
        };
        let event = |kind, code, name: &str| CatalogEvent {
            kind,
            code,
            name: name.to_owned(),
            source_name: "events.c".to_owned(),
            source_offset: 0,
        };

        let inverted = serde_json::json!({
            "minimum": 4,
            "maximum": 3,
        });
        assert!(
            serde_json::from_value::<Envelope>(inverted)
                .unwrap_err()
                .to_string()
                .contains("minimum 4 exceeds maximum 3")
        );

        let mut schema = CatalogSchema::new(CatalogFamily::Stm32Wb, "v1.17.1");
        schema
            .commands
            .extend([command(1, "First"), command(1, "DuplicateCode")]);
        assert!(schema.validate().unwrap_err().contains("duplicate command"));
        assert!(serde_json::to_value(&schema).is_err());

        schema.commands = vec![command(1, "First")];
        let mut serialized = serde_json::to_value(&schema).unwrap();
        let duplicate = serialized["commands"][0].clone();
        serialized["commands"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(
            serde_json::from_value::<CatalogSchema>(serialized)
                .unwrap_err()
                .to_string()
                .contains("duplicate command")
        );

        schema.commands = vec![command(0x0400, "WideVendorOcf")];
        assert!(schema.validate().unwrap_err().contains("exceeds ten bits"));

        schema.commands = vec![command(1, "RepeatedName"), command(2, "RepeatedName")];
        assert!(
            schema
                .validate()
                .unwrap_err()
                .contains("command name RepeatedName is inconsistent")
        );

        schema.commands.clear();
        schema.events = vec![event(CatalogEventKind::StandardHci, 0x0100, "WideEvent")];
        assert!(
            schema
                .validate()
                .unwrap_err()
                .contains("exceeds eight bits")
        );

        schema.events = vec![
            event(CatalogEventKind::LeMeta, 1, "FirstEvent"),
            event(CatalogEventKind::LeMeta, 1, "DuplicateEvent"),
        ];
        assert!(schema.validate().unwrap_err().contains("duplicate event"));

        schema.events = vec![
            event(CatalogEventKind::LeMeta, 1, "RepeatedEvent"),
            event(CatalogEventKind::LeMeta, 2, "RepeatedEvent"),
        ];
        assert!(
            schema
                .validate()
                .unwrap_err()
                .contains("event name RepeatedEvent is inconsistent")
        );
    }
}
