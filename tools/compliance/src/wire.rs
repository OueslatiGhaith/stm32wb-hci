//! Conservative validation of vendor command and event transport envelopes.
//!
//! The CubeWB generated C functions expose enough information to check the
//! parts of a command which are independent of C structure definitions:
//! whether a request body is empty, whether completion is delivered through
//! Command Status, and fixed-size command-complete responses. Fixed packed
//! `sizeof(resp)` layouts are resolved from CubeWB's tagged `ble_types.h` by
//! the source loader; only capacity-sized or unsupported structures remain
//! unavailable here. Vendor events are checked against the fixed or
//! capacity-shaped packed payload structures used by their generated process
//! functions.

use std::collections::BTreeMap;

use crate::catalog::{
    CatalogCommand, CatalogEvent, CommandScope, CompletionExpectation, EventPayloadLayout,
    EventScope, RequestLayout, ResponseLayout,
};
use crate::rust_source::{CrateCoverage, DescriptorMetadata, EventMetadata};

/// A definite incompatibility between a generated vendor C wire declaration
/// and its active Rust command or event declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireDifference {
    pub code: u16,
    pub command: String,
    pub issue: String,
}

/// A command or event whose complete envelope cannot yet be compared without
/// guessing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireUnavailable {
    pub code: u16,
    pub command: String,
    pub reason: String,
}

/// Result of checking active vendor descriptors against CubeWB C metadata.
///
/// `checked` counts descriptors that matched exactly one generated vendor ACI
/// function and whose envelope was inspected.  Entries in `unavailable` are
/// intentionally not failures: they name a schema detail the checker does not
/// yet have enough information to validate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WireReport {
    pub checked: usize,
    pub differences: Vec<WireDifference>,
    pub unavailable: Vec<WireUnavailable>,
}

/// Compare active command descriptors and event payloads for the selected
/// firmware.
pub(crate) fn compare_vendor_wire(
    commands: &[CatalogCommand],
    events: &[CatalogEvent],
    crate_coverage: &CrateCoverage,
) -> WireReport {
    let mut by_ocf = BTreeMap::<u16, Vec<&CatalogCommand>>::new();
    for command in commands {
        if command.scope == CommandScope::VendorAci {
            by_ocf.entry(command.ocf).or_default().push(command);
        }
    }

    let mut report = WireReport::default();
    for descriptor in crate_coverage.descriptor_metadata.values() {
        let Some(candidates) = by_ocf.get(&descriptor.code) else {
            report.unavailable.push(WireUnavailable {
                code: descriptor.code,
                command: descriptor.name.clone(),
                reason: "no generated vendor ACI function has this OCF".to_owned(),
            });
            continue;
        };
        let [command] = candidates.as_slice() else {
            report.unavailable.push(WireUnavailable {
                code: descriptor.code,
                command: descriptor.name.clone(),
                reason: format!(
                    "{} generated vendor ACI functions share this OCF",
                    candidates.len()
                ),
            });
            continue;
        };

        report.checked += 1;
        compare_request(command, descriptor, &mut report);
        compare_completion(command, descriptor, &mut report);
    }

    let mut events_by_code = BTreeMap::<u16, Vec<&CatalogEvent>>::new();
    for event in events {
        if event.scope == EventScope::VendorAci {
            events_by_code.entry(event.code).or_default().push(event);
        }
    }
    for metadata in crate_coverage.event_metadata.values() {
        let Some(candidates) = events_by_code.get(&metadata.code) else {
            report.unavailable.push(WireUnavailable {
                code: metadata.code,
                command: metadata.name.clone(),
                reason: "no generated vendor event-table entry has this code".to_owned(),
            });
            continue;
        };
        let [event] = candidates.as_slice() else {
            report.unavailable.push(WireUnavailable {
                code: metadata.code,
                command: metadata.name.clone(),
                reason: format!(
                    "{} generated vendor events share this code",
                    candidates.len()
                ),
            });
            continue;
        };
        report.checked += 1;
        compare_event_payload(event, metadata, &mut report);
    }

    report.differences.sort_by_key(|difference| {
        (
            difference.code,
            difference.command.clone(),
            difference.issue.clone(),
        )
    });
    report.unavailable.sort_by_key(|unavailable| {
        (
            unavailable.code,
            unavailable.command.clone(),
            unavailable.reason.clone(),
        )
    });
    report
}

