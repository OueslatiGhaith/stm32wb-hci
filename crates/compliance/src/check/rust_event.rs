//! Structured parser for Rust vendor event decoding coverage.
//!
//! This module uses `syn` for the Rust side of the compliance check. That keeps
//! event coverage independent from formatting details such as line breaks,
//! comments, or match arm layout.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;
use syn::visit::Visit;
use syn::{Expr, ExprMatch, File, Item, Pat};

use super::MarkerLocation;

const EVENT_MARKER_PREFIX: &str = "compliance:";

/// Rust event coverage surfaces discovered in `src/vendor/event`.
#[derive(Debug)]
pub(super) struct RustEventCoverage {
    /// `event=ACI_*_EVENT` markers attached to `VendorEvent` variants.
    pub(super) vendor_event_markers: Vec<EventMarker>,
    /// Variants declared in `VendorEvent`.
    pub(super) vendor_event_variants: HashSet<String>,
    /// Variants constructed by `VendorEvent::new`.
    pub(super) vendor_event_handlers: HashSet<String>,
    /// Opcode constants handled by `VendorReturnParameters::new`.
    pub(super) vendor_return_handlers: HashSet<String>,
}

/// Explicit marker tying one Rust event variant to one generated ST event.
#[derive(Clone, Debug)]
pub(super) struct EventMarker {
    pub(super) st_event: String,
    pub(super) variant: Option<String>,
    pub(super) location: MarkerLocation,
}

/// Loads vendor event enum variants and dispatch handlers from the Rust crate.
pub(super) fn load_rust_event_coverage(rust_crate: &Path) -> Result<RustEventCoverage> {
    let event_path = rust_crate.join("src/vendor/event/mod.rs");
    let command_event_path = rust_crate.join("src/vendor/event/command.rs");
    let event_source = std::fs::read_to_string(&event_path)
        .with_context(|| format!("failed to read {}", event_path.display()))?;
    let command_event_source = std::fs::read_to_string(&command_event_path)
        .with_context(|| format!("failed to read {}", command_event_path.display()))?;
    let event_file = syn::parse_file(&event_source)
        .with_context(|| format!("failed to parse {}", event_path.display()))?;
    let command_event_file = syn::parse_file(&command_event_source)
        .with_context(|| format!("failed to parse {}", command_event_path.display()))?;

    Ok(RustEventCoverage {
        vendor_event_markers: parse_event_markers_in_file(&event_path, &event_source, &event_file),
        vendor_event_variants: parse_enum_variants(&event_file, "VendorEvent"),
        vendor_event_handlers: parse_vendor_event_handlers(&event_file),
        vendor_return_handlers: parse_vendor_return_handlers(&command_event_file),
    })
}

/// Parses `// compliance: event=...` markers attached to `VendorEvent` variants.
fn parse_event_markers_in_file(path: &Path, source: &str, file: &File) -> Vec<EventMarker> {
    let variants = parse_enum_variant_locations(path, file, "VendorEvent");
    let marker_lines = source
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| parse_event_marker_line(line).map(|event| (idx + 1, event)))
        .collect::<Vec<_>>();

    marker_lines
        .iter()
        .enumerate()
        .map(|(idx, (line, st_event))| {
            let next_marker_line = marker_lines.get(idx + 1).map(|(line, _)| *line);
            EventMarker {
                st_event: st_event.clone(),
                variant: next_variant_name(&variants, *line, next_marker_line),
                location: MarkerLocation {
                    file: path.display().to_string(),
                    line: *line,
                },
            }
        })
        .collect()
}

/// Parses simple enum variant names from a named Rust enum.
fn parse_enum_variants(file: &File, enum_name: &str) -> HashSet<String> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(item_enum) if item_enum.ident == enum_name => Some(item_enum),
            _ => None,
        })
        .flat_map(|item_enum| item_enum.variants.iter())
        .map(|variant| variant.ident.to_string())
        .collect()
}

/// Parses enum variant names and source locations from a named Rust enum.
fn parse_enum_variant_locations(
    path: &Path,
    file: &File,
    enum_name: &str,
) -> Vec<(String, MarkerLocation)> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(item_enum) if item_enum.ident == enum_name => Some(item_enum),
            _ => None,
        })
        .flat_map(|item_enum| item_enum.variants.iter())
        .map(|variant| {
            (
                variant.ident.to_string(),
                MarkerLocation {
                    file: path.display().to_string(),
                    line: variant.ident.span().start().line,
                },
            )
        })
        .collect()
}

