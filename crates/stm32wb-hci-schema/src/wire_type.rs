//! Shared syntax for semantic values with canonical HCI wire representations.

use std::collections::BTreeSet;

use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Block, Expr, Ident, LitInt, Token, Type, Visibility, braced, bracketed};

/// One protocol adapter generated for a semantic wire type.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WireAdapter {
    /// Fixed-width command parameter encoding and return decoding.
    Command,
    /// Fixed-width vendor-event decoding.
    Event,
    /// Bidirectional conversion between a closed enum and its scalar representation.
    Conversion,
}

/// Validated adapter set attached to one wire declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireAdapters {
    values: BTreeSet<WireAdapter>,
}

impl WireAdapters {
    /// Whether command encoding/decoding was requested.
    pub fn command(&self) -> bool {
        self.values.contains(&WireAdapter::Command)
    }

    /// Whether vendor-event decoding was requested.
    pub fn event(&self) -> bool {
        self.values.contains(&WireAdapter::Event)
    }

    /// Whether standalone scalar conversion was requested.
    pub fn conversion(&self) -> bool {
        self.values.contains(&WireAdapter::Conversion)
    }

    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let label = input.parse::<Ident>()?;
        if label != "adapters" {
            return Err(syn::Error::new(label.span(), "expected `adapters`"));
        }
        input.parse::<Token![:]>()?;
        let content;
        bracketed!(content in input);

        let mut values = BTreeSet::new();
        while !content.is_empty() {
            let adapter = content.parse::<Ident>()?;
            let value = match adapter.to_string().as_str() {
                "command" => WireAdapter::Command,
                "event" => WireAdapter::Event,
                "conversion" => WireAdapter::Conversion,
                _ => {
                    return Err(syn::Error::new(
                        adapter.span(),
                        "wire adapter must be `command`, `event`, or `conversion`",
                    ));
                }
            };
            if !values.insert(value) {
                return Err(syn::Error::new(adapter.span(), "duplicate wire adapter"));
            }
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            } else if !content.is_empty() {
                return Err(content.error("expected `,` between wire adapters"));
            }
        }
        input.parse::<Token![;]>()?;
        if values.is_empty() {
            return Err(input.error("a wire declaration requires at least one adapter"));
        }
        Ok(Self { values })
    }

    fn reject_conversion(&self, span: proc_macro2::Span) -> syn::Result<()> {
        if self.conversion() {
            Err(syn::Error::new(
                span,
                "the `conversion` adapter is only valid for closed enums",
            ))
        } else {
            Ok(())
        }
    }
}

/// One parsed `wire_type!` declaration.
pub struct SemanticWireType {
    /// Protocol contexts generated for this semantic type.
    pub adapters: WireAdapters,
    /// Shape-specific declaration.
    pub declaration: WireTypeDeclaration,
}

/// Supported semantic wire shapes.
pub enum WireTypeDeclaration {
    /// Closed scalar enum.
    ClosedEnum(ClosedEnumWireType),
    /// Forward-compatible scalar enum with a raw fallback variant.
    OpenEnum(OpenEnumWireType),
    /// Transparent scalar accepting every underlying bit pattern.
    OpenScalar(OpenScalarWireType),
    /// Inclusive scalar range with an optional out-of-range sentinel.
    Ranged(RangedWireType),
    /// Bitflags value.
    Bitflags(BitflagsWireType),
    /// Exact-width semantic composition.
    Composite(CompositeWireType),
    /// Built-in integer scalar.
    Primitive(PrimitiveWireType),
    /// Existing tuple newtype delegating to its inner wire type.
    Transparent(TransparentWireType),
}

/// One enum discriminant.
pub struct WireEnumVariant {
    /// Documentation and cfg attributes.
    pub attrs: Vec<Attribute>,
    /// Rust variant name.
    pub name: Ident,
    /// Scalar wire value.
    pub value: Expr,
}

/// Closed enum declaration.
pub struct ClosedEnumWireType {
    pub attrs: Vec<Attribute>,
    pub visibility: Visibility,
    pub name: Ident,
    pub repr: Type,
    /// Width is optional for a conversion-only enum.
    pub width: Option<LitInt>,
    pub variants: Vec<WireEnumVariant>,
    pub try_from_error: Option<Type>,
    pub invalid_value: Option<Expr>,
    pub event_error: Option<Expr>,
}

/// Open enum declaration.
pub struct OpenEnumWireType {
    pub attrs: Vec<Attribute>,
    pub visibility: Visibility,
    pub name: Ident,
    pub repr: Type,
    pub width: LitInt,
    pub variants: Vec<WireEnumVariant>,
    pub fallback: Ident,
}

