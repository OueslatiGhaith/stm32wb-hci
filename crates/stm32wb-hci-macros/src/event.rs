//! `vendor_event!` and `system_event!`: an ST vendor or system event whose
//! code, availability, and parameter layout come from the catalog.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use stm32wb_catalog::{Bundled, Field, ReleaseRange, Structs, bundled};
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Ident, Lifetime, Token};

use crate::command::{
    BearerGroup, Channel, ElementType, Fields, InputField, Side, Slot, Targets, add_bearers,
    bearer_assertions, bearer_groups, bearer_positions, check_bounds,
    elements_lifetime_and_element, fields_in, plan, record_history_names, record_names,
    same_layout, slice_lifetime_and_element, width_assertions,
};
use crate::{cfg, complete};

/// ```text
/// /// Documentation for the event.
/// aci_hal_end_of_radio_activity_event => HalEndOfRadioActivityEvent {
///     last_state: u8,
///     ...
/// }
/// ```
pub struct Input {
    attrs: Vec<Attribute>,
    c_name: Ident,
    fields: Fields,
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let c_name = input.parse()?;
        input.parse::<Token![=>]>()?;
        let fields = input.parse()?;
        Ok(Self {
            attrs,
            c_name,
            fields,
        })
    }
}

pub fn expand(input: Input, channel: Channel) -> syn::Result<TokenStream> {
    let bundled = bundled().map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("the bundled catalog is invalid: {error}"),
        )
    })?;
    let variants = Event::resolve(bundled, &input, channel)?;
    let several = variants.len() > 1;
    let mut tokens = TokenStream::new();
    for variant in &variants {
        let expanded = expand_variant(&input, variant, channel).map_err(|error| {
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
    let c_name = input.c_name.to_string();
    tokens.extend(
        complete::declared(bundled, complete::Kind::Event, &c_name)
            .map_err(|error| syn::Error::new(input.c_name.span(), error.to_string()))?,
    );
    Ok(tokens)
}

/// The facts a declaration is generated from, identical on the wire in every
/// release and profile it covers.
struct Facts<'a> {
    code: u16,
    payload: &'a [Field],
    structs: &'a Structs,
}

impl Facts<'_> {
    fn same_wire(&self, other: &Facts<'_>) -> bool {
        self.code == other.code
            && self.structs == other.structs
            && same_layout(self.payload, other.payload)
    }
}

/// The declaration for the releases in which the same fields exist.
struct Event<'a> {
    /// The declared fields without those its releases lack.
    fields: Fields,
    facts: Facts<'a>,
    /// Every name each parameter had, by position, latest first.
    names: Vec<Vec<&'a str>>,
    releases: ReleaseRange,
    cfg: Option<TokenStream>,
    /// The latest C name of the catalog entry, which the event's type names.
    latest: &'a str,
    /// The parameters addressing ATT bearers.
    bearers: Vec<BearerGroup>,
}

