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
use super::cfg::FirmwareCfg;
use super::marker::{MarkerTarget, attach_markers, marker_value};

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
pub(super) fn load_rust_event_coverage(
    rust_crate: &Path,
    firmware_cfg: Option<&FirmwareCfg>,
) -> Result<RustEventCoverage> {
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
        vendor_event_markers: parse_event_markers_in_file(
            &event_path,
            &event_source,
            &event_file,
            firmware_cfg,
        ),
        vendor_event_variants: parse_enum_variants(&event_file, "VendorEvent", firmware_cfg),
        vendor_event_handlers: parse_vendor_event_handlers(&event_file, firmware_cfg),
        vendor_return_handlers: parse_vendor_return_handlers(&command_event_file, firmware_cfg),
    })
}

/// Parses `// compliance: event=...` markers attached to `VendorEvent` variants.
fn parse_event_markers_in_file(
    path: &Path,
    source: &str,
    file: &File,
    firmware_cfg: Option<&FirmwareCfg>,
) -> Vec<EventMarker> {
    let active_variants = parse_enum_variants(file, "VendorEvent", firmware_cfg);
    let targets = parse_enum_variant_targets(path, file, "VendorEvent", None);
    attach_markers(path, source, &targets, |body| marker_value(body, "event"))
        .into_iter()
        .filter(|marker| {
            marker
                .target
                .as_ref()
                .is_none_or(|variant| active_variants.contains(variant))
        })
        .map(|marker| EventMarker {
            st_event: marker.value,
            variant: marker.target,
            location: marker.location,
        })
        .collect()
}

/// Parses simple enum variant names from a named Rust enum.
fn parse_enum_variants(
    file: &File,
    enum_name: &str,
    firmware_cfg: Option<&FirmwareCfg>,
) -> HashSet<String> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(item_enum) if item_enum.ident == enum_name => Some(item_enum),
            _ => None,
        })
        .flat_map(|item_enum| item_enum.variants.iter())
        .filter(|variant| firmware_cfg.is_none_or(|cfg| cfg.allows_attrs(&variant.attrs)))
        .map(|variant| variant.ident.to_string())
        .collect()
}

/// Parses enum variant names and source locations from a named Rust enum.
fn parse_enum_variant_targets(
    path: &Path,
    file: &File,
    enum_name: &str,
    firmware_cfg: Option<&FirmwareCfg>,
) -> Vec<MarkerTarget> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(item_enum) if item_enum.ident == enum_name => Some(item_enum),
            _ => None,
        })
        .flat_map(|item_enum| item_enum.variants.iter())
        .filter(|variant| firmware_cfg.is_none_or(|cfg| cfg.allows_attrs(&variant.attrs)))
        .map(|variant| MarkerTarget {
            name: variant.ident.to_string(),
            location: MarkerLocation {
                file: path.display().to_string(),
                line: variant.ident.span().start().line,
            },
        })
        .collect()
}

/// Parses event variants constructed in `VendorEvent::new` match arms.
fn parse_vendor_event_handlers(file: &File, firmware_cfg: Option<&FirmwareCfg>) -> HashSet<String> {
    let mut visitor = MatchArmPathVisitor::new(PathKind::VendorEventVariant, firmware_cfg);
    visitor.visit_file(file);
    visitor.items
}

/// Parses vendor opcode constants handled in command-complete return decoding.
fn parse_vendor_return_handlers(
    file: &File,
    firmware_cfg: Option<&FirmwareCfg>,
) -> HashSet<String> {
    let mut visitor = MatchArmPathVisitor::new(PathKind::VendorOpcodeConst, firmware_cfg);
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
    firmware_cfg: Option<FirmwareCfg>,
    items: HashSet<String>,
}

impl MatchArmPathVisitor {
    fn new(kind: PathKind, firmware_cfg: Option<&FirmwareCfg>) -> Self {
        Self {
            kind,
            firmware_cfg: firmware_cfg.copied(),
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
            if self
                .firmware_cfg
                .as_ref()
                .is_some_and(|cfg| !cfg.allows_attrs(&arm.attrs))
            {
                continue;
            }
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
