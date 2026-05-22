//! Parser for Rust vendor opcode constants.
//!
//! The checked crate defines vendor opcodes through the local `vendor_opcodes!`
//! macro. This module parses the Rust file with `syn`, then walks the macro
//! token tree to reconstruct each final Bluetooth vendor opcode value.

use super::cfg::FirmwareCfg;
use anyhow::{Context, Result};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::path::Path;
use syn::Item;

/// Rust opcode constant with its fully reconstructed numeric opcode.
#[derive(Debug)]
pub(super) struct RustOpcode {
    pub(super) name: String,
    pub(super) opcode: u16,
}

/// Parses `src/vendor/opcode.rs` and returns all vendor opcode constants.
pub(super) fn parse_rust_opcodes(
    path: &Path,
    firmware_cfg: Option<&FirmwareCfg>,
) -> Result<Vec<RustOpcode>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let file =
        syn::parse_file(&source).with_context(|| format!("failed to parse {}", path.display()))?;
    let mut opcodes = Vec::new();

    for item in &file.items {
        let Item::Macro(item_macro) = item else {
            continue;
        };
        if item_macro
            .mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "vendor_opcodes")
        {
            opcodes.extend(parse_vendor_opcode_macro(
                item_macro.mac.tokens.clone(),
                firmware_cfg,
            ));
        }
    }

    Ok(opcodes)
}

/// Parses the body of the `vendor_opcodes!` invocation.
fn parse_vendor_opcode_macro(
    tokens: TokenStream,
    firmware_cfg: Option<&FirmwareCfg>,
) -> Vec<RustOpcode> {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut opcodes = Vec::new();
    let mut idx = 0;

    while idx + 4 < tokens.len() {
        let Some(cgid) = parse_group_header(&tokens[idx..idx + 4]) else {
            idx += 1;
            continue;
        };

        if let TokenTree::Group(group) = &tokens[idx + 4]
            && group.delimiter() == Delimiter::Brace
        {
            opcodes.extend(parse_group_opcodes(cgid, group.stream(), firmware_cfg));
            idx += 5;
            continue;
        }

        idx += 1;
    }

    opcodes
}

/// Parses a group header token sequence like `Gap = 0x1;`.
fn parse_group_header(tokens: &[TokenTree]) -> Option<u16> {
    let [
        TokenTree::Ident(_),
        equals,
        TokenTree::Literal(value),
        semicolon,
    ] = tokens
    else {
        return None;
    };
    (is_punct(equals, '=') && is_punct(semicolon, ';')).then(|| parse_int_literal(value))?
}

/// Parses command ID constants inside one opcode group.
fn parse_group_opcodes(
    cgid: u16,
    tokens: TokenStream,
    firmware_cfg: Option<&FirmwareCfg>,
) -> Vec<RustOpcode> {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    let mut opcodes = Vec::new();
    let mut idx = 0;

    while idx + 5 < tokens.len() {
        let mut attrs_allowed = true;
        while let Some((allowed, next_idx)) = parse_attribute(&tokens, idx, firmware_cfg) {
            attrs_allowed &= allowed;
            idx = next_idx;
        }

        let [
            TokenTree::Ident(pub_ident),
            TokenTree::Ident(const_ident),
            TokenTree::Ident(name),
            equals,
            TokenTree::Literal(cid),
            semicolon,
        ] = &tokens[idx..idx + 6]
        else {
            idx += 1;
            continue;
        };

        if pub_ident == "pub"
            && const_ident == "const"
            && is_punct(equals, '=')
            && is_punct(semicolon, ';')
            && let Some(cid) = parse_int_literal(cid)
        {
            if attrs_allowed {
                let ocf = ((cgid & 0b111) << 7) | (cid & 0b111_1111);
                let opcode = (0x3f << 10) | ocf;
                opcodes.push(RustOpcode {
                    name: name.to_string(),
                    opcode,
                });
            }
            idx += 6;
            continue;
        }

        idx += 1;
    }

    opcodes
}

/// Parses and evaluates one attribute token sequence like `#[cfg(...)]`.
fn parse_attribute(
    tokens: &[TokenTree],
    idx: usize,
    firmware_cfg: Option<&FirmwareCfg>,
) -> Option<(bool, usize)> {
    let [TokenTree::Punct(hash), TokenTree::Group(group), ..] = &tokens[idx..] else {
        return None;
    };
    if hash.as_char() != '#' || group.delimiter() != Delimiter::Bracket {
        return None;
    }

    let attr_tokens = group.stream().into_iter().collect::<Vec<_>>();
    let allowed = match attr_tokens.as_slice() {
        [TokenTree::Ident(cfg_ident), TokenTree::Group(cfg_group)]
            if cfg_ident == "cfg" && cfg_group.delimiter() == Delimiter::Parenthesis =>
        {
            firmware_cfg
                .is_none_or(|firmware_cfg| firmware_cfg.allows_cfg_stream(cfg_group.stream()))
        }
        _ => true,
    };

    Some((allowed, idx + 2))
}

/// Checks a single punctuation token.
fn is_punct(token: &TokenTree, value: char) -> bool {
    matches!(token, TokenTree::Punct(punct) if punct.as_char() == value)
}

/// Parses a decimal or hex literal token.
fn parse_int_literal(literal: &proc_macro2::Literal) -> Option<u16> {
    let value = literal.to_string().replace('_', "");
    if let Some(hex) = value.strip_prefix("0x") {
        let digits = hex
            .chars()
            .take_while(|c| c.is_ascii_hexdigit())
            .collect::<String>();
        u16::from_str_radix(&digits, 16).ok()
    } else {
        let digits = value
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        digits.parse().ok()
    }
}
