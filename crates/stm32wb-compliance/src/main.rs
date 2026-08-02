use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod output;
mod policy;

use output::{
    batch_to_human, batch_to_json, checked_run_to_human, checked_run_to_json, diff_to_human,
    version_diff_to_json,
};
use policy::{ExclusionPolicy, PolicyAudit};

use clap::{ArgGroup, Args, CommandFactory, Parser, Subcommand};
use stm32wb_compliance::{
    CheckOptions, CheckReport, ComplianceTarget, FirmwareVersion, McuFamily, StackProfile, check,
    diff_catalogs, find_crate_root, load_catalog, workspace_root,
};

const DEFAULT_POLICY_PATH: &str = "crates/stm32wb-compliance/exclusions.toml";

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
    let current_dir =
        env::current_dir().map_err(|error| format!("could not read current directory: {error}"))?;
    match &cli.command {
        CliCommand::ListSupported(args) => {
            list_supported(&crate_dir(&args.crate_dir, &current_dir)?)
        }
        CliCommand::Check(args) => run_check(args, crate_dir(&args.crate_dir, &current_dir)?),
        CliCommand::Diff(args) => run_diff(args, &current_dir),
    }
}

fn crate_dir(crate_override: &Option<PathBuf>, current_dir: &Path) -> Result<PathBuf, String> {
    crate_override
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

fn run_check(args: &CheckArgs, crate_dir: PathBuf) -> Result<ExitCode, String> {
    let declared_firmwares =
        FirmwareVersion::declared_in_manifest(&crate_dir).map_err(|error| error.to_string())?;
    let firmwares = if args.all_supported {
        declared_firmwares.clone()
    } else {
        let firmware = args
            .release
            .expect("the parser requires --release or --all-supported");
        if !declared_firmwares.contains(&firmware) {
            return Err(format!(
                "Cube release {firmware} is not declared by {}; add `{}` to [features] or use `list-supported`",
                crate_dir.join("Cargo.toml").display(),
                firmware.feature_name()
            ));
        }
        vec![firmware]
    };

    let workspace_dir = workspace_root(&crate_dir);
    let cube_dir = args
        .cube_dir
        .clone()
        .unwrap_or_else(|| workspace_dir.join("STM32CubeWB"));
    let policy_path = args
        .policy_path
        .clone()
        .unwrap_or_else(|| workspace_dir.join(DEFAULT_POLICY_PATH));
    let policy = ExclusionPolicy::load(policy_path)?;
    policy.validate_for(&declared_firmwares)?;

    let mut results = Vec::with_capacity(firmwares.len());
    for firmware in firmwares {
        let target = ComplianceTarget::new(firmware, args.family, args.profile);
        match run_one_check(target, &crate_dir, &cube_dir, &policy, args.skip_build) {
            Ok(result) => results.push(BatchResult::Success(Box::new(result))),
            Err(error) => results.push(BatchResult::Error { target, error }),
        }
    }

    if args.all_supported {
        if args.json {
            println!("{}", batch_to_json(&results, &policy, &crate_dir));
        } else {
            print!("{}", batch_to_human(&results, &policy, &crate_dir));
        }
    } else {
        let result = results
            .into_iter()
            .next()
            .expect("at least one requested Cube release");
        match result {
            BatchResult::Success(result) => {
                if args.json {
                    println!("{}", checked_run_to_json(&result, &policy, &crate_dir));
                } else {
                    print!("{}", checked_run_to_human(&result, &policy, &crate_dir));
                }
                return Ok(report_exit_code(args.deny, &result.report));
            }
            BatchResult::Error { error, .. } => return Err(error),
        }
    }

    let has_errors = results.iter().any(BatchResult::is_error);
    let has_noncompliant = results.iter().any(BatchResult::is_noncompliant);
    Ok(if has_errors {
        ExitCode::from(2)
    } else if args.deny && has_noncompliant {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn report_exit_code(deny: bool, report: &CheckReport) -> ExitCode {
    if deny && !report.is_compliant() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_diff(args: &DiffArgs, current_dir: &Path) -> Result<ExitCode, String> {
    let from = args.from;
    let to = args.to;
    if from == to {
        return Err("diff requires two different Cube releases".to_owned());
    }
    let crate_dir = args
        .crate_dir
        .clone()
        .or_else(|| find_crate_root(current_dir));
    let workspace_dir = crate_dir.as_deref().map(workspace_root);
    let cube_dir = args
        .cube_dir
        .clone()
        .or_else(|| workspace_dir.as_ref().map(|path| path.join("STM32CubeWB")))
        .ok_or_else(|| {
            "could not locate the stm32wb-hci crate for the default CubeWB path; pass --cube <path>"
                .to_owned()
        })?;
    let display_root = crate_dir.as_deref().unwrap_or(current_dir);

    let from_target = ComplianceTarget::new(from, args.family, args.profile);
    let to_target = ComplianceTarget::new(to, args.family, args.profile);
    let from_provenance = CubeProvenance::resolve(&cube_dir, from_target)?;
    let to_provenance = CubeProvenance::resolve(&cube_dir, to_target)?;
    let from_catalog = load_catalog(&cube_dir, from_target).map_err(|error| error.to_string())?;
    let to_catalog = load_catalog(&cube_dir, to_target).map_err(|error| error.to_string())?;
    let diff = diff_catalogs(&from_catalog, &to_catalog).map_err(|error| error.to_string())?;

    if args.json {
        println!(
            "{}",
            version_diff_to_json(
                &diff,
                from,
                to,
                &from_provenance,
                &to_provenance,
                display_root,
            )
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

    Ok(if args.deny && diff.has_changes() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn run_one_check(
    target: ComplianceTarget,
    crate_dir: &Path,
    cube_dir: &Path,
    policy: &ExclusionPolicy,
    skip_build: bool,
) -> Result<CheckedRun, String> {
    let firmware = target.release;
    let provenance = CubeProvenance::resolve(cube_dir, target)?;
    let active_exclusions = policy.active_for(target);

    let mut options = CheckOptions::new(target, crate_dir.to_path_buf(), cube_dir.to_path_buf());
    options
        .excluded_commands
        .extend(active_exclusions.commands.clone());
    options
        .excluded_events
        .extend(active_exclusions.events.clone());
    options.skip_build = skip_build;

    let report = check(&options).map_err(|error| error.to_string())?;
    let policy_audit = active_exclusions.audit(&report, target)?;

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

#[derive(Debug)]
enum BatchResult {
    Success(Box<CheckedRun>),
    Error {
        target: ComplianceTarget,
        error: String,
    },
}

impl BatchResult {
    fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    fn is_noncompliant(&self) -> bool {
        matches!(self, Self::Success(result) if !result.report.is_compliant())
    }
}

#[derive(Clone, Debug)]
struct CubeProvenance {
    cube_dir: PathBuf,
    tag: String,
    tag_object: String,
    commit: String,
    binary_path: PathBuf,
    binary_blob: String,
}

impl CubeProvenance {
    fn resolve(cube_dir: &Path, target: ComplianceTarget) -> Result<Self, String> {
        let tag = target.release.cube_tag();
        let tag_ref = format!("refs/tags/{tag}");
        let tag_object = git_rev_parse(cube_dir, &tag_ref)?;
        let commit = git_rev_parse(cube_dir, &format!("{tag_ref}^{{commit}}"))?;
        let binary_path = target.binary_path();
        let binary_blob = git_rev_parse(cube_dir, &format!("{tag}:{}", binary_path.display()))?;
        Ok(Self {
            cube_dir: cube_dir
                .canonicalize()
                .unwrap_or_else(|_| cube_dir.to_path_buf()),
            tag,
            tag_object,
            commit,
            binary_path,
            binary_blob,
        })
    }

    fn display_path(&self, crate_dir: &Path) -> String {
        display_path(&self.cube_dir, crate_dir)
    }
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

fn display_path(path: &Path, crate_dir: &Path) -> String {
    path.strip_prefix(crate_dir).map_or_else(
        |_| path.display().to_string(),
        |path| path.display().to_string(),
    )
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Check one Cube release or every release declared by Cargo.
    Check(CheckArgs),
    /// Compare target-filtered CubeWB protocol catalogs for two releases.
    Diff(DiffArgs),
    /// Print the crate's canonical `fw_<major>_<minor>_<patch>` release features.
    #[command(name = "list-supported")]
    ListSupported(ListSupportedArgs),
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("firmware_selection")
        .required(true)
        .multiple(false)
        .args(["release", "all_supported"])
))]
struct CheckArgs {
    #[arg(
        short = 'f',
        long = "release",
        visible_alias = "firmware",
        value_name = "VERSION",
        help = "STM32CubeWB release (for example 1.15.0 or v1.15.0)"
    )]
    release: Option<FirmwareVersion>,

    #[arg(long, help = "Check every release feature declared in Cargo.toml")]
    all_supported: bool,

    #[arg(long, default_value = "wb5x", help = "Target MCU family")]
    family: McuFamily,

    #[arg(
        long,
        default_value = "full-extended",
        help = "Target wireless BLE binary profile"
    )]
    profile: StackProfile,

    #[arg(
        long = "crate",
        value_name = "PATH",
        help = "stm32wb-hci package directory (defaults to the current/containing workspace member)"
    )]
    crate_dir: Option<PathBuf>,

    #[arg(
        long = "cube",
        value_name = "PATH",
        help = "STM32CubeWB git checkout (defaults to <workspace>/STM32CubeWB)"
    )]
    cube_dir: Option<PathBuf>,

    #[arg(
        long = "policy",
        value_name = "PATH",
        help = "Checked-in exclusion policy (defaults to crates/stm32wb-compliance/exclusions.toml)"
    )]
    policy_path: Option<PathBuf>,

    #[arg(long, help = "Emit a machine-readable report")]
    json: bool,

    #[arg(long, help = "Exit nonzero when a check is non-compliant")]
    deny: bool,

    #[arg(long, help = "Skip cargo check of each selected release feature")]
    skip_build: bool,
}

