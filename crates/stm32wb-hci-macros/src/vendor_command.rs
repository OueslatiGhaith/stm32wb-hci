//! Expansion backend for declarative vendor commands.

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, quote_spanned};
use stm32wb_hci_schema::{
    Completion, Constraint, Constraints, Field, FieldEncoding, Fields, Returns, TaggedEncoding,
    VariableEncodingShape, VendorCommand,
};

/// Generate the complete command type directly from the shared schema.
pub(crate) fn expand_vendor_command(command: &VendorCommand) -> TokenStream2 {
    let name = &command.name;
    let params_name = format_ident!("{}Params", name);
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
    let params_constructor =
        expand_params_constructor(command, &field_names, &field_types, &params_value);
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
        pub struct #params_name #impl_generics(
            crate::vendor::command::DeclarativeParams<#params_type>
        );

        impl #impl_generics #params_name #type_generics {
            #params_constructor

            /// Number of parameter bytes in the HCI wire representation.
            #[inline]
            pub fn encoded_len(&self) -> usize {
                ::bt_hci::WriteHci::size(self)
            }
        }

        impl #impl_generics ::bt_hci::WriteHci for #params_name #type_generics {
            #[inline]
            fn size(&self) -> usize {
                ::bt_hci::WriteHci::size(&self.0)
            }

            #[inline]
            fn write_hci<W: ::embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                ::bt_hci::WriteHci::write_hci(&self.0, writer)
            }

            #[inline]
            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                ::bt_hci::WriteHci::write_hci_async(&self.0, writer).await
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
                let count_width = &count.width.literal;
                let item_ty = &item.ty;
                let item_width = &item.width.literal;
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
            VariableEncodingShape::CountedBytes {
                count,
                min_len,
                max_len,
            } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote!(crate::vendor::command::CountedBytes<
                    #ty, #count_ty, #count_width, #min_len, #max_len
                >)
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
                quote!(crate::vendor::command::CountedItems<
                    #ty, #item_ty, #count_ty, #count_width, #item_width, #min_items, #max_items
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
            VariableEncodingShape::CountedBytes {
                count,
                min_len,
                max_len,
            } => {
                let count_ty = &count.ty;
                let count_width = &count.width.literal;
                let min_len = &min_len.literal;
                let max_len = &max_len.literal;
                quote!(crate::vendor::command::CountedBytes::<
                    _, #count_ty, #count_width, #min_len, #max_len
                >::try_new(#name)?)
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
                quote!(crate::vendor::command::CountedItems::<
                    _, #item_ty, #count_ty, #count_width, #item_width, #min_items, #max_items
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
