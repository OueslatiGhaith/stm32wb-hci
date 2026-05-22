//! Shared Rust source loading for command compliance scanners.
//!
//! Command markers, trait methods, and method implementations all inspect the
//! same vendor command files. This module reads and parses those files once so
//! each scanner can focus on its own extraction logic.

use super::COMMAND_GROUPS;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use syn::File;

/// Parsed Rust vendor command file.
pub(super) struct RustCommandFile {
    /// Path to the source file.
    pub(super) path: PathBuf,
    /// Original Rust source text.
    pub(super) source: String,
    /// Parsed Rust syntax tree for `source`.
    pub(super) syntax: File,
}

/// Loads and parses all Rust vendor command files checked for compliance.
pub(super) fn load_rust_command_files(rust_crate: &Path) -> Result<Vec<RustCommandFile>> {
    let command_dir = rust_crate.join("src/vendor/command");
    let mut files = Vec::new();

    for group in COMMAND_GROUPS {
        let path = command_dir.join(format!("{group}.rs"));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let syntax = parse_rust_file(&path, &source)?;
        files.push(RustCommandFile {
            path,
            source,
            syntax,
        });
    }

    Ok(files)
}

/// Parses Rust source into a syntax tree with a path-aware error.
pub(super) fn parse_rust_file(path: &Path, source: &str) -> Result<File> {
    syn::parse_file(source).with_context(|| format!("failed to parse {}", path.display()))
}
