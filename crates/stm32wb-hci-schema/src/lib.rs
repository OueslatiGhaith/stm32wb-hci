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

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
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

/// Event payload syntax, including its optional borrowing lifetime.
pub struct EventPayload {
    /// Lifetime declared by `Payload<'a>`, if present.
    pub lifetime: Option<syn::Lifetime>,
    /// Unit or inline-field event shape.
    pub shape: EventPayloadShape,
}

/// Unit or inline-field event payload shape.
pub enum EventPayloadShape {
    /// `Payload = ();`
    Unit,
    /// `Payload = { ... };` or `Payload<'a> = { ... };`
    Fields(Fields),
}

impl EventPayload {
    /// Parsed fields, if the payload is not unit.
    pub const fn fields(&self) -> Option<&Fields> {
        match &self.shape {
            EventPayloadShape::Unit => None,
            EventPayloadShape::Fields(fields) => Some(fields),
        }
    }

    /// Whether the generated payload type borrows from the event packet.
    pub const fn borrows(&self) -> bool {
        self.lifetime.is_some()
    }

    /// Minimum encoded payload size, excluding the two-byte event code.
    pub fn min_size(&self) -> WireSize {
        self.fields()
            .map_or_else(WireSize::default, Fields::min_size)
    }

    /// Maximum encoded payload size, excluding the two-byte event code.
    pub fn max_size(&self) -> WireSize {
        self.fields()
            .map_or_else(WireSize::default, Fields::max_size)
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
    pub fn min_size(&self) -> WireSize {
        self.fields()
            .map_or_else(WireSize::default, Fields::min_size)
    }

    /// Maximum encoded request size before the HCI packet limit is applied.
    pub fn max_size(&self) -> WireSize {
        self.fields()
            .map_or_else(WireSize::default, Fields::max_size)
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
    pub fn min_size(&self) -> WireSize {
        self.fields()
            .map_or_else(WireSize::default, Fields::min_size)
    }

    /// Maximum encoded return size.
    pub fn max_size(&self) -> WireSize {
        self.fields()
            .map_or_else(WireSize::default, Fields::max_size)
    }
}

/// Parsed aggregate metadata for an inline field body.
pub struct Fields {
    fields: Vec<Field>,
    names: BTreeSet<String>,
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

    /// Minimum encoded size, expressed in canonical semantic-type widths.
    pub fn min_size(&self) -> WireSize {
        self.fields.iter().map(Field::min_size).sum()
    }

    /// Maximum encoded size, expressed in canonical semantic-type widths.
    pub fn max_size(&self) -> WireSize {
        self.fields.iter().map(Field::max_size).sum()
    }
}

/// One typed field in a Params, Return, or event payload body.
pub struct Field {
    /// Documentation attached to the semantic field.
    pub attrs: Vec<syn::Attribute>,
    /// Field binding and generated member name.
    pub name: syn::Ident,
    /// Semantic Rust type from the declaration.
    pub ty: Type,
    /// Fixed-width or variable wire encoding.
    pub encoding: FieldEncoding,
}

impl Field {
    /// Minimum size of this field in canonical semantic-type widths.
    pub fn min_size(&self) -> WireSize {
        match &self.encoding {
            FieldEncoding::Fixed(_) => WireSize::type_width(&self.ty),
            FieldEncoding::Variable(encoding) => encoding.min_size(),
        }
    }

    /// Maximum size of this field in canonical semantic-type widths.
    pub fn max_size(&self) -> WireSize {
        match &self.encoding {
            FieldEncoding::Fixed(_) => WireSize::type_width(&self.ty),
            FieldEncoding::Variable(encoding) => encoding.max_size(),
        }
    }
}

/// The fixed or variable wire encoding attached to a semantic field.
pub enum FieldEncoding {
    /// One canonical fixed-width HCI field.
    Fixed(FixedEncoding),
    /// A variable schema body retained losslessly for later code generation.
    Variable(Box<VariableEncoding>),
}

/// A fixed-width field whose size comes from its semantic Rust type.
pub struct FixedEncoding;

/// An encoded size composed from constants and canonical semantic-type widths.
#[derive(Clone, Default)]
pub struct WireSize {
    constant: usize,
    terms: Vec<WireSizeTerm>,
}

/// One `type width * multiplier` term in an encoded size.
#[derive(Clone)]
pub struct WireSizeTerm {
    ty: Type,
    multiplier: usize,
}

impl WireSize {
    /// A constant encoded size.
    pub const fn constant(value: usize) -> Self {
        Self {
            constant: value,
            terms: Vec::new(),
        }
    }

    /// The constant part of this size.
    pub const fn constant_part(&self) -> usize {
        self.constant
    }

    /// Semantic-width terms in this size.
    pub fn terms(&self) -> &[WireSizeTerm] {
        &self.terms
    }

    fn type_width(ty: &Type) -> Self {
        Self {
            constant: 0,
            terms: vec![WireSizeTerm {
                ty: ty.clone(),
                multiplier: 1,
            }],
        }
    }

    fn scaled(mut self, multiplier: usize) -> Self {
        self.constant = self
            .constant
            .checked_mul(multiplier)
            .expect("validated declarative size must fit usize");
        for term in &mut self.terms {
            term.multiplier = term
                .multiplier
                .checked_mul(multiplier)
                .expect("validated declarative size must fit usize");
        }
        self
    }
}

impl WireSizeTerm {
    /// Semantic Rust type whose canonical width contributes to the size.
    pub const fn ty(&self) -> &Type {
        &self.ty
    }

    /// Number of values of this type contributing to the size.
    pub const fn multiplier(&self) -> usize {
        self.multiplier
    }
}

impl core::ops::Add for WireSize {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.constant = self
            .constant
            .checked_add(rhs.constant)
            .expect("validated declarative size must fit usize");
        self.terms.extend(rhs.terms);
        self
    }
}

impl core::iter::Sum for WireSize {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), core::ops::Add::add)
    }
}

