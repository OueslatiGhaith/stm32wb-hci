//! `vendor_struct!`: a Rust type standing for one C structure of the catalog,
//! checked against every definition of that structure.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use stm32wb_catalog::layout::struct_width;
use stm32wb_catalog::{Field, Profile, ReleaseRange, Structs, bundled};
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Ident, Token};

use crate::cfg;
use crate::command::{Fields, Side, field_size, field_write, plan, width_assertions};

/// ```text
/// /// Documentation for the structure.
/// Peer_Entry_t => PeerEntry {
///     peer_address_type: u8,
///     peer_address: BdAddr,
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

pub fn expand(input: Input) -> syn::Result<TokenStream> {
    let bundled = bundled().map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("the bundled catalog is invalid: {error}"),
        )
    })?;
    let c_name = &input.c_name;
    let c_name_string = c_name.to_string();
    let name = &input.fields.name;
    if let Some(field) = input
        .fields
        .fields
        .iter()
        .find(|field| field.since.is_some() || field.before.is_some())
    {
        return Err(syn::Error::new(
            field.name.span(),
            "structures cannot be release-specific; declare one per layout",
        ));
    }

    // Every definition of the structure, with the targets of the commands and
    // events using it and the structures it may refer to.
    let mut definition: Option<(&[Field], &Structs)> = None;
    let mut targets: Vec<(ReleaseRange, &[Profile])> = Vec::new();
    let mut consider = |user: &str,
                        structs: &'static Structs,
                        releases: ReleaseRange,
                        profiles: &'static [Profile]|
     -> syn::Result<()> {
        let Some(fields) = structs.get(&c_name_string) else {
            return Ok(());
        };
        match definition {
            Some((first, _)) if first != fields.as_slice() => {
                return Err(syn::Error::new(
                    c_name.span(),
                    format!(
                        "{c_name} is defined differently in {user} ({releases}); structures \
                         whose layout changes are not supported yet"
                    ),
                ));
            }
            Some(_) => {}
            None => definition = Some((fields, structs)),
        }
        if !targets.contains(&(releases, profiles)) {
            targets.push((releases, profiles));
        }
        Ok(())
    };
    for command in &bundled.catalog.commands {
        let Ok(segments) = bundled.command_segments(command.name()) else {
            continue;
        };
        for segment in segments {
            let active = &segment.entry;
            for structs in std::iter::once(active.params.structs)
                .chain(active.returns.map(|returns| returns.structs))
            {
                consider(active.name, structs, segment.releases, segment.profiles)?;
            }
        }
    }
    for event in &bundled.catalog.events {
        let Ok(segments) = bundled.event_segments(event.name()) else {
            continue;
        };
        for segment in segments {
            let active = &segment.entry;
            consider(
                active.name,
                active.payload.structs,
                segment.releases,
                segment.profiles,
            )?;
        }
    }
    let Some((members, structs)) = definition else {
        return Err(syn::Error::new(
            c_name.span(),
            format!("no command or event of the catalog uses a structure named {c_name}"),
        ));
    };

    let names = members
        .iter()
        .map(|member| vec![member.name.as_str()])
        .collect::<Vec<_>>();
    let slots = plan(
        c_name,
        &input.fields,
        members,
        &names,
        structs,
        Side::Struct,
    )?;
    let width = usize::from(
        struct_width(&c_name_string, structs)
            .map_err(|error| syn::Error::new(c_name.span(), error.to_string()))?,
    );
    let cfg = cfg::targets(&bundled.catalog, targets).map(|predicate| quote!(#[cfg(#predicate)]));

    let fields = &input.fields.fields;
    let (field_names, types): (Vec<_>, Vec<_>) =
        fields.iter().map(|field| (&field.name, &field.ty)).unzip();
    let sizes = fields.iter().map(field_size);
    let sync_writes = fields.iter().map(|field| field_write(field, false));
    let async_writes = fields.iter().map(|field| field_write(field, true));
    let widths = width_assertions(&cfg, name, &slots);
    let attrs = &input.attrs;
    let doc = format!("`{c_name}` in the catalog, {width} bytes on the wire.");
    Ok(quote! {
        #cfg
        #(#attrs)*
        #[doc = ""]
        #[doc = #doc]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        #[allow(missing_docs)]
        pub struct #name {
            #(pub #field_names: #types,)*
        }

        #cfg
        impl ::bt_hci::WriteHci for #name {
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
        impl<'de> ::bt_hci::FromHciBytes<'de> for #name {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> Result<(Self, &'de [u8]), ::bt_hci::FromHciBytesError> {
                let rest = data;
                #(
                    let (#field_names, rest) =
                        <#types as ::bt_hci::FromHciBytes<'de>>::from_hci_bytes(rest)?;
                )*
                Ok((Self { #(#field_names),* }, rest))
            }
        }

        #cfg
        impl ::stm32wb_hci::wire::HciWireType for #name {
            const WIDTH: usize = #width;
        }

        #cfg
        impl ::stm32wb_hci::wire::CatalogStruct for #name {
            const C_NAME: &'static str = #c_name_string;
        }

        #(#widths)*
    })
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

    #[test]
    fn structures_follow_the_catalog() {
        let tokens =
            expand_str("Peer_Entry_t => PeerEntry { peer_address_type: u8, peer_address: BdAddr }")
                .unwrap();
        assert!(tokens.contains("const WIDTH : usize = 7usize"), "{tokens}");
        assert!(
            tokens.contains("const C_NAME : & 'static str = \"Peer_Entry_t\""),
            "{tokens}"
        );
        assert!(tokens.contains("feature = \"fw_1_17_0\""), "{tokens}");
        assert!(!tokens.contains("feature = \"fw_1_16_0\""), "{tokens}");

        let tokens = expand_str(
            "Whitelist_Entry_t => WhitelistEntry { peer_address_type: u8, peer_address: BdAddr }",
        )
        .unwrap();
        assert!(tokens.contains("feature = \"fw_1_16_0\""), "{tokens}");
        assert!(!tokens.contains("feature = \"fw_1_17_0\""), "{tokens}");

        let tokens = expand_str("Handle_Item_t => HandleItem { handle: u16 }").unwrap();
        assert!(tokens.contains("const WIDTH : usize = 2usize"), "{tokens}");

        let error = expand_str("Peer_Entry_t => PeerEntry { peer_address_type: u8 }").unwrap_err();
        assert!(
            error.contains("missing fields of Peer_Entry_t: peer_address"),
            "{error}"
        );

        let error = expand_str("Missing_t => Missing { value: u8 }").unwrap_err();
        assert!(
            error.contains("no command or event of the catalog uses"),
            "{error}"
        );

        let error = expand_str(
            "Peer_Entry_t => PeerEntry {
                 #[wire(since = \"1.17.0\")]
                 peer_address_type: u8,
                 peer_address: BdAddr,
             }",
        )
        .unwrap_err();
        assert!(error.contains("cannot be release-specific"), "{error}");
    }
}
