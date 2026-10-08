//! `#[cfg]` predicates selecting the targets a catalog entry exists on.

use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;
use stm32wb_catalog::{Catalog, Profile, ReleaseRange, Version};

/// The predicate enabling an item on every `(release, profile)` pair of the
/// catalog's platform the segments cover: the platform's feature when they
/// cover every target.
pub fn targets<'a>(
    catalog: &Catalog,
    segments: impl IntoIterator<Item = (ReleaseRange, &'a [Profile])>,
) -> TokenStream {
    let segments = segments.into_iter().collect::<Vec<_>>();
    let covered = segments
        .iter()
        .flat_map(|(releases, profiles)| {
            releases_in(catalog, *releases)
                .flat_map(move |release| profiles.iter().map(move |profile| (release, *profile)))
        })
        .collect::<BTreeSet<_>>();
    if covered.len() == catalog.targets().count() {
        let feature = catalog.platform.feature();
        return quote!(feature = #feature);
    }
    let alternatives = segments.iter().map(|(releases, profiles)| {
        let releases = releases_in(catalog, *releases)
            .map(|release| catalog.platform.release_feature(release));
        let profiles = profiles.iter().map(|profile| profile.feature_name());
        quote! {
            all(
                any(#(feature = #releases),*),
                any(#(feature = #profiles),*)
            )
        }
    });
    quote!(any(#(#alternatives),*))
}

/// `#[cfg(predicate)]`.
pub fn attr(predicate: &TokenStream) -> TokenStream {
    quote!(#[cfg(#predicate)])
}

fn releases_in(catalog: &Catalog, range: ReleaseRange) -> impl Iterator<Item = Version> + '_ {
    catalog
        .versions()
        .filter(move |release| range.contains(*release))
}