/// Validated variable field encoding.
pub struct VariableEncoding {
    /// Complete typed encoding schema used by code generation.
    pub shape: VariableEncodingShape,
    /// Generated C storage minimum for this field.
    ///
    /// This defaults to `min_len`. A smaller value is an explicit declaration
    /// that the Rust API intentionally enforces a narrower semantic domain
    /// than the generated transport buffer can represent.
    pub storage_min_len: Option<IntegerValue>,
    /// Generated C storage capacity for this field.
    ///
    /// This defaults to `max_len`. A larger value is an explicit declaration
    /// that the Rust API intentionally enforces a narrower semantic domain
    /// than the generated transport buffer can store.
    pub storage_max_len: Option<IntegerValue>,
    /// Whether this field consumes every remaining payload byte.
    pub consumes_remainder: bool,
}

impl VariableEncoding {
    /// Minimum semantic encoded size.
    pub fn min_size(&self) -> WireSize {
        match &self.shape {
            VariableEncodingShape::CountedBytes { count, min_len, .. } => {
                WireSize::type_width(&count.ty) + WireSize::constant(min_len.value)
            }
            VariableEncodingShape::CountedItems {
                count,
                item,
                min_items,
                ..
            } => {
                WireSize::type_width(&count.ty)
                    + WireSize::type_width(&item.ty).scaled(min_items.value)
            }
            VariableEncodingShape::Tagged(tagged) => WireSize::constant(tagged.min_len.value),
            VariableEncodingShape::LengthPrefixedRecords {
                record_len, length, ..
            } => WireSize::type_width(&record_len.ty) + WireSize::type_width(&length.ty),
            VariableEncodingShape::TaggedItems(tagged) => {
                WireSize::type_width(&tagged.tag.ty) + WireSize::type_width(&tagged.length.ty)
            }
            VariableEncodingShape::TrailingBytes { min_len, .. } => {
                WireSize::constant(min_len.value)
            }
            VariableEncodingShape::BitmapItems { .. } => WireSize::default(),
        }
    }

