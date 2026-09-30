//! Extract one tagged STM32CubeWBA release into a catalog snapshot.
//!
//! STM32CubeWBA ships its BLE stack as a library: the generated headers
//! declare the command functions, the event callbacks, and the packed
//! structures the library reads and writes, but no code fills or reads them.
//! So each fact comes from where ST states it:
//!
//! - commands and events, their opcodes and codes, and the profiles
//!   supporting them, from the tables of `STM32WBA_BLE_Wireless_Interface.html`;
//! - each command's completion from the "Events generated" list of its
//!   section, which `snapshot.rs` checks against every STM32WB wrapper;
//! - layouts from the `_cpN` and `_rp0` structures of `ble_types.h`, by the
//!   rule of `declared.rs`, which `snapshot.rs` checks against every layout
//!   the STM32WB code proves;
//! - an event's payload must be the members its callback in `ble_events.h`
//!   receives, in order;
//! - bearers and values from the doc comments of the header declarations, as
//!   on STM32WB, and statuses from `ble_defs.h`.
//!
//! STM32CubeWBA v1.9.0 and later pin `Middlewares/ST/STM32_WPAN` as a
//! submodule, read from its clone at the pinned commit.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use clang::{EntityKind, Index};
use stm32wb_catalog::{
    Bearer, Field, FieldType, Layout, Platform, ReleaseSource, Snapshot, SnapshotCommand,
    SnapshotEvent, Version,
};

use crate::c::{self, CMSIS_PACKING, Shim};
use crate::cube::CubeTag;
use crate::docs::{self, Key};
use crate::domains::Documented;
use crate::snapshot::{
    FieldDomains, Report, TRANSPORT_EVENTS, note_unresolved, resolved_domains, transport_event,
};
use crate::statuses;

pub const WPAN_DIR: &str = "Middlewares/ST/STM32_WPAN";
const INCLUDE_DIR: &str = "ble/stack/include";
const INTERFACE_DOCUMENT: &str = "ble/stack/doc/STM32WBA_BLE_Wireless_Interface.html";
/// Headers of `auto/` that declare no commands or events.
const TYPE_HEADERS: [&str; 3] = ["ble_types.h", "ble_raw_api.h", "ble_vs_codes.h"];
const EVENTS_HEADER: &str = "ble_events.h";
/// Declarations of the headers that are neither commands nor events:
/// sending and, before 1.4.0, receiving ACL data, and the wrapper's event
/// dispatcher.
const NOT_INTERFACE: [&str; 3] = [
    "hci_tx_acl_data",
    "hci_rx_acl_data_event",
    "BLE_EventProcess",
];

/// What a header declares about one function.
struct Declared {
    parameters: Vec<String>,
    bearers: Vec<Bearer>,
    domains: Vec<Documented>,
    /// Members whose declared size is only a maximum.
    maximum_sized: Vec<String>,
}

