//! The catalog as seen by one wireless-binary target.

use crate::annotations::{Annotation, Annotations, LayoutSlot};
use crate::{
    Catalog, Command, CommandDefinition, Error, Event, EventDefinition, Family, Field, Layout,
    Profile, Structs, Version, profiles_at,
};

/// One Cube release running one stack profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Target {
    pub release: Version,
    pub profile: Profile,
}

/// Every fact that applies to one target, with all three layers resolved.
pub struct TargetView<'a> {
    catalog: &'a Catalog,
    annotations: &'a Annotations,
    target: Target,
}

/// Where a resolved layout came from.
#[derive(Clone, Copy, Debug)]
pub enum Provenance<'a> {
    Extracted,
    Annotated(&'a Annotation),
}

#[derive(Clone, Copy, Debug)]
pub struct ResolvedLayout<'a> {
    /// The fields, or the extractor's reason for leaving them unresolved.
    pub fields: Result<&'a [Field], &'a str>,
    pub structs: &'a Structs,
    pub provenance: Provenance<'a>,
}

pub struct ActiveCommand<'a> {
    pub command: &'a Command,
    pub name: &'a str,
    pub definition: &'a CommandDefinition,
    pub params: ResolvedLayout<'a>,
    pub returns: Option<ResolvedLayout<'a>>,
    /// Set when an annotation places the command outside this crate's scope.
    pub excluded: Option<&'a Annotation>,
}

pub struct ActiveEvent<'a> {
    pub event: &'a Event,
    pub name: &'a str,
    pub definition: &'a EventDefinition,
    pub payload: ResolvedLayout<'a>,
    pub excluded: Option<&'a Annotation>,
}

impl<'a> TargetView<'a> {
    pub fn new(
        catalog: &'a Catalog,
        annotations: &'a Annotations,
        target: Target,
    ) -> Result<Self, Error> {
        if !catalog.versions().any(|release| release == target.release) {
            return Err(Error::invalid(format!(
                "the catalog does not describe Cube release {}",
                target.release
            )));
        }
        annotations.audit(catalog)?;
        Ok(Self {
            catalog,
            annotations,
            target,
        })
    }

    pub fn target(&self) -> Target {
        self.target
    }

    /// Families shipping a binary for this target's profile and release.
    pub fn families(&self) -> Vec<Family> {
        let mut families = self
            .catalog
            .binaries
            .iter()
            .filter(|binary| {
                binary.profile == self.target.profile
                    && binary.releases.contains(self.target.release)
            })
            .map(|binary| binary.family)
            .collect::<Vec<_>>();
        families.sort();
        families.dedup();
        families
    }

    /// Commands the target's binary accepts.
    pub fn commands(&self) -> impl Iterator<Item = ActiveCommand<'a>> + '_ {
        let release = self.target.release;
        self.catalog.commands.iter().filter_map(move |command| {
            let definition = command.definition_at(release)?;
            profiles_at(&command.availability, release)?
                .contains(&self.target.profile)
                .then_some(())?;
            let name = name_at(&command.names, release);
            Some(ActiveCommand {
                command,
                name,
                definition,
                params: self.resolve(
                    name,
                    true,
                    LayoutSlot::Params,
                    &definition.params,
                    &definition.structs,
                ),
                returns: definition.returns.as_ref().map(|returns| {
                    self.resolve(
                        name,
                        true,
                        LayoutSlot::Returns,
                        returns,
                        &definition.structs,
                    )
                }),
                excluded: self.annotations.exclusion(name, true, release),
            })
        })
    }

    /// Events the target's binary can emit.
    pub fn events(&self) -> impl Iterator<Item = ActiveEvent<'a>> + '_ {
        let release = self.target.release;
        self.catalog.events.iter().filter_map(move |event| {
            let definition = event.definition_at(release)?;
            profiles_at(&event.availability, release)?
                .contains(&self.target.profile)
                .then_some(())?;
            let name = name_at(&event.names, release);
            Some(ActiveEvent {
                event,
                name,
                definition,
                payload: self.resolve(
                    name,
                    false,
                    LayoutSlot::Payload,
                    &definition.payload,
                    &definition.structs,
                ),
                excluded: self.annotations.exclusion(name, false, release),
            })
        })
    }

    fn resolve(
        &self,
        name: &str,
        is_command: bool,
        slot: LayoutSlot,
        extracted: &'a Layout,
        structs: &'a Structs,
    ) -> ResolvedLayout<'a> {
        if let Some((annotation, fields)) =
            self.annotations
                .layout(name, is_command, slot, self.target.release)
        {
            return ResolvedLayout {
                fields: Ok(fields),
                structs: &annotation.structs,
                provenance: Provenance::Annotated(annotation),
            };
        }
        ResolvedLayout {
            fields: match extracted {
                Layout::Fields(fields) => Ok(fields),
                Layout::Unresolved(reason) => Err(reason),
            },
            structs,
            provenance: Provenance::Extracted,
        }
    }
}

fn name_at(names: &[crate::Named], release: Version) -> &str {
    names
        .iter()
        .find(|named| named.releases.contains(release))
        .map_or("", |named| named.name.as_str())
}
