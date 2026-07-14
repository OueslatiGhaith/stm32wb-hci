//! Expansion backend for semantic HCI wire types.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use stm32wb_hci_schema::{
    BitflagsWireType, ClosedEnumWireType, CompositeWireType, OpenEnumWireType, OpenScalarWireType,
    PrimitiveWireType, RangedWireType, SemanticWireType, TransparentWireType, WireTypeDeclaration,
};

pub(crate) fn expand_wire_type(declaration: &SemanticWireType) -> TokenStream2 {
    match &declaration.declaration {
        WireTypeDeclaration::ClosedEnum(value) => expand_closed_enum(value, &declaration.adapters),
        WireTypeDeclaration::OpenEnum(value) => expand_open_enum(value, &declaration.adapters),
        WireTypeDeclaration::OpenScalar(value) => expand_open_scalar(value, &declaration.adapters),
        WireTypeDeclaration::Ranged(value) => expand_ranged(value, &declaration.adapters),
        WireTypeDeclaration::Bitflags(value) => expand_bitflags(value, &declaration.adapters),
        WireTypeDeclaration::Composite(value) => expand_composite(value, &declaration.adapters),
        WireTypeDeclaration::Primitive(value) => expand_primitive(value, &declaration.adapters),
        WireTypeDeclaration::Transparent(value) => expand_transparent(value, &declaration.adapters),
    }
}

fn expand_closed_enum(
    declaration: &ClosedEnumWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let attrs = &declaration.attrs;
    let visibility = &declaration.visibility;
    let name = &declaration.name;
    let repr = &declaration.repr;
    let variants = declaration.variants.iter().map(|variant| {
        let attrs = &variant.attrs;
        let name = &variant.name;
        let value = &variant.value;
        quote! { #(#attrs)* #name = #value, }
    });

    let command = if adapters.command() {
        let width = declaration
            .width
            .as_ref()
            .expect("the schema requires a command width");
        let encode_arms = declaration
            .variants
            .iter()
            .map(|variant| {
                let cfg = cfg_attributes(&variant.attrs);
                let value = &variant.value;
                let name = &variant.name;
                quote! { #(#cfg)* Self::#name => #value, }
            })
            .collect::<Vec<_>>();
        let decode_checks = declaration.variants.iter().map(|variant| {
            let cfg = cfg_attributes(&variant.attrs);
            let expected = &variant.value;
            let name = &variant.name;
            quote! {
                #(#cfg)*
                if value == #expected {
                    return Ok(Self::#name);
                }
            }
        });
        quote! {
            impl crate::wire::HciEncodeField<#width> for #name {
                fn write_hci_field<W: embedded_io::Write>(
                    &self,
                    writer: W,
                ) -> Result<(), W::Error> {
                    let value: #repr = match self { #(#encode_arms)* };
                    <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field(
                        &value,
                        writer,
                    )
                }

                async fn write_hci_field_async<W: embedded_io_async::Write>(
                    &self,
                    writer: W,
                ) -> Result<(), W::Error> {
                    let value: #repr = match self { #(#encode_arms)* };
                    <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field_async(
                        &value,
                        writer,
                    )
                    .await
                }
            }

            impl crate::wire::HciDecodeField<#width> for #name {
                fn from_hci_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, bt_hci::FromHciBytesError> {
                    let value =
                        <#repr as crate::wire::HciDecodeField<#width>>::from_hci_field(bytes)?;
                    #(#decode_checks)*
                    Err(bt_hci::FromHciBytesError::InvalidValue)
                }
            }
        }
    } else {
        TokenStream2::new()
    };

    let conversion = if adapters.event() || adapters.conversion() {
        let error_ty = declaration
            .try_from_error
            .as_ref()
            .expect("the schema requires a conversion error");
        let invalid_value = declaration
            .invalid_value
            .as_ref()
            .expect("the schema requires an invalid-value expression");
        let try_checks = declaration.variants.iter().map(|variant| {
            let cfg = cfg_attributes(&variant.attrs);
            let expected = &variant.value;
            let name = &variant.name;
            quote! {
                #(#cfg)*
                if value == #expected {
                    return Ok(Self::#name);
                }
            }
        });
        let from_arms = declaration.variants.iter().map(|variant| {
            let cfg = cfg_attributes(&variant.attrs);
            let value = &variant.value;
            let variant_name = &variant.name;
            quote! { #(#cfg)* #name::#variant_name => #value, }
        });
        quote! {
            impl core::convert::TryFrom<#repr> for #name {
                type Error = #error_ty;

                fn try_from(value: #repr) -> Result<Self, #error_ty> {
                    #(#try_checks)*
                    Err((#invalid_value)(value))
                }
            }

            impl From<#name> for #repr {
                fn from(value: #name) -> Self {
                    match value { #(#from_arms)* }
                }
            }
        }
    } else {
        TokenStream2::new()
    };

    let event = if adapters.event() {
        let width = declaration
            .width
            .as_ref()
            .expect("the schema requires an event width");
        let event_error = declaration
            .event_error
            .as_ref()
            .expect("the schema requires an event error mapper");
        quote! {
            impl crate::wire::HciEventField<#width> for #name {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    let value = <#repr>::from_le_bytes(*bytes);
                    <Self as core::convert::TryFrom<#repr>>::try_from(value)
                        .map_err(#event_error)
                }
            }
        }
    } else {
        TokenStream2::new()
    };

    quote! {
        #(#attrs)*
        #[repr(#repr)]
        #visibility enum #name {
            #(#variants)*
        }

        #command
        #conversion
        #event
    }
}

