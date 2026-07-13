//! Feature-aware extraction of the crate's vendor command and event surface.
//!
//! The checker deliberately works from the Rust syntax tree rather than source
//! text. Command, event, and module cfgs are evaluated structurally for the
//! selected firmware.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, File, Item, ItemMacro, ItemMod, Lit, LitInt, Meta, Path as SynPath, Type,
};

use crate::CompletionExpectation;
use crate::FirmwareVersion;
use crate::envelope::WireEnvelope;
use crate::model::{CoverageEntry, CoverageOrigin, ProtocolCoverage};

pub(crate) struct CrateCoverage {
    pub(crate) descriptors: ProtocolCoverage,
    pub(crate) active_api: ProtocolCoverage,
    /// Command-envelope metadata declared by the active `vendor_cmd!` catalog.
    pub(crate) descriptor_metadata: BTreeMap<String, DescriptorMetadata>,
    /// Payload envelopes declared by the active `vendor_event!` catalog.
    pub(crate) event_metadata: BTreeMap<u16, EventMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DescriptorMetadata {
    pub(crate) name: String,
    pub(crate) code: u16,
    pub(crate) completion: CompletionExpectation,
    /// Command parameter bytes, excluding the HCI command header.
    pub(crate) request: WireEnvelope,
    /// Return bytes owned by the command, excluding Command Complete framing
    /// and its status byte. Command Status commands have no return envelope.
    pub(crate) response: Option<WireEnvelope>,
    pub(crate) location: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EventMetadata {
    pub(crate) name: String,
    pub(crate) code: u16,
    /// Vendor event payload bytes, excluding the two-byte vendor event code.
    pub(crate) payload: WireEnvelope,
    pub(crate) location: PathBuf,
}

#[derive(Clone, Debug)]
struct DescriptorDefinition {
    name: String,
    code: u16,
    completion: CompletionExpectation,
    request: WireEnvelope,
    response: Option<WireEnvelope>,
}

#[derive(Clone)]
struct SourceUnit {
    path: PathBuf,
    active: bool,
    file: File,
}

/// Load the declarative vendor command and event catalogs for one selected
/// firmware feature.
pub(crate) fn load_crate_coverage(
    crate_dir: &Path,
    firmware: FirmwareVersion,
) -> Result<CrateCoverage, String> {
    let command_root = crate_dir.join("src/vendor/command/mod.rs");
    let command_root_file = read_rust_file(&command_root)?;
    let mut command_sources = Vec::new();
    let mut visited = BTreeSet::new();
    collect_command_sources(
        command_root,
        command_root_file,
        true,
        firmware,
        &mut visited,
        &mut command_sources,
    )?;

    let descriptors = collect_descriptors(&command_sources, firmware)?;
    let active_commands = descriptor_coverage(&descriptors);

    let event_path = crate_dir.join("src/vendor/event/mod.rs");
    let event_file = read_rust_file(&event_path)?;
    let (active_events, event_metadata) =
        parse_vendor_event_declarations(&event_file, firmware, &event_path)?;

    let mut descriptor_coverage = ProtocolCoverage::default();
    let mut descriptor_metadata = BTreeMap::new();
    for descriptor in descriptors.values() {
        descriptor_coverage.commands.push(
            CoverageEntry::new(
                descriptor.code,
                &descriptor.name,
                CoverageOrigin::VendorCommandDescriptor,
            )
            .at(descriptor.location.clone()),
        );
        descriptor_metadata.insert(descriptor.name.clone(), descriptor.clone());
    }

    descriptor_coverage
        .commands
        .sort_by_key(|entry| (entry.code, entry.name.clone()));

    Ok(CrateCoverage {
        descriptors: descriptor_coverage,
        active_api: ProtocolCoverage {
            commands: active_commands,
            events: active_events,
        },
        descriptor_metadata,
        event_metadata,
    })
}

fn descriptor_coverage(descriptors: &BTreeMap<String, DescriptorMetadata>) -> Vec<CoverageEntry> {
    let mut commands = descriptors
        .values()
        .map(|descriptor| {
            CoverageEntry::new(
                descriptor.code,
                &descriptor.name,
                CoverageOrigin::VendorCommandDescriptor,
            )
            .at(descriptor.location.clone())
        })
        .collect::<Vec<_>>();
    commands.sort_by_key(|entry| (entry.code, entry.name.clone()));
    commands
}

fn read_rust_file(path: &Path) -> Result<File, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    syn::parse_file(&source)
        .map_err(|error| format!("could not parse Rust source {}: {error}", path.display()))
}

/// Discover command modules from `src/vendor/command/mod.rs` rather than
/// maintaining a hand-written group list. This keeps a future, cfg-gated module
/// out of old firmware inventories and automatically brings it in when its cfg
/// becomes active.
fn collect_command_sources(
    path: PathBuf,
    file: File,
    inherited_active: bool,
    firmware: FirmwareVersion,
    visited: &mut BTreeSet<PathBuf>,
    sources: &mut Vec<SourceUnit>,
) -> Result<(), String> {
    let active = inherited_active && attrs_active(&file.attrs, firmware, &path)?;
    if !active {
        return Ok(());
    }

    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    if !visited.insert(canonical) {
        return Ok(());
    }

    // Descend before moving `file` into the source collection. Inline modules
    // are represented as their own source unit so direct-item scans never need
    // to recursively re-scan a parent module.
    for item in &file.items {
        let Item::Mod(module) = item else {
            continue;
        };
        let module_active = active && attrs_active(&module.attrs, firmware, &path)?;
        if !module_active {
            continue;
        }

        if let Some((_, items)) = &module.content {
            let inline_path = path.join(format!("<{}>", module.ident));
            collect_command_sources(
                inline_path,
                File {
                    shebang: None,
                    attrs: Vec::new(),
                    items: items.clone(),
                },
                module_active,
                firmware,
                visited,
                sources,
            )?;
        } else {
            let module_path = external_module_path(&path, module)?;
            let module_file = read_rust_file(&module_path)?;
            collect_command_sources(
                module_path,
                module_file,
                module_active,
                firmware,
                visited,
                sources,
            )?;
        }
    }

    sources.push(SourceUnit { path, active, file });
    Ok(())
}

fn external_module_path(parent_path: &Path, module: &ItemMod) -> Result<PathBuf, String> {
    if let Some(path) = module_path_override(module)? {
        return Ok(parent_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path));
    }

    let parent_dir = parent_path.parent().unwrap_or_else(|| Path::new("."));
    let child_dir = if parent_path.file_name().is_some_and(|name| name == "mod.rs") {
        parent_dir.to_path_buf()
    } else {
        parent_dir.join(
            parent_path
                .file_stem()
                .ok_or_else(|| format!("{} has no file stem", parent_path.display()))?,
        )
    };
    let flat = child_dir.join(format!("{}.rs", module.ident));
    let nested = child_dir.join(module.ident.to_string()).join("mod.rs");

    match (flat.is_file(), nested.is_file()) {
        (true, false) => Ok(flat),
        (false, true) => Ok(nested),
        (false, false) => Err(format!(
            "{}: active module `{}` has no source file (looked for {} or {})",
            parent_path.display(),
            module.ident,
            flat.display(),
            nested.display(),
        )),
        (true, true) => Err(format!(
            "{}: active module `{}` is ambiguous: both {} and {} exist",
            parent_path.display(),
            module.ident,
            flat.display(),
            nested.display(),
        )),
    }
}

