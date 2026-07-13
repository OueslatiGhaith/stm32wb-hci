//! Conservative validation of normalized command and event wire envelopes.
//!
//! Source adapters retain explicit unresolved evidence when CubeWB does not
//! expose a definite size. At this boundary, every resolved layout becomes a
//! [`WireEnvelope`]. The Rust declarations use the same representation, so
//! requests, command returns, and event payloads all follow one comparison
//! path instead of carrying separate flags and length conventions.

use std::collections::BTreeMap;

use crate::catalog::{
    CatalogCommand, CatalogEvent, CommandScope, CompletionExpectation, EventPayloadLayout,
    EventScope, RequestLayout, ResponseLayout,
};
use crate::envelope::WireEnvelope;
use crate::rust_source::{CrateCoverage, DescriptorMetadata, EventMetadata};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvelopeRelation {
    /// The generated declaration proves the complete envelope.
    Exact,
    /// A capacity-shaped event must preserve the generated maximum, while its
    /// Rust schema may enforce a stricter semantic minimum.
    EventCapacity,
    /// A generated request declaration proves the complete safe capacity. A
    /// Rust API may intentionally expose a subset of that capacity.
    RequestCapacity,
    /// A generated response structure proves its fixed prefix and storage
    /// capacity. The controller's semantic response may use a narrower range.
    ResponseCapacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EnvelopeExpectation {
    envelope: WireEnvelope,
    relation: EnvelopeRelation,
}

impl EnvelopeExpectation {
    const fn exact(envelope: WireEnvelope) -> Self {
        Self {
            envelope,
            relation: EnvelopeRelation::Exact,
        }
    }

    const fn event_capacity(envelope: WireEnvelope) -> Self {
        Self {
            envelope,
            relation: EnvelopeRelation::EventCapacity,
        }
    }

    const fn request_capacity(envelope: WireEnvelope) -> Self {
        Self {
            envelope,
            relation: EnvelopeRelation::RequestCapacity,
        }
    }

    const fn response_capacity(envelope: WireEnvelope) -> Self {
        Self {
            envelope,
            relation: EnvelopeRelation::ResponseCapacity,
        }
    }
}

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

/// Result of checking active vendor declarations against CubeWB C metadata.
///
/// `checked` counts individual request, return, or event-payload envelopes
/// compared. Entries in `unavailable` name schema details the checker could
/// not normalize and are compliance failures, preventing silent coverage
/// regressions.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WireReport {
    pub checked: usize,
    pub differences: Vec<WireDifference>,
    pub unavailable: Vec<WireUnavailable>,
}

/// Compare active command and event payload declarations for the selected
/// firmware.
#[cfg(test)]
pub(crate) fn compare_vendor_wire(
    commands: &[CatalogCommand],
    events: &[CatalogEvent],
    crate_coverage: &CrateCoverage,
) -> WireReport {
    compare_vendor_wire_with_external_events(commands, events, crate_coverage, &BTreeMap::new())
}