impl<'a> Event<'a> {
    /// One declaration per set of fields that exist together, each for the
    /// releases and profiles having exactly those fields.
    fn resolve(bundled: &'a Bundled, input: &Input, channel: Channel) -> syn::Result<Vec<Self>> {
        let c_name = input.c_name.to_string();
        let error = |message: String| syn::Error::new(input.c_name.span(), message);
        let segments = bundled
            .event_segments(&c_name)
            .map_err(|failure| error(failure.to_string()))?;
        let ranges = segments
            .iter()
            .map(|segment| segment.releases)
            .collect::<Vec<_>>();
        check_bounds(bundled, &c_name, input.fields.fields.iter(), &ranges)?;

        // Each variant with its code and the fields it declares, which it is
        // keyed by, and the targets it covers.
        #[allow(clippy::type_complexity)]
        let mut variants: Vec<(
            (u16, Vec<bool>),
            Self,
            Targets<'a>,
            Vec<(Vec<(usize, u16)>, Targets<'a>)>,
        )> = Vec::new();
        // Every release's layout, for the names renamed members had.
        let mut history: Vec<&'a [Field]> = Vec::new();
        for segment in &segments {
            let active = &segment.entry;
            let releases = segment.releases;
            if active.event.scope != channel.event_scope() {
                return Err(error(format!(
                    "{c_name} is not an ST {} event",
                    channel.event_kind()
                )));
            }
            if let Some(exclusion) = active.excluded {
                return Err(error(format!(
                    "{c_name} is excluded in {releases}: {}",
                    exclusion.reason
                )));
            }
            let payload = active.payload.fields.map_err(|reason| {
                error(format!(
                    "{c_name} has no derivable parameter layout in {releases}: {reason}"
                ))
            })?;
            history.push(payload);
            let current = Facts {
                code: active.event.code,
                payload,
                structs: active.payload.structs,
            };
            let key = (
                current.code,
                input
                    .fields
                    .fields
                    .iter()
                    .map(|field| field.exists_in(releases))
                    .collect::<Vec<_>>(),
            );
            let bearers = bearer_positions(payload, &active.definition.bearers);
            let Some((_, variant, targets, groups)) =
                variants.iter_mut().find(|(other, ..)| *other == key)
            else {
                let mut variant = Self {
                    fields: fields_in(&input.fields, releases),
                    facts: current,
                    names: Vec::new(),
                    releases,
                    cfg: None,
                    latest: active.event.name(),
                    bearers: Vec::new(),
                };
                record_names(&mut variant.names, payload);
                let target = (releases, segment.profiles);
                variants.push((key, variant, vec![target], vec![(bearers, vec![target])]));
                continue;
            };
            if !variant.facts.same_wire(&current) {
                return Err(error(format!(
                    "{c_name} changes its layout in {releases}; if it adds or removes members \
                     there, declare them with #[wire(since = \"{0}\")] or \
                     #[wire(before = \"{0}\")]",
                    releases.first
                )));
            }
            record_names(&mut variant.names, current.payload);
            variant.facts = current;
            variant.releases.first = variant.releases.first.min(releases.first);
            variant.releases.last = variant.releases.last.max(releases.last);
            targets.push((releases, segment.profiles));
            add_bearers(groups, bearers, (releases, segment.profiles));
        }

        Ok(variants
            .into_iter()
            .map(|(_, mut variant, targets, groups)| {
                for payload in &history {
                    record_history_names(&mut variant.names, variant.facts.payload, payload);
                }
                variant.cfg = cfg::targets(&bundled.catalog, targets);
                variant.bearers = bearer_groups(&bundled.catalog, groups);
                variant
            })
            .collect())
    }
}

/// The lifetime the fields borrow the event with, if any borrows it.
fn borrowed_lifetime(fields: &[InputField]) -> syn::Result<Option<&Lifetime>> {
    let mut lifetime: Option<&Lifetime> = None;
    for field in fields {
        let Some((current, _)) = slice_lifetime_and_element(&field.ty)
            .or_else(|| elements_lifetime_and_element(&field.ty))
        else {
            continue;
        };
        match lifetime {
            Some(first) if first != current => {
                return Err(syn::Error::new(
                    current.span(),
                    format!("borrow every field for the same lifetime, `{first}`"),
                ));
            }
            _ => lifetime = Some(current),
        }
    }
    Ok(lifetime)
}

/// A plain struct with the event's parameters, decoded in catalog order from
/// the bytes after the event code. Variable-length parameters borrow the
/// event, and their counts are left out.
fn expand_variant(input: &Input, event: &Event<'_>, channel: Channel) -> syn::Result<TokenStream> {
    let c_name = &input.c_name;
    let name = &event.fields.name;
    let slots = plan(
        c_name,
        &event.fields,
        event.facts.payload,
        &event.names,
        event.facts.structs,
        Side::Event,
    )?;
    let cfg = event
        .cfg
        .as_ref()
        .map(|predicate| quote!(#[cfg(#predicate)]));
    let lifetime = borrowed_lifetime(&event.fields.fields)?;
    let (generics, de) = match lifetime {
        Some(lifetime) => (quote!(<#lifetime>), quote!(#lifetime)),
        None => (quote!(), quote!('de)),
    };
    let impl_generics = quote!(<#de>);
    let count_of = |counted: &InputField| format_ident!("__{}_count", counted.name);
    let decodes = slots.iter().map(|slot| match slot {
        Slot::Fixed { field, .. } => {
            let (field, ty) = (&field.name, &field.ty);
            quote!(let (#field, rest) = <#ty as ::bt_hci::FromHciBytes<#de>>::from_hci_bytes(rest)?;)
        }
        Slot::Count { counted, scalar } => {
            let (count, scalar) = (count_of(counted), format_ident!("{}", scalar.name()));
            quote!(let (#count, rest) = <#scalar as ::bt_hci::FromHciBytes<#de>>::from_hci_bytes(rest)?;)
        }
        Slot::Elements {
            field,
            element: ElementType::Byte,
            ..
        } => {
            let count = count_of(field);
            let field = &field.name;
            quote! {
                let (#field, rest) = rest
                    .split_at_checked(usize::from(#count))
                    .ok_or(::bt_hci::FromHciBytesError::InvalidSize)?;
            }
        }
        Slot::Elements {
            field,
            element: ElementType::Struct { .. },
            ..
        } => {
            let count = count_of(field);
            let field = &field.name;
            quote! {
                let (#field, rest) =
                    ::stm32wb_hci::wire::Elements::decode(rest, usize::from(#count))?;
            }
        }
        Slot::Selector { .. }
        | Slot::Union { .. } => unreachable!("plan rejects these in events"),
    });
    let names = event.fields.fields.iter().map(|field| &field.name);
    let types = event.fields.fields.iter().map(|field| &field.ty);
    let field_names = names.clone();
    let widths = width_assertions(&cfg, name, &slots).chain(bearer_assertions(
        name,
        &slots,
        event.facts.payload,
        &event.bearers,
    ));
    let code = event.facts.code;
    let latest = event.latest;
    let doc = format!(
        "`{c_name}` in the catalog, {} event code {code:#06X}.",
        channel.event_kind()
    );
    let event_trait = match channel {
        Channel::Vendor => quote!(VendorEvent),
        Channel::System => quote!(SystemEvent),
        Channel::Standard => unreachable!("Core events are bt-hci's"),
    };
    let attrs = &input.attrs;
    Ok(quote! {
        #cfg
        #(#attrs)*
        #[doc = ""]
        #[doc = #doc]
        #[allow(missing_docs)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub struct #name #generics {
            #(pub #names: #types,)*
        }

        #cfg
        impl #impl_generics ::bt_hci::FromHciBytes<#de> for #name #generics {
            fn from_hci_bytes(
                data: &#de [u8],
            ) -> Result<(Self, &#de [u8]), ::bt_hci::FromHciBytesError> {
                let rest = data;
                #(#decodes)*
                Ok((Self { #(#field_names),* }, rest))
            }
        }

        #cfg
        impl #impl_generics ::stm32wb_hci::wire::#event_trait<#de> for #name #generics {
            const CODE: u16 = #code;
            const C_NAME: &'static str = #latest;
        }

        #(#widths)*
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
    fn events_follow_their_code_across_releases() {
        let tokens = expand_str(
            "aci_hal_end_of_radio_activity_event => HalEndOfRadioActivityEvent {
                 last_state: u8,
                 next_state: u8,
                 next_state_sys_time: u32,
                 last_state_slot: u8,
                 next_state_slot: u8,
             }",
        )
        .unwrap();
        assert!(tokens.contains("const CODE : u16 = 4u16"), "{tokens}");
        assert!(tokens.contains("const CODE : u16 = 6148u16"), "{tokens}");
    }

    #[test]
    fn counted_bytes_borrow_the_event() {
        let tokens = expand_str(
            "aci_warning_event => HalWarningEvent {
                 warning_type: u8,
                 data: &'a [u8],
             }",
        )
        .unwrap();
        assert!(
            tokens.contains("pub struct HalWarningEvent < 'a >"),
            "{tokens}"
        );
        assert!(tokens.contains("split_at_checked"), "{tokens}");

        let error = expand_str(
            "aci_warning_event => HalWarningEvent {
                 warning_type: u8,
                 data: BoundedBytes<251>,
             }",
        )
        .unwrap_err();
        assert!(
            error.contains("holds up to 251 bytes; declare it as `&'a [u8]`"),
            "{error}"
        );
    }

    #[test]
    fn events_are_checked_against_the_catalog() {
        let error = expand_str("hci_disconnection_complete_event => Disconnection {}").unwrap_err();
        assert!(error.contains("is not an ST vendor event"), "{error}");

        let error = expand_str("aci_missing_event => Missing {}").unwrap_err();
        assert!(error.contains("has no event"), "{error}");

        let error = expand_str("aci_gap_bond_lost_event => GapBondLost { connection_handle: u16 }")
            .unwrap_err();
        assert!(error.contains("since = \"1.22.0\""), "{error}");

        let tokens = expand_str(
            "aci_gap_bond_lost_event => GapBondLost {
                 #[wire(since = \"1.22.0\")]
                 connection_handle: u16,
             }",
        )
        .unwrap();
        assert!(tokens.contains("feature = \"fw_1_21_0\""), "{tokens}");
    }

    #[test]
    fn structure_lists_are_borrowed_elements() {
        let tokens = expand_str(
            "aci_gatt_read_multi_permit_req_event => GattReadMultiPermitReqEvent {
                 connection_handle: u16,
                 handle_item: Elements<'a, HandleItem>,
             }",
        )
        .unwrap();
        assert!(
            tokens.contains("pub struct GattReadMultiPermitReqEvent < 'a >"),
            "{tokens}"
        );
        assert!(tokens.contains("Elements :: decode"), "{tokens}");
        assert!(tokens.contains("stands_for :: < HandleItem >"), "{tokens}");

        let error = expand_str(
            "aci_gatt_read_multi_permit_req_event => GattReadMultiPermitReqEvent {
                 connection_handle: u16,
                 handle_item: &'a [HandleItem],
             }",
        )
        .unwrap_err();
        assert!(
            error.contains(
                "declare it as `Elements<'a, T>` with T the type standing for Handle_Item_t"
            ),
            "{error}"
        );
    }

    #[test]
    fn system_events_are_not_vendor_events() {
        let tokens = expand_on(
            "SHCI_SUB_EVT_NVM_START_WRITE => NvmStartWriteEvent { number_of_words: u32 }",
            Channel::System,
        )
        .unwrap();
        assert!(tokens.contains("wire :: SystemEvent"), "{tokens}");
        assert!(tokens.contains("const CODE : u16 = 37380u16"), "{tokens}");

        let error = expand_str("SHCI_SUB_EVT_NVM_END_WRITE => NvmEndWriteEvent {}").unwrap_err();
        assert!(error.contains("is not an ST vendor event"), "{error}");
        let error =
            expand_on("aci_warning_event => HalWarningEvent {}", Channel::System).unwrap_err();
        assert!(error.contains("is not an ST system event"), "{error}");
        let error = expand_on(
            "SHCI_SUB_EVT_THREAD_NVM_RAM_UPDATE => ThreadNvmRamUpdateEvent {}",
            Channel::System,
        )
        .unwrap_err();
        assert!(error.contains("is excluded"), "{error}");
    }
}
