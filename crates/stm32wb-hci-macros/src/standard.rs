//! `standard_commands!` and `standard_events!`: the bt-hci types of the
//! Bluetooth Core commands and events the catalog lists, checked against it.
//! Commands are marked `Supported` on the targets implementing them.

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use stm32wb_catalog::{Bundled, CommandScope, Completion, EventScope, bundled};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Ident, Lifetime, Token, Type};

use crate::command::Targets;
use crate::dispatch::with_static_lifetimes;
use crate::{cfg, complete};

/// Types are paths in `bt_hci::cmd` for commands, and in `bt_hci::event` for
/// events:
///
/// ```text
/// hci_reset => controller_baseband::Reset,
/// hci_le_set_extended_advertising_data => le::LeSetExtAdvData<'a>,
/// ```
pub struct Input {
    entries: Vec<Entry>,
}

struct Entry {
    c_name: Ident,
    /// The type, relative to its bt-hci module.
    ty: Type,
}

impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let entries = Punctuated::<Entry, Token![,]>::parse_terminated(input)?
            .into_iter()
            .collect();
        Ok(Self { entries })
    }
}

impl Parse for Entry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let c_name = input.parse()?;
        input.parse::<Token![=>]>()?;
        let ty = input.parse()?;
        Ok(Self { c_name, ty })
    }
}

/// `ty`, a path relative to the bt-hci module `module`, as an absolute path.
fn resolve(ty: &Type, module: TokenStream, example: &str) -> syn::Result<Type> {
    match ty {
        Type::Path(path) if path.qself.is_none() && path.path.leading_colon.is_none() => {
            let relative = &path.path;
            Ok(syn::parse_quote!(::bt_hci::#module::#relative))
        }
        _ => Err(syn::Error::new(
            ty.span(),
            format!("name the bt-hci type by its path in bt_hci::{module}, such as `{example}`"),
        )),
    }
}

/// The lifetimes `ty` mentions, which its `Supported` impl is generic over.
fn lifetimes(ty: &Type) -> Vec<Lifetime> {
    let tokens = quote!(#ty).into_iter().collect::<Vec<_>>();
    let mut lifetimes = Vec::new();
    for pair in tokens.windows(2) {
        if let [
            proc_macro2::TokenTree::Punct(punct),
            proc_macro2::TokenTree::Ident(ident),
        ] = pair
            && punct.as_char() == '\''
        {
            let lifetime = Lifetime::new(&format!("'{ident}"), ident.span());
            if !lifetimes.contains(&lifetime) {
                lifetimes.push(lifetime);
            }
        }
    }
    lifetimes
}

/// What the catalog says a command's bt-hci type must be, identical in every
/// release and profile of one group.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Facts {
    opcode: u16,
    completion: Completion,
    /// The parameters' width, if fixed.
    params: Option<usize>,
    /// The return parameters' width after the status, if fixed.
    returns: Option<usize>,
}

pub fn expand(input: Input) -> syn::Result<TokenStream> {
    let bundled = bundled().map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("the bundled catalog is invalid: {error}"),
        )
    })?;
    let mut declared = BTreeSet::new();
    let mut tokens = TokenStream::new();
    for entry in &input.entries {
        tokens.extend(expand_entry(bundled, entry, &mut declared)?);
    }
    Ok(tokens)
}

pub fn expand_events(input: Input) -> syn::Result<TokenStream> {
    let bundled = bundled().map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!("the bundled catalog is invalid: {error}"),
        )
    })?;
    let mut declared = BTreeSet::new();
    let mut tokens = TokenStream::new();
    for entry in &input.entries {
        tokens.extend(expand_event(bundled, entry, &mut declared)?);
    }
    Ok(tokens)
}