/// Open scalar declaration.
pub struct OpenScalarWireType {
    pub attrs: Vec<Attribute>,
    pub visibility: Visibility,
    pub name: Ident,
    pub repr: Type,
    pub width: LitInt,
}

/// Optional named value accepted outside a ranged type's inclusive bounds.
pub struct WireSentinel {
    pub name: Ident,
    pub value: Expr,
}

/// Ranged scalar declaration.
pub struct RangedWireType {
    pub attrs: Vec<Attribute>,
    pub visibility: Visibility,
    pub name: Ident,
    pub repr: Type,
    pub width: LitInt,
    pub minimum: Expr,
    pub maximum: Expr,
    pub sentinel: Option<WireSentinel>,
    pub event_error: Option<Expr>,
}

/// One bitflag constant.
pub struct WireFlag {
    pub attrs: Vec<Attribute>,
    pub name: Ident,
    pub value: Expr,
}

/// Bitflags declaration.
pub struct BitflagsWireType {
    pub attrs: Vec<Attribute>,
    pub visibility: Visibility,
    pub name: Ident,
    pub repr: Type,
    pub width: LitInt,
    pub flags: Vec<WireFlag>,
    pub event_error: Option<Expr>,
}

/// One exact-width component of a composite value.
pub struct WireCompositeField {
    pub name: Ident,
    pub ty: Type,
    pub width: LitInt,
}

/// Composite declaration. Command adapters consume `encode`; event adapters
/// consume `decode`, and both blocks may coexist.
pub struct CompositeWireType {
    pub ty: Type,
    pub width: LitInt,
    pub fields: Vec<WireCompositeField>,
    pub encode: Option<(Ident, Block)>,
    pub decode: Option<Block>,
}

/// Built-in scalar declaration used to centralize primitive adapters.
pub struct PrimitiveWireType {
    pub ty: Type,
    pub width: LitInt,
}

/// Existing tuple-newtype declaration used to centralize delegating adapters.
pub struct TransparentWireType {
    pub ty: Type,
    pub inner: Type,
    pub width: LitInt,
}

impl Parse for SemanticWireType {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let adapters = WireAdapters::parse(input)?;
        let kind = input.parse::<Ident>()?;
        let declaration = match kind.to_string().as_str() {
            "closed" => WireTypeDeclaration::ClosedEnum(parse_closed_enum(input, &adapters)?),
            "open_enum" => WireTypeDeclaration::OpenEnum(parse_open_enum(input, &adapters)?),
            "open_scalar" => WireTypeDeclaration::OpenScalar(parse_open_scalar(input, &adapters)?),
            "open" => {
                if input.peek(Token![enum]) {
                    WireTypeDeclaration::OpenEnum(parse_open_enum(input, &adapters)?)
                } else {
                    let shape = input.parse::<Ident>()?;
                    match shape.to_string().as_str() {
                        "scalar" => {
                            WireTypeDeclaration::OpenScalar(parse_open_scalar(input, &adapters)?)
                        }
                        _ => {
                            return Err(syn::Error::new(
                                shape.span(),
                                "expected `enum` or `scalar` after `open`",
                            ));
                        }
                    }
                }
            }
            "ranged" => WireTypeDeclaration::Ranged(parse_ranged(input, &adapters)?),
            "bitflags" => WireTypeDeclaration::Bitflags(parse_bitflags(input, &adapters)?),
            "composite" => WireTypeDeclaration::Composite(parse_composite(input, &adapters)?),
            "primitive" => WireTypeDeclaration::Primitive(parse_primitive(input, &adapters)?),
            "transparent" => WireTypeDeclaration::Transparent(parse_transparent(input, &adapters)?),
            _ => {
                return Err(syn::Error::new(
                    kind.span(),
                    "wire type must be `closed`, `open_enum`, `open_scalar`, `ranged`, `bitflags`, `composite`, `primitive`, or `transparent`",
                ));
            }
        };
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after wire declaration"));
        }
        Ok(Self {
            adapters,
            declaration,
        })
    }
}

