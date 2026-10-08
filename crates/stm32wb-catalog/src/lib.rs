//! The normalized STM32WB wireless-interface catalog.
//!
//! The catalog is the single description of what each STM32CubeWB release's
//! CPU2 wireless binaries accept and emit. It has three layers, each with a
//! distinct source:
//!
//! 1. **Generated C** (`ble_*_aci.c`, `ble_hci_le.c`, `ble_events.c`,
//!    `ble_types.h`, `shci.h`, `shci.c`): opcodes, completion kinds, and wire
//!    layouts, and from the generated headers' doc comments, the parameters
//!    addressing an ATT bearer and the values each member may take.
//! 2. **ST documents** (`STM32WB_BLE_Wireless_Interface.html` and each
//!    family's `Release_Notes.html`): which stack profile supports each
//!    command and event, and which binaries exist per MCU family.
//! 3. **Curated annotations** (see [`annotations`]): facts no ST artifact
//!    states in a machine-readable form, each with a cited source.
//!
//! Layers 1 and 2 are written by `stm32wb-catalog-extract` into a checked-in
//! file that can be reproduced from the tagged Cube sources. Layer 3 is a
//! separate hand-maintained file audited against the extracted layers. Both
//! files ship inside this crate; see [`bundled`].

pub mod annotations;
mod bundled;
pub mod domain;
mod error;
mod hex;
mod history;
pub mod layout;
mod merge;
mod release;
mod target;
mod view;

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

pub use bundled::{Bundled, bundled};
pub use domain::{Domain, DomainItem, DomainKind, MemberDomain, StructDomain};
pub use error::{Error, ErrorKind};
pub use history::{CommandSegment, EventSegment, Segment};
pub use layout::{Element, Envelope, Field, FieldType, Layout, Scalar, Structs, UnionVariant};
pub use merge::{Snapshot, SnapshotBinary, SnapshotCommand, SnapshotEvent, merge_snapshots};
pub use release::{ReleaseRange, Version};
pub use target::{Family, Profile};
pub use view::{ActiveCommand, ActiveEvent, Provenance, ResolvedLayout, Target, TargetView};

/// The platform a catalog describes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Stm32wb,
    Stm32wba,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub platform: Platform,
    /// Every release the catalog describes, in ascending order.
    pub releases: Vec<ReleaseSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binaries: Vec<Binary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Command>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    /// The documented values of fields of the structures commands and events
    /// carry, by structure, field, and release.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub struct_domains: Vec<StructDomain>,
    /// The status codes ST's BLE stack returns besides the Bluetooth Core's,
    /// by release.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<StatusCode>,
}

/// A status code the BLE stack defines, `BLE_STATUS_*` in `ble_defs.h`, over
/// the releases defining it with this value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatusCode {
    pub releases: ReleaseRange,
    pub name: String,
    #[serde(with = "hex::byte")]
    pub value: u8,
}

/// Where one release's facts were read from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSource {
    pub version: Version,
    pub tag: String,
    /// The commit the tag resolved to when the catalog was extracted.
    pub commit: String,
    /// The stack profiles the release has, listed by STM32WBA, whose
    /// profiles changed across releases, and empty on STM32WB, where every
    /// release has every profile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<Profile>,
}