    /// Maximum semantic encoded size.
    pub fn max_size(&self) -> WireSize {
        match &self.shape {
            VariableEncodingShape::CountedBytes { count, max_len, .. } => {
                WireSize::type_width(&count.ty) + WireSize::constant(max_len.value)
            }
            VariableEncodingShape::CountedItems {
                count,
                item,
                max_items,
                ..
            } => {
                WireSize::type_width(&count.ty)
                    + WireSize::type_width(&item.ty).scaled(max_items.value)
            }
            VariableEncodingShape::Tagged(tagged) => WireSize::constant(tagged.max_len.value),
            VariableEncodingShape::LengthPrefixedRecords {
                record_len,
                length,
                max_len,
                ..
            } => {
                WireSize::type_width(&record_len.ty)
                    + WireSize::type_width(&length.ty)
                    + WireSize::constant(max_len.value)
            }
            VariableEncodingShape::TaggedItems(tagged) => {
                WireSize::type_width(&tagged.tag.ty)
                    + WireSize::type_width(&tagged.length.ty)
                    + WireSize::constant(tagged.max_len.value)
            }
            VariableEncodingShape::TrailingBytes { max_len, .. } => {
                WireSize::constant(max_len.value)
            }
            VariableEncodingShape::BitmapItems {
                item, max_items, ..
            } => WireSize::type_width(&item.ty).scaled(max_items.value),
        }
    }

    /// Generated C storage minimum, or the semantic minimum when not overridden.
    pub fn storage_min_size(&self) -> WireSize {
        self.storage_min_len
            .as_ref()
            .map_or_else(|| self.min_size(), |value| WireSize::constant(value.value))
    }

    /// Generated C storage maximum, or the semantic maximum when not overridden.
    pub fn storage_max_size(&self) -> WireSize {
        self.storage_max_len
            .as_ref()
            .map_or_else(|| self.max_size(), |value| WireSize::constant(value.value))
    }
}

/// Complete schema for one variable-width field.
pub enum VariableEncodingShape {
    /// A fixed-width count followed by that many bytes.
    CountedBytes {
        /// Count type; its wire width is canonical.
        count: WireType,
        /// Minimum accepted byte count.
        min_len: IntegerValue,
        /// Maximum accepted byte count.
        max_len: IntegerValue,
    },
    /// A fixed-width count followed by that many fixed-width items.
    CountedItems {
        /// Count type; its wire width is canonical.
        count: WireType,
        /// Item type; its wire width is canonical.
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
        /// Record-width type; its wire width is canonical.
        record_len: WireType,
        /// Byte-length type; its wire width is canonical.
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
        /// Item type; its wire width is canonical.
        item: WireType,
        /// Number of selectable items; validated against the mask population.
        max_items: IntegerValue,
    },
}

/// A semantic Rust type with one canonical fixed wire width.
pub struct WireType {
    /// Semantic Rust type from the declaration.
    pub ty: Type,
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
    /// Discriminator type; its wire width is canonical.
    pub tag: WireType,
    /// Variants in declaration order.
    pub variants: Vec<TaggedVariant>,
    /// Declared minimum encoded length, verified against canonical widths.
    pub min_len: IntegerValue,
    /// Declared maximum encoded length, verified against canonical widths.
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
    /// Discriminator type; its wire width is canonical.
    pub tag: WireType,
    /// Combined item-byte length type; its wire width is canonical.
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
    /// Item type with one canonical exact wire width.
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
            let lifetime = parse_optional_declared_lifetime(&body, "Payload")?;
            body.parse::<syn::Token![=]>()?;
            let shape = if body.peek(syn::token::Brace) {
                let fields;
                syn::braced!(fields in body);
                let fields = fields.parse::<Fields>()?;
                validate_field_lifetimes("Payload", lifetime.as_ref(), &fields)?;
                EventPayloadShape::Fields(fields)
            } else if body.peek(syn::token::Paren) {
                let unit;
                syn::parenthesized!(unit in body);
                if !unit.is_empty() {
                    return Err(unit.error("unit event payload must be `()`"));
                }
                if lifetime.is_some() {
                    return Err(syn::Error::new_spanned(
                        &payload_label,
                        "unit Payload must not declare a lifetime",
                    ));
                }
                EventPayloadShape::Unit
            } else {
                return Err(body.error("event Payload must be `()` or a declarative field body"));
            };
            let payload = EventPayload { lifetime, shape };
            body.parse::<syn::Token![;]>()?;
            if !body.is_empty() {
                return Err(body.error("unexpected tokens after event Payload"));
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
        let lifetime = parse_optional_declared_lifetime(input, "Params")?;
        if !input.is_empty() {
            return Err(input.error("Params accepts at most one lifetime parameter"));
        }
        Ok(Self(lifetime))
    }
}