fn module_path_override(module: &ItemMod) -> Result<Option<String>, String> {
    let Some(attribute) = module
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("path"))
    else {
        return Ok(None);
    };
    let Meta::NameValue(value) = &attribute.meta else {
        return Err(format!(
            "module `{}`: unsupported #[path] attribute shape",
            module.ident
        ));
    };
    let Expr::Lit(literal) = &value.value else {
        return Err(format!(
            "module `{}`: #[path] must contain a string literal",
            module.ident
        ));
    };
    let Lit::Str(path) = &literal.lit else {
        return Err(format!(
            "module `{}`: #[path] must contain a string literal",
            module.ident
        ));
    };
    Ok(Some(path.value()))
}

fn collect_descriptors(
    sources: &[SourceUnit],
    firmware: FirmwareVersion,
) -> Result<BTreeMap<String, DescriptorMetadata>, String> {
    let mut descriptors = BTreeMap::<String, DescriptorMetadata>::new();
    let mut codes = BTreeMap::<u16, DescriptorMetadata>::new();

    for source in sources {
        if !source.active {
            continue;
        }
        for item in &source.file.items {
            let Item::Macro(item) = item else {
                continue;
            };
            if !is_macro_named(&item.mac.path, "vendor_cmd")
                || !attrs_active(&item.attrs, firmware, &source.path)?
            {
                continue;
            }

            let definition = parse_vendor_descriptor(item, &source.path)?;
            let metadata = DescriptorMetadata {
                name: definition.name.clone(),
                code: definition.code,
                completion: definition.completion,
                request: definition.request,
                response: definition.response,
                location: source.path.clone(),
            };

            if let Some(previous) = descriptors.get(&definition.name) {
                return Err(format!(
                    "{}: descriptor `{}` is active more than once (also declared in {})",
                    source.path.display(),
                    definition.name,
                    previous.location.display()
                ));
            }
            if let Some(previous) = codes.insert(definition.code, metadata.clone()) {
                return Err(format!(
                    "{}: descriptors `{}` and `{}` both declare active vendor OCF 0x{:03X}",
                    source.path.display(),
                    previous.name,
                    definition.name,
                    definition.code,
                ));
            }
            descriptors.insert(definition.name, metadata);
        }
    }

    if descriptors.is_empty() {
        return Err("no active vendor_cmd! command descriptors were found".to_owned());
    }
    Ok(descriptors)
}

fn parse_vendor_descriptor(item: &ItemMacro, path: &Path) -> Result<DescriptorDefinition, String> {
    syn::parse2::<VendorCommandInvocation>(item.mac.tokens.clone())
        .map_err(|error| {
            format!(
                "{}: unsupported vendor_cmd! declaration: {error}",
                path.display()
            )
        })
        .and_then(|definition| parse_descriptor_definition(definition, path))
}

fn parse_descriptor_definition(
    invocation: VendorCommandInvocation,
    path: &Path,
) -> Result<DescriptorDefinition, String> {
    let mut input = invocation.body.into_iter().peekable();
    let mut saw_params = false;
    let mut request = None;
    let mut saw_return = false;
    let mut response = None;
    let mut completion = None;
    let mut param_names = BTreeSet::new();
    let mut constraint_names = None;

    while input.peek().is_some() {
        let Some(TokenTree::Ident(label)) = input.next() else {
            return Err(format!(
                "{}: expected a field name in vendor_cmd! body for `{}`",
                path.display(),
                invocation.name
            ));
        };

        // `Params<'a> = ...` has generic tokens between the label and `=`.
        // Groups are a single token tree, so an `=` inside a type body cannot
        // accidentally terminate this scan.
        let mut found_equals = false;
        for token in input.by_ref() {
            if matches!(&token, TokenTree::Punct(punctuation) if punctuation.as_char() == '=') {
                found_equals = true;
                break;
            }
        }
        if !found_equals {
            return Err(format!(
                "{}: field `{label}` in vendor_cmd! `{}` has no `=`",
                path.display(),
                invocation.name
            ));
        }

        let mut value = TokenStream::new();
        let mut terminated = false;
        for token in input.by_ref() {
            if matches!(&token, TokenTree::Punct(punctuation) if punctuation.as_char() == ';') {
                terminated = true;
                break;
            }
            value.extend([token]);
        }
        if !terminated {
            return Err(format!(
                "{}: field `{label}` in vendor_cmd! `{}` is missing `;`",
                path.display(),
                invocation.name
            ));
        }

        if label == "Params" {
            if saw_params {
                return Err(format!(
                    "{}: vendor_cmd! `{}` declares `Params` more than once",
                    path.display(),
                    invocation.name
                ));
            }
            saw_params = true;
            let params = parse_params_shape(value, &invocation.name, path)?;
            request = Some(params.envelope);
            param_names = params.names;
        } else if label == "Return" {
            if saw_return {
                return Err(format!(
                    "{}: vendor_cmd! `{}` declares `Return` more than once",
                    path.display(),
                    invocation.name
                ));
            }
            saw_return = true;
            response = Some(parse_return_shape(value, &invocation.name, path)?);
        } else if label == "Completion" {
            if completion.is_some() {
                return Err(format!(
                    "{}: vendor_cmd! `{}` declares `Completion` more than once",
                    path.display(),
                    invocation.name
                ));
            }
            completion = Some(parse_completion_shape(value, &invocation.name, path)?);
        } else if label == "Constraints" {
            if constraint_names.is_some() {
                return Err(format!(
                    "{}: vendor_cmd! `{}` declares `Constraints` more than once",
                    path.display(),
                    invocation.name
                ));
            }
            constraint_names = Some(parse_constraints_shape(value, &invocation.name, path)?);
        } else {
            return Err(format!(
                "{}: vendor_cmd! `{}` contains unknown declaration `{label}`",
                path.display(),
                invocation.name
            ));
        }
    }

    if !saw_params {
        return Err(format!(
            "{}: vendor_cmd! `{}` is missing a `Params = ...` declaration",
            path.display(),
            invocation.name
        ));
    }

    if let Some(constraint_names) = constraint_names {
        let unknown = constraint_names
            .difference(&param_names)
            .cloned()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(format!(
                "{}: vendor_cmd! `{}` constraints reference unknown parameter(s): {}",
                path.display(),
                invocation.name,
                unknown.join(", ")
            ));
        }
    }

    let completion = completion.ok_or_else(|| {
        format!(
            "{}: vendor_cmd! `{}` is missing a `Completion = ...` declaration",
            path.display(),
            invocation.name
        )
    })?;
    let response = match &completion {
        CompletionExpectation::CommandComplete if saw_return => response,
        CompletionExpectation::CommandComplete => {
            return Err(format!(
                "{}: vendor_cmd! `{}` declares CommandComplete but has no Return declaration",
                path.display(),
                invocation.name
            ));
        }
        CompletionExpectation::CommandStatus if saw_return => {
            return Err(format!(
                "{}: vendor_cmd! `{}` declares CommandStatus and must not declare Return",
                path.display(),
                invocation.name
            ));
        }
        CompletionExpectation::CommandStatus => None,
        CompletionExpectation::Event(_) | CompletionExpectation::Unresolved(_) => {
            unreachable!("vendor_cmd! parser accepts only CommandComplete or CommandStatus")
        }
    };

    Ok(DescriptorDefinition {
        name: invocation.name.to_string(),
        code: (invocation.cgid << 7) | invocation.cid,
        completion,
        request: request.expect("presence checked above"),
        response,
    })
}

struct ParsedParamsShape {
    envelope: WireEnvelope,
    names: BTreeSet<String>,
}

