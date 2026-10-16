//! The `Direct` implementations calling a command's STM32WBA BLE stack
//! function, one per segment of the command's history.
//!
//! STM32CubeWBA declares one function per command, taking the command's
//! parameters in order, followed by pointers to its return parameters after
//! the status that do not echo a parameter, and returning the status. The
//! catalog's layouts are derived from the same structures, so each call is
//! generated from them.

use std::collections::BTreeMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use stm32wb_catalog::layout::{element_width, struct_width};
use stm32wb_catalog::{
    Bundled, Completion, Element, Field, FieldType, Layout, Platform, ReleaseRange, Scalar, Structs,
};

use crate::cfg;

/// The `#[cfg]` predicate of the STM32WBA targets implementing `c_name`.
pub(crate) fn available(c_name: &syn::Ident) -> syn::Result<TokenStream> {
    let error = |message: String| syn::Error::new(c_name.span(), message);
    let bundled = stm32wb_catalog::bundled(Platform::Stm32wba)
        .map_err(|failure| error(format!("the bundled catalog is invalid: {failure}")))?;
    let segments = bundled
        .command_segments(&c_name.to_string())
        .map_err(|failure| error(failure.to_string()))?;
    if segments.is_empty() {
        return Err(error(format!("{c_name} exists in no STM32WBA release")));
    }
    Ok(cfg::targets(
        &bundled.catalog,
        segments
            .iter()
            .map(|segment| (segment.releases, segment.profiles)),
    ))
}

