//! The catalog as seen by one wireless-binary target.

use crate::annotations::{Annotation, Annotations, LayoutSlot};
use crate::{
    Bundled, Catalog, Command, CommandDefinition, Error, Event, EventDefinition, Family, Field,
    Layout, Profile, Structs, Version, profiles_at,
};

/// One Cube release running one stack profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Target {
    pub release: Version,
    pub profile: Profile,
}

impl Target {
    /// The Cargo features selecting this target, e.g. `fw_1_24_0,stack-light`.
    pub fn features(self) -> String {
        format!(
            "{},{}",
            self.release.feature_name(),
            self.profile.feature_name()
        )
    }
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

#[derive(Debug)]
pub struct ActiveCommand<'a> {
    pub command: &'a Command,
    pub name: &'a str,
    pub definition: &'a CommandDefinition,
    pub params: ResolvedLayout<'a>,
    pub returns: Option<ResolvedLayout<'a>>,
    /// Set when an annotation places the command outside this crate's scope.
    pub excluded: Option<&'a Annotation>,
}

#[derive(Debug)]
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
            profiles_at(&command.availability, release)?
                .contains(&self.target.profile)
                .then(|| active_command(self.annotations, command, release))?
        })
    }

    /// Events the target's binary can emit.
    pub fn events(&self) -> impl Iterator<Item = ActiveEvent<'a>> + '_ {
        let release = self.target.release;
        self.catalog.events.iter().filter_map(move |event| {
            profiles_at(&event.availability, release)?
                .contains(&self.target.profile)
                .then(|| active_event(self.annotations, event, release))?
        })
    }
}

impl Bundled {
    /// The view of one target over annotations audited when `self` was built.
    pub fn view(&self, target: Target) -> Result<TargetView<'_>, Error> {
        if !self
            .catalog
            .versions()
            .any(|release| release == target.release)
        {
            return Err(Error::invalid(format!(
                "the catalog does not describe Cube release {}",
                target.release
            )));
        }
        Ok(TargetView {
            catalog: &self.catalog,
            annotations: &self.annotations,
            target,
        })
    }

    /// One target for each distinct interface, i.e. set of resolved commands
    /// and events: the first in release, then profile, order. Every other
    /// target compiles exactly the same declarations as its representative.
    pub fn distinct_targets(&self) -> Vec<Target> {
        let mut seen = std::collections::BTreeSet::new();
        let mut representatives = Vec::new();
        for release in self.catalog.versions() {
            for &profile in self.catalog.platform.profiles() {
                let target = Target { release, profile };
                let view = self.view(target).expect("release comes from the catalog");
                let annotation = |provenance: Provenance<'_>| match provenance {
                    Provenance::Extracted => 0,
                    Provenance::Annotated(annotation) => ptr(annotation),
                };
                let commands = view.commands().map(|command| {
                    [
                        ptr(command.definition),
                        annotation(command.params.provenance),
                        command
                            .returns
                            .map_or(0, |returns| annotation(returns.provenance)),
                        command.excluded.map_or(0, ptr),
                    ]
                });
                let events = view.events().map(|event| {
                    [
                        ptr(event.definition),
                        annotation(event.payload.provenance),
                        0,
                        event.excluded.map_or(0, ptr),
                    ]
                });
                if seen.insert(commands.chain(events).collect::<Vec<_>>()) {
                    representatives.push(target);
                }
            }
        }
        representatives
    }
}

/// Identity of a catalog or annotation item, stable for one process.
fn ptr<T>(item: &T) -> usize {
    std::ptr::from_ref(item) as usize
}

/// A command as defined in one release, with every layer resolved.
pub(crate) fn active_command<'a>(
    annotations: &'a Annotations,
    command: &'a Command,
    release: Version,
) -> Option<ActiveCommand<'a>> {
    let definition = command.definition_at(release)?;
    let name = name_at(&command.names, release);
    let resolve = |slot, extracted| {
        resolve(
            annotations,
            name,
            true,
            slot,
            release,
            extracted,
            &definition.structs,
        )
    };
    Some(ActiveCommand {
        command,
        name,
        definition,
        params: resolve(LayoutSlot::Params, &definition.params),
        returns: definition
            .returns
            .as_ref()
            .map(|returns| resolve(LayoutSlot::Returns, returns)),
        excluded: annotations.exclusion(name, true, release),
    })
}

/// An event as defined in one release, with every layer resolved.
pub(crate) fn active_event<'a>(
    annotations: &'a Annotations,
    event: &'a Event,
    release: Version,
) -> Option<ActiveEvent<'a>> {
    let definition = event.definition_at(release)?;
    let name = name_at(&event.names, release);
    Some(ActiveEvent {
        event,
        name,
        definition,
        payload: resolve(
            annotations,
            name,
            false,
            LayoutSlot::Payload,
            release,
            &definition.payload,
            &definition.structs,
        ),
        excluded: annotations.exclusion(name, false, release),
    })
}

fn resolve<'a>(
    annotations: &'a Annotations,
    name: &str,
    is_command: bool,
    slot: LayoutSlot,
    release: Version,
    extracted: &'a Layout,
    structs: &'a Structs,
) -> ResolvedLayout<'a> {
    if let Some((annotation, fields)) = annotations.layout(name, is_command, slot, release) {
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

fn name_at(names: &[crate::Named], release: Version) -> &str {
    names
        .iter()
        .find(|named| named.releases.contains(release))
        .map_or("", |named| named.name.as_str())
}