fn parse_params_shape(
    value: TokenStream,
    descriptor: &syn::Ident,
    path: &Path,
) -> Result<ParsedParamsShape, String> {
    let mut tokens = value.clone().into_iter();
    if let (Some(TokenTree::Group(group)), None) = (tokens.next(), tokens.next())
        && group.delimiter() == Delimiter::Brace
    {
        let fields = syn::parse2::<DeclarativeFields>(group.stream()).map_err(|error| {
            format!(
                "{}: descriptor `{descriptor}` has an unsupported declarative Params shape: {error}",
                path.display()
            )
        })?;
        if fields.contains_payload_field {
            return Err(format!(
                "{}: descriptor `{descriptor}` uses removed `kind: payload` in Params; inline the wire schema instead",
                path.display()
            ));
        }
        if fields.min_len > usize::from(u8::MAX) {
            return Err(format!(
                "{}: descriptor `{descriptor}` has a {}-byte minimum Params envelope, exceeding the HCI 255-byte parameter limit",
                path.display(),
                fields.min_len,
            ));
        }
        return Ok(ParsedParamsShape {
            envelope: if fields.min_len == fields.total_len {
                WireEnvelope::fixed(fields.total_len)
            } else {
                // `vendor_cmd!` rejects an aggregate encoded request above the
                // one-byte HCI parameter-length limit, even when the sum of
                // independent field capacities is larger.
                WireEnvelope::bounded(fields.min_len, fields.total_len.min(usize::from(u8::MAX)))
            },
            names: fields.names,
        });
    }

    let ty = syn::parse2::<Type>(value).map_err(|error| {
        format!(
            "{}: descriptor `{descriptor}` has an unsupported Params shape: {error}",
            path.display()
        )
    })?;
    Ok(ParsedParamsShape {
        envelope: match ty {
            Type::Tuple(tuple) if tuple.elems.is_empty() => WireEnvelope::fixed(0),
            _ => {
                return Err(format!(
                    "{}: descriptor `{descriptor}` has an unsupported Params shape; expected `()` or an inline named field body",
                    path.display()
                ));
            }
        },
        names: BTreeSet::new(),
    })
}

fn parse_constraints_shape(
    value: TokenStream,
    descriptor: &syn::Ident,
    path: &Path,
) -> Result<BTreeSet<String>, String> {
    let mut tokens = value.into_iter();
    let Some(TokenTree::Group(group)) = tokens.next() else {
        return Err(format!(
            "{}: descriptor `{descriptor}` Constraints must be a declarative field body",
            path.display()
        ));
    };
    if tokens.next().is_some() || group.delimiter() != Delimiter::Brace {
        return Err(format!(
            "{}: descriptor `{descriptor}` Constraints must be a declarative field body",
            path.display()
        ));
    }
    syn::parse2::<DeclarativeConstraints>(group.stream())
        .map(|constraints| constraints.fields)
        .map_err(|error| {
            format!(
                "{}: descriptor `{descriptor}` has unsupported declarative Constraints: {error}",
                path.display()
            )
        })
}

fn parse_return_shape(
    value: TokenStream,
    descriptor: &syn::Ident,
    path: &Path,
) -> Result<WireEnvelope, String> {
    if let Ok(shape) = syn::parse2::<DeclarativeReturn>(value.clone()) {
        if shape.fields.contains_payload_field {
            return Err(format!(
                "{}: descriptor `{descriptor}` uses removed `kind: payload` in Return; inline the wire schema instead",
                path.display()
            ));
        }
        return Ok(shape.fields.envelope());
    }

    let ty = syn::parse2::<Type>(value).map_err(|error| {
        format!(
            "{}: descriptor `{descriptor}` has an unsupported Return shape: {error}",
            path.display()
        )
    })?;
    match ty {
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(WireEnvelope::fixed(0)),
        _ => Err(format!(
            "{}: descriptor `{descriptor}` has an unsupported Return shape; expected `()` or an inline named field body",
            path.display()
        )),
    }
}

fn parse_completion_shape(
    value: TokenStream,
    descriptor: &syn::Ident,
    path: &Path,
) -> Result<CompletionExpectation, String> {
    let completion = syn::parse2::<syn::Ident>(value).map_err(|error| {
        format!(
            "{}: descriptor `{descriptor}` has an unsupported Completion shape: {error}",
            path.display()
        )
    })?;
    if completion == "CommandComplete" {
        Ok(CompletionExpectation::CommandComplete)
    } else if completion == "CommandStatus" {
        Ok(CompletionExpectation::CommandStatus)
    } else {
        Err(format!(
            "{}: descriptor `{descriptor}` has unknown Completion `{completion}`",
            path.display()
        ))
    }
}

struct DeclarativeConstraints {
    fields: BTreeSet<String>,
}

impl Parse for DeclarativeConstraints {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut fields = BTreeSet::new();
        while !input.is_empty() {
            let kind = input.parse::<syn::Ident>()?;
            let arguments;
            syn::parenthesized!(arguments in input);

            if kind == "ordered" || kind == "len_at_most" {
                fields.insert(arguments.parse::<syn::Ident>()?.to_string());
                arguments.parse::<syn::Token![,]>()?;
                fields.insert(arguments.parse::<syn::Ident>()?.to_string());
            } else if kind == "range" {
                fields.insert(arguments.parse::<syn::Ident>()?.to_string());
                arguments.parse::<syn::Token![,]>()?;
                arguments.parse::<Expr>()?;
                arguments.parse::<syn::Token![,]>()?;
                arguments.parse::<Expr>()?;
            } else if kind == "one_of" {
                fields.insert(arguments.parse::<syn::Ident>()?.to_string());
                arguments.parse::<syn::Token![,]>()?;
                let allowed;
                syn::bracketed!(allowed in arguments);
                let values = Punctuated::<Expr, syn::Token![,]>::parse_terminated(&allowed)?;
                if values.is_empty() {
                    return Err(allowed.error("one_of must declare at least one allowed value"));
                }
            } else if kind == "non_empty" {
                fields.insert(arguments.parse::<syn::Ident>()?.to_string());
            } else {
                return Err(syn::Error::new_spanned(
                    kind,
                    "unknown declarative constraint",
                ));
            }

            if !arguments.is_empty() {
                return Err(arguments.error("unexpected tokens in declarative constraint"));
            }
            input.parse::<syn::Token![;]>()?;
        }
        Ok(Self { fields })
    }
}

struct DeclarativeFields {
    names: BTreeSet<String>,
    min_len: usize,
    total_len: usize,
    contains_payload_field: bool,
}

impl DeclarativeFields {
    fn envelope(&self) -> WireEnvelope {
        if self.min_len == self.total_len {
            WireEnvelope::fixed(self.total_len)
        } else {
            WireEnvelope::bounded(self.min_len, self.total_len)
        }
    }
}

impl Parse for DeclarativeFields {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut names = BTreeSet::new();
        let mut min_len = 0usize;
        let mut total_len = 0usize;
        let mut consumes_remainder = false;
        let mut contains_payload_field = false;
        while !input.is_empty() {
            if consumes_remainder {
                return Err(input.error("trailing_bytes must be the final declarative field"));
            }
            let name = input.parse::<syn::Ident>()?;
            if !names.insert(name.to_string()) {
                return Err(syn::Error::new_spanned(name, "duplicate declarative field"));
            }
            input.parse::<syn::Token![:]>()?;
            input.parse::<Type>()?;
            input.parse::<syn::Token![=>]>()?;
            let (minimum, width, payload_field, field_consumes_remainder) = if input.peek(LitInt) {
                let width = input.parse::<LitInt>()?;
                let width = parse_usize_literal(&width).map_err(|error| input.error(error))?;
                (width, width, false, false)
            } else if input.peek(syn::token::Brace) {
                let shape;
                syn::braced!(shape in input);
                let shape = shape.parse::<DeclarativeVariableShape>()?;
                (
                    shape.min_len,
                    shape.max_len,
                    shape.payload_field,
                    shape.consumes_remainder,
                )
            } else {
                return Err(input.error("expected a fixed width or variable field shape"));
            };
            consumes_remainder = field_consumes_remainder;
            contains_payload_field |= payload_field;
            total_len = total_len
                .checked_add(width)
                .ok_or_else(|| input.error("declarative field length overflows usize"))?;
            min_len = min_len
                .checked_add(minimum)
                .ok_or_else(|| input.error("declarative field minimum length overflows usize"))?;
            input.parse::<syn::Token![,]>()?;
        }
        Ok(Self {
            names,
            min_len,
            total_len,
            contains_payload_field,
        })
    }
}