/// The lifetimes `types` mention, as generics.
pub(crate) fn lifetime_generics<'a>(
    types: impl Iterator<Item = &'a syn::Type>,
) -> Option<TokenStream> {
    let mut lifetimes: Vec<syn::Lifetime> = Vec::new();
    for ty in types {
        let tokens = quote!(#ty).into_iter().collect::<Vec<_>>();
        for pair in tokens.windows(2) {
            if let [
                proc_macro2::TokenTree::Punct(punct),
                proc_macro2::TokenTree::Ident(ident),
            ] = pair
                && punct.as_char() == '\''
            {
                let lifetime = syn::Lifetime::new(&format!("'{ident}"), ident.span());
                if !lifetimes.contains(&lifetime) {
                    lifetimes.push(lifetime);
                }
            }
        }
    }
    (!lifetimes.is_empty()).then(|| quote!(<#(#lifetimes),*>))
}

/// The `Direct` implementations of the command `c_name`, for every segment
/// of its history on STM32WBA whose layouts a call can be generated from.
/// `ty` gives the command's type in a segment's releases, and the lifetimes
/// it is generic over.
pub(crate) fn expand(
    bundled: &Bundled,
    c_name: &str,
    ty: impl Fn(ReleaseRange) -> (TokenStream, Option<TokenStream>),
) -> syn::Result<TokenStream> {
    if bundled.catalog.platform != Platform::Stm32wba {
        return Ok(TokenStream::new());
    }
    let segments = bundled
        .command_segments(c_name)
        .map_err(|failure| syn::Error::new(proc_macro2::Span::call_site(), failure.to_string()))?;
    let mut tokens = TokenStream::new();
    for segment in &segments {
        let active = &segment.entry;
        if active.excluded.is_some() {
            continue;
        }
        let Ok(params) = active.params.fields else {
            continue;
        };
        let (returns, return_structs) = match &active.returns {
            Some(returns) => match returns.fields {
                Ok(fields) => (fields, returns.structs),
                Err(_) => continue,
            },
            None => (&[][..], active.params.structs),
        };
        let Some(call) = call(
            active.name,
            active.definition.completion,
            params,
            active.params.structs,
            returns,
            return_structs,
        ) else {
            continue;
        };
        let predicate = cfg::targets(&bundled.catalog, [(segment.releases, segment.profiles)]);
        let (ty, generics) = ty(segment.releases);
        let output = match active.definition.completion {
            Completion::CommandComplete => quote!(<Self as ::bt_hci::cmd::SyncCmd>::Return),
            Completion::CommandStatus => quote!(()),
        };
        tokens.extend(quote! {
            #[cfg(all(feature = "direct", #predicate))]
            impl #generics ::stm32wb_hci::direct::Direct for #ty {
                type Output = #output;

                #[inline]
                #[allow(unused_variables, unused_mut, clippy::all)]
                unsafe fn call(
                    &self,
                ) -> ::core::result::Result<Self::Output, ::stm32wb_hci::direct::Error> {
                    #call
                }
            }
        });
    }
    Ok(tokens)
}

fn rust_scalar(scalar: Scalar) -> TokenStream {
    let ident = format_ident!("{}", scalar.name());
    quote!(#ident)
}

/// The width of one element in bytes.
fn width(element: &Element, structs: &Structs) -> Option<usize> {
    element_width(element, structs).ok().map(usize::from)
}

/// The body of `Direct::call`, or `None` if a member has no C argument this
/// generator knows how to pass.
fn call(
    c_name: &str,
    completion: Completion,
    params: &[Field],
    structs: &Structs,
    returns: &[Field],
    return_structs: &Structs,
) -> Option<TokenStream> {
    let link_name = c_name.to_uppercase();
    let function = format_ident!("{c_name}");
    let params_max = Layout::Fields(params.to_vec()).envelope(structs).ok()??.max;

    // The C parameter types and arguments, in order.
    let mut c_params = Vec::new();
    let mut c_args = Vec::new();
    let mut reads = Vec::new();
    // Each parameter's encoded bytes, for echoes, and value, for counts and
    // selectors.
    let mut param_bytes = BTreeMap::new();
    let mut values = BTreeMap::new();

    for (index, field) in params.iter().enumerate() {
        let bytes = format_ident!("b{index}");
        let arg = format_ident!("p{index}");
        match &field.ty {
            FieldType::Scalar(scalar) => {
                let ty = rust_scalar(*scalar);
                let width = usize::from(scalar.width());
                reads.push(quote! {
                    let #bytes: &[u8] = r.array::<#width>()?;
                    let #arg = #ty::from_le_bytes(d::element::<#width>(#bytes, 0));
                });
                c_params.push(ty);
                values.insert(field.name.as_str(), quote!(#arg));
            }
            FieldType::Struct(name) => {
                let width = usize::from(struct_width(name, structs).ok()?);
                reads.push(quote!(let #bytes: &[u8] = r.take(#width)?;));
                c_params.push(quote!(*const u8));
                c_args.push(quote!(#bytes.as_ptr()));
                param_bytes.insert(field.name.as_str(), bytes);
                continue;
            }
            FieldType::Array { element, len } => {
                let len = usize::from(*len);
                reads.push(elements(&bytes, &arg, element, structs, quote!(#len), len)?);
                c_params.push(pointer(element, quote!(*const)));
                c_args.push(quote!(#arg));
                param_bytes.insert(field.name.as_str(), bytes);
                continue;
            }
            FieldType::Counted {
                element,
                count,
                capacity,
            } => {
                let count = values.get(count.as_str())?.clone();
                let capacity = usize::from(*capacity);
                let len = format_ident!("n{index}");
                reads.push(quote!(let #len = d::count(#count, #capacity)?;));
                reads.push(elements(
                    &bytes,
                    &arg,
                    element,
                    structs,
                    quote!(#len),
                    capacity,
                )?);
                c_params.push(pointer(element, quote!(*const)));
                c_args.push(quote!(#arg));
                param_bytes.insert(field.name.as_str(), bytes);
                continue;
            }
            FieldType::Union { selector, variants } => {
                let selector = values.get(selector.as_str())?.clone();
                let mut arms = Vec::new();
                let mut default = quote!(return Err(d::Error::Invalid));
                for variant in variants {
                    let width = usize::from(variant.width);
                    match variant.tag {
                        Some(tag) => arms.push(quote!(#tag => #width,)),
                        None => default = quote!(#width),
                    }
                }
                reads.push(quote! {
                    let #bytes: &[u8] = r.take(match #selector as u64 {
                        #(#arms)*
                        _ => #default,
                    })?;
                });
                // The function takes a pointer to a union of the alternatives.
                c_params.push(quote!(*const u8));
                c_args.push(quote!(#bytes.as_ptr()));
                param_bytes.insert(field.name.as_str(), bytes);
                continue;
            }
            // An omitted optional member changes the call; none is generated.
            FieldType::Optional(_) => return None,
        }
        c_args.push(quote!(#arg));
        param_bytes.insert(field.name.as_str(), bytes);
    }

    let mut outputs = Vec::new();
    let mut puts = Vec::new();
    let mut return_values = BTreeMap::new();
    let after_status = match completion {
        Completion::CommandStatus => &[][..],
        Completion::CommandComplete => match returns {
            [status, rest @ ..]
                if status.name == "Status" && status.ty == FieldType::Scalar(Scalar::U8) =>
            {
                rest
            }
            [] => &[][..],
            _ => return None,
        },
    };
    for (index, field) in after_status.iter().enumerate() {
        if let Some(bytes) = param_bytes.get(field.name.as_str()) {
            puts.push(quote!(w.put(#bytes)?;));
            continue;
        }
        let out = format_ident!("o{index}");
        match &field.ty {
            FieldType::Scalar(scalar) => {
                let ty = rust_scalar(*scalar);
                outputs.push(quote!(let mut #out: #ty = 0;));
                c_params.push(quote!(*mut #ty));
                c_args.push(quote!(&mut #out));
                puts.push(quote!(w.put(&#out.to_le_bytes())?;));
                return_values.insert(field.name.as_str(), quote!(#out));
            }
            FieldType::Struct(name) => {
                let width = usize::from(struct_width(name, return_structs).ok()?);
                outputs.push(quote!(let mut #out = [0u8; #width];));
                c_params.push(quote!(*mut u8));
                c_args.push(quote!(#out.as_mut_ptr()));
                puts.push(quote!(w.put(&#out)?;));
            }
            FieldType::Array { element, len } => {
                let len = usize::from(*len);
                outputs.push(output_buffer(&out, element, return_structs, len)?);
                c_params.push(pointer(element, quote!(*mut)));
                c_args.push(quote!(#out.as_mut_ptr()));
                puts.push(put_elements(&out, element, return_structs, quote!(#len))?);
            }
            FieldType::Counted {
                element,
                count,
                capacity,
            } => {
                let count = return_values.get(count.as_str())?.clone();
                let capacity = usize::from(*capacity);
                outputs.push(output_buffer(&out, element, return_structs, capacity)?);
                c_params.push(pointer(element, quote!(*mut)));
                c_args.push(quote!(#out.as_mut_ptr()));
                let len = format_ident!("m{index}");
                puts.push(quote!(let #len = d::count(#count, #capacity)?;));
                puts.push(put_elements(&out, element, return_structs, quote!(#len))?);
            }
            FieldType::Union { .. } | FieldType::Optional(_) => return None,
        }
    }

    let call_params = c_params.iter().enumerate().map(|(index, ty)| {
        let arg = format_ident!("a{index}");
        quote!(#arg: #ty)
    });
    let finish = match completion {
        Completion::CommandStatus => quote!(Ok(())),
        Completion::CommandComplete => {
            let returns_max = Layout::Fields(after_status.to_vec())
                .envelope(return_structs)
                .ok()??
                .max;
            quote! {
                let mut wbuf = [0u8; #returns_max];
                let mut w = d::Writer::new(&mut wbuf);
                #(#puts)*
                w.finish()
            }
        }
    };
    Some(quote! {
        use ::stm32wb_hci::direct::__private as d;
        unsafe extern "C" {
            #[link_name = #link_name]
            fn #function(#(#call_params),*) -> u8;
        }
        let mut buf = [0u8; #params_max];
        let mut r = d::Reader::new(&mut buf, ::bt_hci::cmd::Cmd::params(self))?;
        #(#reads)*
        r.finish()?;
        #(#outputs)*
        d::status(unsafe { #function(#(#c_args),*) })?;
        #finish
    })
}

/// The C pointer type to an array of `element`: integers by their type,
/// packed structures as bytes.
fn pointer(element: &Element, kind: TokenStream) -> TokenStream {
    match element {
        // A pointer's type does not change how it is passed.
        Element::Scalar(Scalar::I8) => quote!(#kind u8),
        Element::Scalar(scalar) => {
            let ty = rust_scalar(*scalar);
            quote!(#kind #ty)
        }
        Element::Struct(_) => quote!(#kind u8),
    }
}

/// Read `len` elements into `bytes`, and in `arg` the pointer to pass: to
/// the encoded bytes, or for integers wider than a byte to an aligned copy
/// with room for `room` elements.
fn elements(
    bytes: &proc_macro2::Ident,
    arg: &proc_macro2::Ident,
    element: &Element,
    structs: &Structs,
    len: TokenStream,
    room: usize,
) -> Option<TokenStream> {
    let width = width(element, structs)?;
    Some(match element {
        Element::Scalar(scalar) if width > 1 => {
            let ty = rust_scalar(*scalar);
            let copy = format_ident!("{arg}_copy");
            quote! {
                let #bytes: &[u8] = r.take(#len * #width)?;
                let mut #copy = [0 as #ty; #room];
                for (index, value) in #copy.iter_mut().take(#len).enumerate() {
                    *value = #ty::from_le_bytes(d::element::<#width>(#bytes, index));
                }
                let #arg = #copy.as_ptr();
            }
        }
        Element::Scalar(_) => quote! {
            let #bytes: &[u8] = r.take(#len * #width)?;
            let #arg = #bytes.as_ptr();
        },
        Element::Struct(_) => quote! {
            let #bytes: &[u8] = r.take(#len * #width)?;
            let #arg = #bytes.as_ptr();
        },
    })
}

/// An output buffer for `len` elements.
fn output_buffer(
    out: &proc_macro2::Ident,
    element: &Element,
    structs: &Structs,
    len: usize,
) -> Option<TokenStream> {
    let width = width(element, structs)?;
    Some(match element {
        Element::Scalar(Scalar::U8 | Scalar::I8) => quote!(let mut #out = [0u8; #len];),
        Element::Scalar(scalar) => {
            let ty = rust_scalar(*scalar);
            quote!(let mut #out = [0 as #ty; #len];)
        }
        Element::Struct(_) => {
            let bytes = len * width;
            quote!(let mut #out = [0u8; #bytes];)
        }
    })
}

/// Put the first `len` elements of `out`.
fn put_elements(
    out: &proc_macro2::Ident,
    element: &Element,
    structs: &Structs,
    len: TokenStream,
) -> Option<TokenStream> {
    Some(match element {
        Element::Scalar(Scalar::U8 | Scalar::I8) => quote!(w.put(&#out[..#len])?;),
        Element::Scalar(_) => quote! {
            for value in &#out[..#len] {
                w.put(&value.to_le_bytes())?;
            }
        },
        Element::Struct(_) => {
            let width = width(element, structs)?;
            quote!(w.put(&#out[..#len * #width])?;)
        }
    })
}
