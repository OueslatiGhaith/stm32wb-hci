//! Coverage comparison between extracted ST commands and Rust command code.
//!
//! The functions in this module assume parsing has already produced a firmware
//! spec, Rust opcode constants, command markers, and method implementations.
//! The result is a serializable report for CLI/CI consumption.

use super::MarkerLocation;
use super::rust_event::{RustEventCoverage, load_rust_event_coverage};
use super::rust_marker::{CommandMarker, load_command_markers};
use super::rust_method::load_rust_command_methods;
use super::rust_method::{RustCommandMethod, RustMethodImplementation};
use super::rust_opcode::parse_rust_opcodes;
use crate::spec::{CommandSpec, FirmwareSpec};
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
    /// Number of ST command-complete vendor commands.
    pub command_complete_events_total: usize,
    /// Number of Rust vendor command-complete opcode handlers discovered.
    pub rust_vendor_return_handlers_total: usize,
    /// ST vendor events with no corresponding Rust `VendorEvent` variant.
    pub missing_vendor_event_variants: Vec<MissingVendorEventVariant>,
    /// ST vendor events with a Rust variant but no decode dispatch arm.
    pub missing_vendor_event_handlers: Vec<MissingVendorEventHandler>,
    /// Command-complete vendor commands with no Rust return-parameter handler.
    pub missing_vendor_return_handlers: Vec<MissingVendorReturnHandler>,
}

