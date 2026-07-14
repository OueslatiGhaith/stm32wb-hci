//! Shared syntax and validation for declarative STM32WB protocol schemas.
//!
//! This crate is host-only. Both the procedural macros and the compliance
//! checker consume this parser, which prevents either side from silently
//! accepting a command declaration that the other interprets differently.

mod firmware;

pub use firmware::{
    FirmwareFeatureError, FirmwareManifestError, FirmwareVersion, FirmwareVersionError,
};

use std::collections::BTreeSet;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::{
    Expr, LitInt, Type,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    visit::{self, Visit},
};

/// A parsed `vendor_cmd!` declaration.
pub struct VendorCommand {
    /// Generated command type name.
    pub name: syn::Ident,
    /// Three-bit STM32 vendor command-group ID.
    pub cgid: u16,
    /// Seven-bit command ID within `cgid`.
    pub cid: u16,
    /// Request payload schema.
    pub params: Params,
    /// Semantic checks applied by the generated constructor.
    pub constraints: Option<Constraints>,
    /// HCI completion mechanism.
    pub completion: Completion,
    /// Command Complete return schema, absent for Command Status commands.
    pub returns: Option<Returns>,
}

impl VendorCommand {
    /// The ten-bit vendor OCF derived from `cgid` and `cid`.
    pub const fn ocf(&self) -> u16 {
        (self.cgid << 7) | self.cid
    }
}

/// Request payload syntax.
pub enum Params {
    /// `Params = ();`
    Unit,
    /// `Params = { ... };` or `Params<'a> = { ... };`
    Fields(Fields),
}

impl Params {
    /// Parsed field metadata, if the request is not unit.
    pub const fn fields(&self) -> Option<&Fields> {
        match self {
            Self::Unit => None,
            Self::Fields(fields) => Some(fields),
        }
    }

    /// Minimum encoded request size.
    pub fn min_len(&self) -> usize {
        self.fields().map_or(0, Fields::min_len)
    }

    /// Maximum encoded request size before the HCI packet limit is applied.
    pub fn max_len(&self) -> usize {
        self.fields().map_or(0, Fields::max_len)
    }

    /// Declared request field names.
    pub fn field_names(&self) -> BTreeSet<String> {
        self.fields()
            .map_or_else(BTreeSet::new, |fields| fields.names.clone())
    }
}

/// Return payload syntax for a Command Complete command.
pub enum Returns {
    /// `Return = ();`
    Unit,
    /// `Return = Name { ... };`
    Fields {
        /// Generated return type name.
        name: syn::Ident,
        /// Return wire fields.
        fields: Fields,
    },
}

impl Returns {
    /// Parsed return fields, if the return is not unit.
    pub const fn fields(&self) -> Option<&Fields> {
        match self {
            Self::Unit => None,
            Self::Fields { fields, .. } => Some(fields),
        }
    }

    /// Minimum encoded return size.
    pub fn min_len(&self) -> usize {
        self.fields().map_or(0, Fields::min_len)
    }

    /// Maximum encoded return size.
    pub fn max_len(&self) -> usize {
        self.fields().map_or(0, Fields::max_len)
    }
}

/// Parsed aggregate metadata for an inline field body.
pub struct Fields {
    names: BTreeSet<String>,
    min_len: usize,
    max_len: usize,
    contains_removed_payload: bool,
}

impl Fields {
    /// Field names declared by this body.
    pub const fn names(&self) -> &BTreeSet<String> {
        &self.names
    }

    /// Minimum number of bytes encoded by this field body.
    pub const fn min_len(&self) -> usize {
        self.min_len
    }

    /// Maximum number of bytes encoded by this field body.
    pub const fn max_len(&self) -> usize {
        self.max_len
    }

    /// Whether this body used the removed opaque `kind: payload` escape hatch.
    pub const fn contains_removed_payload(&self) -> bool {
        self.contains_removed_payload
    }

    /// An empty field body, used by unit event payloads.
    pub fn empty() -> Self {
        Self {
            names: BTreeSet::new(),
            min_len: 0,
            max_len: 0,
            contains_removed_payload: false,
        }
    }
}

/// Completion mechanism declared by a command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    /// The controller returns a Command Complete event and a return payload.
    CommandComplete,
    /// The controller first returns Command Status and completes asynchronously.
    CommandStatus,
}

