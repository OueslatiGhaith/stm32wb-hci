//! Coverage comparison between extracted ST commands and Rust command code.
//!
//! The functions in this module assume parsing has already produced a firmware
//! spec, Rust opcode constants, command markers, and method implementations.
//! The result is a serializable report for CLI/CI consumption.

use super::MarkerLocation;
use super::rust_event::{EventMarker, RustEventCoverage, load_rust_event_coverage};
use super::rust_marker::{AliasMarker, CommandMarker, load_command_markers};
use super::rust_method::load_rust_command_methods;
use super::rust_method::{RustCommandMethod, RustMethodImplementation};
use super::rust_opcode::{RustOpcode, parse_rust_opcodes};
use crate::spec::{CommandSpec, EventSpec, FirmwareSpec};
use anyhow::Result;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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

/// Firmware-side lookup surface used by coverage rules.
struct FirmwareIndex<'a> {
    spec: &'a FirmwareSpec,
    commands: &'a [CommandSpec],
    command_names: HashSet<String>,
    vendor_events: Vec<&'a EventSpec>,
    vendor_event_names: HashSet<String>,
    command_complete_commands: Vec<&'a CommandSpec>,
}

impl<'a> FirmwareIndex<'a> {
    fn new(spec: &'a FirmwareSpec) -> Self {
        let commands = spec.commands.as_slice();
        Self {
            spec,
            commands,
            command_names: commands.iter().map(formal_st_name).collect(),
            vendor_events: spec
                .events
                .iter()
                .filter(|event| event.name.starts_with("aci_"))
                .collect(),
            vendor_event_names: spec
                .events
                .iter()
                .filter(|event| event.name.starts_with("aci_"))
                .map(|event| formal_event_name(&event.name))
                .collect(),
            command_complete_commands: commands
                .iter()
                .filter(|command| command.event != Some(0x0f))
                .collect(),
        }
    }

    fn has_command(&self, st_command: &str) -> bool {
        self.command_names.contains(st_command)
    }

    fn has_vendor_event(&self, st_event: &str) -> bool {
        self.vendor_event_names.contains(st_event)
    }
}

/// Rust command-side lookup surface used by coverage rules.
struct RustCommandIndex {
    opcodes: Vec<RustOpcode>,
    opcode_const_by_value: HashMap<u16, String>,
    markers: Vec<CommandMarker>,
    alias_markers: Vec<AliasMarker>,
    methods: Vec<RustCommandMethod>,
    method_impls: HashMap<(String, String), RustMethodImplementation>,
    marked_methods: HashSet<(String, String)>,
    markers_by_st: HashMap<String, Vec<usize>>,
}

impl RustCommandIndex {
    fn load(rust_crate: &Path) -> Result<Self> {
        let opcode_path = rust_crate.join("src/vendor/opcode.rs");
        let opcodes = parse_rust_opcodes(&opcode_path)?;
        let opcode_const_by_value = opcodes
            .iter()
            .map(|opcode| (opcode.opcode, opcode.name.clone()))
            .collect();
        let loaded_markers = load_command_markers(rust_crate)?;
        let markers = loaded_markers.primary;
        let alias_markers = loaded_markers.aliases;
        let methods = load_rust_command_methods(rust_crate)?;
        let method_impls = super::rust_method::load_rust_method_implementations(rust_crate)?;
        let marked_methods = command_marked_methods(&markers, &alias_markers);
        let markers_by_st = command_markers_by_st(&markers);

        Ok(Self {
            opcodes,
            opcode_const_by_value,
            markers,
            alias_markers,
            methods,
            method_impls,
            marked_methods,
            markers_by_st,
        })
    }

    fn opcode_const(&self, opcode: u16) -> Option<&str> {
        self.opcode_const_by_value.get(&opcode).map(String::as_str)
    }

    fn markers_for(&self, st_command: &str) -> Option<Vec<&CommandMarker>> {
        self.markers_by_st.get(st_command).map(|indices| {
            indices
                .iter()
                .map(|idx| &self.markers[*idx])
                .collect::<Vec<_>>()
        })
    }
}

/// Rust event-side lookup surface used by coverage rules.
struct RustEventIndex {
    coverage: RustEventCoverage,
    markers_by_st: HashMap<String, Vec<usize>>,
}

impl RustEventIndex {
    fn load(rust_crate: &Path) -> Result<Self> {
        let coverage = load_rust_event_coverage(rust_crate)?;
        let markers_by_st = event_markers_by_st(&coverage.vendor_event_markers);
        Ok(Self {
            coverage,
            markers_by_st,
        })
    }

