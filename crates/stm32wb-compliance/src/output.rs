//! Human and JSON rendering for CLI results.

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use stm32wb_compliance::{
    CheckReportJson, CommandChanges, CommandKey, CommandScope, EventChanges, EventScope,
    FirmwareVersion, VersionDiff,
};

use super::policy::{ExclusionPolicy, PolicyAudit};
use super::{BatchResult, CheckedRun, CubeProvenance};

pub(super) fn version_diff_to_json(
    diff: &VersionDiff,
    from: FirmwareVersion,
    to: FirmwareVersion,
    from_provenance: &CubeProvenance,
    to_provenance: &CubeProvenance,
    display_root: &Path,
) -> String {
    serde_json::to_string(&VersionDiffJson {
        mode: "version-diff",
        from: DiffEndpointJson {
            firmware: from.to_string(),
            feature: from.feature_name(),
            cube_provenance: cube_provenance_json(from_provenance, display_root),
        },
        to: DiffEndpointJson {
            firmware: to.to_string(),
            feature: to.feature_name(),
            cube_provenance: cube_provenance_json(to_provenance, display_root),
        },
        commands: &diff.commands,
        events: &diff.events,
    })
    .expect("a version diff JSON DTO can always serialize to JSON")
}

#[derive(Serialize)]
struct VersionDiffJson<'a> {
    mode: &'static str,
    from: DiffEndpointJson<'a>,
    to: DiffEndpointJson<'a>,
    commands: &'a CommandChanges,
    events: &'a EventChanges,
}

#[derive(Serialize)]
struct DiffEndpointJson<'a> {
    firmware: String,
    feature: String,
    cube_provenance: CubeProvenanceJson<'a>,
}

pub(super) fn diff_to_human(
    diff: &VersionDiff,
    from: FirmwareVersion,
    to: FirmwareVersion,
    from_provenance: &CubeProvenance,
    to_provenance: &CubeProvenance,
    display_root: &Path,
) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "STM32CubeWB version diff: {from} ({}) -> {to} ({})",
        diff.from.cube_tag, diff.to.cube_tag
    );
    let _ = writeln!(
        output,
        "CubeWB checkout: {}",
        from_provenance.display_path(display_root)
    );
    let _ = writeln!(
        output,
        "  from: {} ({})",
        from_provenance.tag, from_provenance.commit
    );
    let _ = writeln!(
        output,
        "  to: {} ({})",
        to_provenance.tag, to_provenance.commit
    );
    write_command_changes(&mut output, &diff.commands);
    write_event_changes(&mut output, &diff.events);
    let _ = writeln!(
        output,
        "result: {}",
        if diff.has_changes() {
            "differences found"
        } else {
            "no differences"
        }
    );
    output
}

fn write_command_changes(output: &mut String, changes: &CommandChanges) {
    let _ = writeln!(
        output,
        "commands: {} added / {} removed / {} changed",
        changes.added.len(),
        changes.removed.len(),
        changes.changed.len()
    );
    for command in &changes.added {
        let _ = writeln!(
            output,
            "  + {} 0x{:04X}: {}",
            command_scope_name(command.scope()),
            command.code(),
            command.name
        );
    }
    for command in &changes.removed {
        let _ = writeln!(
            output,
            "  - {} 0x{:04X}: {}",
            command_scope_name(command.scope()),
            command.code(),
            command.name
        );
    }
    for changed in &changes.changed {
        write_changed_command(output, changed.key, &changed.from.name, &changed.to.name);
    }
}

fn write_changed_command(output: &mut String, key: CommandKey, from: &str, to: &str) {
    if from == to {
        let _ = writeln!(
            output,
            "  ~ {} 0x{:04X}: {from} (wire metadata changed)",
            command_scope_name(key.scope),
            key.code,
        );
    } else {
        let _ = writeln!(
            output,
            "  ~ {} 0x{:04X}: {from} -> {to}",
            command_scope_name(key.scope),
            key.code,
        );
    }
}

fn write_event_changes(output: &mut String, changes: &EventChanges) {
    let _ = writeln!(
        output,
        "events: {} added / {} removed / {} changed",
        changes.added.len(),
        changes.removed.len(),
        changes.changed.len()
    );
    for event in &changes.added {
        let _ = writeln!(
            output,
            "  + {} 0x{:04X}: {}",
            event_scope_name(event.scope()),
            event.code,
            event.name
        );
    }
    for event in &changes.removed {
        let _ = writeln!(
            output,
            "  - {} 0x{:04X}: {}",
            event_scope_name(event.scope()),
            event.code,
            event.name
        );
    }
    for changed in &changes.changed {
        let _ = writeln!(
            output,
            "  ~ {} 0x{:04X}: {} -> {}",
            event_scope_name(changed.key.scope),
            changed.key.code,
            changed.from.name,
            changed.to.name
        );
    }
}

