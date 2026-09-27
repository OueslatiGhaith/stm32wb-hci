//! `vendor_command!`: an ST vendor command whose identity, availability, and
//! wire layout come from the catalog.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use stm32wb_catalog::layout::{element_width, struct_width};
use stm32wb_catalog::{
    Bundled, CommandScope, Completion, Element, Field, FieldType, Scalar, Structs, bundled,
};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Ident, LitStr, Token, Type, braced};

use crate::cfg;

/// ```text
/// /// Documentation for the command.
/// aci_hal_get_anchor_period => HalGetAnchorPeriod {} -> HalAnchorPeriod {
///     anchor_period: u32,
///     max_free_slot: u32,
/// }
/// ```
pub struct Input {
    attrs: Vec<Attribute>,
    c_name: Ident,
    params: Fields,
    /// Return parameters after the status, which bt-hci consumes itself.
    returns: Option<Fields>,
}

/// A named list of fields matched against a catalog layout.
struct Fields {
    name: Ident,
    fields: Vec<InputField>,
}

struct InputField {
    /// The catalog member this field encodes, when it is not the Rust name.
    wire_name: Option<LitStr>,
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
        for attr in &field.attrs {
            if !attr.path().is_ident("wire") {
                return Err(syn::Error::new(
                    attr.span(),
                    "only #[wire(name = \"...\")] is supported on fields",
                ));
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    wire_name = Some(meta.value()?.parse()?);
                    Ok(())
                } else {
                    Err(meta.error("expected `name = \"<catalog member>\"`"))
                }
            })?;
        }
        Ok(Self {
            wire_name,
            name: field.ident.expect("parse_named yields named fields"),
            ty: field.ty,
        })
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
    let command = Command::resolve(bundled, &input)?;
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
                        facts.return_structs,
                        "return parameter",
                        false,
                    )?,
                )),
            }
        }
    };
    let params = plan(
        c_name,
        &input.params,
        facts.params,
        facts.structs,
        "parameter",
        true,
    )?;

    let ocf = facts.opcode & 0x03FF;
    let cfg = command
        .cfg
        .as_ref()
        .map(|predicate| quote!(#[cfg(#predicate)]));
    let returns_tokens = match (&returned, facts.completion) {
        (Some((returns, _)), _) => {
            let returns_name = &returns.name;
            let (names, types) = names_and_types(returns);
            quote!(#returns_name { #(#names: #types,)* })
        }
        (None, Completion::CommandComplete) => quote!(Return = ();),
        (None, Completion::CommandStatus) => quote!(),
    };
    let widths = width_assertions(&cfg, &input.params.name, &params).chain(
        returned
            .iter()
            .flat_map(|(returns, slots)| width_assertions(&cfg, &returns.name, slots)),
    );
    let attrs = &input.attrs;
    let declaration = if params.iter().any(|slot| matches!(slot, Slot::Bytes { .. })) {
        variable_command(&input, &params, &cfg, ocf, &returns_tokens)?
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
        #(#widths)*
    })
}

/// A command with variable-length parameters. bt-hci's cmd! still provides
/// the command itself; the parameters get their own encoding, which derives
/// each count from the length of the field it counts, and a constructor that
/// enforces the catalog's capacities.
fn variable_command(
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
    let lifetime = lifetime.expect("variable commands have a byte field");

    let (names, types) = names_and_types(&input.params);
    let sizes = slots.iter().map(|slot| match slot {
        Slot::Fixed { field, .. } => {
            let (field, ty) = (&field.name, &field.ty);
            quote!(<#ty as ::bt_hci::WriteHci>::size(&self.#field))
        }
        Slot::Count { scalar, .. } => {
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
                Slot::Fixed { field, .. } => {
                    let (field, ty) = (&field.name, &field.ty);
                    quote!(<#ty as ::bt_hci::WriteHci>::#write_hci(&self.#field, &mut writer)#wait?;)
                }
                Slot::Count { counted, scalar } => {
                    let (counted, scalar) = (&counted.name, format_ident!("{}", scalar.name()));
                    quote!(writer.write_all(&(self.#counted.len() as #scalar).to_le_bytes())#wait?;)
                }
                Slot::Bytes { field, .. } => {
                    let field = &field.name;
                    quote!(writer.write_all(self.#field)#wait?;)
                }
            })
            .collect::<Vec<_>>()
    };
    let (sync_writes, async_writes) = (writes(false), writes(true));
    let checks = slots.iter().filter_map(|slot| match slot {
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
    });
    let params_doc = format!(
        "Parameters of [`{name}`], built by [`{name}::try_new`] within the catalog's capacities."
    );
    let attrs = &input.attrs;
    Ok(quote! {
        #cfg
        #[doc = #params_doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct #params_name<#lifetime> {
            #(#names: #types,)*
        }

        #cfg
        impl<#lifetime> ::bt_hci::WriteHci for #params_name<#lifetime> {
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

        #cfg
        ::bt_hci::cmd::cmd! {
            #(#attrs)*
            #name(VENDOR_SPECIFIC, #ocf) {
                Params<#lifetime> = #params_name<#lifetime>;
                #returns_tokens
            }
        }

        #cfg
        impl<#lifetime> #name<#lifetime> {
            /// Build the command, rejecting variable-length fields longer than
            /// the capacity the catalog declares for them.
            #[allow(clippy::too_many_arguments)]
            pub fn try_new(#(#names: #types),*) -> Result<Self, ::stm32wb_hci::wire::TooLong> {
                #(#checks)*
                Ok(Self::new(#params_name { #(#names),* }))
            }
        }
    })
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
/// catalog's width.
fn width_assertions<'a>(
    cfg: &'a Option<TokenStream>,
    owner: &'a Ident,
    slots: &'a [Slot<'a>],
) -> impl Iterator<Item = TokenStream> + 'a {
    slots.iter().filter_map(move |slot| {
        let Slot::Fixed {
            field,
            member,
            width,
        } = slot
        else {
            return None;
        };
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
    })
}

/// The facts a declaration is generated from, identical in every release and
/// profile the command exists on.
#[derive(PartialEq)]
struct Facts<'a> {
    opcode: u16,
    completion: Completion,
    params: &'a [Field],
    structs: &'a Structs,
    /// Every return parameter, including the leading status.
    returns: &'a [Field],
    return_structs: &'a Structs,
}

struct Command<'a> {
    facts: Facts<'a>,
    cfg: Option<TokenStream>,
}

impl<'a> Command<'a> {
    fn resolve(bundled: &'a Bundled, input: &Input) -> syn::Result<Self> {
        let c_name = input.c_name.to_string();
        let error = |message: String| syn::Error::new(input.c_name.span(), message);
        let segments = bundled
            .command_segments(&c_name)
            .map_err(|failure| error(failure.to_string()))?;

        let mut facts: Option<Facts<'a>> = None;
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
            match &facts {
                None => facts = Some(current),
                Some(first) if *first == current => {}
                Some(_) => {
                    return Err(error(format!(
                        "{c_name} changes its opcode, completion, or layout in {releases}; \
                         release-specific declarations are not supported yet"
                    )));
                }
            }
        }

        Ok(Self {
            facts: facts.expect("command_segments never returns an empty history"),
            cfg: cfg::targets(
                &bundled.catalog,
                segments
                    .iter()
                    .map(|segment| (segment.releases, segment.profiles)),
            ),
        })
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
}

/// Match declared fields to catalog members in order and decide how each
/// member is encoded. Members counting a variable-length field are derived
/// from that field and not declared.
fn plan<'a>(
    c_name: &Ident,
    declared: &'a Fields,
    members: &'a [Field],
    structs: &Structs,
    what: &str,
    variable: bool,
) -> syn::Result<Vec<Slot<'a>>> {
    let counts = members
        .iter()
        .filter_map(|member| match &member.ty {
            FieldType::Counted { count, .. } => Some(count.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let visible = members
        .iter()
        .filter(|member| !counts.contains(&member.name.as_str()))
        .collect::<Vec<_>>();

    for field in &declared.fields {
        if let Some(count) = counts.iter().find(|count| field.matches(count)) {
            return Err(syn::Error::new(
                field.name.span(),
                format!(
                    "{count} counts a variable-length field of {c_name} and is written from \
                 that field's length; remove `{}`",
                    field.name
                ),
            ));
        }
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
        if !field.matches(&member.name) {
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
                let counted = members
                    .iter()
                    .find(|other| {
                        matches!(&other.ty, FieldType::Counted { count, .. } if *count == member.name)
                    })
                    .and_then(field_of)
                    .expect("a hidden member counts a declared field");
                let FieldType::Scalar(scalar) = member.ty else {
                    unreachable!("the catalog validates that counts are integers")
                };
                return Ok(Slot::Count { counted, scalar });
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
                } if variable => {
                    let count_width = members
                        .iter()
                        .find(|other| other.name == *count)
                        .map(|other| match other.ty {
                            FieldType::Scalar(scalar) => scalar.width(),
                            _ => unreachable!("the catalog validates that counts are integers"),
                        })
                        .expect("the catalog validates that counts exist");
                    if u64::from(*capacity) >= 1 << (8 * u32::from(count_width)) {
                        return Err(syn::Error::new(
                            field.name.span(),
                            format!("`{member}` has a capacity its {count} cannot express"),
                        ));
                    }
                    return Ok(Slot::Bytes {
                        field,
                        capacity: *capacity,
                    });
                }
                FieldType::Counted { .. } => return unsupported("variable-length"),
                FieldType::Union { .. } => return unsupported("a union"),
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
        assert!(
            tokens.contains("HalAnchorPeriod { anchor_period : u32 , max_free_slot : u32 , }"),
            "{tokens}"
        );
        assert!(!tokens.contains("Return ="), "{tokens}");
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
            error.contains("Length counts a variable-length field"),
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

        let error = expand_err(
            "aci_hal_read_config_data => HalReadConfigData { offset: u8 } -> HalConfigData {
                 data: &'a [u8],
             }",
        );
        assert!(
            error.contains("variable-length; such return parameters"),
            "{error}"
        );
    }
}
