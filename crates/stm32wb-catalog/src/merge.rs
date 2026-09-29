//! Merge per-release extraction snapshots into release-ranged histories.

use std::collections::BTreeMap;

use crate::{
    Availability, Bearer, Binary, Catalog, Command, CommandDefinition, CommandScope, Completion,
    Domain, Error, Event, EventDefinition, EventScope, Family, Layout, MemberDomain, Named,
    Platform, Profile, ReleaseRange, ReleaseSource, StatusCode, StructDomain, Structs, Version,
};

/// Everything extracted from one tagged release.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub source: ReleaseSource,
    pub binaries: Vec<SnapshotBinary>,
    pub commands: Vec<SnapshotCommand>,
    pub events: Vec<SnapshotEvent>,
    /// The `BLE_STATUS_*` codes, by name.
    pub statuses: Vec<(String, u8)>,
    /// Documented values of structure fields, by structure and field.
    pub struct_domains: Vec<(String, String, Domain)>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SnapshotBinary {
    pub family: Family,
    pub profile: Profile,
}

#[derive(Clone, Debug)]
pub struct SnapshotCommand {
    pub scope: CommandScope,
    pub opcode: u16,
    pub name: String,
    pub profiles: Vec<Profile>,
    pub completion: Completion,
    pub params: Layout,
    pub returns: Option<Layout>,
    pub structs: Structs,
    pub bearers: Vec<Bearer>,
    /// Documented values by member, and whether it is a return parameter.
    pub domains: Vec<(String, bool, Domain)>,
}

#[derive(Clone, Debug)]
pub struct SnapshotEvent {
    pub scope: EventScope,
    pub code: u16,
    pub name: String,
    pub profiles: Vec<Profile>,
    pub payload: Layout,
    pub structs: Structs,
    pub bearers: Vec<Bearer>,
    /// Documented values by member.
    pub domains: Vec<(String, Domain)>,
}

/// Coalesce snapshots into one catalog. Identical facts in consecutive
/// releases become one range; a change, gap, or reappearance starts another.
pub fn merge_snapshots(platform: Platform, mut snapshots: Vec<Snapshot>) -> Result<Catalog, Error> {
    snapshots.sort_by_key(|snapshot| snapshot.source.version);
    let versions = snapshots
        .iter()
        .map(|snapshot| snapshot.source.version)
        .collect::<Vec<_>>();

    let mut binaries: BTreeMap<SnapshotBinary, Vec<(usize, ())>> = BTreeMap::new();
    let mut commands: BTreeMap<(CommandScope, u16), Vec<(usize, SnapshotCommand)>> =
        BTreeMap::new();
    let mut events: BTreeMap<(EventScope, u16), Vec<(usize, SnapshotEvent)>> = BTreeMap::new();
    let mut statuses: BTreeMap<(u8, String), Vec<(usize, ())>> = BTreeMap::new();
    let mut struct_domains: BTreeMap<(String, String), Vec<(usize, Domain)>> = BTreeMap::new();

    for (index, snapshot) in snapshots.iter().enumerate() {
        for binary in &snapshot.binaries {
            let entries = binaries.entry(*binary).or_default();
            if entries.last().is_some_and(|(last, ())| *last == index) {
                return Err(Error::invalid(format!(
                    "{}: duplicate binary {} {}",
                    snapshot.source.version, binary.family, binary.profile
                )));
            }
            entries.push((index, ()));
        }
        for command in &snapshot.commands {
            let entries = commands.entry((command.scope, command.opcode)).or_default();
            if entries.last().is_some_and(|(last, _)| *last == index) {
                return Err(Error::invalid(format!(
                    "{}: duplicate {:?} command 0x{:04X}",
                    snapshot.source.version, command.scope, command.opcode
                )));
            }
            let mut command = command.clone();
            command.profiles.sort();
            command.profiles.dedup();
            entries.push((index, command));
        }
        for (name, value) in &snapshot.statuses {
            statuses
                .entry((*value, name.clone()))
                .or_default()
                .push((index, ()));
        }
        for (structure, member, domain) in &snapshot.struct_domains {
            let entries = struct_domains
                .entry((structure.clone(), member.clone()))
                .or_default();
            if entries.last().is_some_and(|(last, _)| *last == index) {
                return Err(Error::invalid(format!(
                    "{}: {structure}.{member} is documented twice",
                    snapshot.source.version
                )));
            }
            entries.push((index, domain.clone()));
        }
        for event in &snapshot.events {
            let entries = events.entry((event.scope, event.code)).or_default();
            if entries.last().is_some_and(|(last, _)| *last == index) {
                return Err(Error::invalid(format!(
                    "{}: duplicate {:?} event 0x{:04X}",
                    snapshot.source.version, event.scope, event.code
                )));
            }
            let mut event = event.clone();
            event.profiles.sort();
            event.profiles.dedup();
            entries.push((index, event));
        }
    }

    let catalog = Catalog {
        platform,
        releases: snapshots
            .iter()
            .map(|snapshot| snapshot.source.clone())
            .collect(),
        binaries: binaries
            .into_iter()
            .flat_map(|(binary, entries)| {
                runs(&entries, &versions)
                    .into_iter()
                    .map(move |(releases, ())| Binary {
                        family: binary.family,
                        profile: binary.profile,
                        // A profile without binaries fails validation.
                        file: binary
                            .family
                            .binary_file_name(binary.profile)
                            .unwrap_or_default(),
                        releases,
                    })
            })
            .collect(),
        commands: commands
            .into_iter()
            .map(|((scope, opcode), entries)| Command {
                scope,
                opcode,
                names: named(&entries, &versions, |command| command.name.clone()),
                availability: available(&entries, &versions, |command| command.profiles.clone()),
                definitions: runs(
                    &project(&entries, |command| {
                        (
                            command.completion,
                            command.params.clone(),
                            command.returns.clone(),
                            command.structs.clone(),
                            command.bearers.clone(),
                        )
                    }),
                    &versions,
                )
                .into_iter()
                .map(
                    |(releases, (completion, params, returns, structs, bearers))| {
                        CommandDefinition {
                            releases,
                            completion,
                            params,
                            returns,
                            structs,
                            bearers,
                        }
                    },
                )
                .collect(),
                domains: domains(&entries, &versions, |command| command.domains.clone()),
            })
            .collect(),
        events: events
            .into_iter()
            .map(|((scope, code), entries)| Event {
                scope,
                code,
                names: named(&entries, &versions, |event| event.name.clone()),
                availability: available(&entries, &versions, |event| event.profiles.clone()),
                definitions: runs(
                    &project(&entries, |event| {
                        (
                            event.payload.clone(),
                            event.structs.clone(),
                            event.bearers.clone(),
                        )
                    }),
                    &versions,
                )
                .into_iter()
                .map(|(releases, (payload, structs, bearers))| EventDefinition {
                    releases,
                    payload,
                    structs,
                    bearers,
                })
                .collect(),
                domains: domains(&entries, &versions, |event| {
                    event
                        .domains
                        .iter()
                        .map(|(member, domain)| (member.clone(), false, domain.clone()))
                        .collect()
                }),
            })
            .collect(),
        struct_domains: struct_domains
            .into_iter()
            .flat_map(|((structure, member), entries)| {
                runs(&entries, &versions)
                    .into_iter()
                    .map(move |(releases, domain)| StructDomain {
                        releases,
                        structure: structure.clone(),
                        member: member.clone(),
                        domain,
                    })
            })
            .collect(),
        statuses: statuses
            .into_iter()
            .flat_map(|((value, name), entries)| {
                runs(&entries, &versions)
                    .into_iter()
                    .map(move |(releases, ())| StatusCode {
                        releases,
                        name: name.clone(),
                        value,
                    })
            })
            .collect(),
    };
    catalog.validate()?;
    Ok(catalog)
}

