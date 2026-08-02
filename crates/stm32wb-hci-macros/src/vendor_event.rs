//! Expansion backend for the declarative vendor-event catalog.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use stm32wb_hci_schema::{
    EventPayload, Field, FieldEncoding, VariableEncodingShape, VendorEvents, WireSize,
};
use syn::fold::Fold;

fn canonical_width(ty: &syn::Type) -> TokenStream2 {
    if let syn::Type::Reference(reference) = ty {
        return canonical_width(&reference.elem);
    }
    quote!({ <#ty as crate::wire::HciWireType>::WIDTH })
}

fn expand_wire_size(size: &WireSize) -> TokenStream2 {
    let constant = size.constant_part();
    let terms = size.terms().iter().map(|term| {
        let width = canonical_width(term.ty());
        let multiplier = term.multiplier();
        quote!(#width * #multiplier)
    });
    quote!(#constant #(+ #terms)*)
}

/// Generate the complete event enum, borrowing payload types, and wire dispatcher.
pub(crate) fn expand_vendor_events(catalog: &VendorEvents) -> TokenStream2 {
    let borrows_payload = catalog.events.iter().any(|event| event.payload.borrows());
    let event_generics = borrows_payload.then(|| quote!(<'event>));
    let impl_generics = borrows_payload.then(|| quote!(<'event>));
    let buffer_type = if borrows_payload {
        quote!(&'event [u8])
    } else {
        quote!(&[u8])
    };
    let variants = catalog.events.iter().map(|event| {
        let attrs = &event.attrs;
        let name = &event.name;
        match event.payload.fields() {
            None => quote! {
                #(#attrs)*
                #name,
            },
            Some(_) if event.payload.borrows() => {
                quote! {
                    #(#attrs)*
                    #name(#name<'event>),
                }
            }
            Some(_) => {
                quote! {
                    #(#attrs)*
                    #name(#name),
                }
            }
        }
    });
    let payload_types = catalog.events.iter().filter_map(|event| {
        let fields = event.payload.fields()?;
        let attrs = &event.attrs;
        let name = &event.name;
        let lifetime = event.payload.lifetime.as_ref();
        let generics = lifetime.map(|lifetime| quote!(<#lifetime>));
        let field_declarations = fields.fields().iter().map(|field| {
            let attrs = &field.attrs;
            let name = &field.name;
            let ty = &field.ty;
            quote! {
                #(#attrs)*
                pub #name: #ty,
            }
        });
        Some(quote! {
            #(#attrs)*
            #[derive(Copy, Clone, Debug)]
            #[cfg_attr(feature = "defmt", derive(defmt::Format))]
            #[allow(missing_docs)]
            pub struct #name #generics {
                #(#field_declarations)*
            }
        })
    });
    let match_arms = catalog.events.iter().map(|event| {
        let cfg_attrs = event
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"));
        let name = &event.name;
        let code = &event.code_literal;
        let decoder = expand_event_payload_decoder(name, &event.payload);
        quote! {
            #(#cfg_attrs)*
            #code => #decoder,
        }
    });
    let payload_length_asserts = catalog.events.iter().map(|event| {
        let cfg_attrs = event
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"));
        let maximum = expand_wire_size(&event.payload.max_size());
        quote! {
            #(#cfg_attrs)*
            const _: () = ::core::assert!(#maximum <= 253);
        }
    });
    let schema_validations = catalog.events.iter().flat_map(|event| {
        let cfg_attrs = event
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg"))
            .cloned()
            .collect::<Vec<_>>();
        event
            .payload
            .fields()
            .into_iter()
            .flat_map(FieldsExt::variable_encodings)
            .filter_map(move |encoding| {
                let validation = match &encoding.shape {
                    VariableEncodingShape::CountedBytes { count, max_len, .. } => {
                        count_capacity_assert(&count.ty, &max_len.literal)
                    }
                    VariableEncodingShape::CountedItems {
                        count, max_items, ..
                    } => count_capacity_assert(&count.ty, &max_items.literal),
                    VariableEncodingShape::LengthPrefixedRecords {
                        length, max_len, ..
                    } => count_capacity_assert(&length.ty, &max_len.literal),
                    VariableEncodingShape::TaggedItems(tagged) => {
                        let length_capacity =
                            count_capacity_assert(&tagged.length.ty, &tagged.max_len.literal);
                        let maximum = &tagged.max_len.literal;
                        let variants = tagged.variants.iter().map(|variant| {
                            let item_width = canonical_width(&variant.item.ty);
                            let max_items = &variant.max_items.literal;
                            quote! {
                                ::core::assert!(#max_items == #maximum / #item_width);
                            }
                        });
                        quote! {
                            #length_capacity
                            const _: () = { #(#variants)* };
                        }
                    }
                    VariableEncodingShape::Tagged(_)
                    | VariableEncodingShape::TrailingBytes { .. }
                    | VariableEncodingShape::BitmapItems { .. } => return None,
                };
                Some(quote! {
                    #(#cfg_attrs)*
                    #validation
                })
            })
    });

    quote! {
        #(#schema_validations)*
        #(#payload_length_asserts)*

        /// Vendor-specific events for the STM32WB5x radio coprocessor.
        #[derive(Clone, Copy, Debug)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum VendorEvent #event_generics {
            #(#variants)*
        }

        #(#payload_types)*

        impl #impl_generics VendorEvent #event_generics {
            /// Decode a two-byte STM32 vendor event code and its complete payload.
            ///
            /// Variable-length fields borrow from `buffer`; decoding performs no
            /// allocation and does not copy maximum-capacity payload arrays.
            pub fn new(buffer: #buffer_type) -> Result<Self, Error> {
                if buffer.len() < 2 {
                    return Err(Error::BadLength(buffer.len(), 2));
                }
                let (event_code, payload) = buffer.split_at(2);
                let event_code = u16::from_le_bytes([event_code[0], event_code[1]]);

                match event_code {
                    #(#match_arms)*
                    _ => Err(Error::Vendor(VendorError::UnknownEvent(event_code))),
                }
            }
        }
    }
}

trait FieldsExt {
    fn variable_encodings(&self) -> impl Iterator<Item = &stm32wb_hci_schema::VariableEncoding>;
}

impl FieldsExt for stm32wb_hci_schema::Fields {
    fn variable_encodings(&self) -> impl Iterator<Item = &stm32wb_hci_schema::VariableEncoding> {
        self.fields()
            .iter()
            .filter_map(|field| match &field.encoding {
                FieldEncoding::Fixed(_) => None,
                FieldEncoding::Variable(encoding) => Some(encoding.as_ref()),
            })
    }
}

fn count_capacity_assert(ty: &syn::Type, maximum: &syn::LitInt) -> TokenStream2 {
    let width = canonical_width(ty);
    quote! {
        const _: () = ::core::assert!(
            #maximum <= <#ty as crate::wire::HciCount<#width>>::MAX
        );
    }
}

fn expand_event_payload_decoder(name: &syn::Ident, payload: &EventPayload) -> TokenStream2 {
    let Some(fields) = payload.fields() else {
        return quote! {{
            if !payload.is_empty() {
                return Err(Error::BadLength(payload.len(), 0));
            }
            Ok(VendorEvent::#name)
        }};
    };

    let original_len = format_ident!("__stm32wb_event_original_len", span = Span::mixed_site());
    let cursor = format_ident!("__stm32wb_event_data", span = Span::mixed_site());
    let field_names = fields
        .fields()
        .iter()
        .map(|field| &field.name)
        .collect::<Vec<_>>();
    let decoders = fields.fields().iter().map(|field| {
        expand_event_field_decoder(field, payload.lifetime.as_ref(), &cursor, &original_len)
    });

    quote! {{
        let #original_len = payload.len();
        let #cursor = payload;
        #(#decoders)*
        if !#cursor.is_empty() {
            return Err(Error::BadLength(
                #original_len,
                #original_len - #cursor.len(),
            ));
        }
        Ok(VendorEvent::#name(#name { #(#field_names,)* }))
    }}
}

fn expand_event_field_decoder(
    field: &Field,
    payload_lifetime: Option<&syn::Lifetime>,
    cursor: &syn::Ident,
    original_len: &syn::Ident,
) -> TokenStream2 {
    let name = &field.name;
    let ty = decoder_field_type(&field.ty, payload_lifetime);
    let decoder = match &field.encoding {
        FieldEncoding::Fixed(_) => {
            let width = canonical_width(&field.ty);
            quote! {
                decode_hci_event_field::<#ty, #width>(#cursor, #original_len)
            }
        }
        FieldEncoding::Variable(encoding) => match &encoding.shape {
            VariableEncodingShape::CountedBytes {
                count,
                min_len,
                max_len,
            } => {
                let count_ty = &count.ty;
                let count_width = canonical_width(count_ty);
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote! {
                    decode_hci_event_counted_bytes::<
                        #ty, #count_ty, #count_width, #min_len, #max_len
                    >(
                        #cursor,
                        #original_len,
                    )
                }
            }
            VariableEncodingShape::CountedItems {
                count,
                item,
                min_items,
                max_items,
            } => {
                let count_ty = &count.ty;
                let count_width = canonical_width(count_ty);
                let item_ty = &item.ty;
                let item_width = canonical_width(item_ty);
                let min_items = &min_items.literal;
                let max_items = &max_items.literal;
                quote! {
                    decode_hci_event_counted_items::<
                        #ty,
                        #item_ty,
                        #count_ty,
                        #count_width,
                        #item_width,
                        #min_items,
                        #max_items,
                    >(#cursor, #original_len)
                }
            }
            VariableEncodingShape::TrailingBytes { min_len, max_len } => {
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote! {
                    decode_hci_event_trailing_bytes::<#ty, #min_len, #max_len>(#cursor)
                }
            }
            VariableEncodingShape::LengthPrefixedRecords {
                record_len,
                length,
                min_record_len,
                max_len,
            } => {
                let record_len_ty = &record_len.ty;
                let record_len_width = canonical_width(record_len_ty);
                let length_ty = &length.ty;
                let length_width = canonical_width(length_ty);
                let min_record_len = &min_record_len.literal;
                let max_len = &max_len.literal;
                quote! {
                    decode_hci_event_length_prefixed_records::<
                        #ty,
                        #record_len_ty,
                        #length_ty,
                        #record_len_width,
                        #length_width,
                        #min_record_len,
                        #max_len,
                    >(#cursor, #original_len)
                }
            }
            VariableEncodingShape::TaggedItems(tagged) => {
                let tag_ty = &tagged.tag.ty;
                let tag_width = canonical_width(tag_ty);
                let length_ty = &tagged.length.ty;
                let length_width = canonical_width(length_ty);
                let max_len = &tagged.max_len.literal;
                let tag_value = format_ident!("__stm32wb_tag", span = Span::mixed_site());
                let records = format_ident!("__stm32wb_records", span = Span::mixed_site());
                let rest = format_ident!("__stm32wb_tagged_rest", span = Span::mixed_site());
                let value = format_ident!("__stm32wb_tagged_value", span = Span::mixed_site());
                let variants = tagged.variants.iter().map(|variant| {
                    let tag = &variant.tag.literal;
                    let item_ty = &variant.item.ty;
                    let item_width = canonical_width(item_ty);
                    let max_items = &variant.max_items.literal;
                    quote! {
                        #tag => decode_hci_event_tagged_items_variant::<
                            #ty, #tag_ty, #item_ty, #item_width, #max_items
                        >(#tag_value, #records),
                    }
                });
                quote! {{
                    let (#tag_value, #records, #rest) = decode_hci_event_prefixed_bytes::<
                        #ty, #tag_ty, #length_ty, #tag_width, #length_width, #max_len
                    >(#cursor, #original_len)?;
                    let #value = match #tag_value {
                        #(#variants)*
                        _ => Err(<#ty as HciEventTaggedItemsTarget<#tag_ty>>::unknown_tag(
                            #tag_value,
                        )),
                    }?;
                    Ok::<(#ty, &[u8]), Error>((#value, #rest))
                }}
            }
            VariableEncodingShape::Tagged(_) | VariableEncodingShape::BitmapItems { .. } => {
                unreachable!("the shared parser rejects event encodings without view decoders")
            }
        },
    };

    quote! {
        let (#name, #cursor): (#ty, &[u8]) = #decoder?;
    }
}

fn decoder_field_type(ty: &syn::Type, payload_lifetime: Option<&syn::Lifetime>) -> syn::Type {
    let Some(payload_lifetime) = payload_lifetime else {
        return ty.clone();
    };
    let mut replacement = DecoderLifetime {
        declared: payload_lifetime,
        generated: syn::Lifetime::new("'event", Span::mixed_site()),
    };
    replacement.fold_type(ty.clone())
}

struct DecoderLifetime<'a> {
    declared: &'a syn::Lifetime,
    generated: syn::Lifetime,
}

impl Fold for DecoderLifetime<'_> {
    fn fold_lifetime(&mut self, lifetime: syn::Lifetime) -> syn::Lifetime {
        if lifetime.ident == self.declared.ident {
            self.generated.clone()
        } else {
            lifetime
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_event_enum_contains_only_catalog_variants() {
        let catalog = syn::parse_str::<VendorEvents>(
            "First(0x0001) { Payload = (); } Second(0x0002) { Payload = { value: u8, }; }",
        )
        .unwrap();
        let output = syn::parse2::<syn::File>(expand_vendor_events(&catalog)).unwrap();
        let generated = output
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Enum(item) if item.ident == "VendorEvent" => Some(item),
                _ => None,
            })
            .expect("generated VendorEvent enum");
        let variants = generated
            .variants
            .iter()
            .map(|variant| variant.ident.to_string())
            .collect::<Vec<_>>();

        assert_eq!(variants, ["First", "Second"]);
    }
}