fn parse_optional_declared_lifetime(
    input: ParseStream<'_>,
    declaration: &str,
) -> syn::Result<Option<syn::Lifetime>> {
    if !input.peek(syn::Token![<]) {
        return Ok(None);
    }
    input.parse::<syn::Token![<]>()?;
    let lifetime = input.parse::<syn::Lifetime>()?;
    input.parse::<syn::Token![>]>()?;
    if lifetime.ident == "static" || lifetime.ident == "_" {
        return Err(syn::Error::new_spanned(
            lifetime,
            format!("{declaration} must declare a named, non-`'static` lifetime"),
        ));
    }
    Ok(Some(lifetime))
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
    let fields = parse_braced_fields(value)?;
    validate_field_lifetimes("Params", lifetime.as_ref(), &fields)?;
    Ok(Params {
        lifetime,
        shape: ParamsShape::Fields(fields),
    })
}

fn validate_field_lifetimes(
    declaration: &str,
    declared: Option<&syn::Lifetime>,
    fields: &Fields,
) -> syn::Result<()> {
    let mut errors = None;
    let mut declared_is_used = false;

    for field in fields.fields() {
        let mut lifetimes = FreeLifetimes::default();
        lifetimes.visit_type(&field.ty);

        for lifetime in lifetimes.named {
            if lifetime.ident == "_" {
                combine_error(
                    &mut errors,
                    syn::Error::new_spanned(
                        &lifetime,
                        format!("field types must name the {declaration} lifetime explicitly"),
                    ),
                );
                continue;
            }

            match declared {
                Some(expected) if lifetime.ident == expected.ident => declared_is_used = true,
                Some(expected) => combine_error(
                    &mut errors,
                    syn::Error::new_spanned(
                        &lifetime,
                        format!(
                            "field lifetime `'{}'` does not match declared {declaration} lifetime `'{}'`",
                            lifetime.ident, expected.ident,
                        ),
                    ),
                ),
                None => combine_error(
                    &mut errors,
                    syn::Error::new_spanned(
                        &lifetime,
                        format!(
                            "undeclared field lifetime `'{}'`; declare `{declaration}<'{}>`",
                            lifetime.ident, lifetime.ident,
                        ),
                    ),
                ),
            }
        }

        for span in lifetimes.elided_references {
            combine_error(
                &mut errors,
                syn::Error::new(
                    span,
                    format!("field references must name the {declaration} lifetime explicitly"),
                ),
            );
        }
    }

    if let Some(lifetime) = declared.filter(|_| !declared_is_used) {
        combine_error(
            &mut errors,
            syn::Error::new_spanned(
                lifetime,
                format!(
                    "declared {declaration} lifetime `'{}'` is not used by any field",
                    lifetime.ident,
                ),
            ),
        );
    }

    errors.map_or(Ok(()), Err)
}

#[derive(Default)]
struct FreeLifetimes {
    bound: Vec<BTreeSet<String>>,
    named: Vec<syn::Lifetime>,
    elided_references: Vec<Span>,
}