fn expand_open_enum(
    declaration: &OpenEnumWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let attrs = &declaration.attrs;
    let visibility = &declaration.visibility;
    let name = &declaration.name;
    let repr = &declaration.repr;
    let width = &declaration.width;
    let fallback = &declaration.fallback;
    let discriminants = declaration.variants.iter().map(|variant| {
        let cfg = cfg_attributes(&variant.attrs);
        let name = &variant.name;
        let value = &variant.value;
        quote! { #(#cfg)* #name = #value, }
    });
    let variants = declaration.variants.iter().map(|variant| {
        let attrs = &variant.attrs;
        let name = &variant.name;
        quote! { #(#attrs)* #name, }
    });
    let from_raw = declaration.variants.iter().map(|variant| {
        let cfg = cfg_attributes(&variant.attrs);
        let expected = &variant.value;
        let name = &variant.name;
        quote! {
            #(#cfg)*
            if value == #expected {
                return Self::#name;
            }
        }
    });
    let into_raw = declaration.variants.iter().map(|variant| {
        let cfg = cfg_attributes(&variant.attrs);
        let value = &variant.value;
        let variant_name = &variant.name;
        quote! { #(#cfg)* #name::#variant_name => #value, }
    });
    let command_encode = declaration
        .variants
        .iter()
        .map(|variant| {
            let cfg = cfg_attributes(&variant.attrs);
            let variant_name = &variant.name;
            let value = &variant.value;
            quote! { #(#cfg)* Self::#variant_name => #value, }
        })
        .collect::<Vec<_>>();

    let command = adapters.command().then(|| {
        quote! {
            impl crate::wire::HciEncodeField<#width> for #name {
                fn write_hci_field<W: embedded_io::Write>(
                    &self,
                    writer: W,
                ) -> Result<(), W::Error> {
                    let value: #repr = match self {
                        #(#command_encode)*
                        Self::#fallback(value) => *value,
                    };
                    <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field(&value, writer)
                }

                async fn write_hci_field_async<W: embedded_io_async::Write>(
                    &self,
                    writer: W,
                ) -> Result<(), W::Error> {
                    let value: #repr = match self {
                        #(#command_encode)*
                        Self::#fallback(value) => *value,
                    };
                    <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field_async(
                        &value,
                        writer,
                    )
                    .await
                }
            }

            impl crate::wire::HciDecodeField<#width> for #name {
                fn from_hci_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, bt_hci::FromHciBytesError> {
                    <#repr as crate::wire::HciDecodeField<#width>>::from_hci_field(bytes)
                        .map(Into::into)
                }
            }
        }
    });
    let event = adapters.event().then(|| {
        quote! {
            impl crate::wire::HciEventField<#width> for #name {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    Ok(<#repr>::from_le_bytes(*bytes).into())
                }
            }
        }
    });

    quote! {
        const _: () = {
            #[allow(dead_code)]
            #[repr(#repr)]
            enum WireDiscriminants { #(#discriminants)* }
        };

        #(#attrs)*
        #visibility enum #name {
            #(#variants)*
            /// Unrecognized wire value retained verbatim.
            #fallback(#repr),
        }

        impl From<#repr> for #name {
            fn from(value: #repr) -> Self {
                #(#from_raw)*
                Self::#fallback(value)
            }
        }

        impl From<#name> for #repr {
            fn from(value: #name) -> Self {
                match value {
                    #(#into_raw)*
                    #name::#fallback(value) => value,
                }
            }
        }

        #command
        #event
    }
}

