//! Discovery of standard-HCI commands declared by this crate.
//!
//! The compliance tool deliberately ignores APIs inherited from dependencies.
//! It only validates the STM32WB-specific standard commands declared in
//! `src/standard.rs` against the selected CubeWB catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Expr, ExprLit, Ident, Item, ItemStruct, Lit, LitInt, Token, Type, braced,
    parenthesized,
};

use crate::ComplianceTarget;
use crate::catalog::WireLayout;
use crate::model::{CoverageEntry, CoverageOrigin};
use crate::rust_cfg::attrs_active;
use crate::rust_source::{CommandCompletion, CommandDeclaration};

/// Load the standard-HCI commands implemented directly by this crate for the
/// selected release/profile target. Codes are full HCI opcodes, rather than
/// vendor OCFs.
pub(crate) fn load_local_standard_commands(
    crate_dir: &Path,
    target: ComplianceTarget,
) -> Result<Vec<CommandDeclaration>, String> {
    let path = crate_dir.join("src/standard.rs");
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    let structs = file
        .items
        .iter()
        .filter_map(|item| {
            let Item::Struct(item) = item else {
                return None;
            };
            match attrs_active(&item.attrs, target, &path) {
                Ok(true) => Some(Ok((item.ident.to_string(), item))),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut declarations = Vec::new();
    for item in &file.items {
        let Item::Macro(item) = item else {
            continue;
        };
        if !macro_name_is(&item.mac, "cmd") || !attrs_active(&item.attrs, target, &path)? {
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
        let request = fixed_type_width(
            &header.params,
            &structs,
            target,
            &path,
            &mut BTreeSet::new(),
        )?;
        let completion = if let Some(returns) = &header.returns {
            CommandCompletion::CommandComplete {
                returns: WireLayout::fixed(fixed_type_width(
                    returns,
                    &structs,
                    target,
                    &path,
                    &mut BTreeSet::new(),
                )?),
            }
        } else {
            CommandCompletion::CommandStatus
        };
        declarations.push(CommandDeclaration {
            code: (u16::from(ogf) << 10) | header.ocf,
            name: header.name,
            request: WireLayout::fixed(request),
            completion,
            location: path.clone(),
        });
    }
    declarations.sort_by_key(|entry| (entry.code, entry.name.clone()));
    for pair in declarations.windows(2) {
        if pair[0].code == pair[1].code {
            return Err(format!(
                "{}: standard commands `{}` and `{}` share opcode 0x{:04X}",
                path.display(),
                pair[0].name,
                pair[1].name,
                pair[0].code,
            ));
        }
    }
    Ok(declarations)
}

pub(crate) fn coverage_entries(declarations: &[CommandDeclaration]) -> Vec<CoverageEntry> {
    declarations
        .iter()
        .map(|declaration| {
            CoverageEntry::new(
                declaration.code,
                &declaration.name,
                CoverageOrigin::StandardHciExtension,
            )
            .at(declaration.location.clone())
        })
        .collect()
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
    params: Type,
    returns: Option<Type>,
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
        let body = body.parse::<CommandMacroBody>()?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after cmd! declaration"));
        }
        Ok(Self {
            name: name.to_string(),
            group: group.to_string(),
            ocf,
            params: body.params,
            returns: body.returns,
        })
    }
}

struct CommandMacroBody {
    params: Type,
    returns: Option<Type>,
}

impl Parse for CommandMacroBody {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut params = None;
        let mut returns = None;
        while !input.is_empty() {
            let label = input.parse::<Ident>()?;
            input.parse::<Token![=]>()?;
            let ty = input.parse::<Type>()?;
            input.parse::<Token![;]>()?;
            match label.to_string().as_str() {
                "Params" => {
                    if params.is_some() {
                        return Err(input.error("duplicate Params declaration"));
                    }
                    params = Some(ty);
                }
                "Return" => {
                    if returns.is_some() {
                        return Err(input.error("duplicate Return declaration"));
                    }
                    returns = Some(ty);
                }
                _ => return Err(input.error(format!("unknown command body label `{label}`"))),
            }
        }
        Ok(Self {
            params: params.ok_or_else(|| input.error("missing Params declaration"))?,
            returns,
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

fn fixed_type_width(
    ty: &Type,
    structs: &BTreeMap<String, &ItemStruct>,
    target: ComplianceTarget,
    path: &Path,
    visiting: &mut BTreeSet<String>,
) -> Result<u32, String> {
    match ty {
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(0),
        Type::Tuple(_) => Err(format!(
            "{}: non-unit tuple is not a supported standard command wire type",
            path.display()
        )),
        Type::Array(array) => {
            let element = fixed_type_width(&array.elem, structs, target, path, visiting)?;
            let Expr::Lit(ExprLit {
                lit: Lit::Int(length),
                ..
            }) = &array.len
            else {
                return Err(format!(
                    "{}: standard command array length is not an integer literal",
                    path.display()
                ));
            };
            element
                .checked_mul(length.base10_parse::<u32>().map_err(|error| {
                    format!(
                        "{}: invalid standard command array length: {error}",
                        path.display()
                    )
                })?)
                .ok_or_else(|| {
                    format!(
                        "{}: standard command array width overflows u32",
                        path.display()
                    )
                })
        }
        Type::Paren(paren) => fixed_type_width(&paren.elem, structs, target, path, visiting),
        Type::Group(group) => fixed_type_width(&group.elem, structs, target, path, visiting),
        Type::Path(type_path) if type_path.qself.is_none() => {
            let Some(segment) = type_path.path.segments.last() else {
                return Err(format!(
                    "{}: empty standard command type path",
                    path.display()
                ));
            };
            let name = segment.ident.to_string();
            if let Some(width) = primitive_width(&name) {
                return Ok(width);
            }
            let item = structs.get(&name).ok_or_else(|| {
                format!(
                    "{}: standard command wire type `{name}` has no active packed struct declaration",
                    path.display()
                )
            })?;
            if !has_repr(&item.attrs, "C") || !has_repr(&item.attrs, "packed") {
                return Err(format!(
                    "{}: standard command wire type `{name}` is not #[repr(C, packed)]",
                    path.display()
                ));
            }
            if !visiting.insert(name.clone()) {
                return Err(format!(
                    "{}: recursive standard command wire type `{name}`",
                    path.display()
                ));
            }
            let mut width = 0_u32;
            for field in &item.fields {
                if attrs_active(&field.attrs, target, path)? {
                    width = width
                        .checked_add(fixed_type_width(
                            &field.ty, structs, target, path, visiting,
                        )?)
                        .ok_or_else(|| {
                            format!(
                                "{}: standard command struct width overflows u32",
                                path.display()
                            )
                        })?;
                }
            }
            visiting.remove(&name);
            Ok(width)
        }
        _ => Err(format!(
            "{}: unsupported standard command wire type",
            path.display()
        )),
    }
}

fn primitive_width(name: &str) -> Option<u32> {
    match name {
        "u8" | "i8" => Some(1),
        "u16" | "i16" => Some(2),
        "u32" | "i32" | "f32" => Some(4),
        "u64" | "i64" | "f64" => Some(8),
        "u128" | "i128" => Some(16),
        _ => None,
    }
}

fn has_repr(attributes: &[Attribute], representation: &str) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("repr") {
            return false;
        }
        let mut found = false;
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident(representation) {
                found = true;
            }
            Ok(())
        });
        found
    })
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

    #[test]
    fn loads_local_standard_command_wire_layouts() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci");
        let declarations = load_local_standard_commands(
            &crate_dir,
            ComplianceTarget::new(
                crate::FirmwareVersion::new(1, 24, 0),
                crate::McuFamily::Wb5x,
                crate::StackProfile::FullExtended,
            ),
        )
        .unwrap();

        assert_eq!(declarations.len(), 10);
        let transmitter = declarations
            .iter()
            .find(|declaration| declaration.name == "LeTransmitterTest")
            .unwrap();
        assert_eq!(transmitter.code, 0x201E);
        assert_eq!(transmitter.request.envelope(), crate::Envelope::fixed(3));
        assert!(matches!(
            transmitter.completion,
            CommandCompletion::CommandComplete { ref returns }
                if returns.envelope() == crate::Envelope::fixed(0)
        ));

        let asynchronous = declarations
            .iter()
            .find(|declaration| declaration.name == "LeReadLocalP256PublicKey")
            .unwrap();
        assert_eq!(asynchronous.request.envelope(), crate::Envelope::fixed(0));
        assert_eq!(asynchronous.completion, CommandCompletion::CommandStatus);

        let address = declarations
            .iter()
            .find(|declaration| declaration.name == "LeReadPeerResolvableAddress")
            .unwrap();
        assert_eq!(address.request.envelope(), crate::Envelope::fixed(7));
        assert!(matches!(
            address.completion,
            CommandCompletion::CommandComplete { ref returns }
                if returns.envelope() == crate::Envelope::fixed(6)
        ));

        let timeout = declarations
            .iter()
            .find(|declaration| declaration.name == "LeSetResolvablePrivateAddressTimeoutV2")
            .unwrap();
        assert_eq!(timeout.request.envelope(), crate::Envelope::fixed(4));
    }
}
