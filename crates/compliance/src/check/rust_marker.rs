//! Parser for Rust `// compliance:` marker comments.
//!
//! Markers are attached to the next Rust method in the same command file. A
//! primary marker claims an ST command; an alias marker documents an additional
//! Rust method that intentionally maps to an already claimed ST command. The
//! comments themselves must be scanned from source text, but method attachment
//! is based on the parsed Rust trait syntax.

use super::marker::{MarkerTarget, attach_markers, marker_value};
use super::{COMMAND_GROUPS, MarkerLocation};
use anyhow::{Context, Result};
use std::path::Path;

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
    let targets = super::rust_method::parse_trait_methods_in_file(path, file)
        .into_iter()
        .map(|method| MarkerTarget {
            name: method.name,
            location: method.location,
        })
        .collect::<Vec<_>>();
    let mut markers = LoadedCommandMarkers::default();

    for marker in attach_markers(path, source, &targets, parse_marker_body) {
        match marker.value {
            MarkerKind::Primary { st } => {
                markers.primary.push(CommandMarker {
                    st_command: st,
                    method: marker.target,
                    location: marker.location,
                });
            }
            MarkerKind::Alias { alias_of } => {
                markers.aliases.push(AliasMarker {
                    alias_of,
                    method: marker.target,
                    location: marker.location,
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
fn parse_marker_body(body: &str) -> Option<MarkerKind> {
    if let Some(alias_of) = marker_value(body, "alias_of") {
        Some(MarkerKind::Alias { alias_of })
    } else {
        marker_value(body, "st").map(|st| MarkerKind::Primary { st })
    }
}