/// Parsed constraint metadata.
///
/// The parser validates each supported form and records all referenced request
/// fields. The procedural macro still delegates runtime code generation during
/// the scaffold phase; the compliance checker consumes the same references.
pub struct Constraints {
    referenced_fields: BTreeSet<String>,
}

impl Constraints {
    /// Every request field referenced by the constraint body.
    pub const fn referenced_fields(&self) -> &BTreeSet<String> {
        &self.referenced_fields
    }
}

struct Invocation {
    name: syn::Ident,
    cgid: u16,
    cid: u16,
    body: TokenStream,
}

impl Parse for Invocation {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let arguments;
        syn::parenthesized!(arguments in input);

        parse_label_equals(&arguments, "cgid")?;
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
        parse_label_equals(&arguments, "cid")?;
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

impl Parse for VendorCommand {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let invocation = input.parse::<Invocation>()?;
        let mut tokens = invocation.body.into_iter().peekable();
        let mut params = None;
        let mut constraints = None;
        let mut completion = None;
        let mut returns = None;

        while tokens.peek().is_some() {
            let Some(TokenTree::Ident(label)) = tokens.next() else {
                return Err(syn::Error::new(
                    invocation.name.span(),
                    "expected a declaration name in vendor_cmd! body",
                ));
            };

            let mut header = TokenStream::new();
            let mut found_equals = false;
            for token in tokens.by_ref() {
                if matches!(&token, TokenTree::Punct(punctuation) if punctuation.as_char() == '=') {
                    found_equals = true;
                    break;
                }
                header.extend([token]);
            }
            if !found_equals {
                return Err(syn::Error::new_spanned(label, "declaration has no `=`"));
            }

            let mut value = TokenStream::new();
            let mut terminated = false;
            for token in tokens.by_ref() {
                if matches!(&token, TokenTree::Punct(punctuation) if punctuation.as_char() == ';') {
                    terminated = true;
                    break;
                }
                value.extend([token]);
            }
            if !terminated {
                return Err(syn::Error::new_spanned(label, "declaration is missing `;`"));
            }

            match label.to_string().as_str() {
                "Params" => {
                    if params.is_some() {
                        return Err(syn::Error::new_spanned(
                            label,
                            "vendor command declares `Params` more than once",
                        ));
                    }
                    syn::parse2::<ParamsLifetime>(header)?;
                    params = Some(parse_params(value)?);
                }
                "Constraints" => {
                    require_empty_header(&header, &label)?;
                    if constraints.is_some() {
                        return Err(syn::Error::new_spanned(
                            label,
                            "vendor command declares `Constraints` more than once",
                        ));
                    }
                    constraints = Some(parse_constraints(value)?);
                }
                "Completion" => {
                    require_empty_header(&header, &label)?;
                    if completion.is_some() {
                        return Err(syn::Error::new_spanned(
                            label,
                            "vendor command declares `Completion` more than once",
                        ));
                    }
                    completion = Some(parse_completion(value)?);
                }
                "Return" => {
                    require_empty_header(&header, &label)?;
                    if returns.is_some() {
                        return Err(syn::Error::new_spanned(
                            label,
                            "vendor command declares `Return` more than once",
                        ));
                    }
                    returns = Some(parse_returns(value)?);
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        label,
                        "unknown vendor command declaration",
                    ));
                }
            }
        }

        let params = params.ok_or_else(|| {
            syn::Error::new(
                invocation.name.span(),
                "vendor command is missing a `Params = ...` declaration",
            )
        })?;
        if params.min_len() > usize::from(u8::MAX) {
            return Err(syn::Error::new(
                invocation.name.span(),
                format!(
                    "minimum Params envelope is {} bytes, exceeding the HCI 255-byte parameter limit",
                    params.min_len()
                ),
            ));
        }
        if params
            .fields()
            .is_some_and(Fields::contains_removed_payload)
        {
            return Err(syn::Error::new(
                invocation.name.span(),
                "Params uses removed `kind: payload`; inline the wire schema instead",
            ));
        }

        if let Some(constraints) = &constraints {
            let names = params.field_names();
            let unknown = constraints
                .referenced_fields()
                .difference(&names)
                .cloned()
                .collect::<Vec<_>>();
            if !unknown.is_empty() {
                return Err(syn::Error::new(
                    invocation.name.span(),
                    format!(
                        "constraints reference unknown parameter(s): {}",
                        unknown.join(", ")
                    ),
                ));
            }
        }

        let completion = completion.ok_or_else(|| {
            syn::Error::new(
                invocation.name.span(),
                "vendor command is missing a `Completion = ...` declaration",
            )
        })?;
        match (completion, &returns) {
            (Completion::CommandComplete, None) => {
                return Err(syn::Error::new(
                    invocation.name.span(),
                    "CommandComplete requires a Return declaration",
                ));
            }
            (Completion::CommandStatus, Some(_)) => {
                return Err(syn::Error::new(
                    invocation.name.span(),
                    "vendor command declares CommandStatus and must not declare Return",
                ));
            }
            _ => {}
        }
        if returns
            .as_ref()
            .and_then(Returns::fields)
            .is_some_and(Fields::contains_removed_payload)
        {
            return Err(syn::Error::new(
                invocation.name.span(),
                "Return uses removed `kind: payload`; inline the wire schema instead",
            ));
        }

        Ok(Self {
            name: invocation.name,
            cgid: invocation.cgid,
            cid: invocation.cid,
            params,
            constraints,
            completion,
            returns,
        })
    }
}

