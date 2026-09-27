//! Procedural macros that derive `stm32wb-hci` declarations from the bundled
//! STM32WB catalog, so the crate cannot drift from what the CPU2 wireless
//! binaries accept and emit.

mod cfg;
mod command;

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

/// Declare an ST vendor command from its generated C name.
///
/// The opcode, the completion kind, and the targets the command exists on
/// come from the catalog; the declaration names and types the fields. Fields
/// must match the catalog's members in order, spelled in snake case or mapped
/// with `#[wire(name = "<member>")]`, and each type's `HciWireType::WIDTH`
/// must equal the member's encoded width.
///
/// A Command Complete command that returns more than its status declares the
/// rest after `->`; bt-hci checks the status itself. The return parameters
/// become a plain struct decoded in catalog order.
///
/// A byte field counted by another member leaves the count undeclared: as a
/// parameter it is declared `&'a [u8]`, its count is written from its length,
/// and the command is built with `try_new`, which rejects fields over the
/// catalog's capacity; as a return parameter it is declared
/// `BoundedBytes<CAPACITY>` and decoded after its count.
///
/// A union parameter leaves its selector undeclared as well: it is declared
/// with a type implementing `HciWireUnion` whose alternatives are exactly the
/// catalog's, such as `Uuid`, and the selector is written from the value's
/// alternative.
///
/// A field the catalog adds in a later release is marked with
/// `#[wire(since = "<release>")]`, the first release that has it. Each set of
/// fields that exist together becomes its own declaration, compiled only for
/// the releases that have exactly those fields.
///
/// ```ignore
/// vendor_command! {
///     /// Set the radio activity events to report.
///     aci_hal_set_radio_activity_mask => HalSetRadioActivityMask {
///         radio_activity_mask: u16,
///     }
/// }
///
/// vendor_command! {
///     /// Read the low-level configuration data at `offset`.
///     aci_hal_read_config_data => HalReadConfigData { offset: u8 } -> HalConfigData {
///         data: BoundedBytes<250>,
///     }
/// }
///
/// vendor_command! {
///     /// Read the current anchor period and the largest free slot.
///     aci_hal_get_anchor_period => HalGetAnchorPeriod {} -> HalAnchorPeriod {
///         anchor_period: u32,
///         max_free_slot: u32,
///     }
/// }
/// ```
#[proc_macro]
pub fn vendor_command(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as command::Input);
    command::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn error(message: &str) -> TokenStream {
    let message = format!("stm32wb-hci-macros: {message}");
    quote::quote_spanned!(Span::call_site() => ::core::compile_error!(#message);).into()
}
