//! Declarative exclusion-policy loading and semantic validation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use stm32wb_compliance::{CheckReport, FirmwareVersion};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CoverageKind {
    Command,
    Event,
}

impl CoverageKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Event => "event",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FirmwareSelector {
    All,
    Only(FirmwareVersion),
}

impl FirmwareSelector {
    fn parse(value: &str) -> Result<Self, String> {
        if value == "*" {
            Ok(Self::All)
        } else {
            FirmwareVersion::from_str(value)
                .map(Self::Only)
                .map_err(|error| error.to_string())
        }
    }

    fn matches(self, firmware: FirmwareVersion) -> bool {
        match self {
            Self::All => true,
            Self::Only(selected) => selected == firmware,
        }
    }
}

#[derive(Clone, Debug)]
struct PolicyEntry {
    kind: CoverageKind,
    code: u16,
    selector: FirmwareSelector,
    reason: String,
    index: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ExclusionPolicy {
    path: PathBuf,
    entries: Vec<PolicyEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDocument {
    #[serde(default)]
    exclusions: Vec<PolicyEntryDocument>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyEntryDocument {
    scope: PolicyScope,
    code: u16,
    firmware: String,
    reason: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PolicyScope {
    Command,
    Event,
}

impl ExclusionPolicy {
    pub(crate) fn load(path: PathBuf) -> Result<Self, String> {
        let source = fs::read_to_string(&path).map_err(|error| {
            format!(
                "could not read exclusion policy {}: {error}",
                path.display()
            )
        })?;
        Self::parse(path, &source)
    }

    fn parse(path: PathBuf, source: &str) -> Result<Self, String> {
        let document = toml::from_str::<PolicyDocument>(source).map_err(|error| {
            format!(
                "could not parse exclusion policy {} as TOML: {error}",
                path.display()
            )
        })?;
        let mut entries = Vec::with_capacity(document.exclusions.len());
        let mut raw_entries = BTreeSet::new();
        for (offset, entry) in document.exclusions.into_iter().enumerate() {
            let index = offset + 1;
            let selector = FirmwareSelector::parse(&entry.firmware).map_err(|error| {
                policy_error(&path, index, &format!("invalid firmware selector: {error}"))
            })?;
            if entry.reason.trim().is_empty() {
                return Err(policy_error(
                    &path,
                    index,
                    "exclusion reason must not be empty",
                ));
            }

            let kind = match entry.scope {
                PolicyScope::Command => CoverageKind::Command,
                PolicyScope::Event => CoverageKind::Event,
            };
            if !raw_entries.insert((kind, entry.code, selector)) {
                return Err(policy_error(
                    &path,
                    index,
                    "this scope, wire code, and firmware selector are declared more than once",
                ));
            }
            entries.push(PolicyEntry {
                kind,
                code: entry.code,
                selector,
                reason: entry.reason,
                index,
            });
        }

        Ok(Self { path, entries })
    }

    /// Reject stale exact versions and selectors which overlap after expansion.
    pub(crate) fn validate_for(&self, declared: &[FirmwareVersion]) -> Result<(), String> {
        let mut expanded = BTreeMap::<(CoverageKind, u16, FirmwareVersion), usize>::new();
        for entry in &self.entries {
            if let FirmwareSelector::Only(firmware) = entry.selector
                && !declared.contains(&firmware)
            {
                return Err(policy_error(
                    &self.path,
                    entry.index,
                    &format!(
                        "firmware selector {firmware} is not declared by this crate's [features] table"
                    ),
                ));
            }
            for firmware in declared
                .iter()
                .copied()
                .filter(|firmware| entry.selector.matches(*firmware))
            {
                let key = (entry.kind, entry.code, firmware);
                if let Some(previous) = expanded.insert(key, entry.index) {
                    return Err(policy_error(
                        &self.path,
                        entry.index,
                        &format!(
                            "overlaps exclusion {previous}: {} 0x{:04X} would be excluded twice for firmware {firmware}",
                            entry.kind.as_str(),
                            entry.code
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn active_for(&self, firmware: FirmwareVersion) -> ActiveExclusions {
        let mut active = ActiveExclusions::default();
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.selector.matches(firmware))
        {
            match entry.kind {
                CoverageKind::Command => {
                    active.commands.insert(entry.code, entry.reason.clone());
                }
                CoverageKind::Event => {
                    active.events.insert(entry.code, entry.reason.clone());
                }
            }
        }
        active
    }

    pub(crate) fn display_path(&self, root: &Path) -> String {
        self.path.strip_prefix(root).map_or_else(
            |_| self.path.display().to_string(),
            |path| path.display().to_string(),
        )
    }

    pub(crate) fn entry_count(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn empty(path: PathBuf) -> Self {
        Self {
            path,
            entries: Vec::new(),
        }
    }
}

fn policy_error(path: &Path, index: usize, message: &str) -> String {
    format!("{}: exclusion {index}: {message}", path.display())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActiveExclusions {
    pub(crate) commands: BTreeMap<u16, String>,
    pub(crate) events: BTreeMap<u16, String>,
}

impl ActiveExclusions {
    pub(crate) fn audit(
        &self,
        report: &CheckReport,
        firmware: FirmwareVersion,
    ) -> Result<PolicyAudit, String> {
        let reported_commands = report
            .excluded_commands()
            .iter()
            .map(|entry| (entry.code, entry.reason.clone()))
            .collect::<BTreeMap<_, _>>();
        let reported_events = report
            .excluded_events()
            .iter()
            .map(|entry| (entry.code, entry.reason.clone()))
            .collect::<BTreeMap<_, _>>();
        if reported_commands != self.commands || reported_events != self.events {
            return Err(format!(
                "checker exclusions for firmware {firmware} do not match the active exclusion policy"
            ));
        }

        audit_exclusion_codes(
            CoverageKind::Command,
            &self.commands,
            &report.vendor().command_codes(),
            &report.active_api().command_codes(),
            firmware,
        )?;
        audit_exclusion_codes(
            CoverageKind::Event,
            &self.events,
            &report.vendor().event_codes(),
            &report.active_api().event_codes(),
            firmware,
        )?;
        Ok(PolicyAudit {
            command_entries: self.commands.len(),
            event_entries: self.events.len(),
        })
    }
}

fn audit_exclusion_codes(
    kind: CoverageKind,
    exclusions: &BTreeMap<u16, String>,
    expected: &BTreeSet<u16>,
    observed: &BTreeSet<u16>,
    firmware: FirmwareVersion,
) -> Result<(), String> {
    for code in exclusions.keys() {
        if expected.contains(code) == observed.contains(code) {
            return Err(format!(
                "exclusion policy for {} 0x{code:04X} on firmware {firmware} is stale: it no longer suppresses a coverage difference",
                kind.as_str(),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PolicyAudit {
    pub(crate) command_entries: usize,
    pub(crate) event_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supported() -> Vec<FirmwareVersion> {
        vec![
            FirmwareVersion::new(1, 15, 0),
            FirmwareVersion::new(1, 16, 0),
        ]
    }

    #[test]
    fn expands_selectors_and_allows_pipes_in_reasons() {
        let policy = ExclusionPolicy::parse(
            "test.toml".into(),
            r#"
                [[exclusions]]
                scope = "event"
                code = 0x9200
                firmware = "*"
                reason = "system | event"

                [[exclusions]]
                scope = "event"
                code = 0x9201
                firmware = "1.15.0"
                reason = "version-specific system event"

                [[exclusions]]
                scope = "command"
                code = 0x0001
                firmware = "1.15.0"
                reason = "legacy command"
            "#,
        )
        .unwrap();
        policy.validate_for(&supported()).unwrap();

        let old = policy.active_for(FirmwareVersion::new(1, 15, 0));
        assert_eq!(old.events.get(&0x9200), Some(&"system | event".to_owned()));
        assert!(old.events.contains_key(&0x9201));
        assert_eq!(old.commands.get(&1), Some(&"legacy command".to_owned()));

        let new = policy.active_for(FirmwareVersion::new(1, 16, 0));
        assert!(!new.commands.contains_key(&1));
        assert!(!new.events.contains_key(&0x9201));
    }

    #[test]
    fn rejects_overlaps_and_unknown_versions() {
        let overlapping = ExclusionPolicy::parse(
            "test.toml".into(),
            r#"
                [[exclusions]]
                scope = "event"
                code = 0x9200
                firmware = "*"
                reason = "system event"
                [[exclusions]]
                scope = "event"
                code = 0x9200
                firmware = "1.15.0"
                reason = "same event"
            "#,
        )
        .unwrap();
        assert!(overlapping.validate_for(&supported()).is_err());

        let unknown = ExclusionPolicy::parse(
            "test.toml".into(),
            r#"
                [[exclusions]]
                scope = "event"
                code = 0x9200
                firmware = "0.99.0"
                reason = "future event"
            "#,
        )
        .unwrap();
        assert!(unknown.validate_for(&supported()).is_err());
    }

    #[test]
    fn rejects_retired_transport_scope_payloads_and_bad_toml() {
        let transport_scope = r#"
            [[exclusions]]
            scope = "transport-event"
            code = 0x9200
            firmware = "*"
            reason = "transport event"
        "#;
        assert!(ExclusionPolicy::parse("test.toml".into(), transport_scope).is_err());

        let payload = r#"
            [[exclusions]]
            scope = "event"
            code = 0x9200
            firmware = "*"
            reason = "system event"
            payload = { minimum = 1, maximum = 1 }
        "#;
        assert!(ExclusionPolicy::parse("test.toml".into(), payload).is_err());
        assert!(ExclusionPolicy::parse("test.toml".into(), "exclusions = [").is_err());
    }
}
