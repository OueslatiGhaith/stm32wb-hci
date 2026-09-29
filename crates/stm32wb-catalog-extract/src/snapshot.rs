//! Extract one tagged release into a catalog snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use clang::Index;
use stm32wb_catalog::{
    CommandScope, Domain, EventScope, Layout, Profile, ReleaseSource, Snapshot, SnapshotCommand,
    SnapshotEvent, Version,
};

use crate::c::{self, Shim};
use crate::commands::{self, ExtractedCommand};
use crate::cube::{BLE_CORE_DIR, CubeTag, SHCI_DIR};
use crate::docs::{self, Key};
use crate::events::{self, ExtractedEvent};
use crate::{shci, statuses};

/// HCI Command Complete and Command Status.
const TRANSPORT_EVENTS: [(EventScope, u16); 2] =
    [(EventScope::Standard, 0x0E), (EventScope::Standard, 0x0F)];

/// Summary of one release's extraction.
pub struct Report {
    pub version: Version,
    pub commands: usize,
    pub events: usize,
    pub proven_counts: usize,
    /// `(entry, layout, reason)` for every layout left unresolved.
    pub unresolved: Vec<(String, &'static str, String)>,
    /// Documented values in total, and `(entry, member)` for every list
    /// dropped because its layout is unresolved.
    pub domains: usize,
    pub dropped_domains: Vec<(String, String)>,
    /// `(structure, field)` for every structure field list dropped because
    /// no resolved layout carries the structure.
    pub dropped_field_domains: Vec<(String, String)>,
}

pub fn extract(
    index: &Index<'_>,
    shim: &Shim,
    repository: &Path,
    version: Version,
) -> Result<(Snapshot, Report), String> {
    let tag = CubeTag::open(repository, version)?;
    let tree = tag.materialize(&[BLE_CORE_DIR, SHCI_DIR])?;
    let core = tree.path().join(BLE_CORE_DIR);
    let core_includes = [core.clone(), core.join("template")];

    let mut sources = tag
        .list(&format!("{BLE_CORE_DIR}/auto"))?
        .into_iter()
        .filter_map(|path| path.rsplit('/').next().map(str::to_owned))
        .filter(|file| file.ends_with(".c") && file != "ble_events.c")
        .collect::<Vec<_>>();
    sources.sort();
    if !sources.iter().any(|file| file == "ble_hci_le.c") {
        return Err(format!("{}: ble_hci_le.c is missing", tag.tag));
    }

    // Structure fields documented in every file parsed, which must agree.
    let mut field_domains: BTreeMap<(String, String), Domain> = BTreeMap::new();
    let mut document_fields =
        |records: &BTreeMap<String, c::CRecord>, file: &str| -> Result<(), String> {
            let documented = crate::domains::field_domains(records)
                .map_err(|error| format!("{} {file}: {error}", tag.tag))?;
            for (structure, member, domain) in documented {
                let key = (structure, member);
                match field_domains.get(&key) {
                    Some(other) if *other != domain => {
                        return Err(format!(
                            "{} {file}: {}.{} is documented differently elsewhere",
                            tag.tag, key.0, key.1
                        ));
                    }
                    Some(_) => {}
                    None => {
                        field_domains.insert(key, domain);
                    }
                }
            }
            Ok(())
        };

    let mut extracted_commands: Vec<ExtractedCommand> = Vec::new();
    for file in &sources {
        let scope = if file == "ble_hci_le.c" {
            CommandScope::Standard
        } else {
            CommandScope::Vendor
        };
        let unit = c::parse(index, &core.join("auto").join(file), shim, &core_includes)?;
        let records = c::records(&unit);
        document_fields(&records, file)?;
        extracted_commands.extend(
            commands::extract(&unit, &records, scope)
                .map_err(|error| format!("{} {file}: {error}", tag.tag))?,
        );
    }

    let unit = c::parse(index, &core.join("auto/ble_events.c"), shim, &core_includes)?;
    let records = c::records(&unit);
    document_fields(&records, "ble_events.c")?;
    let mut extracted_events: Vec<ExtractedEvent> = events::extract(&unit, &records)
        .map_err(|error| format!("{} ble_events.c: {error}", tag.tag))?;

    let shci_dir = tree.path().join(SHCI_DIR);
    let unit = c::parse(
        index,
        &shci_dir.join("shci/shci.h"),
        shim,
        &[shci_dir.join("shci"), shci_dir.join("tl")],
    )?;
    let records = c::records(&unit);
    document_fields(&records, "shci.h")?;
    extracted_events.extend(
        shci::extract(&unit, &records).map_err(|error| format!("{} shci.h: {error}", tag.tag))?,
    );

    let unit = c::parse(
        index,
        &shci_dir.join("shci/shci.c"),
        shim,
        &[shci_dir.join("shci"), shci_dir.join("tl")],
    )?;
    let records = c::records(&unit);
    document_fields(&records, "shci.c")?;
    extracted_commands.extend(
        shci::commands(&unit, &records).map_err(|error| format!("{} shci.c: {error}", tag.tag))?,
    );

    // ST's generator counts every variable buffer with the member right
    // before it. Events rely on that rule; commands prove it wherever their
    // code relates a count to a buffer, so any counterexample is fatal.
    let mut proven_counts = 0;
    for command in &extracted_commands {
        for (count, buffer, adjacent) in &command.proven_counts {
            proven_counts += 1;
            if !adjacent {
                return Err(format!(
                    "{} {}: {buffer} is counted by {count}, which does not immediately precede it; \
                     the event count convention no longer holds",
                    tag.tag, command.name
                ));
            }
        }
    }

    let documented = docs::interface_availability(&tag)?;
    let mut matched = BTreeSet::new();
    let mut availability = |key: Key, name: &str| -> Result<Vec<Profile>, String> {
        let entry = documented
            .get(&key)
            .ok_or_else(|| format!("{}: {name} ({key:?}) has no availability row", tag.tag))?;
        if !docs::same_name(name, &entry.name) {
            return Err(format!(
                "{}: {key:?} is {name} in C but {} in the interface document",
                tag.tag, entry.name
            ));
        }
        matched.insert(key);
        Ok(entry.profiles.clone())
    };

    let mut report = Report {
        version,
        commands: extracted_commands.len(),
        events: extracted_events.len(),
        proven_counts,
        unresolved: Vec::new(),
        domains: 0,
        dropped_domains: Vec::new(),
        dropped_field_domains: Vec::new(),
    };
    let mut commands = Vec::new();
    for command in extracted_commands {
        let profiles = if command.scope == CommandScope::System {
            // Like the system events, the BLE interface document does not
            // list the system channel's commands.
            Profile::ALL.to_vec()
        } else {
            availability(Key::Command(command.scope, command.opcode), &command.name)?
        };
        note_unresolved(&mut report, &command.name, "params", &command.params);
        if let Some(returns) = &command.returns {
            note_unresolved(&mut report, &command.name, "returns", returns);
        }
        let domains = resolved_domains(&mut report, &command.name, command.domains, |returned| {
            if returned {
                command.returns.as_ref()
            } else {
                Some(&command.params)
            }
        });
        commands.push(SnapshotCommand {
            scope: command.scope,
            opcode: command.opcode,
            name: command.name,
            profiles,
            completion: command.completion,
            params: command.params,
            returns: command.returns,
            structs: command.structs,
            bearers: command.bearers,
            domains,
        });
    }
    let mut events = Vec::new();
    for event in extracted_events {
        let profiles = if event.scope == EventScope::System {
            // SHCI is the CPU2 system channel of every wireless binary; the
            // BLE interface document does not list it.
            Profile::ALL.to_vec()
        } else {
            availability(Key::Event(event.scope, event.code), &event.name)?
        };
        note_unresolved(&mut report, &event.name, "payload", &event.payload);
        let domains = resolved_domains(&mut report, &event.name, event.domains, |returned| {
            (!returned).then_some(&event.payload)
        })
        .into_iter()
        .map(|(member, _, domain)| (member, domain))
        .collect();
        events.push(SnapshotEvent {
            scope: event.scope,
            code: event.code,
            name: event.name,
            profiles,
            payload: event.payload,
            structs: event.structs,
            bearers: event.bearers,
            domains,
        });
    }
    // The transport layer (`hci_tl.c`), not the event tables, consumes the
    // command completion events; they carry no generated payload structure.
    for (key, entry) in &documented {
        if let Key::Event(scope, code) = *key
            && !matched.contains(key)
            && TRANSPORT_EVENTS.contains(&(scope, code))
        {
            matched.insert(key.clone());
            events.push(SnapshotEvent {
                scope,
                code,
                name: entry.name.to_ascii_lowercase(),
                profiles: entry.profiles.clone(),
                payload: Layout::Unresolved(
                    "consumed by the HCI transport layer, which declares no payload structure"
                        .to_owned(),
                ),
                structs: Default::default(),
                bearers: Vec::new(),
                domains: Vec::new(),
            });
        }
    }
    let undocumented = documented
        .iter()
        .filter(|(key, _)| !matched.contains(*key))
        .map(|(_, entry)| entry.name.clone())
        .collect::<Vec<_>>();
    if !undocumented.is_empty() {
        return Err(format!(
            "{}: the interface document lists entries the generated C does not define: {}",
            tag.tag,
            undocumented.join(", ")
        ));
    }

    // A structure's field lists belong to the catalog where a resolved
    // layout carries the structure; the others are reported.
    let carried = commands
        .iter()
        .map(|command| &command.structs)
        .chain(events.iter().map(|event| &event.structs))
        .flat_map(|structs| structs.keys())
        .collect::<BTreeSet<_>>();
    let mut struct_domains = Vec::new();
    for ((structure, member), domain) in field_domains {
        if carried.contains(&structure) {
            report.domains += 1;
            struct_domains.push((structure, member, domain));
        } else {
            report.dropped_field_domains.push((structure, member));
        }
    }

    let snapshot = Snapshot {
        source: ReleaseSource {
            version,
            tag: tag.tag.clone(),
            commit: tag.commit.clone(),
        },
        binaries: docs::binaries(&tag)?,
        commands,
        events,
        statuses: statuses::extract(&tag)?,
        struct_domains,
    };
    if snapshot.binaries.is_empty() {
        return Err(format!("{}: no BLE wireless binaries were found", tag.tag));
    }
    Ok((snapshot, report))
}

/// The documented lists whose side has a resolved layout; the others have no
/// member to belong to and are reported. A list for a side the entry does not
/// have is an error.
fn resolved_domains<'a>(
    report: &mut Report,
    name: &str,
    domains: Vec<crate::domains::Documented>,
    layout: impl Fn(bool) -> Option<&'a Layout>,
) -> Vec<crate::domains::Documented> {
    domains
        .into_iter()
        .filter(|(member, returned, _)| match layout(*returned) {
            Some(Layout::Fields(_)) => {
                report.domains += 1;
                true
            }
            _ => {
                report
                    .dropped_domains
                    .push((name.to_owned(), member.clone()));
                false
            }
        })
        .collect()
}

fn note_unresolved(report: &mut Report, name: &str, what: &'static str, layout: &Layout) {
    if let Layout::Unresolved(reason) = layout {
        report
            .unresolved
            .push((name.to_owned(), what, reason.clone()));
    }
}