struct ParamsLifetime;

impl Parse for ParamsLifetime {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self);
        }
        input.parse::<syn::Token![<]>()?;
        input.parse::<syn::Lifetime>()?;
        input.parse::<syn::Token![>]>()?;
        if !input.is_empty() {
            return Err(input.error("Params accepts at most one lifetime parameter"));
        }
        Ok(Self)
    }
}

fn require_empty_header(header: &TokenStream, label: &syn::Ident) -> syn::Result<()> {
    if header.is_empty() {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            label,
            "only Params may declare a lifetime parameter",
        ))
    }
}

fn parse_params(value: TokenStream) -> syn::Result<Params> {
    if is_unit_type(&value) {
        return Ok(Params::Unit);
    }
    parse_braced_fields(value).map(Params::Fields)
}

fn parse_returns(value: TokenStream) -> syn::Result<Returns> {
    if is_unit_type(&value) {
        return Ok(Returns::Unit);
    }
    let mut shape = value.clone().into_iter();
    let inline_fields = matches!(shape.next(), Some(TokenTree::Ident(_)))
        && matches!(shape.next(), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace)
        && shape.next().is_none();
    if !inline_fields {
        return Err(syn::Error::new_spanned(
            value,
            "expected `()` or an inline named field body",
        ));
    }
    syn::parse2::<NamedReturns>(value).map(|returns| Returns::Fields {
        name: returns.name,
        fields: returns.fields,
    })
}

fn is_unit_type(tokens: &TokenStream) -> bool {
    matches!(
        syn::parse2::<Type>(tokens.clone()),
        Ok(Type::Tuple(tuple)) if tuple.elems.is_empty()
    )
}

fn parse_braced_fields(tokens: TokenStream) -> syn::Result<Fields> {
    let mut tokens = tokens.into_iter();
    let Some(TokenTree::Group(group)) = tokens.next() else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected `()` or an inline named field body",
        ));
    };
    if group.delimiter() != Delimiter::Brace || tokens.next().is_some() {
        return Err(syn::Error::new_spanned(
            group,
            "expected `()` or one inline named field body",
        ));
    }
    syn::parse2::<Fields>(group.stream())
}

struct NamedReturns {
    name: syn::Ident,
    fields: Fields,
}

impl Parse for NamedReturns {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let content;
        syn::braced!(content in input);
        let fields = content.parse::<Fields>()?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after declarative Return body"));
        }
        Ok(Self { name, fields })
    }
}

fn parse_completion(value: TokenStream) -> syn::Result<Completion> {
    let completion = syn::parse2::<syn::Ident>(value)?;
    match completion.to_string().as_str() {
        "CommandComplete" => Ok(Completion::CommandComplete),
        "CommandStatus" => Ok(Completion::CommandStatus),
        _ => Err(syn::Error::new_spanned(
            completion,
            "expected `CommandComplete` or `CommandStatus`",
        )),
    }
}

