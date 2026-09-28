//! Merge per-release extraction snapshots into release-ranged histories.

use std::collections::BTreeMap;

use crate::{
    Availability, Bearer, Binary, Catalog, Command, CommandDefinition, CommandScope, Completion,
    Error, Event, EventDefinition, EventScope, Family, Layout, Named, Platform, Profile,
    ReleaseRange, ReleaseSource, Structs, Version,
};

/// Everything extracted from one tagged release.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub source: ReleaseSource,
    pub binaries: Vec<SnapshotBinary>,
    pub commands: Vec<SnapshotCommand>,
    pub events: Vec<SnapshotEvent>,
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
                        file: binary.family.binary_file_name(binary.profile),
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
