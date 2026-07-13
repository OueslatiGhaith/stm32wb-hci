use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::str::FromStr;

use clap::{CommandFactory, Parser, Subcommand};
use serde::Serialize;
use stm32wb_compliance::{
    CATALOG_SCHEMA_VERSION, CheckOptions, CheckReport, CheckReportJson, CommandChanges, CommandKey,
    CommandScope, EventChanges, EventScope, FirmwareVersion, VersionDiff, check, diff_catalogs,
    find_crate_root, load_catalog,
};

const DEFAULT_POLICY_PATH: &str = "tools/compliance/exclusions.policy";
const POLICY_FORMAT_VERSION: u32 = 1;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let status = error.exit_code();
            let _ = error.print();
            return if status == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            };
        }
    };

    match run(cli) {
        Ok(status) => status,
        Err(error) => {
            eprintln!("error: {error}\n\n{}", usage());
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    cli.validate()?;

    let current_dir =
        env::current_dir().map_err(|error| format!("could not read current directory: {error}"))?;
    match cli.command.expect("validation requires a command") {
        CliCommand::ListSupported => list_supported(&crate_dir(&cli, &current_dir)?),
        CliCommand::Check => run_check(&cli, crate_dir(&cli, &current_dir)?),
        CliCommand::Diff => run_diff(&cli, &current_dir),
    }
}

fn crate_dir(cli: &Cli, current_dir: &Path) -> Result<PathBuf, String> {
    cli.crate_dir
        .clone()
        .or_else(|| find_crate_root(current_dir))
        .ok_or_else(|| "could not locate the stm32wb-hci crate; pass --crate <path>".to_owned())
}

fn list_supported(crate_dir: &Path) -> Result<ExitCode, String> {
    for firmware in
        FirmwareVersion::declared_in_manifest(crate_dir).map_err(|error| error.to_string())?
    {
        println!("{}", firmware.feature_name());
    }
    Ok(ExitCode::SUCCESS)
}

fn run_check(cli: &Cli, crate_dir: PathBuf) -> Result<ExitCode, String> {
    let declared_firmwares =
        FirmwareVersion::declared_in_manifest(&crate_dir).map_err(|error| error.to_string())?;
    let firmwares = if cli.all_supported {
        declared_firmwares.clone()
    } else {
        let firmware = cli
            .firmware
            .expect("the parser requires --firmware or --all-supported");
        if !declared_firmwares.contains(&firmware) {
            return Err(format!(
                "firmware {firmware} is not declared by {}; add `{}` to [features] or use `list-supported`",
                crate_dir.join("Cargo.toml").display(),
                firmware.feature_name()
            ));
        }
        vec![firmware]
    };

    let cube_dir = cli
        .cube_dir
        .clone()
        .unwrap_or_else(|| crate_dir.join("STM32CubeWB"));
    let policy_path = cli
        .policy_path
        .clone()
        .unwrap_or_else(|| crate_dir.join(DEFAULT_POLICY_PATH));
    let policy = ExclusionPolicy::load(policy_path)?;
    policy.validate_for(&declared_firmwares)?;

    let mut results = Vec::with_capacity(firmwares.len());
    for firmware in firmwares {
        match run_one_check(firmware, &crate_dir, &cube_dir, &policy, cli.skip_build) {
            Ok(result) => results.push(BatchResult::Success(Box::new(result))),
            Err(error) => results.push(BatchResult::Error { firmware, error }),
        }
    }

    if cli.all_supported {
        if cli.json {
            println!("{}", batch_to_json(&results, &policy, &crate_dir));
        } else {
            print!("{}", batch_to_human(&results, &policy, &crate_dir));
        }
    } else {
        let result = results
            .into_iter()
            .next()
            .expect("at least one requested firmware");
        match result {
            BatchResult::Success(result) => {
                if cli.json {
                    println!("{}", result.to_json(&policy, &crate_dir));
                } else {
                    print!("{}", result.to_human(&policy, &crate_dir));
                }
                return Ok(if cli.deny && !result.report.is_compliant() {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                });
            }
            BatchResult::Error { error, .. } => return Err(error),
        }
    }

    let has_errors = results.iter().any(BatchResult::is_error);
    let has_differences = results.iter().any(BatchResult::has_differences);
    Ok(if has_errors {
        ExitCode::from(2)
    } else if cli.deny && has_differences {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn run_diff(cli: &Cli, current_dir: &Path) -> Result<ExitCode, String> {
    let from = cli.from.expect("validation requires --from");
    let to = cli.to.expect("validation requires --to");
    let crate_dir = cli
        .crate_dir
        .clone()
        .or_else(|| find_crate_root(current_dir));
    let cube_dir = cli
        .cube_dir
        .clone()
        .or_else(|| crate_dir.as_ref().map(|path| path.join("STM32CubeWB")))
        .ok_or_else(|| {
            "could not locate the stm32wb-hci crate for the default CubeWB path; pass --cube <path>"
                .to_owned()
        })?;
    let display_root = crate_dir.as_deref().unwrap_or(current_dir);

    let from_provenance = CubeProvenance::resolve(&cube_dir, from.cube_tag())?;
    let to_provenance = CubeProvenance::resolve(&cube_dir, to.cube_tag())?;
    let from_catalog = load_catalog(&cube_dir, from).map_err(|error| error.to_string())?;
    let to_catalog = load_catalog(&cube_dir, to).map_err(|error| error.to_string())?;
    let diff = diff_catalogs(&from_catalog, &to_catalog).map_err(|error| error.to_string())?;

    if cli.json {
        println!(
            "{}",
            serde_json::to_string(&VersionDiffJson {
                mode: "version-diff",
                catalog_schema_version: CATALOG_SCHEMA_VERSION,
                from: DiffEndpointJson {
                    firmware: from.to_string(),
                    feature: from.feature_name(),
                    cube_provenance: from_provenance.json(display_root),
                },
                to: DiffEndpointJson {
                    firmware: to.to_string(),
                    feature: to.feature_name(),
                    cube_provenance: to_provenance.json(display_root),
                },
                commands: &diff.commands,
                events: &diff.events,
            })
            .expect("a version diff JSON DTO can always serialize to JSON")
        );
    } else {
        print!(
            "{}",
            diff_to_human(
                &diff,
                from,
                to,
                &from_provenance,
                &to_provenance,
                display_root,
            )
        );
    }

    Ok(if cli.deny && diff.has_changes() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

#[derive(Serialize)]
struct VersionDiffJson<'a> {
    mode: &'static str,
    catalog_schema_version: u16,
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

fn diff_to_human(
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
            command_scope_name(command.scope),
            command.opcode.unwrap_or(command.ocf),
            command.name
        );
    }
    for command in &changes.removed {
        let _ = writeln!(
            output,
            "  - {} 0x{:04X}: {}",
            command_scope_name(command.scope),
            command.opcode.unwrap_or(command.ocf),
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
            event_scope_name(event.scope),
            event.code,
            event.name
        );
    }
    for event in &changes.removed {
        let _ = writeln!(
            output,
            "  - {} 0x{:04X}: {}",
            event_scope_name(event.scope),
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
        EventScope::StandardHci => "standard HCI event",
        EventScope::LeMeta => "LE Meta subevent",
    }
}

fn run_one_check(
    firmware: FirmwareVersion,
    crate_dir: &Path,
    cube_dir: &Path,
    policy: &ExclusionPolicy,
    skip_build: bool,
) -> Result<CheckedRun, String> {
    let provenance = CubeProvenance::resolve(cube_dir, firmware.cube_tag())?;
    let active_exclusions = policy.active_for(firmware);

    let mut options = CheckOptions::new(firmware, crate_dir.to_path_buf(), cube_dir.to_path_buf());
    options
        .excluded_commands
        .extend(active_exclusions.commands.clone());
    options
        .excluded_events
        .extend(active_exclusions.events.clone());
    options.skip_build = skip_build;

    let report = check(&options).map_err(|error| error.to_string())?;
    let policy_audit = active_exclusions.audit(&report, firmware)?;

    Ok(CheckedRun {
        firmware,
        report,
        provenance,
        policy_audit,
    })
}

#[derive(Debug)]
struct CheckedRun {
    firmware: FirmwareVersion,
    report: CheckReport,
    provenance: CubeProvenance,
    policy_audit: PolicyAudit,
}

impl CheckedRun {
    fn to_human(&self, policy: &ExclusionPolicy, crate_dir: &Path) -> String {
        let report = self.report.to_human();
        let (heading, remainder) = report.split_once('\n').unwrap_or((&report, ""));
        let mut output = String::new();
        let _ = writeln!(output, "{heading}");
        let _ = writeln!(output, "CubeWB provenance:");
        let _ = writeln!(
            output,
            "  checkout: {}",
            self.provenance.display_path(crate_dir)
        );
        let _ = writeln!(
            output,
            "  tag: {} (tag object {})",
            self.provenance.tag, self.provenance.tag_object
        );
        let _ = writeln!(output, "  resolved commit: {}", self.provenance.commit);
        let _ = writeln!(
            output,
            "exclusion policy: {} (format {}; {} command + {} event entries, all actively suppress a difference)",
            policy.display_path(crate_dir),
            POLICY_FORMAT_VERSION,
            self.policy_audit.command_entries,
            self.policy_audit.event_entries,
        );
        output.push_str(remainder);
        output
    }

    fn to_json(&self, policy: &ExclusionPolicy, crate_dir: &Path) -> String {
        serde_json::to_string(&self.json(policy, crate_dir))
            .expect("a checked compliance result can always serialize to JSON")
    }

    fn json<'a>(&'a self, policy: &ExclusionPolicy, crate_dir: &Path) -> CheckedRunJson<'a> {
        CheckedRunJson {
            report: self.report.json(),
            firmware_feature: self.firmware.feature_name(),
            cube_provenance: self.provenance.json(crate_dir),
            exclusion_policy: self.policy_audit.json(policy, crate_dir),
        }
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

#[derive(Debug)]
enum BatchResult {
    Success(Box<CheckedRun>),
    Error {
        firmware: FirmwareVersion,
        error: String,
    },
}

impl BatchResult {
    fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    fn has_differences(&self) -> bool {
        matches!(self, Self::Success(result) if !result.report.is_compliant())
    }
}

fn batch_to_human(results: &[BatchResult], policy: &ExclusionPolicy, crate_dir: &Path) -> String {
    let mut output = String::new();
    let mut successful = 0usize;
    let mut differences = 0usize;
    let mut errors = 0usize;

    for result in results {
        match result {
            BatchResult::Success(result) => {
                successful += 1;
                differences += usize::from(!result.report.is_compliant());
                let _ = writeln!(
                    output,
                    "=== {} ({}) ===",
                    result.firmware,
                    result.firmware.feature_name()
                );
                output.push_str(&result.to_human(policy, crate_dir));
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
        "all-supported summary: {successful} checked, {differences} with coverage differences, {errors} errors"
    );
    output
}

fn batch_to_json(results: &[BatchResult], policy: &ExclusionPolicy, crate_dir: &Path) -> String {
    let successful = results
        .iter()
        .filter(|result| matches!(result, BatchResult::Success(_)))
        .count();
    let errors = results.len() - successful;
    let differences = results
        .iter()
        .filter(|result| result.has_differences())
        .count();

    let results = results
        .iter()
        .map(|result| match result {
            BatchResult::Success(result) => BatchResultJson::Success {
                firmware: result.firmware.to_string(),
                feature: result.firmware.feature_name(),
                report: Box::new(result.json(policy, crate_dir)),
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
        compliant: errors == 0 && differences == 0,
        exclusion_policy: PolicyMetadataJson::from_policy(policy, crate_dir),
        results,
        summary: BatchSummary {
            checked: successful,
            coverage_differences: differences,
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
    coverage_differences: usize,
    errors: usize,
}

#[derive(Clone, Debug)]
struct CubeProvenance {
    cube_dir: PathBuf,
    tag: String,
    tag_object: String,
    commit: String,
}

impl CubeProvenance {
    fn resolve(cube_dir: &Path, tag: String) -> Result<Self, String> {
        let tag_ref = format!("refs/tags/{tag}");
        let tag_object = git_rev_parse(cube_dir, &tag_ref)?;
        let commit = git_rev_parse(cube_dir, &format!("{tag_ref}^{{commit}}"))?;
        Ok(Self {
            cube_dir: cube_dir
                .canonicalize()
                .unwrap_or_else(|_| cube_dir.to_path_buf()),
            tag,
            tag_object,
            commit,
        })
    }

    fn display_path(&self, crate_dir: &Path) -> String {
        display_path(&self.cube_dir, crate_dir)
    }

    fn json(&self, crate_dir: &Path) -> CubeProvenanceJson<'_> {
        CubeProvenanceJson {
            checkout: self.display_path(crate_dir),
            tag: &self.tag,
            tag_object: &self.tag_object,
            commit: &self.commit,
        }
    }
}

#[derive(Serialize)]
struct CubeProvenanceJson<'a> {
    checkout: String,
    tag: &'a str,
    tag_object: &'a str,
    commit: &'a str,
}

fn git_rev_parse(cube_dir: &Path, revision: &str) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cube_dir)
        .args(["rev-parse", "--verify", "--quiet", revision])
        .output()
        .map_err(|error| format!("could not run git for {}: {error}", cube_dir.display()))?;
    if !output.status.success() {
        return Err(format!(
            "CubeWB revision {revision:?} was not found in {}",
            cube_dir.display()
        ));
    }
    let object_id = String::from_utf8(output.stdout)
        .map_err(|error| format!("git rev-parse returned non-UTF-8 output: {error}"))?;
    let object_id = object_id.trim();
    if object_id.is_empty() || !object_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "git rev-parse returned an invalid object ID for {revision:?} in {}",
            cube_dir.display()
        ));
    }
    Ok(object_id.to_owned())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CoverageKind {
    Command,
    Event,
}

impl CoverageKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "command" => Some(Self::Command),
            "event" => Some(Self::Event),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Event => "event",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FirmwareSelector {
    All,
    Only(FirmwareVersion),
}

impl FirmwareSelector {
    fn parse(value: &str) -> Result<Self, String> {
        if value == "*" {
            Ok(Self::All)
        } else {
            FirmwareVersion::from_str(value)
                .map(Self::Only)
                .map_err(|error| error.to_string())
        }
    }

    fn matches(self, firmware: FirmwareVersion) -> bool {
        match self {
            Self::All => true,
            Self::Only(selected) => selected == firmware,
        }
    }
}

#[derive(Clone, Debug)]
struct PolicyEntry {
    kind: CoverageKind,
    code: u16,
    selector: FirmwareSelector,
    reason: String,
    line: usize,
}

#[derive(Clone, Debug)]
struct ExclusionPolicy {
    path: PathBuf,
    entries: Vec<PolicyEntry>,
}

impl ExclusionPolicy {
    fn load(path: PathBuf) -> Result<Self, String> {
        let source = fs::read_to_string(&path).map_err(|error| {
            format!(
                "could not read exclusion policy {}: {error}",
                path.display()
            )
        })?;
        Self::parse(path, &source)
    }

    fn parse(path: PathBuf, source: &str) -> Result<Self, String> {
        let mut version = None;
        let mut entries = Vec::new();
        let mut raw_entries = BTreeSet::new();

        for (index, raw_line) in source.lines().enumerate() {
            let line_number = index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=')
                && key.trim() == "version"
            {
                if version.is_some() {
                    return Err(policy_error(
                        &path,
                        line_number,
                        "policy format version is declared more than once",
                    ));
                }
                let parsed = value.trim().parse::<u32>().map_err(|_| {
                    policy_error(
                        &path,
                        line_number,
                        "policy format version must be an unsigned integer",
                    )
                })?;
                if parsed != POLICY_FORMAT_VERSION {
                    return Err(policy_error(
                        &path,
                        line_number,
                        &format!(
                            "unsupported policy format version {parsed}; expected {POLICY_FORMAT_VERSION}"
                        ),
                    ));
                }
                version = Some(parsed);
                continue;
            }

            let fields = line.split('|').map(str::trim).collect::<Vec<_>>();
            if fields.len() != 4 {
                return Err(policy_error(
                    &path,
                    line_number,
                    "expected `command|0xNNNN|firmware-selector|reason` or `event|0xNNNN|firmware-selector|reason`",
                ));
            }
            let kind = CoverageKind::parse(fields[0]).ok_or_else(|| {
                policy_error(&path, line_number, "scope must be `command` or `event`")
            })?;
            let code = parse_wire_code(fields[1]).map_err(|error| {
                policy_error(&path, line_number, &format!("invalid wire code: {error}"))
            })?;
            let selector = FirmwareSelector::parse(fields[2]).map_err(|error| {
                policy_error(
                    &path,
                    line_number,
                    &format!("invalid firmware selector: {error}"),
                )
            })?;
            if fields[3].is_empty() {
                return Err(policy_error(
                    &path,
                    line_number,
                    "exclusion reason must not be empty",
                ));
            }
            if !raw_entries.insert((kind, code, selector)) {
                return Err(policy_error(
                    &path,
                    line_number,
                    "this scope, wire code, and firmware selector are declared more than once",
                ));
            }
            entries.push(PolicyEntry {
                kind,
                code,
                selector,
                reason: fields[3].to_owned(),
                line: line_number,
            });
        }

        if version.is_none() {
            return Err(format!(
                "exclusion policy {} has no `version = {POLICY_FORMAT_VERSION}` header",
                path.display()
            ));
        }
        Ok(Self { path, entries })
    }

    /// Validate selectors against the exact set of feature flags in the crate.
    /// This rejects stale version-specific exceptions and conflicting wildcard
    /// and exact entries before any CubeWB source is inspected.
    fn validate_for(&self, declared: &[FirmwareVersion]) -> Result<(), String> {
        let mut expanded = BTreeMap::<(CoverageKind, u16, FirmwareVersion), usize>::new();
        for entry in &self.entries {
            if let FirmwareSelector::Only(firmware) = entry.selector
                && !declared.contains(&firmware)
            {
                return Err(policy_error(
                    &self.path,
                    entry.line,
                    &format!(
                        "firmware selector {firmware} is not declared by this crate's [features] table"
                    ),
                ));
            }
            for firmware in declared
                .iter()
                .copied()
                .filter(|firmware| entry.selector.matches(*firmware))
            {
                let key = (entry.kind, entry.code, firmware);
                if let Some(previous_line) = expanded.insert(key, entry.line) {
                    return Err(policy_error(
                        &self.path,
                        entry.line,
                        &format!(
                            "overlaps line {previous_line}: {} 0x{:04X} would be excluded twice for firmware {firmware}",
                            entry.kind.as_str(),
                            entry.code
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn active_for(&self, firmware: FirmwareVersion) -> ActiveExclusions {
        let mut active = ActiveExclusions::default();
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.selector.matches(firmware))
        {
            match entry.kind {
                CoverageKind::Command => {
                    active.commands.insert(entry.code, entry.reason.clone());
                }
                CoverageKind::Event => {
                    active.events.insert(entry.code, entry.reason.clone());
                }
            }
        }
        active
    }

    fn display_path(&self, crate_dir: &Path) -> String {
        display_path(&self.path, crate_dir)
    }
}

fn policy_error(path: &Path, line: usize, message: &str) -> String {
    format!("{}:{line}: {message}", path.display())
}

fn parse_wire_code(value: &str) -> Result<u16, String> {
    let Some(value) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    else {
        return Err("wire codes must use hexadecimal `0xNNNN` notation".to_owned());
    };
    if value.is_empty() || value.len() > 4 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("wire codes must contain one to four hexadecimal digits".to_owned());
    }
    u16::from_str_radix(value, 16).map_err(|error| error.to_string())
}

#[derive(Clone, Debug, Default)]
struct ActiveExclusions {
    commands: BTreeMap<u16, String>,
    events: BTreeMap<u16, String>,
}

impl ActiveExclusions {
    fn audit(
        &self,
        report: &CheckReport,
        firmware: FirmwareVersion,
    ) -> Result<PolicyAudit, String> {
        let reported_commands = report
            .excluded_commands
            .iter()
            .map(|entry| (entry.code, entry.reason.clone()))
            .collect::<BTreeMap<_, _>>();
        let reported_events = report
            .excluded_events
            .iter()
            .map(|entry| (entry.code, entry.reason.clone()))
            .collect::<BTreeMap<_, _>>();
        if reported_commands != self.commands || reported_events != self.events {
            return Err(format!(
                "checker exclusions for firmware {firmware} do not match the active exclusion policy"
            ));
        }

        audit_exclusion_codes(
            CoverageKind::Command,
            &self.commands,
            &report.vendor.command_codes(),
            &report.active_api.command_codes(),
            firmware,
        )?;
        audit_exclusion_codes(
            CoverageKind::Event,
            &self.events,
            &report.vendor.event_codes(),
            &report.active_api.event_codes(),
            firmware,
        )?;
        Ok(PolicyAudit {
            command_entries: self.commands.len(),
            event_entries: self.events.len(),
        })
    }
}

fn audit_exclusion_codes(
    kind: CoverageKind,
    exclusions: &BTreeMap<u16, String>,
    expected: &BTreeSet<u16>,
    observed: &BTreeSet<u16>,
    firmware: FirmwareVersion,
) -> Result<(), String> {
    for code in exclusions.keys() {
        if expected.contains(code) == observed.contains(code) {
            return Err(format!(
                "exclusion policy for {} 0x{code:04X} on firmware {firmware} is stale: it no longer suppresses a coverage difference",
                kind.as_str(),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct PolicyAudit {
    command_entries: usize,
    event_entries: usize,
}

impl PolicyAudit {
    fn json(&self, policy: &ExclusionPolicy, crate_dir: &Path) -> PolicyAuditJson {
        PolicyAuditJson {
            path: policy.display_path(crate_dir),
            format_version: POLICY_FORMAT_VERSION,
            active_command_entries: self.command_entries,
            active_event_entries: self.event_entries,
            all_entries_suppress_differences: true,
        }
    }
}

#[derive(Serialize)]
struct PolicyAuditJson {
    path: String,
    format_version: u32,
    active_command_entries: usize,
    active_event_entries: usize,
    all_entries_suppress_differences: bool,
}

#[derive(Serialize)]
struct PolicyMetadataJson {
    path: String,
    format_version: u32,
    entries: usize,
}

impl PolicyMetadataJson {
    fn from_policy(policy: &ExclusionPolicy, crate_dir: &Path) -> Self {
        Self {
            path: policy.display_path(crate_dir),
            format_version: POLICY_FORMAT_VERSION,
            entries: policy.entries.len(),
        }
    }
}

fn display_path(path: &Path, crate_dir: &Path) -> String {
    path.strip_prefix(crate_dir).map_or_else(
        |_| path.display().to_string(),
        |path| path.display().to_string(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Subcommand)]
enum CliCommand {
    /// Check one firmware version or every firmware version declared by Cargo.
    Check,
    /// Compare the generated CubeWB protocol catalogs for two firmware versions.
    Diff,
    /// Print the crate's canonical `fw_<major>_<minor>_<patch>` feature names.
    #[command(name = "list-supported")]
    ListSupported,
}

/// Command-line interface for the compliance checker.
///
/// The checker historically accepted its flags on either side of the
/// subcommand. Marking them global retains that invocation style while
/// `validate` still rejects check-only flags with `list-supported`.
#[derive(Debug, Parser)]
#[command(
    name = "stm32wb-compliance",
    about = "Firmware API compliance checks for stm32wb-hci",
    disable_version_flag = true,
    after_help = "`check --all-supported` discovers every canonical `fw_<major>_<minor>_<patch>` feature from [features]. `diff --from <version> --to <version>` compares two generated CubeWB catalogs without building the crate. The checker reads CubeWB tag blobs with git show and never changes the Cube worktree."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,

    #[arg(
        short = 'f',
        long,
        global = true,
        value_name = "VERSION",
        help = "Firmware version (for example 0.15.0 or v1.15.0)"
    )]
    firmware: Option<FirmwareVersion>,

    #[arg(
        long,
        global = true,
        value_name = "VERSION",
        help = "Baseline firmware version for `diff` (for example 0.15.0 or v1.15.0)"
    )]
    from: Option<FirmwareVersion>,

    #[arg(
        long,
        global = true,
        value_name = "VERSION",
        help = "Comparison firmware version for `diff` (for example 0.17.1 or v1.17.1)"
    )]
    to: Option<FirmwareVersion>,

    #[arg(
        long,
        global = true,
        help = "Check every firmware feature declared in Cargo.toml"
    )]
    all_supported: bool,

    #[arg(
        long = "crate",
        global = true,
        value_name = "PATH",
        help = "stm32wb-hci checkout (defaults to the current/containing checkout)"
    )]
    crate_dir: Option<PathBuf>,

    #[arg(
        long = "cube",
        global = true,
        value_name = "PATH",
        help = "STM32CubeWB git checkout (defaults to <crate>/STM32CubeWB)"
    )]
    cube_dir: Option<PathBuf>,

    #[arg(
        long = "policy",
        global = true,
        value_name = "PATH",
        help = "Checked-in exclusion policy (defaults to tools/compliance/exclusions.policy)"
    )]
    policy_path: Option<PathBuf>,

    #[arg(long, global = true, help = "Emit a machine-readable report")]
    json: bool,

    #[arg(
        long,
        global = true,
        help = "Exit nonzero when the report has coverage differences"
    )]
    deny: bool,

    #[arg(
        long,
        global = true,
        help = "Skip cargo check of each selected firmware feature"
    )]
    skip_build: bool,
}