    fn markers_for(&self, st_event: &str) -> Option<Vec<&EventMarker>> {
        self.markers_by_st.get(st_event).map(|indices| {
            indices
                .iter()
                .map(|idx| &self.coverage.vendor_event_markers[*idx])
                .collect::<Vec<_>>()
        })
    }
}

/// Coverage rule runner over typed firmware and Rust indexes.
struct CoverageRules<'a> {
    rust_crate: &'a Path,
    firmware: &'a FirmwareIndex<'a>,
    rust_commands: &'a RustCommandIndex,
    rust_events: &'a RustEventIndex,
}

impl<'a> CoverageRules<'a> {
    fn new(
        rust_crate: &'a Path,
        firmware: &'a FirmwareIndex<'a>,
        rust_commands: &'a RustCommandIndex,
        rust_events: &'a RustEventIndex,
    ) -> Self {
        Self {
            rust_crate,
            firmware,
            rust_commands,
            rust_events,
        }
    }

    fn check(&self) -> CoverageReport {
        let mut covered_by_marker = 0;
        let mut missing_markers = Vec::new();
        let mut duplicate_markers = Vec::new();
        let mut unknown_markers = Vec::new();
        let mut unknown_alias_markers = Vec::new();
        let mut marker_opcode_constants_missing = Vec::new();
        let mut marker_method_missing = Vec::new();
        let mut method_opcode_missing = Vec::new();
        let mut method_opcode_mismatches = Vec::new();
        let rust_methods_without_marker = rust_methods_without_marker(self.rust_commands);

        for command in self.firmware.commands {
            let st_command = formal_st_name(command);
            let Some(command_markers) = self.rust_commands.markers_for(st_command.as_str()) else {
                missing_markers.push(MissingMarker {
                    st_command,
                    opcode: command.opcode,
                    expected_rust_places: expected_places(self.rust_crate, command),
                });
                continue;
            };

            covered_by_marker += 1;
            if command_markers.len() > 1 {
                duplicate_markers.push(DuplicateMarker {
                    st_command: st_command.clone(),
                    locations: command_markers
                        .iter()
                        .map(|marker| marker.location.clone())
                        .collect(),
                });
            }

            for marker in command_markers {
                if marker.method.is_none() {
                    marker_method_missing.push(MarkerMethodMissing {
                        st_command: marker.st_command.clone(),
                        location: marker.location.clone(),
                    });
                }

                let Some(st_opcode) = command.opcode else {
                    marker_opcode_constants_missing.push(MarkerOpcodeConstantMissing {
                        st_command: marker.st_command.clone(),
                        opcode: command.opcode,
                        method: marker.method.clone(),
                        location: marker.location.clone(),
                    });
                    continue;
                };
                let Some(expected_opcode_const) = self.rust_commands.opcode_const(st_opcode) else {
                    marker_opcode_constants_missing.push(MarkerOpcodeConstantMissing {
                        st_command: marker.st_command.clone(),
                        opcode: command.opcode,
                        method: marker.method.clone(),
                        location: marker.location.clone(),
                    });
                    continue;
                };

                check_method_opcode(
                    marker,
                    expected_opcode_const,
                    &self.rust_commands.method_impls,
                    &mut method_opcode_missing,
                    &mut method_opcode_mismatches,
                );
            }
        }

        for marker in &self.rust_commands.markers {
            if !self.firmware.has_command(marker.st_command.as_str()) {
                unknown_markers.push(UnknownMarker {
                    st_command: marker.st_command.clone(),
                    method: marker.method.clone(),
                    location: marker.location.clone(),
                });
            }
        }

        for marker in &self.rust_commands.alias_markers {
            if !self.firmware.has_command(marker.alias_of.as_str()) {
                unknown_alias_markers.push(UnknownAliasMarker {
                    alias_of: marker.alias_of.clone(),
                    method: marker.method.clone(),
                    location: marker.location.clone(),
                });
            }
        }

        CoverageReport {
            firmware: self.firmware.spec.firmware.clone(),
            rust_crate: self.rust_crate.display().to_string(),
            commands_total: self.firmware.commands.len(),
            rust_opcode_constants_total: self.rust_commands.opcodes.len(),
            markers_total: self.rust_commands.markers.len(),
            alias_markers_total: self.rust_commands.alias_markers.len(),
            covered_by_marker,
            missing_markers,
            duplicate_markers,
            unknown_markers,
            unknown_alias_markers,
            marker_opcode_constants_missing,
            marker_method_missing,
            method_opcode_missing,
            method_opcode_mismatches,
            rust_methods_without_marker,
            events: check_event_coverage(self.firmware, self.rust_commands, self.rust_events),
        }
    }
}