fn project<T, U>(entries: &[(usize, T)], value: impl Fn(&T) -> U) -> Vec<(usize, U)> {
    entries
        .iter()
        .map(|(index, entry)| (*index, value(entry)))
        .collect()
}

fn named<T>(
    entries: &[(usize, T)],
    versions: &[Version],
    name: impl Fn(&T) -> String,
) -> Vec<Named> {
    runs(&project(entries, name), versions)
        .into_iter()
        .map(|(releases, name)| Named { releases, name })
        .collect()
}

fn available<T>(
    entries: &[(usize, T)],
    versions: &[Version],
    profiles: impl Fn(&T) -> Vec<Profile>,
) -> Vec<Availability> {
    runs(&project(entries, profiles), versions)
        .into_iter()
        .map(|(releases, profiles)| Availability { releases, profiles })
        .collect()
}

/// Group `(release index, value)` pairs, sorted by index, into maximal runs of
/// consecutive releases carrying an equal value.
/// Each member's documented values over the releases documenting them,
/// ordered by member, side, and release.
fn domains<T>(
    entries: &[(usize, T)],
    versions: &[Version],
    domains: impl Fn(&T) -> Vec<(String, bool, Domain)>,
) -> Vec<MemberDomain> {
    let mut by_member: BTreeMap<(String, bool), Vec<(usize, Domain)>> = BTreeMap::new();
    for (index, entry) in entries {
        for (member, returned, domain) in domains(entry) {
            by_member
                .entry((member, returned))
                .or_default()
                .push((*index, domain));
        }
    }
    by_member
        .into_iter()
        .flat_map(|((member, returned), entries)| {
            runs(&entries, versions)
                .into_iter()
                .map(move |(releases, domain)| MemberDomain {
                    releases,
                    member: member.clone(),
                    returned,
                    domain,
                })
        })
        .collect()
}

fn runs<T: Clone + PartialEq>(
    entries: &[(usize, T)],
    versions: &[Version],
) -> Vec<(ReleaseRange, T)> {
    let mut runs: Vec<(usize, usize, T)> = Vec::new();
    for (index, value) in entries {
        match runs.last_mut() {
            Some((_, last, current)) if *last + 1 == *index && current == value => *last = *index,
            _ => runs.push((*index, *index, value.clone())),
        }
    }
    runs.into_iter()
        .map(|(first, last, value)| {
            (
                ReleaseRange {
                    first: versions[first],
                    last: versions[last],
                },
                value,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_split_on_changes_and_gaps() {
        let versions = [
            Version::new(1, 15, 0),
            Version::new(1, 16, 0),
            Version::new(1, 17, 0),
            Version::new(1, 18, 0),
            Version::new(1, 19, 0),
        ];
        let entries = [(0, 'a'), (1, 'a'), (2, 'b'), (4, 'b')];
        let runs = runs(&entries, &versions);
        let rendered = runs
            .iter()
            .map(|(range, value)| format!("{range}={value}"))
            .collect::<Vec<_>>();
        assert_eq!(rendered, ["1.15.0..=1.16.0=a", "1.17.0=b", "1.19.0=b"]);
    }
}
