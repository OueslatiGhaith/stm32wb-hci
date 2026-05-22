//! Shared parsing and attachment helpers for `// compliance:` markers.
//!
//! Rust doc comments are not part of the `syn` syntax tree, so compliance
//! comments must be scanned from source text. This module centralizes the
//! common behavior: parse marker comment bodies, attach each marker to the next
//! parsed Rust item, and preserve the marker source location.

use super::MarkerLocation;
use std::path::Path;

const MARKER_PREFIX: &str = "compliance:";

/// Rust item that a marker can attach to.
pub(super) struct MarkerTarget {
    /// Rust item name.
    pub(super) name: String,
    /// Rust item source location.
    pub(super) location: MarkerLocation,
}

/// Marker value after it has been attached to the next Rust item.
pub(super) struct AttachedMarker<T> {
    /// Parsed marker payload.
    pub(super) value: T,
    /// Name of the next Rust item before the next marker, if present.
    pub(super) target: Option<String>,
    /// Source location of the marker comment.
    pub(super) location: MarkerLocation,
}

/// Scans source comments and attaches parsed markers to the next target item.
pub(super) fn attach_markers<T>(
    path: &Path,
    source: &str,
    targets: &[MarkerTarget],
    parse_marker: impl Fn(&str) -> Option<T>,
) -> Vec<AttachedMarker<T>> {
    let marker_lines = source
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let marker_body = compliance_marker_body(line)?;
            parse_marker(marker_body).map(|value| (idx + 1, value))
        })
        .collect::<Vec<_>>();

    let next_marker_lines = marker_lines
        .iter()
        .skip(1)
        .map(|(line, _)| Some(*line))
        .chain(std::iter::once(None))
        .collect::<Vec<_>>();

    marker_lines
        .into_iter()
        .zip(next_marker_lines)
        .map(|((line, value), next_marker_line)| AttachedMarker {
            value,
            target: next_target_name(targets, line, next_marker_line),
            location: MarkerLocation {
                file: path.display().to_string(),
                line,
            },
        })
        .collect()
}

/// Reads the body after `// compliance:`.
pub(super) fn compliance_marker_body(line: &str) -> Option<&str> {
    line.trim()
        .strip_prefix("//")?
        .trim()
        .strip_prefix(MARKER_PREFIX)
        .map(str::trim)
}

/// Returns a whitespace-delimited `key=value` marker field.
pub(super) fn marker_value(body: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    body.split_whitespace().find_map(|part| {
        part.strip_prefix(&prefix)
            .map(str::to_owned)
            .filter(|value| !value.is_empty())
    })
}

/// Finds the next Rust item after a marker comment.
///
/// A following compliance marker stops the search so adjacent markers do not
/// accidentally attach to the same item.
fn next_target_name(
    targets: &[MarkerTarget],
    marker_line: usize,
    next_marker_line: Option<usize>,
) -> Option<String> {
    targets
        .iter()
        .find(|target| {
            target.location.line > marker_line
                && next_marker_line
                    .is_none_or(|next_marker_line| target.location.line < next_marker_line)
        })
        .map(|target| target.name.clone())
}
