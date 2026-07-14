//! Procedural entry points for the declarative STM32WB protocol catalog.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned};
use stm32wb_hci_schema::{
    Completion, Constraint, Constraints, EventPayload, Field, FieldEncoding, Fields, Returns,
    TaggedEncoding, VariableEncodingShape, VendorCommand, VendorEvents,
};

/// Declare one complete STM32WB vendor command.
///
/// The declaration is the source of truth for the command's vendor opcode,
/// request wire layout, completion mechanism, return wire layout, and
/// cross-field constraints. The same syntax is parsed by this proc macro and
/// by the compliance tool through `stm32wb-hci-schema`.
///
/// ```text
/// vendor_cmd! {
///     GapSetIoCapability(cgid = 0x1, cid = 0x05) {
///         Params = { io_capability: IoCapability => 1, };
///         Completion = CommandComplete;
///         Return = ();
///     }
/// }
/// ```
///
/// `cgid` is a three-bit command-group ID and `cid` is a seven-bit command ID.
/// The generated command derives its vendor OCF and HCI opcode from those two
/// values.
///
/// `Params` is either `()` or an inline field body. Fixed fields use
/// `field: Type => width`. Borrowing or variable fields use `Params<'a>` and
/// one of these typed schemas:
///
/// - `counted_bytes`: a count field followed by up to `max_len` bytes.
/// - `counted_items`: a count field followed by fixed-width items.
/// - `tagged`: a fixed-width discriminator and one fixed-width variant body.
/// - `trailing_bytes`: a bounded field that consumes the remaining bytes.
/// - `bitmap_items`: fixed-width items selected by bits in an earlier field.
///
/// `CommandComplete` requires `Return = ();` or an inline named return type.
/// `CommandStatus` has no `Return` declaration and implements `AsyncCmd`.
/// Fixed, infallible commands expose `new`; constrained or variable commands
/// expose `try_new` with `HciConstraintError`, `HciLengthError`, or their
/// combined `HciValidationError` as appropriate. Variable construction checks
/// both each field's declared bound and the aggregate 255-byte HCI parameter
/// limit.
///
/// `Constraints` are evaluated in declaration order and stop at the first
/// failure. Supported relationships are `ordered`, `ordered_when_in_range`,
/// `range`, `one_of`, `one_of_or_range`, `paired_value`, `implies_eq`,
/// `implies_range`, `len_at_most`, and `non_empty`. Intrinsic validity should
/// remain in the semantic field type; constraints describe relationships or
/// command-specific subsets.
#[proc_macro]
pub fn vendor_cmd(input: TokenStream) -> TokenStream {
    match syn::parse::<VendorCommand>(input) {
        Ok(command) => expand_vendor_command(&command).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Declare the complete STM32WB vendor-event catalog.
///
/// Each declaration owns its 16-bit vendor event code and complete payload
/// schema. Unit payloads generate unit `VendorEvent` variants; inline payloads
/// generate an owned public payload structure and a tuple variant carrying it.
/// Fixed fields use `field: Type => width`. Owned variable payload fields may
/// use `counted_bytes`, `counted_items`, `length_prefixed_records`,
/// `tagged_items`, or `trailing_bytes`.
///
/// The generated `VendorEvent::new` requires the two-byte event code, decodes
/// every declared field in order, and rejects both truncated and trailing
/// bytes. Event `cfg` attributes gate the enum variant and dispatch arm while
/// retaining the generated payload type, matching the catalog's established
/// cross-firmware API.
#[proc_macro]
pub fn vendor_event(input: TokenStream) -> TokenStream {
    match syn::parse::<VendorEvents>(input) {
        Ok(events) => expand_vendor_events(&events).into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Generate the complete event enum, owned payload types, and wire dispatcher.
fn expand_vendor_events(catalog: &VendorEvents) -> TokenStream2 {
    let variants = catalog.events.iter().map(|event| {
        let attrs = &event.attrs;
        let name = &event.name;
        match &event.payload {
            EventPayload::Unit => quote! {
                #(#attrs)*
                #name,
            },
            EventPayload::Fields(_) => quote! {
                #(#attrs)*
                #name(#name),
            },
        }
    });
    let payload_types = catalog.events.iter().filter_map(|event| {
        let EventPayload::Fields(fields) = &event.payload else {
            return None;
        };
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
        Some(quote! {
            #[derive(Copy, Clone, Debug)]
            #[cfg_attr(feature = "defmt", derive(defmt::Format))]
            #[allow(missing_docs)]
            pub struct #name {
                #(pub #field_names: #field_types,)*
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

    quote! {
        /// Vendor-specific events for the STM32WB5x radio coprocessor.
        #[allow(clippy::large_enum_variant)]
        #[derive(Clone, Copy, Debug)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        pub enum VendorEvent {
            /// If the host fails to read events from the controller quickly enough, the
            /// controller will generate this event. This event is never lost; it is inserted as
            /// soon as space is available in the Tx queue.
            EventsLost(EventFlags),

            #(#variants)*
        }

        #(#payload_types)*

        impl VendorEvent {
            /// Decode a two-byte STM32 vendor event code and its complete payload.
            pub fn new(buffer: &[u8]) -> Result<Self, Error> {
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
                max_items,
            } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
                let max_items = &max_items.literal;
                quote! {
                    decode_hci_event_counted_items::<
                        #ty,
                        #item_ty,
                        #count_ty,
                        #count_width,
                        #item_width,
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
                unreachable!("the shared parser rejects event encodings without owned decoders")
            }
        },
    };

    quote! {
        let (#name, #cursor): (#ty, &[u8]) = #decoder?;
    }
}

/// Generate the complete command type directly from the shared schema.
fn expand_vendor_command(command: &VendorCommand) -> TokenStream2 {
    let name = &command.name;
    let cgid = command.cgid;
    let cid = command.cid;
    let fields = command.params.fields().map_or(&[][..], Fields::fields);
    let field_names = fields.iter().map(|field| &field.name).collect::<Vec<_>>();
    let field_types = fields.iter().map(|field| &field.ty).collect::<Vec<_>>();
    let params_type = field_list_type(fields);
    let params_value = field_list_value(fields);
    let schema_validations = expand_schema_validations(fields);
    let params_length_assert = command.params.lifetime.is_none().then(|| {
        let params_len = command.params.max_len();
        quote! {
            const _: () = crate::vendor::command::assert_hci_field_list_length(#params_len);
        }
    });
    let constructor = expand_constructor(command, &field_names, &field_types, &params_value);
    let completion_impl = expand_completion(command);
    let lifetime = command.params.lifetime.as_ref();
    let impl_generics = lifetime.map(|lifetime| quote!(<#lifetime>));
    let type_generics = lifetime.map(|lifetime| quote!(<#lifetime>));
    let default_impl = command.params.fields().is_none().then(|| {
        quote! {
            impl Default for #name {
                fn default() -> Self {
                    Self::new()
                }
            }
        }
    });

    quote! {
        #schema_validations
        #params_length_assert

        #[allow(missing_docs)]
        pub struct #name #impl_generics(
            crate::vendor::command::DeclarativeParams<#params_type>
        );

        impl #impl_generics #name #type_generics {
            /// STM32 vendor command-group ID.
            pub const CGID: u16 = #cgid;
            /// Command ID within [`Self::CGID`].
            pub const CID: u16 = #cid;
            /// Vendor-specific Opcode Command Field.
            pub const OCF: u16 = crate::vendor::command::vendor_ocf(Self::CGID, Self::CID);

            #constructor
        }

        impl #impl_generics ::bt_hci::cmd::Cmd for #name #type_generics {
            const OPCODE: ::bt_hci::cmd::Opcode = ::bt_hci::cmd::Opcode::new(
                ::bt_hci::cmd::OpcodeGroup::VENDOR_SPECIFIC,
                Self::OCF,
            );
            type Params = crate::vendor::command::DeclarativeParams<#params_type>;

            fn params(&self) -> &Self::Params {
                &self.0
            }
        }

        impl #impl_generics ::bt_hci::WriteHci for #name #type_generics {
            #[inline]
            fn size(&self) -> usize {
                ::bt_hci::WriteHci::size(<Self as ::bt_hci::cmd::Cmd>::params(self)) + 3
            }

            fn write_hci<W: ::embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                ::embedded_io::Write::write_all(
                    &mut writer,
                    &<Self as ::bt_hci::cmd::Cmd>::header(self),
                )?;
                ::bt_hci::WriteHci::write_hci(
                    <Self as ::bt_hci::cmd::Cmd>::params(self),
                    writer,
                )
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                ::embedded_io_async::Write::write_all(
                    &mut writer,
                    &<Self as ::bt_hci::cmd::Cmd>::header(self),
                )
                .await?;
                ::bt_hci::WriteHci::write_hci_async(
                    <Self as ::bt_hci::cmd::Cmd>::params(self),
                    writer,
                )
                .await
            }
        }

        #completion_impl
        #default_impl
    }
}

fn expand_constructor(
    command: &VendorCommand,
    field_names: &[&syn::Ident],
    field_types: &[&syn::Type],
    params_value: &TokenStream2,
) -> TokenStream2 {
    let has_variable_params = command.params.lifetime.is_some();
    match (has_variable_params, &command.constraints) {
        (false, None) => quote! {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn new(#(#field_names: #field_types),*) -> Self {
                Self(crate::vendor::command::DeclarativeParams(#params_value))
            }
        },
        (false, Some(constraints)) => {
            let checks = expand_constraint_checks(&command.name, constraints);
            quote! {
                #[allow(clippy::too_many_arguments)]
                #[allow(missing_docs)]
                pub fn try_new(
                    #(#field_names: #field_types),*
                ) -> Result<Self, crate::vendor::command::HciConstraintError> {
                    #checks
                    Ok(Self(crate::vendor::command::DeclarativeParams(#params_value)))
                }
            }
        }
        (true, constraints) => {
            let checks = constraints
                .as_ref()
                .map(|constraints| expand_constraint_checks(&command.name, constraints));
            let error = if constraints.is_some() {
                quote!(crate::vendor::command::HciValidationError)
            } else {
                quote!(crate::vendor::command::HciLengthError)
            };
            quote! {
                #[allow(clippy::too_many_arguments)]
                #[allow(missing_docs)]
                pub fn try_new(
                    #(#field_names: #field_types),*
                ) -> Result<Self, #error> {
                    #checks
                    let params = crate::vendor::command::DeclarativeParams(#params_value);
                    let actual = crate::vendor::command::DeclarativeFieldList::size(&params.0);
                    if actual > u8::MAX as usize {
                        return Err(crate::vendor::command::HciLengthError::new(
                            actual,
                            0,
                            u8::MAX as usize,
                        ).into());
                    }
                    Ok(Self(params))
                }
            }
        }
    }
}

/// Translate the shared constraint AST into ordered, fail-fast runtime checks.
/// Each branch preserves the established diagnostic wording because
/// `HciConstraintError::constraint` is public API.
fn expand_constraint_checks(command: &syn::Ident, constraints: &Constraints) -> TokenStream2 {
    let checks = constraints
        .nodes()
        .iter()
        .map(|constraint| expand_constraint_check(command, constraint));
    quote! {
        (|| -> Result<(), crate::vendor::command::HciConstraintError> {
            #(#checks)*
            Ok(())
        })()?;
    }
}

fn expand_constraint_check(command: &syn::Ident, constraint: &Constraint) -> TokenStream2 {
    match constraint {
        Constraint::Ordered { minimum, maximum } => quote! {
            if #minimum > #maximum {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(stringify!(#minimum), " <= ", stringify!(#maximum)),
                ));
            }
        },
        Constraint::OrderedWhenInRange {
            minimum,
            maximum,
            range_minimum,
            range_maximum,
        } => quote! {
            if ((#range_minimum)..=(#range_maximum)).contains(&#minimum)
                && ((#range_minimum)..=(#range_maximum)).contains(&#maximum)
                && #minimum > #maximum
            {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#minimum),
                        " <= ",
                        stringify!(#maximum),
                        " when both are in ",
                        stringify!(#range_minimum),
                        "..=",
                        stringify!(#range_maximum),
                    ),
                ));
            }
        },
        Constraint::Range {
            field,
            minimum,
            maximum,
        } => quote! {
            if !((#minimum)..=(#maximum)).contains(&#field) {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#minimum),
                        " <= ",
                        stringify!(#field),
                        " <= ",
                        stringify!(#maximum),
                    ),
                ));
            }
        },
        Constraint::OneOf { field, allowed } => quote! {
            if ![#(#allowed),*].contains(&#field) {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(stringify!(#field), " in ", stringify!([#(#allowed),*])),
                ));
            }
        },
        Constraint::OneOfOrRange {
            field,
            allowed,
            minimum,
            maximum,
        } => quote! {
            if ![#(#allowed),*].contains(&#field)
                && !((#minimum)..=(#maximum)).contains(&#field)
            {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#field),
                        " in ",
                        stringify!([#(#allowed),*]),
                        " or ",
                        stringify!(#minimum),
                        " <= ",
                        stringify!(#field),
                        " <= ",
                        stringify!(#maximum),
                    ),
                ));
            }
        },
        Constraint::PairedValue { left, right, value } => {
            let binding = format_ident!("__stm32wb_constraint_value", span = Span::mixed_site());
            quote! {
                match #value {
                    ref #binding => {
                        if (&#left == #binding) != (&#right == #binding) {
                            return Err(crate::vendor::command::HciConstraintError::new(
                                stringify!(#command),
                                concat!(
                                    stringify!(#left),
                                    " and ",
                                    stringify!(#right),
                                    " are both ",
                                    stringify!(#value),
                                    " or neither is",
                                ),
                            ));
                        }
                    }
                }
            }
        }
        Constraint::ImpliesEq {
            selector,
            selected,
            field,
            required,
        } => quote! {
            if #selector == #selected && #field != #required {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#selector),
                        " == ",
                        stringify!(#selected),
                        " implies ",
                        stringify!(#field),
                        " == ",
                        stringify!(#required),
                    ),
                ));
            }
        },
        Constraint::ImpliesRange {
            selector,
            selected,
            field,
            minimum,
            maximum,
        } => quote! {
            if #selector == #selected && !((#minimum)..=(#maximum)).contains(&#field) {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#selector),
                        " == ",
                        stringify!(#selected),
                        " implies ",
                        stringify!(#minimum),
                        " <= ",
                        stringify!(#field),
                        " <= ",
                        stringify!(#maximum),
                    ),
                ));
            }
        },
        Constraint::LenAtMost { field, maximum } => quote! {
            if #field.len() > usize::from(#maximum) {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(stringify!(#field), ".len() <= ", stringify!(#maximum)),
                ));
            }
        },
        Constraint::NonEmpty { field } => quote! {
            if #field.is_empty() {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(stringify!(#field), " is not empty"),
                ));
            }
        },
    }
}

fn expand_completion(command: &VendorCommand) -> TokenStream2 {
    let name = &command.name;
    let lifetime = command.params.lifetime.as_ref();
    let impl_generics = lifetime.map(|lifetime| quote!(<#lifetime>));
    let type_generics = lifetime.map(|lifetime| quote!(<#lifetime>));
    match (command.completion, &command.returns) {
        (Completion::CommandComplete, Some(Returns::Unit)) => quote! {
            const _: () = crate::vendor::command::assert_hci_field_list_length(0usize);

            impl #impl_generics ::bt_hci::cmd::SyncCmd for #name #type_generics {
                type Return = ();
                type Handle = ();
                type ReturnBuf = [u8; 0];

                fn param_handle(&self) {}

                fn return_handle(
                    _data: &[u8],
                ) -> Result<Self::Handle, ::bt_hci::FromHciBytesError> {
                    Ok(())
                }
            }
        },
        (
            Completion::CommandComplete,
            Some(Returns::Fields {
                name: return_name,
                fields,
            }),
        ) => {
            let return_declaration = expand_return(return_name, fields);
            let return_len = fields.max_len();

            quote! {
                const _: () = crate::vendor::command::assert_hci_field_list_length(
                    #return_len
                );

                #return_declaration

                impl #impl_generics ::bt_hci::cmd::SyncCmd for #name #type_generics {
                    type Return = #return_name;
                    type Handle = ();
                    type ReturnBuf = [u8; #return_len];

                    fn param_handle(&self) {}

                    fn return_handle(
                        _data: &[u8],
                    ) -> Result<Self::Handle, ::bt_hci::FromHciBytesError> {
                        Ok(())
                    }
                }
            }
        }
        (Completion::CommandStatus, None) => quote! {
            impl #impl_generics ::bt_hci::cmd::AsyncCmd for #name #type_generics {}
        },
        _ => unreachable!("the shared parser validates completion and return combinations"),
    }
}

fn expand_return(return_name: &syn::Ident, fields: &Fields) -> TokenStream2 {
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
    let cursor = format_ident!("__stm32wb_return_data", span = Span::mixed_site());
    let decoders = fields
        .fields()
        .iter()
        .map(|field| expand_return_decoder(field, &cursor));

    quote! {
        #[derive(Copy, Clone, Debug)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        #[allow(missing_docs)]
        pub struct #return_name {
            #(pub #field_names: #field_types,)*
        }

        impl<'de> ::bt_hci::FromHciBytes<'de> for #return_name {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> Result<(Self, &'de [u8]), ::bt_hci::FromHciBytesError> {
                let #cursor = data;
                #(#decoders)*

                Ok((Self { #(#field_names,)* }, #cursor))
            }
        }
    }
}

fn expand_return_decoder(field: &Field, cursor: &syn::Ident) -> TokenStream2 {
    let name = &field.name;
    let ty = &field.ty;
    let decoder = match &field.encoding {
        FieldEncoding::Fixed(encoding) => {
            let width = &encoding.width_literal;
            quote! {
                crate::vendor::command::decode_declarative_fixed_field::<#ty, #width>(#cursor)
            }
        }
        FieldEncoding::Variable(encoding) => match &encoding.shape {
            VariableEncodingShape::CountedBytes { count, max_len, .. } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let max_len = &max_len.literal;
                quote! {
                    crate::vendor::command::decode_declarative_counted_bytes::<
                        #ty, #count_ty, #count_width, #max_len
                    >(#cursor)
                }
            }
            VariableEncodingShape::TrailingBytes { min_len, max_len } => {
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote! {
                    crate::vendor::command::decode_declarative_trailing_bytes::<
                        #ty, #min_len, #max_len
                    >(#cursor)
                }
            }
            VariableEncodingShape::CountedItems {
                count,
                item,
                max_items,
            } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
                let max_items = &max_items.literal;
                quote! {
                    crate::vendor::command::decode_declarative_counted_items::<
                        #ty, #item_ty, #count_ty, #count_width, #item_width, #max_items
                    >(#cursor)
                }
            }
            VariableEncodingShape::Tagged(_)
            | VariableEncodingShape::BitmapItems { .. }
            | VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => {
                unreachable!("the shared parser rejects variable returns without owned decoders")
            }
        },
    };
    quote! {
        let (#name, #cursor) = #decoder?;
    }
}

fn field_list_type(fields: &[Field]) -> TokenStream2 {
    fields.iter().rev().fold(quote!(()), |tail, field| {
        let head = encoded_field_type(field);
        quote!((#head, #tail))
    })
}

fn encoded_field_type(field: &Field) -> TokenStream2 {
    let ty = &field.ty;
    match &field.encoding {
        FieldEncoding::Fixed(encoding) => {
            let width = &encoding.width_literal;
            quote_spanned!(width.span()=> crate::vendor::command::DeclarativeField<#ty, #width>)
        }
        FieldEncoding::Variable(encoding) => match &encoding.shape {
            VariableEncodingShape::CountedBytes { count, max_len, .. } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let max_len = &max_len.literal;
                quote!(crate::vendor::command::CountedBytes<#ty, #count_ty, #count_width, #max_len>)
            }
            VariableEncodingShape::CountedItems {
                count,
                item,
                max_items,
            } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
                let max_items = &max_items.literal;
                quote!(crate::vendor::command::CountedItems<
                    #ty, #item_ty, #count_ty, #count_width, #item_width, #max_items
                >)
            }
            VariableEncodingShape::Tagged(tagged) => {
                let max_len = &tagged.max_len.literal;
                quote!(crate::vendor::command::TaggedField<#ty, #max_len>)
            }
            VariableEncodingShape::TrailingBytes { min_len, max_len } => {
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote!(crate::vendor::command::TrailingBytes<#ty, #min_len, #max_len>)
            }
            VariableEncodingShape::BitmapItems {
                item, max_items, ..
            } => {
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
                let max_items = &max_items.literal;
                quote!(crate::vendor::command::BitmapItems<#ty, #item_ty, #item_width, #max_items>)
            }
            VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => {
                unreachable!("the shared parser rejects event-only command fields")
            }
        },
    }
}

fn field_list_value(fields: &[Field]) -> TokenStream2 {
    fields.iter().rev().fold(quote!(()), |tail, field| {
        let head = encoded_field_value(field);
        quote!((#head, #tail))
    })
}

fn encoded_field_value(field: &Field) -> TokenStream2 {
    let name = &field.name;
    let ty = &field.ty;
    match &field.encoding {
        FieldEncoding::Fixed(encoding) => {
            let width = &encoding.width_literal;
            quote_spanned!(width.span()=> crate::vendor::command::DeclarativeField::<_, #width>(#name))
        }
        FieldEncoding::Variable(encoding) => match &encoding.shape {
            VariableEncodingShape::CountedBytes { count, max_len, .. } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let max_len = &max_len.literal;
                quote!(crate::vendor::command::CountedBytes::<
                    _, #count_ty, #count_width, #max_len
                >::try_new(#name)?)
            }
            VariableEncodingShape::CountedItems {
                count,
                item,
                max_items,
            } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
                let max_items = &max_items.literal;
                quote!(crate::vendor::command::CountedItems::<
                    _, #item_ty, #count_ty, #count_width, #item_width, #max_items
                >::try_new(#name)?)
            }
            VariableEncodingShape::Tagged(tagged) => tagged_field_value(name, ty, tagged),
            VariableEncodingShape::TrailingBytes { min_len, max_len } => {
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote!(crate::vendor::command::TrailingBytes::<
                    _, #min_len, #max_len
                >::try_new(#name)?)
            }
            VariableEncodingShape::BitmapItems {
                bitmap,
                mask,
                item,
                max_items,
            } => {
                let mask = &mask.literal;
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
                let max_items = &max_items.literal;
                quote!(crate::vendor::command::BitmapItems::<
                    _, #item_ty, #item_width, #max_items
                >::try_new(#name, #bitmap, #mask)?)
            }
            VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => {
                unreachable!("the shared parser rejects event-only command fields")
            }
        },
    }
}

fn tagged_field_value(name: &syn::Ident, ty: &syn::Type, tagged: &TaggedEncoding) -> TokenStream2 {
    let tag_ty = &tagged.tag.ty;
    let tag_width = &tagged.tag.width.literal;
    let min_len = &tagged.min_len.literal;
    let max_len = &tagged.max_len.literal;
    let arms = tagged.variants.iter().map(|variant| {
        let pattern = &variant.pattern;
        let tag = &variant.tag.literal;
        let payload = tagged_payload_value(variant.fields.fields());
        quote! {
            #pattern => {
                let tag: #tag_ty = #tag;
                crate::vendor::command::TaggedField::<#ty, #max_len>::try_new::<#min_len, _>((
                    crate::vendor::command::DeclarativeField::<#tag_ty, #tag_width>(tag),
                    #payload,
                ))?
            },
        }
    });
    quote! {
        match &#name {
            #(#arms)*
        }
    }
}

fn tagged_payload_value(fields: &[Field]) -> TokenStream2 {
    fields.iter().rev().fold(quote!(()), |tail, field| {
        let name = &field.name;
        let ty = &field.ty;
        let FieldEncoding::Fixed(encoding) = &field.encoding else {
            unreachable!("tagged payload fields are validated as fixed-width")
        };
        let width = &encoding.width_literal;
        quote!((
            crate::vendor::command::DeclarativeField::<&#ty, #width>(#name),
            #tail
        ))
    })
}

fn expand_schema_validations(fields: &[Field]) -> TokenStream2 {
    let validations = fields.iter().filter_map(|field| {
        let FieldEncoding::Variable(encoding) = &field.encoding else {
            return None;
        };
        match &encoding.shape {
            VariableEncodingShape::CountedBytes { count, max_len, .. } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let max_len = &max_len.literal;
                Some(quote! {
                    const _: () = ::core::assert!(
                        #max_len <= <#count_ty as crate::vendor::command::HciCount<#count_width>>::MAX
                    );
                })
            }
            VariableEncodingShape::CountedItems {
                count, max_items, ..
            } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let max_items = &max_items.literal;
                Some(quote! {
                    const _: () = ::core::assert!(
                        #max_items <= <#count_ty as crate::vendor::command::HciCount<#count_width>>::MAX
                    );
                })
            }
            VariableEncodingShape::Tagged(_)
            | VariableEncodingShape::TrailingBytes { .. }
            | VariableEncodingShape::BitmapItems { .. }
            | VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => None,
        }
    });
    quote!(#(#validations)*)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: TokenStream2) -> VendorCommand {
        syn::parse2(source).unwrap()
    }

    fn parse_events(source: TokenStream2) -> VendorEvents {
        syn::parse2(source).unwrap()
    }

    #[test]
    fn directly_generates_fixed_command_complete_unit_return() {
        let command = parse(quote! {
            GapSetIoCapability(cgid = 0x1, cid = 0x05) {
                Params = { io_capability: IoCapability => 1, };
                Completion = CommandComplete;
                Return = ();
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("pub struct GapSetIoCapability"));
        assert!(generated.contains("SyncCmd for GapSetIoCapability"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_fixed_command_status() {
        let command = parse(quote! {
            GapPeripheralSecurityRequest(cgid = 0x1, cid = 0x0D) {
                Params = { conn_handle: ConnHandle => 2, };
                Completion = CommandStatus;
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("AsyncCmd for GapPeripheralSecurityRequest"));
        assert!(!generated.contains("SyncCmd for GapPeripheralSecurityRequest"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_fixed_named_return() {
        let command = parse(quote! {
            CmdGapInit(cgid = 0x1, cid = 0x0A) {
                Params = {
                    role: Role => 1,
                    privacy_enabled: bool => 1,
                    dev_name_characteristic_len: u8 => 1,
                };
                Completion = CommandComplete;
                Return = GapInit {
                    service_handle: AttributeHandle => 2,
                    dev_name_handle: AttributeHandle => 2,
                    appearance_handle: AttributeHandle => 2,
                };
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("pub struct GapInit"));
        assert!(generated.contains("ReturnBuf = [u8 ; 6usize]"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_unit_params_and_default() {
        let command = parse(quote! {
            HalGetFirmwareRevision(cgid = 0x0, cid = 0x00) {
                Params = ();
                Completion = CommandComplete;
                Return = HalFirmwareRevision { revision: u16 => 2, };
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("Default for HalGetFirmwareRevision"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_fixed_constraints_and_try_new() {
        let command = parse(quote! {
            GapAdditionalBeaconStart(cgid = 0x1, cid = 0x30) {
                Params = {
                    advertising_interval_min: u16 => 2,
                    advertising_interval_max: u16 => 2,
                    advertising_channel_map: AdvertisingChannelMap => 1,
                };
                Constraints = {
                    range(advertising_interval_min, 0x0020, 0x4000);
                    ordered(advertising_interval_min, advertising_interval_max);
                    non_empty(advertising_channel_map);
                };
                Completion = CommandComplete;
                Return = ();
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("pub fn try_new"));
        assert!(generated.contains("HciConstraintError :: new"));
        assert!(generated.contains("advertising_channel_map . is_empty"));
        assert!(!generated.contains("pub fn new"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_counted_tagged_and_trailing_params() {
        let command = parse(quote! {
            VariableParams(cgid = 0x2, cid = 0x01) {
                Params<'a> = {
                    bytes: &'a [u8] => {
                        kind: counted_bytes,
                        count: u8 => 1,
                        max_len: 16,
                    },
                    uuid: &'a Uuid => {
                        kind: tagged,
                        tag: u8 => 1,
                        variants: {
                            Uuid::Uuid16(value) => {
                                tag: 0x01,
                                fields: { value: u16 => 2, },
                            },
                            Uuid::Uuid128(value) => {
                                tag: 0x02,
                                fields: { value: [u8; 16] => 16, },
                            },
                        },
                        min_len: 3,
                        max_len: 17,
                    },
                    tail: &'a [u8] => {
                        kind: trailing_bytes,
                        min_len: 0,
                        max_len: 8,
                    },
                };
                Completion = CommandStatus;
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("CountedBytes"));
        assert!(generated.contains("TaggedField"));
        assert!(generated.contains("TrailingBytes"));
        assert!(generated.contains("pub fn try_new"));
        assert!(generated.contains("HciLengthError"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_counted_and_bitmap_items() {
        let command = parse(quote! {
            VariableItems(cgid = 0x1, cid = 0x51) {
                Params<'a> = {
                    list: &'a [Peer] => {
                        kind: counted_items,
                        count: u8 => 1,
                        item: Peer => 7,
                        max_items: 3,
                    },
                    phys: Phys => 1,
                    selected: &'a [PhyParams] => {
                        kind: bitmap_items,
                        bitmap: phys,
                        mask: 0x05,
                        item: PhyParams => 5,
                        max_items: 2,
                    },
                };
                Completion = CommandStatus;
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("CountedItems"));
        assert!(generated.contains("BitmapItems"));
        assert!(generated.contains("try_new (selected , phys , 0x05)"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_all_owned_variable_return_decoders() {
        let command = parse(quote! {
            HalReadConfigData(cgid = 0x0, cid = 0x0D) {
                Params = { param: ConfigParameter => 1, };
                Completion = CommandComplete;
                Return = HalReadConfigDataReturn {
                    bytes: BoundedBytes<16> => {
                        kind: counted_bytes,
                        count: u8 => 1,
                        max_len: 16,
                    },
                    items: BoundedItems<Item, 4> => {
                        kind: counted_items,
                        count: u8 => 1,
                        item: Item => 2,
                        max_items: 4,
                    },
                    tail: BoundedBytes<8> => {
                        kind: trailing_bytes,
                        min_len: 0,
                        max_len: 8,
                    },
                };
            }
        });
        let generated = expand_vendor_command(&command).to_string();
        assert!(generated.contains("decode_declarative_counted_bytes"));
        assert!(generated.contains("decode_declarative_counted_items"));
        assert!(generated.contains("decode_declarative_trailing_bytes"));
        assert!(generated.contains("ReturnBuf = [u8 ; 34usize]"));
        assert!(!generated.contains("vendor_cmd !"));
    }

    #[test]
    fn directly_generates_event_enum_payloads_dispatch_and_cfg() {
        let events = parse_events(quote! {
            /// No payload.
            Unit(0x0001) { Payload = (); }
            #[cfg(since_fw_0_17_0)]
            Fixed(0x0002) {
                Payload = { value: u16 => 2, };
            }
        });
        let generated = expand_vendor_events(&events).to_string();
        assert!(generated.contains("pub enum VendorEvent"));
        assert!(generated.contains("EventsLost (EventFlags)"));
        assert!(generated.contains("pub struct Fixed"));
        assert!(generated.contains("0x0001 =>"));
        assert!(generated.contains("0x0002 =>"));
        assert!(generated.contains("decode_hci_event_field"));
        assert_eq!(generated.matches("cfg (since_fw_0_17_0)").count(), 2);
        assert!(!generated.contains("vendor_event !"));
    }

    #[test]
    fn directly_generates_every_owned_variable_event_decoder() {
        let events = parse_events(quote! {
            Counted(0x0001) {
                Payload = {
                    data: BoundedBytes<8> => {
                        kind: counted_bytes,
                        count: u8 => 1,
                        max_len: 8,
                    },
                };
            }
            Items(0x0002) {
                Payload = {
                    values: BoundedItems<Item, 3> => {
                        kind: counted_items,
                        count: u8 => 1,
                        item: Item => 2,
                        max_items: 3,
                    },
                };
            }
            Records(0x0003) {
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
            Tagged(0x0004) {
                Payload = {
                    value: Tagged => {
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
            Trailing(0x0005) {
                Payload = {
                    value: BoundedBytes<4> => {
                        kind: trailing_bytes,
                        min_len: 0,
                        max_len: 4,
                    },
                };
            }
        });
        let generated = expand_vendor_events(&events).to_string();
        assert!(generated.contains("decode_hci_event_counted_bytes"));
        assert!(generated.contains("decode_hci_event_counted_items"));
        assert!(generated.contains("decode_hci_event_length_prefixed_records"));
        assert!(generated.contains("decode_hci_event_tagged_items_variant"));
        assert!(generated.contains("decode_hci_event_trailing_bytes"));
        assert!(generated.contains("__stm32wb_event_data"));
        assert!(!generated.contains("vendor_event !"));
    }
}