struct DeclarativeVariableShape {
    min_len: usize,
    max_len: usize,
    payload_field: bool,
    consumes_remainder: bool,
}

impl Parse for DeclarativeVariableShape {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        parse_declarative_label(input, "kind")?;
        let kind = input.parse::<syn::Ident>()?;
        input.parse::<syn::Token![,]>()?;

        let mut payload_field = false;
        let mut consumes_remainder = false;
        let (min_len, max_len) = if kind == "counted_bytes" {
            let count_len = parse_declarative_type_width(input, "count")?;
            let max_value = parse_declarative_integer(input, "max_len")?;
            (Some(count_len), count_len.checked_add(max_value))
        } else if kind == "counted_items" {
            let count_len = parse_declarative_type_width(input, "count")?;
            let item_len = parse_declarative_type_width(input, "item")?;
            let max_items = parse_declarative_integer(input, "max_items")?;
            (
                Some(count_len),
                item_len
                    .checked_mul(max_items)
                    .and_then(|items| count_len.checked_add(items)),
            )
        } else if kind == "tagged" {
            parse_declarative_tagged_shape(input)?
        } else if kind == "payload" {
            let min_len = parse_declarative_integer(input, "min_len")?;
            let max_len = parse_declarative_integer(input, "max_len")?;
            if min_len > max_len {
                return Err(input.error(format!(
                    "payload minimum {min_len} exceeds maximum {max_len}"
                )));
            }
            payload_field = true;
            (Some(min_len), Some(max_len))
        } else if kind == "trailing_bytes" {
            let min_len = parse_declarative_integer(input, "min_len")?;
            let max_len = parse_declarative_integer(input, "max_len")?;
            if min_len > max_len {
                return Err(input.error(format!(
                    "trailing_bytes minimum {min_len} exceeds maximum {max_len}"
                )));
            }
            consumes_remainder = true;
            (Some(min_len), Some(max_len))
        } else if kind == "bitmap_items" {
            parse_declarative_label(input, "bitmap")?;
            input.parse::<syn::Ident>()?;
            input.parse::<syn::Token![,]>()?;
            let mask = parse_declarative_integer(input, "mask")?;
            let item_len = parse_declarative_type_width(input, "item")?;
            let max_items = parse_declarative_integer(input, "max_items")?;
            if mask.count_ones() as usize != max_items {
                return Err(input.error(format!(
                    "bitmap mask selects {} bits but max_items is {max_items}",
                    mask.count_ones()
                )));
            }
            (Some(0), item_len.checked_mul(max_items))
        } else {
            return Err(input.error(format!("unknown declarative variable kind `{kind}`")));
        };
        let min_len = min_len
            .ok_or_else(|| input.error("declarative variable minimum length overflows usize"))?;
        let max_len = max_len
            .ok_or_else(|| input.error("declarative variable field length overflows usize"))?;

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after declarative variable field"));
        }
        Ok(Self {
            min_len,
            max_len,
            payload_field,
            consumes_remainder,
        })
    }
}

fn parse_declarative_tagged_shape(
    input: ParseStream<'_>,
) -> syn::Result<(Option<usize>, Option<usize>)> {
    let tag_len = parse_declarative_type_width(input, "tag")?;
    parse_declarative_label(input, "variants")?;
    let variants;
    syn::braced!(variants in input);
    input.parse::<syn::Token![,]>()?;

    let mut tags = BTreeSet::new();
    let mut variant_min = None::<usize>;
    let mut variant_max = None::<usize>;
    while !variants.is_empty() {
        let pattern = variants.call(syn::Pat::parse_single)?;
        let mut bindings = PatternBindings::default();
        bindings.visit_pat(&pattern);
        variants.parse::<syn::Token![=>]>()?;
        let body;
        syn::braced!(body in variants);

        let tag = parse_declarative_integer(&body, "tag")?;
        if !tags.insert(tag) {
            return Err(variants.error(format!("duplicate tagged variant value {tag:#x}")));
        }
        if tag_len < core::mem::size_of::<usize>()
            && tag >= (1usize << (tag_len * u8::BITS as usize))
        {
            return Err(variants.error(format!(
                "tag value {tag:#x} does not fit in {tag_len} bytes"
            )));
        }

        parse_declarative_label(&body, "fields")?;
        let fields;
        syn::braced!(fields in body);
        body.parse::<syn::Token![,]>()?;
        if !body.is_empty() {
            return Err(body.error("unexpected tokens after tagged variant fields"));
        }
        let payload = parse_declarative_fixed_fields(&fields)?;
        for field in &payload.names {
            if !bindings.names.contains(field) {
                return Err(fields.error(format!(
                    "tagged payload field `{field}` is not bound by its variant pattern"
                )));
            }
        }
        let wire_len = tag_len
            .checked_add(payload.len)
            .ok_or_else(|| variants.error("tagged variant length overflows usize"))?;
        variant_min = Some(variant_min.map_or(wire_len, |value| value.min(wire_len)));
        variant_max = Some(variant_max.map_or(wire_len, |value| value.max(wire_len)));
        variants.parse::<syn::Token![,]>()?;
    }

    let Some(computed_min) = variant_min else {
        return Err(input.error("tagged field must declare at least one variant"));
    };
    let computed_max = variant_max.expect("minimum and maximum are populated together");
    let declared_min = parse_declarative_integer(input, "min_len")?;
    let declared_max = parse_declarative_integer(input, "max_len")?;
    if (declared_min, declared_max) != (computed_min, computed_max) {
        return Err(input.error(format!(
            "tagged field declares lengths {declared_min}..={declared_max}, but its variants require {computed_min}..={computed_max}"
        )));
    }
    Ok((Some(computed_min), Some(computed_max)))
}

#[derive(Default)]
struct PatternBindings {
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for PatternBindings {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.names.insert(pattern.ident.to_string());
        visit::visit_pat_ident(self, pattern);
    }
}

struct DeclarativeFixedFields {
    len: usize,
    names: Vec<String>,
}

fn parse_declarative_fixed_fields(input: ParseStream<'_>) -> syn::Result<DeclarativeFixedFields> {
    let mut total = 0usize;
    let mut names = Vec::new();
    while !input.is_empty() {
        names.push(input.parse::<syn::Ident>()?.to_string());
        input.parse::<syn::Token![:]>()?;
        input.parse::<Type>()?;
        input.parse::<syn::Token![=>]>()?;
        let width = input.parse::<LitInt>()?;
        let width = parse_usize_literal(&width).map_err(|error| input.error(error))?;
        total = total
            .checked_add(width)
            .ok_or_else(|| input.error("tagged variant field length overflows usize"))?;
        input.parse::<syn::Token![,]>()?;
    }
    Ok(DeclarativeFixedFields { len: total, names })
}

fn parse_declarative_label(input: ParseStream<'_>, expected: &str) -> syn::Result<()> {
    let label = input.parse::<syn::Ident>()?;
    if label != expected {
        return Err(input.error(format!("expected `{expected}`, found `{label}`")));
    }
    input.parse::<syn::Token![:]>().map(|_| ())
}

fn parse_declarative_type_width(input: ParseStream<'_>, label: &str) -> syn::Result<usize> {
    parse_declarative_label(input, label)?;
    input.parse::<Type>()?;
    input.parse::<syn::Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    input.parse::<syn::Token![,]>()?;
    parse_usize_literal(&width).map_err(|error| input.error(error))
}

