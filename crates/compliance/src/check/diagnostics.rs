//! Internal grouped diagnostics produced by coverage rules.
//!
//! These types model checker output by rule area. The CLI JSON schema is still
//! flattened by `report`, but rule execution no longer needs to build that
//! horizontal schema directly.

use super::MarkerLocation;
use serde::Serialize;

/// Complete grouped diagnostics produced by the compliance rules.
#[derive(Debug)]
pub(super) struct CoverageDiagnostics {
    /// Firmware tag or worktree label used to build the ST command spec.
    pub(super) firmware: String,
    /// Rust crate directory that was checked.
    pub(super) rust_crate: String,
    /// Top-level command coverage counters.
    pub(super) totals: CoverageTotals,
    /// Command marker, opcode, and method diagnostics grouped by rule area.
    pub(super) commands: CommandDiagnostics,
    /// Vendor event and command-complete return diagnostics.
    pub(super) events: EventDiagnostics,
}

/// Top-level command coverage counters.
#[derive(Debug)]
pub(super) struct CoverageTotals {
    pub(super) commands_total: usize,
    pub(super) rust_opcode_constants_total: usize,
    pub(super) markers_total: usize,
    pub(super) alias_markers_total: usize,
    pub(super) covered_by_marker: usize,
}

/// Command-side diagnostics grouped by rule area.
#[derive(Debug, Default)]
pub(super) struct CommandDiagnostics {
    pub(super) marker_coverage: CommandMarkerCoverageDiagnostics,
    pub(super) marker_validity: CommandMarkerValidityDiagnostics,
    pub(super) opcode_consistency: CommandOpcodeDiagnostics,
    pub(super) method_coverage: CommandMethodDiagnostics,
}

/// Marker presence diagnostics for firmware commands.
#[derive(Debug, Default)]
pub(super) struct CommandMarkerCoverageDiagnostics {
    pub(super) missing_markers: Vec<MissingMarker>,
    pub(super) duplicate_markers: Vec<DuplicateMarker>,
}

/// Marker validity diagnostics against the extracted firmware spec.
#[derive(Debug, Default)]
pub(super) struct CommandMarkerValidityDiagnostics {
    pub(super) unknown_markers: Vec<UnknownMarker>,
    pub(super) unknown_alias_markers: Vec<UnknownAliasMarker>,
}

/// Opcode consistency diagnostics for marked command implementations.
#[derive(Debug, Default)]
pub(super) struct CommandOpcodeDiagnostics {
    pub(super) marker_opcode_constants_missing: Vec<MarkerOpcodeConstantMissing>,
    pub(super) method_opcode_missing: Vec<MethodOpcodeMissing>,
    pub(super) method_opcode_mismatches: Vec<MethodOpcodeMismatch>,
}

/// Method coverage diagnostics for markers and Rust command trait methods.
#[derive(Debug, Default)]
pub(super) struct CommandMethodDiagnostics {
    pub(super) marker_method_missing: Vec<MarkerMethodMissing>,
    pub(super) rust_methods_without_marker: Vec<RustMethodWithoutMarker>,
}

/// Event-side diagnostics grouped by rule area.
#[derive(Debug, Default)]
pub(super) struct EventDiagnostics {
    pub(super) totals: EventTotals,
    pub(super) marker_coverage: EventMarkerCoverageDiagnostics,
    pub(super) marker_validity: EventMarkerValidityDiagnostics,
    pub(super) variant_coverage: EventVariantDiagnostics,
    pub(super) return_coverage: EventReturnDiagnostics,
}

/// Event coverage counters.
#[derive(Debug, Default)]
pub(super) struct EventTotals {
    pub(super) vendor_events_total: usize,
    pub(super) rust_vendor_event_variants_total: usize,
    pub(super) rust_vendor_event_handlers_total: usize,
    pub(super) vendor_event_markers_total: usize,
    pub(super) command_complete_events_total: usize,
    pub(super) rust_vendor_return_handlers_total: usize,
}

/// Marker presence diagnostics for vendor events.
#[derive(Debug, Default)]
pub(super) struct EventMarkerCoverageDiagnostics {
    pub(super) missing_vendor_event_markers: Vec<MissingVendorEventMarker>,
    pub(super) duplicate_vendor_event_markers: Vec<DuplicateVendorEventMarker>,
}

