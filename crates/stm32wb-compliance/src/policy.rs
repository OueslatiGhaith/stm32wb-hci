//! Declarative exclusion-policy loading and semantic validation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use stm32wb_compliance::{CheckReport, EnvelopeEvidence, FirmwareVersion};

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
    external_event_payload: Option<EnvelopeEvidence>,
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
    payload: Option<PayloadDocument>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PolicyScope {
    Command,
    Event,
    TransportEvent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDocument {
    minimum: u32,
    maximum: u32,
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

            let (kind, external_event_payload) = match entry.scope {
                PolicyScope::Command => {
                    reject_payload(&path, index, entry.payload.as_ref(), "command")?;
                    (CoverageKind::Command, None)
                }
                PolicyScope::Event => {
                    reject_payload(&path, index, entry.payload.as_ref(), "event")?;
                    (CoverageKind::Event, None)
                }
                PolicyScope::TransportEvent => {
                    let payload = entry.payload.ok_or_else(|| {
                        policy_error(
                            &path,
                            index,
                            "transport-event exclusions require a payload envelope",
                        )
                    })?;
                    (
                        CoverageKind::Event,
                        Some(validate_event_payload(&path, index, payload)?),
                    )
                }
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
                external_event_payload,
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
                    if let Some(payload) = &entry.external_event_payload {
                        active
                            .external_event_payloads
                            .insert(entry.code, payload.clone());
                    }
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

fn reject_payload(
    path: &Path,
    index: usize,
    payload: Option<&PayloadDocument>,
    scope: &str,
) -> Result<(), String> {
    if payload.is_some() {
        Err(policy_error(
            path,
            index,
            &format!("{scope} exclusions cannot declare a payload envelope"),
        ))
    } else {
        Ok(())
    }
}

fn validate_event_payload(
    path: &Path,
    index: usize,
    payload: PayloadDocument,
) -> Result<EnvelopeEvidence, String> {
    const MAX_VENDOR_EVENT_PAYLOAD: u32 = u8::MAX as u32 - 2;

    if payload.minimum > payload.maximum {
        return Err(policy_error(
            path,
            index,
            "payload envelope requires minimum <= maximum",
        ));
    }
    if payload.maximum > MAX_VENDOR_EVENT_PAYLOAD {
        return Err(policy_error(
            path,
            index,
            &format!(
                "payload maximum {} exceeds the {MAX_VENDOR_EVENT_PAYLOAD}-byte vendor-event envelope",
                payload.maximum
            ),
        ));
    }
    Ok(EnvelopeEvidence::known(payload.minimum, payload.maximum))
}

fn policy_error(path: &Path, index: usize, message: &str) -> String {
    format!("{}: exclusion {index}: {message}", path.display())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActiveExclusions {
    pub(crate) commands: BTreeMap<u16, String>,
    pub(crate) events: BTreeMap<u16, String>,
    pub(crate) external_event_payloads: BTreeMap<u16, EnvelopeEvidence>,
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
            FirmwareVersion::new(0, 15, 0),
            FirmwareVersion::new(0, 16, 0),
        ]
    }

    #[test]
    fn expands_selectors_and_allows_pipes_in_reasons() {
        let policy = ExclusionPolicy::parse(
            "test.toml".into(),
            r#"
                [[exclusions]]
                scope = "transport-event"
                code = 0x9200
                firmware = "*"
                reason = "transport | event"
                payload = { minimum = 1, maximum = 1 }

                [[exclusions]]
                scope = "transport-event"
                code = 0x9201
                firmware = "0.15.0"
                reason = "bounded transport event"
                payload = { minimum = 1, maximum = 3 }

                [[exclusions]]
                scope = "command"
                code = 0x0001
                firmware = "0.15.0"
                reason = "legacy command"
            "#,
        )
        .unwrap();
        policy.validate_for(&supported()).unwrap();

        let old = policy.active_for(FirmwareVersion::new(0, 15, 0));
        assert_eq!(
            old.events.get(&0x9200),
            Some(&"transport | event".to_owned())
        );
        assert_eq!(
            old.external_event_payloads.get(&0x9200),
            Some(&EnvelopeEvidence::fixed(1))
        );
        assert_eq!(
            old.external_event_payloads.get(&0x9201),
            Some(&EnvelopeEvidence::known(1, 3))
        );
        assert_eq!(old.commands.get(&1), Some(&"legacy command".to_owned()));

        let new = policy.active_for(FirmwareVersion::new(0, 16, 0));
        assert!(!new.commands.contains_key(&1));
        assert!(!new.external_event_payloads.contains_key(&0x9201));
    }

    #[test]
    fn rejects_overlaps_unknown_versions_and_invalid_payloads() {
        let overlapping = ExclusionPolicy::parse(
            "test.toml".into(),
            r#"
                [[exclusions]]
                scope = "transport-event"
                code = 0x9200
                firmware = "*"
                reason = "transport event"
                payload = { minimum = 1, maximum = 1 }
                [[exclusions]]
                scope = "event"
                code = 0x9200
                firmware = "0.15.0"
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

        for payload in [
            "{ minimum = 2, maximum = 1 }",
            "{ minimum = 1, maximum = 254 }",
        ] {
            let source = format!(
                r#"
                    [[exclusions]]
                    scope = "transport-event"
                    code = 0x9200
                    firmware = "*"
                    reason = "transport event"
                    payload = {payload}
                "#
            );
            assert!(ExclusionPolicy::parse("test.toml".into(), &source).is_err());
        }
    }

    #[test]
    fn rejects_missing_payloads_and_bad_toml() {
        let missing_payload = r#"
            [[exclusions]]
            scope = "transport-event"
            code = 0x9200
            firmware = "*"
            reason = "transport event"
        "#;
        assert!(ExclusionPolicy::parse("test.toml".into(), missing_payload).is_err());
        assert!(ExclusionPolicy::parse("test.toml".into(), "exclusions = [").is_err());
    }
}