fn parse_closed_enum(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<ClosedEnumWireType> {
    let attrs = input.call(Attribute::parse_outer)?;
    let visibility = input.parse::<Visibility>()?;
    input.parse::<Token![enum]>()?;
    let name = input.parse::<Ident>()?;
    input.parse::<Token![:]>()?;
    let repr = input.parse::<Type>()?;
    let width = if input.peek(Token![=>]) {
        input.parse::<Token![=>]>()?;
        Some(input.parse::<LitInt>()?)
    } else {
        None
    };
    let variants = parse_enum_variants(input, false)?.0;

    let mut try_from_error = None;
    let mut invalid_value = None;
    let mut event_error = None;
    while !input.is_empty() {
        let label = input.parse::<Ident>()?;
        match label.to_string().as_str() {
            "TryFromError" => {
                if try_from_error.is_some() {
                    return Err(syn::Error::new(label.span(), "duplicate `TryFromError`"));
                }
                input.parse::<Token![=]>()?;
                try_from_error = Some(input.parse::<Type>()?);
                input.parse::<Token![=>]>()?;
                invalid_value = Some(input.parse::<Expr>()?);
                input.parse::<Token![;]>()?;
            }
            "EventError" => {
                if event_error.is_some() {
                    return Err(syn::Error::new(label.span(), "duplicate `EventError`"));
                }
                input.parse::<Token![=]>()?;
                event_error = Some(input.parse::<Expr>()?);
                input.parse::<Token![;]>()?;
            }
            _ => return Err(syn::Error::new(label.span(), "unknown closed-enum section")),
        }
    }

    let converts = adapters.event() || adapters.conversion();
    if (adapters.command() || adapters.event()) && width.is_none() {
        return Err(syn::Error::new(
            name.span(),
            "command and event enums require an explicit wire width",
        ));
    }
    if converts && try_from_error.is_none() {
        return Err(syn::Error::new(
            name.span(),
            "event and conversion enums require `TryFromError`",
        ));
    }
    if adapters.event() && event_error.is_none() {
        return Err(syn::Error::new(
            name.span(),
            "event enums require `EventError`",
        ));
    }
    if !converts && try_from_error.is_some() {
        return Err(syn::Error::new(
            name.span(),
            "`TryFromError` requires the `event` or `conversion` adapter",
        ));
    }
    if !adapters.event() && event_error.is_some() {
        return Err(syn::Error::new(
            name.span(),
            "`EventError` requires the `event` adapter",
        ));
    }

    Ok(ClosedEnumWireType {
        attrs,
        visibility,
        name,
        repr,
        width,
        variants,
        try_from_error,
        invalid_value,
        event_error,
    })
}

fn parse_open_enum(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<OpenEnumWireType> {
    adapters.reject_conversion(input.span())?;
    let attrs = input.call(Attribute::parse_outer)?;
    let visibility = input.parse::<Visibility>()?;
    input.parse::<Token![enum]>()?;
    let name = input.parse::<Ident>()?;
    input.parse::<Token![:]>()?;
    let repr = input.parse::<Type>()?;
    input.parse::<Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    let (variants, fallback) = parse_enum_variants(input, true)?;
    let fallback = fallback.ok_or_else(|| input.error("open enum requires `_ => Fallback`"))?;
    Ok(OpenEnumWireType {
        attrs,
        visibility,
        name,
        repr,
        width,
        variants,
        fallback,
    })
}

fn parse_enum_variants(
    input: ParseStream<'_>,
    allow_fallback: bool,
) -> syn::Result<(Vec<WireEnumVariant>, Option<Ident>)> {
    let content;
    braced!(content in input);
    let mut variants = Vec::new();
    let mut fallback = None;
    while !content.is_empty() {
        let attrs = content.call(Attribute::parse_outer)?;
        if content.peek(Token![_]) {
            if !allow_fallback {
                return Err(content.error("closed enum cannot declare a fallback variant"));
            }
            content.parse::<Token![_]>()?;
            content.parse::<Token![=>]>()?;
            let name = content.parse::<Ident>()?;
            content.parse::<Token![,]>()?;
            if fallback.replace(name).is_some() {
                return Err(content.error("open enum has more than one fallback variant"));
            }
            if !content.is_empty() {
                return Err(content.error("the fallback variant must be last"));
            }
            continue;
        }
        let name = content.parse::<Ident>()?;
        content.parse::<Token![=]>()?;
        let value = content.parse::<Expr>()?;
        content.parse::<Token![,]>()?;
        variants.push(WireEnumVariant { attrs, name, value });
    }
    if variants.is_empty() {
        return Err(input.error("wire enum requires at least one known variant"));
    }
    Ok((variants, fallback))
}

fn parse_open_scalar(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<OpenScalarWireType> {
    adapters.reject_conversion(input.span())?;
    let attrs = input.call(Attribute::parse_outer)?;
    let visibility = input.parse::<Visibility>()?;
    input.parse::<Token![struct]>()?;
    let name = input.parse::<Ident>()?;
    input.parse::<Token![:]>()?;
    let repr = input.parse::<Type>()?;
    input.parse::<Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    input.parse::<Token![;]>()?;
    Ok(OpenScalarWireType {
        attrs,
        visibility,
        name,
        repr,
        width,
    })
}

fn parse_ranged(input: ParseStream<'_>, adapters: &WireAdapters) -> syn::Result<RangedWireType> {
    adapters.reject_conversion(input.span())?;
    let attrs = input.call(Attribute::parse_outer)?;
    let visibility = input.parse::<Visibility>()?;
    input.parse::<Token![struct]>()?;
    let name = input.parse::<Ident>()?;
    input.parse::<Token![:]>()?;
    let repr = input.parse::<Type>()?;
    input.parse::<Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    let content;
    braced!(content in input);
    parse_label(&content, "minimum")?;
    let minimum = content.parse::<Expr>()?;
    content.parse::<Token![,]>()?;
    parse_label(&content, "maximum")?;
    let maximum = content.parse::<Expr>()?;
    content.parse::<Token![,]>()?;
    let sentinel = if !content.is_empty() {
        parse_label(&content, "sentinel")?;
        let name = content.parse::<Ident>()?;
        content.parse::<Token![=]>()?;
        let value = content.parse::<Expr>()?;
        content.parse::<Token![,]>()?;
        Some(WireSentinel { name, value })
    } else {
        None
    };
    if !content.is_empty() {
        return Err(content.error("unexpected ranged-type field"));
    }
    let event_error = parse_optional_event_error(input, adapters)?;
    Ok(RangedWireType {
        attrs,
        visibility,
        name,
        repr,
        width,
        minimum,
        maximum,
        sentinel,
        event_error,
    })
}

fn parse_bitflags(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<BitflagsWireType> {
    adapters.reject_conversion(input.span())?;
    let attrs = input.call(Attribute::parse_outer)?;
    let visibility = input.parse::<Visibility>()?;
    input.parse::<Token![struct]>()?;
    let name = input.parse::<Ident>()?;
    input.parse::<Token![:]>()?;
    let repr = input.parse::<Type>()?;
    input.parse::<Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    let content;
    braced!(content in input);
    let mut flags = Vec::new();
    while !content.is_empty() {
        let attrs = content.call(Attribute::parse_outer)?;
        content.parse::<Token![const]>()?;
        let name = content.parse::<Ident>()?;
        content.parse::<Token![=]>()?;
        let value = content.parse::<Expr>()?;
        content.parse::<Token![;]>()?;
        flags.push(WireFlag { attrs, name, value });
    }
    if flags.is_empty() {
        return Err(input.error("bitflags declaration requires at least one flag"));
    }
    let event_error = parse_optional_event_error(input, adapters)?;
    Ok(BitflagsWireType {
        attrs,
        visibility,
        name,
        repr,
        width,
        flags,
        event_error,
    })
}

fn parse_optional_event_error(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<Option<Expr>> {
    let event_error = if input.is_empty() {
        None
    } else {
        let label = input.parse::<Ident>()?;
        if label != "EventError" {
            return Err(syn::Error::new(label.span(), "expected `EventError`"));
        }
        input.parse::<Token![=]>()?;
        let error = input.parse::<Expr>()?;
        input.parse::<Token![;]>()?;
        Some(error)
    };
    if adapters.event() != event_error.is_some() {
        return Err(input.error(if adapters.event() {
            "the `event` adapter requires `EventError` for this fallible shape"
        } else {
            "`EventError` requires the `event` adapter"
        }));
    }
    Ok(event_error)
}

fn parse_composite(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<CompositeWireType> {
    adapters.reject_conversion(input.span())?;
    let ty = input.parse::<Type>()?;
    input.parse::<Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    let content;
    braced!(content in input);
    let mut fields = None;
    let mut encode = None;
    let mut decode = None;
    while !content.is_empty() {
        let label = content.parse::<Ident>()?;
        match label.to_string().as_str() {
            "Fields" => {
                if fields.is_some() {
                    return Err(syn::Error::new(label.span(), "duplicate `Fields`"));
                }
                content.parse::<Token![=]>()?;
                let field_content;
                braced!(field_content in content);
                let mut values = Vec::new();
                while !field_content.is_empty() {
                    let name = field_content.parse::<Ident>()?;
                    field_content.parse::<Token![:]>()?;
                    let ty = field_content.parse::<Type>()?;
                    field_content.parse::<Token![=>]>()?;
                    let width = field_content.parse::<LitInt>()?;
                    field_content.parse::<Token![,]>()?;
                    values.push(WireCompositeField { name, ty, width });
                }
                if values.is_empty() {
                    return Err(field_content.error("composite requires at least one field"));
                }
                fields = Some(values);
                content.parse::<Token![;]>()?;
            }
            "Encode" => {
                if encode.is_some() {
                    return Err(syn::Error::new(label.span(), "duplicate `Encode`"));
                }
                content.parse::<Token![=]>()?;
                content.parse::<Token![|]>()?;
                let value = content.parse::<Ident>()?;
                content.parse::<Token![|]>()?;
                let block = content.parse::<Block>()?;
                content.parse::<Token![;]>()?;
                encode = Some((value, block));
            }
            "Decode" => {
                if decode.is_some() {
                    return Err(syn::Error::new(label.span(), "duplicate `Decode`"));
                }
                content.parse::<Token![=]>()?;
                decode = Some(content.parse::<Block>()?);
                content.parse::<Token![;]>()?;
            }
            _ => return Err(syn::Error::new(label.span(), "unknown composite section")),
        }
    }
    if adapters.command() != encode.is_some() {
        return Err(input.error(if adapters.command() {
            "the `command` adapter requires an `Encode` block"
        } else {
            "an `Encode` block requires the `command` adapter"
        }));
    }
    if adapters.event() != decode.is_some() {
        return Err(input.error(if adapters.event() {
            "the `event` adapter requires a `Decode` block"
        } else {
            "a `Decode` block requires the `event` adapter"
        }));
    }
    Ok(CompositeWireType {
        ty,
        width,
        fields: fields.ok_or_else(|| input.error("composite requires `Fields`"))?,
        encode,
        decode,
    })
}

fn parse_primitive(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<PrimitiveWireType> {
    adapters.reject_conversion(input.span())?;
    let ty = input.parse::<Type>()?;
    input.parse::<Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    input.parse::<Token![;]>()?;
    Ok(PrimitiveWireType { ty, width })
}

fn parse_transparent(
    input: ParseStream<'_>,
    adapters: &WireAdapters,
) -> syn::Result<TransparentWireType> {
    adapters.reject_conversion(input.span())?;
    let ty = input.parse::<Type>()?;
    input.parse::<Token![:]>()?;
    let inner = input.parse::<Type>()?;
    input.parse::<Token![=>]>()?;
    let width = input.parse::<LitInt>()?;
    input.parse::<Token![;]>()?;
    Ok(TransparentWireType { ty, inner, width })
}

fn parse_label(input: ParseStream<'_>, expected: &str) -> syn::Result<()> {
    let label = input.parse::<Ident>()?;
    if label != expected {
        return Err(syn::Error::new(
            label.span(),
            format!("expected `{expected}`"),
        ));
    }
    input.parse::<Token![:]>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dual_adapter_ranged_type() {
        let declaration = syn::parse_str::<SemanticWireType>(
            r#"
                adapters: [command, event];
                ranged pub struct L2CocMtu: u16 => 2 {
                    minimum: 23,
                    maximum: u16::MAX,
                }
                EventError = map_mtu_error;
            "#,
        )
        .unwrap();
        assert!(declaration.adapters.command());
        assert!(declaration.adapters.event());
        assert!(matches!(
            declaration.declaration,
            WireTypeDeclaration::Ranged(_)
        ));
    }

    #[test]
    fn validates_adapter_specific_sections() {
        assert!(
            syn::parse_str::<SemanticWireType>(
                r#"
                    adapters: [event];
                    closed enum State: u8 => 1 { Ready = 1, }
                    TryFromError = BadState => BadState;
                "#,
            )
            .is_err()
        );
        assert!(
            syn::parse_str::<SemanticWireType>(
                r#"
                    adapters: [command];
                    composite Value => 2 {
                        Fields = { value: u16 => 2, };
                        Decode = { Ok(Self(value)) };
                    }
                "#,
            )
            .is_err()
        );
    }

    #[test]
    fn parses_cfg_variants_and_conversion_only_enum() {
        let declaration = syn::parse_str::<SemanticWireType>(
            r#"
                adapters: [conversion];
                closed enum Status: u8 {
                    #[cfg(since_fw_0_24_0)]
                    Current = 1,
                }
                TryFromError = Error => Error::BadStatus;
            "#,
        )
        .unwrap();
        let WireTypeDeclaration::ClosedEnum(declaration) = declaration.declaration else {
            panic!("expected closed enum")
        };
        assert!(declaration.width.is_none());
        assert_eq!(declaration.variants.len(), 1);
    }
}
