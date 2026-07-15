//! Expansion backend for the declarative vendor-event catalog.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use stm32wb_hci_schema::{EventPayload, Field, FieldEncoding, VariableEncodingShape, VendorEvents};
use syn::visit::{self, Visit};

/// Generate the complete event enum, borrowing payload types, and wire dispatcher.
pub(crate) fn expand_vendor_events(catalog: &VendorEvents) -> TokenStream2 {
    let borrows_payload = catalog
        .events
        .iter()
        .any(|event| payload_borrows(&event.payload));
    let event_generics = borrows_payload.then(|| quote!(<'a>));
    let impl_generics = borrows_payload.then(|| quote!(<'a>));
    let buffer_type = if borrows_payload {
        quote!(&'a [u8])
    } else {
        quote!(&[u8])
    };
    let variants = catalog.events.iter().map(|event| {
        let attrs = &event.attrs;
        let name = &event.name;
        match &event.payload {
            EventPayload::Unit => quote! {
                #(#attrs)*
                #name,
            },
            EventPayload::Fields(_) if payload_borrows(&event.payload) => {
                quote! {
                    #(#attrs)*
                    #name(#name<'a>),
                }
            }
            EventPayload::Fields(_) => {
                quote! {
                    #(#attrs)*
                    #name(#name),
                }
            }
        }
    });
    let payload_types = catalog.events.iter().filter_map(|event| {
        let EventPayload::Fields(fields) = &event.payload else {
            return None;
        };
        let attrs = &event.attrs;
        let name = &event.name;
        let field_names = fields
            .fields()
            .iter()
            .map(|field| &field.name)
            .collect::<Vec<_>>();
        let field_types = fields
            .fields()
            .iter()
            .map(|field| &field.ty)
            .collect::<Vec<_>>();
        if payload_borrows(&event.payload) {
            Some(quote! {
                #(#attrs)*
                #[derive(Copy, Clone, Debug)]
                #[cfg_attr(feature = "defmt", derive(defmt::Format))]
                #[allow(missing_docs)]
                pub struct #name<'a> {
                    #(pub #field_names: #field_types,)*
                }
            })
        } else {
            Some(quote! {
                #(#attrs)*
                #[derive(Copy, Clone, Debug)]
                #[cfg_attr(feature = "defmt", derive(defmt::Format))]
                #[allow(missing_docs)]
                pub struct #name {
                    #(pub #field_names: #field_types,)*
                }
            })
        }
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

    quote! {
        /// Vendor-specific events for the STM32WB5x radio coprocessor.
        #[derive(Clone, Copy, Debug)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum VendorEvent #event_generics {
            /// If the host fails to read events from the controller quickly enough, the
            /// controller will generate this event. This event is never lost; it is inserted as
            /// soon as space is available in the Tx queue.
            EventsLost(EventFlags),

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

fn payload_borrows(payload: &EventPayload) -> bool {
    let EventPayload::Fields(fields) = payload else {
        return false;
    };
    fields.fields().iter().any(|field| type_borrows(&field.ty))
}

fn type_borrows(ty: &syn::Type) -> bool {
    struct EventLifetime(bool);

    impl<'ast> Visit<'ast> for EventLifetime {
        fn visit_lifetime(&mut self, lifetime: &'ast syn::Lifetime) {
            self.0 |= lifetime.ident == "a";
            visit::visit_lifetime(self, lifetime);
        }
    }

    let mut lifetime = EventLifetime(false);
    lifetime.visit_type(ty);
    lifetime.0
}

fn expand_event_payload_decoder(name: &syn::Ident, payload: &EventPayload) -> TokenStream2 {
    let EventPayload::Fields(fields) = payload else {
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
    let decoders = fields
        .fields()
        .iter()
        .map(|field| expand_event_field_decoder(field, &cursor, &original_len));

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
    cursor: &syn::Ident,
    original_len: &syn::Ident,
) -> TokenStream2 {
    let name = &field.name;
    let ty = &field.ty;
    let decoder = match &field.encoding {
        FieldEncoding::Fixed(encoding) => {
            let width = &encoding.width_literal;
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
                let count_width = &count.width.literal;
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
                let count_width = &count.width.literal;
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
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
                let record_len_width = &record_len.width.literal;
                let length_ty = &length.ty;
                let length_width = &length.width.literal;
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
                let tag_width = &tagged.tag.width.literal;
                let length_ty = &tagged.length.ty;
                let length_width = &tagged.length.width.literal;
                let max_len = &tagged.max_len.literal;
                let tag_value = format_ident!("__stm32wb_tag", span = Span::mixed_site());
                let records = format_ident!("__stm32wb_records", span = Span::mixed_site());
                let rest = format_ident!("__stm32wb_tagged_rest", span = Span::mixed_site());
                let value = format_ident!("__stm32wb_tagged_value", span = Span::mixed_site());
                let variants = tagged.variants.iter().map(|variant| {
                    let tag = &variant.tag.literal;
                    let item_ty = &variant.item.ty;
                    let item_width = &variant.item.width.literal;
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
