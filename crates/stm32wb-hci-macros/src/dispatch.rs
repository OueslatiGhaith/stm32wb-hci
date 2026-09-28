//! `vendor_events!` and `system_events!`: an enum of every vendor or system
//! event the selected target's wireless binary emits, decoding an event into
//! the variant its code selects.

use std::collections::BTreeMap;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use stm32wb_catalog::bundled;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Ident, Lifetime, Token, Type, Visibility, braced, parenthesized};

use crate::cfg;
use crate::command::Channel;
use crate::complete::{Kind, entries};

/// ```text
/// /// Documentation for the enum.
/// pub enum AciEvent<'a> {
///     aci_warning_event => HalWarning(hal::HalWarningEvent<'a>),
///     ...
/// }
/// ```
pub struct Input {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    lifetime: Lifetime,
    variants: Vec<Variant>,
}

struct Variant {
    attrs: Vec<Attribute>,
    c_name: Ident,
    name: Ident,
    ty: Type,
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        input.parse::<Token![enum]>()?;
        let name = input.parse()?;
        input.parse::<Token![<]>()?;
        let lifetime = input.parse()?;
        input.parse::<Token![>]>()?;
        let body;
        braced!(body in input);
        let variants = Punctuated::<Variant, Token![,]>::parse_terminated(&body)?
            .into_iter()
            .collect();
        Ok(Self {
            attrs,
            vis,
            name,
            lifetime,
            variants,
        })
    }
}

impl Parse for Variant {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let c_name = input.parse()?;
        input.parse::<Token![=>]>()?;
        let name = input.parse()?;
        let content;
        parenthesized!(content in input);
        let ty = content.parse()?;
        Ok(Self {
            attrs,
            c_name,
            name,
            ty,
        })
    }
}