impl FreeLifetimes {
    fn visit_with_bound_lifetimes(
        &mut self,
        bound_lifetimes: Option<&syn::BoundLifetimes>,
        visit_body: impl FnOnce(&mut Self),
    ) {
        let names = bound_lifetimes
            .into_iter()
            .flat_map(|lifetimes| &lifetimes.lifetimes)
            .filter_map(|parameter| match parameter {
                syn::GenericParam::Lifetime(parameter) => {
                    Some(parameter.lifetime.ident.to_string())
                }
                syn::GenericParam::Type(_) | syn::GenericParam::Const(_) => None,
            })
            .collect();
        self.bound.push(names);

        if let Some(bound_lifetimes) = bound_lifetimes {
            for parameter in &bound_lifetimes.lifetimes {
                if let syn::GenericParam::Lifetime(parameter) = parameter {
                    for bound in &parameter.bounds {
                        self.visit_lifetime(bound);
                    }
                }
            }
        }
        visit_body(self);
        self.bound.pop();
    }

    fn lifetime_is_bound(&self, lifetime: &syn::Lifetime) -> bool {
        self.bound
            .iter()
            .rev()
            .any(|scope| scope.contains(&lifetime.ident.to_string()))
    }
}

impl<'ast> Visit<'ast> for FreeLifetimes {
    fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
        if lifetime.ident != "static" && !self.lifetime_is_bound(lifetime) {
            self.named.push(lifetime.clone());
        }
    }

    fn visit_type_reference(&mut self, reference: &'ast syn::TypeReference) {
        if reference.lifetime.is_none() {
            self.elided_references.push(reference.and_token.span);
        }
        visit::visit_type_reference(self, reference);
    }

    fn visit_type_bare_fn(&mut self, function: &'ast syn::TypeBareFn) {
        self.visit_with_bound_lifetimes(function.lifetimes.as_ref(), |visitor| {
            for input in &function.inputs {
                visitor.visit_type(&input.ty);
            }
            if let syn::ReturnType::Type(_, output) = &function.output {
                visitor.visit_type(output);
            }
        });
    }

    fn visit_trait_bound(&mut self, bound: &'ast syn::TraitBound) {
        self.visit_with_bound_lifetimes(bound.lifetimes.as_ref(), |visitor| {
            visitor.visit_path(&bound.path);
        });
    }
}

