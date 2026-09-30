//! Layer 3: curated facts that no ST artifact states in a machine-readable form.
//!
//! Every annotation names the catalog entry it refines, the releases it
//! applies to, the evidence it rests on, and why it is needed:
//!
//! ```toml
//! [[annotations]]
//! event = "SHCI_SUB_EVT_CODE_READY"
//! source = "AN5289 rev 13, section 4.8.1"
//! reason = "the payload is an enum; its width depends on the CPU2 ABI"
//! payload = ["sysevt_ready_rsp: u8"]
//! ```
//!
//! Annotations may fill a layout the extractor reported as unresolved, or
//! mark an entry as outside this crate's scope (`exclude = true`). They never
//! silently disagree with extracted facts: [`Annotations::audit`] rejects an
//! annotation whose target no longer exists, one that fills a layout the
//! extractor now derives itself (stale), and one that contradicts an
//! extracted layout unless it is explicitly marked `override = true`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    Catalog, Command, Completion, Error, Event, Field, Layout, ReleaseRange, Structs, Version,
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Annotations {
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    /// Generated C name of the annotated command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Generated C name of the annotated event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Releases the annotation applies to; every defined release by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub releases: Option<ReleaseRange>,
    /// Citation for the fact: a document and section, an erratum, or a
    /// reproducible hardware observation.
    pub source: String,
    pub reason: String,
    /// Replace an extracted layout instead of filling an unresolved one.
    #[serde(default, rename = "override", skip_serializing_if = "is_false")]
    pub replaces_extracted: bool,
    /// The entry is outside this crate's scope.
    #[serde(default, skip_serializing_if = "is_false")]
    pub exclude: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<Field>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Vec<Field>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Vec<Field>>,
    #[serde(default, skip_serializing_if = "Structs::is_empty")]
    pub structs: Structs,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Which layout of an entry an annotation supplies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutSlot {
    Params,
    Returns,
    Payload,
}

impl LayoutSlot {
    fn name(self) -> &'static str {
        match self {
            Self::Params => "params",
            Self::Returns => "returns",
            Self::Payload => "payload",
        }
    }
}

enum Entry<'a> {
    Command(&'a Command),
    Event(&'a Event),
}

impl Annotation {
    fn label(&self) -> String {
        match (&self.command, &self.event) {
            (Some(name), _) => format!("annotation for command {name}"),
            (_, Some(name)) => format!("annotation for event {name}"),
            _ => "annotation without a target".to_owned(),
        }
    }

    fn slot(&self, slot: LayoutSlot) -> Option<&[Field]> {
        match slot {
            LayoutSlot::Params => self.params.as_deref(),
            LayoutSlot::Returns => self.returns.as_deref(),
            LayoutSlot::Payload => self.payload.as_deref(),
        }
    }

    fn applies(&self, release: Version) -> bool {
        self.releases.is_none_or(|range| range.contains(release))
    }

    fn entry<'a>(&self, catalog: &'a Catalog) -> Result<Entry<'a>, Error> {
        match (&self.command, &self.event) {
            (Some(name), None) => catalog
                .command_named(name)
                .map(Entry::Command)
                .ok_or_else(|| Error::audit(format!("{}: no such command", self.label()))),
            (None, Some(name)) => {
                let mut matches = catalog
                    .events
                    .iter()
                    .filter(|event| event.names.iter().any(|named| &named.name == name));
                match (matches.next(), matches.next()) {
                    (Some(event), None) => Ok(Entry::Event(event)),
                    (None, _) => Err(Error::audit(format!("{}: no such event", self.label()))),
                    (Some(_), Some(_)) => Err(Error::audit(format!(
                        "{}: the name is ambiguous",
                        self.label()
                    ))),
                }
            }
            _ => Err(Error::audit(format!(
                "{}: set exactly one of `command` or `event`",
                self.label()
            ))),
        }
    }
}

impl Annotations {
    pub fn from_toml(source: &str) -> Result<Self, Error> {
        toml::from_str(source).map_err(|error| Error::parse(error.to_string()))
    }

