//! Firmware compliance support for `stm32wb-hci`.
//!
//! This host-only crate can run in CI against a local STM32CubeWB checkout;
//! `syn` is used to make the selected Rust API inventory syntax-aware.

mod c_preprocessor;
mod catalog;
mod diff;
mod envelope;
mod firmware;
mod json;
mod model;
mod rust_source;
mod standard;
mod vendor;
mod wire;

pub use catalog::{
    CATALOG_SCHEMA_VERSION, CatalogCommand, CatalogEvent, CatalogFamily, CatalogSchema,
    CommandScope, CompletionExpectation, EventPayloadLayout, EventScope, RequestLayout,
    ResponseLayout,
};
pub use diff::{
    CatalogIdentity, ChangedCommand, ChangedEvent, CommandChanges, CommandKey, EventChanges,
    EventKey, VersionDiff, VersionDiffError, diff_catalogs,
};
pub use firmware::FirmwareVersion;
pub use json::CheckReportJson;
pub use model::{
    CheckReport, CoverageDifference, CoverageEntry, CoverageOrigin, ProtocolCoverage,
    StandardHciCoverage,
};
pub use wire::{WireDifference, WireReport, WireUnavailable};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use model::{compare_coverage, with_standard_hci_coverage, with_wire_report};

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub firmware: FirmwareVersion,
    pub cube_dir: PathBuf,
    pub crate_dir: PathBuf,
    pub skip_build: bool,
    pub excluded_commands: BTreeMap<u16, String>,
    pub excluded_events: BTreeMap<u16, String>,
}

impl CheckOptions {
    pub fn new(firmware: FirmwareVersion, crate_dir: PathBuf, cube_dir: PathBuf) -> Self {
        Self {
            firmware,
            cube_dir,
            crate_dir,
            skip_build: false,
            excluded_commands: BTreeMap::new(),
            excluded_events: BTreeMap::new(),
        }
    }
}

/// Run the selected-feature build check and compare crate coverage with the
/// generated STM32CubeWB API at the matching tag.
pub fn check(options: &CheckOptions) -> Result<CheckReport, ComplianceError> {
    if !options.skip_build {
        cargo_check(&options.crate_dir, &options.firmware.feature_name())?;
    }

    let tag = options.firmware.cube_tag();
    let catalog = load_catalog(&options.cube_dir, options.firmware)?;
    let crate_coverage = rust_source::load_crate_coverage(&options.crate_dir, options.firmware)
        .map_err(ComplianceError::Source)?;
    let standard_provider =
        standard::load_standard_provider_coverage(&options.crate_dir, options.firmware)
            .map_err(ComplianceError::Source)?;

    let wire = wire::compare_vendor_wire(&catalog.commands, &catalog.events, &crate_coverage);
    let vendor = catalog.vendor_coverage();
    let standard_hci = catalog.standard_hci_coverage();

    let report = compare_coverage(
        options.firmware,
        tag,
        vendor,
        crate_coverage.descriptors,
        crate_coverage.active_api,
        options.excluded_commands.clone(),
        options.excluded_events.clone(),
    );
    let report = with_standard_hci_coverage(
        report,
        standard_hci,
        StandardHciCoverage {
            commands: standard_provider.commands,
            events: standard_provider.events,
            le_meta_events: standard_provider.le_meta_events,
        },
    );
    Ok(with_wire_report(report, wire))
}

/// Load the normalized generated protocol catalog for one firmware version.
/// This does not build or inspect the Rust crate, so it can be used by release
/// comparison tooling independently of feature compliance checks.
pub fn load_catalog(
    cube_dir: &Path,
    firmware: FirmwareVersion,
) -> Result<CatalogSchema, ComplianceError> {
    vendor::load_vendor_catalog(cube_dir, &firmware.cube_tag()).map_err(ComplianceError::Source)
}

/// Locate the crate root from the current directory or one of its parents.
pub fn find_crate_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|path| path.join("src/vendor/command").is_dir() && path.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
        .or_else(|| {
            let bundled_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
            bundled_root
                .canonicalize()
                .ok()
                .filter(|path| path.join("src/vendor/command").is_dir())
        })
}

fn cargo_check(crate_dir: &Path, feature: &str) -> Result<(), ComplianceError> {
    let status = Command::new("cargo")
        .args([
            "check",
            "--package",
            "stm32wb-hci",
            "--no-default-features",
            "--features",
            feature,
        ])
        .current_dir(crate_dir)
        .status()
        .map_err(|error| ComplianceError::BuildLaunch(error.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => Err(ComplianceError::BuildFailed(code)),
            None => Err(ComplianceError::BuildTerminated),
        }
    }
}

#[derive(Debug, Error)]
pub enum ComplianceError {
    #[error("could not launch cargo check: {0}")]
    BuildLaunch(String),
    #[error("cargo check failed with exit code {0}")]
    BuildFailed(i32),
    #[error("cargo check was terminated by a signal")]
    BuildTerminated,
    #[error("{0}")]
    Source(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_options_do_not_hide_transport_events_without_a_policy() {
        let options = CheckOptions::new(
            FirmwareVersion::new(0, 17, 1),
            PathBuf::from("crate"),
            PathBuf::from("cube"),
        );
        assert!(options.excluded_commands.is_empty());
        assert!(options.excluded_events.is_empty());
    }
}