fn command_scope_name(scope: CommandScope) -> &'static str {
    match scope {
        CommandScope::VendorAci => "vendor ACI OCF",
        CommandScope::StandardHci => "standard HCI opcode",
    }
}

fn event_scope_name(scope: EventScope) -> &'static str {
    match scope {
        EventScope::VendorAci => "vendor ACI event",
        EventScope::SystemShci => "system SHCI event",
        EventScope::StandardHci => "standard HCI event",
        EventScope::LeMeta => "LE Meta subevent",
    }
}

pub(super) fn checked_run_to_human(
    run: &CheckedRun,
    policy: &ExclusionPolicy,
    crate_dir: &Path,
) -> String {
    let report = run.report.to_human();
    let (heading, remainder) = report.split_once('\n').unwrap_or((&report, ""));
    let mut output = String::new();
    let _ = writeln!(output, "{heading}");
    let _ = writeln!(output, "CubeWB provenance:");
    let _ = writeln!(
        output,
        "  checkout: {}",
        run.provenance.display_path(crate_dir)
    );
    let _ = writeln!(
        output,
        "  tag: {} (tag object {})",
        run.provenance.tag, run.provenance.tag_object
    );
    let _ = writeln!(output, "  resolved commit: {}", run.provenance.commit);
    let _ = writeln!(
        output,
        "exclusion policy: {} ({} command + {} event entries, all actively suppress a difference)",
        policy.display_path(crate_dir),
        run.policy_audit.command_entries,
        run.policy_audit.event_entries,
    );
    output.push_str(remainder);
    output
}

pub(super) fn checked_run_to_json(
    run: &CheckedRun,
    policy: &ExclusionPolicy,
    crate_dir: &Path,
) -> String {
    serde_json::to_string(&checked_run_json(run, policy, crate_dir))
        .expect("a checked compliance result can always serialize to JSON")
}

fn checked_run_json<'a>(
    run: &'a CheckedRun,
    policy: &ExclusionPolicy,
    crate_dir: &Path,
) -> CheckedRunJson<'a> {
    CheckedRunJson {
        report: run.report.json(),
        firmware_feature: run.firmware.feature_name(),
        cube_provenance: cube_provenance_json(&run.provenance, crate_dir),
        exclusion_policy: policy_audit_json(&run.policy_audit, policy, crate_dir),
    }
}

#[derive(Serialize)]
struct CheckedRunJson<'a> {
    #[serde(flatten)]
    report: CheckReportJson<'a>,
    firmware_feature: String,
    cube_provenance: CubeProvenanceJson<'a>,
    exclusion_policy: PolicyAuditJson,
}

pub(super) fn batch_to_human(
    results: &[BatchResult],
    policy: &ExclusionPolicy,
    crate_dir: &Path,
) -> String {
    let mut output = String::new();
    let mut successful = 0usize;
    let mut noncompliant = 0usize;
    let mut errors = 0usize;

    for result in results {
        match result {
            BatchResult::Success(result) => {
                successful += 1;
                noncompliant += usize::from(!result.report.is_compliant());
                let _ = writeln!(
                    output,
                    "=== {} ({}) ===",
                    result.firmware,
                    result.firmware.feature_name()
                );
                output.push_str(&checked_run_to_human(result, policy, crate_dir));
            }
            BatchResult::Error { firmware, error } => {
                errors += 1;
                let _ = writeln!(output, "=== {firmware} ({}) ===", firmware.feature_name());
                let _ = writeln!(output, "error: {error}");
            }
        }
    }

    let _ = writeln!(
        output,
        "all-supported summary: {successful} checked, {noncompliant} non-compliant, {errors} errors"
    );
    output
}

