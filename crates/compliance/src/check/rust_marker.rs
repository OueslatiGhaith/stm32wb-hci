//! Parser for Rust `// compliance:` marker comments.
//!
//! Markers are attached to the next Rust method in the same command file. A
//! primary marker claims an ST command; an alias marker documents an additional
//! Rust method that intentionally maps to an already claimed ST command. The
//! comments themselves must be scanned from source text, but method attachment
//! is based on the parsed Rust trait syntax.

use super::{COMMAND_GROUPS, MarkerLocation};
use anyhow::{Context, Result};
use std::path::Path;

const MARKER_PREFIX: &str = "compliance:";

/// All marker comments discovered in the Rust vendor command modules.
#[derive(Default)]
pub(super) struct LoadedCommandMarkers {
    /// Primary `st=...` markers, one per implemented ST command.
    pub(super) primary: Vec<CommandMarker>,
    /// Alias `alias_of=...` markers for alternate Rust methods.
    pub(super) aliases: Vec<AliasMarker>,
}

/// Primary marker tying one Rust command method to one ST command.
#[derive(Clone, Debug)]
pub(super) struct CommandMarker {
    pub(super) st_command: String,
    pub(super) method: Option<String>,
    pub(super) location: MarkerLocation,
}

/// Alias marker tying an alternate Rust method to an existing ST command.
#[derive(Clone, Debug)]
pub(super) struct AliasMarker {
    pub(super) alias_of: String,
    pub(super) method: Option<String>,
    pub(super) location: MarkerLocation,
}

/// Loads compliance markers from all checked Rust vendor command modules.
pub(super) fn load_command_markers(rust_crate: &Path) -> Result<LoadedCommandMarkers> {
    let command_dir = rust_crate.join("src/vendor/command");
    let mut markers = LoadedCommandMarkers::default();

    for group in COMMAND_GROUPS {
        let path = command_dir.join(format!("{group}.rs"));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let file = super::rust_method::parse_file(&path, &source)?;
        let file_markers = parse_markers_in_file(&path, &source, &file);
        markers.primary.extend(file_markers.primary);
        markers.aliases.extend(file_markers.aliases);
    }

    Ok(markers)
}

/// Parses marker comments in a single Rust command file.
fn parse_markers_in_file(path: &Path, source: &str, file: &syn::File) -> LoadedCommandMarkers {
    let methods = super::rust_method::parse_trait_methods_in_file(path, file);
    let marker_lines = source
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| parse_marker_line(line).map(|marker| (idx + 1, marker)))
        .collect::<Vec<_>>();
    let mut markers = LoadedCommandMarkers::default();

    for (idx, (line, marker)) in marker_lines.iter().enumerate() {
        let next_marker_line = marker_lines.get(idx + 1).map(|(line, _)| *line);
        let location = MarkerLocation {
            file: path.display().to_string(),
            line: *line,
        };
        match marker {
            MarkerKind::Primary { st } => {
                markers.primary.push(CommandMarker {
                    st_command: st.clone(),
                    method: next_method_name(&methods, *line, next_marker_line),
                    location,
                });
            }
            MarkerKind::Alias { alias_of } => {
                markers.aliases.push(AliasMarker {
                    alias_of: alias_of.clone(),
                    method: next_method_name(&methods, *line, next_marker_line),
                    location,
                });
            }
        }
    }

    markers
}

/// Raw marker kind before it is attached to a source location and method.
enum MarkerKind {
    Primary { st: String },
    Alias { alias_of: String },
}

/// Parses one `// compliance:` line.
///
/// Supported forms are `st=ACI_*` and `alias_of=ACI_*`.
fn parse_marker_line(line: &str) -> Option<MarkerKind> {
    let line = line.trim().strip_prefix("//")?.trim();
    let marker = line.strip_prefix(MARKER_PREFIX)?.trim();
    let mut st = None;
    let mut alias_of = None;

    for part in marker.split_whitespace() {
        if let Some(value) = part.strip_prefix("st=") {
            st = Some(value.to_owned());
        } else if let Some(value) = part.strip_prefix("alias_of=") {
            alias_of = Some(value.to_owned());
        }
    }

    if let Some(alias_of) = alias_of {
        Some(MarkerKind::Alias { alias_of })
    } else {
        Some(MarkerKind::Primary { st: st? })
    }
}

/// Finds the next Rust trait method after a marker comment.
///
/// A following compliance marker stops the search so adjacent markers do not
/// accidentally attach to the same method.
fn next_method_name(
    methods: &[super::rust_method::RustCommandMethod],
    marker_line: usize,
    next_marker_line: Option<usize>,
) -> Option<String> {
    methods
        .iter()
        .find(|method| {
            method.location.line > marker_line
                && next_marker_line
                    .is_none_or(|next_marker_line| method.location.line < next_marker_line)
        })
        .map(|method| method.name.clone())
}
