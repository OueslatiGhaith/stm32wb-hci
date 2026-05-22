//! Lightweight scanner for Rust command traits and method implementations.
//!
//! This module does not fully parse Rust. It recognizes the stable formatting
//! patterns used by the vendor command modules and extracts method names plus
//! referenced `crate::vendor::opcode::*` constants.

use super::{COMMAND_GROUPS, MarkerLocation};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Command trait method discovered in a vendor command module.
#[derive(Debug)]
pub(super) struct RustCommandMethod {
    pub(super) name: String,
    pub(super) location: MarkerLocation,
}

/// Opcode references discovered inside a Rust method implementation.
#[derive(Debug)]
pub(super) struct RustMethodImplementation {
    pub(super) opcodes: Vec<String>,
}

/// Loads command trait methods from all checked vendor command modules.
pub(super) fn load_rust_command_methods(rust_crate: &Path) -> Result<Vec<RustCommandMethod>> {
    let command_dir = rust_crate.join("src/vendor/command");
    let mut methods = Vec::new();

    for group in COMMAND_GROUPS {
        let path = command_dir.join(format!("{group}.rs"));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        methods.extend(parse_trait_methods_in_file(&path, &source));
    }

    Ok(methods)
}

/// Loads method implementations keyed by `(file, method_name)`.
pub(super) fn load_rust_method_implementations(
    rust_crate: &Path,
) -> Result<HashMap<(String, String), RustMethodImplementation>> {
    let command_dir = rust_crate.join("src/vendor/command");
    let mut implementations = HashMap::new();

    for group in COMMAND_GROUPS {
        let path = command_dir.join(format!("{group}.rs"));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file = path.display().to_string();
        for (method, implementation) in parse_method_implementations_in_file(&source) {
            implementations.insert((file.clone(), method), implementation);
        }
    }

    Ok(implementations)
}

/// Scans a command trait and returns each method it declares.
fn parse_trait_methods_in_file(path: &Path, source: &str) -> Vec<RustCommandMethod> {
    let mut methods = Vec::new();
    let mut in_command_trait = false;
    let mut trait_depth = 0isize;

    for (idx, line) in source.lines().enumerate() {
        let code = strip_line_comment(line);
        let trimmed = code.trim();

        if !in_command_trait
            && trimmed.starts_with("pub trait ")
            && trimmed.contains("Commands")
            && trimmed.contains('{')
        {
            in_command_trait = true;
        }

        if in_command_trait && let Some(method) = parse_fn_name(trimmed) {
            methods.push(RustCommandMethod {
                name: method,
                location: MarkerLocation {
                    file: path.display().to_string(),
                    line: idx + 1,
                },
            });
        }

        if in_command_trait {
            trait_depth += count_char(code, '{') as isize;
            trait_depth -= count_char(code, '}') as isize;
            if trait_depth <= 0 {
                in_command_trait = false;
                trait_depth = 0;
            }
        }
    }

    methods
}

/// Scans macro invocations and inline method bodies for opcode references.
fn parse_method_implementations_in_file(source: &str) -> Vec<(String, RustMethodImplementation)> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut implementations = Vec::new();

    for idx in 0..lines.len() {
        let trimmed = strip_line_comment(lines[idx]).trim();
        if let Some((method, invocation)) = parse_impl_macro_invocation(&lines, idx) {
            implementations.push((
                method,
                RustMethodImplementation {
                    opcodes: opcode_consts_in_source(&invocation),
                },
            ));
            continue;
        }

        if let Some(method) = parse_fn_name(trimmed)
            && let Some(body) = collect_braced_item(&lines, idx)
        {
            implementations.push((
                method,
                RustMethodImplementation {
                    opcodes: opcode_consts_in_source(&body),
                },
            ));
        }
    }

    implementations
}

/// Parses an `impl_*!(method, ..., opcode)` style macro invocation.
fn parse_impl_macro_invocation(lines: &[&str], start: usize) -> Option<(String, String)> {
    let line = strip_line_comment(lines[start]);
    let trimmed = line.trim();
    if !trimmed.starts_with("impl_") || !trimmed.contains("!(") {
        return None;
    }

    let invocation = collect_macro_invocation(lines, start)?;
    let args = invocation.split_once('(')?.1;
    let method = args
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();
    (!method.is_empty()).then_some((method, invocation))
}

/// Collects a multi-line macro invocation until the closing `);`.
fn collect_macro_invocation(lines: &[&str], start: usize) -> Option<String> {
    let mut out = String::new();
    for line in lines.iter().skip(start) {
        out.push_str(line);
        out.push('\n');
        if strip_line_comment(line).contains(");") {
            return Some(out);
        }
    }
    None
}

/// Collects a braced Rust item using simple brace counting.
fn collect_braced_item(lines: &[&str], start: usize) -> Option<String> {
    let mut out = String::new();
    let mut depth = 0isize;
    let mut seen_open = false;

    for line in lines.iter().skip(start) {
        let code = strip_line_comment(line);
        out.push_str(line);
        out.push('\n');

        let opens = count_char(code, '{') as isize;
        let closes = count_char(code, '}') as isize;
        if opens > 0 {
            seen_open = true;
        }
        depth += opens;
        depth -= closes;
        if seen_open && depth <= 0 {
            return Some(out);
        }
    }

    None
}

/// Extracts unique `crate::vendor::opcode::*` constant names from source text.
fn opcode_consts_in_source(source: &str) -> Vec<String> {
    let mut opcodes = Vec::new();
    let mut rest = source;
    const PREFIX: &str = "crate::vendor::opcode::";

    while let Some(idx) = rest.find(PREFIX) {
        let after_prefix = &rest[idx + PREFIX.len()..];
        let opcode = after_prefix
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .collect::<String>();
        if !opcode.is_empty() && !opcodes.contains(&opcode) {
            opcodes.push(opcode);
        }
        rest = after_prefix;
    }

    opcodes
}

/// Removes line comments before structural scanning.
fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or_default()
}

/// Counts a character in a source fragment.
fn count_char(source: &str, needle: char) -> usize {
    source.chars().filter(|c| *c == needle).count()
}

/// Extracts a Rust function name from a line containing `fn`.
pub(super) fn parse_fn_name(line: &str) -> Option<String> {
    let fn_pos = line.find("fn ")?;
    let rest = &line[fn_pos + 3..];
    let name = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}
