//! Conservative validation of normalized command and event wire envelopes.
//!
//! Source adapters retain explicit unresolved evidence when CubeWB does not
//! expose a definite size. At this boundary, every resolved layout becomes a
//! [`Envelope`]. The Rust declarations use the same representation, so
//! requests, command returns, and event payloads all follow one comparison
//! path instead of carrying separate flags and length conventions.

use std::collections::BTreeMap;

#[cfg(test)]
use crate::catalog::Envelope;
use crate::catalog::{
    CatalogCommand, CatalogCompletion, CatalogEvent, CommandScope, EventScope, Evidence,
    VariableSemantic, WireLayout, WireLayoutEvidence, WireSegment,
};
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

#[derive(Clone, Copy)]
enum ComparisonDetail {
    Segments,
    EnvelopeOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EnvelopeExpectation {
    layout: WireLayout,
    relation: EnvelopeRelation,
}

impl EnvelopeExpectation {
    fn exact(layout: WireLayout) -> Self {
        Self {
            layout,
            relation: EnvelopeRelation::Exact,
        }
    }
}

/// A definite incompatibility between a generated C wire declaration and its
/// active Rust command or event declaration.
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

/// Result of checking active declarations against CubeWB C metadata.
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
    compare_wire(commands, events, crate_coverage, &[])
}