fn parse_constraints(value: TokenStream) -> syn::Result<Constraints> {
    let mut tokens = value.into_iter();
    let Some(TokenTree::Group(group)) = tokens.next() else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "Constraints must be an inline body",
        ));
    };
    if group.delimiter() != Delimiter::Brace || tokens.next().is_some() {
        return Err(syn::Error::new_spanned(
            group,
            "Constraints must be one inline body",
        ));
    }
    syn::parse2::<Constraints>(group.stream())
}

impl Parse for Constraints {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut referenced_fields = BTreeSet::new();
        while !input.is_empty() {
            let kind = input.parse::<syn::Ident>()?;
            let arguments;
            syn::parenthesized!(arguments in input);

            match kind.to_string().as_str() {
                "ordered" | "len_at_most" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    insert_field(&arguments, &mut referenced_fields)?;
                }
                "ordered_when_in_range" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    insert_field(&arguments, &mut referenced_fields)?;
                    parse_two_expressions(&arguments)?;
                }
                "range" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    parse_two_expressions(&arguments)?;
                }
                "one_of" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    parse_nonempty_expression_list(&arguments, "one_of")?;
                }
                "one_of_or_range" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    parse_nonempty_expression_list(&arguments, "one_of_or_range")?;
                    parse_two_expressions(&arguments)?;
                }
                "paired_value" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    arguments.parse::<Expr>()?;
                }
                "implies_eq" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    arguments.parse::<Expr>()?;
                    arguments.parse::<syn::Token![,]>()?;
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    arguments.parse::<Expr>()?;
                }
                "implies_range" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    arguments.parse::<Expr>()?;
                    arguments.parse::<syn::Token![,]>()?;
                    insert_field(&arguments, &mut referenced_fields)?;
                    parse_two_expressions(&arguments)?;
                }
                "pawr_subevents_fit" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    for _ in 0..2 {
                        arguments.parse::<syn::Token![,]>()?;
                        insert_field(&arguments, &mut referenced_fields)?;
                    }
                }
                "pawr_response_slots_fit" => {
                    insert_field(&arguments, &mut referenced_fields)?;
                    for _ in 0..4 {
                        arguments.parse::<syn::Token![,]>()?;
                        insert_field(&arguments, &mut referenced_fields)?;
                    }
                }
                "non_empty" => insert_field(&arguments, &mut referenced_fields)?,
                _ => {
                    return Err(syn::Error::new_spanned(
                        kind,
                        "unknown declarative constraint",
                    ));
                }
            }

            if !arguments.is_empty() {
                return Err(arguments.error("unexpected tokens in declarative constraint"));
            }
            input.parse::<syn::Token![;]>()?;
        }
        Ok(Self { referenced_fields })
    }
}

fn insert_field(input: ParseStream<'_>, fields: &mut BTreeSet<String>) -> syn::Result<()> {
    fields.insert(input.parse::<syn::Ident>()?.to_string());
    Ok(())
}

fn parse_two_expressions(input: ParseStream<'_>) -> syn::Result<()> {
    input.parse::<syn::Token![,]>()?;
    input.parse::<Expr>()?;
    input.parse::<syn::Token![,]>()?;
    input.parse::<Expr>()?;
    Ok(())
}

fn parse_nonempty_expression_list(input: ParseStream<'_>, kind: &str) -> syn::Result<()> {
    let allowed;
    syn::bracketed!(allowed in input);
    let values = Punctuated::<Expr, syn::Token![,]>::parse_terminated(&allowed)?;
    if values.is_empty() {
        return Err(allowed.error(format!("{kind} must declare at least one allowed value")));
    }
    Ok(())
}

impl Parse for Fields {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut names = BTreeSet::new();
        let mut min_len = 0usize;
        let mut max_len = 0usize;
        let mut consumes_remainder = false;
        let mut contains_removed_payload = false;

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

            let shape = if input.peek(LitInt) {
                let width = parse_usize_literal(&input.parse::<LitInt>()?)
                    .map_err(|error| input.error(error))?;
                VariableShape {
                    min_len: width,
                    max_len: width,
                    removed_payload: false,
                    consumes_remainder: false,
                }
            } else if input.peek(syn::token::Brace) {
                let shape;
                syn::braced!(shape in input);
                shape.parse::<VariableShape>()?
            } else {
                return Err(input.error("expected a fixed width or variable field shape"));
            };

