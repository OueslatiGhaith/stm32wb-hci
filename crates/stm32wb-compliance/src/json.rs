//! Stable, derived JSON views for compliance-domain values.
//!
//! The domain model retains source locations and intermediate inventories that
//! are useful while comparing firmware APIs but should not become part of the
//! machine-readable CLI contract. These DTOs explicitly define that contract
//! and use `serde` derives exclusively; no domain type implements `Serialize`.

use serde::Serialize;

use crate::model::{CheckReport, CoverageDifference, ExcludedCode};
use crate::wire::{WireDifference, WireReport, WireUnavailable};

/// Schema version of the machine-readable compliance report.
pub const REPORT_SCHEMA_VERSION: u16 = 1;

/// The stable machine-readable representation of one compliance report.
///
/// Breaking changes to this DTO require incrementing
/// [`REPORT_SCHEMA_VERSION`]. Use [`CheckReport::json`] instead of serializing
/// the domain model directly.
#[derive(Serialize)]
pub struct CheckReportJson<'a> {
    schema_version: u16,
    firmware: String,
    cube_tag: &'a str,
    compliant: bool,
    catalog_counts: CatalogCounts,
    missing_commands: Vec<CoverageDifferenceJson<'a>>,
    extraneous_commands: Vec<CoverageDifferenceJson<'a>>,
    missing_events: Vec<CoverageDifferenceJson<'a>>,
    extraneous_events: Vec<CoverageDifferenceJson<'a>>,
    missing_standard_hci_commands: Vec<CoverageDifferenceJson<'a>>,
    missing_standard_hci_events: Vec<CoverageDifferenceJson<'a>>,
    missing_standard_hci_le_meta_events: Vec<CoverageDifferenceJson<'a>>,
    wire: WireReportJson<'a>,
    excluded_commands: Vec<ExcludedCodeJson<'a>>,
    excluded_events: Vec<ExcludedCodeJson<'a>>,
}

impl<'a> From<&'a CheckReport> for CheckReportJson<'a> {
    fn from(report: &'a CheckReport) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            firmware: report.firmware.to_string(),
            cube_tag: &report.cube_tag,
            compliant: report.is_compliant(),
            catalog_counts: CatalogCounts::from(report),
            missing_commands: coverage_differences(&report.missing_commands),
            extraneous_commands: coverage_differences(&report.extraneous_commands),
            missing_events: coverage_differences(&report.missing_events),
            extraneous_events: coverage_differences(&report.extraneous_events),
            missing_standard_hci_commands: coverage_differences(
                &report.missing_standard_hci_commands,
            ),
            missing_standard_hci_events: coverage_differences(&report.missing_standard_hci_events),
            missing_standard_hci_le_meta_events: coverage_differences(
                &report.missing_standard_hci_le_meta_events,
            ),
            wire: WireReportJson::from(&report.wire),
            excluded_commands: excluded_codes(&report.excluded_commands),
            excluded_events: excluded_codes(&report.excluded_events),
        }
    }
}

impl CheckReport {
    /// Build the stable JSON DTO for this report.
    pub fn json(&self) -> CheckReportJson<'_> {
        self.into()
    }

    /// Serialize the stable JSON DTO for compatibility with existing callers.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.json())
            .expect("a CheckReport JSON DTO can always serialize to JSON")
    }
}

#[derive(Serialize)]
struct CatalogCounts {
    vendor_command_ids: usize,
    vendor_event_ids: usize,
    active_command_ids: usize,
    active_event_ids: usize,
    standard_hci_command_opcodes: usize,
    standard_hci_event_codes: usize,
    standard_hci_le_meta_event_codes: usize,
    standard_hci_provider_command_opcodes: usize,
    standard_hci_provider_event_codes: usize,
    standard_hci_provider_le_meta_event_codes: usize,
}

impl From<&CheckReport> for CatalogCounts {
    fn from(report: &CheckReport) -> Self {
        Self {
            vendor_command_ids: report.vendor.command_codes().len(),
            vendor_event_ids: report.vendor.event_codes().len(),
            active_command_ids: report.active_api.command_codes().len(),
            active_event_ids: report.active_api.event_codes().len(),
            standard_hci_command_opcodes: report.standard_hci.commands.len(),
            standard_hci_event_codes: report.standard_hci.events.len(),
            standard_hci_le_meta_event_codes: report.standard_hci.le_meta_events.len(),
            standard_hci_provider_command_opcodes: report.standard_hci_provider.commands.len(),
            standard_hci_provider_event_codes: report.standard_hci_provider.events.len(),
            standard_hci_provider_le_meta_event_codes: report
                .standard_hci_provider
                .le_meta_events
                .len(),
        }
    }
}

