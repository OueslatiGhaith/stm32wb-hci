//! Conservative validation of normalized command and event wire envelopes.
//!
//! Source adapters retain explicit unresolved evidence when CubeWB does not
//! expose a definite size. At this boundary, every resolved layout becomes a
//! [`WireEnvelope`]. The Rust declarations use the same representation, so
//! requests, command returns, and event payloads all follow one comparison
//! path instead of carrying separate flags and length conventions.

use std::collections::BTreeMap;

use crate::catalog::{
    CatalogCommand, CatalogCompletion, CatalogEvent, CommandScope, EventScope, ExtractedEnvelope,
};
use crate::envelope::WireEnvelope;
use crate::rust_source::{CommandCompletion, CommandDeclaration, EventDeclaration, RustCatalog};

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
    crate_coverage: &RustCatalog,
) -> WireReport {
    compare_vendor_wire_with_external_events(commands, events, crate_coverage, &BTreeMap::new())
}

/// Compare wire declarations while accepting explicit payload evidence for
/// transport-only events absent from CubeWB's generated event table.
pub(crate) fn compare_vendor_wire_with_external_events(
    commands: &[CatalogCommand],
    events: &[CatalogEvent],
    crate_coverage: &RustCatalog,
    external_event_payloads: &BTreeMap<u16, ExtractedEnvelope>,
) -> WireReport {
    let mut by_ocf = BTreeMap::<u16, Vec<&CatalogCommand>>::new();
    for command in commands {
        if command.scope() == CommandScope::VendorAci {
            by_ocf.entry(command.ocf()).or_default().push(command);
        }
    }

    let mut report = WireReport::default();
    for declaration in crate_coverage.commands.values() {
        let Some(candidates) = by_ocf.get(&declaration.code) else {
            report.unavailable.push(WireUnavailable {
                code: declaration.code,
                command: declaration.name.clone(),
                reason: "no generated vendor ACI function has this OCF".to_owned(),
            });
            continue;
        };
        let [command] = candidates.as_slice() else {
            report.unavailable.push(WireUnavailable {
                code: declaration.code,
                command: declaration.name.clone(),
                reason: format!(
                    "{} generated vendor ACI functions share this OCF",
                    candidates.len()
                ),
            });
            continue;
        };

        compare_request(command, declaration, &mut report);
        compare_completion(command, declaration, &mut report);
    }

    let mut events_by_code = BTreeMap::<u16, Vec<&CatalogEvent>>::new();
    for event in events {
        if event.scope() == EventScope::VendorAci {
            events_by_code.entry(event.code).or_default().push(event);
        }
    }
    for metadata in crate_coverage.events.values() {
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

fn compare_event_payload(
    event: &CatalogEvent,
    metadata: &EventDeclaration,
    report: &mut WireReport,
) {
    let payload = event
        .vendor_payload()
        .expect("wire comparison filters for vendor ACI events");
    compare_event_payload_layout(payload, metadata, report);
}

fn compare_event_payload_layout(
    payload: &ExtractedEnvelope,
    metadata: &EventDeclaration,
    report: &mut WireReport,
) {
    let expected = extracted_envelope(payload, EnvelopeRelation::EventCapacity, |reason| {
        format!("CubeWB event payload layout is unresolved: {reason}")
    });
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
    declaration: &CommandDeclaration,
    report: &mut WireReport,
) {
    let expected = extracted_envelope(
        &command.request,
        EnvelopeRelation::RequestCapacity,
        |expression| {
            format!(
                "CubeWB request payload length uses unresolved source expression `{expression}`"
            )
        },
    );
    compare_envelope(
        declaration.code,
        &declaration.name,
        "request payload",
        expected,
        declaration.request,
        report,
    );
}

fn compare_completion(
    command: &CatalogCommand,
    declaration: &CommandDeclaration,
    report: &mut WireReport,
) {
    match (&command.completion, &declaration.completion) {
        (CatalogCompletion::CommandStatus {}, CommandCompletion::CommandStatus) => {}
        (CatalogCompletion::CommandStatus {}, CommandCompletion::CommandComplete { .. }) => {
            difference(
                report,
                declaration,
                "CubeWB waits for Command Status, but Rust declares Command Complete",
            );
        }
        (CatalogCompletion::CommandComplete { .. }, CommandCompletion::CommandStatus) => {
            difference(
                report,
                declaration,
                "CubeWB waits for Command Complete, but Rust declares Command Status",
            );
        }
        (
            CatalogCompletion::CommandComplete { returns: expected },
            CommandCompletion::CommandComplete { returns: actual },
        ) => {
            compare_envelope(
                declaration.code,
                &declaration.name,
                "command return payload",
                extracted_envelope(expected, EnvelopeRelation::ResponseCapacity, |expression| {
                    format!("CubeWB command return layout is unresolved: {expression}")
                }),
                *actual,
                report,
            );
        }
        (CatalogCompletion::Event { code }, _) => unavailable(
            report,
            declaration,
            format!(
                "CubeWB waits for event 0x{code:02X}; this checker only models Command Complete and Command Status"
            ),
        ),
        (CatalogCompletion::Unresolved { expression }, _) => unavailable(
            report,
            declaration,
            format!("CubeWB completion event uses unsupported expression `{expression}`"),
        ),
    }
}

fn extracted_envelope(
    layout: &ExtractedEnvelope,
    variable_relation: EnvelopeRelation,
    unresolved: impl FnOnce(&str) -> String,
) -> Result<EnvelopeExpectation, String> {
    match layout {
        ExtractedEnvelope::Known { minimum, maximum } => {
            if minimum > maximum {
                return Err(format!(
                    "extracted envelope has inverted bounds {minimum}..={maximum}"
                ));
            }
            let envelope = WireEnvelope::bounded(*minimum as usize, *maximum as usize);
            Ok(if minimum == maximum {
                EnvelopeExpectation::exact(envelope)
            } else {
                EnvelopeExpectation {
                    envelope,
                    relation: variable_relation,
                }
            })
        }
        ExtractedEnvelope::Unresolved(expression) => Err(unresolved(expression)),
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

fn difference(report: &mut WireReport, declaration: &CommandDeclaration, issue: impl Into<String>) {
    report.differences.push(WireDifference {
        code: declaration.code,
        command: declaration.name.clone(),
        issue: issue.into(),
    });
}

fn unavailable(
    report: &mut WireReport,
    declaration: &CommandDeclaration,
    reason: impl Into<String>,
) {
    report.unavailable.push(WireUnavailable {
        code: declaration.code,
        command: declaration.name.clone(),
        reason: reason.into(),
    });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::catalog::{CatalogCommandKind, CatalogEventKind};

    use super::*;

    fn fixture_declaration(
        name: &str,
        code: u16,
        completion: CommandCompletion,
        request: WireEnvelope,
    ) -> CommandDeclaration {
        CommandDeclaration {
            name: name.to_owned(),
            code,
            completion,
            request,
            location: PathBuf::from("fixture.rs"),
        }
    }

    fn declaration_complete(returns: WireEnvelope) -> CommandCompletion {
        CommandCompletion::CommandComplete { returns }
    }

    fn fixture_coverage(mut declarations: Vec<CommandDeclaration>, active: &[&str]) -> RustCatalog {
        declarations.retain(|declaration| active.contains(&declaration.name.as_str()));
        RustCatalog {
            commands: declarations
                .into_iter()
                .map(|declaration| (declaration.name.clone(), declaration))
                .collect::<BTreeMap<_, _>>(),
            events: BTreeMap::new(),
        }
    }

    fn fixture_command(
        ocf: u16,
        completion: CatalogCompletion,
        request: ExtractedEnvelope,
    ) -> CatalogCommand {
        CatalogCommand {
            kind: CatalogCommandKind::VendorAci { ocf },
            name: format!("aci_fixture_{ocf:03x}"),
            source_name: "fixture.c".to_owned(),
            source_offset: 0,
            completion,
            request,
        }
    }

    fn catalog_complete(returns: ExtractedEnvelope) -> CatalogCompletion {
        CatalogCompletion::CommandComplete { returns }
    }

    fn fixture_event(code: u16, payload: ExtractedEnvelope) -> CatalogEvent {
        CatalogEvent {
            kind: CatalogEventKind::VendorAci { payload },
            code,
            name: format!("aci_fixture_{code:04x}_event_process"),
            source_name: "ble_events.c".to_owned(),
            source_offset: 0,
        }
    }

    #[test]
    fn checks_only_active_declarations_and_reports_definite_mismatches() {
        let active = fixture_declaration(
            "Active",
            0x001,
            declaration_complete(WireEnvelope::fixed(0)),
            WireEnvelope::fixed(1),
        );
        let inactive = fixture_declaration(
            "Inactive",
            0x002,
            CommandCompletion::CommandStatus,
            WireEnvelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![active, inactive], &["Active"]);
        let commands = vec![
            fixture_command(
                0x001,
                CatalogCompletion::CommandStatus {},
                ExtractedEnvelope::fixed(0),
            ),
            fixture_command(
                0x002,
                catalog_complete(ExtractedEnvelope::fixed(3)),
                ExtractedEnvelope::fixed(0),
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
        coverage.events.insert(
            0x0400,
            EventDeclaration {
                name: "FixedEvent".to_owned(),
                code: 0x0400,
                payload: WireEnvelope::fixed(2),
                location: PathBuf::from("event.rs"),
            },
        );
        coverage.events.insert(
            0x0401,
            EventDeclaration {
                name: "VariableEvent".to_owned(),
                code: 0x0401,
                payload: WireEnvelope::bounded(3, 253),
                location: PathBuf::from("event.rs"),
            },
        );
        let events = vec![
            fixture_event(0x0400, ExtractedEnvelope::fixed(2)),
            fixture_event(
                0x0401,
                ExtractedEnvelope::Known {
                    minimum: 3,
                    maximum: 253,
                },
            ),
        ];

        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.checked, 2);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());

        coverage.events.get_mut(&0x0401).unwrap().payload = WireEnvelope::bounded(3, 252);
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("3..=253 bytes"));
        assert!(report.differences[0].issue.contains("3..=252 bytes"));

        coverage.events.get_mut(&0x0401).unwrap().payload = WireEnvelope::bounded(2, 253);
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("2..=253 bytes"));

        coverage.events.get_mut(&0x0401).unwrap().payload = WireEnvelope::bounded(4, 253);
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert!(report.differences.is_empty());
    }

    #[test]
    fn checks_transport_only_events_from_external_payload_evidence() {
        let mut coverage = fixture_coverage(Vec::new(), &[]);
        coverage.events.insert(
            0x9200,
            EventDeclaration {
                name: "CoprocessorReady".to_owned(),
                code: 0x9200,
                payload: WireEnvelope::fixed(1),
                location: PathBuf::from("event.rs"),
            },
        );

        let unavailable = compare_vendor_wire(&[], &[], &coverage);
        assert_eq!(unavailable.checked, 0);
        assert_eq!(unavailable.unavailable.len(), 1);

        let mut external = BTreeMap::from([(0x9200, ExtractedEnvelope::fixed(1))]);
        let report = compare_vendor_wire_with_external_events(&[], &[], &coverage, &external);
        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());

        external.insert(0x9200, ExtractedEnvelope::fixed(2));
        let report = compare_vendor_wire_with_external_events(&[], &[], &coverage, &external);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("is 2 bytes"));
        assert!(report.differences[0].issue.contains("declares 1 bytes"));
    }

    #[test]
    fn accepts_status_and_fixed_response_envelopes() {
        let status = fixture_declaration(
            "Status",
            0x001,
            declaration_complete(WireEnvelope::fixed(0)),
            WireEnvelope::fixed(0),
        );
        let fixed = fixture_declaration(
            "Fixed",
            0x002,
            declaration_complete(WireEnvelope::fixed(6)),
            WireEnvelope::fixed(3),
        );
        let coverage = fixture_coverage(vec![status, fixed], &["Status", "Fixed"]);
        let commands = vec![
            fixture_command(
                0x001,
                catalog_complete(ExtractedEnvelope::fixed(0)),
                ExtractedEnvelope::fixed(0),
            ),
            fixture_command(
                0x002,
                catalog_complete(ExtractedEnvelope::fixed(6)),
                ExtractedEnvelope::fixed(3),
            ),
        ];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 4);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn capacity_shaped_responses_accept_only_contained_rust_envelopes() {
        let contained = fixture_declaration(
            "Contained",
            0x010,
            declaration_complete(WireEnvelope::bounded(1, 16)),
            WireEnvelope::fixed(0),
        );
        let missing_prefix = fixture_declaration(
            "MissingPrefix",
            0x011,
            declaration_complete(WireEnvelope::bounded(0, 16)),
            WireEnvelope::fixed(0),
        );
        let too_large = fixture_declaration(
            "TooLarge",
            0x012,
            declaration_complete(WireEnvelope::bounded(1, 252)),
            WireEnvelope::fixed(0),
        );
        let coverage = fixture_coverage(
            vec![contained, missing_prefix, too_large],
            &["Contained", "MissingPrefix", "TooLarge"],
        );
        let command = |ocf| {
            fixture_command(
                ocf,
                catalog_complete(ExtractedEnvelope::Known {
                    minimum: 1,
                    maximum: 251,
                }),
                ExtractedEnvelope::fixed(0),
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
        let declaration = fixture_declaration(
            "Variable",
            0x001,
            declaration_complete(WireEnvelope::bounded(1, 6)),
            WireEnvelope::fixed(3),
        );
        let coverage = fixture_coverage(vec![declaration], &["Variable"]);
        let commands = vec![fixture_command(
            0x001,
            catalog_complete(ExtractedEnvelope::fixed(6)),
            ExtractedEnvelope::fixed(3),
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
        let declaration = fixture_declaration(
            "Fixed",
            0x002,
            declaration_complete(WireEnvelope::fixed(5)),
            WireEnvelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![declaration], &["Fixed"]);
        let commands = vec![fixture_command(
            0x002,
            catalog_complete(ExtractedEnvelope::fixed(6)),
            ExtractedEnvelope::fixed(0),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 2);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("is 6 bytes"));
        assert!(report.differences[0].issue.contains("declares 5 bytes"));
    }

    #[test]
    fn compares_fixed_and_variable_requests() {
        let wrong = fixture_declaration(
            "Wrong",
            0x006,
            CommandCompletion::CommandStatus,
            WireEnvelope::fixed(2),
        );
        let dynamic = fixture_declaration(
            "Dynamic",
            0x007,
            CommandCompletion::CommandStatus,
            WireEnvelope::bounded(1, 17),
        );
        let coverage = fixture_coverage(vec![wrong, dynamic], &["Wrong", "Dynamic"]);
        let commands = vec![
            fixture_command(
                0x006,
                CatalogCompletion::CommandStatus {},
                ExtractedEnvelope::fixed(3),
            ),
            fixture_command(
                0x007,
                CatalogCompletion::CommandStatus {},
                ExtractedEnvelope::Known {
                    minimum: 1,
                    maximum: 32,
                },
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
        let declaration = fixture_declaration(
            "Unresolved",
            0x008,
            CommandCompletion::CommandStatus,
            WireEnvelope::bounded(1, 17),
        );
        let coverage = fixture_coverage(vec![declaration], &["Unresolved"]);
        let commands = vec![fixture_command(
            0x008,
            CatalogCompletion::CommandStatus {},
            ExtractedEnvelope::Unresolved("custom(value_len)".to_owned()),
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
        let declaration = fixture_declaration(
            "UnresolvedCompletion",
            0x009,
            CommandCompletion::CommandStatus,
            WireEnvelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![declaration], &["UnresolvedCompletion"]);
        let commands = vec![fixture_command(
            0x009,
            CatalogCompletion::Unresolved {
                expression: "HCI_VENDOR_EVENT".to_owned(),
            },
            ExtractedEnvelope::fixed(0),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert_eq!(report.unavailable.len(), 1);
        assert!(report.unavailable[0].reason.contains("HCI_VENDOR_EVENT"));
    }

    #[test]
    fn unresolved_responses_remain_unavailable() {
        let declaration = fixture_declaration(
            "UnresolvedResponse",
            0x00a,
            declaration_complete(WireEnvelope::fixed(1)),
            WireEnvelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![declaration], &["UnresolvedResponse"]);
        let commands = vec![fixture_command(
            0x00a,
            catalog_complete(ExtractedEnvelope::Unresolved("computed_rlen".to_owned())),
            ExtractedEnvelope::fixed(0),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert_eq!(report.unavailable.len(), 1);
        assert!(report.unavailable[0].reason.contains("computed_rlen"));
    }

    #[test]
    fn request_capacity_requires_the_rust_envelope_to_be_contained() {
        let expected = Ok(EnvelopeExpectation {
            envelope: WireEnvelope::bounded(2, 255),
            relation: EnvelopeRelation::RequestCapacity,
        });

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
        let missing = fixture_declaration(
            "Missing",
            0x004,
            declaration_complete(WireEnvelope::fixed(0)),
            WireEnvelope::fixed(0),
        );
        let ambiguous = fixture_declaration(
            "Ambiguous",
            0x005,
            declaration_complete(WireEnvelope::fixed(0)),
            WireEnvelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![missing, ambiguous], &["Missing", "Ambiguous"]);
        let commands = vec![
            fixture_command(
                0x005,
                catalog_complete(ExtractedEnvelope::fixed(0)),
                ExtractedEnvelope::fixed(0),
            ),
            fixture_command(
                0x005,
                catalog_complete(ExtractedEnvelope::fixed(0)),
                ExtractedEnvelope::fixed(0),
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