/// `ty` with every lifetime replaced by `'static`, for the compile-time
/// checks, which have no lifetime in scope.
pub(crate) fn with_static_lifetimes(ty: &Type) -> Type {
    fn visit(ty: &mut Type) {
        match ty {
            Type::Reference(reference) => {
                if let Some(lifetime) = &mut reference.lifetime {
                    *lifetime = Lifetime::new("'static", lifetime.span());
                }
                visit(&mut reference.elem);
            }
            Type::Slice(slice) => visit(&mut slice.elem),
            Type::Array(array) => visit(&mut array.elem),
            Type::Path(path) => {
                for segment in &mut path.path.segments {
                    if let syn::PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                        for argument in &mut arguments.args {
                            match argument {
                                syn::GenericArgument::Lifetime(lifetime) => {
                                    *lifetime = Lifetime::new("'static", lifetime.span());
                                }
                                syn::GenericArgument::Type(ty) => visit(ty),
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut ty = ty.clone();
    visit(&mut ty);
    ty
}

/// Whether `ty` mentions `lifetime`.
fn borrows(ty: &Type, lifetime: &Lifetime) -> bool {
    quote!(#ty)
        .into_iter()
        .zip(quote!(#ty).into_iter().skip(1))
        .any(|(apostrophe, ident)| {
            matches!(&apostrophe, proc_macro2::TokenTree::Punct(punct) if punct.as_char() == '\'')
                && lifetime.ident == ident.to_string()
        })
}

pub fn expand(input: Input, channel: Channel) -> syn::Result<TokenStream> {
    let bundled = bundled().map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("the bundled catalog is invalid: {error}"),
        )
    })?;
    let catalog = &bundled.catalog;

    // The variant covering each catalog entry, by the entry's latest name.
    let mut covered: BTreeMap<&str, &Variant> = BTreeMap::new();
    let mut variants = Vec::new();
    for variant in &input.variants {
        let c_name = variant.c_name.to_string();
        let error = |message: String| syn::Error::new(variant.c_name.span(), message);
        let scope = catalog
            .events
            .iter()
            .find(|event| event.names.iter().any(|named| named.name == c_name))
            .map(|event| event.scope)
            .ok_or_else(|| error(format!("the catalog has no event {c_name}")))?;
        if scope != channel.event_scope() {
            return Err(error(format!(
                "{c_name} is not an ST {} event",
                channel.event_kind()
            )));
        }
        let entries =
            entries(bundled, Kind::Event, &c_name).map_err(|failure| error(failure.to_string()))?;
        let mut targets = Vec::new();
        let mut latest = None;
        for (entry, entry_targets) in entries {
            if let Some(other) = covered.insert(entry, variant) {
                return Err(error(format!(
                    "{entry} is already the variant {}",
                    other.name
                )));
            }
            latest.get_or_insert(entry);
            targets.extend(entry_targets);
        }
        let Some(latest) = latest else {
            return Err(error(format!("{c_name} is excluded on every target")));
        };
        let cfg = cfg::targets(catalog, targets.iter().copied());
        variants.push((variant, latest, targets, cfg));
    }

    let mut missing = Vec::new();
    for event in catalog
        .events
        .iter()
        .filter(|event| event.scope == channel.event_scope())
    {
        let name = event.name();
        let required = entries(bundled, Kind::Event, name)
            .map_err(|failure| syn::Error::new(Span::call_site(), failure.to_string()))?;
        for entry in required.keys() {
            if !covered.contains_key(entry) && !missing.contains(entry) {
                missing.push(*entry);
            }
        }
    }
    if !missing.is_empty() {
        return Err(syn::Error::new(
            input.name.span(),
            format!(
                "{} has no variant for these {} events of the catalog: {}",
                input.name,
                channel.event_kind(),
                missing.join(", ")
            ),
        ));
    }

    let name = &input.name;
    let lifetime = &input.lifetime;
    let attrs = &input.attrs;
    let vis = &input.vis;
    let cfg_attr =
        |cfg: &Option<TokenStream>| cfg.as_ref().map(|predicate| quote!(#[cfg(#predicate)]));

    let declarations = variants.iter().map(|(variant, latest, _, cfg)| {
        let cfg = cfg_attr(cfg);
        let (attrs, variant_name, ty) = (&variant.attrs, &variant.name, &variant.ty);
        let doc = format!("`{latest}`.");
        quote! {
            #cfg
            #(#attrs)*
            #[doc = #doc]
            #variant_name(#ty),
        }
    });
    let (event_trait, declares, declaration) = match channel {
        Channel::Vendor => (quote!(VendorEvent), quote!(declares_event), "vendor_event!"),
        Channel::System => (
            quote!(SystemEvent),
            quote!(declares_system_event),
            "system_event!",
        ),
        Channel::Standard => unreachable!("Core events are bt-hci's"),
    };
    let decodes = variants.iter().map(|(variant, _, _, cfg)| {
        let cfg = cfg_attr(cfg);
        let (variant_name, ty) = (&variant.name, &variant.ty);
        quote! {
            #cfg
            if code == <#ty as ::stm32wb_hci::wire::#event_trait<#lifetime>>::CODE {
                return ::core::option::Option::Some(
                    <#ty as ::bt_hci::FromHciBytes<#lifetime>>::from_hci_bytes_complete(payload)
                        .map(Self::#variant_name),
                );
            }
        }
    });
    let checks = variants.iter().map(|(variant, latest, _, cfg)| {
        let cfg = cfg_attr(cfg);
        let ty = with_static_lifetimes(&variant.ty);
        let message = format!(
            "{name}::{}: the catalog event is {latest}; use the type {declaration} declares for it",
            variant.name
        );
        quote::quote_spanned! {variant.ty.span()=>
            #cfg
            const _: () = ::core::assert!(
                ::stm32wb_hci::wire::#declares::<#ty>(#latest),
                #message
            );
        }
    });

    // Where no variant borrows the event, an uninhabited variant uses the
    // lifetime; matches need no arm for it.
    let borrowing = variants
        .iter()
        .filter(|(variant, ..)| borrows(&variant.ty, lifetime))
        .flat_map(|(_, _, targets, _)| targets.iter().copied())
        .collect::<Vec<_>>();
    let unused_lifetime = match cfg::targets(catalog, borrowing.iter().copied()) {
        None if !borrowing.is_empty() => None,
        predicate => {
            let cfg = predicate.map(|predicate| quote!(#[cfg(not(#predicate))]));
            Some(quote! {
                #cfg
                #[doc(hidden)]
                __Unborrowed(::core::convert::Infallible, ::core::marker::PhantomData<&#lifetime ()>),
            })
        }
    };

    let decode_params = quote! {
        let (code, payload) = params.split_first_chunk::<2>()?;
        let code = u16::from_le_bytes(*code);
        #(#decodes)*
        let _ = payload;
        ::core::option::Option::None
    };
    let decoders = match channel {
        Channel::Vendor => quote! {
            /// Decode the parameters of a vendor event, code included, into
            /// the variant its code selects, or `None` if the selected
            /// target emits no vendor event with that code.
            pub fn from_vendor_params(
                params: &#lifetime [u8],
            ) -> ::core::option::Option<::core::result::Result<Self, ::bt_hci::FromHciBytesError>> {
                #decode_params
            }

            /// Decode a bt-hci vendor event into the variant its code selects.
            pub fn from_vendor(
                event: &#lifetime ::bt_hci::event::Vendor<'_>,
            ) -> ::core::option::Option<::core::result::Result<Self, ::bt_hci::FromHciBytesError>> {
                Self::from_vendor_params(&event.params)
            }
        },
        Channel::System => quote! {
            /// Decode the parameters of a system event, sub-event code
            /// included, into the variant its code selects, or `None` if the
            /// selected target emits no system event with that code.
            pub fn from_system_params(
                params: &#lifetime [u8],
            ) -> ::core::option::Option<::core::result::Result<Self, ::bt_hci::FromHciBytesError>> {
                #decode_params
            }
        },
        Channel::Standard => unreachable!("Core events are bt-hci's"),
    };

    Ok(quote! {
        #(#attrs)*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        #vis enum #name<#lifetime> {
            #(#declarations)*
            #unused_lifetime
        }

        impl<#lifetime> #name<#lifetime> {
            #decoders
        }

        #(#checks)*
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_str(source: &str) -> Result<String, String> {
        expand_on(source, Channel::Vendor)
    }

    fn expand_on(source: &str, channel: Channel) -> Result<String, String> {
        let input = syn::parse_str::<Input>(source).map_err(|error| error.to_string())?;
        expand(input, channel)
            .map(|tokens| tokens.to_string())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn enums_list_every_vendor_event() {
        let error = expand_str(
            "pub enum AciEvent<'a> {
                 aci_warning_event => HalWarning(hal::HalWarningEvent<'a>),
             }",
        )
        .unwrap_err();
        assert!(
            error.contains("AciEvent has no variant for these vendor events of the catalog"),
            "{error}"
        );
        assert!(error.contains("aci_gatt_eatt_bearer_event"), "{error}");
        assert!(!error.contains("aci_warning_event"), "{error}");
    }

    #[test]
    fn variants_name_vendor_events_once() {
        let error = expand_str(
            "pub enum AciEvent<'a> {
                 aci_hal_fw_error_event => Old(hal::HalWarningEvent<'a>),
                 aci_warning_event => HalWarning(hal::HalWarningEvent<'a>),
             }",
        )
        .unwrap_err();
        assert!(
            error.contains("aci_warning_event is already the variant Old"),
            "{error}"
        );

        let error = expand_str(
            "pub enum AciEvent<'a> {
                 hci_disconnection_complete_event => Disconnection(Disconnection),
             }",
        )
        .unwrap_err();
        assert!(error.contains("is not an ST vendor event"), "{error}");
    }

    #[test]
    fn system_enums_list_every_system_event() {
        let error = expand_on(
            "pub enum ShciEvent<'a> {
                 SHCI_SUB_EVT_CODE_READY => Ready(ReadyEvent),
             }",
            Channel::System,
        )
        .unwrap_err();
        assert!(
            error.contains("ShciEvent has no variant for these system events of the catalog"),
            "{error}"
        );
        assert!(error.contains("SHCI_SUB_EVT_NVM_END_ERASE"), "{error}");
        assert!(!error.contains("SHCI_SUB_EVT_THREAD"), "{error}");

        let error = expand_on(
            "pub enum ShciEvent<'a> {
                 aci_warning_event => HalWarning(hal::HalWarningEvent<'a>),
             }",
            Channel::System,
        )
        .unwrap_err();
        assert!(error.contains("is not an ST system event"), "{error}");
    }

    #[test]
    fn types_are_checked_and_lifetimes_made_static() {
        let ty: Type = syn::parse_str("hal::HalWarningEvent<'a>").unwrap();
        let ty = with_static_lifetimes(&ty);
        assert_eq!(
            quote!(#ty).to_string(),
            "hal :: HalWarningEvent < 'static >"
        );
        let lifetime = Lifetime::new("'a", Span::call_site());
        assert!(borrows(&syn::parse_str("Event<'a>").unwrap(), &lifetime));
        assert!(!borrows(&syn::parse_str("Event").unwrap(), &lifetime));
    }
}