#[derive(Debug, Args)]
struct DiffArgs {
    #[arg(
        long,
        value_name = "VERSION",
        help = "Baseline STM32CubeWB release (for example 1.15.0 or v1.15.0)"
    )]
    from: FirmwareVersion,

    #[arg(
        long,
        value_name = "VERSION",
        help = "Comparison STM32CubeWB release (for example 1.17.1 or v1.17.1)"
    )]
    to: FirmwareVersion,

    #[arg(long, default_value = "wb5x", help = "Target MCU family")]
    family: McuFamily,

    #[arg(
        long,
        default_value = "full-extended",
        help = "Target wireless BLE binary profile"
    )]
    profile: StackProfile,

    #[arg(
        long = "crate",
        value_name = "PATH",
        help = "stm32wb-hci package directory (used to locate the default CubeWB checkout)"
    )]
    crate_dir: Option<PathBuf>,

    #[arg(
        long = "cube",
        value_name = "PATH",
        help = "STM32CubeWB git checkout (defaults to <workspace>/STM32CubeWB)"
    )]
    cube_dir: Option<PathBuf>,

    #[arg(long, help = "Emit a machine-readable report")]
    json: bool,

    #[arg(long, help = "Exit nonzero when the version diff has changes")]
    deny: bool,
}

#[derive(Debug, Args)]
struct ListSupportedArgs {
    #[arg(
        long = "crate",
        value_name = "PATH",
        help = "stm32wb-hci package directory (defaults to the current/containing workspace member)"
    )]
    crate_dir: Option<PathBuf>,
}

