//! Serializable coverage report types.
//!
//! These types define the CLI/CI JSON surface. Coverage rules produce grouped
//! diagnostics internally; this module flattens those groups into the existing
//! report schema at the serialization edge.

use super::diagnostics::{
    CoverageDiagnostics, DuplicateMarker, DuplicateVendorEventMarker, MarkerMethodMissing,
    MarkerOpcodeConstantMissing, MethodOpcodeMismatch, MethodOpcodeMissing, MissingMarker,
    MissingVendorEventHandler, MissingVendorEventMarker, MissingVendorEventVariant,
    MissingVendorReturnHandler, RustMethodWithoutMarker, UnknownAliasMarker, UnknownMarker,
    UnknownVendorEventMarker,
};
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

impl CoverageReport {
    /// Flattens grouped rule diagnostics into the stable JSON report schema.
    pub(super) fn from_diagnostics(diagnostics: CoverageDiagnostics) -> Self {
        let commands = diagnostics.commands;
        Self {
            firmware: diagnostics.firmware,
            rust_crate: diagnostics.rust_crate,
            commands_total: diagnostics.totals.commands_total,
            rust_opcode_constants_total: diagnostics.totals.rust_opcode_constants_total,
            markers_total: diagnostics.totals.markers_total,
            alias_markers_total: diagnostics.totals.alias_markers_total,
            covered_by_marker: diagnostics.totals.covered_by_marker,
            missing_markers: commands.marker_coverage.missing_markers,
            duplicate_markers: commands.marker_coverage.duplicate_markers,
            unknown_markers: commands.marker_validity.unknown_markers,
            unknown_alias_markers: commands.marker_validity.unknown_alias_markers,
            marker_opcode_constants_missing: commands
                .opcode_consistency
                .marker_opcode_constants_missing,
            marker_method_missing: commands.method_coverage.marker_method_missing,
            method_opcode_missing: commands.opcode_consistency.method_opcode_missing,
            method_opcode_mismatches: commands.opcode_consistency.method_opcode_mismatches,
            rust_methods_without_marker: commands.method_coverage.rust_methods_without_marker,
            events: EventCoverageReport::from_diagnostics(diagnostics.events),
        }
    }
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

impl EventCoverageReport {
    /// Flattens grouped event diagnostics into the stable JSON event schema.
    fn from_diagnostics(diagnostics: super::diagnostics::EventDiagnostics) -> Self {
        Self {
            vendor_events_total: diagnostics.totals.vendor_events_total,
            rust_vendor_event_variants_total: diagnostics.totals.rust_vendor_event_variants_total,
            rust_vendor_event_handlers_total: diagnostics.totals.rust_vendor_event_handlers_total,
            vendor_event_markers_total: diagnostics.totals.vendor_event_markers_total,
            command_complete_events_total: diagnostics.totals.command_complete_events_total,
            rust_vendor_return_handlers_total: diagnostics.totals.rust_vendor_return_handlers_total,
            missing_vendor_event_markers: diagnostics.marker_coverage.missing_vendor_event_markers,
            duplicate_vendor_event_markers: diagnostics
                .marker_coverage
                .duplicate_vendor_event_markers,
            unknown_vendor_event_markers: diagnostics.marker_validity.unknown_vendor_event_markers,
            missing_vendor_event_variants: diagnostics
                .variant_coverage
                .missing_vendor_event_variants,
            missing_vendor_event_handlers: diagnostics
                .variant_coverage
                .missing_vendor_event_handlers,
            missing_vendor_return_handlers: diagnostics
                .return_coverage
                .missing_vendor_return_handlers,
        }
    }
}
