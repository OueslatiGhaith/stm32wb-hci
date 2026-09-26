//! Procedural macros that derive `stm32wb-hci` declarations from the bundled
//! STM32WB catalog, so the crate cannot drift from what the CPU2 wireless
//! binaries accept and emit.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use stm32wb_catalog::{Profile, bundled};

/// Require exactly one `fw_*` feature (a catalog release) and exactly one
/// `stack-*` feature (a BLE stack profile).
///
/// The features themselves are declared in `stm32wb-hci`'s manifest, which a
/// test keeps in sync with the catalog; this macro only counts the enabled
/// ones, so its lists always come from the catalog.
#[proc_macro]
pub fn check_target(input: TokenStream) -> TokenStream {
    if !input.is_empty() {
        return error("check_target! takes no arguments");
    }
    let catalog = match bundled() {
        Ok(bundled) => &bundled.catalog,
        Err(error) => return self::error(&format!("the bundled catalog is invalid: {error}")),
    };
    let releases = catalog.versions().map(|release| release.feature_name());
    let profiles = Profile::ALL.map(Profile::feature_name);
    let release_message = format!(
        "enable exactly one fw_* feature, naming the STM32CubeWB release of the CPU2 wireless binary ({})",
        catalog
            .versions()
            .map(|release| release.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let profile_message = format!(
        "enable exactly one stack-* feature, naming the BLE stack profile of the CPU2 wireless binary ({})",
        Profile::ALL.map(Profile::name).join(", ")
    );
    quote! {
        const _: () = {
            let releases = 0 #(+ cfg!(feature = #releases) as usize)*;
            ::core::assert!(releases == 1, #release_message);
            let profiles = 0 #(+ cfg!(feature = #profiles) as usize)*;
            ::core::assert!(profiles == 1, #profile_message);
        };
    }
    .into()
}

fn error(message: &str) -> TokenStream {
    let message = format!("stm32wb-hci-macros: {message}");
    quote::quote_spanned!(Span::call_site() => ::core::compile_error!(#message);).into()
}