/// Command-line interface for the compliance checker.
#[derive(Debug, Parser)]
#[command(
    name = "stm32wb-compliance",
    about = "Wireless-binary API compliance checks for stm32wb-hci",
    disable_version_flag = true,
    after_help = "`check --all-supported` discovers every canonical `fw_<major>_<minor>_<patch>` release feature from [features]. `--family` and `--profile` select an exact wireless CPU2 binary. `diff --from <version> --to <version>` compares two target-filtered CubeWB catalogs without building the crate. The checker reads CubeWB tag blobs with git show and never changes the Cube worktree."
)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

fn usage() -> String {
    Cli::command().render_long_help().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stm32wb_compliance::{ProtocolCoverage, StandardHciCoverage, WireReport, WireUnavailable};

    fn parse_cli(arguments: &[&str]) -> Result<Cli, String> {
        Cli::try_parse_from(std::iter::once("stm32wb-compliance").chain(arguments.iter().copied()))
            .map_err(|error| error.to_string())
    }

    #[test]
    fn parses_typed_check_arguments_after_the_subcommand() {
        let cli = parse_cli(&["check", "--firmware", "v1.15.0", "--skip-build", "--deny"]).unwrap();
        let CliCommand::Check(args) = cli.command else {
            panic!("expected check arguments");
        };
        assert_eq!(args.release, Some(FirmwareVersion::new(1, 15, 0)));
        assert_eq!(args.family, McuFamily::Wb5x);
        assert_eq!(args.profile, StackProfile::FullExtended);
        assert!(args.deny);
        assert!(args.skip_build);

        let error = parse_cli(&["--deny", "check", "--firmware", "v1.15.0"]).unwrap_err();
        assert!(error.contains("unexpected argument '--deny'"));
    }

    #[test]
    fn parses_all_supported_check() {
        let cli = parse_cli(&["check", "--all-supported"]).unwrap();
        let CliCommand::Check(args) = cli.command else {
            panic!("expected check arguments");
        };
        assert!(args.all_supported);
    }

    #[test]
    fn parses_version_diff_arguments_and_rejects_check_only_flags() {
        let cli = parse_cli(&[
            "diff", "--from", "1.15.0", "--to", "v1.17.1", "--json", "--deny",
        ])
        .unwrap();
        let CliCommand::Diff(args) = cli.command else {
            panic!("expected diff arguments");
        };
        assert_eq!(args.from, FirmwareVersion::new(1, 15, 0));
        assert_eq!(args.to, FirmwareVersion::new(1, 17, 1));
        assert!(args.json);
        assert!(args.deny);

        let error =
            parse_cli(&["diff", "--from", "1.15.0", "--to", "1.17.1", "--skip-build"]).unwrap_err();
        assert!(error.contains("unexpected argument '--skip-build'"));
    }

    #[test]
    fn rejects_ambiguous_firmware_selection() {
        let error = parse_cli(&["check", "--all-supported", "--firmware", "1.15.0"]).unwrap_err();
        assert!(error.contains("cannot be used with"));
    }

    #[test]
    fn list_supported_accepts_a_crate_override_only() {
        let cli = parse_cli(&["list-supported", "--crate", "/tmp/crate"]).unwrap();
        let CliCommand::ListSupported(args) = cli.command else {
            panic!("expected list-supported arguments");
        };
        assert_eq!(args.crate_dir, Some(PathBuf::from("/tmp/crate")));

        let error = parse_cli(&["list-supported", "--json"]).unwrap_err();
        assert!(error.contains("unexpected argument '--json'"));
    }

    #[test]
    fn clap_rejects_unknown_arguments_before_running_checks() {
        let error = Cli::try_parse_from(["stm32wb-compliance", "check", "--unknown"])
            .unwrap_err()
            .to_string();
        assert!(error.contains("unexpected argument"));
    }

    #[test]
    fn deny_fails_when_wire_evidence_is_unavailable() {
        let firmware = FirmwareVersion::new(1, 17, 1);
        let report = CheckReport::new(
            ComplianceTarget::new(firmware, McuFamily::Wb5x, StackProfile::FullExtended),
            ProtocolCoverage::default(),
            ProtocolCoverage::default(),
            StandardHciCoverage::default(),
            Vec::new(),
            WireReport {
                checked: 0,
                differences: Vec::new(),
                unavailable: vec![WireUnavailable {
                    code: 0x9200,
                    command: "CoprocessorReady".into(),
                    reason: "missing payload evidence".into(),
                }],
            },
            Default::default(),
            Default::default(),
        );

        assert_eq!(report_exit_code(false, &report), ExitCode::SUCCESS);
        assert_eq!(report_exit_code(true, &report), ExitCode::FAILURE);
    }
}