fn compare_event_payload(event: &CatalogEvent, metadata: &EventMetadata, report: &mut WireReport) {
    match &event.payload {
        EventPayloadLayout::Fixed(expected) => {
            let expected = *expected as usize;
            if metadata.variable {
                event_difference(
                    report,
                    metadata,
                    format!(
                        "CubeWB event payload is fixed at {expected} bytes, but Rust declares a variable Payload"
                    ),
                );
            } else if (metadata.min_payload_len, metadata.max_payload_len) != (expected, expected) {
                event_difference(
                    report,
                    metadata,
                    format!(
                        "CubeWB event payload is fixed at {expected} bytes, but Rust declares {}..={} bytes",
                        metadata.min_payload_len, metadata.max_payload_len
                    ),
                );
            }
        }
        EventPayloadLayout::Variable { minimum, maximum } => {
            let minimum = *minimum as usize;
            let maximum = *maximum as usize;
            if !metadata.variable {
                event_difference(
                    report,
                    metadata,
                    format!(
                        "CubeWB event payload has a capacity-shaped {minimum}..={maximum}-byte envelope, but Rust declares a fixed Payload"
                    ),
                );
                return;
            }
            if metadata.min_payload_len < minimum || metadata.min_payload_len > maximum {
                event_difference(
                    report,
                    metadata,
                    format!(
                        "CubeWB event payload has a {minimum}-byte fixed prefix, but Rust declares a minimum of {} bytes",
                        metadata.min_payload_len
                    ),
                );
            }
            if metadata.max_payload_len != maximum {
                event_difference(
                    report,
                    metadata,
                    format!(
                        "CubeWB event payload capacity is {maximum} bytes, but Rust declares a maximum of {} bytes",
                        metadata.max_payload_len
                    ),
                );
            }
        }
        EventPayloadLayout::CStruct(type_name) => report.unavailable.push(WireUnavailable {
            code: metadata.code,
            command: metadata.name.clone(),
            reason: format!(
                "CubeWB event payload uses unresolved packed C structure `{type_name}`"
            ),
        }),
    }
}

fn event_difference(report: &mut WireReport, metadata: &EventMetadata, issue: impl Into<String>) {
    report.differences.push(WireDifference {
        code: metadata.code,
        command: metadata.name.clone(),
        issue: issue.into(),
    });
}

fn compare_request(
    command: &CatalogCommand,
    descriptor: &DescriptorMetadata,
    report: &mut WireReport,
) {
    if matches!(command.request, RequestLayout::Empty) && !descriptor.params_empty {
        difference(
            report,
            descriptor,
            "CubeWB declares an empty request, but the Rust descriptor declares non-empty Params",
        );
    }
}

fn compare_completion(
    command: &CatalogCommand,
    descriptor: &DescriptorMetadata,
    report: &mut WireReport,
) {
    match &command.completion {
        CompletionExpectation::CommandStatus => {
            if descriptor.declares_return {
                difference(
                    report,
                    descriptor,
                    "CubeWB waits for Command Status, but the Rust descriptor declares Return; command-status commands must omit Return",
                );
            }
        }
        CompletionExpectation::CommandComplete => match &command.response {
            ResponseLayout::Status => compare_status_response(descriptor, report),
            ResponseLayout::Fixed(expected) => {
                compare_fixed_response(descriptor, *expected, report)
            }
            ResponseLayout::CStruct(type_name) => {
                if !descriptor.declares_return || descriptor.response_len.is_none() {
                    difference(
                        report,
                        descriptor,
                        format!(
                            "CubeWB response uses packed C structure `{type_name}`, but Rust does not declare an inline Return schema"
                        ),
                    );
                } else {
                    unavailable(
                        report,
                        descriptor,
                        format!(
                            "CubeWB response uses packed C structure `{type_name}` with a capacity-sized or unsupported field schema"
                        ),
                    );
                }
            }
            ResponseLayout::Expression(expression) => unavailable(
                report,
                descriptor,
                format!("CubeWB response length uses unsupported expression `{expression}`"),
            ),
            // The generated wrapper did not state an `rq.rlen`.  It would be
            // unsafe to infer whether the Rust descriptor should have Return.
            ResponseLayout::None => unavailable(
                report,
                descriptor,
                "CubeWB does not state a command-complete response length".to_owned(),
            ),
        },
        CompletionExpectation::Event(event) => unavailable(
            report,
            descriptor,
            format!(
                "CubeWB waits for event 0x{event:02X}; this checker only models Command Complete and Command Status"
            ),
        ),
        CompletionExpectation::Expression(expression) => unavailable(
            report,
            descriptor,
            format!("CubeWB completion event uses unsupported expression `{expression}`"),
        ),
    }
}

fn compare_status_response(descriptor: &DescriptorMetadata, report: &mut WireReport) {
    if !descriptor.declares_return {
        difference(
            report,
            descriptor,
            "CubeWB response is one status byte, but the Rust descriptor omits Return; expected Return = ()",
        );
    } else if descriptor.response_len.is_some() {
        difference(
            report,
            descriptor,
            "CubeWB response is one status byte, but the Rust descriptor declares a non-empty Return payload; expected Return = ()",
        );
    }
}

