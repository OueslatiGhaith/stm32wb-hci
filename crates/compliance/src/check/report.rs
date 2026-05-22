//! Serializable coverage report types.
//!
//! These types define the CLI/CI JSON surface. The checker rules construct
//! these values, but parsing and lookup concerns live in sibling modules.

use super::MarkerLocation;
use serde::Serialize;

/// Full JSON report produced by the `check` subcommand.
///
/// Empty diagnostic vectors mean the Rust crate is in sync with the extracted
/// ST command set for the currently selected firmware tag.
#[derive(Debug, Serialize)]
pub struct CoverageReport {
    /// Firmware tag or worktree label used to build the ST command spec.
    pub firmware: String,
    /// Rust crate directory that was checked.
    pub rust_crate: String,
    /// Number of ST commands extracted from firmware sources.
    pub commands_total: usize,
    /// Number of Rust vendor opcode constants discovered.
    pub rust_opcode_constants_total: usize,
    /// Number of primary `compliance: st=...` markers found in Rust commands.
    pub markers_total: usize,
    /// Number of `compliance: alias_of=...` markers found in Rust commands.
    pub alias_markers_total: usize,
    /// Number of ST commands that have at least one primary marker.
    pub covered_by_marker: usize,
    /// ST commands that do not have a corresponding Rust marker.
    pub missing_markers: Vec<MissingMarker>,
    /// ST commands that are claimed by more than one primary marker.
    pub duplicate_markers: Vec<DuplicateMarker>,
    /// Primary markers whose ST command is not present in the firmware spec.
    pub unknown_markers: Vec<UnknownMarker>,
    /// Alias markers whose target ST command is not present in the firmware spec.
    pub unknown_alias_markers: Vec<UnknownAliasMarker>,
    /// Marked commands whose ST opcode has no matching Rust opcode constant.
    pub marker_opcode_constants_missing: Vec<MarkerOpcodeConstantMissing>,
    /// Markers that were not close enough to a Rust method to attach to it.
    pub marker_method_missing: Vec<MarkerMethodMissing>,
    /// Marked methods where no opcode use was found in the implementation.
    pub method_opcode_missing: Vec<MethodOpcodeMissing>,
    /// Marked methods whose implementation uses a different opcode constant.
    pub method_opcode_mismatches: Vec<MethodOpcodeMismatch>,
    /// Rust command trait methods that have no primary or alias marker.
    pub rust_methods_without_marker: Vec<RustMethodWithoutMarker>,
    /// Vendor event and command-complete return coverage.
    pub events: EventCoverageReport,
}

/// Firmware command that is not represented by a Rust compliance marker.
#[derive(Debug, Serialize)]
pub struct MissingMarker {
    pub st_command: String,
    pub opcode: Option<u16>,
    pub expected_rust_places: Vec<String>,
}

/// Firmware command that is represented by multiple primary Rust markers.
#[derive(Debug, Serialize)]
pub struct DuplicateMarker {
    pub st_command: String,
    pub locations: Vec<MarkerLocation>,
}

/// Rust primary marker that does not match any extracted ST command.
#[derive(Debug, Serialize)]
pub struct UnknownMarker {
    pub st_command: String,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

/// Rust alias marker whose target does not match any extracted ST command.
#[derive(Debug, Serialize)]
pub struct UnknownAliasMarker {
    pub alias_of: String,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

/// Marked command whose firmware opcode cannot be resolved to a Rust constant.
#[derive(Debug, Serialize)]
pub struct MarkerOpcodeConstantMissing {
    pub st_command: String,
    pub opcode: Option<u16>,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

/// Marked method whose implementation does not reference any opcode constant.
#[derive(Debug, Serialize)]
pub struct MethodOpcodeMissing {
    pub st_command: String,
    pub expected_opcode_const: String,
    pub method: Option<String>,
    pub location: MarkerLocation,
}

/// Marker that could not be associated with a following Rust method.
#[derive(Debug, Serialize)]
pub struct MarkerMethodMissing {
    pub st_command: String,
    pub location: MarkerLocation,
}

/// Marked method whose implementation references the wrong opcode constant.
#[derive(Debug, Serialize)]
pub struct MethodOpcodeMismatch {
    pub st_command: String,
    pub expected_opcode_const: String,
    pub actual_opcode_const: String,
    pub method: String,
    pub location: MarkerLocation,
}

/// Rust command trait method that has no compliance marker.
#[derive(Debug, Serialize)]
pub struct RustMethodWithoutMarker {
    pub method: String,
    pub location: MarkerLocation,
}

/// Event coverage diagnostics for Rust vendor event decoding.
#[derive(Debug, Serialize)]
pub struct EventCoverageReport {
    /// Number of ACI event prototypes extracted from ST `ble_events.h`.
    pub vendor_events_total: usize,
    /// Number of Rust `VendorEvent` variants declared.
    pub rust_vendor_event_variants_total: usize,
    /// Number of Rust `VendorEvent::new` dispatch arms discovered.
    pub rust_vendor_event_handlers_total: usize,
    /// Number of explicit `compliance: event=...` markers found on Rust variants.
    pub vendor_event_markers_total: usize,
    /// Number of ST command-complete vendor commands.
    pub command_complete_events_total: usize,
    /// Number of Rust vendor command-complete opcode handlers discovered.
    pub rust_vendor_return_handlers_total: usize,
    /// ST vendor events with no explicit Rust event marker.
    pub missing_vendor_event_markers: Vec<MissingVendorEventMarker>,
    /// ST vendor events claimed by more than one explicit Rust event marker.
    pub duplicate_vendor_event_markers: Vec<DuplicateVendorEventMarker>,
    /// Rust event markers whose ST event is not present in the firmware spec.
    pub unknown_vendor_event_markers: Vec<UnknownVendorEventMarker>,
    /// ST vendor events with no corresponding Rust `VendorEvent` variant.
    pub missing_vendor_event_variants: Vec<MissingVendorEventVariant>,
    /// ST vendor events with a Rust variant but no decode dispatch arm.
    pub missing_vendor_event_handlers: Vec<MissingVendorEventHandler>,
    /// Command-complete vendor commands with no Rust return-parameter handler.
    pub missing_vendor_return_handlers: Vec<MissingVendorReturnHandler>,
}

/// ST vendor event that is not represented by a Rust event marker.
#[derive(Debug, Serialize)]
pub struct MissingVendorEventMarker {
    pub st_event: String,
}

/// ST vendor event that is represented by multiple Rust event markers.
#[derive(Debug, Serialize)]
pub struct DuplicateVendorEventMarker {
    pub st_event: String,
    pub locations: Vec<MarkerLocation>,
}

/// Rust event marker that does not match any extracted ST event.
#[derive(Debug, Serialize)]
pub struct UnknownVendorEventMarker {
    pub st_event: String,
    pub variant: Option<String>,
    pub location: MarkerLocation,
}

/// ST vendor event whose expected Rust variant is missing.
#[derive(Debug, Serialize)]
pub struct MissingVendorEventVariant {
    pub st_event: String,
}

/// ST vendor event whose Rust variant is not constructed by the decoder.
#[derive(Debug, Serialize)]
pub struct MissingVendorEventHandler {
    pub st_event: String,
    pub expected_variant: String,
}

/// ST command-complete command whose opcode is not decoded as vendor returns.
#[derive(Debug, Serialize)]
pub struct MissingVendorReturnHandler {
    pub st_command: String,
    pub expected_opcode_const: String,
}