impl Cli {
    fn validate(&self) -> Result<(), String> {
        let command = self.command.ok_or_else(|| {
            "expected the `check`, `diff`, or `list-supported` command".to_owned()
        })?;
        match command {
            CliCommand::Check => {
                if self.firmware.is_some() == self.all_supported {
                    return Err(
                        "exactly one of --firmware <0.15.0|v1.15.0> or --all-supported is required"
                            .to_owned(),
                    );
                }
                if self.from.is_some() || self.to.is_some() {
                    return Err("check does not accept --from or --to; use `diff`".to_owned());
                }
            }
            CliCommand::Diff => {
                let (Some(from), Some(to)) = (self.from, self.to) else {
                    return Err("diff requires both --from <version> and --to <version>".to_owned());
                };
                if from == to {
                    return Err("diff requires two different firmware versions".to_owned());
                }
                if self.firmware.is_some()
                    || self.all_supported
                    || self.policy_path.is_some()
                    || self.skip_build
                {
                    return Err(
                        "diff accepts --from, --to, --cube, --crate, --json, and --deny only"
                            .to_owned(),
                    );
                }
            }
            CliCommand::ListSupported => {
                if self.firmware.is_some()
                    || self.from.is_some()
                    || self.to.is_some()
                    || self.all_supported
                    || self.cube_dir.is_some()
                    || self.policy_path.is_some()
                    || self.json
                    || self.deny
                    || self.skip_build
                {
                    return Err(
                        "list-supported only accepts --crate <path> (and --help)".to_owned()
                    );
                }
            }
        }
        Ok(())
    }
}

