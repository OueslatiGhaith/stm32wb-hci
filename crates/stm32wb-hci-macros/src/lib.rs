//! Procedural macros that derive `stm32wb-hci` declarations from the bundled
//! STM32WB catalog, so the crate cannot drift from what the CPU2 wireless
//! binaries accept and emit.

mod bearer;
mod cfg;
mod command;
mod complete;
mod dispatch;
mod event;
mod standard;
mod structs;

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
/// `#[wire(since = "<release>")]`, the first release that has it, and a field
/// it removes with `#[wire(before = "<release>")]`, the first release without
/// it. Each set of fields that exist together becomes its own declaration,
/// compiled only for the releases that have exactly those fields.
///
/// A `u16` member the catalog documents as addressing an ATT bearer is
/// declared `AttBearer`, and no other member is; its enhanced range must be
/// the selected release's. A member that only addresses a bearer from a
/// later release is declared twice, as `ConnHandle` with `before` and as
/// `AttBearer` with `since`.
///
/// A member that is a C structure, or an array of them, is declared with the
/// type [`vendor_struct!`] declares for that structure: `T`, `[T; N]`,
/// `&'a [T]` for a counted parameter, or `BoundedArray<T, CAPACITY>` for a
/// counted return parameter.
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
    command::expand(input, command::Channel::Vendor)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare an ST system (SHCI) command from its C function name.
///
/// The catalog checks the fields as for [`vendor_command!`], `since` and
/// `before` included, and the command's return parameters after the status
/// are declared after `->`. The command is not a bt-hci command: it wraps its
/// parameters and implements `SystemCommand`, which the system channel's
/// transport sends it with.
///
/// ```ignore
/// system_command! {
///     /// Read the state of the firmware upgrade service.
///     SHCI_C2_FUS_GetState => FusGetState {} -> FusState {
///         error_code: u8,
///     }
/// }
/// ```
#[proc_macro]
pub fn system_command(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as command::Input);
    command::expand(input, command::Channel::System)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare the Rust type standing for one of the catalog's C structures.
///
/// Fields must match the structure's members in order, as for
/// [`vendor_command!`], in every definition of the structure. The type exists
/// on the targets where a command or event uses the structure, and implements
/// `WriteHci`, `FromHciBytes`, `HciWireType`, and `CatalogStruct`, which
/// commands check their structure members against.
///
/// ```ignore
/// vendor_struct! {
///     /// A peer device address.
///     Peer_Entry_t => PeerEntry {
///         peer_address_type: u8,
///         peer_address: BdAddr,
///     }
/// }
/// ```
#[proc_macro]
pub fn vendor_struct(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as structs::Input);
    structs::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare an ST vendor event from its generated C name.
///
/// The vendor event code and the targets the event exists on come from the
/// catalog; the declaration names and types the parameters, which must match
/// the catalog's members as for [`vendor_command!`], `since` and `before`
/// included. The event becomes a plain struct implementing `FromHciBytes`
/// for the parameters after the code, and `VendorEvent`, which decodes a
/// bt-hci vendor event carrying the code.
///
/// A byte parameter counted by another member leaves the count undeclared and
/// is declared `&'a [u8]`, borrowing the event; a list of structures is
/// declared `Elements<'a, T>`, with `T` the type [`vendor_struct!`] declares,
/// and decodes each element as it is read. The struct then takes that
/// lifetime.
///
/// ```ignore
/// vendor_event! {
///     /// A warning from the wireless stack, with data depending on its type.
///     aci_warning_event => HalWarningEvent {
///         warning_type: u8,
///         data: &'a [u8],
///     }
/// }
/// ```
#[proc_macro]
pub fn vendor_event(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as event::Input);
    event::expand(input, command::Channel::Vendor)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare an ST system (SHCI) event from its generated C name.
///
/// The catalog checks the parameters as for [`vendor_event!`]. The event
/// becomes a plain struct implementing `FromHciBytes` for the parameters
/// after its sub-event code, and `SystemEvent`, which decodes a system event
/// carrying the code.
///
/// ```ignore
/// system_event! {
///     /// The wireless CPU is about to write to flash.
///     SHCI_SUB_EVT_NVM_START_WRITE => NvmStartWriteEvent {
///         number_of_words: u32,
///     }
/// }
/// ```
#[proc_macro]
pub fn system_event(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as event::Input);
    event::expand(input, command::Channel::System)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Require a declaration of every vendor and system command and event the
