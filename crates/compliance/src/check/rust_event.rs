//! Lightweight scanner for Rust vendor event decoding coverage.
//!
//! This module recognizes enum variants and match arms in the vendor event
//! modules. It intentionally mirrors the style of the command checker: parse
//! only the stable local formatting needed for compliance reporting.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

/// Rust event coverage surfaces discovered in `src/vendor/event`.
#[derive(Debug)]
pub(super) struct RustEventCoverage {
    /// Variants declared in `VendorEvent`.
    pub(super) vendor_event_variants: HashSet<String>,
    /// Variants constructed by `VendorEvent::new`.
    pub(super) vendor_event_handlers: HashSet<String>,
    /// Opcode constants handled by `VendorReturnParameters::new`.
    pub(super) vendor_return_handlers: HashSet<String>,
}

/// Loads vendor event enum variants and dispatch handlers from the Rust crate.
pub(super) fn load_rust_event_coverage(rust_crate: &Path) -> Result<RustEventCoverage> {
    let event_path = rust_crate.join("src/vendor/event/mod.rs");
    let command_event_path = rust_crate.join("src/vendor/event/command.rs");
    let event_source = std::fs::read_to_string(&event_path)
        .with_context(|| format!("failed to read {}", event_path.display()))?;
    let command_event_source = std::fs::read_to_string(&command_event_path)
        .with_context(|| format!("failed to read {}", command_event_path.display()))?;

    Ok(RustEventCoverage {
        vendor_event_variants: parse_enum_variants(&event_source, "VendorEvent"),
        vendor_event_handlers: parse_vendor_event_handlers(&event_source),
        vendor_return_handlers: parse_vendor_return_handlers(&command_event_source),
    })
}

/// Parses simple enum variant names from a named Rust enum.
fn parse_enum_variants(source: &str, enum_name: &str) -> HashSet<String> {
    let mut variants = HashSet::new();
    let mut in_enum = false;
    let mut depth = 0isize;

    for line in source.lines() {
        let code = strip_line_comment(line);
        let trimmed = code.trim();

        if !in_enum
            && trimmed.starts_with("pub enum ")
            && trimmed.contains(enum_name)
            && trimmed.contains('{')
        {
            in_enum = true;
        }

        if in_enum {
            depth += count_char(code, '{') as isize;
            depth -= count_char(code, '}') as isize;
            if let Some(variant) = parse_variant_name(trimmed) {
                variants.insert(variant);
            }
            if depth <= 0 {
                in_enum = false;
                depth = 0;
            }
        }
    }

    variants
}

/// Parses event variants constructed in `VendorEvent::new` match arms.
fn parse_vendor_event_handlers(source: &str) -> HashSet<String> {
    let mut handlers = HashSet::new();
    const PREFIX: &str = "VendorEvent::";

    for line in source.lines().filter(|line| line.contains("=>")) {
        let mut rest = line;
        while let Some(idx) = rest.find(PREFIX) {
            let after_prefix = &rest[idx + PREFIX.len()..];
            let variant = take_ident(after_prefix);
            if !variant.is_empty() {
                handlers.insert(variant);
            }
            rest = after_prefix;
        }
    }

    handlers
}

/// Parses vendor opcode constants handled in command-complete return decoding.
fn parse_vendor_return_handlers(source: &str) -> HashSet<String> {
    let mut handlers = HashSet::new();
    const PREFIX: &str = "crate::vendor::opcode::";

    for line in source.lines().filter(|line| line.contains("=>")) {
        let mut rest = line;
        while let Some(idx) = rest.find(PREFIX) {
            let after_prefix = &rest[idx + PREFIX.len()..];
            let opcode = after_prefix
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                .collect::<String>();
            if !opcode.is_empty() {
                handlers.insert(opcode);
            }
            rest = after_prefix;
        }
    }

    handlers
}

/// Parses an enum variant from a line inside an enum body.
fn parse_variant_name(line: &str) -> Option<String> {
    let first = take_ident(line);
    if first.is_empty() || matches!(first.as_str(), "pub" | "enum") {
        return None;
    }
    line[first.len()..]
        .trim_start()
        .starts_with(['(', ',', '='])
        .then_some(first)
}

/// Takes a Rust identifier from the start of `source`.
fn take_ident(source: &str) -> String {
    source
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// Removes line comments before structural scanning.
fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or_default()
}

/// Counts a character in a source fragment.
fn count_char(source: &str, needle: char) -> usize {
    source.chars().filter(|c| *c == needle).count()
}