/// Marker validity diagnostics for vendor event markers.
#[derive(Debug, Default)]
pub(super) struct EventMarkerValidityDiagnostics {
    pub(super) unknown_vendor_event_markers: Vec<UnknownVendorEventMarker>,
}

/// Variant declaration and dispatch diagnostics for vendor events.
#[derive(Debug, Default)]
pub(super) struct EventVariantDiagnostics {
    pub(super) missing_vendor_event_variants: Vec<MissingVendorEventVariant>,
    pub(super) missing_vendor_event_handlers: Vec<MissingVendorEventHandler>,
}

/// Command-complete return handler diagnostics.
#[derive(Debug, Default)]
pub(super) struct EventReturnDiagnostics {
    pub(super) missing_vendor_return_handlers: Vec<MissingVendorReturnHandler>,
}

/// Firmware command that is not represented by a Rust compliance marker.
#[derive(Debug, Serialize)]
pub struct MissingMarker {
    pub(super) st_command: String,
    pub(super) opcode: Option<u16>,
    pub(super) expected_rust_places: Vec<String>,
}

/// Firmware command that is represented by multiple primary Rust markers.
#[derive(Debug, Serialize)]
pub struct DuplicateMarker {
    pub(super) st_command: String,
    pub(super) locations: Vec<MarkerLocation>,
}

/// Rust primary marker that does not match any extracted ST command.
#[derive(Debug, Serialize)]
pub struct UnknownMarker {
    pub(super) st_command: String,
    pub(super) method: Option<String>,
    pub(super) location: MarkerLocation,
}

/// Rust alias marker whose target does not match any extracted ST command.
#[derive(Debug, Serialize)]
pub struct UnknownAliasMarker {
    pub(super) alias_of: String,
    pub(super) method: Option<String>,
    pub(super) location: MarkerLocation,
}

/// Marked command whose firmware opcode cannot be resolved to a Rust constant.
#[derive(Debug, Serialize)]
pub struct MarkerOpcodeConstantMissing {
    pub(super) st_command: String,
    pub(super) opcode: Option<u16>,
    pub(super) method: Option<String>,
    pub(super) location: MarkerLocation,
}

/// Marked method whose implementation does not reference any opcode constant.
#[derive(Debug, Serialize)]
pub struct MethodOpcodeMissing {
    pub(super) st_command: String,
    pub(super) expected_opcode_const: String,
    pub(super) method: Option<String>,
    pub(super) location: MarkerLocation,
}

/// Marker that could not be associated with a following Rust method.
#[derive(Debug, Serialize)]
pub struct MarkerMethodMissing {
    pub(super) st_command: String,
    pub(super) location: MarkerLocation,
}

/// Marked method whose implementation references the wrong opcode constant.
#[derive(Debug, Serialize)]
pub struct MethodOpcodeMismatch {
    pub(super) st_command: String,
    pub(super) expected_opcode_const: String,
    pub(super) actual_opcode_const: String,
    pub(super) method: String,
    pub(super) location: MarkerLocation,
}

/// Rust command trait method that has no compliance marker.
#[derive(Debug, Serialize)]
pub struct RustMethodWithoutMarker {
    pub(super) method: String,
    pub(super) location: MarkerLocation,
}

/// ST vendor event that is not represented by a Rust event marker.
#[derive(Debug, Serialize)]
pub struct MissingVendorEventMarker {
    pub(super) st_event: String,
}

/// ST vendor event that is represented by multiple Rust event markers.
#[derive(Debug, Serialize)]
pub struct DuplicateVendorEventMarker {
    pub(super) st_event: String,
    pub(super) locations: Vec<MarkerLocation>,
}

/// Rust event marker that does not match any extracted ST event.
#[derive(Debug, Serialize)]
pub struct UnknownVendorEventMarker {
    pub(super) st_event: String,
    pub(super) variant: Option<String>,
    pub(super) location: MarkerLocation,
}

/// ST vendor event whose expected Rust variant is missing.
#[derive(Debug, Serialize)]
pub struct MissingVendorEventVariant {
    pub(super) st_event: String,
}

/// ST vendor event whose Rust variant is not constructed by the decoder.
#[derive(Debug, Serialize)]
pub struct MissingVendorEventHandler {
    pub(super) st_event: String,
    pub(super) expected_variant: String,
}

/// ST command-complete command whose opcode is not decoded as vendor returns.
#[derive(Debug, Serialize)]
pub struct MissingVendorReturnHandler {
    pub(super) st_command: String,
    pub(super) expected_opcode_const: String,
}