pub fn extract(
    index: &Index<'_>,
    shim: &Shim,
    repository: &Path,
    version: Version,
) -> Result<(Snapshot, Report), String> {
    let cube = CubeTag::open(repository, version)?;
    let (wpan, prefix) = cube.directory(WPAN_DIR)?;
    let include = format!("{prefix}{INCLUDE_DIR}");
    let tree = wpan.materialize(&[&include])?;
    let include_dir = tree.path().join(&include);

    let mut headers = wpan
        .list(&format!("{include}/auto"))?
        .into_iter()
        .filter_map(|path| path.rsplit('/').next().map(str::to_owned))
        .filter(|file| file.ends_with(".h") && !TYPE_HEADERS.contains(&file.as_str()))
        .collect::<Vec<_>>();
    headers.sort();
    if !headers.iter().any(|file| file == EVENTS_HEADER) {
        return Err(format!("{}: {EVENTS_HEADER} is missing", cube.tag));
    }

    let mut field_domains = FieldDomains::default();
    let mut records = None;
    let mut functions = BTreeMap::new();
    let mut callbacks = BTreeMap::new();
    let mut orphaned_lists = Vec::new();
    for file in &headers {
        let context = format!("{} {file}", cube.tag);
        let unit = c::parse_with(
            index,
            &include_dir.join("auto").join(file),
            shim,
            std::slice::from_ref(&include_dir),
            &[CMSIS_PACKING],
        )?;
        let unit_records = c::records(&unit);
        field_domains.add(&unit_records, &context)?;
        records.get_or_insert(unit_records);
        let declared = if file == EVENTS_HEADER {
            &mut callbacks
        } else {
            &mut functions
        };
        for function in c::main_file_entities(&unit) {
            if function.get_kind() != EntityKind::FunctionDecl {
                continue;
            }
            let name = function.get_name().unwrap_or_default();
            let parameters = function
                .get_arguments()
                .unwrap_or_default()
                .into_iter()
                .map(|parameter| parameter.get_name().unwrap_or_default())
                .collect();
            let (domains, orphans) = crate::domains::domains_and_orphans(function)
                .map_err(|error| format!("{context} {name}: {error}"))?;
            orphaned_lists.extend(orphans.into_iter().map(|orphan| (name.clone(), orphan)));
            let entry = Declared {
                parameters,
                bearers: c::bearers(function)
                    .map_err(|error| format!("{context} {name}: {error}"))?,
                domains,
                maximum_sized: c::maximum_sized(function),
            };
            if declared.insert(name.clone(), entry).is_some() {
                return Err(format!("{context}: {name} is declared twice"));
            }
        }
    }
    let records = records.ok_or_else(|| format!("{}: no headers were parsed", cube.tag))?;

    let documented = docs::interface_availability(
        &wpan,
        &format!("{prefix}{INTERFACE_DOCUMENT}"),
        Platform::Stm32wba,
    )?;
    let completions = docs::command_completions(&wpan, &format!("{prefix}{INTERFACE_DOCUMENT}"))?;
    let mut report = Report::new(version);
    report.orphaned_lists = orphaned_lists;
    let mut commands = Vec::new();
    let mut events = Vec::new();
    let mut used = BTreeSet::new();
    for (key, entry) in &documented {
        let name = entry.name.to_ascii_lowercase();
        match *key {
            Key::Command(scope, opcode) => {
                let Some(declared) = functions.get(&name) else {
                    report.undeclared.push(name);
                    continue;
                };
                used.insert(name.clone());
                let Some(Some(completion)) = completions.get(&entry.name) else {
                    report.unstated_completions.push(name);
                    continue;
                };
                report.documented_completions += 1;
                let (params, returns, structs) =
                    crate::declared::command(&name, *completion, &records);
                let params = bounded_by_maximum(&name, params, &declared.maximum_sized)?;
                let returns = returns.map(|returns| {
                    returned_in_order(&name, &params, returns, &declared.parameters)
                });
                note_unresolved(&mut report, &name, "params", &params);
                if let Some(returns) = &returns {
                    note_unresolved(&mut report, &name, "returns", returns);
                }
                let domains =
                    resolved_domains(&mut report, &name, declared.domains.clone(), |returned| {
                        if returned {
                            returns.as_ref()
                        } else {
                            Some(&params)
                        }
                    });
                let bearers = resolved_bearers(&mut report, &name, &params, &declared.bearers);
                report.commands += 1;
                commands.push(SnapshotCommand {
                    scope,
                    opcode,
                    name,
                    profiles: entry.profiles.clone(),
                    completion: *completion,
                    params,
                    returns,
                    structs,
                    bearers,
                    domains,
                });
            }
            Key::Event(scope, code) if TRANSPORT_EVENTS.contains(&(scope, code)) => {
                report.events += 1;
                events.push(transport_event(scope, code, entry));
            }
            Key::Event(scope, code) => {
                let Some(callback) = callbacks.get(&name) else {
                    report.undeclared.push(name);
                    continue;
                };
                used.insert(name.clone());
                let (mut payload, mut structs) = crate::declared::event(&name, &records);
                if let Layout::Fields(fields) = &payload
                    && fields
                        .iter()
                        .map(|field| &field.name)
                        .ne(callback.parameters.iter())
                {
                    payload = Layout::Unresolved(format!(
                        "the callback does not receive every {name}_rp0 member in order"
                    ));
                    structs.clear();
                }
                let payload = bounded_by_maximum(&name, payload, &callback.maximum_sized)?;
                if !matches!(payload, Layout::Fields(_)) {
                    structs.clear();
                }
                note_unresolved(&mut report, &name, "payload", &payload);
                let domains =
                    resolved_domains(&mut report, &name, callback.domains.clone(), |returned| {
                        (!returned).then_some(&payload)
                    })
                    .into_iter()
                    .map(|(member, _, domain)| (member, domain))
                    .collect();
                let bearers = resolved_bearers(&mut report, &name, &payload, &callback.bearers);
                report.events += 1;
                events.push(SnapshotEvent {
                    scope,
                    code,
                    name,
                    profiles: entry.profiles.clone(),
                    payload,
                    structs,
                    bearers,
                    domains,
                });
            }
        }
    }
    report.undocumented = functions
        .keys()
        .chain(callbacks.keys())
        .filter(|name| !used.contains(*name) && !NOT_INTERFACE.contains(&name.as_str()))
        .cloned()
        .collect();

    let struct_domains = field_domains.carried(&mut report, &commands, &events);
    let snapshot = Snapshot {
        source: ReleaseSource {
            version,
            tag: cube.tag.clone(),
            commit: cube.commit.clone(),
            profiles: docs::interface_profiles(
                &wpan,
                &format!("{prefix}{INTERFACE_DOCUMENT}"),
                Platform::Stm32wba,
            )?,
        },
        binaries: Vec::new(),
        commands,
        events,
        statuses: statuses::extract(&wpan, &format!("{include}/ble_defs.h"))?,
        struct_domains,
    };
    Ok((snapshot, report))
}