    pub fn load(path: &Path) -> Result<Self, Error> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| Error::parse(format!("could not read {}: {error}", path.display())))?;
        Self::from_toml(&source).map_err(|error| error.context(path.display()))
    }

    /// Reject dangling, stale, contradictory, or overlapping annotations.
    pub fn audit(&self, catalog: &Catalog) -> Result<(), Error> {
        for (index, annotation) in self.annotations.iter().enumerate() {
            audit_one(annotation, catalog)?;
            for other in &self.annotations[index + 1..] {
                if annotation.command == other.command
                    && annotation.event == other.event
                    && ranges_overlap(annotation.releases, other.releases)
                    && facts_overlap(annotation, other)
                {
                    return Err(Error::audit(format!(
                        "{}: overlaps another annotation for the same fact",
                        annotation.label()
                    )));
                }
            }
        }
        Ok(())
    }

    /// The annotation excluding a named entry in a release, if any.
    pub fn exclusion(&self, name: &str, is_command: bool, release: Version) -> Option<&Annotation> {
        self.matching(name, is_command, release)
            .find(|annotation| annotation.exclude)
    }

    /// An annotated layout for a named entry in a release, if any.
    pub fn layout(
        &self,
        name: &str,
        is_command: bool,
        slot: LayoutSlot,
        release: Version,
    ) -> Option<(&Annotation, &[Field])> {
        self.matching(name, is_command, release)
            .find_map(|annotation| annotation.slot(slot).map(|fields| (annotation, fields)))
    }

    fn matching<'s>(
        &'s self,
        name: &str,
        is_command: bool,
        release: Version,
    ) -> impl Iterator<Item = &'s Annotation> {
        self.annotations.iter().filter(move |annotation| {
            let target = if is_command {
                &annotation.command
            } else {
                &annotation.event
            };
            target.as_deref() == Some(name) && annotation.applies(release)
        })
    }
}

fn ranges_overlap(left: Option<ReleaseRange>, right: Option<ReleaseRange>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.overlaps(right),
        _ => true,
    }
}

fn facts_overlap(left: &Annotation, right: &Annotation) -> bool {
    (left.exclude && right.exclude)
        || [LayoutSlot::Params, LayoutSlot::Returns, LayoutSlot::Payload]
            .into_iter()
            .any(|slot| left.slot(slot).is_some() && right.slot(slot).is_some())
}

