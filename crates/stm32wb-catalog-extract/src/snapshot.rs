//! Extract one tagged release into a catalog snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use clang::Index;
use stm32wb_catalog::{
    CommandScope, Domain, DomainKind, EventScope, Layout, Platform, Profile, ReleaseSource,
    Snapshot, SnapshotCommand, SnapshotEvent, Structs, Version,
};

use crate::c::{self, Shim};
use crate::commands::{self, ExtractedCommand};
use crate::cube::{BLE_CORE_DIR, CubeTag, INTERFACE_DOCUMENT, SHCI_DIR};
use crate::docs::{self, Key};
use crate::events::{self, ExtractedEvent};
use crate::{shci, statuses};

/// HCI Command Complete and Command Status.
pub const TRANSPORT_EVENTS: [(EventScope, u16); 2] =
    [(EventScope::Standard, 0x0E), (EventScope::Standard, 0x0F)];

/// Summary of one release's extraction.
pub struct Report {
    pub version: Version,
    pub commands: usize,
    pub events: usize,
    pub proven_counts: usize,
    /// Layouts the code proves that the structures alone reproduce, and
    /// `(entry, reason)` for the commands and events they do not.
    pub declared_agreements: usize,
    pub declared_differences: Vec<(String, String)>,
    /// Command completions the interface document states, all matching the
    /// code, and the commands whose completion it does not state.
    pub documented_completions: usize,
    pub unstated_completions: Vec<String>,
    /// `(entry, layout, reason)` for every layout left unresolved.
    pub unresolved: Vec<(String, &'static str, String)>,
    /// Documented values in total, and `(entry, member)` for every list
    /// dropped because its layout is unresolved.
    pub domains: usize,
    pub dropped_domains: Vec<(String, String)>,
    /// `(entry, member)` for every `Flags:` list dropped because an item is
    /// not a single bit, such as one listing packed subfields.
    pub dropped_flags: Vec<(String, String)>,
    /// `(structure, field)` for every structure field list dropped because
    /// no resolved layout carries the structure.
    pub dropped_field_domains: Vec<(String, String)>,
    /// `(entry, member)` for every bearer dropped because its layout is
    /// unresolved.
    pub dropped_bearers: Vec<(String, String)>,
}

impl Report {
    pub fn new(version: Version) -> Self {
        Self {
            version,
            commands: 0,
            events: 0,
            proven_counts: 0,
            declared_agreements: 0,
            declared_differences: Vec::new(),
            documented_completions: 0,
            unstated_completions: Vec::new(),
            unresolved: Vec::new(),
            domains: 0,
            dropped_domains: Vec::new(),
            dropped_flags: Vec::new(),
            dropped_field_domains: Vec::new(),
            dropped_bearers: Vec::new(),
        }
    }
}

/// The documented values of structure fields, gathered from every file
/// parsed, which must agree.
#[derive(Default)]
pub struct FieldDomains(BTreeMap<(String, String), Domain>);

impl FieldDomains {
    pub fn add(
        &mut self,
        records: &BTreeMap<String, c::CRecord>,
        context: &str,
    ) -> Result<(), String> {
        let documented = crate::domains::field_domains(records)
            .map_err(|error| format!("{context}: {error}"))?;
        for (structure, member, domain) in documented {
            let key = (structure, member);
            match self.0.get(&key) {
                Some(other) if *other != domain => {
                    return Err(format!(
                        "{context}: {}.{} is documented differently elsewhere",
                        key.0, key.1
                    ));
                }
                Some(_) => {}
                None => {
                    self.0.insert(key, domain);
                }
            }
        }
        Ok(())
    }