fn compare_fixed_response(descriptor: &DescriptorMetadata, expected: u32, report: &mut WireReport) {
    let expected = usize::try_from(expected)
        .expect("the intermediate schema's u32 response length fits the host usize");
    if descriptor.return_variable {
        difference(
            report,
            descriptor,
            format!(
                "CubeWB response is fixed at {expected} bytes, but Rust declares a variable return payload"
            ),
        );
        return;
    }
    match (descriptor.declares_return, descriptor.response_len) {
        (false, _) => difference(
            report,
            descriptor,
            format!(
                "CubeWB response is {expected} bytes, but the Rust descriptor omits Return; expected an inline Return schema with {} payload bytes",
                expected.saturating_sub(1),
            ),
        ),
        (true, Some(actual)) if actual != expected => difference(
            report,
            descriptor,
            format!(
                "CubeWB response is {expected} bytes, but the Rust inline Return schema allows {actual} bytes including status"
            ),
        ),
        (true, Some(_)) => {}
        (true, None) => difference(
            report,
            descriptor,
            format!(
                "CubeWB response is {expected} bytes, but Rust declares Return = (); expected an inline Return schema with {} payload bytes",
                expected.saturating_sub(1),
            ),
        ),
    }
}

fn difference(report: &mut WireReport, descriptor: &DescriptorMetadata, issue: impl Into<String>) {
    report.differences.push(WireDifference {
        code: descriptor.code,
        command: descriptor.name.clone(),
        issue: issue.into(),
    });
}

