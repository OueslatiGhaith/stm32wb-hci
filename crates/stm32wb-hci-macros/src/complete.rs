//! `catalog_complete!`: every vendor, system, and Bluetooth Core command and
//! event the selected target's wireless binary implements has a declaration.
//!
//! Each catalog entry gets a marker type. A declaration implements `Declared`
//! for the markers of the entries it covers, on the targets it covers, and
//! `catalog_complete!` requires the implementation on every target the
//! catalog lists the entry for. A missing declaration fails to compile with
//! the entry's name; a duplicate one conflicts with the first.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use stm32wb_catalog::{Bundled, CommandScope, Error, EventScope};

use crate::cfg;
use crate::command::Targets;

/// The kind of entry a declaration covers.
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Command,
    Event,
}

/// Targets of each catalog entry carrying `name`, keyed by the entry's
/// latest name, which its marker is named after. Excluded releases need no
/// declaration.
pub(crate) fn entries<'a>(
    bundled: &'a Bundled,
    kind: Kind,
    name: &str,
) -> Result<BTreeMap<&'a str, Targets<'a>>, Error> {
    let mut entries: BTreeMap<&str, Vec<_>> = BTreeMap::new();
    match kind {
        Kind::Command => {
            for segment in bundled.command_segments(name)? {
                if segment.entry.excluded.is_none() {
                    entries
                        .entry(segment.entry.command.name())
                        .or_default()
                        .push((segment.releases, segment.profiles));
                }
            }
        }
        Kind::Event => {
            for segment in bundled.event_segments(name)? {
                if segment.entry.excluded.is_none() {
                    entries
                        .entry(segment.entry.event.name())
                        .or_default()
                        .push((segment.releases, segment.profiles));
                }
            }
        }
    }
    Ok(entries)
}

/// Mark the entries the declaration of `name` covers as declared.
pub(crate) fn declared(bundled: &Bundled, kind: Kind, name: &str) -> Result<TokenStream, Error> {
    Ok(entries(bundled, kind, name)?
        .into_iter()
        .map(|(entry, targets)| {
            let marker = format_ident!("{entry}");
            let cfg = cfg::attr(&cfg::targets(&bundled.catalog, targets));
            quote! {
                #cfg
                impl ::stm32wb_hci::catalog::Declared for ::stm32wb_hci::catalog::#marker {}
            }
        })
        .collect())
}

/// The markers and checks of every platform's catalog, each check gated to
/// the targets of its own platform.
pub fn expand(catalogs: &[&Bundled]) -> Result<TokenStream, Error> {
    let mut markers = BTreeSet::new();
    let mut checks = TokenStream::new();
    for bundled in catalogs {
        let (entries, tokens) = platform_checks(bundled)?;
        markers.extend(entries);
        checks.extend(tokens);
    }
    let markers = markers.iter().map(|entry| format_ident!("{entry}"));
    Ok(quote! {
        /// Implemented by the marker of each catalog entry a declaration
        /// covers, on the targets it covers.
        #[diagnostic::on_unimplemented(
            message = "the catalog lists `{Self}` for the selected target, but nothing declares it",
            label = "no vendor_command!, system_command!, standard_command!, standard_commands! entry, vendor_event!, system_event!, standard_event!, or standard_events! entry declares `{Self}` for this target",
            note = "declare it in the module of its group, or check the since/before bounds of an existing declaration"
        )]
        pub trait Declared {}

        #(
            #[allow(non_camel_case_types, missing_docs)]
            pub struct #markers;
        )*

        const fn declared<T: Declared>() {}

        #checks
    })
}

/// The entries one platform's catalog requires a declaration of, and the
/// checks requiring them on its targets.
fn platform_checks(bundled: &Bundled) -> Result<(Vec<&str>, TokenStream), Error> {
    let catalog = &bundled.catalog;
    let mut required: BTreeMap<&str, (Kind, Targets<'_>)> = BTreeMap::new();
    let commands = catalog
        .commands
        .iter()
        .filter(|command| {
            matches!(
                command.scope,
                CommandScope::Vendor | CommandScope::System | CommandScope::Standard
            )
        })
        .map(|command| (Kind::Command, command.name()));
    let events = catalog
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.scope,
                EventScope::Vendor | EventScope::System | EventScope::Standard | EventScope::LeMeta
            )
        })
        .map(|event| (Kind::Event, event.name()));
    for (kind, name) in commands.chain(events) {
        if required.contains_key(name) {
            continue;
        }
        for (entry, targets) in entries(bundled, kind, name)? {
            required
                .entry(entry)
                .or_insert((kind, Vec::new()))
                .1
                .extend(targets);
        }
    }

    let checks = required.iter().map(|(entry, (kind, targets))| {
        let marker = format_ident!("{entry}");
        let cfg = cfg::attr(&cfg::targets(catalog, targets.iter().copied()));
        let doc = match kind {
            Kind::Command => {
                format!(
                    "Requires a `vendor_command!`, `system_command!`, `standard_command!`, or \
                     `standard_commands!` entry declaring `{entry}`."
                )
            }
            Kind::Event => {
                format!(
                    "Requires a `vendor_event!`, `system_event!`, `standard_event!`, or \
                     `standard_events!` entry declaring `{entry}`."
                )
            }
        };
        quote! {
            #cfg
            #[doc = #doc]
            const _: () = declared::<#marker>();
        }
    });
    Ok((required.keys().copied().collect(), quote!(#(#checks)*)))
}

#[cfg(test)]
mod tests {
    use stm32wb_catalog::{Platform, bundled};

    use super::*;

    #[test]
    fn declarations_mark_their_entries_by_latest_name() {
        let bundled = bundled(Platform::Stm32wb).unwrap();
        let tokens = declared(bundled, Kind::Command, "aci_hal_set_slave_latency")
            .unwrap()
            .to_string();
        assert!(
            tokens.contains("catalog :: aci_hal_set_peripheral_latency {"),
            "{tokens}"
        );

        // The HAL events moved to another code in 1.24.0: one marker covers
        // both entries.
        let tokens = declared(bundled, Kind::Event, "aci_hal_end_of_radio_activity_event")
            .unwrap()
            .to_string();
        assert_eq!(tokens.matches("impl").count(), 1, "{tokens}");
    }

    #[test]
    fn every_vendor_and_system_entry_is_required() {
        let catalogs = Platform::ALL.map(|platform| bundled(platform).unwrap());
        let tokens = expand(&catalogs).unwrap().to_string();
        assert!(tokens.contains("pub struct aci_reset ;"), "{tokens}");
        assert!(tokens.contains("pub struct hci_reset ;"), "{tokens}");
        assert!(
            tokens.contains("pub struct aci_gatt_eatt_bearer_event ;"),
            "{tokens}"
        );
        assert!(
            tokens.contains("pub struct hci_le_connection_complete_event ;"),
            "{tokens}"
        );
        assert!(
            tokens.contains("pub struct SHCI_SUB_EVT_CODE_READY ;"),
            "{tokens}"
        );
        assert!(!tokens.contains("SHCI_SUB_EVT_THREAD"), "{tokens}");
        assert!(
            tokens.contains("declared :: < aci_gatt_notification_complete_event > ()"),
            "{tokens}"
        );
        // STM32WBA's own entries are required on its targets only.
        assert!(
            tokens.contains("pub struct hci_le_create_cis ;"),
            "{tokens}"
        );
        assert!(
            tokens.contains("feature = \"wba_1_10_0\"")
                || tokens.contains("feature = \"_stm32wba\""),
            "{tokens}"
        );
        assert_eq!(tokens.matches("pub trait Declared").count(), 1);
    }
}