fn parse_declarative_integer(input: ParseStream<'_>, label: &str) -> syn::Result<usize> {
    parse_declarative_label(input, label)?;
    let value = input.parse::<LitInt>()?;
    input.parse::<syn::Token![,]>()?;
    parse_usize_literal(&value).map_err(|error| input.error(error))
}

struct DeclarativeReturn {
    fields: DeclarativeFields,
}

impl Parse for DeclarativeReturn {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        input.parse::<SynPath>()?;
        let content;
        syn::braced!(content in input);
        let fields = content.parse::<DeclarativeFields>()?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after declarative Return body"));
        }
        Ok(Self { fields })
    }
}

fn parse_vendor_event_declarations(
    file: &File,
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<(Vec<CoverageEntry>, BTreeMap<u16, EventMetadata>), String> {
    if !attrs_active(&file.attrs, firmware, path)? {
        return Err(format!(
            "{}: VendorEvent source is disabled for selected firmware {firmware}",
            path.display()
        ));
    }

    let mut macros = Vec::new();
    collect_vendor_event_macros(&file.items, firmware, path, &mut macros)?;
    let [item] = macros.as_slice() else {
        return Err(format!(
            "{}: found {} active `vendor_event!` catalogs; expected exactly one",
            path.display(),
            macros.len()
        ));
    };
    let invocation =
        syn::parse2::<VendorEventsInvocation>(item.mac.tokens.clone()).map_err(|error| {
            format!(
                "{}: unsupported vendor_event! declaration: {error}",
                path.display()
            )
        })?;

    let mut events = Vec::new();
    let mut metadata = BTreeMap::new();
    let mut names = BTreeSet::new();
    for definition in invocation.events {
        if !attrs_active(&definition.attrs, firmware, path)? {
            continue;
        }
        if definition.payload.total_len > 253 {
            return Err(format!(
                "{}: event `{}` declares a maximum {}-byte payload; vendor event payloads cannot exceed 253 bytes after the two-byte event code",
                path.display(),
                definition.name,
                definition.payload.total_len
            ));
        }
        if !names.insert(definition.name.to_string()) {
            return Err(format!(
                "{}: event `{}` is active more than once",
                path.display(),
                definition.name
            ));
        }
        let event = EventMetadata {
            name: definition.name.to_string(),
            code: definition.code,
            payload: definition.payload.envelope(),
            location: path.to_path_buf(),
        };
        if let Some(previous) = metadata.insert(event.code, event.clone()) {
            return Err(format!(
                "{}: events `{}` and `{}` both declare active code 0x{:04X}",
                path.display(),
                previous.name,
                event.name,
                event.code
            ));
        }
        events.push(
            CoverageEntry::new(event.code, &event.name, CoverageOrigin::VendorEventDispatch)
                .at(path.to_path_buf()),
        );
    }
    if events.is_empty() {
        return Err(format!(
            "{}: vendor_event! has no active declarations for firmware {firmware}",
            path.display()
        ));
    }
    events.sort_by_key(|entry| (entry.code, entry.name.clone()));
    Ok((events, metadata))
}

fn collect_vendor_event_macros<'ast>(
    items: &'ast [Item],
    firmware: FirmwareVersion,
    path: &Path,
    macros: &mut Vec<&'ast ItemMacro>,
) -> Result<(), String> {
    for item in items {
        if !item_is_active(item, firmware, path)? {
            continue;
        }
        match item {
            Item::Macro(item) if is_macro_named(&item.mac.path, "vendor_event") => {
                macros.push(item);
            }
            Item::Mod(module) if module.content.is_some() => {
                let (_, nested) = module.content.as_ref().expect("checked above");
                collect_vendor_event_macros(nested, firmware, path, macros)?;
            }
            _ => {}
        }
    }
    Ok(())
}

struct VendorEventsInvocation {
    events: Vec<VendorEventDefinition>,
}

struct VendorEventDefinition {
    attrs: Vec<Attribute>,
    name: syn::Ident,
    code: u16,
    payload: DeclarativeFields,
}

impl Parse for VendorEventsInvocation {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut events = Vec::new();
        while !input.is_empty() {
            let attrs = input.call(Attribute::parse_outer)?;
            let name = input.parse::<syn::Ident>()?;
            let arguments;
            syn::parenthesized!(arguments in input);
            let code = arguments.parse::<LitInt>()?;
            let code = parse_u16_literal(&code).map_err(|error| arguments.error(error))?;
            if !arguments.is_empty() {
                return Err(arguments.error("event code must be one integer literal"));
            }

            let body;
            syn::braced!(body in input);
            let payload_label = body.parse::<syn::Ident>()?;
            if payload_label != "Payload" {
                return Err(body.error(format!("expected `Payload`, found `{payload_label}`")));
            }
            body.parse::<syn::Token![=]>()?;
            let payload = if body.peek(syn::token::Brace) {
                let payload;
                syn::braced!(payload in body);
                payload.parse::<DeclarativeFields>()?
            } else if body.peek(syn::token::Paren) {
                let unit;
                syn::parenthesized!(unit in body);
                if !unit.is_empty() {
                    return Err(unit.error("unit event payload must be `()`"));
                }
                DeclarativeFields {
                    names: BTreeSet::new(),
                    min_len: 0,
                    total_len: 0,
                    contains_payload_field: false,
                }
            } else {
                return Err(body.error("event Payload must be `()` or a declarative field body"));
            };
            body.parse::<syn::Token![;]>()?;
            if !body.is_empty() {
                return Err(body.error("unexpected tokens after event Payload"));
            }
            events.push(VendorEventDefinition {
                attrs,
                name,
                code,
                payload,
            });
        }
        Ok(Self { events })
    }
}

fn item_is_active(item: &Item, firmware: FirmwareVersion, path: &Path) -> Result<bool, String> {
    let attributes = match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => return Ok(true),
        _ => return Ok(true),
    };
    attrs_active(attributes, firmware, path)
}

/// Evaluate all `#[cfg]` / `#[cfg_attr]` attributes attached to one syntax
/// node. Unknown predicates are errors rather than false: silently treating an
/// unsupported condition as disabled would turn an incomplete parser into a
/// false compliance success.
fn attrs_active(
    attributes: &[Attribute],
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<bool, String> {
    let mut active = true;
    for attribute in attributes {
        if attribute.path().is_ident("cfg") {
            active &= eval_cfg_attribute(&attribute.meta, firmware, path)?;
        } else if attribute.path().is_ident("cfg_attr") {
            active &= eval_cfg_attr_attribute(&attribute.meta, firmware, path)?;
        }
    }
    Ok(active)
}

fn eval_cfg_attribute(meta: &Meta, firmware: FirmwareVersion, path: &Path) -> Result<bool, String> {
    let Meta::List(list) = meta else {
        return Err(format!("{}: #[cfg] must use parentheses", path.display()));
    };
    let conditions = list
        .parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| {
            format!(
                "{}: could not parse #[cfg(...)] condition: {error}",
                path.display()
            )
        })?;
    let conditions = conditions.into_iter().collect::<Vec<_>>();
    let [condition] = conditions.as_slice() else {
        return Err(format!(
            "{}: #[cfg(...)] requires exactly one condition",
            path.display()
        ));
    };
    eval_cfg_meta(condition, firmware, path)
}

