//! Naming and placement helpers for compliance comparisons.
//!
//! This module owns the mapping between generated ST names and the formal names
//! used in Rust compliance markers and diagnostics.

use crate::spec::CommandSpec;
use std::path::{Path, PathBuf};

/// Returns the ST command name used in marker comments.
///
/// ST headers usually provide the formal `ACI_*` name in `@brief`; when that is
/// unavailable, the generated C function name is uppercased as a fallback.
pub(super) fn formal_st_name(command: &CommandSpec) -> String {
    command
        .doc
        .as_ref()
        .and_then(|doc| doc.brief.as_deref())
        .filter(|brief| brief.starts_with("ACI_"))
        .map(str::to_owned)
        .unwrap_or_else(|| command.name.to_ascii_uppercase())
}

/// Converts a generated C event function name to ST's formal `ACI_*` name.
pub(super) fn formal_event_name(event_name: &str) -> String {
    event_name.to_ascii_uppercase()
}

/// Suggests Rust files where a missing command marker would likely belong.
pub(super) fn expected_places(rust_crate: &Path, command: &CommandSpec) -> Vec<String> {
    let mut paths = vec![
        rust_crate
            .join("src/vendor/opcode.rs")
            .display()
            .to_string(),
    ];
    if let Some(command_file) = command_file(rust_crate, command.group.as_str()) {
        paths.push(command_file.display().to_string());
    }
    paths
}

/// Maps an ST command group to the corresponding Rust vendor command file.
fn command_file(rust_crate: &Path, group: &str) -> Option<PathBuf> {
    match group {
        "gap" | "gatt" | "hal" | "l2cap" => {
            Some(rust_crate.join(format!("src/vendor/command/{group}.rs")))
        }
        _ => None,
    }
}