/// Builds a compliance report for `rust_crate` against an extracted ST spec.
///
/// This is the public entry point used by the CLI. It intentionally performs
/// only comparison/reporting; parsing details live in the sibling modules.
pub fn check_coverage(spec: &FirmwareSpec, rust_crate: &Path) -> Result<CoverageReport> {
    let firmware = FirmwareIndex::new(spec);
    let rust_commands = RustCommandIndex::load(rust_crate)?;
    let rust_events = RustEventIndex::load(rust_crate)?;
    Ok(CoverageRules::new(rust_crate, &firmware, &rust_commands, &rust_events).check())
}

/// Checks Rust vendor event variants, event dispatch arms, and return handlers.
fn check_event_coverage(
    firmware: &FirmwareIndex<'_>,
    rust_commands: &RustCommandIndex,
    rust_events: &RustEventIndex,
) -> EventCoverageReport {
    let mut missing_vendor_event_variants = Vec::new();
    let mut missing_vendor_event_handlers = Vec::new();
    let mut missing_vendor_event_markers = Vec::new();
    let mut duplicate_vendor_event_markers = Vec::new();
    let mut unknown_vendor_event_markers = Vec::new();

    for event in &firmware.vendor_events {
        let st_event = formal_event_name(&event.name);
        let Some(markers) = rust_events.markers_for(st_event.as_str()) else {
            missing_vendor_event_markers.push(MissingVendorEventMarker { st_event });
            continue;
        };

        if markers.len() > 1 {
            duplicate_vendor_event_markers.push(DuplicateVendorEventMarker {
                st_event: st_event.clone(),
                locations: markers
                    .iter()
                    .map(|marker| marker.location.clone())
                    .collect(),
            });
        }

        let Some(expected_variant) = markers.first().and_then(|marker| marker.variant.as_ref())
        else {
            missing_vendor_event_variants.push(MissingVendorEventVariant { st_event });
            continue;
        };

        if !rust_events
            .coverage
            .vendor_event_variants
            .contains(expected_variant)
        {
            missing_vendor_event_variants.push(MissingVendorEventVariant { st_event });
            continue;
        }

        if !rust_events
            .coverage
            .vendor_event_handlers
            .contains(expected_variant)
        {
            missing_vendor_event_handlers.push(MissingVendorEventHandler {
                st_event,
                expected_variant: expected_variant.clone(),
            });
        }
    }

    for marker in &rust_events.coverage.vendor_event_markers {
        if !firmware.has_vendor_event(marker.st_event.as_str()) {
            unknown_vendor_event_markers.push(UnknownVendorEventMarker {
                st_event: marker.st_event.clone(),
                variant: marker.variant.clone(),
                location: marker.location.clone(),
            });
        }
    }

    let missing_vendor_return_handlers = firmware
        .command_complete_commands
        .iter()
        .filter_map(|command| {
            let opcode = command.opcode?;
            let expected_opcode_const = rust_commands.opcode_const(opcode)?;
            (!rust_events
                .coverage
                .vendor_return_handlers
                .contains(expected_opcode_const))
            .then(|| MissingVendorReturnHandler {
                st_command: formal_st_name(command),
                expected_opcode_const: expected_opcode_const.to_owned(),
            })
        })
        .collect();

    EventCoverageReport {
        vendor_events_total: firmware.vendor_events.len(),
        rust_vendor_event_variants_total: rust_events.coverage.vendor_event_variants.len(),
        rust_vendor_event_handlers_total: rust_events.coverage.vendor_event_handlers.len(),
        vendor_event_markers_total: rust_events.coverage.vendor_event_markers.len(),
        command_complete_events_total: firmware.command_complete_commands.len(),
        rust_vendor_return_handlers_total: rust_events.coverage.vendor_return_handlers.len(),
        missing_vendor_event_markers,
        duplicate_vendor_event_markers,
        unknown_vendor_event_markers,
        missing_vendor_event_variants,
        missing_vendor_event_handlers,
        missing_vendor_return_handlers,
    }
}

/// Returns all Rust methods already claimed by primary or alias markers.
fn command_marked_methods(
    markers: &[CommandMarker],
    alias_markers: &[AliasMarker],
) -> HashSet<(String, String)> {
    markers
        .iter()
        .filter_map(|marker| {
            marker
                .method
                .as_ref()
                .map(|method| (marker.location.file.clone(), method.clone()))
        })
        .chain(alias_markers.iter().filter_map(|marker| {
            marker
                .method
                .as_ref()
                .map(|method| (marker.location.file.clone(), method.clone()))
        }))
        .collect()
}

