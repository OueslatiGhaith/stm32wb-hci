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
    Bearer, Layout, Platform, ReleaseSource, Snapshot, SnapshotCommand, SnapshotEvent, Version,
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
/// sending ACL data, and the wrapper's event dispatcher.
const NOT_INTERFACE: [&str; 2] = ["hci_tx_acl_data", "BLE_EventProcess"];

/// What a header declares about one function.
struct Declared {
    parameters: Vec<String>,
    bearers: Vec<Bearer>,
    domains: Vec<Documented>,
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
            let entry = Declared {
                parameters,
                bearers: c::bearers(function)
                    .map_err(|error| format!("{context} {name}: {error}"))?,
                domains: crate::domains::domains(function)
                    .map_err(|error| format!("{context} {name}: {error}"))?,
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
    let mut commands = Vec::new();
    let mut events = Vec::new();
    let mut used = BTreeSet::new();
    for (key, entry) in &documented {
        let name = entry.name.to_ascii_lowercase();
        match *key {
            Key::Command(scope, opcode) => {
                let declared = functions.get(&name).ok_or_else(|| {
                    format!(
                        "{}: the interface document lists {}, which no header declares",
                        cube.tag, entry.name
                    )
                })?;
                used.insert(name.clone());
                let Some(Some(completion)) = completions.get(&entry.name) else {
                    report.unstated_completions.push(name);
                    continue;
                };
                report.documented_completions += 1;
                let (params, returns, structs) =
                    crate::declared::command(&name, *completion, &records);
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
                let callback = callbacks.get(&name).ok_or_else(|| {
                    format!(
                        "{}: the interface document lists {}, which {EVENTS_HEADER} does not declare",
                        cube.tag, entry.name
                    )
                })?;
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
    let undocumented = functions
        .keys()
        .chain(callbacks.keys())
        .filter(|name| !used.contains(*name) && !NOT_INTERFACE.contains(&name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !undocumented.is_empty() {
        return Err(format!(
            "{}: the headers declare commands or events the interface document does not list: {}",
            cube.tag,
            undocumented.join(", ")
        ));
    }

    let struct_domains = field_domains.carried(&mut report, &commands, &events);
    let snapshot = Snapshot {
        source: ReleaseSource {
            version,
            tag: cube.tag.clone(),
            commit: cube.commit.clone(),
        },
        binaries: Vec::new(),
        commands,
        events,
        statuses: statuses::extract(&wpan, &format!("{include}/ble_defs.h"))?,
        struct_domains,
    };
    Ok((snapshot, report))
}

/// The bearers of a layout, or none, reported, where it is unresolved.
fn resolved_bearers(
    report: &mut Report,
    name: &str,
    layout: &Layout,
    bearers: &[Bearer],
) -> Vec<Bearer> {
    if let Layout::Fields(_) = layout {
        return bearers.to_vec();
    }
    report.dropped_bearers.extend(
        bearers
            .iter()
            .map(|bearer| (name.to_owned(), bearer.member.clone())),
    );
    Vec::new()
}
