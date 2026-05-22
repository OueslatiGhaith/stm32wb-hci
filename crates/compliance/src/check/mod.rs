//! Compliance checking for ST firmware commands implemented by the Rust crate.
//!
//! This module compares three sources of truth:
//! ST command metadata extracted from STM32CubeWB, Rust opcode constants, and
//! `// compliance:` markers attached to Rust command methods.

mod cfg;
mod coverage;
mod diagnostics;
mod index;
mod marker;
mod naming;
mod report;
mod rules;
mod rust_event;
mod rust_marker;
mod rust_method;
mod rust_opcode;
mod rust_source;

pub use coverage::check_coverage;

use serde::Serialize;

/// Source location for a compliance marker or Rust command method.
#[derive(Clone, Debug, Serialize)]
pub struct MarkerLocation {
    /// Path to the Rust source file that contains the item.
    pub file: String,
    /// One-based line number within `file`.
    pub line: usize,
}

/// Vendor command modules currently covered by the compliance checker.
const COMMAND_GROUPS: [&str; 4] = ["gap", "gatt", "hal", "l2cap"];