fn expand_open_scalar(
    declaration: &OpenScalarWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let attrs = &declaration.attrs;
    let visibility = &declaration.visibility;
    let name = &declaration.name;
    let repr = &declaration.repr;
    let width = &declaration.width;
    let command = expand_transparent_scalar_command(name, repr, width, quote!(self.0), true);
    let command = adapters.command().then_some(command);
    let event = adapters.event().then(|| {
        quote! {
            impl crate::wire::HciEventField<#width> for #name {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    Ok(Self::new(<#repr>::from_le_bytes(*bytes)))
                }
            }
        }
    });
    quote! {
        #(#attrs)*
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #visibility struct #name(#repr);

        impl #name {
            /// Construct the semantic value from its complete wire domain.
            pub const fn new(value: #repr) -> Self { Self(value) }

            /// Return the underlying wire value.
            pub const fn value(self) -> #repr { self.0 }
        }

        impl From<#repr> for #name {
            fn from(value: #repr) -> Self { Self::new(value) }
        }

        impl From<#name> for #repr {
            fn from(value: #name) -> Self { value.value() }
        }

        #command
        #event
    }
}

fn expand_ranged(
    declaration: &RangedWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let attrs = &declaration.attrs;
    let visibility = &declaration.visibility;
    let name = &declaration.name;
    let repr = &declaration.repr;
    let width = &declaration.width;
    let minimum = &declaration.minimum;
    let maximum = &declaration.maximum;
    let sentinel_const = declaration.sentinel.as_ref().map(|sentinel| {
        let sentinel_name = &sentinel.name;
        let sentinel = &sentinel.value;
        quote! {
            /// Additional accepted value outside the inclusive range.
            pub const #sentinel_name: Self = Self(#sentinel);
        }
    });
    let sentinel_accept = declaration.sentinel.as_ref().map(|sentinel| {
        let value = &sentinel.value;
        quote!(|| value == #value)
    });
    let sentinel_error = declaration
        .sentinel
        .as_ref()
        .map(|sentinel| {
            let value = &sentinel.value;
            quote!(Some(#value as u64))
        })
        .unwrap_or_else(|| quote!(None));
    let is_sentinel = declaration.sentinel.as_ref().map(|sentinel| {
        let value = &sentinel.value;
        quote! {
            /// Whether this value is the declared out-of-range sentinel.
            pub const fn is_sentinel(self) -> bool { self.0 == #value }
        }
    });
    let command = if adapters.command() {
        let scalar = expand_transparent_scalar_command(name, repr, width, quote!(self.0), false);
        quote! {
            #scalar

            impl crate::wire::HciDecodeField<#width> for #name {
                fn from_hci_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, bt_hci::FromHciBytesError> {
                    let value =
                        <#repr as crate::wire::HciDecodeField<#width>>::from_hci_field(bytes)?;
                    Self::try_new(value).map_err(|_| bt_hci::FromHciBytesError::InvalidValue)
                }
            }
        }
    } else {
        TokenStream2::new()
    };
    let event = declaration.event_error.as_ref().map(|event_error| {
        quote! {
            impl crate::wire::HciEventField<#width> for #name {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    let value = <#repr>::from_le_bytes(*bytes);
                    Self::try_new(value).map_err(#event_error)
                }
            }
        }
    });
    quote! {
        #(#attrs)*
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #visibility struct #name(#repr);

        impl #name {
            /// Smallest accepted value.
            pub const MINIMUM: #repr = #minimum;
            /// Largest accepted value.
            pub const MAXIMUM: #repr = #maximum;
            #sentinel_const

            /// Construct a value within the declared domain.
            pub const fn try_new(
                value: #repr,
            ) -> Result<Self, crate::vendor::command::HciValueError> {
                if (value >= Self::MINIMUM && value <= Self::MAXIMUM)
                    #sentinel_accept
                {
                    Ok(Self(value))
                } else {
                    Err(crate::vendor::command::HciValueError::new(
                        value as u64,
                        Self::MINIMUM as u64,
                        Self::MAXIMUM as u64,
                        #sentinel_error,
                    ))
                }
            }

            #is_sentinel

            /// Return the underlying wire value.
            pub const fn value(self) -> #repr { self.0 }
        }

        impl TryFrom<#repr> for #name {
            type Error = crate::vendor::command::HciValueError;
            fn try_from(value: #repr) -> Result<Self, Self::Error> { Self::try_new(value) }
        }

        impl From<#name> for #repr {
            fn from(value: #name) -> Self { value.0 }
        }

        #command
        #event
    }
}

fn expand_bitflags(
    declaration: &BitflagsWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let attrs = &declaration.attrs;
    let visibility = &declaration.visibility;
    let name = &declaration.name;
    let repr = &declaration.repr;
    let width = &declaration.width;
    let flags = declaration
        .flags
        .iter()
        .map(|flag| {
            let attrs = &flag.attrs;
            let name = &flag.name;
            let value = &flag.value;
            quote! { #(#attrs)* const #name = #value; }
        })
        .collect::<Vec<_>>();
    let command = adapters.command().then(|| {
        quote! {
            impl crate::wire::HciEncodeField<#width> for #name {
                fn write_hci_field<W: embedded_io::Write>(
                    &self,
                    writer: W,
                ) -> Result<(), W::Error> {
                    <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field(
                        &self.bits(),
                        writer,
                    )
                }

                async fn write_hci_field_async<W: embedded_io_async::Write>(
                    &self,
                    writer: W,
                ) -> Result<(), W::Error> {
                    <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field_async(
                        &self.bits(),
                        writer,
                    )
                    .await
                }
            }

            impl crate::wire::HciDecodeField<#width> for #name {
                fn from_hci_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, bt_hci::FromHciBytesError> {
                    let bits =
                        <#repr as crate::wire::HciDecodeField<#width>>::from_hci_field(bytes)?;
                    Self::from_bits(bits).ok_or(bt_hci::FromHciBytesError::InvalidValue)
                }
            }

            impl crate::vendor::command::HciBitmap for #name {
                fn to_usize(self) -> usize { self.bits() as usize }
            }
        }
    });
    let event = declaration.event_error.as_ref().map(|event_error| {
        quote! {
            impl crate::wire::HciEventField<#width> for #name {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    let bits = <#repr>::from_le_bytes(*bytes);
                    Self::from_bits(bits).ok_or_else(|| (#event_error)(bits))
                }
            }
        }
    });
    quote! {
        #[cfg(not(feature = "defmt"))]
        bitflags::bitflags! {
            #(#attrs)*
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            #visibility struct #name: #repr { #(#flags)* }
        }

        #[cfg(feature = "defmt")]
        defmt::bitflags! {
            #(#attrs)*
            #visibility struct #name: #repr { #(#flags)* }
        }

        #command
        #event
    }
}