fn audit_one(annotation: &Annotation, catalog: &Catalog) -> Result<(), Error> {
    let label = annotation.label();
    if annotation.source.trim().is_empty() {
        return Err(Error::audit(format!(
            "{label}: `source` must cite evidence"
        )));
    }
    if annotation.reason.trim().is_empty() {
        return Err(Error::audit(format!("{label}: `reason` must not be empty")));
    }
    let slots = [LayoutSlot::Params, LayoutSlot::Returns, LayoutSlot::Payload]
        .into_iter()
        .filter(|slot| annotation.slot(*slot).is_some())
        .collect::<Vec<_>>();
    match (annotation.exclude, slots.is_empty()) {
        (true, false) => {
            return Err(Error::audit(format!(
                "{label}: an exclusion cannot also supply layouts"
            )));
        }
        (false, true) => {
            return Err(Error::audit(format!("{label}: states no fact")));
        }
        _ => {}
    }
    if annotation.exclude && annotation.replaces_extracted {
        return Err(Error::audit(format!(
            "{label}: `override` only applies to layouts"
        )));
    }
    if !annotation.structs.is_empty() && slots.is_empty() {
        return Err(Error::audit(format!("{label}: structs without a layout")));
    }

    let entry = annotation.entry(catalog)?;
    let defined = |release: Version| match &entry {
        Entry::Command(command) => command.definition_at(release).is_some(),
        Entry::Event(event) => event.definition_at(release).is_some(),
    };
    let releases = catalog
        .versions()
        .filter(|release| annotation.applies(*release))
        .collect::<Vec<_>>();
    if let Some(range) = annotation.releases {
        if !catalog.versions().any(|release| release == range.first)
            || !catalog.versions().any(|release| release == range.last)
        {
            return Err(Error::audit(format!(
                "{label}: release range {range} is not bounded by declared releases"
            )));
        }
        if let Some(missing) = releases.iter().find(|release| !defined(**release)) {
            return Err(Error::audit(format!(
                "{label}: the entry is not defined in {missing}"
            )));
        }
    }

    for slot in slots {
        let fields = annotation.slot(slot).expect("slot was selected as present");
        Layout::Fields(fields.to_vec())
            .envelope(&annotation.structs)
            .map_err(|error| error.context(format!("{label} {}", slot.name())))?;
        for release in releases.iter().copied().filter(|release| defined(*release)) {
            let extracted = match (&entry, slot) {
                (Entry::Command(command), LayoutSlot::Params) => command
                    .definition_at(release)
                    .map(|definition| &definition.params),
                (Entry::Command(command), LayoutSlot::Returns) => {
                    let definition = command.definition_at(release).expect("release is defined");
                    if definition.completion == Completion::CommandStatus {
                        return Err(Error::audit(format!(
                            "{label}: a Command Status command has no return parameters"
                        )));
                    }
                    definition.returns.as_ref()
                }
                (Entry::Event(event), LayoutSlot::Payload) => event
                    .definition_at(release)
                    .map(|definition| &definition.payload),
                _ => {
                    return Err(Error::audit(format!(
                        "{label}: {} does not apply to this kind of entry",
                        slot.name()
                    )));
                }
            };
            match (extracted, annotation.replaces_extracted) {
                (Some(Layout::Unresolved(_)), false) => {
                    // What the headers document for the members the
                    // extractor could not lay out must fit the annotation.
                    let annotated = Layout::Fields(fields.to_vec());
                    let (domains, bearers) = match &entry {
                        Entry::Command(command) => (
                            &command.domains,
                            command
                                .definition_at(release)
                                .filter(|_| slot == LayoutSlot::Params)
                                .map_or(&[][..], |definition| &definition.bearers),
                        ),
                        Entry::Event(event) => (
                            &event.domains,
                            event
                                .definition_at(release)
                                .map_or(&[][..], |definition| &definition.bearers),
                        ),
                    };
                    let context = || format!("{label} in {release}");
                    for domain in domains.iter().filter(|domain| {
                        domain.releases.contains(release)
                            && domain.returned == (slot == LayoutSlot::Returns)
                    }) {
                        crate::domain::validate_domain(&annotated, domain)
                            .map_err(|error| error.context(context()))?;
                    }
                    crate::validate_bearers(&annotated, bearers)
                        .map_err(|error| error.context(context()))?;
                }
                (Some(Layout::Fields(extracted)), false) if extracted.as_slice() == fields => {
                    return Err(Error::audit(format!(
                        "{label}: stale in {release}; the extractor now derives this {} itself",
                        slot.name()
                    )));
                }
                (Some(Layout::Fields(_)), false) => {
                    return Err(Error::audit(format!(
                        "{label}: contradicts the extracted {} in {release}; mark it `override = true` if the extracted layout is wrong",
                        slot.name()
                    )));
                }
                (Some(Layout::Unresolved(_)), true) => {
                    return Err(Error::audit(format!(
                        "{label}: nothing to override in {release}; the {} is unresolved, so remove `override`",
                        slot.name()
                    )));
                }
                (Some(Layout::Fields(extracted)), true) if extracted.as_slice() == fields => {
                    return Err(Error::audit(format!(
                        "{label}: stale override in {release}; the extracted {} now agrees",
                        slot.name()
                    )));
                }
                (Some(Layout::Fields(_)), true) => {}
                (None, _) => {
                    return Err(Error::audit(format!(
                        "{label}: no extracted {} in {release}",
                        slot.name()
                    )));
                }
            }
        }
    }
    Ok(())
}
