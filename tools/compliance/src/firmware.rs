use core::fmt;
use core::str::FromStr;
use std::fs;
use std::path::Path;

/// A BLE coprocessor firmware version.
///
/// STM32CubeWB tags use a different major version from the BLE firmware: firmware
/// `0.15.0` is generated from CubeWB tag `v1.15.0`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FirmwareVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl FirmwareVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// The Cargo feature which selects this firmware API.
    pub fn feature_name(self) -> String {
        format!("fw_{}_{}_{}", self.major, self.minor, self.patch)
    }

    /// Parse a Cargo feature which selects a firmware API.
    ///
    /// Feature names are deliberately parsed independently of the CubeWB tag
    /// spelling. This is used by the checker to enumerate the crate's source
    /// of truth (`[features]` in `Cargo.toml`) instead of maintaining a second
    /// list of supported versions.
    pub fn from_feature_name(feature: &str) -> Result<Self, FirmwareFeatureError> {
        let version = feature.strip_prefix("fw_").ok_or_else(|| {
            FirmwareFeatureError(format!(
                "expected a firmware feature named `fw_<major>_<minor>_<patch>`, got {feature:?}"
            ))
        })?;
        let mut components = version.split('_');
        let major = parse_feature_component(components.next(), feature)?;
        let minor = parse_feature_component(components.next(), feature)?;
        let patch = parse_feature_component(components.next(), feature)?;
        if components.next().is_some() {
            return Err(FirmwareFeatureError(format!(
                "expected a firmware feature named `fw_<major>_<minor>_<patch>`, got {feature:?}"
            )));
        }

        let version = Self::new(major, minor, patch);
        if feature != version.feature_name() {
            return Err(FirmwareFeatureError(format!(
                "firmware feature {feature:?} is not canonical; use `{}`",
                version.feature_name()
            )));
        }
        Ok(version)
    }

    /// Discover all firmware features declared by the target crate.
    ///
    /// The checker intentionally reads the same `[features]` table that Cargo
    /// and `build.rs` use. Adding a new `fw_*` feature therefore makes it
    /// visible to `check --all-supported` and CI automatically.
    pub fn declared_in_manifest(crate_dir: &Path) -> Result<Vec<Self>, FirmwareManifestError> {
        let manifest_path = crate_dir.join("Cargo.toml");
        let manifest = fs::read_to_string(&manifest_path).map_err(|error| {
            FirmwareManifestError(format!(
                "could not read crate manifest at {}: {error}",
                manifest_path.display()
            ))
        })?;
        firmware_versions_from_manifest(&manifest).map_err(|error| {
            FirmwareManifestError(format!(
                "invalid firmware features in {}: {error}",
                manifest_path.display()
            ))
        })
    }

    /// The corresponding STM32CubeWB release tag.
    pub fn cube_tag(self) -> String {
        if self.major == 0 {
            format!("v1.{}.{}", self.minor, self.patch)
        } else {
            format!("v{}.{}.{}", self.major, self.minor, self.patch)
        }
    }

    /// Returns whether a custom version cfg is active for this firmware.
    ///
    /// Both the documented `*_fw_0_15_0` spelling and the shorter historical
    /// `*_0_15_0` spelling are accepted to keep the checker useful while the
    /// feature-gating convention evolves.
    pub fn matches_version_cfg(self, cfg: &str) -> Option<bool> {
        let (relation, value) = if let Some(value) = cfg.strip_prefix("before_fw_") {
            ("before", value)
        } else if let Some(value) = cfg.strip_prefix("only_fw_") {
            ("only", value)
        } else if let Some(value) = cfg.strip_prefix("since_fw_") {
            ("since", value)
        } else if let Some(value) = cfg.strip_prefix("before_") {
            ("before", value)
        } else if let Some(value) = cfg.strip_prefix("only_") {
            ("only", value)
        } else if let Some(value) = cfg.strip_prefix("since_") {
            ("since", value)
        } else {
            return None;
        };

        let version = value.replace('_', ".").parse().ok()?;
        Some(match relation {
            "before" => self < version,
            "only" => self == version,
            "since" => self >= version,
            _ => unreachable!("the relation is selected above"),
        })
    }
}

fn parse_feature_component(
    component: Option<&str>,
    feature: &str,
) -> Result<u16, FirmwareFeatureError> {
    component
        .filter(|component| !component.is_empty())
        .ok_or_else(|| {
            FirmwareFeatureError(format!(
                "expected a firmware feature named `fw_<major>_<minor>_<patch>`, got {feature:?}"
            ))
        })?
        .parse()
        .map_err(|_| {
            FirmwareFeatureError(format!(
                "expected a firmware feature named `fw_<major>_<minor>_<patch>`, got {feature:?}"
            ))
        })
}

