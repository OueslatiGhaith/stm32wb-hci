//! Shared syntax and validation for declarative STM32WB protocol schemas.
//!
//! This crate is host-only. Both the procedural macros and the compliance
//! checker consume this parser, which prevents either side from silently
//! accepting a command declaration that the other interprets differently.

mod firmware;
mod wire_type;

pub use firmware::{
    FirmwareFeatureError, FirmwareManifestError, FirmwareVersion, FirmwareVersionError,
};
pub use wire_type::{
    BitflagsWireType, ClosedEnumWireType, CompositeWireType, OpenEnumWireType, OpenScalarWireType,
    PrimitiveWireType, RangedWireType, SemanticWireType, TransparentWireType, WireAdapter,
    WireAdapters, WireCompositeField, WireEnumVariant, WireFlag, WireSentinel, WireTypeDeclaration,
};

use std::collections::{BTreeMap, BTreeSet};

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

/// A complete parsed `vendor_event!` catalog.
pub struct VendorEvents {
    /// Event declarations in source order.
    pub events: Vec<VendorEvent>,
}

/// One vendor-event declaration and its payload schema.
pub struct VendorEvent {
    /// Documentation and firmware cfg attributes attached to the event.
    pub attrs: Vec<syn::Attribute>,
    /// Generated payload type and `VendorEvent` variant name.
    pub name: syn::Ident,
    /// Original event-code literal, retaining its spelling and span.
    pub code_literal: LitInt,
    /// Parsed 16-bit STM32 vendor event code.
    pub code: u16,
    /// Complete event payload schema.
    pub payload: EventPayload,
}

/// Unit or inline-field event payload.
pub enum EventPayload {
    /// `Payload = ();`
    Unit,
    /// `Payload = { ... };`
    Fields(Fields),
}

impl EventPayload {
    /// Parsed fields, if the payload is not unit.
    pub const fn fields(&self) -> Option<&Fields> {
        match self {
            Self::Unit => None,
            Self::Fields(fields) => Some(fields),
        }
    }

    /// Minimum encoded payload size, excluding the two-byte event code.
    pub fn min_len(&self) -> usize {
        self.fields().map_or(0, Fields::min_len)
    }

    /// Maximum encoded payload size, excluding the two-byte event code.
    pub fn max_len(&self) -> usize {
        self.fields().map_or(0, Fields::max_len)
    }
}

impl VendorCommand {
    /// The ten-bit vendor OCF derived from `cgid` and `cid`.
    pub const fn ocf(&self) -> u16 {
        (self.cgid << 7) | self.cid
    }
}

/// Request payload syntax, including its optional borrowing lifetime.
pub struct Params {
    /// Lifetime declared by `Params<'a>`, if present.
    pub lifetime: Option<syn::Lifetime>,
    /// Unit or inline-field request shape.
    pub shape: ParamsShape,
}

/// Unit or inline-field request payload.
pub enum ParamsShape {
    /// `Params = ();`
    Unit,
    /// `Params = { ... };` or `Params<'a> = { ... };`
    Fields(Fields),
}

impl Params {
    /// Parsed field metadata, if the request is not unit.
    pub const fn fields(&self) -> Option<&Fields> {
        match &self.shape {
            ParamsShape::Unit => None,
            ParamsShape::Fields(fields) => Some(fields),
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
    fields: Vec<Field>,
    names: BTreeSet<String>,
    min_len: usize,
    max_len: usize,
}

impl Fields {
    /// Lossless typed fields in declaration order.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

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
}

/// One typed field in a Params, Return, or event payload body.
pub struct Field {
    /// Field binding and generated member name.
    pub name: syn::Ident,
    /// Semantic Rust type from the declaration.
    pub ty: Type,
    /// Fixed-width or variable wire encoding.
    pub encoding: FieldEncoding,
}

/// Wire encoding declared after a field's `=>` token.
pub enum FieldEncoding {
    /// One canonical fixed-width HCI field.
    Fixed(FixedEncoding),
    /// A variable schema body retained losslessly for later code generation.
    Variable(Box<VariableEncoding>),
}

/// Fixed-width field encoding.
pub struct FixedEncoding {
    /// Original integer literal, retaining its span and spelling.
    pub width_literal: LitInt,
    /// Parsed width used by envelope validation.
    pub width: usize,
}

/// Validated variable field encoding.
pub struct VariableEncoding {
    /// Complete typed encoding schema used by code generation.
    pub shape: VariableEncodingShape,
    /// Minimum encoded field size.
    pub min_len: usize,
    /// Maximum encoded field size.
    pub max_len: usize,
    /// Whether this field consumes every remaining payload byte.
    pub consumes_remainder: bool,
}

/// Complete schema for one variable-width field.
pub enum VariableEncodingShape {
    /// A fixed-width count followed by that many bytes.
    CountedBytes {
        /// Count type and its wire width.
        count: WireType,
        /// Minimum accepted byte count.
        min_len: IntegerValue,
        /// Maximum accepted byte count.
        max_len: IntegerValue,
    },
    /// A fixed-width count followed by that many fixed-width items.
    CountedItems {
        /// Count type and its wire width.
        count: WireType,
        /// Item type and its wire width.
        item: WireType,
        /// Minimum accepted item count.
        min_items: IntegerValue,
        /// Maximum accepted item count.
        max_items: IntegerValue,
    },
    /// A discriminator followed by a variant-specific fixed payload.
    Tagged(TaggedEncoding),
    /// A record width and byte length followed by homogeneous raw records.
    LengthPrefixedRecords {
        /// Record-width type and its wire width.
        record_len: WireType,
        /// Byte-length type and its wire width.
        length: WireType,
        /// Smallest valid record width.
        min_record_len: IntegerValue,
        /// Maximum accepted combined record byte length.
        max_len: IntegerValue,
    },
    /// A tag and byte length followed by tag-selected fixed-width items.
    TaggedItems(TaggedItemsEncoding),
    /// A bounded field that consumes every remaining byte.
    TrailingBytes {
        /// Minimum accepted byte count.
        min_len: IntegerValue,
        /// Maximum accepted byte count.
        max_len: IntegerValue,
    },
    /// Fixed-width items selected by set bits in an earlier bitmap field.
    BitmapItems {
        /// Earlier request field whose bits select the encoded items.
        bitmap: syn::Ident,
        /// Bits that participate in item selection.
        mask: IntegerValue,
        /// Item type and its wire width.
        item: WireType,
        /// Number of selectable items; validated against the mask population.
        max_items: IntegerValue,
    },
}

/// A semantic Rust type paired with its fixed wire width.
pub struct WireType {
    /// Semantic Rust type from the declaration.
    pub ty: Type,
    /// Fixed encoded width.
    pub width: IntegerValue,
}

/// An integer literal together with its validated `usize` value.
pub struct IntegerValue {
    /// Original literal, retaining its spelling and span for generated code.
    pub literal: LitInt,
    /// Parsed value used for validation and envelope arithmetic.
    pub value: usize,
}

/// Tagged-union encoding details.
pub struct TaggedEncoding {
    /// Discriminator type and wire width.
    pub tag: WireType,
    /// Variants in declaration order.
    pub variants: Vec<TaggedVariant>,
    /// Declared and validated minimum encoded length.
    pub min_len: IntegerValue,
    /// Declared and validated maximum encoded length.
    pub max_len: IntegerValue,
}

/// One tagged-union match arm and its fixed payload fields.
pub struct TaggedVariant {
    /// Refutable pattern used to select and bind the source variant.
    pub pattern: syn::Pat,
    /// Discriminator value emitted for this variant.
    pub tag: IntegerValue,
    /// Fixed-width fields bound by `pattern` and encoded after the tag.
    pub fields: Fields,
}

/// Tag-selected fixed-width item encoding details.
pub struct TaggedItemsEncoding {
    /// Discriminator type and wire width.
    pub tag: WireType,
    /// Combined item-byte length type and wire width.
    pub length: WireType,
    /// Variants in declaration order.
    pub variants: Vec<TaggedItemsVariant>,
    /// Maximum accepted combined item byte length.
    pub max_len: IntegerValue,
}

/// One tag-selected fixed-width item variant.
pub struct TaggedItemsVariant {
    /// Discriminator value selecting this item representation.
    pub tag: IntegerValue,
    /// Item type and its exact wire width.
    pub item: WireType,
    /// Maximum item count for this representation.
    pub max_items: IntegerValue,
}

/// Completion mechanism declared by a command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    /// The controller returns a Command Complete event and a return payload.
    CommandComplete,
    /// The controller first returns Command Status and completes asynchronously.
    CommandStatus,
}

/// Parsed constraints in declaration order.
pub struct Constraints {
    nodes: Vec<Constraint>,
    referenced_fields: BTreeSet<String>,
}

impl Constraints {
    /// Structured constraint nodes in source order.
    pub fn nodes(&self) -> &[Constraint] {
        &self.nodes
    }

