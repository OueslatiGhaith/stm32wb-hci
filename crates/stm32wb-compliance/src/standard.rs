//! Discovery of standard-HCI commands declared by this crate.
//!
//! The compliance tool deliberately ignores APIs inherited from dependencies.
//! It only validates the STM32WB-specific standard commands declared in
//! `src/standard.rs` against the selected CubeWB catalog.

use std::fs;
use std::path::Path;

use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Ident, Item, LitInt, Token, braced, parenthesized};

use crate::FirmwareVersion;
use crate::model::{CoverageEntry, CoverageOrigin};
use crate::rust_cfg::attrs_active;

/// Load the standard-HCI commands implemented directly by this crate for the
/// selected firmware. Codes are full HCI opcodes, rather than vendor OCFs.
pub(crate) fn load_local_standard_commands(
    crate_dir: &Path,
    firmware: FirmwareVersion,
) -> Result<Vec<CoverageEntry>, String> {
    let path = crate_dir.join("src/standard.rs");
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    let mut entries = Vec::new();
    for item in &file.items {
        let Item::Macro(item) = item else {
            continue;
        };
        if !macro_name_is(&item.mac, "cmd") || !attrs_active(&item.attrs, firmware, &path)? {
            continue;
        }
        let header =
            syn::parse2::<CommandMacroHeader>(item.mac.tokens.clone()).map_err(|error| {
                format!(
                    "{}: could not parse cmd! declaration structurally: {error}",
                    path.display()
                )
            })?;
        let Some(ogf) = standard_ogf(&header.group) else {
            // `BASE` macro implementation details and non-standard groups are
            // deliberately not treated as public standard command declarations.
            continue;
        };
        if header.ocf > 0x03ff {
            return Err(format!(
                "{}: standard command {} has OCF 0x{:X}, which exceeds ten bits",
                path.display(),
                header.name,
                header.ocf
            ));
        }
        entries.push(
            CoverageEntry::new(
                (u16::from(ogf) << 10) | header.ocf,
                header.name,
                CoverageOrigin::StandardHciExtension,
            )
            .at(path.clone()),
        );
    }
    sort_and_deduplicate(&mut entries);
    Ok(entries)
}

fn macro_name_is(mac: &syn::Macro, name: &str) -> bool {
    mac.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

struct CommandMacroHeader {
    name: String,
    group: String,
    ocf: u16,
}

/// Small grammar for `[BASE] [attributes] Name(GROUP, OCF) { ... }`.
impl Parse for CommandMacroHeader {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(Ident) {
            let fork = input.fork();
            let marker = fork.parse::<Ident>()?;
            if marker == "BASE" {
                input.parse::<Ident>()?;
            }
        }
        let _attributes = input.call(Attribute::parse_outer)?;
        let name = input.parse::<Ident>()?;
        let arguments;
        parenthesized!(arguments in input);
        let group = arguments.parse::<Ident>()?;
        arguments.parse::<Token![,]>()?;
        let ocf = arguments.parse::<LitInt>()?.base10_parse::<u16>()?;
        if !arguments.is_empty() {
            return Err(arguments.error("unexpected command header tokens"));
        }
        let body;
        braced!(body in input);
        let _body = body.parse::<TokenStream>()?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after cmd! declaration"));
        }
        Ok(Self {
            name: name.to_string(),
            group: group.to_string(),
            ocf,
        })
    }
}

fn standard_ogf(group: &str) -> Option<u8> {
    match group {
        "LINK_CONTROL" => Some(0x01),
        "LINK_POLICY" => Some(0x02),
        "CONTROL_BASEBAND" => Some(0x03),
        "INFO_PARAMS" => Some(0x04),
        "STATUS_PARAMS" => Some(0x05),
        "TESTING" => Some(0x06),
        "LE" => Some(0x08),
        _ => None,
    }
}

fn sort_and_deduplicate(entries: &mut Vec<CoverageEntry>) {
    entries.sort_by_key(|entry| (entry.code, entry.name.clone()));
    entries.dedup_by(|left, right| left.code == right.code && left.name == right.name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_command_headers() {
        let header = syn::parse_str::<CommandMacroHeader>(
            "LeSetAdvData ( LE , 0x0008 ) { Params = [u8 ; 32] ; Return = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeSetAdvData");
        assert_eq!(header.group, "LE");
        assert_eq!(header.ocf, 8);
    }

    #[test]
    fn ignores_doc_attributes_before_a_command_header() {
        let header = syn::parse_str::<CommandMacroHeader>(
            "# [ doc = \"command\" ] LeTest ( LE , 0x001F ) { Params = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeTest");
    }

    #[test]
    fn accepts_base_command_headers() {
        let header = syn::parse_str::<CommandMacroHeader>(
            "BASE # [ doc = \"command\" ] LeExtended ( LE , 0x0041 ) { Params = () ; }",
        )
        .unwrap();
        assert_eq!(header.name, "LeExtended");
        assert_eq!(header.ocf, 0x41);
    }
}
