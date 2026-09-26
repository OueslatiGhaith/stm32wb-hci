//! An entry's facts across releases, as the declaring macros consume them.
//!
//! A Rust declaration names one C command or event and must be correct in
//! every release the catalog describes. Its history is a list of segments:
//! maximal runs of consecutive releases over which everything a declaration
//! depends on stays the same (the entry and its code, the name it was
//! generated under, its definition, the profiles supporting it, and the
//! annotations resolving or excluding it). Each segment becomes one
//! conditionally compiled variant of the declaration.

use std::ptr;

use crate::annotations::Annotation;
use crate::view::{active_command, active_event};
use crate::{
    ActiveCommand, ActiveEvent, Bundled, Error, Named, Profile, Provenance, ReleaseRange, Version,
    profiles_at,
};

/// Consecutive releases over which an entry's facts do not change.
#[derive(Debug)]
pub struct Segment<'a, T> {
    pub releases: ReleaseRange,
    /// Profiles supporting the entry throughout the segment.
    pub profiles: &'a [Profile],
    /// The entry as resolved in every release of the segment.
    pub entry: T,
}

pub type CommandSegment<'a> = Segment<'a, ActiveCommand<'a>>;
pub type EventSegment<'a> = Segment<'a, ActiveEvent<'a>>;

impl Bundled {
    /// The history of every command that carried `name` in any release.
    ///
    /// A command keeps its history across renames. A name may also move to a
    /// different opcode between releases, but never names two commands in
    /// the same release.
    pub fn command_segments(&self, name: &str) -> Result<Vec<CommandSegment<'_>>, Error> {
        let commands = self
            .catalog
            .commands
            .iter()
            .filter(|command| carries(&command.names, name))
            .collect::<Vec<_>>();
        self.segments(
            &format!("command {name}"),
            |release| {
                commands
                    .iter()
                    .filter_map(|command| {
                        Some((
                            profiles_at(&command.availability, release)?,
                            active_command(&self.annotations, command, release)?,
                        ))
                    })
                    .collect()
            },
            |left, right| {
                ptr::eq(left.command, right.command)
                    && ptr::eq(left.name, right.name)
                    && ptr::eq(left.definition, right.definition)
                    && same(
                        annotation(left.params.provenance),
                        annotation(right.params.provenance),
                    )
                    && same(
                        left.returns
                            .and_then(|returns| annotation(returns.provenance)),
                        right
                            .returns
                            .and_then(|returns| annotation(returns.provenance)),
                    )
                    && same(left.excluded, right.excluded)
            },
        )
    }

    /// The history of every event that carried `name` in any release.
    pub fn event_segments(&self, name: &str) -> Result<Vec<EventSegment<'_>>, Error> {
        let events = self
            .catalog
            .events
            .iter()
            .filter(|event| carries(&event.names, name))
            .collect::<Vec<_>>();
        self.segments(
            &format!("event {name}"),
            |release| {
                events
                    .iter()
                    .filter_map(|event| {
                        Some((
                            profiles_at(&event.availability, release)?,
                            active_event(&self.annotations, event, release)?,
                        ))
                    })
                    .collect()
            },
            |left, right| {
                ptr::eq(left.event, right.event)
                    && ptr::eq(left.name, right.name)
                    && ptr::eq(left.definition, right.definition)
                    && same(
                        annotation(left.payload.provenance),
                        annotation(right.payload.provenance),
                    )
                    && same(left.excluded, right.excluded)
            },
        )
    }

    /// Resolve the entry in every release, in catalog order, and coalesce
    /// neighbours with identical facts. A release without the entry ends a
    /// segment.
    fn segments<'a, T>(
        &'a self,
        label: &str,
        at: impl Fn(Version) -> Vec<(&'a [Profile], T)>,
        same_entry: impl Fn(&T, &T) -> bool,
    ) -> Result<Vec<Segment<'a, T>>, Error> {
        let mut segments: Vec<Segment<'a, T>> = Vec::new();
        // Whether the previous release defined the entry, i.e. ends the last segment.
        let mut adjacent = false;
        for release in self.catalog.versions() {
            let mut active = at(release);
            if active.len() > 1 {
                return Err(Error::invalid(format!(
                    "{label} names {} entries in {release}",
                    active.len()
                )));
            }
            let Some((profiles, entry)) = active.pop() else {
                adjacent = false;
                continue;
            };
            let extends = std::mem::replace(&mut adjacent, true);
            if let Some(last) = segments.last_mut()
                && extends
                && ptr::eq(last.profiles, profiles)
                && same_entry(&last.entry, &entry)
            {
                last.releases.last = release;
                continue;
            }
            segments.push(Segment {
                releases: ReleaseRange::single(release),
                profiles,
                entry,
            });
        }
        if segments.is_empty() {
            return Err(Error::invalid(format!("the catalog has no {label}")));
        }
        Ok(segments)
    }
}

fn carries(names: &[Named], name: &str) -> bool {
    names.iter().any(|named| named.name == name)
}

fn annotation(provenance: Provenance<'_>) -> Option<&Annotation> {
    match provenance {
        Provenance::Extracted => None,
        Provenance::Annotated(annotation) => Some(annotation),
    }
}

fn same(left: Option<&Annotation>, right: Option<&Annotation>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => ptr::eq(left, right),
        (None, None) => true,
        _ => false,
    }
}