/// Parses one `// compliance: event=ACI_*_EVENT` line.
fn parse_event_marker_line(line: &str) -> Option<String> {
    let line = line.trim().strip_prefix("//")?.trim();
    let marker = line.strip_prefix(EVENT_MARKER_PREFIX)?.trim();
    marker.split_whitespace().find_map(|part| {
        part.strip_prefix("event=")
            .map(str::to_owned)
            .filter(|event| !event.is_empty())
    })
}

/// Finds the next enum variant after a marker comment.
fn next_variant_name(
    variants: &[(String, MarkerLocation)],
    marker_line: usize,
    next_marker_line: Option<usize>,
) -> Option<String> {
    variants
        .iter()
        .find(|(_, location)| {
            location.line > marker_line
                && next_marker_line.is_none_or(|next_marker_line| location.line < next_marker_line)
        })
        .map(|(name, _)| name.clone())
}

/// Parses event variants constructed in `VendorEvent::new` match arms.
fn parse_vendor_event_handlers(file: &File) -> HashSet<String> {
    let mut visitor = MatchArmPathVisitor::new(PathKind::VendorEventVariant);
    visitor.visit_file(file);
    visitor.items
}

/// Parses vendor opcode constants handled in command-complete return decoding.
fn parse_vendor_return_handlers(file: &File) -> HashSet<String> {
    let mut visitor = MatchArmPathVisitor::new(PathKind::VendorOpcodeConst);
    visitor.visit_file(file);
    visitor.items
}

/// Which path form should be collected from match arms.
#[derive(Clone, Copy)]
enum PathKind {
    VendorEventVariant,
    VendorOpcodeConst,
}

/// Collects selected path names from match arm patterns and bodies.
struct MatchArmPathVisitor {
    kind: PathKind,
    items: HashSet<String>,
}

impl MatchArmPathVisitor {
    fn new(kind: PathKind) -> Self {
        Self {
            kind,
            items: HashSet::new(),
        }
    }

    fn collect_from_pat(&mut self, pat: &Pat) {
        let mut collector = PathCollector {
            kind: self.kind,
            items: &mut self.items,
        };
        collector.visit_pat(pat);
    }

    fn collect_from_expr(&mut self, expr: &Expr) {
        let mut collector = PathCollector {
            kind: self.kind,
            items: &mut self.items,
        };
        collector.visit_expr(expr);
    }
}

impl<'ast> Visit<'ast> for MatchArmPathVisitor {
    fn visit_expr_match(&mut self, expr_match: &'ast ExprMatch) {
        for arm in &expr_match.arms {
            self.collect_from_pat(&arm.pat);
            if let Some((_, guard)) = &arm.guard {
                self.collect_from_expr(guard);
            }
            self.collect_from_expr(&arm.body);
        }
    }
}

/// Path visitor that extracts one category of compliance-relevant path.
struct PathCollector<'a> {
    kind: PathKind,
    items: &'a mut HashSet<String>,
}

impl<'ast> Visit<'ast> for PathCollector<'_> {
    fn visit_pat(&mut self, pat: &'ast Pat) {
        if let Pat::Path(pat_path) = pat {
            self.collect_path(&pat_path.path);
        }

        syn::visit::visit_pat(self, pat);
    }

    fn visit_expr_path(&mut self, expr_path: &'ast syn::ExprPath) {
        self.collect_path(&expr_path.path);
        syn::visit::visit_expr_path(self, expr_path);
    }
}

impl PathCollector<'_> {
    fn collect_path(&mut self, path: &syn::Path) {
        let segments = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();

        match self.kind {
            PathKind::VendorEventVariant => {
                if let Some(variant) = vendor_event_variant(&segments) {
                    self.items.insert(variant);
                }
            }
            PathKind::VendorOpcodeConst => {
                if let Some(opcode) = vendor_opcode_const(&segments) {
                    self.items.insert(opcode);
                }
            }
        }
    }
}

/// Extracts `Variant` from `VendorEvent::Variant`.
fn vendor_event_variant(segments: &[String]) -> Option<String> {
    let [.., enum_name, variant] = segments else {
        return None;
    };
    (enum_name == "VendorEvent").then(|| variant.clone())
}

/// Extracts `CONST` from `crate::vendor::opcode::CONST`.
fn vendor_opcode_const(segments: &[String]) -> Option<String> {
    let [krate, vendor, opcode, name] = segments else {
        return None;
    };
    (krate == "crate"
        && vendor == "vendor"
        && opcode == "opcode"
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
    .then(|| name.clone())
}
