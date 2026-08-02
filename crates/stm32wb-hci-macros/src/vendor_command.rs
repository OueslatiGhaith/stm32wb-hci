//! Expansion backend for declarative vendor commands.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use stm32wb_hci_schema::{
    Completion, Constraint, Constraints, Field, FieldEncoding, Fields, Returns, TaggedEncoding,
    VariableEncodingShape, VendorCommand, WireSize,
};

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

/// Generate the complete command type directly from the shared schema.
pub(crate) fn expand_vendor_command(command: &VendorCommand) -> TokenStream2 {
    let name = &command.name;
    let params_name = format_ident!("{}Params", name);
    let cgid = command.cgid;
    let cid = command.cid;
    let fields = command.params.fields().map_or(&[][..], Fields::fields);
    let field_names = fields.iter().map(|field| &field.name).collect::<Vec<_>>();
    let field_types = fields.iter().map(|field| &field.ty).collect::<Vec<_>>();
    let params_fields = fields.iter().map(|field| {
        let attrs = &field.attrs;
        let name = &field.name;
        let ty = &field.ty;
        quote! {
            #(#attrs)*
            #name: #ty,
        }
    });
    let params_getters = expand_params_getters(fields);
    let params_size = expand_params_size(fields);
    let params_write = expand_params_write(fields, false);
    let params_write_async = expand_params_write(fields, true);
    let field_validations = expand_param_field_validations(fields);
    let schema_validations = expand_schema_validations(fields);
    let params_length_assert = command.params.lifetime.is_none().then(|| {
        let params_len = expand_wire_size(&command.params.max_size());
        quote! {
            const _: () = crate::vendor::command::assert_hci_payload_length(#params_len);
        }
    });
    let params_constructor =
        expand_params_constructor(command, &field_names, &field_types, &field_validations);
    let command_constructor =
        expand_command_constructor(command, &params_name, &field_names, &field_types);
    let completion_impl = expand_completion(command);
    let lifetime = command.params.lifetime.as_ref();
    let impl_generics = lifetime.map(|lifetime| quote!(<#lifetime>));
    let type_generics = lifetime.map(|lifetime| quote!(<#lifetime>));
    let default_impl = command.params.fields().is_none().then(|| {
        quote! {
            impl Default for #params_name {
                fn default() -> Self {
                    Self::new()
                }
            }

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

        #[doc = concat!("Parameters for [`", stringify!(#name), "`].")]
        pub struct #params_name #impl_generics {
            #(#params_fields)*
        }

        impl #impl_generics #params_name #type_generics {
            #params_constructor

            #params_getters

            /// Number of parameter bytes in the HCI wire representation.
            #[inline]
            pub fn encoded_len(&self) -> usize {
                #params_size
            }
        }

        impl #impl_generics ::bt_hci::WriteHci for #params_name #type_generics {
            #[inline]
            fn size(&self) -> usize {
                self.encoded_len()
            }

            #[inline]
            fn write_hci<W: ::embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                #params_write
            }

            #[inline]
            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                #params_write_async
            }
        }

        #[doc = concat!("STM32WB vendor command using [`", stringify!(#params_name), "`].")]
        pub struct #name #impl_generics(#params_name #type_generics);

        impl #impl_generics #name #type_generics {
            /// STM32 vendor command-group ID.
            pub const CGID: u16 = #cgid;
            /// Command ID within [`Self::CGID`].
            pub const CID: u16 = #cid;
            /// Vendor-specific Opcode Command Field.
            pub const OCF: u16 = crate::vendor::command::vendor_ocf(Self::CGID, Self::CID);

            #command_constructor

            /// Build the command from its already validated parameters.
            #[inline]
            pub fn from_params(params: #params_name #type_generics) -> Self {
                Self(params)
            }

            /// Borrow the command's domain-specific parameters.
            #[inline]
            pub fn params(&self) -> &#params_name #type_generics {
                &self.0
            }

            /// Consume the command and return its domain-specific parameters.
            #[inline]
            pub fn into_params(self) -> #params_name #type_generics {
                self.0
            }
        }

        impl #impl_generics ::bt_hci::cmd::Cmd for #name #type_generics {
            const OPCODE: ::bt_hci::cmd::Opcode = ::bt_hci::cmd::Opcode::new(
                ::bt_hci::cmd::OpcodeGroup::VENDOR_SPECIFIC,
                Self::OCF,
            );
            type Params = #params_name #type_generics;

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