/// Check an event type's code at compile time and, where the catalog's
/// width is fixed, generate a test that bt-hci decodes exactly that width.
fn expand_event(
    bundled: &Bundled,
    entry: &Entry,
    declared: &mut BTreeSet<String>,
) -> syn::Result<TokenStream> {
    let c_name = entry.c_name.to_string();
    let error = |message: String| syn::Error::new(entry.c_name.span(), message);
    let ty = resolve(&entry.ty, quote!(event), "le::LeConnectionComplete")?;
    let segments = bundled
        .event_segments(&c_name)
        .map_err(|failure| error(failure.to_string()))?;
    let latest = segments
        .last()
        .map(|segment| segment.entry.event.name())
        .ok_or_else(|| error(format!("{c_name} exists in no release")))?;
    if !declared.insert(latest.to_owned()) {
        return Err(error(format!("{latest} is listed twice")));
    }

    // Each distinct scope, code, and fixed width, with the targets having it.
    type Key = (EventScope, u16, Option<usize>);
    let mut groups: Vec<(Key, Targets<'_>)> = Vec::new();
    for segment in &segments {
        let active = &segment.entry;
        let releases = segment.releases;
        if !matches!(
            active.event.scope,
            EventScope::Standard | EventScope::LeMeta
        ) {
            return Err(error(format!("{c_name} is not a Bluetooth Core event")));
        }
        if let Some(exclusion) = active.excluded {
            return Err(error(format!(
                "{c_name} is excluded in {releases}: {}",
                exclusion.reason
            )));
        }
        // A layout the extractor left unresolved has no width to test.
        let width = match active.payload.fields {
            Ok(fields) => {
                let envelope = stm32wb_catalog::Layout::Fields(fields.to_vec())
                    .envelope(active.payload.structs)
                    .map_err(|failure| error(failure.to_string()))?
                    .expect("a resolved layout has an envelope");
                (envelope.min == envelope.max).then_some(envelope.max)
            }
            Err(_) => None,
        };
        let key = (active.event.scope, active.event.code, width);
        let target = (releases, segment.profiles);
        match groups.iter_mut().find(|(other, _)| *other == key) {
            Some((_, targets)) => targets.push(target),
            None => groups.push((key, vec![target])),
        }
    }

    let catalog = &bundled.catalog;
    let checked = with_static_lifetimes(&ty);
    let shown = quote!(#ty).to_string().replace(' ', "");
    let mut tokens = TokenStream::new();
    for (index, ((scope, code, width), targets)) in groups.into_iter().enumerate() {
        let predicate = cfg::targets(catalog, targets.iter().copied());
        let cfg = predicate
            .as_ref()
            .map(|predicate| quote!(#[cfg(#predicate)]));
        let code = u8::try_from(code).map_err(|_| {
            error(format!(
                "{latest} has code 0x{code:04X}, wider than bt-hci's"
            ))
        })?;
        let (code_of, what) = match scope {
            EventScope::LeMeta => (
                quote!(<#checked as ::bt_hci::event::le::LeEventParams<'static>>::SUBEVENT_CODE),
                "LE meta subevent code",
            ),
            _ => (
                quote!(<#checked as ::bt_hci::event::EventParams<'static>>::EVENT_CODE),
                "event code",
            ),
        };
        let message = format!("{latest} is {what} 0x{code:02X}, but `{shown}` has another");
        tokens.extend(quote_spanned! {ty.span()=>
            #cfg
            const _: () = ::core::assert!(#code_of == #code, #message);
        });
        if let Some(width) = width {
            let test = Ident::new(
                &format!(
                    "{}_decodes_the_catalog_width_{index}",
                    latest.to_lowercase()
                ),
                entry.c_name.span(),
            );
            let test_cfg = match &predicate {
                Some(predicate) => quote!(#[cfg(all(test, #predicate))]),
                None => quote!(#[cfg(test)]),
            };
            tokens.extend(quote! {
                #test_cfg
                #[test]
                #[allow(non_snake_case)]
                fn #test() {
                    ::stm32wb_hci::wire::assert_event_width::<#checked>(#width);
                }
            });
        }
    }
    tokens.extend(
        complete::declared(bundled, complete::Kind::Event, &c_name)
            .map_err(|failure| error(failure.to_string()))?,
    );
    Ok(tokens)
}

fn expand_entry(
    bundled: &Bundled,
    entry: &Entry,
    declared: &mut BTreeSet<String>,
) -> syn::Result<TokenStream> {
    let c_name = entry.c_name.to_string();
    let error = |message: String| syn::Error::new(entry.c_name.span(), message);
    let ty = resolve(&entry.ty, quote!(cmd), "le::LeSetEventMask")?;
    let segments = bundled
        .command_segments(&c_name)
        .map_err(|failure| error(failure.to_string()))?;
    let latest = segments
        .last()
        .map(|segment| segment.entry.command.name())
        .ok_or_else(|| error(format!("{c_name} exists in no release")))?;
    if !declared.insert(latest.to_owned()) {
        return Err(error(format!("{latest} is listed twice")));
    }

    let mut groups: Vec<(Facts, Targets<'_>)> = Vec::new();
    let mut all: Targets<'_> = Vec::new();
    for segment in &segments {
        let active = &segment.entry;
        let releases = segment.releases;
        if active.command.scope != CommandScope::Standard {
            return Err(error(format!("{c_name} is not a Bluetooth Core command")));
        }
        if let Some(exclusion) = active.excluded {
            return Err(error(format!(
                "{c_name} is excluded in {releases}: {}",
                exclusion.reason
            )));
        }
        let fixed = |layout: &stm32wb_catalog::ResolvedLayout<'_>, what: &str| {
            let fields = layout.fields.map_err(|reason| {
                error(format!(
                    "{c_name} has no derivable {what} layout in {releases}: {reason}"
                ))
            })?;
            let envelope = stm32wb_catalog::Layout::Fields(fields.to_vec())
                .envelope(layout.structs)
                .map_err(|failure| error(failure.to_string()))?
                .expect("a resolved layout has an envelope");
            Ok::<_, syn::Error>((envelope.min == envelope.max).then_some(envelope.max))
        };
        let facts = Facts {
            opcode: active.command.opcode,
            completion: active.definition.completion,
            params: fixed(&active.params, "parameter")?,
            // The status is bt-hci's to consume.
            returns: match &active.returns {
                Some(returns) => fixed(returns, "return")?.map(|width| width - 1),
                None => None,
            },
        };
        let target = (releases, segment.profiles);
        all.push(target);
        match groups.iter_mut().find(|(other, _)| *other == facts) {
            Some((_, targets)) => targets.push(target),
            None => groups.push((facts, vec![target])),
        }
    }

    let catalog = &bundled.catalog;
    let ty = &ty;
    let checked = with_static_lifetimes(ty);
    let lifetimes = lifetimes(ty);
    let shown = quote!(#ty).to_string().replace(' ', "");
    let generics = (!lifetimes.is_empty()).then(|| quote!(<#(#lifetimes),*>));
    let supported_cfg =
        cfg::targets(catalog, all.iter().copied()).map(|predicate| quote!(#[cfg(#predicate)]));
    let mut tokens = quote! {
        #supported_cfg
        impl #generics ::stm32wb_hci::catalog::Supported for #ty {}
    };
    for (facts, targets) in groups {
        let cfg = cfg::targets(catalog, targets.iter().copied())
            .map(|predicate| quote!(#[cfg(#predicate)]));
        let mut releases = targets
            .iter()
            .map(|(releases, _)| releases.to_string())
            .collect::<Vec<_>>();
        releases.dedup();
        let releases = releases.join(", ");
        let opcode = facts.opcode;
        let message =
            format!("{latest} is opcode 0x{opcode:04X} in {releases}, but `{shown}` has another");
        tokens.extend(quote_spanned! {ty.span()=>
            #cfg
            const _: () = ::core::assert!(
                <#checked as ::bt_hci::cmd::Cmd>::OPCODE.to_raw() == #opcode,
                #message
            );
        });
        let completion = match facts.completion {
            Completion::CommandComplete => quote!(completes_with_command_complete),
            Completion::CommandStatus => quote!(completes_with_command_status),
        };
        tokens.extend(quote_spanned! {ty.span()=>
            #cfg
            const _: () = ::stm32wb_hci::wire::#completion::<#checked>();
        });
        if let Some(width) = facts.params {
            let message = format!(
                "{latest}'s parameters are {width} bytes in {releases}, but `{shown}` encodes \
                 another width"
            );
            tokens.extend(quote_spanned! {ty.span()=>
                #cfg
                const _: () = ::core::assert!(
                    ::stm32wb_hci::wire::fixed_width::<
                        <#checked as ::bt_hci::cmd::Cmd>::Params
                    >() == #width,
                    #message
                );
            });
        }
        if let (Completion::CommandComplete, Some(width)) = (facts.completion, facts.returns) {
            let message = format!(
                "{latest} returns {width} bytes after its status in {releases}, but `{shown}` \
                 decodes another width"
            );
            tokens.extend(quote_spanned! {ty.span()=>
                #cfg
                const _: () = ::core::assert!(
                    ::stm32wb_hci::wire::fixed_width::<
                        <#checked as ::bt_hci::cmd::SyncCmd>::Return
                    >() == #width,
                    #message
                );
            });
        }
    }
    tokens.extend(
        complete::declared(bundled, complete::Kind::Command, &c_name)
            .map_err(|failure| error(failure.to_string()))?,
    );
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_str(source: &str) -> Result<String, String> {
        let input = syn::parse_str::<Input>(source).map_err(|error| error.to_string())?;
        expand(input)
            .map(|tokens| tokens.to_string())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn commands_are_checked_against_the_catalog() {
        let tokens = expand_str("hci_reset => controller_baseband::Reset").unwrap();
        assert!(
            tokens.contains("Supported for :: bt_hci :: cmd :: controller_baseband :: Reset"),
            "{tokens}"
        );
        assert!(tokens.contains("to_raw () == 3075u16"), "{tokens}");
        assert!(
            tokens.contains("completes_with_command_complete"),
            "{tokens}"
        );
        // No parameters, and nothing after the status.
        assert_eq!(tokens.matches("== 0usize").count(), 2, "{tokens}");

        let tokens = expand_str("hci_le_create_connection => le::LeCreateConn").unwrap();
        assert!(tokens.contains("completes_with_command_status"), "{tokens}");
        assert!(tokens.contains("== 25usize"), "{tokens}");
    }

    #[test]
    fn borrowed_commands_are_generic_and_variable_widths_unchecked() {
        let tokens =
            expand_str("hci_le_set_extended_advertising_data => le::LeSetExtAdvData<'a>").unwrap();
        assert!(
            tokens.contains("impl < 'a > :: stm32wb_hci :: catalog :: Supported"),
            "{tokens}"
        );
        assert!(
            tokens.contains("le :: LeSetExtAdvData < 'static >"),
            "{tokens}"
        );
        assert!(!tokens.contains("Cmd > :: Params"), "{tokens}");
    }

    #[test]
    fn entries_must_be_standard_and_listed_once() {
        let error =
            expand_str("hci_reset => ::bt_hci::cmd::controller_baseband::Reset").unwrap_err();
        assert!(error.contains("by its path in bt_hci::cmd"), "{error}");
        let error = expand_str("aci_hal_get_fw_build_number => Build").unwrap_err();
        assert!(error.contains("is not a Bluetooth Core command"), "{error}");
        let error = expand_str(
            "hci_le_read_local_supported_features => le::LeReadLocalSupportedFeatures,
             hci_le_read_local_supported_features_page_0 => le::LeReadLocalSupportedFeatures",
        )
        .unwrap_err();
        assert!(
            error.contains("hci_le_read_local_supported_features_page_0 is listed twice"),
            "{error}"
        );
    }

    /// Renamed in 1.23.0, one entry covers both names.
    #[test]
    fn renamed_commands_keep_their_history() {
        let tokens =
            expand_str("hci_le_read_local_supported_features => le::LeReadLocalSupportedFeatures")
                .unwrap();
        assert_eq!(tokens.matches("to_raw ()").count(), 1, "{tokens}");
    }

    fn expand_events_str(source: &str) -> Result<String, String> {
        let input = syn::parse_str::<Input>(source).map_err(|error| error.to_string())?;
        expand_events(input)
            .map(|tokens| tokens.to_string())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn events_check_their_code_and_test_their_width() {
        let tokens =
            expand_events_str("hci_disconnection_complete_event => DisconnectionComplete").unwrap();
        assert!(
            tokens.contains("EventParams < 'static >> :: EVENT_CODE == 5u8"),
            "{tokens}"
        );
        assert!(tokens.contains("assert_event_width"), "{tokens}");
        assert!(tokens.contains("(4usize)"), "{tokens}");
        assert!(
            tokens.contains(
                "Declared for :: stm32wb_hci :: catalog :: hci_disconnection_complete_event"
            ),
            "{tokens}"
        );

        let tokens =
            expand_events_str("hci_le_connection_complete_event => le::LeConnectionComplete")
                .unwrap();
        assert!(tokens.contains("SUBEVENT_CODE == 1u8"), "{tokens}");

        // Unresolved layouts, and variable ones, have only their code checked.
        let tokens =
            expand_events_str("hci_le_advertising_report_event => le::LeAdvertisingReport<'a>")
                .unwrap();
        assert!(tokens.contains("SUBEVENT_CODE == 2u8"), "{tokens}");
        assert!(!tokens.contains("assert_event_width"), "{tokens}");

        let error = expand_events_str("aci_warning_event => Vendor<'a>").unwrap_err();
        assert!(error.contains("is not a Bluetooth Core event"), "{error}");
        let error = expand_events_str("hci_reset => Reset").unwrap_err();
        assert!(error.contains("no event"), "{error}");
    }
}
