//! Semantic differences between two normalized firmware catalogs.

use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;

use crate::catalog::{
    CatalogCommand, CatalogEvent, CatalogFamily, CatalogSchema, CommandScope, EventScope,
};

/// Identity of a catalog in a version-diff report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CatalogIdentity {
    pub family: CatalogFamily,
    pub cube_tag: String,
}

/// Stable identity for a command across firmware versions.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CommandKey {
    pub scope: CommandScope,
    /// Vendor ACI OCF or standard HCI full opcode, depending on `scope`.
    pub code: u16,
}

/// Stable identity for an event across firmware versions.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EventKey {
    pub scope: EventScope,
    pub code: u16,
}

/// A command retaining its wire identity but changing semantic metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChangedCommand {
    pub key: CommandKey,
    pub from: CatalogCommand,
    pub to: CatalogCommand,
}

/// An event retaining its wire identity but changing its handler name.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ChangedEvent {
    pub key: EventKey,
    pub from: CatalogEvent,
    pub to: CatalogEvent,
}

/// Additions, removals, and semantic changes in the command namespace.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CommandChanges {
    pub added: Vec<CatalogCommand>,
    pub removed: Vec<CatalogCommand>,
    pub changed: Vec<ChangedCommand>,
}

/// Additions, removals, and semantic changes in the event namespace.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct EventChanges {
    pub added: Vec<CatalogEvent>,
    pub removed: Vec<CatalogEvent>,
    pub changed: Vec<ChangedEvent>,
}

/// Semantic delta between two catalog schemas from the same firmware family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VersionDiff {
    pub from: CatalogIdentity,
    pub to: CatalogIdentity,
    pub commands: CommandChanges,
    pub events: EventChanges,
}

impl VersionDiff {
    pub fn has_changes(&self) -> bool {
        !self.commands.added.is_empty()
            || !self.commands.removed.is_empty()
            || !self.commands.changed.is_empty()
            || !self.events.added.is_empty()
            || !self.events.removed.is_empty()
            || !self.events.changed.is_empty()
    }
}

#[derive(Debug, Error)]
pub enum VersionDiffError {
    #[error("cannot diff invalid source catalog: {reason}")]
    InvalidFromCatalog { reason: String },
    #[error("cannot diff invalid destination catalog: {reason}")]
    InvalidToCatalog { reason: String },
    #[error("cannot diff different firmware families: {from:?} and {to:?}")]
    FamilyMismatch {
        from: CatalogFamily,
        to: CatalogFamily,
    },
    #[error("catalog {tag} contains more than one command for {key:?}")]
    DuplicateCommand { tag: String, key: CommandKey },
    #[error("catalog {tag} contains more than one event for {key:?}")]
    DuplicateEvent { tag: String, key: EventKey },
}

/// Compare protocol semantics, not parser incidental details such as source
/// byte offsets. A generated declaration moving within a C file is therefore
/// not reported as a firmware API change.
pub fn diff_catalogs(
    from: &CatalogSchema,
    to: &CatalogSchema,
) -> Result<VersionDiff, VersionDiffError> {
    ensure_compatible(from, to)?;
    let from_commands = unique_commands(from)?;
    let to_commands = unique_commands(to)?;
    let from_events = unique_events(from)?;
    let to_events = unique_events(to)?;

    let mut commands = CommandChanges::default();
    for (key, command) in &from_commands {
        match to_commands.get(key) {
            None => commands.removed.push((*command).clone()),
            Some(next) if !same_command_shape(command, next) => {
                commands.changed.push(ChangedCommand {
                    key: *key,
                    from: (*command).clone(),
                    to: (*next).clone(),
                });
            }
            Some(_) => {}
        }
    }
    for (key, command) in &to_commands {
        if !from_commands.contains_key(key) {
            commands.added.push((*command).clone());
        }
    }

    let mut events = EventChanges::default();
    for (key, event) in &from_events {
        match to_events.get(key) {
            None => events.removed.push((*event).clone()),
            Some(next) if !same_event_shape(event, next) => {
                events.changed.push(ChangedEvent {
                    key: *key,
                    from: (*event).clone(),
                    to: (*next).clone(),
                });
            }
            Some(_) => {}
        }
    }
    for (key, event) in &to_events {
        if !from_events.contains_key(key) {
            events.added.push((*event).clone());
        }
    }

    Ok(VersionDiff {
        from: CatalogIdentity {
            family: from.family,
            cube_tag: from.cube_tag.clone(),
        },
        to: CatalogIdentity {
            family: to.family,
            cube_tag: to.cube_tag.clone(),
        },
        commands,
        events,
    })
}