/// A CPU2 wireless binary shipped for one MCU family.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binary {
    pub family: Family,
    pub profile: Profile,
    pub file: String,
    pub releases: ReleaseRange,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandScope {
    /// A Bluetooth Core command, identified by its full opcode.
    Standard,
    /// An ST vendor ACI command (OGF 0x3F), identified by its full opcode.
    Vendor,
    /// An ST system (SHCI) command, sent on the CPU2 system channel (OGF
    /// 0x3F), identified by its full opcode.
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventScope {
    /// A Bluetooth Core event, identified by its event code.
    Standard,
    /// A Bluetooth Core LE meta event, identified by its subevent code.
    LeMeta,
    /// An ST vendor ACI event, identified by its 16-bit vendor event code.
    Vendor,
    /// An ST system (SHCI) event, identified by its 16-bit subevent code.
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completion {
    CommandComplete,
    CommandStatus,
}

/// Stack profiles supporting an entry over a range of releases.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Availability {
    pub releases: ReleaseRange,
    pub profiles: Vec<Profile>,
}

/// A name an entry carried over a range of releases.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub releases: ReleaseRange,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub scope: CommandScope,
    #[serde(with = "hex")]
    pub opcode: u16,
    /// Generated C names over time; almost always a single entry.
    pub names: Vec<Named>,
    pub availability: Vec<Availability>,
    pub definitions: Vec<CommandDefinition>,
    /// The documented values of members, by member and release.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<MemberDomain>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDefinition {
    pub releases: ReleaseRange,
    pub completion: Completion,
    /// Command parameters, excluding the HCI command header.
    pub params: Layout,
    /// Command Complete return parameters, including the leading status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<Layout>,
    #[serde(default, skip_serializing_if = "Structs::is_empty")]
    pub structs: Structs,
    /// Parameters addressing an ATT bearer rather than only a connection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bearers: Vec<Bearer>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub scope: EventScope,
    #[serde(with = "hex")]
    pub code: u16,
    pub names: Vec<Named>,
    pub availability: Vec<Availability>,
    pub definitions: Vec<EventDefinition>,
    /// The documented values of members, by member and release.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<MemberDomain>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventDefinition {
    pub releases: ReleaseRange,
    /// Event parameters after the event code (and subevent code, if any).
    pub payload: Layout,
    #[serde(default, skip_serializing_if = "Structs::is_empty")]
    pub structs: Structs,
    /// Parameters addressing an ATT bearer rather than only a connection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bearers: Vec<Bearer>,
}

/// A parameter addressing an ATT bearer: a connection handle
/// (0x0000..=0x0EFF) for the unenhanced bearer, or `first..=last` for an
/// enhanced one, whose low byte is its L2CAP channel index. The generated
/// documentation states the range; C declares only a `uint16_t`.
///
/// Written `"<member>: 0xEA00..=0xEA3F"`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, DeserializeFromStr, SerializeDisplay)]
pub struct Bearer {
    pub member: String,
    pub first: u16,
    pub last: u16,
}

impl fmt::Display for Bearer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: 0x{:04X}..=0x{:04X}",
            self.member, self.first, self.last
        )
    }
}

impl FromStr for Bearer {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Error> {
        let hex = |value: &str| {
            value
                .strip_prefix("0x")
                .and_then(|digits| u16::from_str_radix(digits, 16).ok())
        };
        source
            .split_once(": ")
            .and_then(|(member, range)| {
                let (first, last) = range.split_once("..=")?;
                Some(Self {
                    member: member.to_owned(),
                    first: hex(first)?,
                    last: hex(last)?,
                })
            })
            .filter(|bearer| bearer.to_string() == source)
            .ok_or_else(|| {
                Error::parse(format!(
                    "expected \"<member>: 0xEA00..=0xEAnn\", got {source:?}"
                ))
            })
    }
}

impl Command {
    /// The documented values of a parameter, or a return parameter if
    /// `returned`, in `release`.
    pub fn domain(&self, member: &str, returned: bool, release: Version) -> Option<&Domain> {
        self.domains
            .iter()
            .find(|domain| {
                domain.member == member
                    && domain.returned == returned
                    && domain.releases.contains(release)
            })
            .map(|domain| &domain.domain)
    }

    /// The name in the newest release that defines this command.
    pub fn name(&self) -> &str {
        latest_name(&self.names)
    }

    pub fn definition_at(&self, release: Version) -> Option<&CommandDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.releases.contains(release))
    }
}

impl Event {
    /// The documented values of a parameter in `release`.
    pub fn domain(&self, member: &str, release: Version) -> Option<&Domain> {
        self.domains
            .iter()
            .find(|domain| domain.member == member && domain.releases.contains(release))
            .map(|domain| &domain.domain)
    }

    pub fn name(&self) -> &str {
        latest_name(&self.names)
    }

    pub fn definition_at(&self, release: Version) -> Option<&EventDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.releases.contains(release))
    }
}

fn latest_name(names: &[Named]) -> &str {
    names
        .iter()
        .max_by_key(|named| named.releases.last)
        .map_or("", |named| named.name.as_str())
}

/// Profiles that support an entry in one release, if it exists there.
pub fn profiles_at(availability: &[Availability], release: Version) -> Option<&[Profile]> {
    availability
        .iter()
        .find(|entry| entry.releases.contains(release))
        .map(|entry| entry.profiles.as_slice())
}