/// Compare wire declarations while accepting explicit payload evidence for
/// transport-only events absent from CubeWB's generated event table.
pub(crate) fn compare_vendor_wire_with_external_events(
    commands: &[CatalogCommand],
    events: &[CatalogEvent],
    crate_coverage: &CrateCoverage,
    external_event_payloads: &BTreeMap<u16, EventPayloadLayout>,
) -> WireReport {
    let mut by_ocf = BTreeMap::<u16, Vec<&CatalogCommand>>::new();
    for command in commands {
        if command.scope() == CommandScope::VendorAci {
            by_ocf.entry(command.ocf()).or_default().push(command);
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

        compare_request(command, descriptor, &mut report);
        compare_completion(command, descriptor, &mut report);
    }

    let mut events_by_code = BTreeMap::<u16, Vec<&CatalogEvent>>::new();
    for event in events {
        if event.scope() == EventScope::VendorAci {
            events_by_code.entry(event.code).or_default().push(event);
        }
    }
    for metadata in crate_coverage.event_metadata.values() {
        let Some(candidates) = events_by_code.get(&metadata.code) else {
            if let Some(payload) = external_event_payloads.get(&metadata.code) {
                compare_event_payload_layout(payload, metadata, &mut report);
            } else {
                report.unavailable.push(WireUnavailable {
                    code: metadata.code,
                    command: metadata.name.clone(),
                    reason: "no generated vendor event-table entry or external payload declaration has this code"
                        .to_owned(),
                });
            }
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
    let payload = event
        .vendor_payload()
        .expect("wire comparison filters for vendor ACI events");
    compare_event_payload_layout(payload, metadata, report);
}

fn compare_event_payload_layout(
    payload: &EventPayloadLayout,
    metadata: &EventMetadata,
    report: &mut WireReport,
) {
    let expected = event_payload_envelope(payload);
    compare_envelope(
        metadata.code,
        &metadata.name,
        "event payload",
        expected,
        metadata.payload,
        report,
    );
}

fn compare_request(
    command: &CatalogCommand,
    descriptor: &DescriptorMetadata,
    report: &mut WireReport,
) {
    let expected = request_envelope(&command.request);
    compare_envelope(
        descriptor.code,
        &descriptor.name,
        "request payload",
        expected,
        descriptor.request,
        report,
    );
}

fn compare_completion(
    command: &CatalogCommand,
    descriptor: &DescriptorMetadata,
    report: &mut WireReport,
) {
    match &command.completion {
        CompletionExpectation::CommandStatus => {
            if !matches!(descriptor.completion, CompletionExpectation::CommandStatus) {
                difference(
                    report,
                    descriptor,
                    "CubeWB waits for Command Status, but Rust declares Command Complete",
                );
            }
        }
        CompletionExpectation::CommandComplete => {
            if !matches!(
                descriptor.completion,
                CompletionExpectation::CommandComplete
            ) {
                difference(
                    report,
                    descriptor,
                    "CubeWB waits for Command Complete, but Rust declares Command Status",
                );
                return;
            }
            let Some(actual) = descriptor.response else {
                difference(
                    report,
                    descriptor,
                    "Rust declares Command Complete without a Return payload envelope",
                );
                return;
            };
            compare_envelope(
                descriptor.code,
                &descriptor.name,
                "command return payload",
                response_envelope(&command.response),
                actual,
                report,
            );
        }
        CompletionExpectation::Event(event) => unavailable(
            report,
            descriptor,
            format!(
                "CubeWB waits for event 0x{event:02X}; this checker only models Command Complete and Command Status"
            ),
        ),
        CompletionExpectation::Unresolved(expression) => unavailable(
            report,
            descriptor,
            format!("CubeWB completion event uses unsupported expression `{expression}`"),
        ),
    }
}

fn request_envelope(layout: &RequestLayout) -> Result<EnvelopeExpectation, String> {
    match layout {
        RequestLayout::Empty => Ok(EnvelopeExpectation::exact(WireEnvelope::fixed(0))),
        RequestLayout::Fixed(length) => Ok(EnvelopeExpectation::exact(WireEnvelope::fixed(
            *length as usize,
        ))),
        RequestLayout::Variable { minimum, maximum } => Ok(EnvelopeExpectation::request_capacity(
            WireEnvelope::bounded(*minimum as usize, *maximum as usize),
        )),
        RequestLayout::Unresolved(expression) => Err(format!(
            "CubeWB request payload length uses unresolved source expression `{expression}`"
        )),
    }
}

fn response_envelope(layout: &ResponseLayout) -> Result<EnvelopeExpectation, String> {
    match layout {
        // CubeWB's rlen includes the transport status byte. `Return` does not.
        ResponseLayout::Status => Ok(EnvelopeExpectation::exact(WireEnvelope::fixed(0))),
        ResponseLayout::Fixed(length) => length
            .checked_sub(1)
            .map(|length| EnvelopeExpectation::exact(WireEnvelope::fixed(length as usize)))
            .ok_or_else(|| {
                "CubeWB command-complete response length is zero and cannot contain status"
                    .to_owned()
            }),
        ResponseLayout::Variable { minimum, maximum } => {
            // CubeWB's packed response includes status; the declarative Rust
            // `Return` starts immediately after it.
            let minimum = minimum.checked_sub(1).ok_or_else(|| {
                "CubeWB variable command response cannot contain status".to_owned()
            })?;
            let maximum = maximum.checked_sub(1).ok_or_else(|| {
                "CubeWB variable command response cannot contain status".to_owned()
            })?;
            Ok(EnvelopeExpectation::response_capacity(
                WireEnvelope::bounded(minimum as usize, maximum as usize),
            ))
        }
        ResponseLayout::Unresolved(expression) => Err(format!(
            "CubeWB command return layout is unresolved: {expression}"
        )),
        ResponseLayout::None => {
            Err("CubeWB does not state a command-complete response length".to_owned())
        }
    }
}

fn event_payload_envelope(layout: &EventPayloadLayout) -> Result<EnvelopeExpectation, String> {
    match layout {
        EventPayloadLayout::Fixed(length) => Ok(EnvelopeExpectation::exact(WireEnvelope::fixed(
            *length as usize,
        ))),
        EventPayloadLayout::Variable { minimum, maximum } => {
            Ok(EnvelopeExpectation::event_capacity(WireEnvelope::bounded(
                *minimum as usize,
                *maximum as usize,
            )))
        }
        EventPayloadLayout::Unresolved(reason) => Err(format!(
            "CubeWB event payload layout is unresolved: {reason}"
        )),
    }
}

fn compare_envelope(
    code: u16,
    name: &str,
    label: &str,
    expected: Result<EnvelopeExpectation, String>,
    actual: WireEnvelope,
    report: &mut WireReport,
) {
    let expected = match expected {
        Ok(expected) => expected,
        Err(reason) => {
            report.unavailable.push(WireUnavailable {
                code,
                command: name.to_owned(),
                reason,
            });
            return;
        }
    };

    report.checked += 1;
    let compatible = match expected.relation {
        EnvelopeRelation::Exact => actual == expected.envelope,
        EnvelopeRelation::EventCapacity => {
            // The C type proves only its fixed prefix. Rust may add a stricter
            // semantic minimum, such as requiring one counted item, but must
            // accept the complete generated event capacity.
            !actual.is_fixed()
                && actual.minimum >= expected.envelope.minimum
                && actual.minimum <= expected.envelope.maximum
                && actual.maximum == expected.envelope.maximum
        }
        EnvelopeRelation::RequestCapacity => {
            // The C wrapper's command buffer proves a safe outer capacity,
            // while public parameter constraints can intentionally be
            // narrower. The entire Rust envelope must fit within that proof.
            actual.minimum >= expected.envelope.minimum
                && actual.maximum <= expected.envelope.maximum
        }
        EnvelopeRelation::ResponseCapacity => {
            // A capacity-sized C response buffer is an outer storage bound.
            // Rust may preserve stricter command semantics, but must never
            // decode outside the proven prefix/capacity envelope.
            actual.minimum >= expected.envelope.minimum
                && actual.maximum <= expected.envelope.maximum
        }
    };
    if !compatible {
        report.differences.push(WireDifference {
            code,
            command: name.to_owned(),
            issue: format!(
                "CubeWB {label} envelope is {}, but Rust declares {actual}",
                expected.envelope
            ),
        });
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

    use crate::catalog::{CatalogCommandKind, CatalogEventKind};
    use crate::model::ProtocolCoverage;

    use super::*;

    fn fixture_descriptor(
        name: &str,
        code: u16,
        completion: CompletionExpectation,
        request: WireEnvelope,
        response: Option<WireEnvelope>,
    ) -> DescriptorMetadata {
        DescriptorMetadata {
            name: name.to_owned(),
            code,
            completion,
            request,
            response,
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
            kind: CatalogCommandKind::VendorAci { ocf },
            name: format!("aci_fixture_{ocf:03x}"),
            source_name: "fixture.c".to_owned(),
            source_offset: 0,
            completion,
            request,
            response,
        }
    }

    fn fixture_event(code: u16, payload: EventPayloadLayout) -> CatalogEvent {
        CatalogEvent {
            kind: CatalogEventKind::VendorAci { payload },
            code,
            name: format!("aci_fixture_{code:04x}_event_process"),
            source_name: "ble_events.c".to_owned(),
            source_offset: 0,
        }
    }

    #[test]
    fn checks_only_active_descriptors_and_reports_definite_mismatches() {
        let active = fixture_descriptor(
            "Active",
            0x001,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(1),
            Some(WireEnvelope::fixed(0)),
        );
        let inactive = fixture_descriptor(
            "Inactive",
            0x002,
            CompletionExpectation::CommandStatus,
            WireEnvelope::fixed(0),
            None,
        );
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
                .any(|difference| difference.issue.contains("request payload envelope"))
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
                payload: WireEnvelope::fixed(2),
                location: PathBuf::from("event.rs"),
            },
        );
        coverage.event_metadata.insert(
            0x0401,
            EventMetadata {
                name: "VariableEvent".to_owned(),
                code: 0x0401,
                payload: WireEnvelope::bounded(3, 253),
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

        coverage.event_metadata.get_mut(&0x0401).unwrap().payload = WireEnvelope::bounded(3, 252);
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("3..=253 bytes"));
        assert!(report.differences[0].issue.contains("3..=252 bytes"));

        coverage.event_metadata.get_mut(&0x0401).unwrap().payload = WireEnvelope::bounded(2, 253);
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("2..=253 bytes"));

        coverage.event_metadata.get_mut(&0x0401).unwrap().payload = WireEnvelope::bounded(4, 253);
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert!(report.differences.is_empty());
    }

    #[test]
    fn checks_transport_only_events_from_external_payload_evidence() {
        let mut coverage = fixture_coverage(Vec::new(), &[]);
        coverage.event_metadata.insert(
            0x9200,
            EventMetadata {
                name: "CoprocessorReady".to_owned(),
                code: 0x9200,
                payload: WireEnvelope::fixed(1),
                location: PathBuf::from("event.rs"),
            },
        );

        let unavailable = compare_vendor_wire(&[], &[], &coverage);
        assert_eq!(unavailable.checked, 0);
        assert_eq!(unavailable.unavailable.len(), 1);

        let mut external = BTreeMap::from([(0x9200, EventPayloadLayout::Fixed(1))]);
        let report = compare_vendor_wire_with_external_events(&[], &[], &coverage, &external);
        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());

        external.insert(0x9200, EventPayloadLayout::Fixed(2));
        let report = compare_vendor_wire_with_external_events(&[], &[], &coverage, &external);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("is 2 bytes"));
        assert!(report.differences[0].issue.contains("declares 1 bytes"));
    }

    #[test]
    fn accepts_status_and_fixed_response_envelopes() {
        let status = fixture_descriptor(
            "Status",
            0x001,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::fixed(0)),
        );
        let fixed = fixture_descriptor(
            "Fixed",
            0x002,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(3),
            Some(WireEnvelope::fixed(6)),
        );
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

        assert_eq!(report.checked, 4);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn capacity_shaped_responses_accept_only_contained_rust_envelopes() {
        let contained = fixture_descriptor(
            "Contained",
            0x010,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::bounded(1, 16)),
        );
        let missing_prefix = fixture_descriptor(
            "MissingPrefix",
            0x011,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::bounded(0, 16)),
        );
        let too_large = fixture_descriptor(
            "TooLarge",
            0x012,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::bounded(1, 252)),
        );
        let coverage = fixture_coverage(
            vec![contained, missing_prefix, too_large],
            &["Contained", "MissingPrefix", "TooLarge"],
        );
        let command = |ocf| {
            fixture_command(
                ocf,
                CompletionExpectation::CommandComplete,
                RequestLayout::Empty,
                ResponseLayout::Variable {
                    minimum: 2,
                    maximum: 252,
                },
            )
        };
        let commands = vec![command(0x010), command(0x011), command(0x012)];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 6);
        assert_eq!(report.differences.len(), 2);
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.command == "MissingPrefix")
        );
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.command == "TooLarge")
        );
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn rejects_variable_rust_return_for_fixed_cube_response() {
        let descriptor = fixture_descriptor(
            "Variable",
            0x001,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(3),
            Some(WireEnvelope::bounded(1, 6)),
        );
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
                .contains("command return payload envelope")
        );
    }

    #[test]
    fn detects_an_incorrect_fixed_response_buffer_length() {
        let descriptor = fixture_descriptor(
            "Fixed",
            0x002,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::fixed(5)),
        );
        let coverage = fixture_coverage(vec![descriptor], &["Fixed"]);
        let commands = vec![fixture_command(
            0x002,
            CompletionExpectation::CommandComplete,
            RequestLayout::Empty,
            ResponseLayout::Fixed(7),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 2);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("is 6 bytes"));
        assert!(report.differences[0].issue.contains("declares 5 bytes"));
    }

    #[test]
    fn rejects_command_complete_without_a_return_envelope() {
        let descriptor = fixture_descriptor(
            "Structured",
            0x003,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            None,
        );
        let coverage = fixture_coverage(vec![descriptor], &["Structured"]);
        let commands = vec![fixture_command(
            0x003,
            CompletionExpectation::CommandComplete,
            RequestLayout::Empty,
            ResponseLayout::Unresolved("packed C structure `aci_fixture_rp0`".to_owned()),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("without a Return"));
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn compares_fixed_and_variable_requests() {
        let wrong = fixture_descriptor(
            "Wrong",
            0x006,
            CompletionExpectation::CommandStatus,
            WireEnvelope::fixed(2),
            None,
        );
        let dynamic = fixture_descriptor(
            "Dynamic",
            0x007,
            CompletionExpectation::CommandStatus,
            WireEnvelope::bounded(1, 17),
            None,
        );
        let coverage = fixture_coverage(vec![wrong, dynamic], &["Wrong", "Dynamic"]);
        let commands = vec![
            fixture_command(
                0x006,
                CompletionExpectation::CommandStatus,
                RequestLayout::Fixed(3),
                ResponseLayout::None,
            ),
            fixture_command(
                0x007,
                CompletionExpectation::CommandStatus,
                RequestLayout::Variable {
                    minimum: 1,
                    maximum: 32,
                },
                ResponseLayout::None,
            ),
        ];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 2);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("is 3 bytes"));
        assert!(report.differences[0].issue.contains("declares 2 bytes"));
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn unresolved_requests_remain_unavailable() {
        let descriptor = fixture_descriptor(
            "Unresolved",
            0x008,
            CompletionExpectation::CommandStatus,
            WireEnvelope::bounded(1, 17),
            None,
        );
        let coverage = fixture_coverage(vec![descriptor], &["Unresolved"]);
        let commands = vec![fixture_command(
            0x008,
            CompletionExpectation::CommandStatus,
            RequestLayout::Unresolved("custom(value_len)".to_owned()),
            ResponseLayout::None,
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 0);
        assert!(report.differences.is_empty());
        assert_eq!(report.unavailable.len(), 1);
        assert!(
            report.unavailable[0]
                .reason
                .contains("unresolved source expression")
        );
    }

    #[test]
    fn unresolved_completions_remain_unavailable() {
        let descriptor = fixture_descriptor(
            "UnresolvedCompletion",
            0x009,
            CompletionExpectation::CommandStatus,
            WireEnvelope::fixed(0),
            None,
        );
        let coverage = fixture_coverage(vec![descriptor], &["UnresolvedCompletion"]);
        let commands = vec![fixture_command(
            0x009,
            CompletionExpectation::Unresolved("HCI_VENDOR_EVENT".to_owned()),
            RequestLayout::Empty,
            ResponseLayout::None,
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert_eq!(report.unavailable.len(), 1);
        assert!(report.unavailable[0].reason.contains("HCI_VENDOR_EVENT"));
    }

    #[test]
    fn unresolved_responses_remain_unavailable() {
        let descriptor = fixture_descriptor(
            "UnresolvedResponse",
            0x00a,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::fixed(1)),
        );
        let coverage = fixture_coverage(vec![descriptor], &["UnresolvedResponse"]);
        let commands = vec![fixture_command(
            0x00a,
            CompletionExpectation::CommandComplete,
            RequestLayout::Empty,
            ResponseLayout::Unresolved("computed_rlen".to_owned()),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert_eq!(report.unavailable.len(), 1);
        assert!(report.unavailable[0].reason.contains("computed_rlen"));
    }

    #[test]
    fn request_capacity_requires_the_rust_envelope_to_be_contained() {
        let expected = Ok(EnvelopeExpectation::request_capacity(
            WireEnvelope::bounded(2, 255),
        ));

        let mut report = WireReport::default();
        compare_envelope(
            1,
            "Contained",
            "request payload",
            expected.clone(),
            WireEnvelope::bounded(2, 48),
            &mut report,
        );
        assert!(report.differences.is_empty());

        compare_envelope(
            2,
            "MissingPrefix",
            "request payload",
            expected.clone(),
            WireEnvelope::bounded(1, 48),
            &mut report,
        );
        compare_envelope(
            3,
            "TooLarge",
            "request payload",
            expected,
            WireEnvelope::bounded(2, 256),
            &mut report,
        );
        assert_eq!(report.differences.len(), 2);
    }

    #[test]
    fn reports_unknown_or_ambiguous_generated_commands_as_unavailable() {
        let missing = fixture_descriptor(
            "Missing",
            0x004,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::fixed(0)),
        );
        let ambiguous = fixture_descriptor(
            "Ambiguous",
            0x005,
            CompletionExpectation::CommandComplete,
            WireEnvelope::fixed(0),
            Some(WireEnvelope::fixed(0)),
        );
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