pub(super) fn batch_to_json(
    results: &[BatchResult],
    policy: &ExclusionPolicy,
    crate_dir: &Path,
) -> String {
    let successful = results
        .iter()
        .filter(|result| matches!(result, BatchResult::Success(_)))
        .count();
    let errors = results.len() - successful;
    let noncompliant = results
        .iter()
        .filter(|result| result.is_noncompliant())
        .count();

    let results = results
        .iter()
        .map(|result| match result {
            BatchResult::Success(result) => BatchResultJson::Success {
                firmware: result.firmware.to_string(),
                feature: result.firmware.feature_name(),
                report: Box::new(checked_run_json(result, policy, crate_dir)),
            },
            BatchResult::Error { firmware, error } => BatchResultJson::Error {
                firmware: firmware.to_string(),
                feature: firmware.feature_name(),
                error,
            },
        })
        .collect();
    let report = BatchJson {
        mode: "all-supported",
        compliant: errors == 0 && noncompliant == 0,
        exclusion_policy: PolicyMetadataJson::from_policy(policy, crate_dir),
        results,
        summary: BatchSummary {
            checked: successful,
            noncompliant_reports: noncompliant,
            errors,
        },
    };
    serde_json::to_string(&report).expect("a batch compliance result can always serialize to JSON")
}

#[derive(Serialize)]
struct BatchJson<'a> {
    mode: &'static str,
    compliant: bool,
    exclusion_policy: PolicyMetadataJson,
    results: Vec<BatchResultJson<'a>>,
    summary: BatchSummary,
}

#[derive(Serialize)]
#[serde(tag = "status")]
enum BatchResultJson<'a> {
    #[serde(rename = "ok")]
    Success {
        firmware: String,
        feature: String,
        report: Box<CheckedRunJson<'a>>,
    },
    #[serde(rename = "error")]
    Error {
        firmware: String,
        feature: String,
        error: &'a str,
    },
}

#[derive(Serialize)]
struct BatchSummary {
    checked: usize,
    noncompliant_reports: usize,
    errors: usize,
}

#[derive(Serialize)]
struct CubeProvenanceJson<'a> {
    checkout: String,
    tag: &'a str,
    tag_object: &'a str,
    commit: &'a str,
}

fn cube_provenance_json<'a>(provenance: &'a CubeProvenance, root: &Path) -> CubeProvenanceJson<'a> {
    CubeProvenanceJson {
        checkout: provenance.display_path(root),
        tag: &provenance.tag,
        tag_object: &provenance.tag_object,
        commit: &provenance.commit,
    }
}

#[derive(Serialize)]
struct PolicyAuditJson {
    path: String,
    active_command_entries: usize,
    active_event_entries: usize,
    all_entries_suppress_differences: bool,
}

fn policy_audit_json(
    audit: &PolicyAudit,
    policy: &ExclusionPolicy,
    crate_dir: &Path,
) -> PolicyAuditJson {
    PolicyAuditJson {
        path: policy.display_path(crate_dir),
        active_command_entries: audit.command_entries,
        active_event_entries: audit.event_entries,
        all_entries_suppress_differences: true,
    }
}

#[derive(Serialize)]
struct PolicyMetadataJson {
    path: String,
    entries: usize,
}

impl PolicyMetadataJson {
    fn from_policy(policy: &ExclusionPolicy, crate_dir: &Path) -> Self {
        Self {
            path: policy.display_path(crate_dir),
            entries: policy.entry_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use stm32wb_compliance::{CheckReport, ProtocolCoverage, StandardHciCoverage, WireReport};

    use super::*;

    #[test]
    fn batch_json_is_structured() {
        let firmware = FirmwareVersion::new(1, 15, 0);
        let report = CheckReport::new(
            firmware,
            ProtocolCoverage::default(),
            ProtocolCoverage::default(),
            StandardHciCoverage::default(),
            Vec::new(),
            WireReport::default(),
            Default::default(),
            Default::default(),
        );
        let policy = ExclusionPolicy::empty(PathBuf::from("policy"));
        let results = [BatchResult::Success(Box::new(CheckedRun {
            firmware,
            report,
            provenance: CubeProvenance {
                cube_dir: PathBuf::from("cube"),
                tag: "v1.15.0".to_owned(),
                tag_object: "tag-object".to_owned(),
                commit: "commit".to_owned(),
            },
            policy_audit: PolicyAudit {
                command_entries: 0,
                event_entries: 0,
            },
        }))];

        let output = batch_to_json(&results, &policy, Path::new("."));
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["mode"], "all-supported");
        assert_eq!(value["summary"]["checked"], 1);
        assert_eq!(value["results"][0]["status"], "ok");
        assert_eq!(value["results"][0]["report"]["firmware"], "1.15.0");
        assert_eq!(
            value["results"][0]["report"]["cube_provenance"]["commit"],
            "commit"
        );
    }
}