/// Finds command trait methods that were not claimed by a marker.
fn rust_methods_without_marker(rust_commands: &RustCommandIndex) -> Vec<RustMethodWithoutMarker> {
    rust_commands
        .methods
        .iter()
        .filter(|method| {
            !rust_commands
                .marked_methods
                .contains(&(method.location.file.clone(), method.name.clone()))
        })
        .map(|method| RustMethodWithoutMarker {
            method: method.name.clone(),
            location: method.location.clone(),
        })
        .collect()
}

/// Checks that the Rust implementation of a marked method uses the expected opcode.
fn check_method_opcode(
    marker: &CommandMarker,
    expected_opcode_const: &str,
    rust_method_impls: &HashMap<(String, String), RustMethodImplementation>,
    method_opcode_missing: &mut Vec<MethodOpcodeMissing>,
    method_opcode_mismatches: &mut Vec<MethodOpcodeMismatch>,
) {
    let Some(method) = marker.method.as_deref() else {
        return;
    };
    let impl_key = (marker.location.file.clone(), method.to_owned());
    let Some(method_impl) = rust_method_impls.get(&impl_key) else {
        method_opcode_missing.push(MethodOpcodeMissing {
            st_command: marker.st_command.clone(),
            expected_opcode_const: expected_opcode_const.to_owned(),
            method: marker.method.clone(),
            location: marker.location.clone(),
        });
        return;
    };
    if method_impl
        .opcodes
        .iter()
        .any(|opcode| opcode == expected_opcode_const)
    {
        return;
    }

    if let Some(actual_opcode_const) = method_impl.opcodes.first() {
        method_opcode_mismatches.push(MethodOpcodeMismatch {
            st_command: marker.st_command.clone(),
            expected_opcode_const: expected_opcode_const.to_owned(),
            actual_opcode_const: actual_opcode_const.clone(),
            method: method.to_owned(),
            location: marker.location.clone(),
        });
    } else {
        method_opcode_missing.push(MethodOpcodeMissing {
            st_command: marker.st_command.clone(),
            expected_opcode_const: expected_opcode_const.to_owned(),
            method: marker.method.clone(),
            location: marker.location.clone(),
        });
    }
}

/// Groups primary markers by formal ST command name.
fn command_markers_by_st(markers: &[CommandMarker]) -> HashMap<String, Vec<usize>> {
    let mut out = HashMap::<String, Vec<usize>>::new();
    for (idx, marker) in markers.iter().enumerate() {
        out.entry(marker.st_command.clone()).or_default().push(idx);
    }
    out
}

/// Groups event markers by formal ST event name.
fn event_markers_by_st(markers: &[EventMarker]) -> HashMap<String, Vec<usize>> {
    let mut out = HashMap::<String, Vec<usize>>::new();
    for (idx, marker) in markers.iter().enumerate() {
        out.entry(marker.st_event.clone()).or_default().push(idx);
    }
    out
}

/// Returns the ST command name used in marker comments.
///
/// ST headers usually provide the formal `ACI_*` name in `@brief`; when that is
/// unavailable, the generated C function name is uppercased as a fallback.
fn formal_st_name(command: &CommandSpec) -> String {
    command
        .doc
        .as_ref()
        .and_then(|doc| doc.brief.as_deref())
        .filter(|brief| brief.starts_with("ACI_"))
        .map(str::to_owned)
        .unwrap_or_else(|| command.name.to_ascii_uppercase())
}

/// Converts a generated C event function name to ST's formal `ACI_*` name.
fn formal_event_name(event_name: &str) -> String {
    event_name.to_ascii_uppercase()
}

/// Suggests Rust files where a missing command marker would likely belong.
fn expected_places(rust_crate: &Path, command: &CommandSpec) -> Vec<String> {
    let mut paths = vec![
        rust_crate
            .join("src/vendor/opcode.rs")
            .display()
            .to_string(),
    ];
    if let Some(command_file) = command_file(rust_crate, command.group.as_str()) {
        paths.push(command_file.display().to_string());
    }
    paths
}

/// Maps an ST command group to the corresponding Rust vendor command file.
fn command_file(rust_crate: &Path, group: &str) -> Option<PathBuf> {
    match group {
        "gap" | "gatt" | "hal" | "l2cap" => {
            Some(rust_crate.join(format!("src/vendor/command/{group}.rs")))
        }
        _ => None,
    }
}