fn usage() -> String {
    Cli::command().render_long_help().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stm32wb_compliance::{ProtocolCoverage, StandardHciCoverage, WireReport};

    fn parse_cli(arguments: &[&str]) -> Result<Cli, String> {
        let cli = Cli::try_parse_from(
            std::iter::once("stm32wb-compliance").chain(arguments.iter().copied()),
        )
        .map_err(|error| error.to_string())?;
        cli.validate()?;
        Ok(cli)
    }

    fn supported() -> Vec<FirmwareVersion> {
        vec![
            FirmwareVersion::new(0, 15, 0),
            FirmwareVersion::new(0, 16, 0),
        ]
    }

    #[test]
    fn parses_single_firmware_check_arguments_in_any_order() {
        let cli = parse_cli(&["--deny", "check", "--firmware", "v1.15.0", "--skip-build"]).unwrap();

        assert_eq!(cli.command, Some(CliCommand::Check));
        assert_eq!(cli.firmware, Some(FirmwareVersion::new(0, 15, 0)));
        assert!(cli.deny);
        assert!(cli.skip_build);
    }

    #[test]
    fn parses_all_supported_check() {
        let cli = parse_cli(&["check", "--all-supported"]).unwrap();
        assert_eq!(cli.command, Some(CliCommand::Check));
        assert!(cli.all_supported);
    }

    #[test]
    fn parses_version_diff_arguments_and_rejects_check_only_flags() {
        let cli = parse_cli(&[
            "diff", "--from", "0.15.0", "--to", "v1.17.1", "--json", "--deny",
        ])
        .unwrap();
        assert_eq!(cli.command, Some(CliCommand::Diff));
        assert_eq!(cli.from, Some(FirmwareVersion::new(0, 15, 0)));
        assert_eq!(cli.to, Some(FirmwareVersion::new(0, 17, 1)));
        assert!(cli.json);
        assert!(cli.deny);

        let error =
            parse_cli(&["diff", "--from", "0.15.0", "--to", "0.17.1", "--skip-build"]).unwrap_err();
        assert!(error.contains("diff accepts"));
    }

    #[test]
    fn rejects_ambiguous_firmware_selection() {
        let error = parse_cli(&["check", "--all-supported", "--firmware", "0.15.0"]).unwrap_err();
        assert!(error.contains("exactly one"));
    }

    #[test]
    fn list_supported_accepts_a_crate_override_only() {
        let cli = parse_cli(&["list-supported", "--crate", "/tmp/crate"]).unwrap();
        assert_eq!(cli.command, Some(CliCommand::ListSupported));

        let error = parse_cli(&["list-supported", "--json"]).unwrap_err();
        assert!(error.contains("only accepts"));
    }

    #[test]
    fn clap_rejects_unknown_arguments_before_running_checks() {
        let error = Cli::try_parse_from(["stm32wb-compliance", "check", "--unknown"])
            .unwrap_err()
            .to_string();
        assert!(error.contains("unexpected argument"));
    }

    #[test]
    fn policy_expands_version_selectors_and_rejects_overlaps() {
        let policy = ExclusionPolicy::parse(
            "test.policy".into(),
            "version = 1\nevent|0x9200|*|transport event\ncommand|0x0001|0.15.0|legacy command\n",
        )
        .unwrap();
        policy.validate_for(&supported()).unwrap();
        let old = policy.active_for(FirmwareVersion::new(0, 15, 0));
        assert_eq!(old.events.get(&0x9200), Some(&"transport event".to_owned()));
        assert_eq!(
            old.commands.get(&0x0001),
            Some(&"legacy command".to_owned())
        );
        let new = policy.active_for(FirmwareVersion::new(0, 16, 0));
        assert!(!new.commands.contains_key(&0x0001));

        let overlapping = ExclusionPolicy::parse(
            "test.policy".into(),
            "version = 1\nevent|0x9200|*|transport event\nevent|0x9200|0.15.0|same event\n",
        )
        .unwrap();
        assert!(overlapping.validate_for(&supported()).is_err());
    }

    #[test]
    fn policy_rejects_unknown_versions_and_bad_codes() {
        let policy = ExclusionPolicy::parse(
            "test.policy".into(),
            "version = 1\nevent|0x9200|0.99.0|future event\n",
        )
        .unwrap();
        assert!(policy.validate_for(&supported()).is_err());

        let error = ExclusionPolicy::parse(
            "test.policy".into(),
            "version = 1\nevent|9200|*|missing hex prefix\n",
        )
        .unwrap_err();
        assert!(error.contains("wire codes"));
    }

    #[test]
    fn policy_requires_a_version_header() {
        let error =
            ExclusionPolicy::parse("test.policy".into(), "event|0x9200|*|transport event\n")
                .unwrap_err();
        assert!(error.contains("no `version = 1` header"));
    }

    #[test]
    fn wire_codes_are_strict_16_bit_hexadecimal() {
        assert_eq!(parse_wire_code("0x0"), Ok(0));
        assert_eq!(parse_wire_code("0xFFFF"), Ok(u16::MAX));
        assert!(parse_wire_code("0x10000").is_err());
        assert!(parse_wire_code("0xGG").is_err());
    }

    #[test]
    fn batch_json_is_serialized_as_structured_data() {
        let firmware = FirmwareVersion::new(0, 15, 0);
        let report = CheckReport {
            firmware,
            cube_tag: "v1.15.0".to_owned(),
            vendor: ProtocolCoverage::default(),
            descriptors: ProtocolCoverage::default(),
            active_api: ProtocolCoverage::default(),
            standard_hci: StandardHciCoverage::default(),
            standard_hci_provider: StandardHciCoverage::default(),
            missing_commands: Vec::new(),
            extraneous_commands: Vec::new(),
            missing_events: Vec::new(),
            extraneous_events: Vec::new(),
            missing_standard_hci_commands: Vec::new(),
            missing_standard_hci_events: Vec::new(),
            missing_standard_hci_le_meta_events: Vec::new(),
            wire: WireReport::default(),
            excluded_commands: Vec::new(),
            excluded_events: Vec::new(),
        };
        let policy = ExclusionPolicy {
            path: PathBuf::from("policy"),
            entries: Vec::new(),
        };
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
        assert_eq!(
            value["results"][0]["report"]["cube_provenance"]["commit"],
            "commit"
        );
    }
}
