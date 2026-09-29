//! The check that a type stands for exactly the status codes the BLE stack
//! defines besides success, on every release.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use stm32wb_catalog::{Platform, ReleaseRange, Version, bundled};
use syn::spanned::Spanned;

use crate::cfg;
use crate::command::release_runs;

pub fn expand(ty: syn::Type) -> syn::Result<TokenStream> {
    let bundled = bundled(Platform::Stm32wb).map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("the bundled catalog is invalid: {error}"),
        )
    })?;
    let catalog = &bundled.catalog;
    let mut groups: Vec<(Vec<u8>, Vec<Version>)> = Vec::new();
    for release in catalog.versions() {
        let values = catalog
            .statuses_at(release)
            .into_iter()
            .map(|status| status.value)
            .filter(|value| *value != 0)
            .collect::<Vec<_>>();
        if values.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("the catalog defines no status codes in {release}"),
            ));
        }
        match groups.iter_mut().find(|(other, _)| *other == values) {
            Some((_, releases)) => releases.push(release),
            None => groups.push((values, vec![release])),
        }
    }
    let assertions = groups.into_iter().map(|(values, releases)| {
        let listed = values
            .iter()
            .map(|value| format!("{value:#04x}"))
            .collect::<Vec<_>>()
            .join(", ");
        let named = release_runs(catalog, releases.iter().copied().map(ReleaseRange::single));
        let message = format!(
            "{}: the catalog defines the status codes {listed} in {named}; the declared type must stand for exactly them",
            quote!(#ty)
        );
        let predicate = cfg::targets(
            catalog,
            releases
                .iter()
                .map(|release| (ReleaseRange::single(*release), catalog.platform.profiles())),
        )
        .map(|predicate| quote!(#[cfg(#predicate)]));
        let values = values.iter().map(|value| i64::from(*value));
        quote_spanned! {ty.span()=>
            #predicate
            const _: () = ::core::assert!(
                ::stm32wb_hci::wire::values_exactly::<#ty>(&[#(#values),*]),
                #message
            );
        }
    });
    Ok(quote!(#(#assertions)*))
}
