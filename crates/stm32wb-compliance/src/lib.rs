//! Firmware compliance support for `stm32wb-hci`.
//!
//! This host-only crate can run in CI against a local STM32CubeWB checkout;
//! `syn` is used to make the selected Rust API inventory syntax-aware.
//!
//! The normalized catalog and machine-readable check report are internal
//! representations validated at their construction boundaries.

mod c_preprocessor;
mod catalog;
mod diff;
mod json;
mod model;
mod rust_cfg;
mod rust_source;
mod standard;
mod vendor;
mod wire;

pub use catalog::{
    CatalogCommand, CatalogCommandKind, CatalogCompletion, CatalogEvent, CatalogEventKind,
    CatalogFamily, CatalogSchema, CommandScope, Envelope, EventScope, Evidence, WireLayout,
    WireLayoutEvidence, WireSegment,
};
pub use diff::{
    CatalogIdentity, ChangedCommand, ChangedEvent, CommandChanges, CommandKey, EventChanges,
    EventKey, VersionDiff, VersionDiffError, diff_catalogs,
};
pub use json::CheckReportJson;
pub use model::{
    CheckReport, CoverageDifference, CoverageEntry, CoverageOrigin, ProtocolCoverage,
    StandardHciCoverage,
};
pub use stm32wb_hci_schema::FirmwareVersion;
pub use wire::{WireDifference, WireReport, WireUnavailable};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub firmware: FirmwareVersion,
    pub cube_dir: PathBuf,
    pub crate_dir: PathBuf,
    pub skip_build: bool,
    pub excluded_commands: BTreeMap<u16, String>,
    pub excluded_events: BTreeMap<u16, String>,
    /// Payload layouts supplied by the checked-in policy for transport-only
    /// events which do not exist in CubeWB's generated event table.
    pub external_event_payloads: BTreeMap<u16, WireLayoutEvidence>,
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
            external_event_payloads: BTreeMap::new(),
        }
    }
}

/// Run the selected-feature build check and compare crate coverage with the
/// generated STM32CubeWB API at the matching tag.
pub fn check(options: &CheckOptions) -> Result<CheckReport, ComplianceError> {
    if !options.skip_build {
        cargo_check(&options.crate_dir, &options.firmware.feature_name())?;
    }

    let catalog = load_catalog(&options.cube_dir, options.firmware)?;
    let rust_catalog = rust_source::load_rust_catalog(&options.crate_dir, options.firmware)
        .map_err(ComplianceError::Source)?;
    let local_standard_hci_declarations =
        standard::load_local_standard_commands(&options.crate_dir, options.firmware)
            .map_err(ComplianceError::Source)?;
    let local_standard_hci_commands = standard::coverage_entries(&local_standard_hci_declarations);

    let wire = wire::compare_wire_with_external_events(
        &catalog.commands,
        &catalog.events,
        &rust_catalog,
        &local_standard_hci_declarations,
        &options.external_event_payloads,
    );
    let vendor = catalog.vendor_coverage();
    let standard_hci = catalog.standard_hci_coverage();
    let active_api = rust_catalog.coverage();

    Ok(CheckReport::new(
        options.firmware,
        vendor,
        active_api,
        standard_hci,
        local_standard_hci_commands,
        wire,
        options.excluded_commands.clone(),
        options.excluded_events.clone(),
    ))
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
        .find_map(|path| {
            if is_hci_crate(path) {
                return Some(path.to_path_buf());
            }

            let workspace_member = path.join("crates/stm32wb-hci");
            is_hci_crate(&workspace_member).then_some(workspace_member)
        })
        .or_else(|| {
            let bundled_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci");
            bundled_crate
                .canonicalize()
                .ok()
                .filter(|path| is_hci_crate(path))
        })
}

fn is_hci_crate(path: &Path) -> bool {
    path.join("src/vendor/command").is_dir() && path.join("Cargo.toml").is_file()
}

/// Locate the workspace containing the library crate.
///
/// A standalone checkout of the package may have its lockfile beside the
/// package manifest, while this repository keeps it at the virtual-workspace
/// root. Choosing the nearest lockfile supports both layouts.
pub fn workspace_root(crate_dir: &Path) -> PathBuf {
    crate_dir
        .ancestors()
        .find(|path| path.join("Cargo.lock").is_file())
        .unwrap_or(crate_dir)
        .to_path_buf()
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
        assert!(options.external_event_payloads.is_empty());
    }
}