    /// Every request field referenced by the constraint body.
    pub const fn referenced_fields(&self) -> &BTreeSet<String> {
        &self.referenced_fields
    }
}

/// One semantic relationship between command parameters.
pub enum Constraint {
    /// Require `minimum <= maximum`.
    Ordered {
        minimum: syn::Ident,
        maximum: syn::Ident,
    },
    /// Require ordering only when both operands are inside the inclusive range.
    OrderedWhenInRange {
        minimum: syn::Ident,
        maximum: syn::Ident,
        range_minimum: Expr,
        range_maximum: Expr,
    },
    /// Require a field to be inside an inclusive range.
    Range {
        field: syn::Ident,
        minimum: Expr,
        maximum: Expr,
    },
    /// Require a field to equal one expression from a nonempty set.
    OneOf {
        field: syn::Ident,
        allowed: Vec<Expr>,
    },
    /// Require a field to be in a sparse set or an inclusive range.
    OneOfOrRange {
        field: syn::Ident,
        allowed: Vec<Expr>,
        minimum: Expr,
        maximum: Expr,
    },
    /// Require both fields to equal a sentinel value, or neither field to do so.
    PairedValue {
        left: syn::Ident,
        right: syn::Ident,
        value: Expr,
    },
    /// Require an exact dependent value when a selector has one value.
    ImpliesEq {
        selector: syn::Ident,
        selected: Expr,
        field: syn::Ident,
        required: Expr,
    },
    /// Require a dependent field range when a selector has one value.
    ImpliesRange {
        selector: syn::Ident,
        selected: Expr,
        field: syn::Ident,
        minimum: Expr,
        maximum: Expr,
    },
    /// Require a dependent field to be in a sparse set or range when selected.
    ImpliesOneOfOrRange {
        selector: syn::Ident,
        selected: Expr,
        field: syn::Ident,
        allowed: Vec<Expr>,
        minimum: Expr,
        maximum: Expr,
    },
    /// Require a dependent collection's minimum length when selected.
    ImpliesLenAtLeast {
        selector: syn::Ident,
        selected: Expr,
        field: syn::Ident,
        minimum: Expr,
    },
    /// Require a dependent collection's exact length when selected.
    ImpliesLenEq {
        selector: syn::Ident,
        selected: Expr,
        field: syn::Ident,
        required: Expr,
    },
    /// Require a collection length to equal a semantic scalar field.
    LenEq {
        field: syn::Ident,
        expected: syn::Ident,
    },
    /// Require a collection's runtime length not to exceed another field.
    LenAtMost {
        field: syn::Ident,
        maximum: syn::Ident,
    },
    /// Require `offset + collection.len()` not to exceed a total-length field.
    OffsetLenAtMost {
        offset: syn::Ident,
        field: syn::Ident,
        total: syn::Ident,
    },
    /// Require a collection or bitflags field to be nonempty.
    NonEmpty { field: syn::Ident },
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
                    let lifetime = syn::parse2::<ParamsLifetime>(header)?.0;
                    params = Some(parse_params(value, lifetime)?);
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
        if params.fields().is_some_and(|fields| {
            fields.fields().iter().any(|field| {
                let FieldEncoding::Variable(encoding) = &field.encoding else {
                    return false;
                };
                matches!(
                    encoding.shape,
                    VariableEncodingShape::LengthPrefixedRecords { .. }
                        | VariableEncodingShape::TaggedItems(_)
                )
            })
        }) {
            return Err(syn::Error::new(
                invocation.name.span(),
                "Params uses an event-only variable encoding",
            ));
        }
        let has_variable_params = params.fields().is_some_and(|fields| {
            fields
                .fields()
                .iter()
                .any(|field| matches!(field.encoding, FieldEncoding::Variable(_)))
        });
        if params.lifetime.is_none() && has_variable_params {
            return Err(syn::Error::new(
                invocation.name.span(),
                "variable Params must declare a lifetime",
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
            .is_some_and(|fields| {
                fields.fields().iter().any(|field| {
                    let FieldEncoding::Variable(encoding) = &field.encoding else {
                        return false;
                    };
                    matches!(
                        encoding.shape,
                        VariableEncodingShape::Tagged(_)
                            | VariableEncodingShape::BitmapItems { .. }
                            | VariableEncodingShape::LengthPrefixedRecords { .. }
                            | VariableEncodingShape::TaggedItems(_)
                    )
                })
            })
        {
            return Err(syn::Error::new(
                invocation.name.span(),
                "Return uses a variable encoding that has no owned decoder",
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

impl Parse for VendorEvents {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut events = Vec::new();
        let mut names = BTreeMap::new();
        let mut codes = BTreeMap::new();

        while !input.is_empty() {
            let attrs = input.call(syn::Attribute::parse_outer)?;
            let mut cfg_count = 0usize;
            for attr in &attrs {
                if attr.path().is_ident("cfg") {
                    cfg_count += 1;
                } else if !attr.path().is_ident("doc") {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "vendor events accept only documentation and cfg attributes",
                    ));
                }
            }
            if cfg_count > 1 {
                return Err(input.error("vendor events accept at most one cfg attribute"));
            }

            let partition = firmware_partition(&attrs);
            let name = input.parse::<syn::Ident>()?;
            let previous_partitions = names.entry(name.to_string()).or_insert_with(Vec::new);
            if previous_partitions
                .iter()
                .any(|previous| !firmware_partitions_are_complementary(previous, &partition))
            {
                return Err(syn::Error::new_spanned(
                    &name,
                    "duplicate vendor event name must use complementary \
                     `before_fw_*` and `since_fw_*` cfg attributes",
                ));
            }
            previous_partitions.push(partition);

            let arguments;
            syn::parenthesized!(arguments in input);
            let code_literal = arguments.parse::<LitInt>()?;
            let code = parse_u16_literal(&code_literal).map_err(|error| {
                syn::Error::new_spanned(
                    &code_literal,
                    format!("invalid vendor event code: {error}"),
                )
            })?;
            if !arguments.is_empty() {
                return Err(arguments.error("event code must be one integer literal"));
            }
            let previous_partitions = codes.entry(code).or_insert_with(Vec::new);
            if previous_partitions
                .iter()
                .any(|previous| !firmware_partitions_are_complementary(previous, &partition))
            {
                return Err(syn::Error::new_spanned(
                    &code_literal,
                    format!(
                        "duplicate vendor event code 0x{code:04X} must use complementary \
                         `before_fw_*` and `since_fw_*` cfg attributes"
                    ),
                ));
            }
            previous_partitions.push(partition);

            let body;
            syn::braced!(body in input);
            let payload_label = body.parse::<syn::Ident>()?;
            if payload_label != "Payload" {
                return Err(body.error(format!("expected `Payload`, found `{payload_label}`")));
            }
            body.parse::<syn::Token![=]>()?;
            let payload = if body.peek(syn::token::Brace) {
                let fields;
                syn::braced!(fields in body);
                EventPayload::Fields(fields.parse::<Fields>()?)
            } else if body.peek(syn::token::Paren) {
                let unit;
                syn::parenthesized!(unit in body);
                if !unit.is_empty() {
                    return Err(unit.error("unit event payload must be `()`"));
                }
                EventPayload::Unit
            } else {
                return Err(body.error("event Payload must be `()` or a declarative field body"));
            };
            body.parse::<syn::Token![;]>()?;
            if !body.is_empty() {
                return Err(body.error("unexpected tokens after event Payload"));
            }

            if payload.max_len() > 253 {
                return Err(syn::Error::new_spanned(
                    &name,
                    format!(
                        "vendor event payload is at most 253 bytes, but this schema allows {}",
                        payload.max_len(),
                    ),
                ));
            }
            if payload.fields().is_some_and(|fields| {
                fields.fields().iter().any(|field| {
                    let FieldEncoding::Variable(encoding) = &field.encoding else {
                        return false;
                    };
                    matches!(
                        encoding.shape,
                        VariableEncodingShape::Tagged(_)
                            | VariableEncodingShape::BitmapItems { .. }
                    )
                })
            }) {
                return Err(syn::Error::new_spanned(
                    &name,
                    "vendor event payload uses a variable encoding that has no owned decoder",
                ));
            }

            events.push(VendorEvent {
                attrs,
                name,
                code_literal,
                code,
                payload,
            });
        }

        if events.is_empty() {
            return Err(input.error("vendor_event! must declare at least one event"));
        }
        Ok(Self { events })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FirmwarePartitionSide {
    Before,
    Since,
}

#[derive(Clone, Copy)]
struct FirmwarePartition {
    side: FirmwarePartitionSide,
    boundary: FirmwareVersion,
}

/// Return the simple firmware partition carried by an event declaration.
///
/// Duplicate event names or codes are useful when a firmware release changes a
/// wire payload. They are safe only when the declarations are exact halves of
/// one canonical version boundary. Complex cfg expressions deliberately return
/// `None`: proving that arbitrary boolean cfg expressions do not overlap is
/// outside the schema parser's scope, so such expressions cannot justify a
/// duplicate declaration.
fn firmware_partition(attrs: &[syn::Attribute]) -> Option<FirmwarePartition> {
    let cfg = attrs.iter().find(|attr| attr.path().is_ident("cfg"))?;
    let path = cfg.parse_args::<syn::Path>().ok()?;
    let ident = path.get_ident()?.to_string();

    let (side, feature) = if let Some(version) = ident.strip_prefix("before_") {
        (FirmwarePartitionSide::Before, version)
    } else if let Some(version) = ident.strip_prefix("since_") {
        (FirmwarePartitionSide::Since, version)
    } else {
        return None;
    };
    let boundary = FirmwareVersion::from_feature_name(feature).ok()?;
    Some(FirmwarePartition { side, boundary })
}

fn firmware_partitions_are_complementary(
    left: &Option<FirmwarePartition>,
    right: &Option<FirmwarePartition>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.side != right.side && left.boundary == right.boundary,
        _ => false,
    }
}

struct ParamsLifetime(Option<syn::Lifetime>);

impl Parse for ParamsLifetime {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self(None));
        }
        input.parse::<syn::Token![<]>()?;
        let lifetime = input.parse::<syn::Lifetime>()?;
        input.parse::<syn::Token![>]>()?;
        if !input.is_empty() {
            return Err(input.error("Params accepts at most one lifetime parameter"));
        }
        Ok(Self(Some(lifetime)))
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

fn parse_params(value: TokenStream, lifetime: Option<syn::Lifetime>) -> syn::Result<Params> {
    if is_unit_type(&value) {
        if lifetime.is_some() {
            return Err(syn::Error::new_spanned(
                value,
                "unit Params must not declare a lifetime",
            ));
        }
        return Ok(Params {
            lifetime,
            shape: ParamsShape::Unit,
        });
    }
    parse_braced_fields(value).map(|fields| Params {
        lifetime,
        shape: ParamsShape::Fields(fields),
    })
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
        if input.is_empty() {
            return Err(input.error("Constraints must declare at least one check"));
        }
        let mut nodes = Vec::new();
        let mut referenced_fields = BTreeSet::new();
        while !input.is_empty() {
            let kind = input.parse::<syn::Ident>()?;
            let arguments;
            syn::parenthesized!(arguments in input);

            let node = match kind.to_string().as_str() {
                "ordered" => {
                    let minimum = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let maximum = parse_field_reference(&arguments, &mut referenced_fields)?;
                    Constraint::Ordered { minimum, maximum }
                }
                "ordered_when_in_range" => {
                    let minimum = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let maximum = parse_field_reference(&arguments, &mut referenced_fields)?;
                    let (range_minimum, range_maximum) = parse_expression_pair(&arguments)?;
                    Constraint::OrderedWhenInRange {
                        minimum,
                        maximum,
                        range_minimum,
                        range_maximum,
                    }
                }
                "range" => {
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    let (minimum, maximum) = parse_expression_pair(&arguments)?;
                    Constraint::Range {
                        field,
                        minimum,
                        maximum,
                    }
                }
                "one_of" => {
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let allowed = parse_nonempty_expression_list(&arguments, "one_of")?;
                    Constraint::OneOf { field, allowed }
                }
                "one_of_or_range" => {
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let allowed = parse_nonempty_expression_list(&arguments, "one_of_or_range")?;
                    let (minimum, maximum) = parse_expression_pair(&arguments)?;
                    Constraint::OneOfOrRange {
                        field,
                        allowed,
                        minimum,
                        maximum,
                    }
                }
                "paired_value" => {
                    let left = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let right = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let value = arguments.parse::<Expr>()?;
                    Constraint::PairedValue { left, right, value }
                }
                "implies_eq" => {
                    let selector = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let selected = arguments.parse::<Expr>()?;
                    arguments.parse::<syn::Token![,]>()?;
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let required = arguments.parse::<Expr>()?;
                    Constraint::ImpliesEq {
                        selector,
                        selected,
                        field,
                        required,
                    }
                }
                "implies_range" => {
                    let selector = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let selected = arguments.parse::<Expr>()?;
                    arguments.parse::<syn::Token![,]>()?;
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    let (minimum, maximum) = parse_expression_pair(&arguments)?;
                    Constraint::ImpliesRange {
                        selector,
                        selected,
                        field,
                        minimum,
                        maximum,
                    }
                }
                "implies_one_of_or_range" => {
                    let selector = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let selected = arguments.parse::<Expr>()?;
                    arguments.parse::<syn::Token![,]>()?;
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let allowed =
                        parse_nonempty_expression_list(&arguments, "implies_one_of_or_range")?;
                    let (minimum, maximum) = parse_expression_pair(&arguments)?;
                    Constraint::ImpliesOneOfOrRange {
                        selector,
                        selected,
                        field,
                        allowed,
                        minimum,
                        maximum,
                    }
                }
                "implies_len_at_least" => {
                    let selector = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let selected = arguments.parse::<Expr>()?;
                    arguments.parse::<syn::Token![,]>()?;
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let minimum = arguments.parse::<Expr>()?;
                    Constraint::ImpliesLenAtLeast {
                        selector,
                        selected,
                        field,
                        minimum,
                    }
                }
                "implies_len_eq" => {
                    let selector = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let selected = arguments.parse::<Expr>()?;
                    arguments.parse::<syn::Token![,]>()?;
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let required = arguments.parse::<Expr>()?;
                    Constraint::ImpliesLenEq {
                        selector,
                        selected,
                        field,
                        required,
                    }
                }
                "len_eq" => {
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let expected = parse_field_reference(&arguments, &mut referenced_fields)?;
                    Constraint::LenEq { field, expected }
                }
                "len_at_most" => {
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let maximum = parse_field_reference(&arguments, &mut referenced_fields)?;
                    Constraint::LenAtMost { field, maximum }
                }
                "offset_len_at_most" => {
                    let offset = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    arguments.parse::<syn::Token![,]>()?;
                    let total = parse_field_reference(&arguments, &mut referenced_fields)?;
                    Constraint::OffsetLenAtMost {
                        offset,
                        field,
                        total,
                    }
                }
                "non_empty" => {
                    let field = parse_field_reference(&arguments, &mut referenced_fields)?;
                    Constraint::NonEmpty { field }
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        kind,
                        "unknown declarative constraint",
                    ));
                }
            };

            if !arguments.is_empty() {
                return Err(arguments.error("unexpected tokens in declarative constraint"));
            }
            input.parse::<syn::Token![;]>()?;
            nodes.push(node);
        }
        Ok(Self {
            nodes,
            referenced_fields,
        })
    }
}

fn parse_field_reference(
    input: ParseStream<'_>,
    fields: &mut BTreeSet<String>,
) -> syn::Result<syn::Ident> {
    let field = input.parse::<syn::Ident>()?;
    fields.insert(field.to_string());
    Ok(field)
}

fn parse_expression_pair(input: ParseStream<'_>) -> syn::Result<(Expr, Expr)> {
    input.parse::<syn::Token![,]>()?;
    let first = input.parse::<Expr>()?;
    input.parse::<syn::Token![,]>()?;
    let second = input.parse::<Expr>()?;
    Ok((first, second))
}

fn parse_nonempty_expression_list(input: ParseStream<'_>, kind: &str) -> syn::Result<Vec<Expr>> {
    let allowed;
    syn::bracketed!(allowed in input);
    let values = Punctuated::<Expr, syn::Token![,]>::parse_terminated(&allowed)?;
    if values.is_empty() {
        return Err(allowed.error(format!("{kind} must declare at least one allowed value")));
    }
    Ok(values.into_iter().collect())
}
impl Parse for Fields {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut fields = Vec::new();
        let mut names = BTreeSet::new();
        let mut min_len = 0usize;
        let mut max_len = 0usize;
        let mut consumes_remainder = false;