impl Catalog {
    pub fn from_toml(source: &str) -> Result<Self, Error> {
        let catalog: Self =
            toml::from_str(source).map_err(|error| Error::parse(error.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn load(path: &Path) -> Result<Self, Error> {
        let source = std::fs::read_to_string(path)
            .map_err(|error| Error::parse(format!("could not read {}: {error}", path.display())))?;
        Self::from_toml(&source).map_err(|error| error.context(path.display()))
    }

    pub fn to_toml(&self) -> Result<String, Error> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|error| Error::parse(error.to_string()))
    }

    pub fn versions(&self) -> impl Iterator<Item = Version> + '_ {
        self.releases.iter().map(|release| release.version)
    }

    /// The stack profiles `release` has, empty if the catalog does not
    /// describe it.
    pub fn profiles_in(&self, release: Version) -> &[Profile] {
        match self
            .releases
            .iter()
            .find(|source| source.version == release)
        {
            Some(source) if source.profiles.is_empty() => self.platform.profiles(),
            Some(source) => &source.profiles,
            None => &[],
        }
    }

    /// Every `(release, profile)` target the catalog describes.
    pub fn targets(&self) -> impl Iterator<Item = (Version, Profile)> + '_ {
        self.versions().flat_map(move |release| {
            self.profiles_in(release)
                .iter()
                .map(move |profile| (release, *profile))
        })
    }

    pub fn command(&self, scope: CommandScope, opcode: u16) -> Option<&Command> {
        self.commands
            .iter()
            .find(|command| command.scope == scope && command.opcode == opcode)
    }

    pub fn event(&self, scope: EventScope, code: u16) -> Option<&Event> {
        self.events
            .iter()
            .find(|event| event.scope == scope && event.code == code)
    }

    /// The documented values of the field `member` of the structure
    /// `structure` in `release`.
    pub fn struct_domain(
        &self,
        structure: &str,
        member: &str,
        release: Version,
    ) -> Option<&Domain> {
        self.struct_domains
            .iter()
            .find(|domain| {
                domain.structure == structure
                    && domain.member == member
                    && domain.releases.contains(release)
            })
            .map(|domain| &domain.domain)
    }

    /// The fields of the structure `structure` as a command or event of
    /// `release` defines it.
    pub fn structure_at(&self, structure: &str, release: Version) -> Option<&[Field]> {
        let commands = self
            .commands
            .iter()
            .filter_map(move |command| command.definition_at(release))
            .map(|definition| &definition.structs);
        let events = self
            .events
            .iter()
            .filter_map(move |event| event.definition_at(release))
            .map(|definition| &definition.structs);
        commands
            .chain(events)
            .find_map(|structs| structs.get(structure))
            .map(Vec::as_slice)
    }

    /// The status codes `release` defines, by value.
    pub fn statuses_at(&self, release: Version) -> Vec<&StatusCode> {
        let mut statuses = self
            .statuses
            .iter()
            .filter(|status| status.releases.contains(release))
            .collect::<Vec<_>>();
        statuses.sort_by_key(|status| status.value);
        statuses
    }

    /// Find a command by any name it has carried.
    pub fn command_named(&self, name: &str) -> Option<&Command> {
        self.commands
            .iter()
            .find(|command| command.names.iter().any(|named| named.name == name))
    }

    pub fn event_named(&self, name: &str) -> Option<&Event> {
        self.events
            .iter()
            .find(|event| event.names.iter().any(|named| named.name == name))
    }

    /// Check every structural invariant of the catalog.
    pub fn validate(&self) -> Result<(), Error> {
        let versions = self.versions().collect::<Vec<_>>();
        if versions.is_empty() {
            return Err(Error::invalid("the catalog declares no releases"));
        }
        if versions.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Error::invalid(
                "releases must be unique and in ascending order",
            ));
        }
        let check_range = |range: ReleaseRange, context: &dyn Fn() -> String| {
            if versions.contains(&range.first) && versions.contains(&range.last) {
                Ok(())
            } else {
                Err(Error::invalid(format!(
                    "{}: release range {range} is not bounded by declared releases",
                    context()
                )))
            }
        };

        for release in &self.releases {
            let mut profiles = release.profiles.clone();
            profiles.sort();
            profiles.dedup();
            let valid = match self.platform {
                Platform::Stm32wb => profiles.is_empty(),
                Platform::Stm32wba => {
                    profiles == release.profiles
                        && profiles.contains(&self.platform.complete_profile())
                        && profiles
                            .iter()
                            .all(|profile| profile.platform() == self.platform)
                }
            };
            if !valid {
                return Err(Error::invalid(format!(
                    "release {}: invalid profiles {:?}; STM32WB lists none, and STM32WBA its \
                     own, sorted, unique, and with the complete one",
                    release.version, release.profiles
                )));
            }
        }
        let check_profiles = |availability: &[Availability], context: &dyn Fn() -> String| {
            for entry in availability {
                for release in versions
                    .iter()
                    .filter(|release| entry.releases.contains(**release))
                {
                    let profiles = self.profiles_in(*release);
                    if let Some(profile) = entry
                        .profiles
                        .iter()
                        .find(|profile| !profiles.contains(profile))
                    {
                        return Err(Error::invalid(format!(
                            "{}: release {release} has no {profile} profile",
                            context()
                        )));
                    }
                }
            }
            Ok(())
        };

        let mut binaries = BTreeSet::new();
        for binary in &self.binaries {
            check_range(binary.releases, &|| format!("binary {}", binary.file))?;
            if binary.profile.platform() != self.platform {
                return Err(Error::invalid(format!(
                    "binary {} is not a {:?} binary",
                    binary.file, self.platform
                )));
            }
            if Some(&binary.file) != binary.family.binary_file_name(binary.profile).as_ref() {
                return Err(Error::invalid(format!(
                    "binary {} does not match its {} {} identity",
                    binary.file, binary.family, binary.profile
                )));
            }
            if !binaries.insert((binary.family, binary.profile, binary.releases)) {
                return Err(Error::invalid(format!("duplicate binary {}", binary.file)));
            }
        }
        for (index, binary) in self.binaries.iter().enumerate() {
            for other in &self.binaries[index + 1..] {
                if binary.family == other.family
                    && binary.profile == other.profile
                    && binary.releases.overlaps(other.releases)
                {
                    return Err(Error::invalid(format!(
                        "binary {} has overlapping release ranges",
                        binary.file
                    )));
                }
            }
        }

        let mut identities = BTreeSet::new();
        for command in &self.commands {
            let label = || format!("{:?} command 0x{:04X}", command.scope, command.opcode);
            if !identities.insert((command.scope, command.opcode)) {
                return Err(Error::invalid(format!("duplicate {}", label())));
            }
            if matches!(command.scope, CommandScope::Vendor | CommandScope::System)
                && command.opcode >> 10 != 0x3F
            {
                return Err(Error::invalid(format!("{} is not in OGF 0x3F", label())));
            }
            let definitions = command
                .definitions
                .iter()
                .map(|definition| definition.releases)
                .collect::<Vec<_>>();
            validate_history(
                self.platform,
                &versions,
                &definitions,
                &command.names,
                &command.availability,
                &label,
            )?;
            check_profiles(&command.availability, &label)?;
            for range in definitions
                .iter()
                .chain(command.names.iter().map(|named| &named.releases))
                .chain(command.availability.iter().map(|entry| &entry.releases))
            {
                check_range(*range, &label)?;
            }
            for definition in &command.definitions {
                let context = || format!("{} ({})", label(), definition.releases);
                validate_layout(&definition.params, &definition.structs)
                    .map_err(|error| error.context(format!("{} params", context())))?;
                match (definition.completion, &definition.returns) {
                    (Completion::CommandStatus, Some(_)) => {
                        return Err(Error::invalid(format!(
                            "{}: Command Status commands have no return parameters",
                            context()
                        )));
                    }
                    (Completion::CommandComplete, None) => {
                        return Err(Error::invalid(format!(
                            "{}: Command Complete commands must declare return parameters",
                            context()
                        )));
                    }
                    (_, Some(returns)) => validate_layout(returns, &definition.structs)
                        .map_err(|error| error.context(format!("{} returns", context())))?,
                    (_, None) => {}
                }
                validate_bearers(&definition.params, &definition.bearers)
                    .map_err(|error| error.context(format!("{} params", context())))?;
            }
            validate_domains(&versions, &command.domains, &label, |release, returned| {
                let definition = command
                    .definitions
                    .iter()
                    .find(|definition| definition.releases.contains(release))?;
                if returned {
                    definition.returns.as_ref()
                } else {
                    Some(&definition.params)
                }
            })?;
        }

        let mut identities = BTreeSet::new();
        for event in &self.events {
            let label = || format!("{:?} event 0x{:04X}", event.scope, event.code);
            if !identities.insert((event.scope, event.code)) {
                return Err(Error::invalid(format!("duplicate {}", label())));
            }
            let definitions = event
                .definitions
                .iter()
                .map(|definition| definition.releases)
                .collect::<Vec<_>>();
            validate_history(
                self.platform,
                &versions,
                &definitions,
                &event.names,
                &event.availability,
                &label,
            )?;
            check_profiles(&event.availability, &label)?;
            for range in definitions
                .iter()
                .chain(event.names.iter().map(|named| &named.releases))
                .chain(event.availability.iter().map(|entry| &entry.releases))
            {
                check_range(*range, &label)?;
            }
            for definition in &event.definitions {
                let context = || format!("{} ({}) payload", label(), definition.releases);
                validate_layout(&definition.payload, &definition.structs)
                    .map_err(|error| error.context(context()))?;
                validate_bearers(&definition.payload, &definition.bearers)
                    .map_err(|error| error.context(context()))?;
            }
            validate_domains(&versions, &event.domains, &label, |release, returned| {
                (!returned).then_some(())?;
                event
                    .definitions
                    .iter()
                    .find(|definition| definition.releases.contains(release))
                    .map(|definition| &definition.payload)
            })?;
        }

        for (index, domain) in self.struct_domains.iter().enumerate() {
            let label = || {
                format!(
                    "{}.{} ({}) domain",
                    domain.structure, domain.member, domain.releases
                )
            };
            check_range(domain.releases, &label)?;
            if self.struct_domains[..index].iter().any(|other| {
                other.structure == domain.structure
                    && other.member == domain.member
                    && other.releases.overlaps(domain.releases)
            }) {
                return Err(Error::invalid(format!("{}: overlaps another", label())));
            }
            let member = MemberDomain {
                releases: domain.releases,
                member: domain.member.clone(),
                returned: false,
                domain: domain.domain.clone(),
            };
            for &release in versions
                .iter()
                .filter(|release| domain.releases.contains(**release))
            {
                let fields = self
                    .structure_at(&domain.structure, release)
                    .ok_or_else(|| {
                        Error::invalid(format!(
                            "{}: no command or event of {release} carries {}",
                            label(),
                            domain.structure
                        ))
                    })?;
                domain::validate_domain(&Layout::Fields(fields.to_vec()), &member)
                    .map_err(|error| error.context(label()))?;
            }
        }
        for status in &self.statuses {
            check_range(status.releases, &|| format!("status {}", status.name))?;
        }
        for release in &versions {
            let statuses = self.statuses_at(*release);
            let mut names = BTreeSet::new();
            for (index, status) in statuses.iter().enumerate() {
                if !names.insert(&status.name)
                    || statuses[index + 1..]
                        .iter()
                        .any(|other| other.value == status.value)
                {
                    return Err(Error::invalid(format!(
                        "{release}: status {} (0x{:02X}) is defined twice",
                        status.name, status.value
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Definitions, names, and availability must each partition exactly the same
/// set of releases: an entry is named and documented wherever it is defined.
fn validate_history(
    platform: Platform,
    versions: &[Version],
    definitions: &[ReleaseRange],
    names: &[Named],
    availability: &[Availability],
    label: &dyn Fn() -> String,
) -> Result<(), Error> {
    if definitions.is_empty() {
        return Err(Error::invalid(format!("{} has no definitions", label())));
    }
    let names_ranges = names.iter().map(|named| named.releases).collect::<Vec<_>>();
    let availability_ranges = availability
        .iter()
        .map(|entry| entry.releases)
        .collect::<Vec<_>>();
    let covered = |ranges: &[ReleaseRange], what: &str| -> Result<Vec<Version>, Error> {
        for (index, range) in ranges.iter().enumerate() {
            if ranges[index + 1..]
                .iter()
                .any(|other| range.overlaps(*other))
            {
                return Err(Error::invalid(format!(
                    "{}: overlapping {what} ranges",
                    label()
                )));
            }
        }
        Ok(versions
            .iter()
            .copied()
            .filter(|version| ranges.iter().any(|range| range.contains(*version)))
            .collect())
    };
    let defined = covered(definitions, "definition")?;
    if covered(&names_ranges, "name")? != defined {
        return Err(Error::invalid(format!(
            "{}: names do not cover exactly the defined releases",
            label()
        )));
    }
    if covered(&availability_ranges, "availability")? != defined {
        return Err(Error::invalid(format!(
            "{}: availability does not cover exactly the defined releases",
            label()
        )));
    }
    if names.iter().any(|named| named.name.is_empty()) {
        return Err(Error::invalid(format!("{} has an empty name", label())));
    }
    for entry in availability {
        if let Some(profile) = entry
            .profiles
            .iter()
            .find(|profile| profile.platform() != platform)
        {
            return Err(Error::invalid(format!(
                "{}: {profile} is not a {platform:?} profile",
                label()
            )));
        }
        let mut profiles = entry.profiles.clone();
        profiles.sort();
        profiles.dedup();
        if profiles != entry.profiles {
            return Err(Error::invalid(format!(
                "{}: availability profiles must be sorted and unique",
                label()
            )));
        }
    }
    Ok(())
}

/// Each domain applies to releases with a layout holding its member, valid
/// for every one of them, and no two domains of a member overlap.
fn validate_domains<'a>(
    versions: &[Version],
    domains: &[MemberDomain],
    label: &dyn Fn() -> String,
    layout: impl Fn(Version, bool) -> Option<&'a Layout>,
) -> Result<(), Error> {
    for (index, domain) in domains.iter().enumerate() {
        let context = || format!("{} ({}) domain", label(), domain.releases);
        if domains[..index].iter().any(|other| {
            other.member == domain.member
                && other.returned == domain.returned
                && other.releases.overlaps(domain.releases)
        }) {
            return Err(Error::invalid(format!(
                "{}: {} has overlapping domains",
                context(),
                domain.member
            )));
        }
        for &release in versions
            .iter()
            .filter(|release| domain.releases.contains(**release))
        {
            let layout = layout(release, domain.returned).ok_or_else(|| {
                Error::invalid(format!(
                    "{}: {} is not defined in {release}",
                    context(),
                    domain.member
                ))
            })?;
            domain::validate_domain(layout, domain).map_err(|error| error.context(context()))?;
        }
    }
    Ok(())
}

/// Each bearer is a distinct `u16` member of a resolved layout, with an
/// enhanced range of channel indexes after 0xEA00.
/// Each bearer member is listed once, with the enhanced bearers' range, and
/// is a `u16` member of the layout. The bearers of an unresolved layout are
/// checked against the annotation supplying it.
pub(crate) fn validate_bearers(layout: &Layout, bearers: &[Bearer]) -> Result<(), Error> {
    for (index, bearer) in bearers.iter().enumerate() {
        let member = &bearer.member;
        if bearers[..index].iter().any(|other| other.member == *member) {
            return Err(Error::invalid(format!("bearer {member} is listed twice")));
        }
        if let Layout::Fields(fields) = layout
            && !fields
                .iter()
                .any(|field| field.name == *member && field.ty == FieldType::Scalar(Scalar::U16))
        {
            return Err(Error::invalid(format!(
                "bearer {member} is not a u16 member"
            )));
        }
        if bearer.first != 0xEA00 || !(bearer.first..=0xEAFF).contains(&bearer.last) {
            return Err(Error::invalid(format!(
                "bearer {member} has the range 0x{:04X}..=0x{:04X}, not 0xEA00 and channel indexes",
                bearer.first, bearer.last
            )));
        }
    }
    Ok(())
}

fn validate_layout(layout: &Layout, structs: &Structs) -> Result<(), Error> {
    layout.envelope(structs)?;
    for (name, fields) in structs {
        layout::struct_width(name, structs)?;
        if fields.is_empty() {
            return Err(Error::invalid(format!("structure {name} is empty")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