fn combine_error(errors: &mut Option<syn::Error>, error: syn::Error) {
    if let Some(errors) = errors {
        errors.combine(error);
    } else {
        *errors = Some(error);
    }
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
        let mut consumes_remainder = false;

        while !input.is_empty() {
            if consumes_remainder {
                return Err(input.error("trailing_bytes must be the final declarative field"));
            }
            let attrs = input.call(syn::Attribute::parse_outer)?;
            for attr in &attrs {
                if !attr.path().is_ident("doc") {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "declarative fields accept only documentation attributes",
                    ));
                }
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

            let (encoding, field_consumes_remainder) = if input.peek(syn::Token![=>]) {
                input.parse::<syn::Token![=>]>()?;
                if input.peek(syn::token::Brace) {
                    let shape;
                    syn::braced!(shape in input);
                    let encoding = shape.parse::<VariableEncoding>()?;
                    let field_consumes_remainder = encoding.consumes_remainder;
                    (
                        FieldEncoding::Variable(Box::new(encoding)),
                        field_consumes_remainder,
                    )
                } else {
                    return Err(input.error(
                        "fixed fields use `field: Type`; `=>` is reserved for variable shapes",
                    ));
                }
            } else {
                (FieldEncoding::Fixed(FixedEncoding), false)
            };

            consumes_remainder = field_consumes_remainder;
            fields.push(Field {
                attrs,
                name,
                ty,
                encoding,
            });
            input.parse::<syn::Token![,]>()?;
        }

        Ok(Self { fields, names })
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

        let storage_min_len = if next_label_is(input, "storage_min_len")? {
            Some(parse_integer_value(input, "storage_min_len")?)
        } else {
            None
        };
        let storage_max_len = if next_label_is(input, "storage_max_len")? {
            Some(parse_integer_value(input, "storage_max_len")?)
        } else {
            None
        };
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after declarative variable field"));
        }
        let consumes_remainder = matches!(shape, VariableEncodingShape::TrailingBytes { .. });
        Ok(Self {
            shape,
            storage_min_len,
            storage_max_len,
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

fn next_label_is(input: ParseStream<'_>, expected: &str) -> syn::Result<bool> {
    if !input.peek(syn::Ident) {
        return Ok(false);
    }
    let fork = input.fork();
    Ok(fork.parse::<syn::Ident>()? == expected)
}

fn parse_tagged_encoding(input: ParseStream<'_>) -> syn::Result<TaggedEncoding> {
    let tag = parse_wire_type(input, "tag")?;
    parse_colon_label(input, "variants")?;
    let variants;
    syn::braced!(variants in input);
    input.parse::<syn::Token![,]>()?;

    let mut tags = BTreeSet::new();
    let mut parsed_variants = Vec::new();
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
        parsed_variants.push(TaggedVariant {
            pattern,
            tag: variant_tag,
            fields: payload,
        });
        variants.parse::<syn::Token![,]>()?;
    }

    if parsed_variants.is_empty() {
        return Err(input.error("tagged field must declare at least one variant"));
    }
    let declared_min = parse_integer_value(input, "min_len")?;
    let declared_max = parse_integer_value(input, "max_len")?;
    validate_range(
        input,
        "tagged field",
        declared_min.value,
        declared_max.value,
    )?;
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
        variants.parse::<syn::Token![=>]>()?;

        let body;
        syn::braced!(body in variants);
        let item = parse_wire_type(&body, "item")?;
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
    input.parse::<syn::Token![,]>()?;
    Ok(WireType { ty })
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

    fn resolve_test_size(size: &WireSize) -> usize {
        size.terms()
            .iter()
            .fold(size.constant_part(), |total, term| {
                let name = simple_test_type_name(term.ty());
                let width = match name.as_str() {
                    "u8" | "IoCapability" => 1,
                    "u16" | "Item" => 2,
                    _ => panic!("test fixture has no width for {name}"),
                };
                total + width * term.multiplier()
            })
    }

    fn simple_test_type_name(ty: &Type) -> String {
        match ty {
            Type::Path(path) => path.path.segments.last().unwrap().ident.to_string(),
            Type::Reference(reference) => simple_test_type_name(&reference.elem),
            _ => panic!("test fixture uses an unsupported semantic type"),
        }
    }

    #[test]
    fn parses_fixed_command_and_derives_ocf() {
        let command = syn::parse_str::<VendorCommand>(
            r#"
                GapSetIoCapability(cgid = 0x1, cid = 0x05) {
                    Params = { io_capability: IoCapability, };
                    Completion = CommandComplete;
                    Return = ();
                }
            "#,
        )
        .unwrap();
        assert_eq!(command.name, "GapSetIoCapability");
        assert_eq!(command.ocf(), 0x085);
        assert_eq!(resolve_test_size(&command.params.min_size()), 1);
        assert_eq!(resolve_test_size(&command.params.max_size()), 1);
        assert!(command.params.lifetime.is_none());
        let [field] = command.params.fields().unwrap().fields() else {
            panic!("expected one typed Params field");
        };
        assert_eq!(field.name, "io_capability");
        let FieldEncoding::Fixed(_) = &field.encoding else {
            panic!("expected a fixed encoding");
        };
        assert_eq!(command.completion, Completion::CommandComplete);
        assert_eq!(resolve_test_size(&command.returns.unwrap().max_size()), 0);
    }

    #[test]
    fn rejects_field_local_width_metadata() {
        let fixed = syn::parse_str::<VendorCommand>(
            "Bad(cgid = 0, cid = 1) { Params = { value: u16 => 1, }; Completion = CommandStatus; }",
        )
        .err()
        .expect("field-local width must be rejected")
        .to_string();
        assert!(
            fixed.contains("`=>` is reserved for variable shapes"),
            "{fixed}"
        );

        assert!(
            syn::parse_str::<VendorCommand>(
                "Bad(cgid = 0, cid = 1) { Params<'a> = { values: &'a [u16] => { kind: counted_items, count: u8 => 1, item: u16, max_items: 2, }, }; Completion = CommandStatus; }",
            )
            .is_err()
        );
    }

    #[test]
    fn parses_variable_shapes_and_constraints_together() {
        let command = syn::parse_str::<VendorCommand>(
            r#"
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        limit: u8,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8,
                            min_len: 1,
                            max_len: 16,
                            storage_min_len: 1,
                            storage_max_len: 33,
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
        assert_eq!(resolve_test_size(&command.params.min_size()), 3);
        assert_eq!(resolve_test_size(&command.params.max_size()), 18);
        assert_eq!(command.params.lifetime.as_ref().unwrap().ident, "a");
        let fields = command.params.fields().unwrap().fields();
        assert_eq!(fields.len(), 2);
        let FieldEncoding::Variable(encoding) = &fields[1].encoding else {
            panic!("expected a variable encoding");
        };
        assert_eq!(resolve_test_size(&encoding.min_size()), 2);
        assert_eq!(resolve_test_size(&encoding.max_size()), 17);
        assert_eq!(encoding.storage_min_len.as_ref().unwrap().value, 1);
        assert_eq!(encoding.storage_max_len.as_ref().unwrap().value, 33);
        let VariableEncodingShape::CountedBytes {
            count,
            min_len,
            max_len,
        } = &encoding.shape
        else {
            panic!("expected typed counted-bytes metadata");
        };
        assert_eq!(simple_test_type_name(&count.ty), "u8");
        assert_eq!(min_len.value, 1);
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
                        total: u16,
                        offset: u16,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8,
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
                        mode: u8,
                        error: u8,
                        limit: u8,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8,
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
                #[cfg(since_fw_1_17_0)]
                Counted(0x0002) {
                    Payload<'packet> = {
                        handle: u16,
                        bytes: BoundedBytes<'packet, 8> => {
                            kind: counted_bytes,
                            count: u8,
                            max_len: 8,
                        },
                    };
                }
                Items(0x0003) {
                    Payload = {
                        values: BoundedItems<Item, 3> => {
                            kind: counted_items,
                            count: u8,
                            item: Item,
                            min_items: 1,
                            max_items: 3,
                        },
                    };
                }
                Records(0x0004) {
                    Payload = {
                        value: Records => {
                            kind: length_prefixed_records,
                            record_len: u8,
                            length: u8,
                            min_record_len: 2,
                            max_len: 8,
                        },
                    };
                }
                TaggedItems(0x0005) {
                    Payload = {
                        value: TaggedItems => {
                            kind: tagged_items,
                            tag: u8,
                            length: u8,
                            variants: {
                                1 => { item: Short, max_items: 4, },
                                2 => { item: Long, max_items: 2, },
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
        assert!(matches!(
            catalog.events[0].payload.shape,
            EventPayloadShape::Unit
        ));
        assert_eq!(catalog.events[1].code, 0x0002);
        assert_eq!(catalog.events[1].attrs.len(), 1);
        assert_eq!(
            catalog.events[1].payload.lifetime.as_ref().unwrap().ident,
            "packet"
        );
        assert!(catalog.events[1].payload.borrows());
        assert!(!catalog.events[2].payload.borrows());
        assert_eq!(
            (
                resolve_test_size(&catalog.events[1].payload.min_size()),
                resolve_test_size(&catalog.events[1].payload.max_size())
            ),
            (3, 11),
        );
        assert_eq!(
            (
                resolve_test_size(&catalog.events[2].payload.min_size()),
                resolve_test_size(&catalog.events[2].payload.max_size())
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
    fn validates_declared_event_payload_lifetimes() {
        for (source, expected) in [
            (
                "Borrowed(1) { Payload = { value: View<'packet>, }; }",
                "undeclared field lifetime `'packet'`; declare `Payload<'packet>`",
            ),
            (
                "Borrowed(1) { Payload<'packet> = { value: View<'other>, }; }",
                "field lifetime `'other'` does not match declared Payload lifetime `'packet'`",
            ),
            (
                "Owned(1) { Payload<'packet> = { value: Owned, }; }",
                "declared Payload lifetime `'packet'` is not used by any field",
            ),
            (
                "Unit(1) { Payload<'packet> = (); }",
                "unit Payload must not declare a lifetime",
            ),
            (
                "Borrowed(1) { Payload = { value: &u8, }; }",
                "field references must name the Payload lifetime explicitly",
            ),
            (
                "Borrowed(1) { Payload<'static> = { value: View<'static>, }; }",
                "Payload must declare a named, non-`'static` lifetime",
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
    fn validates_declared_command_parameter_lifetimes() {
        for (params, expected) in [
            (
                "Params<'packet> = { value: &'other [u8], };",
                "field lifetime `'other'` does not match declared Params lifetime `'packet'`",
            ),
            (
                "Params<'packet> = { value: u8, };",
                "declared Params lifetime `'packet'` is not used by any field",
            ),
        ] {
            let source =
                format!("Bad(cgid = 0, cid = 1) {{ {params} Completion = CommandStatus; }}");
            let error = syn::parse_str::<VendorCommand>(&source)
                .err()
                .expect("fixture must be rejected")
                .to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn accepts_complementary_firmware_shapes_for_one_event_code() {
        let catalog = syn::parse_str::<VendorEvents>(
            r#"
                #[cfg(before_fw_1_22_0)]
                Changed(0x0405) { Payload = (); }
                #[cfg(since_fw_1_22_0)]
                Changed(0x0405) { Payload = { handle: u16, }; }
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
                "#[cfg(since_fw_1_21_0)] First(1) { Payload = (); } #[cfg(since_fw_1_22_0)] Second(1) { Payload = (); }",
                "complementary `before_fw_*` and `since_fw_*`",
            ),
            (
                "#[cfg(before_fw_1_21_0)] First(1) { Payload = (); } #[cfg(since_fw_1_22_0)] Second(1) { Payload = (); }",
                "complementary `before_fw_*` and `since_fw_*`",
            ),
            (
                "#[cfg(before_fw_1_22_0)] First(1) { Payload = (); } #[cfg(since_fw_1_22_0)] Second(1) { Payload = (); } #[cfg(before_fw_1_22_0)] Third(1) { Payload = (); }",
                "duplicate vendor event code",
            ),
            (
                "#[allow(dead_code)] BadAttr(1) { Payload = (); }",
                "only documentation and cfg attributes",
            ),
            (
                "BadRange(1) { Payload = { values: BoundedItems<Item, 2> => { kind: counted_items, count: u8, item: Item, min_items: 3, max_items: 2, }, }; }",
                "counted_items minimum 3 exceeds maximum 2",
            ),
            (
                "NoDecoder(1) { Payload = { bitmap: u8, values: BoundedItems<Item, 1> => { kind: bitmap_items, bitmap: bitmap, mask: 0x01, item: Item, max_items: 1, }, }; }",
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
                "Bad(cgid = 0, cid = 1) { Params = { value: u8, }; Constraints = { range(missing, 0, 1); }; Completion = CommandStatus; }",
                "unknown parameter(s): missing",
            ),
            (
                "Bad(cgid = 0, cid = 1) { Params = { value: u8, }; Constraints = {}; Completion = CommandStatus; }",
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