fn expand_params_constructor(
    command: &VendorCommand,
    field_names: &[&syn::Ident],
    field_types: &[&syn::Type],
    field_validations: &TokenStream2,
) -> TokenStream2 {
    let has_variable_params = command.params.lifetime.is_some();
    match (has_variable_params, &command.constraints) {
        (false, None) => quote! {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn new(#(#field_names: #field_types),*) -> Self {
                Self { #(#field_names,)* }
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
                    Ok(Self { #(#field_names,)* })
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
                    #field_validations
                    let params = Self { #(#field_names,)* };
                    let actual = params.encoded_len();
                    if actual > u8::MAX as usize {
                        return Err(crate::vendor::command::HciLengthError::new(
                            actual,
                            0,
                            u8::MAX as usize,
                        ).into());
                    }
                    Ok(params)
                }
            }
        }
    }
}

fn expand_command_constructor(
    command: &VendorCommand,
    params_name: &syn::Ident,
    field_names: &[&syn::Ident],
    field_types: &[&syn::Type],
) -> TokenStream2 {
    let has_variable_params = command.params.lifetime.is_some();
    match (has_variable_params, &command.constraints) {
        (false, None) => quote! {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn new(#(#field_names: #field_types),*) -> Self {
                Self(#params_name::new(#(#field_names),*))
            }
        },
        (false, Some(_)) => quote! {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn try_new(
                #(#field_names: #field_types),*
            ) -> Result<Self, crate::vendor::command::HciConstraintError> {
                #params_name::try_new(#(#field_names),*).map(Self)
            }
        },
        (true, Some(_)) => quote! {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn try_new(
                #(#field_names: #field_types),*
            ) -> Result<Self, crate::vendor::command::HciValidationError> {
                #params_name::try_new(#(#field_names),*).map(Self)
            }
        },
        (true, None) => quote! {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn try_new(
                #(#field_names: #field_types),*
            ) -> Result<Self, crate::vendor::command::HciLengthError> {
                #params_name::try_new(#(#field_names),*).map(Self)
            }
        },
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
        Constraint::ImpliesOneOfOrRange {
            selector,
            selected,
            field,
            allowed,
            minimum,
            maximum,
        } => quote! {
            if #selector == #selected
                && ![#(#allowed),*].contains(&#field)
                && !((#minimum)..=(#maximum)).contains(&#field)
            {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#selector),
                        " == ",
                        stringify!(#selected),
                        " implies ",
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
        Constraint::ImpliesLenAtLeast {
            selector,
            selected,
            field,
            minimum,
        } => quote! {
            if #selector == #selected && #field.len() < (#minimum as usize) {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#selector),
                        " == ",
                        stringify!(#selected),
                        " implies ",
                        stringify!(#field),
                        ".len() >= ",
                        stringify!(#minimum),
                    ),
                ));
            }
        },
        Constraint::ImpliesLenEq {
            selector,
            selected,
            field,
            required,
        } => quote! {
            if #selector == #selected && #field.len() != (#required as usize) {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#selector),
                        " == ",
                        stringify!(#selected),
                        " implies ",
                        stringify!(#field),
                        ".len() == ",
                        stringify!(#required),
                    ),
                ));
            }
        },
        Constraint::LenEq { field, expected } => quote! {
            if #field.len() != usize::from(#expected) {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#field),
                        ".len() == usize::from(",
                        stringify!(#expected),
                        ")",
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
        Constraint::OffsetLenAtMost {
            offset,
            field,
            total,
        } => quote! {
            if usize::from(#offset)
                .checked_add(#field.len())
                .map_or(true, |end| end > usize::from(#total))
            {
                return Err(crate::vendor::command::HciConstraintError::new(
                    stringify!(#command),
                    concat!(
                        stringify!(#offset),
                        " + ",
                        stringify!(#field),
                        ".len() <= ",
                        stringify!(#total),
                    ),
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
            const _: () = crate::vendor::command::assert_hci_payload_length(0usize);

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
            let return_len = expand_wire_size(&fields.max_size());

            quote! {
                const _: () = crate::vendor::command::assert_hci_payload_length(
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
    let field_declarations = fields.fields().iter().map(|field| {
        let attrs = &field.attrs;
        let name = &field.name;
        let ty = &field.ty;
        quote! {
            #(#attrs)*
            pub #name: #ty,
        }
    });
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
            #(#field_declarations)*
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
        FieldEncoding::Fixed(_) => {
            let width = canonical_width(ty);
            quote! {
                crate::vendor::command::decode_declarative_fixed_field::<#ty, #width>(#cursor)
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
                    crate::vendor::command::decode_declarative_counted_bytes::<
                        #ty, #count_ty, #count_width, #min_len, #max_len
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
                    crate::vendor::command::decode_declarative_counted_items::<
                        #ty, #item_ty, #count_ty, #count_width, #item_width, #min_items, #max_items
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

fn expand_params_getters(fields: &[Field]) -> TokenStream2 {
    let getters = fields.iter().map(|field| {
        let attrs = &field.attrs;
        let name = &field.name;
        let ty = &field.ty;
        let (return_type, value) = match ty {
            syn::Type::Reference(reference) if reference.mutability.is_none() => {
                (quote!(#ty), quote!(self.#name))
            }
            _ => (quote!(&#ty), quote!(&self.#name)),
        };
        quote! {
            #(#attrs)*
            #[doc = concat!("Access the `", stringify!(#name), "` command parameter.")]
            #[inline]
            pub fn #name(&self) -> #return_type {
                #value
            }
        }
    });
    quote!(#(#getters)*)
}

fn expand_params_size(fields: &[Field]) -> TokenStream2 {
    let sizes = fields.iter().map(expand_field_size);
    quote!(0usize #(+ #sizes)*)
}

fn expand_field_size(field: &Field) -> TokenStream2 {
    let name = &field.name;
    match &field.encoding {
        FieldEncoding::Fixed(_) => canonical_width(&field.ty),
        FieldEncoding::Variable(encoding) => match &encoding.shape {
            VariableEncodingShape::CountedBytes { count, .. } => {
                let count_width = canonical_width(&count.ty);
                quote!(
                    #count_width
                        + ::core::convert::AsRef::<[u8]>::as_ref(&self.#name).len()
                )
            }
            VariableEncodingShape::CountedItems { count, item, .. } => {
                let count_width = canonical_width(&count.ty);
                let item_ty = &item.ty;
                let item_width = canonical_width(item_ty);
                quote!(
                    #count_width
                        + #item_width
                            * ::core::convert::AsRef::<[#item_ty]>::as_ref(&self.#name).len()
                )
            }
            VariableEncodingShape::Tagged(tagged) => {
                let arms = tagged.variants.iter().map(|variant| {
                    let pattern = &variant.pattern;
                    let tag_width = canonical_width(&tagged.tag.ty);
                    let payload_width = expand_wire_size(&variant.fields.max_size());
                    quote!(#pattern => (#tag_width + #payload_width),)
                });
                quote!({
                    #[allow(unused_variables)]
                    match &self.#name { #(#arms)* }
                })
            }
            VariableEncodingShape::TrailingBytes { .. } => quote!(
                ::core::convert::AsRef::<[u8]>::as_ref(&self.#name).len()
            ),
            VariableEncodingShape::BitmapItems { item, .. } => {
                let item_ty = &item.ty;
                let item_width = canonical_width(item_ty);
                quote!(
                    #item_width
                        * ::core::convert::AsRef::<[#item_ty]>::as_ref(&self.#name).len()
                )
            }
            VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => {
                unreachable!("the shared parser rejects event-only command fields")
            }
        },
    }
}

fn expand_params_write(fields: &[Field], asynchronous: bool) -> TokenStream2 {
    if fields.is_empty() {
        return quote! {
            let _ = writer;
            Ok(())
        };
    }

    let writes = fields
        .iter()
        .map(|field| expand_field_write(field, asynchronous));
    quote! {
        let mut writer = writer;
        #(#writes)*
        Ok(())
    }
}

fn expand_field_write(field: &Field, asynchronous: bool) -> TokenStream2 {
    let name = &field.name;
    match &field.encoding {
        FieldEncoding::Fixed(_) => {
            let width = canonical_width(&field.ty);
            expand_fixed_write(quote!(&self.#name), &width, asynchronous)
        }
        FieldEncoding::Variable(encoding) => match &encoding.shape {
            VariableEncodingShape::CountedBytes { count, .. } => {
                let count_ty = &count.ty;
                let count_width = canonical_width(count_ty);
                let write_count = expand_fixed_write(quote!(&count), &count_width, asynchronous);
                let write_value = if asynchronous {
                    quote!(
                        ::embedded_io_async::Write::write_all(&mut writer, value).await?;
                    )
                } else {
                    quote!(::embedded_io::Write::write_all(&mut writer, value)?;)
                };
                quote! {
                    {
                        let value = ::core::convert::AsRef::<[u8]>::as_ref(&self.#name);
                        let count = <#count_ty as crate::vendor::command::HciCount<
                            #count_width
                        >>::from_usize(value.len()).expect(
                            "validated counted byte length no longer fits its count type",
                        );
                        #write_count
                        #write_value
                    }
                }
            }
            VariableEncodingShape::CountedItems { count, item, .. } => {
                let count_ty = &count.ty;
                let count_width = canonical_width(count_ty);
                let item_ty = &item.ty;
                let item_width = canonical_width(item_ty);
                let write_count = expand_fixed_write(quote!(&count), &count_width, asynchronous);
                let write_item = expand_fixed_write(quote!(item), &item_width, asynchronous);
                quote! {
                    {
                        let value = ::core::convert::AsRef::<[#item_ty]>::as_ref(&self.#name);
                        let count = <#count_ty as crate::vendor::command::HciCount<
                            #count_width
                        >>::from_usize(value.len()).expect(
                            "validated counted item length no longer fits its count type",
                        );
                        #write_count
                        for item in value {
                            #write_item
                        }
                    }
                }
            }
            VariableEncodingShape::Tagged(tagged) => {
                expand_tagged_write(name, tagged, asynchronous)
            }
            VariableEncodingShape::TrailingBytes { .. } => {
                if asynchronous {
                    quote! {
                        ::embedded_io_async::Write::write_all(
                            &mut writer,
                            ::core::convert::AsRef::<[u8]>::as_ref(&self.#name),
                        ).await?;
                    }
                } else {
                    quote! {
                        ::embedded_io::Write::write_all(
                            &mut writer,
                            ::core::convert::AsRef::<[u8]>::as_ref(&self.#name),
                        )?;
                    }
                }
            }
            VariableEncodingShape::BitmapItems { item, .. } => {
                let item_ty = &item.ty;
                let item_width = canonical_width(item_ty);
                let write_item = expand_fixed_write(quote!(item), &item_width, asynchronous);
                quote! {
                    for item in ::core::convert::AsRef::<[#item_ty]>::as_ref(&self.#name) {
                        #write_item
                    }
                }
            }
            VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => {
                unreachable!("the shared parser rejects event-only command fields")
            }
        },
    }
}

fn expand_fixed_write(
    value: TokenStream2,
    width: &TokenStream2,
    asynchronous: bool,
) -> TokenStream2 {
    if asynchronous {
        quote! {
            crate::vendor::command::HciEncodeField::<#width>::write_hci_field_async(
                #value,
                &mut writer,
            ).await?;
        }
    } else {
        quote! {
            crate::vendor::command::HciEncodeField::<#width>::write_hci_field(
                #value,
                &mut writer,
            )?;
        }
    }
}

fn expand_tagged_write(
    name: &syn::Ident,
    tagged: &TaggedEncoding,
    asynchronous: bool,
) -> TokenStream2 {
    let tag_ty = &tagged.tag.ty;
    let tag_width = canonical_width(tag_ty);
    let arms = tagged.variants.iter().map(|variant| {
        let pattern = &variant.pattern;
        let tag = &variant.tag.literal;
        let write_tag = expand_fixed_write(quote!(&tag), &tag_width, asynchronous);
        let payload_writes = variant.fields.fields().iter().map(|field| {
            let field_name = &field.name;
            let FieldEncoding::Fixed(_) = &field.encoding else {
                unreachable!("tagged payload fields are validated as fixed-width")
            };
            let width = canonical_width(&field.ty);
            expand_fixed_write(quote!(#field_name), &width, asynchronous)
        });
        quote! {
            #pattern => {
                let tag: #tag_ty = #tag;
                #write_tag
                #(#payload_writes)*
            }
        }
    });
    quote! {
        match &self.#name {
            #(#arms)*
        }
    }
}

fn expand_param_field_validations(fields: &[Field]) -> TokenStream2 {
    let validations = fields.iter().filter_map(|field| {
        let name = &field.name;
        let FieldEncoding::Variable(encoding) = &field.encoding else {
            return None;
        };
        let validation = match &encoding.shape {
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
                    {
                        let actual = ::core::convert::AsRef::<[u8]>::as_ref(&#name).len();
                        let maximum = ::core::cmp::min(
                            #max_len,
                            <#count_ty as crate::vendor::command::HciCount<#count_width>>::MAX,
                        );
                        if <#count_ty as crate::vendor::command::HciCount<
                            #count_width
                        >>::from_usize(actual).is_none()
                            || !(#min_len..=maximum).contains(&actual)
                        {
                            return Err(crate::vendor::command::HciLengthError::new(
                                actual,
                                #min_len,
                                maximum,
                            ).into());
                        }
                    }
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
                let min_items = &min_items.literal;
                let max_items = &max_items.literal;
                quote! {
                    {
                        let actual = ::core::convert::AsRef::<[#item_ty]>::as_ref(&#name).len();
                        let maximum = ::core::cmp::min(
                            #max_items,
                            <#count_ty as crate::vendor::command::HciCount<#count_width>>::MAX,
                        );
                        if <#count_ty as crate::vendor::command::HciCount<
                            #count_width
                        >>::from_usize(actual).is_none()
                            || !(#min_items..=maximum).contains(&actual)
                        {
                            return Err(crate::vendor::command::HciLengthError::new(
                                actual,
                                #min_items,
                                maximum,
                            ).into());
                        }
                    }
                }
            }
            VariableEncodingShape::Tagged(_) => return None,
            VariableEncodingShape::TrailingBytes { min_len, max_len } => {
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote! {
                    {
                        let actual = ::core::convert::AsRef::<[u8]>::as_ref(&#name).len();
                        if !(#min_len..=#max_len).contains(&actual) {
                            return Err(crate::vendor::command::HciLengthError::new(
                                actual,
                                #min_len,
                                #max_len,
                            ).into());
                        }
                    }
                }
            }
            VariableEncodingShape::BitmapItems {
                bitmap,
                mask,
                item,
                max_items,
            } => {
                let mask = &mask.literal;
                let item_ty = &item.ty;
                let max_items = &max_items.literal;
                quote! {
                    {
                        let bitmap = crate::vendor::command::HciBitmap::to_usize(#bitmap);
                        if bitmap & !(#mask) != 0 {
                            return Err(crate::vendor::command::HciLengthError::new(
                                bitmap,
                                0,
                                #mask,
                            ).into());
                        }

                        let expected = (bitmap & #mask).count_ones() as usize;
                        let actual = ::core::convert::AsRef::<[#item_ty]>::as_ref(&#name).len();
                        if actual != expected || actual > #max_items {
                            return Err(crate::vendor::command::HciLengthError::new(
                                actual,
                                expected,
                                expected,
                            ).into());
                        }
                    }
                }
            }
            VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => {
                unreachable!("the shared parser rejects event-only command fields")
            }
        };
        Some(validation)
    });
    quote!(#(#validations)*)
}

fn expand_schema_validations(fields: &[Field]) -> TokenStream2 {
    let validations = fields.iter().filter_map(|field| {
        let FieldEncoding::Variable(encoding) = &field.encoding else {
            return None;
        };
        match &encoding.shape {
            VariableEncodingShape::CountedBytes { count, max_len, .. } => {
                let count_ty = &count.ty;
                let count_width = canonical_width(count_ty);
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
                let count_width = canonical_width(count_ty);
                let max_items = &max_items.literal;
                Some(quote! {
                    const _: () = ::core::assert!(
                        #max_items <= <#count_ty as crate::vendor::command::HciCount<#count_width>>::MAX
                    );
                })
            }
            VariableEncodingShape::Tagged(tagged) => {
                let tag_width = canonical_width(&tagged.tag.ty);
                let lengths = tagged.variants.iter().map(|variant| {
                    let payload = expand_wire_size(&variant.fields.max_size());
                    quote!((#tag_width + #payload))
                });
                let declared_minimum = &tagged.min_len.literal;
                let declared_maximum = &tagged.max_len.literal;
                Some(quote! {
                    const _: () = {
                        let lengths = [#(#lengths),*];
                        let mut minimum = usize::MAX;
                        let mut maximum = 0usize;
                        let mut index = 0usize;
                        while index < lengths.len() {
                            if lengths[index] < minimum {
                                minimum = lengths[index];
                            }
                            if lengths[index] > maximum {
                                maximum = lengths[index];
                            }
                            index += 1;
                        }
                        ::core::assert!(minimum == #declared_minimum);
                        ::core::assert!(maximum == #declared_maximum);
                    };
                })
            }
            VariableEncodingShape::TrailingBytes { .. }
            | VariableEncodingShape::BitmapItems { .. }
            | VariableEncodingShape::LengthPrefixedRecords { .. }
            | VariableEncodingShape::TaggedItems(_) => None,
        }
    });
    quote!(#(#validations)*)
}