        while !input.is_empty() {
            if consumes_remainder {
                return Err(input.error("trailing_bytes must be the final declarative field"));
            }
            let name = input.parse::<syn::Ident>()?;
            if !names.insert(name.to_string()) {
                return Err(syn::Error::new_spanned(
                    &name,
                    "duplicate declarative field",
                ));
            }
            input.parse::<syn::Token![:]>()?;
            let ty = input.parse::<Type>()?;
            input.parse::<syn::Token![=>]>()?;

            let (encoding, field_min_len, field_max_len, field_consumes_remainder) =
                if input.peek(LitInt) {
                    let width_literal = input.parse::<LitInt>()?;
                    let width =
                        parse_usize_literal(&width_literal).map_err(|error| input.error(error))?;
                    (
                        FieldEncoding::Fixed(FixedEncoding {
                            width_literal,
                            width,
                        }),
                        width,
                        width,
                        false,
                    )
                } else if input.peek(syn::token::Brace) {
                    let shape;
                    syn::braced!(shape in input);
                    let encoding = shape.parse::<VariableEncoding>()?;
                    let field_min_len = encoding.min_len;
                    let field_max_len = encoding.max_len;
                    let field_consumes_remainder = encoding.consumes_remainder;
                    (
                        FieldEncoding::Variable(Box::new(encoding)),
                        field_min_len,
                        field_max_len,
                        field_consumes_remainder,
                    )
                } else {
                    return Err(input.error("expected a fixed width or variable field shape"));
                };

            consumes_remainder = field_consumes_remainder;
            min_len = min_len
                .checked_add(field_min_len)
                .ok_or_else(|| input.error("declarative field minimum length overflows usize"))?;
            max_len = max_len
                .checked_add(field_max_len)
                .ok_or_else(|| input.error("declarative field length overflows usize"))?;
            fields.push(Field { name, ty, encoding });
            input.parse::<syn::Token![,]>()?;
        }