fn expand_composite(
    declaration: &CompositeWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let ty = &declaration.ty;
    let width = &declaration.width;
    let field_widths = declaration.fields.iter().map(|field| &field.width);
    let command = declaration.encode.as_ref().map(|(value, encode)| {
        let field_names = declaration
            .fields
            .iter()
            .map(|field| &field.name)
            .collect::<Vec<_>>();
        let sync_fields = declaration.fields.iter().map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let width = &field.width;
            quote! {
                <#ty as crate::wire::HciEncodeField<#width>>::write_hci_field(
                    &#name,
                    &mut writer,
                )?;
            }
        });
        let async_fields = declaration.fields.iter().map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let width = &field.width;
            quote! {
                <#ty as crate::wire::HciEncodeField<#width>>::write_hci_field_async(
                    &#name,
                    &mut writer,
                )
                .await?;
            }
        });
        quote! {
            impl crate::wire::HciEncodeField<#width> for #ty {
                fn write_hci_field<W: embedded_io::Write>(
                    &self,
                    mut writer: W,
                ) -> Result<(), W::Error> {
                    let (#(#field_names,)*) = (|#value: &#ty| #encode)(self);
                    #(#sync_fields)*
                    Ok(())
                }

                async fn write_hci_field_async<W: embedded_io_async::Write>(
                    &self,
                    mut writer: W,
                ) -> Result<(), W::Error> {
                    let (#(#field_names,)*) = (|#value: &#ty| #encode)(self);
                    #(#async_fields)*
                    Ok(())
                }
            }
        }
    });
    let event = declaration.decode.as_ref().map(|decode| {
        let decode_fields = declaration.fields.iter().map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let width = &field.width;
            quote! {
                let #name = {
                    let __end = __offset + #width;
                    let __bytes: &[u8; #width] = core::convert::TryInto::try_into(
                        &bytes[__offset..__end],
                    )
                    .expect("declared composite field width");
                    __offset = __end;
                    <#ty as crate::wire::HciEventField<#width>>::from_hci_event_field(__bytes)?
                };
            }
        });
        quote! {
            impl crate::wire::HciEventField<#width> for #ty {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    let mut __offset = 0usize;
                    #(#decode_fields)*
                    debug_assert_eq!(__offset, #width);
                    #decode
                }
            }
        }
    });
    debug_assert_eq!(adapters.command(), command.is_some());
    debug_assert_eq!(adapters.event(), event.is_some());
    quote! {
        const _: [(); #width] = [(); 0 #(+ #field_widths)*];
        #command
        #event
    }
}

