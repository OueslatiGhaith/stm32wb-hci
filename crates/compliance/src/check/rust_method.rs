//! Structured parser for Rust command traits and method implementations.
//!
//! This module uses `syn` for Rust syntax and `proc_macro2` token trees for
//! local helper macro invocations. It extracts trait method declarations and
//! implementation opcode references without depending on source formatting.

use super::{COMMAND_GROUPS, MarkerLocation};
use anyhow::{Context, Result};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::collections::HashMap;
use std::path::Path;
use syn::visit::Visit;
use syn::{File, ImplItem, Item, TraitItem};

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
        let file = parse_file(&path, &source)?;
        methods.extend(parse_trait_methods_in_file(&path, &file));
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
        let file = parse_file(&path, &source)?;
        let file_name = path.display().to_string();
        for (method, implementation) in parse_method_implementations_in_file(&file) {
            implementations.insert((file_name.clone(), method), implementation);
        }
    }

    Ok(implementations)
}

/// Parses Rust source into a syntax tree with a path-aware error.
pub(super) fn parse_file(path: &Path, source: &str) -> Result<File> {
    syn::parse_file(source).with_context(|| format!("failed to parse {}", path.display()))
}

/// Parses command trait method declarations in one syntax tree.
pub(super) fn parse_trait_methods_in_file(path: &Path, file: &File) -> Vec<RustCommandMethod> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Trait(item_trait) if item_trait.ident.to_string().contains("Commands") => {
                Some(item_trait)
            }
            _ => None,
        })
        .flat_map(|item_trait| item_trait.items.iter())
        .filter_map(|trait_item| match trait_item {
            TraitItem::Fn(method) => Some(RustCommandMethod {
                name: method.sig.ident.to_string(),
                location: MarkerLocation {
                    file: path.display().to_string(),
                    line: method.sig.ident.span().start().line,
                },
            }),
            _ => None,
        })
        .collect()
}

/// Parses impl methods and local impl macro invocations for opcode references.
fn parse_method_implementations_in_file(file: &File) -> Vec<(String, RustMethodImplementation)> {
    let mut implementations = Vec::new();

    for item in &file.items {
        let Item::Impl(item_impl) = item else {
            continue;
        };

        for impl_item in &item_impl.items {
            match impl_item {
                ImplItem::Fn(method) => implementations.push((
                    method.sig.ident.to_string(),
                    RustMethodImplementation {
                        opcodes: opcode_consts_in_block(&method.block),
                    },
                )),
                ImplItem::Macro(item_macro) => {
                    let tokens = item_macro.mac.tokens.clone();
                    if let Some(method) = method_name_from_macro_tokens(&tokens) {
                        implementations.push((
                            method,
                            RustMethodImplementation {
                                opcodes: opcode_consts_in_tokens(tokens),
                            },
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    implementations
}

/// Extracts unique `crate::vendor::opcode::*` constants from a method body.
fn opcode_consts_in_block(block: &syn::Block) -> Vec<String> {
    let mut visitor = OpcodePathVisitor::default();
    visitor.visit_block(block);
    visitor.opcodes
}

/// Path visitor that records vendor opcode constants in first-seen order.
#[derive(Default)]
struct OpcodePathVisitor {
    opcodes: Vec<String>,
}

impl<'ast> Visit<'ast> for OpcodePathVisitor {
    fn visit_expr_path(&mut self, expr_path: &'ast syn::ExprPath) {
        let segments = path_segments(&expr_path.path);
        if let Some(opcode) = vendor_opcode_const(&segments) {
            push_unique(&mut self.opcodes, opcode);
        }

        syn::visit::visit_expr_path(self, expr_path);
    }
}

/// Extracts the first macro argument, which is the generated method name.
fn method_name_from_macro_tokens(tokens: &TokenStream) -> Option<String> {
    tokens.clone().into_iter().find_map(|token| match token {
        TokenTree::Ident(ident) => Some(ident.to_string()),
        _ => None,
    })
}

/// Extracts unique `crate::vendor::opcode::*` constants from macro tokens.
fn opcode_consts_in_tokens(tokens: TokenStream) -> Vec<String> {
    let mut idents = Vec::new();
    collect_ident_tokens(tokens, &mut idents);

    let mut opcodes = Vec::new();
    for window in idents.windows(4) {
        if let Some(opcode) = vendor_opcode_const(window) {
            push_unique(&mut opcodes, opcode);
        }
    }

    opcodes
}

/// Recursively flattens identifiers from a token stream.
fn collect_ident_tokens(tokens: TokenStream, out: &mut Vec<String>) {
    for token in tokens {
        match token {
            TokenTree::Ident(ident) => out.push(ident.to_string()),
            TokenTree::Group(group) if group.delimiter() != Delimiter::None => {
                collect_ident_tokens(group.stream(), out);
            }
            _ => {}
        }
    }
}

/// Converts a syn path into string segments.
fn path_segments(path: &syn::Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
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

/// Appends an item only if it has not already been seen.
fn push_unique(items: &mut Vec<String>, item: String) {
    if !items.contains(&item) {
        items.push(item);
    }
}
