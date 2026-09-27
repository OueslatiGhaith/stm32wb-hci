//! `vendor_command!`: an ST vendor command whose identity, availability, and
//! wire layout come from the catalog.

use std::ptr;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use stm32wb_catalog::layout::{element_width, struct_width};
use stm32wb_catalog::{
    Bundled, CommandScope, Completion, Element, Field, FieldType, Profile, ReleaseRange, Scalar,
    Structs, UnionVariant, Version, bundled,
};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Ident, LitStr, Token, Type, braced};

use crate::cfg;

/// ```text
/// /// Documentation for the command.
/// aci_hal_read_config_data => HalReadConfigData { offset: u8 } -> HalConfigData {
///     data: BoundedBytes<250>,
/// }
/// ```
#[derive(Clone)]
pub struct Input {
    attrs: Vec<Attribute>,
    c_name: Ident,
    params: Fields,
    /// Return parameters after the status, which bt-hci consumes itself.
    returns: Option<Fields>,
}

/// A named list of fields matched against a catalog layout.
#[derive(Clone)]
struct Fields {
    name: Ident,
    fields: Vec<InputField>,
}

#[derive(Clone)]
struct InputField {
    /// The catalog member this field encodes, when it is not the Rust name.
    wire_name: Option<LitStr>,
    /// The first release with the field; it is declared in no earlier one.
    since: Option<(Version, Span)>,
    name: Ident,
    ty: Type,
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let c_name = input.parse()?;
        input.parse::<Token![=>]>()?;
        let params = input.parse()?;
        let returns = if input.parse::<Option<Token![->]>>()?.is_some() {
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self {
            attrs,
            c_name,
            params,
            returns,
        })
    }
}

impl Parse for Fields {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse()?;
        let body;
        braced!(body in input);
        let fields = Punctuated::<syn::Field, Token![,]>::parse_terminated_with(
            &body,
            syn::Field::parse_named,
        )?
        .into_iter()
        .map(InputField::new)
        .collect::<syn::Result<_>>()?;
        Ok(Self { name, fields })
    }
}

impl InputField {
    fn new(field: syn::Field) -> syn::Result<Self> {
        if !matches!(field.vis, syn::Visibility::Inherited) {
            return Err(syn::Error::new(
                field.vis.span(),
                "fields are always public; remove the visibility",
            ));
        }
        let mut wire_name = None;
        let mut since = None;
        for attr in &field.attrs {
            if !attr.path().is_ident("wire") {
                return Err(syn::Error::new(
                    attr.span(),
                    "only #[wire(name = \"...\", since = \"...\")] is supported on fields",
                ));
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    wire_name = Some(meta.value()?.parse()?);
                    Ok(())
                } else if meta.path.is_ident("since") {
                    let release: LitStr = meta.value()?.parse()?;
                    let version = release
                        .value()
                        .parse::<Version>()
                        .map_err(|error| syn::Error::new(release.span(), error.to_string()))?;
                    since = Some((version, release.span()));
                    Ok(())
                } else {
                    Err(meta
                        .error("expected `name = \"<catalog member>\"` or `since = \"<release>\"`"))
                }
            })?;
        }
        Ok(Self {
            wire_name,
            since,
            name: field.ident.expect("parse_named yields named fields"),
            ty: field.ty,
        })
    }

    /// Whether the field exists in releases starting at `first`.
    fn exists_from(&self, first: Version) -> bool {
        self.since.is_none_or(|(since, _)| since <= first)
    }

    fn matches(&self, member: &str) -> bool {
        match &self.wire_name {
            Some(wire_name) => wire_name.value() == member,
            None => self.name == snake_case(member),
        }
    }
}

pub fn expand(input: Input) -> syn::Result<TokenStream> {
    let bundled = bundled().map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("the bundled catalog is invalid: {error}"),
        )
    })?;
    let variants = Command::resolve(bundled, &input)?;
    let several = variants.len() > 1;
    let mut tokens = TokenStream::new();
    for variant in &variants {
        let expanded = expand_variant(variant).map_err(|error| {
            if several {
                syn::Error::new(
                    error.span(),
                    format!("in releases {}: {error}", variant.releases),
                )
            } else {
                error
            }
        })?;
        tokens.extend(expanded);
    }
    Ok(tokens)
}