/// A return layout, unresolved unless the function's parameters are the
/// command's parameters followed by every `_rp0` member after the status
/// that does not echo one of them, in order: a function returning more than
/// `_rp0` declares shows the structure incomplete.
pub(crate) fn returned_in_order(
    name: &str,
    params: &Layout,
    returns: Layout,
    parameters: &[String],
) -> Layout {
    let (Layout::Fields(inputs), Layout::Fields(outputs)) = (params, &returns) else {
        return returns;
    };
    let echoed = |field: &&Field| inputs.iter().any(|input| input.name == field.name);
    let expected = inputs
        .iter()
        .chain(outputs.iter().skip(1).filter(|field| !echoed(field)))
        .map(|field| &field.name);
    if expected.ne(parameters.iter()) {
        return Layout::Unresolved(format!(
            "the function's parameters are not the {name}_cpN members followed by the \
             {name}_rp0 members after Status, in order"
        ));
    }
    returns
}

/// A layout holding a member whose declared size the documentation calls a
/// maximum, unresolved: the member's documented fields repeat, each sized by
/// its own length, so the packed structure does not give its width.
pub(crate) fn bounded_by_maximum(
    name: &str,
    layout: Layout,
    members: &[String],
) -> Result<Layout, String> {
    let Layout::Fields(fields) = &layout else {
        return Ok(layout);
    };
    let mut reasons = Vec::new();
    for member in members {
        match fields.iter().find(|field| field.name == *member) {
            Some(Field {
                ty: FieldType::Array { len, .. },
                ..
            }) => reasons.push(format!(
                "{member} declares {len} elements, which its documentation calls the maximum size"
            )),
            _ => {
                return Err(format!(
                    "{name}: {member} is documented with a maximum size but is not a fixed array"
                ));
            }
        }
    }
    if reasons.is_empty() {
        Ok(layout)
    } else {
        Ok(Layout::Unresolved(reasons.join("; ")))
    }
}

/// The bearers of a layout, reported where it is unresolved.
fn resolved_bearers(
    report: &mut Report,
    name: &str,
    layout: &Layout,
    bearers: &[Bearer],
) -> Vec<Bearer> {
    if let Layout::Unresolved(_) = layout {
        report.deferred_bearers.extend(
            bearers
                .iter()
                .map(|bearer| (name.to_owned(), bearer.member.clone())),
        );
    }
    bearers.to_vec()
}
