//! Coverage checking entry point.
//!
//! This module coordinates the high-level coverage check. Firmware, Rust
//! commands, Rust events, naming, and rule evaluation live in focused sibling
//! modules so the public entry point stays small.

use super::cfg::FirmwareCfg;
use super::index::{FirmwareIndex, RustCommandIndex, RustEventIndex};
use super::report::CoverageReport;
use super::rules::CoverageRules;
use crate::spec::FirmwareSpec;
use anyhow::Result;
use std::path::Path;

/// Builds a compliance report for `rust_crate` against an extracted ST spec.
pub fn check_coverage(spec: &FirmwareSpec, rust_crate: &Path) -> Result<CoverageReport> {
    let firmware = FirmwareIndex::new(spec);
    let firmware_cfg = FirmwareCfg::parse(&spec.firmware);
    let rust_commands = RustCommandIndex::load(rust_crate, firmware_cfg.as_ref())?;
    let rust_events = RustEventIndex::load(rust_crate, firmware_cfg.as_ref())?;
    let diagnostics =
        CoverageRules::new(rust_crate, &firmware, &rust_commands, &rust_events).check();

    Ok(CoverageReport::from_diagnostics(diagnostics))
}