/// The declaration for the releases sharing one set of declared fields.
fn expand_variant(command: &Command<'_>) -> syn::Result<TokenStream> {
    let input = &command.input;
    let facts = &command.facts;
    let c_name = &input.c_name;
    let name = &input.params.name;

    let returned = match (facts.completion, &input.returns) {
        (Completion::CommandStatus, Some(returns)) => {
            return Err(syn::Error::new(
                returns.name.span(),
                format!("{c_name} completes with Command Status, which has no return parameters"),
            ));
        }
        (Completion::CommandStatus, None) => None,
        (Completion::CommandComplete, returns) => {
            let after_status = match facts.returns {
                [status, rest @ ..]
                    if status.name == "Status" && status.ty == FieldType::Scalar(Scalar::U8) =>
                {
                    rest
                }
                _ => {
                    return Err(syn::Error::new(
                        c_name.span(),
                        format!("{c_name}'s return parameters do not start with a status"),
                    ));
                }
            };
            match (after_status, returns) {
                ([], None) => None,
                ([], Some(returns)) => {
                    return Err(syn::Error::new(
                        returns.name.span(),
                        format!(
                            "{c_name} returns only a status; remove `-> {}`",
                            returns.name
                        ),
                    ));
                }
                (_, None) => {
                    return Err(syn::Error::new(
                        name.span(),
                        format!(
                            "{c_name} returns {}; declare them with `-> {name}Return {{ ... }}`",
                            after_status
                                .iter()
                                .map(|member| snake_case(&member.name))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    ));
                }
                (members, Some(returns)) => Some((
                    returns,
                    plan(
                        c_name,
                        returns,
                        members,
                        &command.return_names[1..],
                        facts.return_structs,
                        Side::Returns,
                    )?,
                )),
            }
        }
    };
    let params = plan(
        c_name,
        &input.params,
        facts.params,
        &command.param_names,
        facts.structs,
        Side::Params,
    )?;

    let ocf = facts.opcode & 0x03FF;
    let cfg = command
        .cfg
        .as_ref()
        .map(|predicate| quote!(#[cfg(#predicate)]));
    let returns_tokens = match (&returned, facts.completion) {
        (Some((returns, _)), _) => {
            let returns_name = &returns.name;
            quote!(Return = #returns_name;)
        }
        (None, Completion::CommandComplete) => quote!(Return = ();),
        (None, Completion::CommandStatus) => quote!(),
    };
    let widths = width_assertions(&cfg, &input.params.name, &params).chain(
        returned
            .iter()
            .flat_map(|(returns, slots)| width_assertions(&cfg, &returns.name, slots)),
    );
    let return_struct = returned
        .iter()
        .map(|(returns, slots)| return_struct(name, returns, slots, &cfg));
    let attrs = &input.attrs;
    let derived = params
        .iter()
        .any(|slot| matches!(slot, Slot::Count { .. } | Slot::Selector { .. }));
    let declaration = if derived {
        encoded_command(input, &params, &cfg, ocf, &returns_tokens)?
    } else {
        let params_tokens = if input.params.fields.is_empty() {
            quote!(Params = ();)
        } else {
            let params_name = format_ident!("{name}Params");
            let (names, types) = names_and_types(&input.params);
            quote!(#params_name { #(#names: #types,)* })
        };
        quote! {
            #cfg
            ::bt_hci::cmd::cmd! {
                #(#attrs)*
                #name(VENDOR_SPECIFIC, #ocf) {
                    #params_tokens
                    #returns_tokens
                }
            }
        }
    };
    Ok(quote! {
        #declaration
        #(#return_struct)*
        #(#widths)*
    })
}

/// The return parameters after the status, as a plain struct whose fields
/// can be borrowed. Decoding follows the catalog order, reads each count
/// before the field it counts, and leaves the count out of the struct.
fn return_struct(
    command: &Ident,
    returns: &Fields,
    slots: &[Slot<'_>],
    cfg: &Option<TokenStream>,
) -> TokenStream {
    let name = &returns.name;
    let (names, types) = names_and_types(returns);
    let count_of = |counted: &InputField| format_ident!("__{}_count", counted.name);

    let decodes = slots.iter().map(|slot| match slot {
        Slot::Fixed { field, .. } => {
            let (field, ty) = (&field.name, &field.ty);
            quote!(let (#field, rest) = <#ty as ::bt_hci::FromHciBytes<'de>>::from_hci_bytes(rest)?;)
        }
        Slot::Count { counted, scalar } => {
            let (count, scalar) = (count_of(counted), format_ident!("{}", scalar.name()));
            quote!(let (#count, rest) = <#scalar as ::bt_hci::FromHciBytes<'de>>::from_hci_bytes(rest)?;)
        }
        Slot::Bytes { field, .. } => {
            let count = count_of(field);
            let field = &field.name;
            quote! {
                let len = usize::from(#count);
                let bytes = rest.get(..len).ok_or(::bt_hci::FromHciBytesError::InvalidSize)?;
                let rest = &rest[len..];
                let #field = ::stm32wb_hci::wire::BoundedBytes::new(bytes)?;
            }
        }
        Slot::Selector { .. } | Slot::Union { .. } => unreachable!("plan rejects unions in return parameters"),
    });
    let max_lens = slots.iter().map(|slot| {
        let len = match slot {
            Slot::Fixed { width, .. } => usize::from(*width),
            Slot::Count { scalar, .. } => usize::from(scalar.width()),
            Slot::Bytes { capacity, .. } => usize::from(*capacity),
            Slot::Selector { .. } | Slot::Union { .. } => {
                unreachable!("plan rejects unions in return parameters")
            }
        };
        quote!(#len)
    });
    let reads = |asynchronous: bool| {
        let wait = asynchronous.then(|| quote!(.await));
        slots
            .iter()
            .map(|slot| {
                let (len, count) = match slot {
                    Slot::Fixed { width, .. } => {
                        let width = usize::from(*width);
                        (quote!(#width), None)
                    }
                    Slot::Count { counted, scalar } => {
                        let width = usize::from(scalar.width());
                        let scalar = format_ident!("{}", scalar.name());
                        (quote!(#width), Some((count_of(counted), scalar)))
                    }
                    Slot::Bytes { field, .. } => {
                        let count = count_of(field);
                        (quote!(usize::from(#count)), None)
                    }
                    Slot::Selector { .. } | Slot::Union { .. } => unreachable!("plan rejects unions in return parameters"),
                };
                let count = count.map(|(count, scalar)| {
                    quote! {
                        let (#count, _) =
                            <#scalar as ::bt_hci::FromHciBytes<'_>>::from_hci_bytes(&buf[start..filled])?;
                    }
                });
                quote! {
                    let start = filled;
                    filled += #len;
                    reader
                        .read_exact(buf.get_mut(start..filled).ok_or(::bt_hci::ReadHciError::BufferTooSmall)?)
                        #wait?;
                    #count
                }
            })
            .collect::<Vec<_>>()
    };
    let (sync_reads, async_reads) = (reads(false), reads(true));
    let doc = format!("Return parameters of [`{command}`] after the status.");
    quote! {
        #cfg
        #[doc = #doc]
        #[allow(missing_docs)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct #name {
            #(pub #names: #types,)*
        }

        #cfg
        impl<'de> ::bt_hci::FromHciBytes<'de> for #name {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> Result<(Self, &'de [u8]), ::bt_hci::FromHciBytesError> {
                let rest = data;
                #(#decodes)*
                Ok((Self { #(#names),* }, rest))
            }
        }

        #cfg
        impl<'de> ::bt_hci::ReadHci<'de> for #name {
            const MAX_LEN: usize = 0 #(+ #max_lens)*;

            fn read_hci<R: ::embedded_io::Read>(
                mut reader: R,
                buf: &'de mut [u8],
            ) -> Result<Self, ::bt_hci::ReadHciError<R::Error>> {
                let mut filled = 0;
                #(#sync_reads)*
                Ok(<Self as ::bt_hci::FromHciBytes<'_>>::from_hci_bytes_complete(&buf[..filled])?)
            }

            async fn read_hci_async<R: ::embedded_io_async::Read>(
                mut reader: R,
                buf: &'de mut [u8],
            ) -> Result<Self, ::bt_hci::ReadHciError<R::Error>> {
                let mut filled = 0;
                #(#async_reads)*
                Ok(<Self as ::bt_hci::FromHciBytes<'_>>::from_hci_bytes_complete(&buf[..filled])?)
            }
        }
    }
}

/// A command whose parameters bt-hci cannot encode field by field: some
/// member is derived from a declared field, the count of a variable-length
/// field or the selector of a union. bt-hci's cmd! still provides the command
/// itself; the parameters get their own encoding, writing each derived
/// member from the field it serves, and a constructor that enforces the
/// catalog's capacities.
fn encoded_command(
    input: &Input,
    slots: &[Slot<'_>],
    cfg: &Option<TokenStream>,
    ocf: u16,
    returns_tokens: &TokenStream,
) -> syn::Result<TokenStream> {
    let name = &input.params.name;
    let params_name = format_ident!("{name}Params");
    let mut lifetime: Option<&syn::Lifetime> = None;
    for slot in slots {
        if let Slot::Bytes { field, .. } = slot {
            let this = byte_slice_lifetime(&field.ty).ok_or_else(|| {
                syn::Error::new(
                    field.ty.span(),
                    "a variable-length byte field must be declared as `&'a [u8]`",
                )
            })?;
            match lifetime {
                Some(first) if first != this => {
                    return Err(syn::Error::new(
                        this.span(),
                        "all variable-length fields must share one lifetime",
                    ));
                }
                _ => lifetime = Some(this),
            }
        }
    }
    let generics = lifetime.map(|lifetime| quote!(<#lifetime>));

    let (names, types) = names_and_types(&input.params);
    let sizes = slots.iter().map(|slot| match slot {
        Slot::Fixed { field, .. } | Slot::Union { field, .. } => {
            let (field, ty) = (&field.name, &field.ty);
            quote!(<#ty as ::bt_hci::WriteHci>::size(&self.#field))
        }
        Slot::Count { scalar, .. } | Slot::Selector { scalar, .. } => {
            let width = usize::from(scalar.width());
            quote!(#width)
        }
        Slot::Bytes { field, .. } => {
            let field = &field.name;
            quote!(self.#field.len())
        }
    });
    let writes = |asynchronous: bool| {
        let (write_hci, wait) = if asynchronous {
            (quote!(write_hci_async), quote!(.await))
        } else {
            (quote!(write_hci), quote!())
        };
        slots
            .iter()
            .map(move |slot| match slot {
                Slot::Fixed { field, .. } | Slot::Union { field, .. } => {
                    let (field, ty) = (&field.name, &field.ty);
                    quote!(<#ty as ::bt_hci::WriteHci>::#write_hci(&self.#field, &mut writer)#wait?;)
                }
                Slot::Count { counted, scalar } => {
                    let (counted, scalar) = (&counted.name, format_ident!("{}", scalar.name()));
                    quote!(writer.write_all(&(self.#counted.len() as #scalar).to_le_bytes())#wait?;)
                }
                Slot::Selector { union, scalar } => {
                    let (union, ty) = (&union.name, &union.ty);
                    let scalar = format_ident!("{}", scalar.name());
                    quote! {
                        writer
                            .write_all(
                                &(<#ty as ::stm32wb_hci::wire::HciWireUnion>::selector(&self.#union)
                                    as #scalar)
                                    .to_le_bytes(),
                            )
                            #wait?;
                    }
                }
                Slot::Bytes { field, .. } => {
                    let field = &field.name;
                    quote!(writer.write_all(self.#field)#wait?;)
                }
            })
            .collect::<Vec<_>>()
    };
    let (sync_writes, async_writes) = (writes(false), writes(true));
    let checks = slots
        .iter()
        .filter_map(|slot| match slot {
            Slot::Bytes { field, capacity } => {
                let field = &field.name;
                let label = field.to_string();
                let capacity = usize::from(*capacity);
                Some(quote! {
                    if #field.len() > #capacity {
                        return Err(::stm32wb_hci::wire::TooLong {
                            field: #label,
                            len: #field.len(),
                            capacity: #capacity,
                        });
                    }
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let constructor = if checks.is_empty() {
        quote! {
            /// Create a new instance of the command.
            #[allow(clippy::too_many_arguments)]
            pub fn new(#(#names: #types),*) -> Self {
                Self::from(#params_name { #(#names),* })
            }
        }
    } else {
        quote! {
            /// Build the command, rejecting variable-length fields longer than
            /// the capacity the catalog declares for them.
            #[allow(clippy::too_many_arguments)]
            pub fn try_new(#(#names: #types),*) -> Result<Self, ::stm32wb_hci::wire::TooLong> {
                #(#checks)*
                Ok(Self::from(#params_name { #(#names),* }))
            }
        }
    };
    let params_doc = if checks.is_empty() {
        format!("Parameters of [`{name}`], built by [`{name}::new`].")
    } else {
        format!(
            "Parameters of [`{name}`], built by [`{name}::try_new`] within the catalog's \
             capacities."
        )
    };
    let attrs = &input.attrs;
    Ok(quote! {
        #cfg
        #[doc = #params_doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct #params_name #generics {
            #(#names: #types,)*
        }

        #cfg
        impl #generics ::bt_hci::WriteHci for #params_name #generics {
            #[inline]
            fn size(&self) -> usize {
                0 #(+ #sizes)*
            }

            fn write_hci<W: ::embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
                #(#sync_writes)*
                Ok(())
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                #(#async_writes)*
                Ok(())
            }
        }

        // The BASE arm declares the command without the `new(params)`
        // constructor, which the parameters' private fields make unusable.
        #cfg
        ::bt_hci::cmd::cmd! {
            BASE
            #(#attrs)*
            #name(VENDOR_SPECIFIC, #ocf) {
                Params #generics = #params_name #generics;
                #returns_tokens
            }
        }

        #cfg
        impl #generics #name #generics {
            #constructor
        }
    })
}

/// The capacity of a `BoundedBytes<N>` type with a literal `N`.
fn bounded_bytes_capacity(ty: &Type) -> Option<u64> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let arguments = arguments.args.iter().collect::<Vec<_>>();
    let [
        syn::GenericArgument::Const(syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(capacity),
            ..
        })),
    ] = arguments[..]
    else {
        return None;
    };
    (path.qself.is_none() && segment.ident == "BoundedBytes")
        .then(|| capacity.base10_parse().ok())
        .flatten()
}

/// The lifetime of a `&'a [u8]` type.
fn byte_slice_lifetime(ty: &Type) -> Option<&syn::Lifetime> {
    let Type::Reference(reference) = ty else {
        return None;
    };
    let Type::Slice(slice) = &*reference.elem else {
        return None;
    };
    let is_u8 = matches!(&*slice.elem, Type::Path(path) if path.qself.is_none() && path.path.is_ident("u8"));
    (reference.mutability.is_none() && is_u8)
        .then_some(reference.lifetime.as_ref())
        .flatten()
}

fn names_and_types(fields: &Fields) -> (Vec<&Ident>, Vec<&Type>) {
    fields
        .fields
        .iter()
        .map(|field| (&field.name, &field.ty))
        .unzip()
}

/// Assert at compile time that each fixed-width field's type has the
/// catalog's width, and each union's type the catalog's alternatives.
fn width_assertions<'a>(
    cfg: &'a Option<TokenStream>,
    owner: &'a Ident,
    slots: &'a [Slot<'a>],
) -> impl Iterator<Item = TokenStream> + 'a {
    slots.iter().filter_map(move |slot| match slot {
        Slot::Fixed {
            field,
            member,
            width,
        } => {
            let ty = &field.ty;
            let width = usize::from(*width);
            let message = format!(
                "{owner}.{}: the catalog encodes {} in {width} bytes",
                field.name, member.name
            );
            Some(quote_spanned! {ty.span()=>
                #cfg
                const _: () = ::core::assert!(
                    <#ty as ::stm32wb_hci::wire::HciWireType>::WIDTH == #width,
                    #message
                );
            })
        }
        Slot::Union {
            field,
            member,
            variants,
            selector,
        } => {
            let ty = &field.ty;
            let explicit = variants
                .iter()
                .filter_map(|variant| {
                    let width = usize::from(variant.width);
                    variant.tag.map(|tag| quote!((#tag, #width)))
                })
                .collect::<Vec<_>>();
            let default = match variants.iter().find(|variant| variant.tag.is_none()) {
                Some(variant) => {
                    let width = usize::from(variant.width);
                    quote!(::core::option::Option::Some(#width))
                }
                None => quote!(::core::option::Option::None),
            };
            let FieldType::Scalar(scalar) = selector.ty else {
                unreachable!("the catalog validates that selectors are integers")
            };
            let selector_width = usize::from(scalar.width());
            let message = format!(
                "{owner}.{}: the catalog encodes `{member}`, and the type's \
                 HciWireUnion::VARIANTS must list exactly those alternatives",
                field.name
            )
            .replace('{', "{{")
            .replace('}', "}}");
            Some(quote_spanned! {ty.span()=>
                #cfg
                const _: () = ::core::assert!(
                    ::stm32wb_hci::wire::union_matches::<#ty>(
                        &[#(#explicit),*],
                        #default,
                        #selector_width,
                    ),
                    #message
                );
            })
        }
        _ => None,
    })
}

/// The facts a declaration is generated from, identical on the wire in every
/// release and profile the command exists on. Member names are the latest
/// release's.
struct Facts<'a> {
    opcode: u16,
    completion: Completion,
    params: &'a [Field],
    structs: &'a Structs,
    /// Every return parameter, including the leading status.
    returns: &'a [Field],
    return_structs: &'a Structs,
}

impl Facts<'_> {
    /// Whether both encode the same bytes, whatever their members are named.
    fn same_wire(&self, other: &Facts<'_>) -> bool {
        self.opcode == other.opcode
            && self.completion == other.completion
            && self.structs == other.structs
            && self.return_structs == other.return_structs
            && same_layout(self.params, other.params)
            && same_layout(self.returns, other.returns)
    }
}

/// Whether two member lists have the same types in the same order, with
/// counts and selectors referring to the same positions.
fn same_layout(left: &[Field], right: &[Field]) -> bool {
    /// A member's type with the members it refers to replaced by their positions.
    fn shape(members: &[Field], member: &Field) -> FieldType {
        let position = |name: &str| {
            members
                .iter()
                .position(|other| other.name == name)
                .expect("the catalog validates that referenced members exist")
                .to_string()
        };
        match &member.ty {
            FieldType::Counted {
                element,
                count,
                capacity,
            } => FieldType::Counted {
                element: element.clone(),
                count: position(count),
                capacity: *capacity,
            },
            FieldType::Union { selector, variants } => FieldType::Union {
                selector: position(selector),
                variants: variants.clone(),
            },
            ty => ty.clone(),
        }
    }
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(l, r)| shape(left, l) == shape(right, r))
}

/// The declaration for the releases in which the same fields exist.
struct Command<'a> {
    /// The declaration without the fields its releases lack.
    input: Input,
    facts: Facts<'a>,
    /// Every name each parameter had, by position, latest first.
    param_names: Vec<Vec<&'a str>>,
    /// Every name each return parameter had, by position, latest first.
    return_names: Vec<Vec<&'a str>>,
    /// The span of the releases, for messages.
    releases: ReleaseRange,
    cfg: Option<TokenStream>,
}

/// Releases and the profiles supporting a command in them.
type Targets<'a> = Vec<(ReleaseRange, &'a [Profile])>;

/// Add the names of `members` to the names each position had so far.
fn record_names<'a>(names: &mut Vec<Vec<&'a str>>, members: &'a [Field]) {
    names.resize_with(members.len(), Vec::new);
    for (names, member) in names.iter_mut().zip(members) {
        if !names.contains(&member.name.as_str()) {
            names.insert(0, &member.name);
        }
    }
}

/// The declared fields existing in releases starting at `first`.
fn fields_from(fields: &Fields, first: Version) -> Fields {
    Fields {
        name: fields.name.clone(),
        fields: fields
            .fields
            .iter()
            .filter(|field| field.exists_from(first))
            .cloned()
            .collect(),
    }
}

impl<'a> Command<'a> {
    /// One declaration per set of fields that exist together, each for the
    /// releases and profiles having exactly those fields.
    fn resolve(bundled: &'a Bundled, input: &Input) -> syn::Result<Vec<Self>> {
        let c_name = input.c_name.to_string();
        let error = |message: String| syn::Error::new(input.c_name.span(), message);
        let segments = bundled
            .command_segments(&c_name)
            .map_err(|failure| error(failure.to_string()))?;

        let fields = || {
            input
                .params
                .fields
                .iter()
                .chain(input.returns.iter().flat_map(|returns| &returns.fields))
        };
        for field in fields() {
            let Some((since, span)) = field.since else {
                continue;
            };
            if !bundled.catalog.versions().any(|release| release == since) {
                return Err(syn::Error::new(
                    span,
                    format!("{since} is not a release the catalog describes"),
                ));
            }
            if let Some(segment) = segments
                .iter()
                .find(|segment| segment.releases.first < since && since <= segment.releases.last)
            {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "{c_name} has one layout throughout {}; `{}` cannot start in {since}",
                        segment.releases, field.name
                    ),
                ));
            }
            if segments
                .iter()
                .all(|segment| field.exists_from(segment.releases.first))
            {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "`{}` exists in every release of {c_name}; remove `since`",
                        field.name
                    ),
                ));
            }
        }

        // Each variant with the fields it declares and the targets it covers.
        let mut variants: Vec<(Vec<bool>, Self, Targets<'a>)> = Vec::new();
        for segment in &segments {
            let active = &segment.entry;
            let releases = segment.releases;
            if active.command.scope != CommandScope::Vendor {
                return Err(error(format!("{c_name} is not an ST vendor command")));
            }
            if let Some(exclusion) = active.excluded {
                return Err(error(format!(
                    "{c_name} is excluded in {releases}: {}",
                    exclusion.reason
                )));
            }
            let params = active.params.fields.map_err(|reason| {
                error(format!(
                    "{c_name} has no derivable parameter layout in {releases}: {reason}"
                ))
            })?;
            let (returns, return_structs) = match &active.returns {
                Some(returns) => (
                    returns.fields.map_err(|reason| {
                        error(format!(
                            "{c_name} has no derivable return layout in {releases}: {reason}"
                        ))
                    })?,
                    returns.structs,
                ),
                None => (&[][..], active.params.structs),
            };
            let current = Facts {
                opcode: active.command.opcode,
                completion: active.definition.completion,
                params,
                structs: active.params.structs,
                returns,
                return_structs,
            };

            let key = fields()
                .map(|field| field.exists_from(releases.first))
                .collect::<Vec<_>>();
            let Some((_, variant, targets)) = variants.iter_mut().find(|(other, ..)| *other == key)
            else {
                // A release without any of the declared return fields, which
                // are all added later, returns only a status.
                let declared_returns = input
                    .returns
                    .as_ref()
                    .map(|returns| fields_from(returns, releases.first))
                    .filter(|returns| {
                        !returns.fields.is_empty()
                            || input
                                .returns
                                .as_ref()
                                .is_some_and(|declared| declared.fields.is_empty())
                    });
                let mut variant = Self {
                    input: Input {
                        attrs: input.attrs.clone(),
                        c_name: input.c_name.clone(),
                        params: fields_from(&input.params, releases.first),
                        returns: declared_returns,
                    },
                    facts: current,
                    param_names: Vec::new(),
                    return_names: Vec::new(),
                    releases,
                    cfg: None,
                };
                record_names(&mut variant.param_names, params);
                record_names(&mut variant.return_names, returns);
                variants.push((key, variant, vec![(releases, segment.profiles)]));
                continue;
            };
            if !variant.facts.same_wire(&current) {
                return Err(error(format!(
                    "{c_name} changes its opcode, completion, or layout in {releases}; if it \
                     adds members there, declare them with #[wire(since = \"{}\")]",
                    releases.first
                )));
            }
            record_names(&mut variant.param_names, current.params);
            record_names(&mut variant.return_names, current.returns);
            variant.facts = current;
            variant.releases.first = variant.releases.first.min(releases.first);
            variant.releases.last = variant.releases.last.max(releases.last);
            targets.push((releases, segment.profiles));
        }

        Ok(variants
            .into_iter()
            .map(|(_, mut variant, targets)| {
                variant.cfg = cfg::targets(&bundled.catalog, targets);
                variant
            })
            .collect())
    }
}

/// Which side of the command a layout describes.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Params,
    Returns,
}

impl Side {
    fn what(self) -> &'static str {
        match self {
            Side::Params => "parameter",
            Side::Returns => "return parameter",
        }
    }
}

/// How one catalog member is encoded, in catalog order.
enum Slot<'a> {
    /// A declared fixed-width field.
    Fixed {
        field: &'a InputField,
        member: &'a Field,
        width: u16,
    },
    /// The element count of a variable-length field, derived from its length
    /// rather than declared.
    Count {
        counted: &'a InputField,
        scalar: Scalar,
    },
    /// A declared variable-length byte field.
    Bytes {
        field: &'a InputField,
        capacity: u16,
    },
    /// The selector of a union, derived from the alternative of the union's
    /// value rather than declared.
    Selector {
        union: &'a InputField,
        scalar: Scalar,
    },
    /// A declared union, whose type's alternatives must be the catalog's.
    Union {
        field: &'a InputField,
        member: &'a Field,
        variants: &'a [UnionVariant],
        selector: &'a Field,
    },
}

/// The member another member is derived from: the variable-length field a
/// count counts, or the union a selector selects.
fn dependent<'a>(members: &'a [Field], name: &str) -> Option<&'a Field> {
    members.iter().find(|other| match &other.ty {
        FieldType::Counted { count, .. } => count == name,
        FieldType::Union { selector, .. } => selector == name,
        _ => false,
    })
}

/// Match declared fields to catalog members in order and decide how each
/// member is encoded. Members counting a variable-length field or selecting
/// a union's alternative are derived from that field and not declared.
fn plan<'a>(
    c_name: &Ident,
    declared: &'a Fields,
    members: &'a [Field],
    names: &[Vec<&str>],
    structs: &Structs,
    side: Side,
) -> syn::Result<Vec<Slot<'a>>> {
    let what = side.what();
    // Whether `field` declares `member` under any name it had.
    let declares = |field: &InputField, member: &Field| {
        let position = members
            .iter()
            .position(|other| ptr::eq(other, member))
            .expect("the member is one of the members");
        names[position].iter().any(|name| field.matches(name))
    };
    let visible = members
        .iter()
        .filter(|member| dependent(members, &member.name).is_none())
        .collect::<Vec<_>>();

    for field in &declared.fields {
        let Some((derived, dependent)) = members.iter().find_map(|member| {
            dependent(members, &member.name)
                .filter(|_| declares(field, member))
                .map(|dependent| (member, dependent))
        }) else {
            continue;
        };
        let source = match dependent.ty {
            FieldType::Union { .. } => "alternative",
            _ => "length",
        };
        return Err(syn::Error::new(
            field.name.span(),
            format!(
                "{c_name} writes {} from the {source} of {}; remove `{}`",
                derived.name, dependent.name, field.name
            ),
        ));
    }
    for (index, member) in visible.iter().enumerate() {
        let Some(field) = declared.fields.get(index) else {
            let missing = visible[index..]
                .iter()
                .map(|member| snake_case(&member.name))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(syn::Error::new(
                declared.name.span(),
                format!(
                    "{} is missing {what}s of {c_name}: {missing}",
                    declared.name
                ),
            ));
        };
        if !declares(field, member) {
            let expected = snake_case(&member.name);
            return Err(syn::Error::new(
                field.name.span(),
                format!(
                    "{what} {} of {c_name} is `{}` in the catalog; name this field \
                     `{expected}` or add #[wire(name = \"{}\")]",
                    index + 1,
                    member.name,
                    member.name
                ),
            ));
        }
    }
    if let Some(extra) = declared.fields.get(visible.len()) {
        return Err(syn::Error::new(
            extra.name.span(),
            format!(
                "{c_name} has {} {what}s to declare; `{}` is not one of them",
                visible.len(),
                extra.name
            ),
        ));
    }

    let field_of = |member: &Field| {
        visible
            .iter()
            .position(|visible| visible.name == member.name)
            .map(|index| &declared.fields[index])
    };
    members
        .iter()
        .map(|member| {
            let Some(field) = field_of(member) else {
                let dependent =
                    dependent(members, &member.name).expect("hidden members are derived");
                let field = field_of(dependent).expect("derived members serve declared fields");
                let FieldType::Scalar(scalar) = member.ty else {
                    unreachable!("the catalog validates that counts and selectors are integers")
                };
                return Ok(match dependent.ty {
                    FieldType::Union { .. } => Slot::Selector {
                        union: field,
                        scalar,
                    },
                    _ => Slot::Count {
                        counted: field,
                        scalar,
                    },
                });
            };
            let unsupported = |kind: &str| {
                Err(syn::Error::new(
                    field.name.span(),
                    format!("`{member}` is {kind}; such {what}s are not supported yet"),
                ))
            };
            let width = match &member.ty {
                FieldType::Scalar(scalar) => Ok(scalar.width()),
                FieldType::Struct(name) => struct_width(name, structs),
                FieldType::Array { element, len } => {
                    element_width(element, structs).map(|width| width * len)
                }
                FieldType::Counted {
                    element: Element::Scalar(Scalar::U8),
                    count,
                    capacity,
                } => {
                    let (count_index, count_width) = members
                        .iter()
                        .enumerate()
                        .find(|(_, other)| other.name == *count)
                        .map(|(index, other)| match other.ty {
                            FieldType::Scalar(scalar) => (index, scalar.width()),
                            _ => unreachable!("the catalog validates that counts are integers"),
                        })
                        .expect("the catalog validates that counts exist");
                    if u64::from(*capacity) >= 1 << (8 * u32::from(count_width)) {
                        return Err(syn::Error::new(
                            field.name.span(),
                            format!("`{member}` has a capacity its {count} cannot express"),
                        ));
                    }
                    if side == Side::Returns {
                        let member_index = members
                            .iter()
                            .position(|other| ptr::eq(other, member))
                            .expect("the member is one of the members");
                        if count_index > member_index {
                            return unsupported("counted by a later member");
                        }
                        if bounded_bytes_capacity(&field.ty) != Some(u64::from(*capacity)) {
                            return Err(syn::Error::new(
                                field.ty.span(),
                                format!(
                                    "`{member}` holds up to {capacity} bytes; declare it as \
                                     `BoundedBytes<{capacity}>`"
                                ),
                            ));
                        }
                    }
                    return Ok(Slot::Bytes {
                        field,
                        capacity: *capacity,
                    });
                }
                FieldType::Counted { .. } => return unsupported("a variable-length array"),
                FieldType::Union { .. } if side == Side::Returns => {
                    return unsupported("a union");
                }
                FieldType::Union { selector, variants } => {
                    return Ok(Slot::Union {
                        field,
                        member,
                        variants,
                        selector: members
                            .iter()
                            .find(|other| other.name == *selector)
                            .expect("the catalog validates that selectors exist"),
                    });
                }
            };
            width
                .map(|width| Slot::Fixed {
                    field,
                    member,
                    width,
                })
                .map_err(|error| syn::Error::new(field.name.span(), error.to_string()))
        })
        .collect()
}

/// The Rust spelling of a catalog member: `Radio_Activity_Mask` and
/// `RadioActivityMask` both become `radio_activity_mask`.
fn snake_case(member: &str) -> String {
    let mut name = String::with_capacity(member.len() + 4);
    let mut previous: Option<char> = None;
    for character in member.chars() {
        if character.is_ascii_uppercase()
            && previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
        {
            name.push('_');
        }
        name.push(character.to_ascii_lowercase());
        previous = Some(character);
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_str(source: &str) -> Result<String, String> {
        let input = syn::parse_str::<Input>(source).map_err(|error| error.to_string())?;
        expand(input)
            .map(|tokens| tokens.to_string())
            .map_err(|error| error.to_string())
    }

    fn expand_err(source: &str) -> String {
        expand_str(source).expect_err(source)
    }

    #[test]
    fn member_names_become_snake_case() {
        assert_eq!(snake_case("Radio_Activity_Mask"), "radio_activity_mask");
        assert_eq!(snake_case("Freq_offset"), "freq_offset");
        assert_eq!(snake_case("PA_Level"), "pa_level");
        assert_eq!(snake_case("StartAddress"), "start_address");
        assert_eq!(snake_case("RSSI"), "rssi");
        assert_eq!(snake_case("reg_val"), "reg_val");
    }

    #[test]
    fn commands_expand_to_bt_hci_declarations() {
        let tokens = expand_str(
            "/// Set the mask.
             aci_hal_set_radio_activity_mask => HalSetRadioActivityMask {
                 radio_activity_mask: u16,
             }",
        )
        .unwrap();
        assert!(tokens.contains("cmd !"), "{tokens}");
        assert!(
            tokens.contains("HalSetRadioActivityMask (VENDOR_SPECIFIC , 24u16)"),
            "{tokens}"
        );
        assert!(tokens.contains("HalSetRadioActivityMaskParams"), "{tokens}");
        assert!(tokens.contains("Return = ()"), "{tokens}");
        assert!(tokens.contains("WIDTH == 2usize"), "{tokens}");
        assert!(!tokens.contains("cfg"), "available everywhere: {tokens}");

        let tokens = expand_str("aci_hal_tone_stop => HalToneStop {}").unwrap();
        assert!(tokens.contains("Params = ()"), "{tokens}");
    }

    #[test]
    fn availability_becomes_cfg() {
        let tokens = expand_str("aci_hal_stack_reset => HalStackReset {}").unwrap();
        assert!(tokens.contains("feature = \"fw_1_22_1\""), "{tokens}");
        assert!(!tokens.contains("fw_1_23_0"), "{tokens}");

        let tokens =
            expand_str("aci_hal_set_event_mask => HalSetEventMask { event_mask: u32 }").unwrap();
        assert!(
            tokens.contains("feature = \"stack-full-extended\""),
            "{tokens}"
        );
        assert!(tokens.contains("feature = \"stack-light\""), "{tokens}");
    }

    #[test]
    fn wire_names_override_the_spelling() {
        expand_str(
            "aci_hal_tone_start => HalToneStart {
                 #[wire(name = \"RF_Channel\")]
                 channel: u8,
                 freq_offset: u8,
             }",
        )
        .unwrap();
    }

    #[test]
    fn declarations_must_match_the_catalog() {
        let error = expand_err("aci_hal_missing => Missing {}");
        assert!(error.contains("aci_hal_missing"), "{error}");

        let error =
            expand_err("aci_hal_tone_start => HalToneStart { freq_offset: u8, rf_channel: u8 }");
        assert!(error.contains("`RF_Channel`"), "{error}");

        let error = expand_err("aci_hal_tone_start => HalToneStart { rf_channel: u8 }");
        assert!(
            error.contains("missing parameters of aci_hal_tone_start: freq_offset"),
            "{error}"
        );

        let error = expand_err(
            "aci_hal_set_radio_activity_mask => HalSetRadioActivityMask {
                 radio_activity_mask: u16,
                 extra: u8,
             }",
        );
        assert!(
            error.contains("1 parameters to declare; `extra`"),
            "{error}"
        );

        let error = expand_err("aci_hal_get_fw_build_number => HalGetFwBuildNumber {}");
        assert!(
            error.contains("returns build_number; declare them with `-> HalGetFwBuildNumberReturn"),
            "{error}"
        );

        let error = expand_err("hci_reset => Reset {}");
        assert!(error.contains("not an ST vendor command"), "{error}");

        let error = expand_err(
            "aci_hal_tone_start => HalToneStart { #[doc = \"x\"] rf_channel: u8, freq_offset: u8 }",
        );
        assert!(error.contains("#[wire"), "{error}");
    }

    #[test]
    fn return_parameters_follow_the_status() {
        let tokens = expand_str(
            "aci_hal_get_anchor_period => HalGetAnchorPeriod {} -> HalAnchorPeriod {
                 anchor_period: u32,
                 max_free_slot: u32,
             }",
        )
        .unwrap();
        assert!(tokens.contains("Params = ()"), "{tokens}");
        assert!(tokens.contains("Return = HalAnchorPeriod ;"), "{tokens}");
        assert!(
            tokens.contains(
                "pub struct HalAnchorPeriod { pub anchor_period : u32 , pub max_free_slot : u32 , }"
            ),
            "{tokens}"
        );
        assert!(!tokens.contains("packed"), "{tokens}");
        assert!(tokens.contains("WIDTH == 4usize"), "{tokens}");

        let tokens = expand_str(
            "aci_hal_get_link_status => HalGetLinkStatus {} -> HalLinkStatus {
                 link_status: [u8; 8],
                 link_connection_handle: [u16; 8],
             }",
        )
        .unwrap();
        assert!(tokens.contains("WIDTH == 16usize"), "{tokens}");

        let error = expand_err(
            "aci_hal_get_anchor_period => HalGetAnchorPeriod {} -> HalAnchorPeriod {
                 max_free_slot: u32,
                 anchor_period: u32,
             }",
        );
        assert!(error.contains("return parameter 1"), "{error}");

        let error = expand_err("aci_hal_tone_stop => HalToneStop {} -> Nothing {}");
        assert!(error.contains("returns only a status"), "{error}");
    }

    #[test]
    fn command_status_commands_have_no_returns() {
        let tokens = expand_str(
            "aci_gap_peripheral_security_req => GapPeripheralSecurityReq { connection_handle: u16 }",
        )
        .unwrap();
        assert!(!tokens.contains("Return"), "{tokens}");

        let error = expand_err(
            "aci_gap_peripheral_security_req => GapPeripheralSecurityReq {
                 connection_handle: u16,
             } -> Nothing {}",
        );
        assert!(error.contains("Command Status"), "{error}");
    }

    #[test]
    fn renamed_members_keep_every_name() {
        let tokens = expand_str(
            "aci_l2cap_connection_parameter_update_req => L2capConnectionParameterUpdateReq {
                 connection_handle: u16,
                 conn_interval_min: u16,
                 conn_interval_max: u16,
                 latency: u16,
                 timeout_multiplier: u16,
             }",
        )
        .unwrap();
        assert!(
            tokens.contains("feature = \"fw_1_15_0\""),
            "declared on releases using the older name too: {tokens}"
        );

        expand_str(
            "aci_l2cap_connection_parameter_update_req => L2capConnectionParameterUpdateReq {
                 connection_handle: u16,
                 conn_interval_min: u16,
                 conn_interval_max: u16,
                 slave_latency: u16,
                 timeout_multiplier: u16,
             }",
        )
        .unwrap();

        let error = expand_err(
            "aci_l2cap_connection_parameter_update_req => L2capConnectionParameterUpdateReq {
                 connection_handle: u16,
                 conn_interval_min: u16,
                 conn_interval_max: u16,
                 peripheral_latency: u16,
                 timeout_multiplier: u16,
             }",
        );
        assert!(error.contains("name this field `latency`"), "{error}");

        let error = expand_err(
            "aci_l2cap_coc_connect_confirm => L2capCocConnectConfirm {
                 connection_handle: u16,
                 mtu: u16,
                 mps: u16,
                 initial_credits: u16,
                 result: u16,
             } -> L2capCocChannels { channel_index_list: BoundedBytes<250> }",
        );
        assert!(
            error.contains("changes its opcode, completion, or layout in 1.23.0"),
            "{error}"
        );
    }

    const CONNECT_CONFIRM: &str = "aci_l2cap_coc_connect_confirm => L2capCocConnectConfirm {
        connection_handle: u16,
        mtu: u16,
        mps: u16,
        initial_credits: u16,
        result: u16,
        #[wire(since = \"SINCE\")]
        max_channel_number: u8,
    } -> L2capCocChannels { channel_index_list: BoundedBytes<250> }";

    #[test]
    fn added_members_start_in_their_release() {
        let tokens = expand_str(&CONNECT_CONFIRM.replace("SINCE", "1.23.0")).unwrap();
        assert_eq!(
            tokens.matches(":: bt_hci :: cmd :: cmd !").count(),
            2,
            "{tokens}"
        );
        assert_eq!(
            tokens.matches("max_channel_number : u8").count(),
            1,
            "only the later variant has the field: {tokens}"
        );
        let later = tokens.find("feature = \"fw_1_23_0\"").unwrap();
        let earlier = tokens.find("feature = \"fw_1_22_1\"").unwrap();
        let field = tokens.find("max_channel_number : u8").unwrap();
        assert!(earlier < later && later < field, "{tokens}");

        let error = expand_err(&CONNECT_CONFIRM.replace("SINCE", "1.22.1"));
        assert!(
            error.contains(
                "one layout throughout 1.15.0..=1.22.1; `max_channel_number` cannot start in 1.22.1"
            ),
            "{error}"
        );
        let error = expand_err(&CONNECT_CONFIRM.replace("SINCE", "1.24.0"));
        assert!(error.contains("cannot start in 1.24.0"), "{error}");
        let error = expand_err(&CONNECT_CONFIRM.replace("SINCE", "1.30.0"));
        assert!(error.contains("1.30.0 is not a release"), "{error}");
        let error = expand_err(&CONNECT_CONFIRM.replace("SINCE", "one"));
        assert!(error.contains("invalid Cube release"), "{error}");
        let error = expand_err(&CONNECT_CONFIRM.replace("SINCE", "1.15.0"));
        assert!(
            error.contains("`max_channel_number` exists in every release"),
            "{error}"
        );

        let error = expand_err(
            "aci_l2cap_coc_connect_confirm => L2capCocConnectConfirm {
                 connection_handle: u16,
                 mtu: u16,
                 mps: u16,
                 #[wire(since = \"1.23.0\")]
                 initial_credits: u16,
                 result: u16,
             } -> L2capCocChannels { channel_index_list: BoundedBytes<250> }",
        );
        assert!(
            error.starts_with("in releases 1.15.0..=1.22.1: "),
            "{error}"
        );
    }

    #[test]
    fn added_return_members_start_in_their_release() {
        let tokens = expand_str(
            "aci_gap_check_bonded_device => GapCheckBondedDevice {
                 peer_address_type: u8,
                 peer_address: [u8; 6],
             } -> GapBondedDevice {
                 #[wire(since = \"1.22.0\")]
                 id_address_type: u8,
                 #[wire(since = \"1.22.0\")]
                 id_address: [u8; 6],
             }",
        )
        .unwrap();
        assert!(tokens.contains("Return = () ;"), "{tokens}");
        assert!(tokens.contains("Return = GapBondedDevice ;"), "{tokens}");
    }

    #[test]
    fn counted_bytes_derive_their_count() {
        let tokens = expand_str(
            "aci_hal_write_config_data => HalWriteConfigData {
                 offset: u8,
                 value: &'a [u8],
             }",
        )
        .unwrap();
        assert!(
            tokens.contains("Params < 'a > = HalWriteConfigDataParams < 'a >"),
            "{tokens}"
        );
        assert!(tokens.contains("self . value . len () as u8"), "{tokens}");
        assert!(tokens.contains("fn try_new"), "{tokens}");
        assert!(tokens.contains("value . len () > 253usize"), "{tokens}");

        let tokens = expand_str(
            "aci_l2cap_coc_tx_data => L2capCocTxData { channel_index: u8, data: &'a [u8] }",
        )
        .unwrap();
        assert!(tokens.contains("self . data . len () as u16"), "{tokens}");

        let error = expand_err(
            "aci_hal_write_config_data => HalWriteConfigData {
                 offset: u8,
                 length: u8,
                 value: &'a [u8],
             }",
        );
        assert!(
            error.contains("writes Length from the length of Value; remove `length`"),
            "{error}"
        );

        for ty in ["&'a [u16]", "&'a mut [u8]", "[u8; 4]"] {
            let error = expand_err(&format!(
                "aci_hal_write_config_data => HalWriteConfigData {{ offset: u8, value: {ty} }}"
            ));
            assert!(
                error.contains("must be declared as `&'a [u8]`"),
                "{ty}: {error}"
            );
        }
    }

    #[test]
    fn union_selectors_derive_from_the_alternative() {
        let tokens = expand_str(
            "aci_gatt_add_service => GattAddService {
                 service_uuid: Uuid,
                 service_type: u8,
                 max_attribute_records: u8,
             } -> GattService { service_handle: u16 }",
        )
        .unwrap();
        assert!(
            tokens.contains("HciWireUnion > :: selector (& self . service_uuid) as u8"),
            "{tokens}"
        );
        assert!(
            tokens.contains("union_matches :: < Uuid > (& [(1u64 , 2usize) , (2u64 , 16usize)] , :: core :: option :: Option :: None , 1usize ,)"),
            "{tokens}"
        );
        assert!(
            tokens.contains("pub fn new (service_uuid : Uuid"),
            "{tokens}"
        );
        assert!(!tokens.contains("try_new"), "{tokens}");
        assert!(tokens.contains("BASE"), "{tokens}");

        let tokens = expand_str(
            "aci_gatt_include_service => GattIncludeService {
                 service_handle: u16,
                 include_start_handle: u16,
                 include_end_handle: u16,
                 include_uuid: Uuid,
             } -> GattInclude { include_handle: u16 }",
        )
        .unwrap();
        assert!(
            tokens.contains("& [(2u64 , 16usize)] , :: core :: option :: Option :: Some (2usize)"),
            "{tokens}"
        );

        let error = expand_err(
            "aci_gatt_add_service => GattAddService {
                 service_uuid_type: u8,
                 service_uuid: Uuid,
                 service_type: u8,
                 max_attribute_records: u8,
             } -> GattService { service_handle: u16 }",
        );
        assert!(
            error.contains(
                "writes Service_UUID_Type from the alternative of Service_UUID; remove \
                 `service_uuid_type`"
            ),
            "{error}"
        );
    }

    #[test]
    fn counted_return_bytes_are_bounded() {
        let tokens = expand_str(
            "aci_hal_read_config_data => HalReadConfigData { offset: u8 } -> HalConfigData {
                 data: BoundedBytes<250>,
             }",
        )
        .unwrap();
        assert!(
            tokens.contains("pub struct HalConfigData { pub data : BoundedBytes < 250 > , }"),
            "{tokens}"
        );
        assert!(
            tokens.contains("MAX_LEN : usize = 0 + 1usize + 250usize"),
            "{tokens}"
        );

        for ty in ["BoundedBytes<251>", "&'a [u8]", "[u8; 250]"] {
            let error = expand_err(&format!(
                "aci_hal_read_config_data => HalReadConfigData {{ offset: u8 }} -> HalConfigData {{
                     data: {ty},
                 }}"
            ));
            assert!(
                error.contains("holds up to 250 bytes; declare it as `BoundedBytes<250>`"),
                "{ty}: {error}"
            );
        }

        let error = expand_err(
            "aci_hal_read_config_data => HalReadConfigData { offset: u8 } -> HalConfigData {
                 data_length: u8,
                 data: BoundedBytes<250>,
             }",
        );
        assert!(
            error.contains("writes Data_Length from the length of Data"),
            "{error}"
        );
    }
}