fn eval_cfg_attr_attribute(
    meta: &Meta,
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<bool, String> {
    let Meta::List(list) = meta else {
        return Err(format!(
            "{}: #[cfg_attr] must use parentheses",
            path.display()
        ));
    };
    let values = list
        .parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| {
            format!(
                "{}: could not parse #[cfg_attr(...)] condition: {error}",
                path.display()
            )
        })?;
    let mut values = values.into_iter();
    let Some(condition) = values.next() else {
        return Err(format!("{}: #[cfg_attr] has no condition", path.display()));
    };
    if !eval_cfg_meta(&condition, firmware, path)? {
        return Ok(true);
    }
    let mut active = true;
    for generated in values {
        active &= eval_generated_cfg_attribute(&generated, firmware, path)?;
    }
    Ok(active)
}

fn eval_generated_cfg_attribute(
    generated: &Meta,
    firmware: FirmwareVersion,
    path: &Path,
) -> Result<bool, String> {
    if generated.path().is_ident("cfg") {
        return eval_cfg_attribute(generated, firmware, path);
    }
    if generated.path().is_ident("cfg_attr") {
        return eval_cfg_attr_attribute(generated, firmware, path);
    }
    Ok(true)
}

fn eval_cfg_meta(meta: &Meta, firmware: FirmwareVersion, path: &Path) -> Result<bool, String> {
    match meta {
        Meta::Path(path_meta) => {
            let name = path_meta
                .get_ident()
                .map(ToString::to_string)
                .ok_or_else(|| {
                    format!(
                        "{}: unsupported multi-segment cfg path `{}`",
                        path.display(),
                        cfg_path_name(path_meta)
                    )
                })?;
            if let Some(value) = firmware.matches_version_cfg(&name) {
                return Ok(value);
            }
            match name.as_str() {
                // The checker invokes `cargo check`, not `cargo test` or rustdoc.
                "test" | "doctest" | "doc" => Ok(false),
                // `cargo check` uses the development profile unless the caller
                // explicitly changes it, matching this conservative default.
                "debug_assertions" => Ok(true),
                _ => Err(format!(
                    "{}: unsupported cfg predicate `{name}`; add it to the compliance cfg evaluator",
                    path.display()
                )),
            }
        }
        Meta::NameValue(value) if value.path.is_ident("feature") => {
            let Expr::Lit(literal) = &value.value else {
                return Err(format!(
                    "{}: cfg(feature = ...) must use a string literal",
                    path.display()
                ));
            };
            let Lit::Str(feature) = &literal.lit else {
                return Err(format!(
                    "{}: cfg(feature = ...) must use a string literal",
                    path.display()
                ));
            };
            Ok(feature.value() == firmware.feature_name())
        }
        Meta::NameValue(value) => Err(format!(
            "{}: unsupported cfg key `{}`",
            path.display(),
            cfg_path_name(&value.path)
        )),
        Meta::List(list) if list.path.is_ident("all") => {
            let values = parse_cfg_list(list, path)?;
            values
                .iter()
                .map(|value| eval_cfg_meta(value, firmware, path))
                .try_fold(true, |active, value| value.map(|value| active && value))
        }
        Meta::List(list) if list.path.is_ident("any") => {
            let values = parse_cfg_list(list, path)?;
            values
                .iter()
                .map(|value| eval_cfg_meta(value, firmware, path))
                .try_fold(false, |active, value| value.map(|value| active || value))
        }
        Meta::List(list) if list.path.is_ident("not") => {
            let values = parse_cfg_list(list, path)?;
            let values = values.iter().collect::<Vec<_>>();
            let [value] = values.as_slice() else {
                return Err(format!(
                    "{}: cfg(not(...)) requires exactly one predicate",
                    path.display()
                ));
            };
            Ok(!eval_cfg_meta(value, firmware, path)?)
        }
        Meta::List(list) => Err(format!(
            "{}: unsupported cfg combinator `{}`",
            path.display(),
            cfg_path_name(&list.path)
        )),
    }
}

fn parse_cfg_list(
    list: &syn::MetaList,
    path: &Path,
) -> Result<Punctuated<Meta, syn::Token![,]>, String> {
    list.parse_args_with(Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| format!("{}: could not parse cfg list: {error}", path.display()))
}

fn is_macro_named(path: &SynPath, name: &str) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

fn cfg_path_name(path: &SynPath) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

struct VendorCommandInvocation {
    name: syn::Ident,
    cgid: u16,
    cid: u16,
    body: TokenStream,
}

impl Parse for VendorCommandInvocation {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let arguments;
        syn::parenthesized!(arguments in input);
        let cgid_label = arguments.parse::<syn::Ident>()?;
        if cgid_label != "cgid" {
            return Err(syn::Error::new_spanned(cgid_label, "expected `cgid`"));
        }
        arguments.parse::<syn::Token![=]>()?;
        let cgid_literal = arguments.parse::<LitInt>()?;
        let cgid = parse_u16_literal(&cgid_literal).map_err(|error| {
            syn::Error::new_spanned(&cgid_literal, format!("invalid command group ID: {error}"))
        })?;
        if cgid > 0b111 {
            return Err(syn::Error::new_spanned(
                cgid_literal,
                "vendor command group ID must fit in three bits",
            ));
        }
        arguments.parse::<syn::Token![,]>()?;
        let cid_label = arguments.parse::<syn::Ident>()?;
        if cid_label != "cid" {
            return Err(syn::Error::new_spanned(cid_label, "expected `cid`"));
        }
        arguments.parse::<syn::Token![=]>()?;
        let cid_literal = arguments.parse::<LitInt>()?;
        let cid = parse_u16_literal(&cid_literal).map_err(|error| {
            syn::Error::new_spanned(&cid_literal, format!("invalid command ID: {error}"))
        })?;
        if cid > 0b111_1111 {
            return Err(syn::Error::new_spanned(
                cid_literal,
                "vendor command ID must fit in seven bits",
            ));
        }
        if !arguments.is_empty() {
            return Err(arguments.error("unexpected tokens after vendor command IDs"));
        }
        let body;
        syn::braced!(body in input);
        let body = body.parse::<TokenStream>()?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after vendor_cmd! body"));
        }
        Ok(Self {
            name,
            cgid,
            cid,
            body,
        })
    }
}

fn parse_u16_literal(literal: &LitInt) -> Result<u16, String> {
    parse_integer_literal(literal)
        .and_then(|value| u16::try_from(value).map_err(|_| format!("{value} does not fit in u16")))
}

fn parse_usize_literal(literal: &LitInt) -> Result<usize, String> {
    parse_integer_literal(literal).and_then(|value| {
        usize::try_from(value).map_err(|_| format!("{value} does not fit in usize"))
    })
}

