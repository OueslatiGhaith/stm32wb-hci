//! Coverage rule execution over typed indexes.

use super::index::{FirmwareIndex, RustCommandIndex, RustEventIndex};
use super::report::{
    CoverageReport, DuplicateMarker, DuplicateVendorEventMarker, EventCoverageReport,
    MarkerMethodMissing, MarkerOpcodeConstantMissing, MethodOpcodeMismatch, MethodOpcodeMissing,
    MissingMarker, MissingVendorEventHandler, MissingVendorEventMarker, MissingVendorEventVariant,
    MissingVendorReturnHandler, RustMethodWithoutMarker, UnknownAliasMarker, UnknownMarker,
    UnknownVendorEventMarker,
};
use super::rust_marker::CommandMarker;
use super::rust_method::RustMethodImplementation;
use std::collections::HashMap;
use std::path::Path;

/// Coverage rule runner over typed firmware and Rust indexes.
pub(super) struct CoverageRules<'a> {
    rust_crate: &'a Path,
    firmware: &'a FirmwareIndex<'a>,
    rust_commands: &'a RustCommandIndex,
    rust_events: &'a RustEventIndex,
}

impl<'a> CoverageRules<'a> {
    pub(super) fn new(
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

    pub(super) fn check(&self) -> CoverageReport {
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

        for command in &self.firmware.commands {
            let st_command = command.st_name.clone();
            let Some(command_markers) = self.rust_commands.markers_for(st_command.as_str()) else {
                missing_markers.push(MissingMarker {
                    st_command,
                    opcode: command.spec.opcode,
                    expected_rust_places: command.expected_places(self.rust_crate),
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

                let Some(st_opcode) = command.spec.opcode else {
                    marker_opcode_constants_missing.push(MarkerOpcodeConstantMissing {
                        st_command: marker.st_command.clone(),
                        opcode: command.spec.opcode,
                        method: marker.method.clone(),
                        location: marker.location.clone(),
                    });
                    continue;
                };
                let Some(expected_opcode_const) = self.rust_commands.opcode_const(st_opcode) else {
                    marker_opcode_constants_missing.push(MarkerOpcodeConstantMissing {
                        st_command: marker.st_command.clone(),
                        opcode: command.spec.opcode,
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
        let st_event = event.st_name.clone();
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
            let opcode = command.spec.opcode?;
            let expected_opcode_const = rust_commands.opcode_const(opcode)?;
            (!rust_events
                .coverage
                .vendor_return_handlers
                .contains(expected_opcode_const))
            .then(|| MissingVendorReturnHandler {
                st_command: command.st_name.clone(),
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