            consumes_remainder = shape.consumes_remainder;
            contains_removed_payload |= shape.removed_payload;
            min_len = min_len
                .checked_add(shape.min_len)
                .ok_or_else(|| input.error("declarative field minimum length overflows usize"))?;
            max_len = max_len
                .checked_add(shape.max_len)
                .ok_or_else(|| input.error("declarative field length overflows usize"))?;
            input.parse::<syn::Token![,]>()?;
        }

        Ok(Self {
            names,
            min_len,
            max_len,
            contains_removed_payload,
        })
    }
}

struct VariableShape {
    min_len: usize,
    max_len: usize,
    removed_payload: bool,
    consumes_remainder: bool,
}

impl Parse for VariableShape {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        parse_colon_label(input, "kind")?;
        let kind = input.parse::<syn::Ident>()?;
        input.parse::<syn::Token![,]>()?;

        let mut removed_payload = false;
        let mut consumes_remainder = false;
        let (min_len, max_len) = match kind.to_string().as_str() {
            "counted_bytes" => {
                let count_len = parse_type_width(input, "count")?;
                let max_value = parse_integer(input, "max_len")?;
                (Some(count_len), count_len.checked_add(max_value))
            }
            "counted_items" => {
                let count_len = parse_type_width(input, "count")?;
                let item_len = parse_type_width(input, "item")?;
                let max_items = parse_integer(input, "max_items")?;
                (
                    Some(count_len),
                    item_len
                        .checked_mul(max_items)
                        .and_then(|items| count_len.checked_add(items)),
                )
            }
            "tagged" => parse_tagged_shape(input)?,
            "payload" => {
                let min_len = parse_integer(input, "min_len")?;
                let max_len = parse_integer(input, "max_len")?;
                validate_range(input, "payload", min_len, max_len)?;
                removed_payload = true;
                (Some(min_len), Some(max_len))
            }
            "trailing_bytes" => {
                let min_len = parse_integer(input, "min_len")?;
                let max_len = parse_integer(input, "max_len")?;
                validate_range(input, "trailing_bytes", min_len, max_len)?;
                consumes_remainder = true;
                (Some(min_len), Some(max_len))
            }
            "bitmap_items" => {
                parse_colon_label(input, "bitmap")?;
                input.parse::<syn::Ident>()?;
                input.parse::<syn::Token![,]>()?;
                let mask = parse_integer(input, "mask")?;
                let item_len = parse_type_width(input, "item")?;
                let max_items = parse_integer(input, "max_items")?;
                if mask.count_ones() as usize != max_items {
                    return Err(input.error(format!(
                        "bitmap mask selects {} bits but max_items is {max_items}",
                        mask.count_ones()
                    )));
                }
                (Some(0), item_len.checked_mul(max_items))
            }
            _ => {
                return Err(input.error(format!("unknown declarative variable kind `{kind}`")));
            }
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
            removed_payload,
            consumes_remainder,
        })
    }
}

fn validate_range(
    input: ParseStream<'_>,
    kind: &str,
    minimum: usize,
    maximum: usize,
) -> syn::Result<()> {
    if minimum > maximum {
        Err(input.error(format!(
            "{kind} minimum {minimum} exceeds maximum {maximum}"
        )))
    } else {
        Ok(())
    }
}

