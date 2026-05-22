//! Typed firmware and Rust lookup indexes for coverage rules.
//!
//! The rule layer compares these indexes instead of rebuilding ad hoc maps from
//! raw parser outputs.

use super::naming::{formal_event_name, formal_st_name};
use super::rust_event::{EventMarker, RustEventCoverage, load_rust_event_coverage};
use super::rust_marker::{AliasMarker, CommandMarker, load_command_markers};
use super::rust_method::{RustCommandMethod, RustMethodImplementation, load_rust_command_methods};
use super::rust_opcode::{RustOpcode, parse_rust_opcodes};
use crate::spec::{CommandSpec, FirmwareSpec};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Firmware command with the formal ST name precomputed for rule checks.
pub(super) struct FirmwareCommand<'a> {
    pub(super) spec: &'a CommandSpec,
    pub(super) st_name: String,
}

impl FirmwareCommand<'_> {
    pub(super) fn expected_places(&self, rust_crate: &Path) -> Vec<String> {
        super::naming::expected_places(rust_crate, self.spec)
    }
}

/// Firmware event with the formal ST event name precomputed for rule checks.
pub(super) struct FirmwareEvent {
    pub(super) st_name: String,
}

/// Firmware-side lookup surface used by coverage rules.
pub(super) struct FirmwareIndex<'a> {
    pub(super) spec: &'a FirmwareSpec,
    pub(super) commands: Vec<FirmwareCommand<'a>>,
    command_names: HashSet<String>,
    pub(super) vendor_events: Vec<FirmwareEvent>,
    vendor_event_names: HashSet<String>,
    pub(super) command_complete_commands: Vec<FirmwareCommand<'a>>,
}

impl<'a> FirmwareIndex<'a> {
    pub(super) fn new(spec: &'a FirmwareSpec) -> Self {
        let commands: Vec<_> = spec
            .commands
            .iter()
            .map(|command| FirmwareCommand {
                spec: command,
                st_name: formal_st_name(command),
            })
            .collect();
        let vendor_events: Vec<_> = spec
            .events
            .iter()
            .filter(|event| event.name.starts_with("aci_"))
            .map(|event| FirmwareEvent {
                st_name: formal_event_name(&event.name),
            })
            .collect();
        let command_complete_commands = spec
            .commands
            .iter()
            .filter(|command| command.event != Some(0x0f))
            .map(|command| FirmwareCommand {
                spec: command,
                st_name: formal_st_name(command),
            })
            .collect();

        Self {
            spec,
            command_names: commands
                .iter()
                .map(|command| command.st_name.clone())
                .collect(),
            vendor_event_names: vendor_events
                .iter()
                .map(|event| event.st_name.clone())
                .collect(),
            commands,
            vendor_events,
            command_complete_commands,
        }
    }

    pub(super) fn has_command(&self, st_command: &str) -> bool {
        self.command_names.contains(st_command)
    }

    pub(super) fn has_vendor_event(&self, st_event: &str) -> bool {
        self.vendor_event_names.contains(st_event)
    }
}

/// Rust command-side lookup surface used by coverage rules.
pub(super) struct RustCommandIndex {
    pub(super) opcodes: Vec<RustOpcode>,
    opcode_const_by_value: HashMap<u16, String>,
    pub(super) markers: Vec<CommandMarker>,
    pub(super) alias_markers: Vec<AliasMarker>,
    pub(super) methods: Vec<RustCommandMethod>,
    pub(super) method_impls: HashMap<(String, String), RustMethodImplementation>,
    pub(super) marked_methods: HashSet<(String, String)>,
    markers_by_st: HashMap<String, Vec<usize>>,
}

impl RustCommandIndex {
    pub(super) fn load(rust_crate: &Path) -> Result<Self> {
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

    pub(super) fn opcode_const(&self, opcode: u16) -> Option<&str> {
        self.opcode_const_by_value.get(&opcode).map(String::as_str)
    }

    pub(super) fn markers_for(&self, st_command: &str) -> Option<Vec<&CommandMarker>> {
        self.markers_by_st.get(st_command).map(|indices| {
            indices
                .iter()
                .map(|idx| &self.markers[*idx])
                .collect::<Vec<_>>()
        })
    }
}

/// Rust event-side lookup surface used by coverage rules.
pub(super) struct RustEventIndex {
    pub(super) coverage: RustEventCoverage,
    markers_by_st: HashMap<String, Vec<usize>>,
}

impl RustEventIndex {
    pub(super) fn load(rust_crate: &Path) -> Result<Self> {
        let coverage = load_rust_event_coverage(rust_crate)?;
        let markers_by_st = event_markers_by_st(&coverage.vendor_event_markers);
        Ok(Self {
            coverage,
            markers_by_st,
        })
    }

    pub(super) fn markers_for(&self, st_event: &str) -> Option<Vec<&EventMarker>> {
        self.markers_by_st.get(st_event).map(|indices| {
            indices
                .iter()
                .map(|idx| &self.coverage.vendor_event_markers[*idx])
                .collect::<Vec<_>>()
        })
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