        Ok(Self {
            fields,
            names,
            min_len,
            max_len,
        })
    }
}

impl Parse for VariableEncoding {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        parse_colon_label(input, "kind")?;
        let kind = input.parse::<syn::Ident>()?;
        input.parse::<syn::Token![,]>()?;

        let shape = match kind.to_string().as_str() {
            "counted_bytes" => {
                let count = parse_wire_type(input, "count")?;
                let min_len = if next_label_is(input, "min_len")? {
                    parse_integer_value(input, "min_len")?
                } else {
                    IntegerValue {
                        literal: LitInt::new("0", kind.span()),
                        value: 0,
                    }
                };
                let max_len = parse_integer_value(input, "max_len")?;
                validate_range(input, "counted_bytes", min_len.value, max_len.value)?;
                VariableEncodingShape::CountedBytes {
                    count,
                    min_len,
                    max_len,
                }
            }
            "counted_items" => {
                let count = parse_wire_type(input, "count")?;
                let item = parse_wire_type(input, "item")?;
                let min_items = if next_label_is(input, "min_items")? {
                    parse_integer_value(input, "min_items")?
                } else {
                    IntegerValue {
                        literal: LitInt::new("0", kind.span()),
                        value: 0,
                    }
                };
                let max_items = parse_integer_value(input, "max_items")?;
                validate_range(input, "counted_items", min_items.value, max_items.value)?;
                VariableEncodingShape::CountedItems {
                    count,
                    item,
                    min_items,
                    max_items,
                }
            }
            "tagged" => VariableEncodingShape::Tagged(parse_tagged_encoding(input)?),
            "length_prefixed_records" => {
                let record_len = parse_wire_type(input, "record_len")?;
                let length = parse_wire_type(input, "length")?;
                let min_record_len = parse_integer_value(input, "min_record_len")?;
                let max_len = parse_integer_value(input, "max_len")?;
                if min_record_len.value == 0 {
                    return Err(
                        input.error("length_prefixed_records minimum record length is zero")
                    );
                }
                validate_integer_capacity(input, "record byte length", &length, max_len.value)?;
                VariableEncodingShape::LengthPrefixedRecords {
                    record_len,
                    length,
                    min_record_len,
                    max_len,
                }
            }
            "tagged_items" => {
                VariableEncodingShape::TaggedItems(parse_tagged_items_encoding(input)?)
            }
            "trailing_bytes" => {
                let min_len = parse_integer_value(input, "min_len")?;
                let max_len = parse_integer_value(input, "max_len")?;
                validate_range(input, "trailing_bytes", min_len.value, max_len.value)?;
                VariableEncodingShape::TrailingBytes { min_len, max_len }
            }
            "bitmap_items" => {
                parse_colon_label(input, "bitmap")?;
                let bitmap = input.parse::<syn::Ident>()?;
                input.parse::<syn::Token![,]>()?;
                let mask = parse_integer_value(input, "mask")?;
                let item = parse_wire_type(input, "item")?;
                let max_items = parse_integer_value(input, "max_items")?;
                if mask.value.count_ones() as usize != max_items.value {
                    return Err(input.error(format!(
                        "bitmap mask selects {} bits but max_items is {}",
                        mask.value.count_ones(),
                        max_items.value,
                    )));
                }
                VariableEncodingShape::BitmapItems {
                    bitmap,
                    mask,
                    item,
                    max_items,
                }
            }
            _ => {
                return Err(input.error(format!("unknown declarative variable kind `{kind}`")));
            }
        };