fn ensure_compatible(from: &CatalogSchema, to: &CatalogSchema) -> Result<(), VersionDiffError> {
    from.validate()
        .map_err(|reason| VersionDiffError::InvalidFromCatalog { reason })?;
    to.validate()
        .map_err(|reason| VersionDiffError::InvalidToCatalog { reason })?;
    if from.family != to.family {
        return Err(VersionDiffError::FamilyMismatch {
            from: from.family,
            to: to.family,
        });
    }
    Ok(())
}

fn unique_commands(
    catalog: &CatalogSchema,
) -> Result<BTreeMap<CommandKey, &CatalogCommand>, VersionDiffError> {
    let mut commands = BTreeMap::new();
    for command in &catalog.commands {
        let key = CommandKey {
            scope: command.scope(),
            code: command.code(),
        };
        if commands.insert(key, command).is_some() {
            return Err(VersionDiffError::DuplicateCommand {
                tag: catalog.cube_tag.clone(),
                key,
            });
        }
    }
    Ok(commands)
}

fn unique_events(
    catalog: &CatalogSchema,
) -> Result<BTreeMap<EventKey, &CatalogEvent>, VersionDiffError> {
    let mut events = BTreeMap::new();
    for event in &catalog.events {
        let key = EventKey {
            scope: event.scope(),
            code: event.code,
        };
        if events.insert(key, event).is_some() {
            return Err(VersionDiffError::DuplicateEvent {
                tag: catalog.cube_tag.clone(),
                key,
            });
        }
    }
    Ok(events)
}

fn same_command_shape(left: &CatalogCommand, right: &CatalogCommand) -> bool {
    left.kind == right.kind
        && left.name == right.name
        && left.completion == right.completion
        && left.request == right.request
}

fn same_event_shape(left: &CatalogEvent, right: &CatalogEvent) -> bool {
    left.kind == right.kind && left.code == right.code && left.name == right.name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CatalogCommandKind, CatalogCompletion, CatalogEventKind, EnvelopeEvidence};

    fn command(ocf: u16, name: &str) -> CatalogCommand {
        CatalogCommand {
            kind: CatalogCommandKind::VendorAci { ocf },
            name: name.to_owned(),
            source_name: "fixture.c".to_owned(),
            source_offset: 0,
            completion: CatalogCompletion::CommandComplete {
                returns: EnvelopeEvidence::fixed(0),
            },
            request: EnvelopeEvidence::fixed(0),
        }
    }

    fn event(code: u16, name: &str) -> CatalogEvent {
        CatalogEvent {
            kind: CatalogEventKind::VendorAci {
                payload: EnvelopeEvidence::fixed(0),
            },
            code,
            name: name.to_owned(),
            source_name: "events.c".to_owned(),
            source_offset: 0,
        }
    }

    fn schema(tag: &str) -> CatalogSchema {
        CatalogSchema {
            family: CatalogFamily::Stm32Wb,
            cube_tag: tag.to_owned(),
            commands: Vec::new(),
            events: Vec::new(),
        }
    }

    #[test]
    fn reports_additions_removals_and_semantic_changes_but_ignores_source_moves() {
        let mut old = schema("v1.15.0");
        old.commands = vec![command(1, "renamed"), command(2, "removed")];
        old.events = vec![event(0x400, "renamed_event"), event(0x401, "removed_event")];

        let mut new = schema("v1.17.0");
        let mut moved = command(1, "renamed");
        moved.source_offset = 99;
        new.commands = vec![moved, command(3, "added")];
        new.commands[0].completion = CatalogCompletion::CommandComplete {
            returns: EnvelopeEvidence::fixed(3),
        };
        new.events = vec![event(0x400, "renamed_event"), event(0x402, "added_event")];
        new.events[0].source_name = "new_events.c".to_owned();

        let diff = diff_catalogs(&old, &new).unwrap();
        assert_eq!(
            diff.commands
                .added
                .iter()
                .map(CatalogCommand::ocf)
                .collect::<Vec<_>>(),
            [3]
        );
        assert_eq!(
            diff.commands
                .removed
                .iter()
                .map(CatalogCommand::ocf)
                .collect::<Vec<_>>(),
            [2]
        );
        assert_eq!(diff.commands.changed.len(), 1);
        assert_eq!(diff.commands.changed[0].key.code, 1);
        assert_eq!(
            diff.events
                .added
                .iter()
                .map(|entry| entry.code)
                .collect::<Vec<_>>(),
            [0x402]
        );
        assert_eq!(
            diff.events
                .removed
                .iter()
                .map(|entry| entry.code)
                .collect::<Vec<_>>(),
            [0x401]
        );
        assert!(diff.events.changed.is_empty());
    }
}
