//! Parser for Rust vendor opcode constants.
//!
//! The checked crate defines vendor opcodes as command IDs grouped by command
//! group ID. This module reconstructs the final Bluetooth vendor opcode value
//! so it can be matched against the opcode extracted from ST C sources.

use anyhow::{Context, Result};
use std::path::Path;

/// Rust opcode constant with its fully reconstructed numeric opcode.
#[derive(Debug)]
pub(super) struct RustOpcode {
    pub(super) name: String,
    pub(super) opcode: u16,
}

/// Parses `src/vendor/opcode.rs` and returns all vendor opcode constants.
pub(super) fn parse_rust_opcodes(path: &Path) -> Result<Vec<RustOpcode>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut current_cgid = None;
    let mut opcodes = Vec::new();

    for line in source.lines() {
        let line = line.split("//").next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }

        if let Some((_, cgid)) = parse_group_cgid(line) {
            current_cgid = Some(cgid);
            continue;
        }

        let Some(cgid) = current_cgid else {
            continue;
        };
        if let Some((name, cid)) = parse_opcode_const(line) {
            let ocf = ((cgid & 0b111) << 7) | (cid & 0b111_1111);
            let opcode = (0x3f << 10) | ocf;
            opcodes.push(RustOpcode { name, opcode });
        }
    }

    Ok(opcodes)
}

/// Parses a command group ID line such as `Gap = 0x1;`.
fn parse_group_cgid(line: &str) -> Option<(String, u16)> {
    if line.starts_with("pub const") {
        return None;
    }
    let (name, value) = line.strip_suffix(';')?.split_once('=')?;
    Some((name.trim().to_owned(), parse_int(value.trim())?))
}

/// Parses a command ID constant line inside the current group.
fn parse_opcode_const(line: &str) -> Option<(String, u16)> {
    let rest = line.strip_prefix("pub const ")?;
    let (name, value) = rest.strip_suffix(';')?.split_once('=')?;
    Some((name.trim().to_owned(), parse_int(value.trim())?))
}

/// Parses a decimal or lowercase-hex integer literal.
fn parse_int(value: &str) -> Option<u16> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix("0x") {
        u16::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}