fn parse_integer_literal(literal: &LitInt) -> Result<u128, String> {
    let mut value = literal.to_string();
    let suffix = literal.suffix();
    if !suffix.is_empty() {
        value.truncate(value.len() - suffix.len());
    }
    let digits = value.replace('_', "");
    // Rust literal suffixes are not useful in this table. Parse a leading radix
    // literal and reject anything that is not entirely numeric afterwards.
    let (radix, digits) = if let Some(digits) = digits.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = digits.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = digits.strip_prefix("0b") {
        (2, digits)
    } else {
        (10, digits.as_str())
    };
    u128::from_str_radix(digits, radix).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn version(major: u16, minor: u16, patch: u16) -> FirmwareVersion {
        FirmwareVersion::new(major, minor, patch)
    }

    fn fixture_descriptors(
        source: &str,
        firmware: FirmwareVersion,
    ) -> (Vec<CoverageEntry>, BTreeMap<String, DescriptorMetadata>) {
        let path = PathBuf::from("fixture.rs");
        let unit = SourceUnit {
            path: path.clone(),
            active: true,
            file: syn::parse_file(source).unwrap(),
        };
        let descriptors = collect_descriptors(std::slice::from_ref(&unit), firmware).unwrap();
        let commands = descriptor_coverage(&descriptors);
        (commands, descriptors)
    }

    #[test]
    fn evaluates_nested_firmware_cfgs() {
        let firmware = version(0, 17, 0);
        let path = Path::new("fixture.rs");
        let attribute: Attribute = syn::parse_quote!(
            #[cfg(all(since_fw_0_17_0, not(after_fw_0_17_0)))]
        );
        assert!(attrs_active(&[attribute], firmware, path).unwrap());
    }

    #[test]
    fn keeps_descriptor_return_metadata() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = ();
                    Completion = CommandComplete;
                    Return = Result { value: [u8; 8] => 8, };
                }
            }
            impl<T> Commands for T {
                hci_impl_params!(command, Params, Current);
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.code, 0x0003);
        assert_eq!(
            descriptor.completion,
            CompletionExpectation::CommandComplete
        );
        assert_eq!(descriptor.request, WireEnvelope::fixed(0));
        assert_eq!(descriptor.response, Some(WireEnvelope::fixed(8)));
    }

    #[test]
    fn parses_declarative_fixed_command_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = {
                        role: Role => 1,
                        enabled: bool => 1,
                        name_len: u8 => 1,
                    };
                    Completion = CommandComplete;
                    Return = Result {
                        first_handle: AttributeHandle => 2,
                        second_handle: AttributeHandle => 2,
                        third_handle: AttributeHandle => 2,
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::try_new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(
            descriptor.completion,
            CompletionExpectation::CommandComplete
        );
        assert_eq!(descriptor.request, WireEnvelope::fixed(3));
        assert_eq!(descriptor.response, Some(WireEnvelope::fixed(6)));
    }

    #[test]
    fn parses_explicit_command_status_shape() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { procedure: u8 => 1, };
                    Completion = CommandStatus;
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.completion, CompletionExpectation::CommandStatus);
        assert_eq!(descriptor.request, WireEnvelope::fixed(1));
        assert_eq!(descriptor.response, None);
    }

    #[test]
    fn parses_counted_request_and_bounded_return_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        conn_handle: ConnHandle => 2,
                        handles: &'a [AttributeHandle] => {
                            kind: counted_items,
                            count: u8 => 1,
                            item: AttributeHandle => 2,
                            max_items: 126,
                        },
                    };
                    Completion = CommandComplete;
                    Return = Result {
                        total_length: u16 => 2,
                        value: BoundedBytes<249> => {
                            kind: counted_bytes,
                            count: u16 => 2,
                            max_len: 249,
                        },
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(
            descriptor.completion,
            CompletionExpectation::CommandComplete
        );
        assert_eq!(descriptor.request, WireEnvelope::bounded(3, 255));
        // The status byte is framing, not part of the command-owned return.
        assert_eq!(descriptor.response, Some(WireEnvelope::bounded(4, 253)));
    }

    #[test]
    fn parses_inline_trailing_byte_returns() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { offset: u8 => 1, };
                    Completion = CommandComplete;
                    Return = Result {
                        value: BoundedBytes<16> => {
                            kind: trailing_bytes,
                            min_len: 1,
                            max_len: 16,
                        },
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.request, WireEnvelope::fixed(1));
        assert_eq!(descriptor.response, Some(WireEnvelope::bounded(1, 16)));
    }

    #[test]
    fn rejects_fields_after_trailing_bytes() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        value: &'a [u8] => {
                            kind: trailing_bytes,
                            min_len: 0,
                            max_len: 16,
                        },
                        suffix: u8 => 1,
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("trailing_bytes must be the final declarative field"));
    }

    #[test]
    fn rejects_invalid_or_legacy_command_ids() {
        for (source, expected) in [
            (
                "vendor_cmd! { Current(cgid = 0x8, cid = 0x01) {} }",
                "command group ID must fit in three bits",
            ),
            (
                "vendor_cmd! { Current(cgid = 0x1, cid = 0x80) {} }",
                "command ID must fit in seven bits",
            ),
            ("vendor_cmd! { Current(CURRENT) {} }", "expected `cgid`"),
        ] {
            let file = syn::parse_file(source).unwrap();
            let Item::Macro(item) = &file.items[0] else {
                panic!("expected vendor_cmd! macro item");
            };
            let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }

        let unit = SourceUnit {
            path: PathBuf::from("fixture.rs"),
            active: true,
            file: syn::parse_file(
                r#"
                    vendor_cmd! { First(cgid = 0x1, cid = 0x02) { Params = (); Completion = CommandStatus; } }
                    vendor_cmd! { Second(cgid = 0x1, cid = 0x02) { Params = (); Completion = CommandStatus; } }
                "#,
            )
            .unwrap(),
        };
        let error =
            collect_descriptors(std::slice::from_ref(&unit), version(0, 17, 0)).unwrap_err();
        assert!(error.contains("both declare active vendor OCF 0x082"));
    }

    #[test]
    fn parses_tagged_and_bitmap_selected_request_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        scanning_phys: u8 => 1,
                        phy_params: &'a [Phy] => {
                            kind: bitmap_items,
                            bitmap: scanning_phys,
                            mask: 0x05,
                            item: Phy => 5,
                            max_items: 2,
                        },
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8 => 1,
                            variants: {
                                Uuid::Uuid16(value) => {
                                    tag: 0x01,
                                    fields: { value: u16 => 2, },
                                },
                                Uuid::Uuid128(value) => {
                                    tag: 0x02,
                                    fields: { value: [u8; 16] => 16, },
                                },
                            },
                            min_len: 3,
                            max_len: 17,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::try_new(); }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        let descriptor = descriptors.get("Current").unwrap();
        assert_eq!(descriptor.completion, CompletionExpectation::CommandStatus);
        assert_eq!(descriptor.request, WireEnvelope::bounded(4, 28));
        assert_eq!(descriptor.response, None);
    }

    #[test]
    fn parses_constraints_and_rejects_unknown_parameter_references() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        minimum: u16 => 2,
                        maximum: u16 => 2,
                        mode: u8 => 1,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8 => 1,
                            max_len: 16,
                        },
                    };
                    Constraints = {
                        ordered(minimum, maximum);
                        range(minimum, 0x20, 0x4000);
                        one_of(mode, [0x00, 0x02]);
                        len_at_most(data, mode);
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let (_, descriptors) = fixture_descriptors(source, version(0, 17, 0));
        assert_eq!(
            descriptors.get("Current").unwrap().request,
            WireEnvelope::bounded(6, 22)
        );

        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { value: u8 => 1, };
                    Constraints = { ordered(value, missing); };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("unknown parameter(s): missing"), "{error}");
    }

    #[test]
    fn rejects_removed_command_payload_fields() {
        for (source, section) in [
            (
                r#"
                    vendor_cmd! {
                        Current(cgid = 0x0, cid = 0x03) {
                            Params<'a> = {
                                uuid: &'a Uuid => {
                                    kind: payload,
                                    min_len: 3,
                                    max_len: 17,
                                },
                            };
                            Completion = CommandStatus;
                        }
                    }
                "#,
                "Params",
            ),
            (
                r#"
                    vendor_cmd! {
                        Current(cgid = 0x0, cid = 0x03) {
                            Params = ();
                            Completion = CommandComplete;
                            Return = Result {
                                uuid: Uuid => {
                                    kind: payload,
                                    min_len: 3,
                                    max_len: 17,
                                },
                            };
                        }
                    }
                "#,
                "Return",
            ),
        ] {
            let file = syn::parse_file(source).unwrap();
            let Item::Macro(item) = &file.items[0] else {
                panic!("expected vendor_cmd! macro item");
            };
            let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
            assert!(error.contains("removed `kind: payload`"), "{error}");
            assert!(error.contains(section), "{error}");
            assert!(error.contains("inline the wire schema"), "{error}");
        }
    }

    #[test]
    fn rejects_incorrect_tagged_range_and_bitmap_cardinality() {
        let bad_tagged = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8 => 1,
                            variants: {
                                Uuid::Uuid16(value) => {
                                    tag: 0x01,
                                    fields: { value: u16 => 2, },
                                },
                                Uuid::Uuid128(value) => {
                                    tag: 0x02,
                                    fields: { value: [u8; 16] => 16, },
                                },
                            },
                            min_len: 2,
                            max_len: 17,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(bad_tagged).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("variants require 3..=17"));

        let bad_bitmap = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        scanning_phys: u8 => 1,
                        phy_params: &'a [Phy] => {
                            kind: bitmap_items,
                            bitmap: scanning_phys,
                            mask: 0x05,
                            item: Phy => 5,
                            max_items: 3,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(bad_bitmap).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("mask selects 2 bits but max_items is 3"));
    }

    #[test]
    fn rejects_tagged_payload_field_not_bound_by_pattern() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8 => 1,
                            variants: {
                                Uuid::Uuid16(actual) => {
                                    tag: 0x01,
                                    fields: { typo: u16 => 2, },
                                },
                            },
                            min_len: 3,
                            max_len: 3,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("payload field `typo` is not bound"));
    }

    #[test]
    fn rejects_return_on_explicit_command_status() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = ();
                    Completion = CommandStatus;
                    Return = ();
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("CommandStatus and must not declare Return"));
    }

    #[test]
    fn rejects_legacy_completion_inference_and_return_buffers() {
        let missing_completion = syn::parse_file(
            r#"vendor_cmd! { Current(cgid = 0x0, cid = 0x03) { Params = (); Return = (); } }"#,
        )
        .unwrap();
        let Item::Macro(item) = &missing_completion.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("missing a `Completion = ...` declaration"));

        let return_buffer = syn::parse_file(
            r#"
                vendor_cmd! {
                    Current(cgid = 0x0, cid = 0x03) {
                        Params = ();
                        Completion = CommandComplete;
                        Return = ReturnBuffer<9>;
                    }
                }
            "#,
        )
        .unwrap();
        let Item::Macro(item) = &return_buffer.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_descriptor(item, Path::new("fixture.rs")).unwrap_err();
        assert!(error.contains("expected `()` or an inline named field body"));
    }

    #[test]
    fn loads_declarative_variable_shapes_from_the_real_crate() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let coverage = load_crate_coverage(&crate_dir, version(0, 17, 1)).unwrap();

        let update = coverage
            .descriptor_metadata
            .get("GapUpdateAdvertisingData")
            .unwrap();
        assert_eq!(update.request, WireEnvelope::bounded(1, 32));
        assert_eq!(update.response, Some(WireEnvelope::fixed(0)));

        let discoverable = coverage
            .descriptor_metadata
            .get("GapSetLimitedDiscoverable")
            .unwrap();
        // Independent field capacities exceed one HCI command, but the
        // generated constructor rejects aggregate payloads above 255 bytes.
        assert_eq!(discoverable.request, WireEnvelope::bounded(13, 255));

        let read = coverage
            .descriptor_metadata
            .get("GattReadHandleValue")
            .unwrap();
        assert_eq!(read.request, WireEnvelope::fixed(6));
        assert_eq!(read.response, Some(WireEnvelope::bounded(4, 251)));

        assert!(
            coverage
                .descriptor_metadata
                .contains_key("GattReadMultipleVarCharValue")
        );

        let tagged = coverage
            .descriptor_metadata
            .get("GattDiscoverPrimaryServicesByUUID")
            .unwrap();
        assert_eq!(tagged.request, WireEnvelope::bounded(5, 19));
        assert_eq!(tagged.completion, CompletionExpectation::CommandStatus);

        assert!(!coverage.descriptor_metadata.contains_key("GapExtStartScan"));
        let future_coverage = load_crate_coverage(&crate_dir, version(0, 18, 0)).unwrap();
        let bitmap = future_coverage
            .descriptor_metadata
            .get("GapExtStartScan")
            .unwrap();
        assert_eq!(bitmap.request, WireEnvelope::bounded(10, 20));
        assert_eq!(bitmap.response, None);

        let bonded = coverage
            .descriptor_metadata
            .get("GapGetBondedDevices")
            .unwrap();
        // Count plus at most 35 seven-byte address records; status is framing.
        assert_eq!(bonded.response, Some(WireEnvelope::bounded(1, 246)));

        let config = coverage
            .descriptor_metadata
            .get("HalReadConfigData")
            .unwrap();
        assert_eq!(config.response, Some(WireEnvelope::bounded(1, 16)));

        let channels = coverage
            .descriptor_metadata
            .get("L2CocConnectConfirm")
            .unwrap();
        assert_eq!(channels.response, Some(WireEnvelope::bounded(1, 6)));

        assert_eq!(coverage.event_metadata.len(), 55);
        let gap_procedure = coverage.event_metadata.get(&0x0407).unwrap();
        assert_eq!(gap_procedure.name, "GapProcedureComplete");
        assert_eq!(gap_procedure.payload, WireEnvelope::bounded(3, 253));

        let bond_lost = coverage.event_metadata.get(&0x0405).unwrap();
        assert_eq!(bond_lost.payload, WireEnvelope::fixed(0));

        let read_multiple = coverage.event_metadata.get(&0x0C15).unwrap();
        assert_eq!(read_multiple.payload, WireEnvelope::bounded(3, 253));
    }

    #[test]
    fn loads_unique_command_ids_for_every_declared_firmware() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for firmware in FirmwareVersion::declared_in_manifest(&crate_dir).unwrap() {
            load_crate_coverage(&crate_dir, firmware).unwrap();
        }
    }

    #[test]
    fn selects_commands_from_descriptor_cfg() {
        let source = r#"
            vendor_cmd! { Current(cgid = 0x0, cid = 0x03) { Params = (); Completion = CommandStatus; } }
            #[cfg(after_fw_0_17_1)]
            vendor_cmd! {
                Retained(cgid = 0x0, cid = 0x01) {
                    Params = ();
                    Completion = CommandStatus;
                }
            }
        "#;
        let path = PathBuf::from("fixture.rs");
        let unit = SourceUnit {
            path,
            active: true,
            file: syn::parse_file(source).unwrap(),
        };
        let firmware = version(0, 17, 0);
        let descriptors = collect_descriptors(std::slice::from_ref(&unit), firmware).unwrap();
        let active = descriptor_coverage(&descriptors);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Current");
        assert!(descriptors.contains_key("Current"));
        assert!(!descriptors.contains_key("Retained"));

        let future = collect_descriptors(std::slice::from_ref(&unit), version(0, 18, 0)).unwrap();
        assert!(future.contains_key("Current"));
        assert!(future.contains_key("Retained"));
    }

    #[test]
    fn discovers_only_active_command_modules() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "stm32wb-compliance-command-modules-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let root = directory.join("mod.rs");
        fs::write(
            &root,
            r#"
                pub mod current;
                #[cfg(after_fw_0_17_1)]
                pub mod future;
            "#,
        )
        .unwrap();
        fs::write(directory.join("current.rs"), "").unwrap();
        fs::write(directory.join("future.rs"), "").unwrap();

        let mut sources = Vec::new();
        let mut visited = BTreeSet::new();
        collect_command_sources(
            root.clone(),
            read_rust_file(&root).unwrap(),
            true,
            version(0, 17, 1),
            &mut visited,
            &mut sources,
        )
        .unwrap();
        let names = sources
            .iter()
            .filter_map(|source| source.path.file_name().and_then(|name| name.to_str()))
            .collect::<BTreeSet<_>>();
        assert!(names.contains("current.rs"));
        assert!(!names.contains("future.rs"));

        fs::remove_dir_all(directory).unwrap();
    }
}