        let (min_len, max_len, consumes_remainder) = variable_bounds(input, &shape)?;
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after declarative variable field"));
        }
        Ok(Self {
            shape,
            min_len,
            max_len,
            consumes_remainder,
        })
    }
}

fn variable_bounds(
    input: ParseStream<'_>,
    shape: &VariableEncodingShape,
) -> syn::Result<(usize, usize, bool)> {
    let bounds = match shape {
        VariableEncodingShape::CountedBytes {
            count,
            min_len,
            max_len,
        } => (
            count
                .width
                .value
                .checked_add(min_len.value)
                .ok_or_else(|| input.error("counted_bytes minimum field length overflows usize"))?,
            count.width.value.checked_add(max_len.value),
            false,
        ),
        VariableEncodingShape::CountedItems {
            count,
            item,
            min_items,
            max_items,
        } => (
            item.width
                .value
                .checked_mul(min_items.value)
                .and_then(|items| count.width.value.checked_add(items))
                .ok_or_else(|| input.error("counted_items minimum field length overflows usize"))?,
            item.width
                .value
                .checked_mul(max_items.value)
                .and_then(|items| count.width.value.checked_add(items)),
            false,
        ),
        VariableEncodingShape::Tagged(tagged) => {
            (tagged.min_len.value, Some(tagged.max_len.value), false)
        }
        VariableEncodingShape::LengthPrefixedRecords {
            record_len,
            length,
            max_len,
            ..
        } => (
            record_len
                .width
                .value
                .checked_add(length.width.value)
                .ok_or_else(|| {
                    input.error("length_prefixed_records prefix length overflows usize")
                })?,
            record_len
                .width
                .value
                .checked_add(length.width.value)
                .and_then(|prefix| prefix.checked_add(max_len.value)),
            false,
        ),
        VariableEncodingShape::TaggedItems(tagged) => (
            tagged
                .tag
                .width
                .value
                .checked_add(tagged.length.width.value)
                .ok_or_else(|| input.error("tagged_items prefix length overflows usize"))?,
            tagged
                .tag
                .width
                .value
                .checked_add(tagged.length.width.value)
                .and_then(|prefix| prefix.checked_add(tagged.max_len.value)),
            false,
        ),
        VariableEncodingShape::TrailingBytes { min_len, max_len } => {
            (min_len.value, Some(max_len.value), true)
        }
        VariableEncodingShape::BitmapItems {
            item, max_items, ..
        } => (0, item.width.value.checked_mul(max_items.value), false),
    };
    let max_len = bounds
        .1
        .ok_or_else(|| input.error("declarative variable field length overflows usize"))?;
    Ok((bounds.0, max_len, bounds.2))
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

fn next_label_is(input: ParseStream<'_>, expected: &str) -> syn::Result<bool> {
    if !input.peek(syn::Ident) {
        return Ok(false);
    }
    let fork = input.fork();
    Ok(fork.parse::<syn::Ident>()? == expected)
}

fn validate_integer_capacity(
    input: ParseStream<'_>,
    label: &str,
    wire_type: &WireType,
    maximum: usize,
) -> syn::Result<()> {
    let width = wire_type.width.value;
    if width == 0 {
        return Err(input.error(format!("{label} wire width must be nonzero")));
    }
    if width < core::mem::size_of::<usize>() {
        let capacity = (1usize << (width * u8::BITS as usize)) - 1;
        if maximum > capacity {
            return Err(input.error(format!(
                "{label} maximum {maximum} does not fit in {width} bytes"
            )));
        }
    }
    Ok(())
}

fn parse_tagged_encoding(input: ParseStream<'_>) -> syn::Result<TaggedEncoding> {
    let tag = parse_wire_type(input, "tag")?;
    parse_colon_label(input, "variants")?;
    let variants;
    syn::braced!(variants in input);
    input.parse::<syn::Token![,]>()?;

    let mut tags = BTreeSet::new();
    let mut parsed_variants = Vec::new();
    let mut variant_min = None::<usize>;
    let mut variant_max = None::<usize>;
    while !variants.is_empty() {
        let pattern = variants.call(syn::Pat::parse_single)?;
        let mut bindings = PatternBindings::default();
        bindings.visit_pat(&pattern);
        variants.parse::<syn::Token![=>]>()?;
        let body;
        syn::braced!(body in variants);

        let variant_tag = parse_integer_value(&body, "tag")?;
        if !tags.insert(variant_tag.value) {
            return Err(variants.error(format!(
                "duplicate tagged variant value {:#x}",
                variant_tag.value,
            )));
        }
        if tag.width.value < core::mem::size_of::<usize>()
            && variant_tag.value >= (1usize << (tag.width.value * u8::BITS as usize))
        {
            return Err(variants.error(format!(
                "tag value {:#x} does not fit in {} bytes",
                variant_tag.value, tag.width.value,
            )));
        }

        parse_colon_label(&body, "fields")?;
        let fields;
        syn::braced!(fields in body);
        body.parse::<syn::Token![,]>()?;
        if !body.is_empty() {
            return Err(body.error("unexpected tokens after tagged variant fields"));
        }
        let payload = fields.parse::<Fields>()?;
        if payload
            .fields()
            .iter()
            .any(|field| !matches!(field.encoding, FieldEncoding::Fixed(_)))
        {
            return Err(fields.error("tagged variant payload fields must be fixed-width"));
        }
        for field in payload.names() {
            if !bindings.names.contains(field) {
                return Err(fields.error(format!(
                    "tagged payload field `{field}` is not bound by its variant pattern"
                )));
            }
        }
        let wire_len = tag
            .width
            .value
            .checked_add(payload.max_len())
            .ok_or_else(|| variants.error("tagged variant length overflows usize"))?;
        variant_min = Some(variant_min.map_or(wire_len, |value| value.min(wire_len)));
        variant_max = Some(variant_max.map_or(wire_len, |value| value.max(wire_len)));
        parsed_variants.push(TaggedVariant {
            pattern,
            tag: variant_tag,
            fields: payload,
        });
        variants.parse::<syn::Token![,]>()?;
    }

    let Some(computed_min) = variant_min else {
        return Err(input.error("tagged field must declare at least one variant"));
    };
    let computed_max = variant_max.expect("minimum and maximum are populated together");
    let declared_min = parse_integer_value(input, "min_len")?;
    let declared_max = parse_integer_value(input, "max_len")?;
    if (declared_min.value, declared_max.value) != (computed_min, computed_max) {
        return Err(input.error(format!(
            "tagged field declares lengths {}..={}, but its variants require {computed_min}..={computed_max}",
            declared_min.value, declared_max.value,
        )));
    }
    Ok(TaggedEncoding {
        tag,
        variants: parsed_variants,
        min_len: declared_min,
        max_len: declared_max,
    })
}

fn parse_tagged_items_encoding(input: ParseStream<'_>) -> syn::Result<TaggedItemsEncoding> {
    let tag = parse_wire_type(input, "tag")?;
    let length = parse_wire_type(input, "length")?;
    parse_colon_label(input, "variants")?;
    let variants;
    syn::braced!(variants in input);
    input.parse::<syn::Token![,]>()?;

    let mut tags = BTreeSet::new();
    let mut parsed_variants = Vec::new();
    while !variants.is_empty() {
        let variant_tag = parse_integer_literal_value(&variants)?;
        if !tags.insert(variant_tag.value) {
            return Err(variants.error(format!(
                "duplicate tagged_items variant value {:#x}",
                variant_tag.value,
            )));
        }
        validate_integer_capacity(&variants, "tagged_items tag", &tag, variant_tag.value)?;
        variants.parse::<syn::Token![=>]>()?;

        let body;
        syn::braced!(body in variants);
        let item = parse_wire_type(&body, "item")?;
        if item.width.value == 0 {
            return Err(body.error("tagged_items item wire width must be nonzero"));
        }
        let max_items = parse_integer_value(&body, "max_items")?;
        if !body.is_empty() {
            return Err(body.error("unexpected tokens after tagged_items variant"));
        }
        parsed_variants.push(TaggedItemsVariant {
            tag: variant_tag,
            item,
            max_items,
        });
        variants.parse::<syn::Token![,]>()?;
    }
    if parsed_variants.is_empty() {
        return Err(input.error("tagged_items must declare at least one variant"));
    }

    let max_len = parse_integer_value(input, "max_len")?;
    validate_integer_capacity(input, "tagged_items byte length", &length, max_len.value)?;
    for variant in &parsed_variants {
        let expected = max_len.value / variant.item.width.value;
        if variant.max_items.value != expected {
            return Err(input.error(format!(
                "tagged_items variant {:#x} declares {} items, but max_len {} and item width {} allow {expected}",
                variant.tag.value,
                variant.max_items.value,
                max_len.value,
                variant.item.width.value,
            )));
        }
    }

    Ok(TaggedItemsEncoding {
        tag,
        length,
        variants: parsed_variants,
        max_len,
    })
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

fn parse_wire_type(input: ParseStream<'_>, label: &str) -> syn::Result<WireType> {
    parse_colon_label(input, label)?;
    let ty = input.parse::<Type>()?;
    input.parse::<syn::Token![=>]>()?;
    let width = parse_integer_literal_value(input)?;
    input.parse::<syn::Token![,]>()?;
    Ok(WireType { ty, width })
}

fn parse_integer_value(input: ParseStream<'_>, label: &str) -> syn::Result<IntegerValue> {
    parse_colon_label(input, label)?;
    let value = parse_integer_literal_value(input)?;
    input.parse::<syn::Token![,]>()?;
    Ok(value)
}

fn parse_integer_literal_value(input: ParseStream<'_>) -> syn::Result<IntegerValue> {
    let literal = input.parse::<LitInt>()?;
    let value = parse_usize_literal(&literal).map_err(|error| input.error(error))?;
    Ok(IntegerValue { literal, value })
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
        assert!(command.params.lifetime.is_none());
        let [field] = command.params.fields().unwrap().fields() else {
            panic!("expected one typed Params field");
        };
        assert_eq!(field.name, "io_capability");
        let FieldEncoding::Fixed(encoding) = &field.encoding else {
            panic!("expected a fixed encoding");
        };
        assert_eq!(encoding.width, 1);
        assert_eq!(encoding.width_literal.base10_digits(), "1");
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
        assert_eq!(command.params.lifetime.as_ref().unwrap().ident, "a");
        let fields = command.params.fields().unwrap().fields();
        assert_eq!(fields.len(), 2);
        let FieldEncoding::Variable(encoding) = &fields[1].encoding else {
            panic!("expected a variable encoding");
        };
        assert_eq!((encoding.min_len, encoding.max_len), (1, 17));
        let VariableEncodingShape::CountedBytes {
            count,
            min_len,
            max_len,
        } = &encoding.shape
        else {
            panic!("expected typed counted-bytes metadata");
        };
        assert_eq!(count.width.value, 1);
        assert_eq!(min_len.value, 0);
        assert_eq!(max_len.value, 16);
        let constraints = command.constraints.as_ref().unwrap();
        assert_eq!(
            constraints.referenced_fields(),
            &BTreeSet::from(["data".to_owned(), "limit".to_owned()])
        );
        assert!(matches!(constraints.nodes()[0], Constraint::Range { .. }));
        assert!(matches!(
            constraints.nodes()[1],
            Constraint::LenAtMost { .. }
        ));
        assert!(matches!(
            constraints.nodes()[2],
            Constraint::NonEmpty { .. }
        ));
    }

    #[test]
    fn parses_offset_plus_length_constraint() {
        let command = syn::parse_str::<VendorCommand>(
            r#"
                Chunk(cgid = 0x0, cid = 0x04) {
                    Params<'a> = {
                        total: u16 => 2,
                        offset: u16 => 2,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8 => 1,
                            max_len: 16,
                        },
                    };
                    Constraints = {
                        offset_len_at_most(offset, data, total);
                    };
                    Completion = CommandComplete;
                    Return = ();
                }
            "#,
        )
        .unwrap();

        let constraints = command.constraints.as_ref().unwrap();
        assert_eq!(
            constraints.referenced_fields(),
            &BTreeSet::from(["data".to_owned(), "offset".to_owned(), "total".to_owned(),])
        );
        assert!(matches!(
            constraints.nodes(),
            [Constraint::OffsetLenAtMost { .. }]
        ));
    }

    #[test]
    fn parses_selector_dependent_domain_and_length_constraints() {
        let command = syn::parse_str::<VendorCommand>(
            r#"
                Conditional(cgid = 0x0, cid = 0x04) {
                    Params<'a> = {
                        mode: u8 => 1,
                        error: u8 => 1,
                        limit: u8 => 1,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8 => 1,
                            max_len: 16,
                        },
                    };
                    Constraints = {
                        implies_one_of_or_range(mode, 1, error, [8], 0x80, 0x9F);
                        implies_len_at_least(mode, 2, data, 9);
                        implies_len_eq(mode, 3, data, 6);
                        len_eq(data, limit);
                    };
                    Completion = CommandComplete;
                    Return = ();
                }
            "#,
        )
        .unwrap();

        let constraints = command.constraints.as_ref().unwrap();
        assert_eq!(
            constraints.referenced_fields(),
            &BTreeSet::from([
                "data".to_owned(),
                "error".to_owned(),
                "limit".to_owned(),
                "mode".to_owned(),
            ])
        );
        assert!(matches!(
            constraints.nodes()[0],
            Constraint::ImpliesOneOfOrRange { .. }
        ));
        assert!(matches!(
            constraints.nodes()[1],
            Constraint::ImpliesLenAtLeast { .. }
        ));
        assert!(matches!(
            constraints.nodes()[2],
            Constraint::ImpliesLenEq { .. }
        ));
        assert!(matches!(constraints.nodes()[3], Constraint::LenEq { .. }));
    }

    #[test]
    fn parses_complete_vendor_event_catalog_shapes() {
        let catalog = syn::parse_str::<VendorEvents>(
            r#"
                /// Unit event.
                Unit(0x0001) { Payload = (); }
                #[cfg(since_fw_0_17_0)]
                Counted(0x0002) {
                    Payload = {
                        handle: u16 => 2,
                        bytes: BoundedBytes<8> => {
                            kind: counted_bytes,
                            count: u8 => 1,
                            max_len: 8,
                        },
                    };
                }
                Items(0x0003) {
                    Payload = {
                        values: BoundedItems<Item, 3> => {
                            kind: counted_items,
                            count: u8 => 1,
                            item: Item => 2,
                            min_items: 1,
                            max_items: 3,
                        },
                    };
                }
                Records(0x0004) {
                    Payload = {
                        value: Records => {
                            kind: length_prefixed_records,
                            record_len: u8 => 1,
                            length: u8 => 1,
                            min_record_len: 2,
                            max_len: 8,
                        },
                    };
                }
                TaggedItems(0x0005) {
                    Payload = {
                        value: TaggedItems => {
                            kind: tagged_items,
                            tag: u8 => 1,
                            length: u8 => 1,
                            variants: {
                                1 => { item: Short => 2, max_items: 4, },
                                2 => { item: Long => 4, max_items: 2, },
                            },
                            max_len: 8,
                        },
                    };
                }
                Trailing(0x0006) {
                    Payload = {
                        value: BoundedBytes<4> => {
                            kind: trailing_bytes,
                            min_len: 0,
                            max_len: 4,
                        },
                    };
                }
            "#,
        )
        .unwrap();

        assert_eq!(catalog.events.len(), 6);
        assert!(matches!(catalog.events[0].payload, EventPayload::Unit));
        assert_eq!(catalog.events[1].code, 0x0002);
        assert_eq!(catalog.events[1].attrs.len(), 1);
        assert_eq!(
            (
                catalog.events[1].payload.min_len(),
                catalog.events[1].payload.max_len()
            ),
            (3, 11),
        );
        assert_eq!(
            (
                catalog.events[2].payload.min_len(),
                catalog.events[2].payload.max_len()
            ),
            (3, 7),
        );
        let records = catalog.events[3].payload.fields().unwrap();
        let FieldEncoding::Variable(encoding) = &records.fields()[0].encoding else {
            panic!("expected record encoding");
        };
        assert!(matches!(
            encoding.shape,
            VariableEncodingShape::LengthPrefixedRecords { .. }
        ));
        let tagged = catalog.events[4].payload.fields().unwrap();
        let FieldEncoding::Variable(encoding) = &tagged.fields()[0].encoding else {
            panic!("expected tagged item encoding");
        };
        assert!(matches!(
            encoding.shape,
            VariableEncodingShape::TaggedItems(_)
        ));
    }

    #[test]
    fn accepts_complementary_firmware_shapes_for_one_event_code() {
        let catalog = syn::parse_str::<VendorEvents>(
            r#"
                #[cfg(before_fw_0_22_0)]
                Changed(0x0405) { Payload = (); }
                #[cfg(since_fw_0_22_0)]
                Changed(0x0405) { Payload = { handle: u16 => 2, }; }
            "#,
        )
        .unwrap();

        assert_eq!(catalog.events.len(), 2);
        assert_eq!(catalog.events[0].code, catalog.events[1].code);
    }

    #[test]
    fn rejects_invalid_vendor_event_catalog_states() {
        for (source, expected) in [
            ("", "must declare at least one event"),
            (
                "First(1) { Payload = (); } First(2) { Payload = (); }",
                "duplicate vendor event name",
            ),
            (
                "First(1) { Payload = (); } Second(1) { Payload = (); }",
                "duplicate vendor event code",
            ),
            (
                "#[cfg(since_fw_0_21_0)] First(1) { Payload = (); } #[cfg(since_fw_0_22_0)] Second(1) { Payload = (); }",
                "complementary `before_fw_*` and `since_fw_*`",
            ),
            (
                "#[cfg(before_fw_0_21_0)] First(1) { Payload = (); } #[cfg(since_fw_0_22_0)] Second(1) { Payload = (); }",
                "complementary `before_fw_*` and `since_fw_*`",
            ),
            (
                "#[cfg(before_fw_0_22_0)] First(1) { Payload = (); } #[cfg(since_fw_0_22_0)] Second(1) { Payload = (); } #[cfg(before_fw_0_22_0)] Third(1) { Payload = (); }",
                "duplicate vendor event code",
            ),
            (
                "TooLarge(1) { Payload = { data: [u8; 254] => 254, }; }",
                "at most 253 bytes",
            ),
            (
                "#[allow(dead_code)] BadAttr(1) { Payload = (); }",
                "only documentation and cfg attributes",
            ),
            (
                "BadRange(1) { Payload = { values: BoundedItems<Item, 2> => { kind: counted_items, count: u8 => 1, item: Item => 1, min_items: 3, max_items: 2, }, }; }",
                "counted_items minimum 3 exceeds maximum 2",
            ),
            (
                "NoDecoder(1) { Payload = { bitmap: u8 => 1, values: BoundedItems<Item, 1> => { kind: bitmap_items, bitmap: bitmap, mask: 0x01, item: Item => 1, max_items: 1, }, }; }",
                "no owned decoder",
            ),
            (
                "Removed(1) { Payload = { value: Value => { kind: payload, min_len: 1, max_len: 2, }, }; }",
                "unknown declarative variable kind `payload`",
            ),
        ] {
            let error = syn::parse_str::<VendorEvents>(source)
                .err()
                .expect("fixture must be rejected")
                .to_string();
            assert!(error.contains(expected), "{error}");
        }
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
            (
                "Bad(cgid = 0, cid = 1) { Params = { value: u8 => 1, }; Constraints = {}; Completion = CommandStatus; }",
                "Constraints must declare at least one check",
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