fn unavailable(
    report: &mut WireReport,
    descriptor: &DescriptorMetadata,
    reason: impl Into<String>,
) {
    report.unavailable.push(WireUnavailable {
        code: descriptor.code,
        command: descriptor.name.clone(),
        reason: reason.into(),
    });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::model::ProtocolCoverage;

    use super::*;

    fn fixture_descriptor(
        name: &str,
        code: u16,
        params_empty: bool,
        declares_return: bool,
        response_len: Option<usize>,
    ) -> DescriptorMetadata {
        DescriptorMetadata {
            name: name.to_owned(),
            code,
            params_empty,
            declares_return,
            response_len,
            return_variable: false,
            location: PathBuf::from("fixture.rs"),
        }
    }

    fn fixture_coverage(
        mut descriptors: Vec<DescriptorMetadata>,
        active: &[&str],
    ) -> CrateCoverage {
        descriptors.retain(|descriptor| active.contains(&descriptor.name.as_str()));
        CrateCoverage {
            descriptors: ProtocolCoverage::default(),
            active_api: ProtocolCoverage::default(),
            descriptor_metadata: descriptors
                .into_iter()
                .map(|descriptor| (descriptor.name.clone(), descriptor))
                .collect::<BTreeMap<_, _>>(),
            event_metadata: BTreeMap::new(),
        }
    }

    fn fixture_command(
        ocf: u16,
        completion: CompletionExpectation,
        request: RequestLayout,
        response: ResponseLayout,
    ) -> CatalogCommand {
        CatalogCommand {
            scope: CommandScope::VendorAci,
            name: format!("aci_fixture_{ocf:03x}"),
            source_name: "fixture.c".to_owned(),
            source_offset: 0,
            ogf: None,
            ocf,
            opcode: None,
            completion,
            request,
            response,
        }
    }

    fn fixture_event(code: u16, payload: EventPayloadLayout) -> CatalogEvent {
        CatalogEvent {
            scope: EventScope::VendorAci,
            code,
            name: format!("aci_fixture_{code:04x}_event_process"),
            source_name: "ble_events.c".to_owned(),
            source_offset: 0,
            payload,
        }
    }

    #[test]
    fn checks_only_active_descriptors_and_reports_definite_mismatches() {
        let active = fixture_descriptor("Active", 0x001, false, true, None);
        let inactive = fixture_descriptor("Inactive", 0x002, true, false, None);
        let coverage = fixture_coverage(vec![active, inactive], &["Active"]);
        let commands = vec![
            fixture_command(
                0x001,
                CompletionExpectation::CommandStatus,
                RequestLayout::Empty,
                ResponseLayout::Status,
            ),
            fixture_command(
                0x002,
                CompletionExpectation::CommandComplete,
                RequestLayout::Empty,
                ResponseLayout::Fixed(4),
            ),
        ];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert_eq!(report.differences.len(), 2);
        assert!(
            report
                .differences
                .iter()
                .all(|difference| difference.command == "Active")
        );
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.issue.contains("empty request"))
        );
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.issue.contains("Command Status"))
        );
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn compares_fixed_and_capacity_shaped_event_payloads() {
        let mut coverage = fixture_coverage(Vec::new(), &[]);
        coverage.event_metadata.insert(
            0x0400,
            EventMetadata {
                name: "FixedEvent".to_owned(),
                code: 0x0400,
                min_payload_len: 2,
                max_payload_len: 2,
                variable: false,
                location: PathBuf::from("event.rs"),
            },
        );
        coverage.event_metadata.insert(
            0x0401,
            EventMetadata {
                name: "VariableEvent".to_owned(),
                code: 0x0401,
                min_payload_len: 3,
                max_payload_len: 253,
                variable: true,
                location: PathBuf::from("event.rs"),
            },
        );
        let events = vec![
            fixture_event(0x0400, EventPayloadLayout::Fixed(2)),
            fixture_event(
                0x0401,
                EventPayloadLayout::Variable {
                    minimum: 3,
                    maximum: 253,
                },
            ),
        ];

        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.checked, 2);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());

        coverage
            .event_metadata
            .get_mut(&0x0401)
            .unwrap()
            .max_payload_len = 252;
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("capacity is 253"));
    }

    #[test]
    fn accepts_status_and_fixed_response_envelopes() {
        let status = fixture_descriptor("Status", 0x001, true, true, None);
        let fixed = fixture_descriptor("Fixed", 0x002, false, true, Some(7));
        let coverage = fixture_coverage(vec![status, fixed], &["Status", "Fixed"]);
        let commands = vec![
            fixture_command(
                0x001,
                CompletionExpectation::CommandComplete,
                RequestLayout::Empty,
                ResponseLayout::Status,
            ),
            fixture_command(
                0x002,
                CompletionExpectation::CommandComplete,
                RequestLayout::Fixed(3),
                ResponseLayout::Fixed(7),
            ),
        ];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 2);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn rejects_variable_rust_return_for_fixed_cube_response() {
        let mut descriptor = fixture_descriptor("Variable", 0x001, false, true, Some(7));
        descriptor.return_variable = true;
        let coverage = fixture_coverage(vec![descriptor], &["Variable"]);
        let commands = vec![fixture_command(
            0x001,
            CompletionExpectation::CommandComplete,
            RequestLayout::Fixed(3),
            ResponseLayout::Fixed(7),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.differences.len(), 1);
        assert!(
            report.differences[0]
                .issue
                .contains("variable return payload")
        );
    }

    #[test]
    fn detects_an_incorrect_fixed_response_buffer_length() {
        let descriptor = fixture_descriptor("Fixed", 0x002, true, true, Some(6));
        let coverage = fixture_coverage(vec![descriptor], &["Fixed"]);
        let commands = vec![fixture_command(
            0x002,
            CompletionExpectation::CommandComplete,
            RequestLayout::Empty,
            ResponseLayout::Fixed(7),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert_eq!(report.differences.len(), 1);
        assert!(
            report.differences[0]
                .issue
                .contains("allows 6 bytes including status")
        );
    }

    #[test]
    fn reports_unresolved_packed_struct_responses_as_unavailable() {
        let descriptor = fixture_descriptor("Structured", 0x003, true, true, Some(5));
        let coverage = fixture_coverage(vec![descriptor], &["Structured"]);
        let commands = vec![fixture_command(
            0x003,
            CompletionExpectation::CommandComplete,
            RequestLayout::Empty,
            ResponseLayout::CStruct("aci_fixture_rp0".to_owned()),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert_eq!(report.unavailable.len(), 1);
        assert!(report.unavailable[0].reason.contains("aci_fixture_rp0"));
    }

    #[test]
    fn requires_an_inline_return_for_an_unresolved_struct_response() {
        let descriptor = fixture_descriptor("Structured", 0x003, true, true, None);
        let coverage = fixture_coverage(vec![descriptor], &["Structured"]);
        let commands = vec![fixture_command(
            0x003,
            CompletionExpectation::CommandComplete,
            RequestLayout::Empty,
            ResponseLayout::CStruct("aci_fixture_rp0".to_owned()),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("inline Return schema"));
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn reports_unknown_or_ambiguous_generated_commands_as_unavailable() {
        let missing = fixture_descriptor("Missing", 0x004, true, true, None);
        let ambiguous = fixture_descriptor("Ambiguous", 0x005, true, true, None);
        let coverage = fixture_coverage(vec![missing, ambiguous], &["Missing", "Ambiguous"]);
        let commands = vec![
            fixture_command(
                0x005,
                CompletionExpectation::CommandComplete,
                RequestLayout::Empty,
                ResponseLayout::Status,
            ),
            fixture_command(
                0x005,
                CompletionExpectation::CommandComplete,
                RequestLayout::Empty,
                ResponseLayout::Status,
            ),
        ];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 0);
        assert_eq!(report.unavailable.len(), 2);
        assert!(
            report
                .unavailable
                .iter()
                .any(|unavailable| unavailable.reason.contains("no generated"))
        );
        assert!(
            report
                .unavailable
                .iter()
                .any(|unavailable| unavailable.reason.contains("share this OCF"))
        );
    }
}