/// ST vendor event whose expected Rust variant is missing.
#[derive(Debug, Serialize)]
pub struct MissingVendorEventVariant {
    pub st_event: String,
    pub expected_variant: String,
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

/// Builds a compliance report for `rust_crate` against an extracted ST spec.
///
/// This is the public entry point used by the CLI. It intentionally performs
/// only comparison/reporting; parsing details live in the sibling modules.
pub fn check_coverage(spec: &FirmwareSpec, rust_crate: &Path) -> Result<CoverageReport> {
    let opcode_path = rust_crate.join("src/vendor/opcode.rs");
    let opcodes = parse_rust_opcodes(&opcode_path)?;
    let opcode_const_by_value = opcodes
        .iter()
        .map(|opcode| (opcode.opcode, opcode.name.as_str()))
        .collect::<HashMap<_, _>>();
    let loaded_markers = load_command_markers(rust_crate)?;
    let markers = loaded_markers.primary;
    let alias_markers = loaded_markers.aliases;
    let rust_methods = load_rust_command_methods(rust_crate)?;
    let rust_method_impls = super::rust_method::load_rust_method_implementations(rust_crate)?;
    let rust_events = load_rust_event_coverage(rust_crate)?;
    let marked_methods = marked_methods(&markers, &alias_markers);
    let markers_by_st = group_markers_by_st(&markers);
    let st_by_formal_name = spec
        .commands
        .iter()
        .map(|command| (formal_st_name(command), command))
        .collect::<HashMap<_, _>>();

    let mut covered_by_marker = 0;
    let mut missing_markers = Vec::new();
    let mut duplicate_markers = Vec::new();
    let mut unknown_markers = Vec::new();
    let mut unknown_alias_markers = Vec::new();
    let mut marker_opcode_constants_missing = Vec::new();
    let mut marker_method_missing = Vec::new();
    let mut method_opcode_missing = Vec::new();
    let mut method_opcode_mismatches = Vec::new();
    let rust_methods_without_marker = rust_methods_without_marker(&rust_methods, &marked_methods);

    for command in &spec.commands {
        let st_command = formal_st_name(command);
        let Some(command_markers) = markers_by_st.get(st_command.as_str()) else {
            missing_markers.push(MissingMarker {
                st_command,
                opcode: command.opcode,
                expected_rust_places: expected_places(rust_crate, command),
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
            let Some(expected_opcode_const) = opcode_const_by_value.get(&st_opcode) else {
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
                &rust_method_impls,
                &mut method_opcode_missing,
                &mut method_opcode_mismatches,
            );
        }
    }

    for marker in &markers {
        if !st_by_formal_name.contains_key(marker.st_command.as_str()) {
            unknown_markers.push(UnknownMarker {
                st_command: marker.st_command.clone(),
                method: marker.method.clone(),
                location: marker.location.clone(),
            });
        }
    }

    for marker in &alias_markers {
        if !st_by_formal_name.contains_key(marker.alias_of.as_str()) {
            unknown_alias_markers.push(UnknownAliasMarker {
                alias_of: marker.alias_of.clone(),
                method: marker.method.clone(),
                location: marker.location.clone(),
            });
        }
    }

    Ok(CoverageReport {
        firmware: spec.firmware.clone(),
        rust_crate: rust_crate.display().to_string(),
        commands_total: spec.commands.len(),
        rust_opcode_constants_total: opcodes.len(),
        markers_total: markers.len(),
        alias_markers_total: alias_markers.len(),
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
        events: check_event_coverage(spec, &opcode_const_by_value, &rust_events),
    })
}

/// Checks Rust vendor event variants, event dispatch arms, and return handlers.
fn check_event_coverage(
    spec: &FirmwareSpec,
    opcode_const_by_value: &HashMap<u16, &str>,
    rust_events: &RustEventCoverage,
) -> EventCoverageReport {
    let expected_vendor_events = spec
        .events
        .iter()
        .filter(|event| event.name.starts_with("aci_"))
        .collect::<Vec<_>>();
    let command_complete_commands = spec
        .commands
        .iter()
        .filter(|command| command.event != Some(0x0f))
        .collect::<Vec<_>>();

    let mut missing_vendor_event_variants = Vec::new();
    let mut missing_vendor_event_handlers = Vec::new();
    for event in &expected_vendor_events {
        let expected_variant = expected_vendor_event_variant(&event.name);
        if !rust_events
            .vendor_event_variants
            .contains(expected_variant.as_str())
        {
            missing_vendor_event_variants.push(MissingVendorEventVariant {
                st_event: formal_event_name(&event.name),
                expected_variant,
            });
            continue;
        }
        if !rust_events
            .vendor_event_handlers
            .contains(expected_variant.as_str())
        {
            missing_vendor_event_handlers.push(MissingVendorEventHandler {
                st_event: formal_event_name(&event.name),
                expected_variant,
            });
        }
    }

    let missing_vendor_return_handlers = command_complete_commands
        .iter()
        .filter_map(|command| {
            let opcode = command.opcode?;
            let expected_opcode_const = opcode_const_by_value.get(&opcode)?;
            (!rust_events
                .vendor_return_handlers
                .contains(*expected_opcode_const))
            .then(|| MissingVendorReturnHandler {
                st_command: formal_st_name(command),
                expected_opcode_const: (*expected_opcode_const).to_owned(),
            })
        })
        .collect();

    EventCoverageReport {
        vendor_events_total: expected_vendor_events.len(),
        rust_vendor_event_variants_total: rust_events.vendor_event_variants.len(),
        rust_vendor_event_handlers_total: rust_events.vendor_event_handlers.len(),
        command_complete_events_total: command_complete_commands.len(),
        rust_vendor_return_handlers_total: rust_events.vendor_return_handlers.len(),
        missing_vendor_event_variants,
        missing_vendor_event_handlers,
        missing_vendor_return_handlers,
    }
}

/// Returns all Rust methods already claimed by primary or alias markers.
fn marked_methods<'a>(
    markers: &'a [CommandMarker],
    alias_markers: &'a [super::rust_marker::AliasMarker],
) -> HashSet<(&'a str, &'a str)> {
    markers
        .iter()
        .filter_map(|marker| {
            marker
                .method
                .as_ref()
                .map(|method| (marker.location.file.as_str(), method.as_str()))
        })
        .chain(alias_markers.iter().filter_map(|marker| {
            marker
                .method
                .as_ref()
                .map(|method| (marker.location.file.as_str(), method.as_str()))
        }))
        .collect()
}

/// Finds command trait methods that were not claimed by a marker.
fn rust_methods_without_marker(
    rust_methods: &[RustCommandMethod],
    marked_methods: &HashSet<(&str, &str)>,
) -> Vec<RustMethodWithoutMarker> {
    rust_methods
        .iter()
        .filter(|method| {
            !marked_methods.contains(&(method.location.file.as_str(), method.name.as_str()))
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
fn group_markers_by_st(markers: &[CommandMarker]) -> HashMap<String, Vec<&CommandMarker>> {
    let mut out = HashMap::<String, Vec<&CommandMarker>>::new();
    for marker in markers {
        out.entry(marker.st_command.clone())
            .or_default()
            .push(marker);
    }
    out
}

/// Returns the Rust `VendorEvent` variant expected for an ST event function.
fn expected_vendor_event_variant(event_name: &str) -> String {
    match event_name {
        "aci_gap_limited_discoverable_event" => {
            return "GapLimitedDiscoverableTimeout".to_owned();
        }
        "aci_gap_slave_security_initiated_event" => {
            return "GapPeripheralSecurityInitiated".to_owned();
        }
        "aci_gap_authorization_req_event" => return "GapAuthorizationRequest".to_owned(),
        "aci_gatt_attribute_modified_event" => return "GattAttributeModified".to_owned(),
        "aci_gatt_write_permit_req_event" => return "AttWritePermitRequest".to_owned(),
        "aci_gatt_read_permit_req_event" => return "AttReadPermitRequest".to_owned(),
        "aci_gatt_read_multi_permit_req_event" => return "AttReadMultiplePermitRequest".to_owned(),
        "aci_gatt_prepare_write_permit_req_event" => {
            return "AttPrepareWritePermitRequest".to_owned();
        }
        "aci_gatt_error_resp_event" => return "AttErrorResponse".to_owned(),
        "aci_gatt_disc_read_char_by_uuid_resp_event" => {
            return "GattDiscoverOrReadCharacteristicByUuidResponse".to_owned();
        }
        "aci_gatt_mult_notification_event" => return "GattMultiNotification".to_owned(),
        "aci_hal_scan_req_report_event" => return "HalScanReqReport".to_owned(),
        _ => {}
    }

    event_name
        .trim_start_matches("aci_")
        .trim_end_matches("_event")
        .split('_')
        .map(event_token_to_rust)
        .collect()
}

/// Maps ST event-name tokens to the naming used by Rust event variants.
fn event_token_to_rust(token: &str) -> &'static str {
    match token {
        "addr" => "Address",
        "att" => "Att",
        "char" => "Characteristic",
        "coc" => "Coc",
        "disc" => "Discover",
        "eatt" => "Eatt",
        "exec" => "Execute",
        "fw" => "Firmware",
        "gap" => "Gap",
        "gatt" => "Gatt",
        "hal" => "Hal",
        "info" => "Information",
        "l2cap" => "L2Cap",
        "multi" | "mult" => "Multiple",
        "proc" => "Procedure",
        "reconf" => "Reconfig",
        "req" => "Request",
        "resp" => "Response",
        "rx" => "Rx",
        "tx" => "Tx",
        "uuid" => "Uuid",
        "oob" => "Oob",
        "io" => "Io",
        "rssi" => "Rssi",
        "eab" => "Eab",
        "bd" => "Bd",
        "le" => "Le",
        "mtu" => "Mtu",
        "pool" => "Pool",
        "available" => "Available",
        "activity" => "Activity",
        "bearer" => "Bearer",
        "blob" => "Blob",
        "bond" => "Bond",
        "by" => "By",
        "command" => "Command",
        "complete" => "Complete",
        "confirm" => "Confirm",
        "confirmation" => "Confirmation",
        "comparison" => "Comparison",
        "connect" => "Connect",
        "connection" => "Connection",
        "control" => "Control",
        "data" => "Data",
        "discoverable" => "Discoverable",
        "disconnect" => "Disconnect",
        "end" => "End",
        "error" => "Error",
        "exchange" => "Exchange",
        "ext" => "Ext",
        "find" => "Find",
        "flow" => "Flow",
        "group" => "Group",
        "indication" => "Indication",
        "initiated" => "Initiated",
        "key" => "Key",
        "keypress" => "Keypress",
        "limited" => "Limited",
        "lost" => "Lost",
        "modified" => "Modified",
        "notification" => "Notification",
        "not" => "Not",
        "numeric" => "Numeric",
        "of" => "Of",
        "pairing" => "Pairing",
        "pass" => "Pass",
        "prepare" => "Prepare",
        "radio" => "Radio",
        "read" => "Read",
        "reject" => "Reject",
        "report" => "Report",
        "resolved" => "Resolved",
        "scan" => "Scan",
        "security" => "Security",
        "server" => "Server",
        "slave" => "Slave",
        "timeout" => "Timeout",
        "type" => "Type",
        "update" => "Update",
        "value" => "Value",
        "write" => "Write",
        _ => "",
    }
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