fn parse_tagged_shape(input: ParseStream<'_>) -> syn::Result<(Option<usize>, Option<usize>)> {
    let tag_len = parse_type_width(input, "tag")?;
    parse_colon_label(input, "variants")?;
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

        let tag = parse_integer(&body, "tag")?;
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

        parse_colon_label(&body, "fields")?;
        let fields;
        syn::braced!(fields in body);
        body.parse::<syn::Token![,]>()?;
        if !body.is_empty() {
            return Err(body.error("unexpected tokens after tagged variant fields"));
        }
        let payload = parse_fixed_fields(&fields)?;
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
    let declared_min = parse_integer(input, "min_len")?;
    let declared_max = parse_integer(input, "max_len")?;
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

struct FixedFields {
    len: usize,
    names: Vec<String>,
}

fn parse_fixed_fields(input: ParseStream<'_>) -> syn::Result<FixedFields> {
    let mut len = 0usize;
    let mut names = Vec::new();
    while !input.is_empty() {
        names.push(input.parse::<syn::Ident>()?.to_string());
        input.parse::<syn::Token![:]>()?;
        input.parse::<Type>()?;
        input.parse::<syn::Token![=>]>()?;
        let width =
            parse_usize_literal(&input.parse::<LitInt>()?).map_err(|error| input.error(error))?;
        len = len
            .checked_add(width)
            .ok_or_else(|| input.error("tagged variant field length overflows usize"))?;
        input.parse::<syn::Token![,]>()?;
    }
    Ok(FixedFields { len, names })
}

fn parse_label_equals(input: ParseStream<'_>, expected: &str) -> syn::Result<()> {
    let label = input.parse::<syn::Ident>()?;
    if label != expected {
        return Err(syn::Error::new_spanned(
            label,
            format!("expected `{expected}`"),
        ));
    }
    input.parse::<syn::Token![=]>().map(|_| ())
}

fn parse_colon_label(input: ParseStream<'_>, expected: &str) -> syn::Result<()> {
    let label = input.parse::<syn::Ident>()?;
    if label != expected {
        return Err(syn::Error::new_spanned(
            label,
            format!("expected `{expected}`"),
        ));
    }
    input.parse::<syn::Token![:]>().map(|_| ())
}

fn parse_type_width(input: ParseStream<'_>, label: &str) -> syn::Result<usize> {
    parse_colon_label(input, label)?;
    input.parse::<Type>()?;
    input.parse::<syn::Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    input.parse::<syn::Token![,]>()?;
    parse_usize_literal(&width).map_err(|error| input.error(error))
}

fn parse_integer(input: ParseStream<'_>, label: &str) -> syn::Result<usize> {
    parse_colon_label(input, label)?;
    let value = input.parse::<LitInt>()?;
    input.parse::<syn::Token![,]>()?;
    parse_usize_literal(&value).map_err(|error| input.error(error))
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

    #[test]
    fn parses_fixed_command_and_derives_ocf() {
        let command = syn::parse_str::<VendorCommand>(
            r#"
                GapSetIoCapability(cgid = 0x1, cid = 0x05) {
                    Params = { io_capability: IoCapability => 1, };
                    Completion = CommandComplete;
                    Return = ();
                }
            "#,
        )
        .unwrap();
        assert_eq!(command.name, "GapSetIoCapability");
        assert_eq!(command.ocf(), 0x085);
        assert_eq!(command.params.min_len(), 1);
        assert_eq!(command.params.max_len(), 1);
        assert_eq!(command.completion, Completion::CommandComplete);
        assert_eq!(command.returns.unwrap().max_len(), 0);
    }

    #[test]
    fn parses_variable_shapes_and_constraints_together() {
        let command = syn::parse_str::<VendorCommand>(
            r#"
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        limit: u8 => 1,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8 => 1,
                            max_len: 16,
                        },
                    };
                    Constraints = {
                        range(limit, 0, 16);
                        len_at_most(data, limit);
                        non_empty(data);
                    };
                    Completion = CommandStatus;
                }
            "#,
        )
        .unwrap();
        assert_eq!(command.params.min_len(), 2);
        assert_eq!(command.params.max_len(), 18);
        assert_eq!(
            command.constraints.unwrap().referenced_fields(),
            &BTreeSet::from(["data".to_owned(), "limit".to_owned()])
        );
    }

    #[test]
    fn rejects_cross_section_invalid_states() {
        for (source, expected) in [
            (
                "Bad(cgid = 0, cid = 1) { Params = (); Completion = CommandComplete; }",
                "CommandComplete requires a Return declaration",
            ),
            (
                "Bad(cgid = 0, cid = 1) { Params = (); Completion = CommandStatus; Return = (); }",
                "declares CommandStatus and must not declare Return",
            ),
            (
                "Bad(cgid = 0, cid = 1) { Params = { value: u8 => 1, }; Constraints = { range(missing, 0, 1); }; Completion = CommandStatus; }",
                "unknown parameter(s): missing",
            ),
        ] {
            let error = syn::parse_str::<VendorCommand>(source)
                .err()
                .expect("fixture must be rejected")
                .to_string();
            assert!(error.contains(expected), "{error}");
        }
    }
}