fn expand_primitive(
    declaration: &PrimitiveWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let ty = &declaration.ty;
    let width = &declaration.width;
    let command = adapters.command().then(|| {
        quote! {
            impl crate::wire::HciEncodeField<#width> for #ty {
                fn write_hci_field<W: embedded_io::Write>(
                    &self,
                    mut writer: W,
                ) -> Result<(), W::Error> {
                    writer.write_all(&self.to_le_bytes())
                }

                async fn write_hci_field_async<W: embedded_io_async::Write>(
                    &self,
                    mut writer: W,
                ) -> Result<(), W::Error> {
                    writer.write_all(&self.to_le_bytes()).await
                }
            }

            impl crate::wire::HciDecodeField<#width> for #ty {
                fn from_hci_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, bt_hci::FromHciBytesError> {
                    Ok(<#ty>::from_le_bytes(*bytes))
                }
            }
        }
    });
    let event = adapters.event().then(|| {
        quote! {
            impl crate::wire::HciEventField<#width> for #ty {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    Ok(<#ty>::from_le_bytes(*bytes))
                }
            }
        }
    });
    quote! { #command #event }
}

fn expand_transparent(
    declaration: &TransparentWireType,
    adapters: &stm32wb_hci_schema::WireAdapters,
) -> TokenStream2 {
    let ty = &declaration.ty;
    let inner = &declaration.inner;
    let width = &declaration.width;
    let command = adapters.command().then(|| quote! {
        impl crate::wire::HciEncodeField<#width> for #ty {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <#inner as crate::wire::HciEncodeField<#width>>::write_hci_field(&self.0, writer)
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <#inner as crate::wire::HciEncodeField<#width>>::write_hci_field_async(
                    &self.0,
                    writer,
                )
                .await
            }
        }

        impl crate::wire::HciDecodeField<#width> for #ty {
            fn from_hci_field(
                bytes: &[u8; #width],
            ) -> Result<Self, bt_hci::FromHciBytesError> {
                <#inner as crate::wire::HciDecodeField<#width>>::from_hci_field(bytes).map(Self)
            }
        }
    });
    let event = adapters.event().then(|| {
        quote! {
            impl crate::wire::HciEventField<#width> for #ty {
                fn from_hci_event_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, crate::vendor::event::Error> {
                    <#inner as crate::wire::HciEventField<#width>>::from_hci_event_field(bytes)
                        .map(Self)
                }
            }
        }
    });
    quote! { #command #event }
}

fn expand_transparent_scalar_command(
    name: &syn::Ident,
    repr: &syn::Type,
    width: &syn::LitInt,
    value: TokenStream2,
    include_decode: bool,
) -> TokenStream2 {
    let decode = include_decode.then(|| {
        quote! {
            impl crate::wire::HciDecodeField<#width> for #name {
                fn from_hci_field(
                    bytes: &[u8; #width],
                ) -> Result<Self, bt_hci::FromHciBytesError> {
                    <#repr as crate::wire::HciDecodeField<#width>>::from_hci_field(bytes)
                        .map(Self::new)
                }
            }
        }
    });
    quote! {
        impl crate::wire::HciEncodeField<#width> for #name {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field(
                    &#value,
                    writer,
                )
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <#repr as crate::wire::HciEncodeField<#width>>::write_hci_field_async(
                    &#value,
                    writer,
                )
                .await
            }
        }
        #decode
    }
}

fn cfg_attributes(attrs: &[syn::Attribute]) -> Vec<&syn::Attribute> {
    attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .collect()
}