#[derive(Serialize)]
struct CoverageDifferenceJson<'a> {
    code: u16,
    hex: String,
    expected: &'a [String],
    observed: &'a [String],
}

impl<'a> From<&'a CoverageDifference> for CoverageDifferenceJson<'a> {
    fn from(difference: &'a CoverageDifference) -> Self {
        Self {
            code: difference.code,
            hex: hex_code(difference.code),
            expected: &difference.expected,
            observed: &difference.observed,
        }
    }
}

fn coverage_differences(differences: &[CoverageDifference]) -> Vec<CoverageDifferenceJson<'_>> {
    differences
        .iter()
        .map(CoverageDifferenceJson::from)
        .collect()
}

#[derive(Serialize)]
struct ExcludedCodeJson<'a> {
    code: u16,
    hex: String,
    reason: &'a str,
}

impl<'a> From<&'a ExcludedCode> for ExcludedCodeJson<'a> {
    fn from(excluded: &'a ExcludedCode) -> Self {
        Self {
            code: excluded.code,
            hex: hex_code(excluded.code),
            reason: &excluded.reason,
        }
    }
}

fn excluded_codes(codes: &[ExcludedCode]) -> Vec<ExcludedCodeJson<'_>> {
    codes.iter().map(ExcludedCodeJson::from).collect()
}

#[derive(Serialize)]
struct WireReportJson<'a> {
    checked: usize,
    differences: Vec<WireDifferenceJson<'a>>,
    unavailable: Vec<WireUnavailableJson<'a>>,
}

impl<'a> From<&'a WireReport> for WireReportJson<'a> {
    fn from(report: &'a WireReport) -> Self {
        Self {
            checked: report.checked,
            differences: report
                .differences
                .iter()
                .map(WireDifferenceJson::from)
                .collect(),
            unavailable: report
                .unavailable
                .iter()
                .map(WireUnavailableJson::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct WireDifferenceJson<'a> {
    code: u16,
    hex: String,
    command: &'a str,
    issue: &'a str,
}

impl<'a> From<&'a WireDifference> for WireDifferenceJson<'a> {
    fn from(difference: &'a WireDifference) -> Self {
        Self {
            code: difference.code,
            hex: hex_code(difference.code),
            command: &difference.command,
            issue: &difference.issue,
        }
    }
}

#[derive(Serialize)]
struct WireUnavailableJson<'a> {
    code: u16,
    hex: String,
    command: &'a str,
    reason: &'a str,
}

impl<'a> From<&'a WireUnavailable> for WireUnavailableJson<'a> {
    fn from(unavailable: &'a WireUnavailable) -> Self {
        Self {
            code: unavailable.code,
            hex: hex_code(unavailable.code),
            command: &unavailable.command,
            reason: &unavailable.reason,
        }
    }
}

fn hex_code(code: u16) -> String {
    format!("0x{code:04X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_json_dto_keeps_codes_machine_and_human_readable() {
        let report = WireReport {
            checked: 1,
            differences: vec![WireDifference {
                code: 0x00AB,
                command: "Mismatch".to_owned(),
                issue: "expected an inline Return schema".to_owned(),
            }],
            unavailable: vec![WireUnavailable {
                code: 0x00CD,
                command: "Unavailable".to_owned(),
                reason: "response length is capacity-dependent".to_owned(),
            }],
        };

        assert_eq!(
            serde_json::to_value(WireReportJson::from(&report)).unwrap(),
            serde_json::json!({
                "checked": 1,
                "differences": [{
                    "code": 0x00AB,
                    "hex": "0x00AB",
                    "command": "Mismatch",
                    "issue": "expected an inline Return schema",
                }],
                "unavailable": [{
                    "code": 0x00CD,
                    "hex": "0x00CD",
                    "command": "Unavailable",
                    "reason": "response length is capacity-dependent",
                }],
            })
        );
    }
}