/// Compare proprietary STM32 declarations and local standard-HCI commands
/// against source-backed wire evidence.
pub(crate) fn compare_wire(
    commands: &[CatalogCommand],
    events: &[CatalogEvent],
    crate_coverage: &RustCatalog,
    local_standard_commands: &[CommandDeclaration],
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

        compare_request(
            command,
            declaration,
            ComparisonDetail::Segments,
            &mut report,
        );
        compare_completion(
            command,
            declaration,
            ComparisonDetail::Segments,
            &mut report,
        );
    }

    let mut by_opcode = BTreeMap::<u16, Vec<&CatalogCommand>>::new();
    for command in commands {
        if command.scope() == CommandScope::StandardHci {
            by_opcode.entry(command.code()).or_default().push(command);
        }
    }
    for declaration in local_standard_commands {
        let Some(candidates) = by_opcode.get(&declaration.code) else {
            report.unavailable.push(WireUnavailable {
                code: declaration.code,
                command: declaration.name.clone(),
                reason: "no generated standard-HCI function has this opcode".to_owned(),
            });
            continue;
        };
        let [command] = candidates.as_slice() else {
            report.unavailable.push(WireUnavailable {
                code: declaration.code,
                command: declaration.name.clone(),
                reason: format!(
                    "{} generated standard-HCI functions share this opcode",
                    candidates.len()
                ),
            });
            continue;
        };
        compare_request(
            command,
            declaration,
            ComparisonDetail::EnvelopeOnly,
            &mut report,
        );
        compare_completion(
            command,
            declaration,
            ComparisonDetail::EnvelopeOnly,
            &mut report,
        );
    }

    let mut events_by_code = BTreeMap::<u16, Vec<&CatalogEvent>>::new();
    for event in events {
        if matches!(
            event.scope(),
            EventScope::VendorAci | EventScope::SystemShci
        ) {
            events_by_code.entry(event.code).or_default().push(event);
        }
    }
    for metadata in crate_coverage.events.values() {
        let Some(candidates) = events_by_code.get(&metadata.code) else {
            report.unavailable.push(WireUnavailable {
                code: metadata.code,
                command: metadata.name.clone(),
                reason: "no generated vendor ACI or system SHCI event has this code".to_owned(),
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
        .proprietary_payload()
        .expect("wire comparison filters for proprietary STM32 events");
    compare_event_payload_layout(payload, metadata, report);
}

fn compare_event_payload_layout(
    payload: &WireLayoutEvidence,
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
        &metadata.payload,
        report,
    );
}

fn compare_request(
    command: &CatalogCommand,
    declaration: &CommandDeclaration,
    detail: ComparisonDetail,
    report: &mut WireReport,
) {
    let expected = with_comparison_detail(
        extracted_envelope(
            &command.request,
            EnvelopeRelation::RequestCapacity,
            |expression| {
                format!(
                    "CubeWB request payload length uses unresolved source expression `{expression}`"
                )
            },
        ),
        detail,
    );
    compare_envelope(
        declaration.code,
        &declaration.name,
        "request payload",
        expected,
        &declaration.request,
        report,
    );
}

fn compare_completion(
    command: &CatalogCommand,
    declaration: &CommandDeclaration,
    detail: ComparisonDetail,
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
                with_comparison_detail(
                    extracted_envelope(
                        expected,
                        EnvelopeRelation::ResponseCapacity,
                        |expression| {
                            format!("CubeWB command return layout is unresolved: {expression}")
                        },
                    ),
                    detail,
                ),
                actual,
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

fn with_comparison_detail(
    expectation: Result<EnvelopeExpectation, String>,
    detail: ComparisonDetail,
) -> Result<EnvelopeExpectation, String> {
    expectation.map(|mut expectation| {
        if matches!(detail, ComparisonDetail::EnvelopeOnly) {
            let envelope = expectation.layout.envelope();
            expectation.layout = WireLayout::byte_capacity(envelope.minimum(), envelope.maximum());
        }
        expectation
    })
}

fn extracted_envelope(
    layout: &WireLayoutEvidence,
    variable_relation: EnvelopeRelation,
    unresolved: impl FnOnce(&str) -> String,
) -> Result<EnvelopeExpectation, String> {
    match layout {
        Evidence::Known(layout) => Ok(if layout.envelope().is_fixed() {
            EnvelopeExpectation::exact(layout.clone())
        } else {
            EnvelopeExpectation {
                layout: layout.clone(),
                relation: variable_relation,
            }
        }),
        Evidence::Unresolved(expression) => Err(unresolved(expression)),
    }
}

fn compare_envelope(
    code: u16,
    name: &str,
    label: &str,
    expected: Result<EnvelopeExpectation, String>,
    actual: &WireLayout,
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
    let expected_envelope = expected.layout.envelope();
    let actual_envelope = actual.envelope();
    let actual_storage_envelope = actual
        .segments()
        .and_then(|segments| WireLayout::from_segments(segments.to_vec()))
        .map_or(actual_envelope, |layout| layout.envelope());
    let envelope_only_storage_compatible =
        expected.layout.segments().is_some() || actual_storage_envelope == expected_envelope;
    let envelope_compatible = match expected.relation {
        EnvelopeRelation::Exact => actual_envelope == expected_envelope,
        EnvelopeRelation::EventCapacity
        | EnvelopeRelation::RequestCapacity
        | EnvelopeRelation::ResponseCapacity => {
            // Public semantics may be narrower only when the declarative
            // schema separately records the complete generated storage range.
            // This remains enforceable when Cube proves only an envelope and
            // cannot expose individual segments.
            actual_envelope.minimum() >= expected_envelope.minimum()
                && actual_envelope.maximum() <= expected_envelope.maximum()
                && envelope_only_storage_compatible
        }
    };
    let schema_compatible = compatible_segments(
        expected.layout.segments(),
        actual.segments(),
        expected.relation,
    );
    if !envelope_compatible || !schema_compatible {
        report.differences.push(WireDifference {
            code,
            command: name.to_owned(),
            issue: format!(
                "CubeWB {label} layout is {} with {:?}, but Rust declares {} with {:?}",
                expected_envelope,
                expected.layout.segments(),
                actual_envelope,
                actual.segments(),
            ),
        });
    }
}

fn compatible_segments(
    expected: Option<&[WireSegment]>,
    actual: Option<&[WireSegment]>,
    relation: EnvelopeRelation,
) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    let Some(actual) = actual else {
        return false;
    };
    expected.len() == actual.len()
        && expected
            .iter()
            .zip(actual)
            .all(|(expected, actual)| match (expected, actual) {
                (
                    WireSegment::Fixed {
                        length: expected_length,
                        ..
                    },
                    WireSegment::Fixed {
                        length: actual_length,
                        ..
                    },
                ) => expected_length == actual_length,
                (
                    WireSegment::Variable {
                        element_width: expected_width,
                        minimum_elements: expected_minimum,
                        maximum_elements: expected_maximum,
                        semantic: expected_semantic,
                    },
                    WireSegment::Variable {
                        element_width: actual_width,
                        minimum_elements: actual_minimum,
                        maximum_elements: actual_maximum,
                        semantic: actual_semantic,
                    },
                ) => {
                    if expected_width != actual_width
                        || !compatible_variable_semantics(
                            expected_semantic.as_ref(),
                            actual_semantic.as_ref(),
                        )
                    {
                        return false;
                    }
                    match relation {
                        EnvelopeRelation::Exact => {
                            expected_minimum == actual_minimum && expected_maximum == actual_maximum
                        }
                        EnvelopeRelation::EventCapacity
                        | EnvelopeRelation::RequestCapacity
                        | EnvelopeRelation::ResponseCapacity => {
                            expected_minimum == actual_minimum && expected_maximum == actual_maximum
                        }
                    }
                }
                _ => false,
            })
}

fn compatible_variable_semantics(
    expected: Option<&VariableSemantic>,
    actual: Option<&VariableSemantic>,
) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    let Some(actual) = actual else {
        return false;
    };
    match (expected, actual) {
        (
            VariableSemantic::Counted {
                prefix_width: expected,
            },
            VariableSemantic::Counted {
                prefix_width: actual,
            },
        ) => expected == actual,
        (
            VariableSemantic::Tagged {
                tag_width: expected_width,
                variants: expected_variants,
            },
            VariableSemantic::Tagged {
                tag_width: actual_width,
                variants: actual_variants,
            },
        ) => {
            expected_width == actual_width
                && (expected_variants.is_empty() || expected_variants == actual_variants)
        }
        (
            VariableSemantic::LengthPrefixedRecords {
                record_len_width: expected_record_width,
                length_width: expected_length_width,
                minimum_record_len: expected_minimum,
            },
            VariableSemantic::LengthPrefixedRecords {
                record_len_width: actual_record_width,
                length_width: actual_length_width,
                minimum_record_len: actual_minimum,
            },
        ) => {
            expected_record_width == actual_record_width
                && expected_length_width == actual_length_width
                && expected_minimum.is_none_or(|expected| Some(expected) == *actual_minimum)
        }
        (
            VariableSemantic::TaggedItems {
                tag_width: expected_tag_width,
                length_width: expected_length_width,
                variants: expected_variants,
            },
            VariableSemantic::TaggedItems {
                tag_width: actual_tag_width,
                length_width: actual_length_width,
                variants: actual_variants,
            },
        ) => {
            expected_tag_width == actual_tag_width
                && expected_length_width == actual_length_width
                && (expected_variants.is_empty() || expected_variants == actual_variants)
        }
        (VariableSemantic::TrailingBytes, VariableSemantic::TrailingBytes) => true,
        (
            VariableSemantic::BitmapItems {
                bitmap_field: expected_field,
                mask: expected_mask,
            },
            VariableSemantic::BitmapItems {
                bitmap_field: actual_field,
                mask: actual_mask,
            },
        ) => expected_field == actual_field && expected_mask == actual_mask,
        _ => false,
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

    fn layout(envelope: Envelope) -> WireLayout {
        WireLayout::byte_capacity(envelope.minimum(), envelope.maximum())
    }

    fn fixture_declaration(
        name: &str,
        code: u16,
        completion: CommandCompletion,
        request: Envelope,
    ) -> CommandDeclaration {
        CommandDeclaration {
            name: name.to_owned(),
            code,
            completion,
            request: layout(request),
            location: PathBuf::from("fixture.rs"),
        }
    }

    fn declaration_complete(returns: Envelope) -> CommandCompletion {
        CommandCompletion::CommandComplete {
            returns: layout(returns),
        }
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
        request: WireLayoutEvidence,
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

    fn fixture_standard_command(
        opcode: u16,
        completion: CatalogCompletion,
        request: WireLayoutEvidence,
    ) -> CatalogCommand {
        CatalogCommand {
            kind: CatalogCommandKind::StandardHci { opcode },
            name: format!("hci_fixture_{opcode:04x}"),
            source_name: "fixture.c".to_owned(),
            source_offset: 0,
            completion,
            request,
        }
    }

    fn catalog_complete(returns: WireLayoutEvidence) -> CatalogCompletion {
        CatalogCompletion::CommandComplete { returns }
    }

    fn fixture_event(code: u16, payload: WireLayoutEvidence) -> CatalogEvent {
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
            declaration_complete(Envelope::fixed(0)),
            Envelope::fixed(1),
        );
        let inactive = fixture_declaration(
            "Inactive",
            0x002,
            CommandCompletion::CommandStatus,
            Envelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![active, inactive], &["Active"]);
        let commands = vec![
            fixture_command(
                0x001,
                CatalogCompletion::CommandStatus {},
                WireLayoutEvidence::fixed(0),
            ),
            fixture_command(
                0x002,
                catalog_complete(WireLayoutEvidence::fixed(3)),
                WireLayoutEvidence::fixed(0),
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
                .any(|difference| difference.issue.contains("request payload layout"))
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
                payload: layout(Envelope::fixed(2)),
                location: PathBuf::from("event.rs"),
            },
        );
        coverage.events.insert(
            0x0401,
            EventDeclaration {
                name: "VariableEvent".to_owned(),
                code: 0x0401,
                payload: layout(Envelope::bounded(3, 253)),
                location: PathBuf::from("event.rs"),
            },
        );
        let events = vec![
            fixture_event(0x0400, WireLayoutEvidence::fixed(2)),
            fixture_event(0x0401, WireLayoutEvidence::known(3, 253)),
        ];

        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.checked, 2);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());

        coverage.events.get_mut(&0x0401).unwrap().payload = layout(Envelope::bounded(3, 252));
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("3..=253 bytes"));
        assert!(report.differences[0].issue.contains("3..=252 bytes"));

        coverage.events.get_mut(&0x0401).unwrap().payload = layout(Envelope::bounded(2, 253));
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("2..=253 bytes"));

        coverage.events.get_mut(&0x0401).unwrap().payload = layout(Envelope::bounded(4, 253));
        let report = compare_vendor_wire(&[], &events, &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("4..=253 bytes"));
    }

    #[test]
    fn checks_system_events_from_catalog_payload_evidence() {
        let mut coverage = fixture_coverage(Vec::new(), &[]);
        coverage.events.insert(
            0x9200,
            EventDeclaration {
                name: "CoprocessorReady".to_owned(),
                code: 0x9200,
                payload: layout(Envelope::fixed(1)),
                location: PathBuf::from("event.rs"),
            },
        );

        let mut event = CatalogEvent {
            kind: CatalogEventKind::SystemShci {
                payload: WireLayoutEvidence::fixed(1),
            },
            code: 0x9200,
            name: "SHCI_SUB_EVT_CODE_READY".to_owned(),
            source_name: "shci.h".to_owned(),
            source_offset: 0,
        };
        let report = compare_vendor_wire(&[], &[event.clone()], &coverage);
        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());

        event.kind = CatalogEventKind::SystemShci {
            payload: WireLayoutEvidence::fixed(2),
        };
        let report = compare_vendor_wire(&[], &[event], &coverage);
        assert_eq!(report.differences.len(), 1);
        assert!(report.differences[0].issue.contains("is 2 bytes"));
        assert!(report.differences[0].issue.contains("declares 1 bytes"));
    }

    #[test]
    fn accepts_status_and_fixed_response_envelopes() {
        let status = fixture_declaration(
            "Status",
            0x001,
            declaration_complete(Envelope::fixed(0)),
            Envelope::fixed(0),
        );
        let fixed = fixture_declaration(
            "Fixed",
            0x002,
            declaration_complete(Envelope::fixed(6)),
            Envelope::fixed(3),
        );
        let coverage = fixture_coverage(vec![status, fixed], &["Status", "Fixed"]);
        let commands = vec![
            fixture_command(
                0x001,
                catalog_complete(WireLayoutEvidence::fixed(0)),
                WireLayoutEvidence::fixed(0),
            ),
            fixture_command(
                0x002,
                catalog_complete(WireLayoutEvidence::fixed(6)),
                WireLayoutEvidence::fixed(3),
            ),
        ];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 4);
        assert!(report.differences.is_empty());
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn capacity_shaped_responses_reject_unannotated_subsets() {
        let contained = fixture_declaration(
            "Contained",
            0x010,
            declaration_complete(Envelope::bounded(1, 16)),
            Envelope::fixed(0),
        );
        let missing_prefix = fixture_declaration(
            "MissingPrefix",
            0x011,
            declaration_complete(Envelope::bounded(0, 16)),
            Envelope::fixed(0),
        );
        let too_large = fixture_declaration(
            "TooLarge",
            0x012,
            declaration_complete(Envelope::bounded(1, 252)),
            Envelope::fixed(0),
        );
        let coverage = fixture_coverage(
            vec![contained, missing_prefix, too_large],
            &["Contained", "MissingPrefix", "TooLarge"],
        );
        let command = |ocf| {
            fixture_command(
                ocf,
                catalog_complete(WireLayoutEvidence::known(1, 251)),
                WireLayoutEvidence::fixed(0),
            )
        };
        let commands = vec![command(0x010), command(0x011), command(0x012)];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 6);
        assert_eq!(report.differences.len(), 3);
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.command == "Contained")
        );
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
            declaration_complete(Envelope::bounded(1, 6)),
            Envelope::fixed(3),
        );
        let coverage = fixture_coverage(vec![declaration], &["Variable"]);
        let commands = vec![fixture_command(
            0x001,
            catalog_complete(WireLayoutEvidence::fixed(6)),
            WireLayoutEvidence::fixed(3),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.differences.len(), 1);
        assert!(
            report.differences[0]
                .issue
                .contains("command return payload layout")
        );
    }

    #[test]
    fn detects_an_incorrect_fixed_response_buffer_length() {
        let declaration = fixture_declaration(
            "Fixed",
            0x002,
            declaration_complete(Envelope::fixed(5)),
            Envelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![declaration], &["Fixed"]);
        let commands = vec![fixture_command(
            0x002,
            catalog_complete(WireLayoutEvidence::fixed(6)),
            WireLayoutEvidence::fixed(0),
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
            Envelope::fixed(2),
        );
        let dynamic = fixture_declaration(
            "Dynamic",
            0x007,
            CommandCompletion::CommandStatus,
            Envelope::bounded(1, 17),
        );
        let coverage = fixture_coverage(vec![wrong, dynamic], &["Wrong", "Dynamic"]);
        let commands = vec![
            fixture_command(
                0x006,
                CatalogCompletion::CommandStatus {},
                WireLayoutEvidence::fixed(3),
            ),
            fixture_command(
                0x007,
                CatalogCompletion::CommandStatus {},
                WireLayoutEvidence::known(1, 32),
            ),
        ];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 2);
        assert_eq!(report.differences.len(), 2);
        assert!(report.differences[0].issue.contains("is 3 bytes"));
        assert!(report.differences[0].issue.contains("declares 2 bytes"));
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.command == "Dynamic")
        );
        assert!(report.unavailable.is_empty());
    }

    #[test]
    fn compares_local_standard_hci_request_and_return_sizes() {
        let declaration = fixture_declaration(
            "LeFixture",
            0x201E,
            declaration_complete(Envelope::fixed(1)),
            Envelope::fixed(2),
        );
        let command = fixture_standard_command(
            0x201E,
            catalog_complete(WireLayoutEvidence::fixed(0)),
            WireLayoutEvidence::fixed(3),
        );
        let coverage = fixture_coverage(Vec::new(), &[]);

        let report = compare_wire(&[command], &[], &coverage, &[declaration]);

        assert_eq!(report.checked, 2);
        assert_eq!(report.differences.len(), 2);
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.issue.contains("request payload"))
        );
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.issue.contains("command return payload"))
        );
    }

    #[test]
    fn unresolved_requests_remain_unavailable() {
        let declaration = fixture_declaration(
            "Unresolved",
            0x008,
            CommandCompletion::CommandStatus,
            Envelope::bounded(1, 17),
        );
        let coverage = fixture_coverage(vec![declaration], &["Unresolved"]);
        let commands = vec![fixture_command(
            0x008,
            CatalogCompletion::CommandStatus {},
            WireLayoutEvidence::Unresolved("custom(value_len)".to_owned()),
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
            Envelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![declaration], &["UnresolvedCompletion"]);
        let commands = vec![fixture_command(
            0x009,
            CatalogCompletion::Unresolved {
                expression: "HCI_VENDOR_EVENT".to_owned(),
            },
            WireLayoutEvidence::fixed(0),
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
            declaration_complete(Envelope::fixed(1)),
            Envelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![declaration], &["UnresolvedResponse"]);
        let commands = vec![fixture_command(
            0x00a,
            catalog_complete(WireLayoutEvidence::Unresolved("computed_rlen".to_owned())),
            WireLayoutEvidence::fixed(0),
        )];

        let report = compare_vendor_wire(&commands, &[], &coverage);

        assert_eq!(report.checked, 1);
        assert!(report.differences.is_empty());
        assert_eq!(report.unavailable.len(), 1);
        assert!(report.unavailable[0].reason.contains("computed_rlen"));
    }

    #[test]
    fn envelope_only_capacity_requires_explicit_matching_storage() {
        let expected = Ok(EnvelopeExpectation {
            layout: layout(Envelope::bounded(2, 255)),
            relation: EnvelopeRelation::RequestCapacity,
        });

        let mut report = WireReport::default();
        let silently_narrowed = layout(Envelope::bounded(2, 48));
        compare_envelope(
            1,
            "ImplicitSubset",
            "request payload",
            expected.clone(),
            &silently_narrowed,
            &mut report,
        );
        let explicit_capacity = WireLayout::with_envelope(
            Envelope::bounded(2, 48),
            vec![WireSegment::fixed(2), WireSegment::variable(1, 0, 253)],
        )
        .unwrap();
        compare_envelope(
            2,
            "ExplicitSubset",
            "request payload",
            expected.clone(),
            &explicit_capacity,
            &mut report,
        );

        let missing_prefix = layout(Envelope::bounded(1, 48));
        compare_envelope(
            3,
            "MissingPrefix",
            "request payload",
            expected.clone(),
            &missing_prefix,
            &mut report,
        );
        let too_large = layout(Envelope::bounded(2, 256));
        compare_envelope(
            4,
            "TooLarge",
            "request payload",
            expected,
            &too_large,
            &mut report,
        );
        assert_eq!(report.differences.len(), 3);
        assert_eq!(report.differences[0].command, "ImplicitSubset");
    }

    #[test]
    fn equal_envelopes_do_not_hide_different_variable_field_structure() {
        let expected_layout =
            WireLayout::from_segments(vec![WireSegment::fixed(1), WireSegment::variable(2, 0, 2)])
                .unwrap();
        let actual_layout =
            WireLayout::from_segments(vec![WireSegment::fixed(1), WireSegment::variable(1, 0, 4)])
                .unwrap();
        assert_eq!(expected_layout.envelope(), actual_layout.envelope());

        let mut report = WireReport::default();
        compare_envelope(
            1,
            "DifferentStructure",
            "request payload",
            Ok(EnvelopeExpectation {
                layout: expected_layout,
                relation: EnvelopeRelation::RequestCapacity,
            }),
            &actual_layout,
            &mut report,
        );

        assert_eq!(report.differences.len(), 1);
    }

    #[test]
    fn equal_storage_does_not_hide_different_variable_semantics() {
        let expected_layout = WireLayout::from_segments(vec![
            WireSegment::fixed(1),
            WireSegment::variable_with_semantic(
                1,
                0,
                10,
                VariableSemantic::Counted { prefix_width: 1 },
            ),
        ])
        .unwrap();
        let actual_layout = WireLayout::from_segments(vec![
            WireSegment::fixed(1),
            WireSegment::variable_with_semantic(1, 0, 10, VariableSemantic::TrailingBytes),
        ])
        .unwrap();

        let mut report = WireReport::default();
        compare_envelope(
            1,
            "DifferentSemantics",
            "event payload",
            Ok(EnvelopeExpectation {
                layout: expected_layout,
                relation: EnvelopeRelation::EventCapacity,
            }),
            &actual_layout,
            &mut report,
        );

        assert_eq!(report.differences.len(), 1);
    }

    #[test]
    fn equal_envelopes_do_not_hide_reordered_fixed_fields() {
        let expected_layout =
            WireLayout::from_segments(vec![WireSegment::fixed(1), WireSegment::fixed(2)]).unwrap();
        let actual_layout =
            WireLayout::from_segments(vec![WireSegment::fixed(2), WireSegment::fixed(1)]).unwrap();
        assert_eq!(expected_layout.envelope(), actual_layout.envelope());

        let mut report = WireReport::default();
        compare_envelope(
            1,
            "ReorderedFields",
            "request payload",
            Ok(EnvelopeExpectation {
                layout: expected_layout,
                relation: EnvelopeRelation::Exact,
            }),
            &actual_layout,
            &mut report,
        );

        assert_eq!(report.differences.len(), 1);
    }

    #[test]
    fn narrower_semantics_require_an_explicit_matching_storage_capacity() {
        let expected_layout =
            WireLayout::from_segments(vec![WireSegment::fixed(1), WireSegment::variable(1, 0, 10)])
                .unwrap();
        let silently_narrowed =
            WireLayout::from_segments(vec![WireSegment::fixed(1), WireSegment::variable(1, 0, 5)])
                .unwrap();
        let explicit_capacity = WireLayout::with_envelope(
            silently_narrowed.envelope(),
            vec![WireSegment::fixed(1), WireSegment::variable(1, 0, 10)],
        )
        .unwrap();
        let silently_raised_minimum =
            WireLayout::from_segments(vec![WireSegment::fixed(1), WireSegment::variable(1, 1, 10)])
                .unwrap();
        let explicit_minimum = WireLayout::with_envelope(
            silently_raised_minimum.envelope(),
            vec![WireSegment::fixed(1), WireSegment::variable(1, 0, 10)],
        )
        .unwrap();
        let expectation = || EnvelopeExpectation {
            layout: expected_layout.clone(),
            relation: EnvelopeRelation::RequestCapacity,
        };

        let mut report = WireReport::default();
        compare_envelope(
            1,
            "ImplicitSubset",
            "request payload",
            Ok(expectation()),
            &silently_narrowed,
            &mut report,
        );
        compare_envelope(
            2,
            "ExplicitSubset",
            "request payload",
            Ok(expectation()),
            &explicit_capacity,
            &mut report,
        );
        compare_envelope(
            3,
            "ImplicitMinimum",
            "request payload",
            Ok(expectation()),
            &silently_raised_minimum,
            &mut report,
        );
        compare_envelope(
            4,
            "ExplicitMinimum",
            "request payload",
            Ok(expectation()),
            &explicit_minimum,
            &mut report,
        );

        assert_eq!(report.differences.len(), 2);
        assert_eq!(report.differences[0].command, "ImplicitSubset");
        assert_eq!(report.differences[1].command, "ImplicitMinimum");
    }

    #[test]
    fn reports_unknown_or_ambiguous_generated_commands_as_unavailable() {
        let missing = fixture_declaration(
            "Missing",
            0x004,
            declaration_complete(Envelope::fixed(0)),
            Envelope::fixed(0),
        );
        let ambiguous = fixture_declaration(
            "Ambiguous",
            0x005,
            declaration_complete(Envelope::fixed(0)),
            Envelope::fixed(0),
        );
        let coverage = fixture_coverage(vec![missing, ambiguous], &["Missing", "Ambiguous"]);
        let commands = vec![
            fixture_command(
                0x005,
                catalog_complete(WireLayoutEvidence::fixed(0)),
                WireLayoutEvidence::fixed(0),
            ),
            fixture_command(
                0x005,
                catalog_complete(WireLayoutEvidence::fixed(0)),
                WireLayoutEvidence::fixed(0),
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