fn firmware_versions_from_manifest(manifest: &str) -> Result<Vec<FirmwareVersion>, String> {
    let document = manifest
        .parse::<toml::Table>()
        .map_err(|error| format!("invalid TOML: {error}"))?;
    let feature_table = document
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "missing [features] table".to_owned())?;

    let mut features = feature_table
        .keys()
        .filter(|name| name.starts_with("fw_"))
        .map(|name| FirmwareVersion::from_feature_name(name).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    if features.is_empty() {
        return Err("no `fw_<major>_<minor>_<patch>` features were found in [features]".into());
    }

    features.sort();
    for pair in features.windows(2) {
        if pair[0] == pair[1] {
            return Err(format!(
                "firmware feature `{}` is declared more than once",
                pair[0].feature_name()
            ));
        }
    }
    Ok(features)
}

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for FirmwareVersion {
    type Err = FirmwareVersionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim().strip_prefix('v').unwrap_or(value.trim());
        let mut parts = value.split('.');
        let major = parse_part(parts.next(), value)?;
        let minor = parse_part(parts.next(), value)?;
        let patch = parse_part(parts.next(), value)?;
        if parts.next().is_some() {
            return Err(FirmwareVersionError(value.to_owned()));
        }

        // The crate historically used 0.x.y for the BLE firmware while ST tags
        // use v1.x.y. Accept either spelling at the command line, but retain
        // the crate spelling internally so feature lookup is unambiguous.
        Ok(Self::new(if major == 1 { 0 } else { major }, minor, patch))
    }
}

fn parse_part(part: Option<&str>, whole: &str) -> Result<u16, FirmwareVersionError> {
    part.ok_or_else(|| FirmwareVersionError(whole.to_owned()))?
        .parse()
        .map_err(|_| FirmwareVersionError(whole.to_owned()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmwareVersionError(String);

impl fmt::Display for FirmwareVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "expected a semantic firmware version such as 0.15.0, got {:?}",
            self.0
        )
    }
}

impl std::error::Error for FirmwareVersionError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmwareFeatureError(String);

impl fmt::Display for FirmwareFeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FirmwareFeatureError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmwareManifestError(String);

impl fmt::Display for FirmwareManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FirmwareManifestError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_firmware_to_cube_tag_and_feature() {
        let version: FirmwareVersion = "0.15.0".parse().unwrap();
        assert_eq!(version.cube_tag(), "v1.15.0");
        assert_eq!(version.feature_name(), "fw_0_15_0");
    }

    #[test]
    fn version_cfgs_are_compared_numerically() {
        let version = FirmwareVersion::new(0, 15, 0);
        assert_eq!(version.matches_version_cfg("before_fw_0_16_0"), Some(true));
        assert_eq!(version.matches_version_cfg("only_fw_0_15_0"), Some(true));
        assert_eq!(version.matches_version_cfg("since_0_14_1"), Some(true));
        assert_eq!(version.matches_version_cfg("since_fw_0_15_0"), Some(true));
        assert_eq!(version.matches_version_cfg("since_fw_0_15_1"), Some(false));
    }

    #[test]
    fn accepts_cube_tag_version_spelling() {
        let version: FirmwareVersion = "v1.15.0".parse().unwrap();
        assert_eq!(version, FirmwareVersion::new(0, 15, 0));
        assert_eq!(version.feature_name(), "fw_0_15_0");
    }

    #[test]
    fn discovers_and_sorts_declared_firmware_features() {
        let versions = firmware_versions_from_manifest(
            r#"
                [package]
                name = "not-a-feature"

                [features] # generated firmware APIs
                default = ["fw_0_17_1"]
                fw_0_17_1 = []
                fw_0_9_0 = []
                fw_0_15_0 = []

                [dependencies]
                fw_9_9_9 = "not a feature"
            "#,
        )
        .unwrap();

        assert_eq!(
            versions,
            vec![
                FirmwareVersion::new(0, 9, 0),
                FirmwareVersion::new(0, 15, 0),
                FirmwareVersion::new(0, 17, 1),
            ]
        );
    }

    #[test]
    fn rejects_invalid_toml_instead_of_attempting_line_based_recovery() {
        let error = firmware_versions_from_manifest("[features\nfw_0_15_0 = []").unwrap_err();
        assert!(error.starts_with("invalid TOML:"));
    }

    #[test]
    fn rejects_noncanonical_or_malformed_feature_names() {
        assert!(FirmwareVersion::from_feature_name("fw_0_015_0").is_err());
        assert!(FirmwareVersion::from_feature_name("fw_0_15").is_err());
        assert!(FirmwareVersion::from_feature_name("not_fw_0_15_0").is_err());
    }
}
