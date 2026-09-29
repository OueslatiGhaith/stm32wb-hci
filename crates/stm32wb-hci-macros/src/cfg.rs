//! `#[cfg]` predicates selecting the targets a catalog entry exists on.

use std::collections::BTreeSet;

use proc_macro2::TokenStream;
use quote::quote;
use stm32wb_catalog::{Catalog, Profile, ReleaseRange, Version};

/// The predicate enabling an item on every `(release, profile)` pair the
/// segments cover, or `None` when they cover every target.
pub fn targets<'a>(
    catalog: &Catalog,
    segments: impl IntoIterator<Item = (ReleaseRange, &'a [Profile])>,
) -> Option<TokenStream> {
    let segments = segments.into_iter().collect::<Vec<_>>();
    let covered = segments
        .iter()
        .flat_map(|(releases, profiles)| {
            releases_in(catalog, *releases)
                .flat_map(move |release| profiles.iter().map(move |profile| (release, *profile)))
        })
        .collect::<BTreeSet<_>>();
    if covered.len() == catalog.releases.len() * catalog.platform.profiles().len() {
        return None;
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
    Some(quote!(any(#(#alternatives),*)))
}

fn releases_in(catalog: &Catalog, range: ReleaseRange) -> impl Iterator<Item = Version> + '_ {
    catalog
        .versions()
        .filter(move |release| range.contains(*release))
}