    /// The lists of the structures a resolved layout carries, which belong
    /// to the catalog; the others are reported.
    pub fn carried(
        self,
        report: &mut Report,
        commands: &[SnapshotCommand],
        events: &[SnapshotEvent],
    ) -> Vec<(String, String, Domain)> {
        let carried = commands
            .iter()
            .map(|command| &command.structs)
            .chain(events.iter().map(|event| &event.structs))
            .flat_map(|structs| structs.keys())
            .collect::<BTreeSet<_>>();
        let mut struct_domains = Vec::new();
        for ((structure, member), domain) in self.0 {
            if carried.contains(&structure) {
                report.domains += 1;
                struct_domains.push((structure, member, domain));
            } else {
                report.dropped_field_domains.push((structure, member));
            }
        }
        struct_domains
    }
}

/// Command Complete or Command Status, which the HCI transport consumes
/// rather than a generated process function.
pub fn transport_event(scope: EventScope, code: u16, entry: &docs::Documented) -> SnapshotEvent {
    SnapshotEvent {
        scope,
        code,
        name: entry.name.to_ascii_lowercase(),
        profiles: entry.profiles.clone(),
        payload: Layout::Unresolved(
            "consumed by the HCI transport layer, which declares no payload structure".to_owned(),
        ),
        structs: Default::default(),
        bearers: Vec::new(),
        domains: Vec::new(),
    }
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
    let mut field_domains = FieldDomains::default();
    let mut document_fields = |records: &BTreeMap<String, c::CRecord>, file: &str| {
        field_domains.add(records, &format!("{} {file}", tag.tag))
    };

    let mut extracted_commands: Vec<ExtractedCommand> = Vec::new();
    let (mut declared_agreements, mut declared_differences) = (0, Vec::new());
    for file in &sources {
        let scope = if file == "ble_hci_le.c" {
            CommandScope::Standard
        } else {
            CommandScope::Vendor
        };
        let unit = c::parse(index, &core.join("auto").join(file), shim, &core_includes)?;
        let records = c::records(&unit);
        document_fields(&records, file)?;
        let extracted = commands::extract(&unit, &records, scope)
            .map_err(|error| format!("{} {file}: {error}", tag.tag))?;
        for command in &extracted {
            match compare_declared(command, &records)
                .map_err(|error| format!("{} {file}: {error}", tag.tag))?
            {
                Ok(sides) => declared_agreements += sides,
                Err(difference) => declared_differences.push((command.name.clone(), difference)),
            }
        }
        extracted_commands.extend(extracted);
    }

    let unit = c::parse(index, &core.join("auto/ble_events.c"), shim, &core_includes)?;
    let records = c::records(&unit);
    document_fields(&records, "ble_events.c")?;
    let mut extracted_events: Vec<ExtractedEvent> = events::extract(&unit, &records)
        .map_err(|error| format!("{} ble_events.c: {error}", tag.tag))?;
    for event in &extracted_events {
        match compare_declared_event(event, &records)
            .map_err(|error| format!("{} ble_events.c: {error}", tag.tag))?
        {
            Ok(sides) => declared_agreements += sides,
            Err(difference) => declared_differences.push((event.name.clone(), difference)),
        }
    }

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

    let documented = docs::interface_availability(&tag, INTERFACE_DOCUMENT, Platform::Stm32wb)?;
    let completions = docs::command_completions(&tag, INTERFACE_DOCUMENT)?;
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
        commands: extracted_commands.len(),
        events: extracted_events.len(),
        proven_counts,
        declared_agreements,
        declared_differences,
        ..Report::new(version)
    };
    let mut commands = Vec::new();
    for command in extracted_commands {
        let profiles = if command.scope == CommandScope::System {
            // Like the system events, the BLE interface document does not
            // list the system channel's commands.
            Platform::Stm32wb.profiles().to_vec()
        } else {
            let key = Key::Command(command.scope, command.opcode);
            let profiles = availability(key.clone(), &command.name)?;
            // The document states the completion STM32WBA's commands must
            // take from it, so it must agree with every STM32WB wrapper.
            match completions.get(&documented[&key].name) {
                Some(Some(completion)) if *completion == command.completion => {
                    report.documented_completions += 1;
                }
                Some(Some(completion)) => {
                    return Err(format!(
                        "{}: {} completes with {:?} but its documentation lists {completion:?}",
                        tag.tag, command.name, command.completion
                    ));
                }
                Some(None) | None => report.unstated_completions.push(command.name.clone()),
            }
            profiles
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
            Platform::Stm32wb.profiles().to_vec()
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
            events.push(transport_event(scope, code, entry));
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

    let struct_domains = field_domains.carried(&mut report, &commands, &events);

    let snapshot = Snapshot {
        source: ReleaseSource {
            version,
            tag: tag.tag.clone(),
            commit: tag.commit.clone(),
        },
        binaries: docs::binaries(&tag)?,
        commands,
        events,
        statuses: statuses::extract(&tag, &format!("{BLE_CORE_DIR}/ble_defs.h"))?,
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
pub fn resolved_domains<'a>(
    report: &mut Report,
    name: &str,
    domains: Vec<crate::domains::Documented>,
    layout: impl Fn(bool) -> Option<&'a Layout>,
) -> Vec<crate::domains::Documented> {
    domains
        .into_iter()
        .filter(|(member, returned, domain)| match layout(*returned) {
            Some(Layout::Fields(_)) if !single_bits(domain) => {
                report.dropped_flags.push((name.to_owned(), member.clone()));
                false
            }
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

/// Whether a `Flags:` list lists only single bits or a named zero, as values
/// lists always do.
fn single_bits(domain: &Domain) -> bool {
    domain.kind != DomainKind::Flags
        || domain
            .items
            .iter()
            .all(|item| item.first == item.last && item.first & (item.first - 1) == 0)
}

/// Compare the layouts the code of a command proves with those its
/// structures alone give, returning how many sides agree, or why the
/// structures do not reproduce them. A contradiction is an error: the rule of
/// `declared.rs` would misread the commands of a platform without code.
pub fn compare_declared(
    command: &ExtractedCommand,
    records: &BTreeMap<String, c::CRecord>,
) -> Result<Result<usize, String>, String> {
    let (params, returns, structs) =
        crate::declared::command(&command.name, command.completion, records);
    let contradiction = |what: &str, detail: String| {
        if command.byte_counted.is_empty() {
            Err(format!(
                "{}: its {what} contradict the rule of ST's structures: {detail}",
                command.name
            ))
        } else {
            // The wrapper disagrees with its own structures, which place what
            // follows as if the count were in elements.
            Ok(Err(format!(
                "its code counts {} in bytes where its structures declare wider elements",
                command.byte_counted.join(", ")
            )))
        }
    };
    let mut sides = 0;
    let (mut proven, mut derived) = (Structs::new(), Structs::new());
    for (what, code, declared) in [
        ("params", Some(&command.params), Some(&params)),
        ("returns", command.returns.as_ref(), returns.as_ref()),
    ] {
        let Some(Layout::Fields(code)) = code else {
            continue;
        };
        match declared {
            Some(Layout::Fields(declared)) if declared == code => {
                commands::collect_structs(code, &command.structs, &mut proven);
                commands::collect_structs(declared, &structs, &mut derived);
                sides += 1;
            }
            Some(Layout::Fields(declared)) => {
                return contradiction(what, format!("code {code:?}, structures {declared:?}"));
            }
            Some(Layout::Unresolved(reason)) => {
                return Ok(Err(format!(
                    "its structures leave its {what} unresolved: {reason}"
                )));
            }
            None => return contradiction(what, "the structures give none".to_owned()),
        }
    }
    if proven != derived {
        return contradiction(
            "structures",
            format!("code {proven:?}, structures {derived:?}"),
        );
    }
    Ok(Ok(sides))
}

/// Compare the payload the code of an event proves with the one its
/// structure alone gives: 1 if they agree, 0 if neither resolves it. The rule
/// resolving a payload the code decodes procedurally is an error, as it would
/// misread that event where no code exists.
pub fn compare_declared_event(
    event: &ExtractedEvent,
    records: &BTreeMap<String, c::CRecord>,
) -> Result<Result<usize, String>, String> {
    let (payload, structs) = crate::declared::event(&event.name, records);
    match (&event.payload, &payload) {
        (Layout::Fields(code), Layout::Fields(declared))
            if code == declared && event.structs == structs =>
        {
            Ok(Ok(1))
        }
        (Layout::Unresolved(_), Layout::Unresolved(_)) => Ok(Ok(0)),
        (Layout::Fields(_), Layout::Unresolved(reason)) => Ok(Err(format!(
            "its structure leaves its payload unresolved: {reason}"
        ))),
        (code, declared) => Err(format!(
            "{}: its payload contradicts the rule of ST's structures: code {code:?} with \
             {:?}, structures {declared:?} with {structs:?}",
            event.name, event.structs
        )),
    }
}

pub fn note_unresolved(report: &mut Report, name: &str, what: &'static str, layout: &Layout) {
    if let Layout::Unresolved(reason) = layout {
        report
            .unresolved
            .push((name.to_owned(), what, reason.clone()));
    }
}