/// catalog lists for the selected target.
///
/// Invoked once, in `stm32wb_hci::catalog`. It defines a marker type for each
/// catalog entry and the `Declared` trait, which [`vendor_command!`],
/// [`system_command!`], [`vendor_event!`], and [`system_event!`] implement
/// for the markers of the entries they declare, on the targets they declare
/// them for. A target whose binary implements an
/// entry nothing declares fails to compile, naming the entry.
#[proc_macro]
pub fn catalog_complete(input: TokenStream) -> TokenStream {
    if !input.is_empty() {
        return error("catalog_complete! takes no arguments");
    }
    match bundled()
        .map_err(|error| error.to_string())
        .and_then(|bundled| complete::expand(bundled).map_err(|error| error.to_string()))
    {
        Ok(tokens) => tokens.into(),
        Err(message) => error(&format!("the bundled catalog is invalid: {message}")),
    }
}

/// Check bt-hci's types for the Bluetooth Core commands against the catalog,
/// and mark each `Supported` on the targets whose binary implements it.
///
/// Each entry names a catalog command, by any name it had, and the bt-hci
/// type sending it, by its path in `bt_hci::cmd`. On every target the command exists on, the type's opcode
/// must be the catalog's, it must be a `SyncCmd` for a Command Complete
/// command or an `AsyncCmd` for a Command Status one, and its parameters and
/// return parameters after the status must be `FixedSizeValue`s of the
/// catalog's width wherever the catalog's width is fixed.
///
/// ```ignore
/// standard_commands! {
///     hci_reset => controller_baseband::Reset,
///     hci_le_set_extended_advertising_data => le::LeSetExtAdvData<'a>,
/// }
/// ```
#[proc_macro]
pub fn standard_commands(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as standard::Input);
    standard::expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// The last enhanced ATT bearer the selected release documents, as
/// `LAST_ENHANCED` constants.
///
/// Invoked once, in `stm32wb_hci::wire::AttBearer`'s `impl` block. Every
/// parameter the catalog lists as addressing an ATT bearer must give the same
/// range in a release.
#[proc_macro]
pub fn att_bearer_range(input: TokenStream) -> TokenStream {
    if !input.is_empty() {
        return error("att_bearer_range! takes no arguments");
    }
    match bundled()
        .map_err(|error| format!("the bundled catalog is invalid: {error}"))
        .and_then(bearer::expand)
    {
        Ok(tokens) => tokens.into(),
        Err(message) => error(&message),
    }
}

/// Declare an enum of every ST vendor event the selected target emits.
///
/// Each variant names a catalog event, the variant, and the type
/// [`vendor_event!`] declares for the event. A variant exists on the targets
/// the event does, the enum must list every vendor event of the catalog
/// exactly once, and each type must be the one declared for its event.
/// `from_vendor` decodes a bt-hci vendor event into the variant its code
/// selects.
///
/// ```ignore
/// vendor_events! {
///     /// Every ST vendor event of the selected target.
///     pub enum AciEvent<'a> {
///         aci_warning_event => HalWarning(hal::HalWarningEvent<'a>),
///         // ...
///     }
/// }
/// ```
#[proc_macro]
pub fn vendor_events(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as dispatch::Input);
    dispatch::expand(input, command::Channel::Vendor)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declare an enum of every ST system (SHCI) event the selected target emits.
///
/// The variants are checked as for [`vendor_events!`], against the types
/// [`system_event!`] declares. `from_system_params` decodes a system event,
/// sub-event code first, into the variant its code selects.
///
/// ```ignore
/// system_events! {
///     /// Every ST system event of the selected target.
///     pub enum ShciEvent<'a> {
///         SHCI_SUB_EVT_CODE_READY => Ready(ReadyEvent),
///         // ...
///     }
/// }
/// ```
#[proc_macro]
pub fn system_events(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as dispatch::Input);
    dispatch::expand(input, command::Channel::System)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn error(message: &str) -> TokenStream {
    let message = format!("stm32wb-hci-macros: {message}");
    quote::quote_spanned!(Span::call_site() => ::core::compile_error!(#message);).into()
}
