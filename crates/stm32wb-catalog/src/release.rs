//! Cube release identities and contiguous release ranges.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Error;

/// One STM32CubeWB release, such as `1.17.1`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// The Git tag STMicroelectronics uses for this release.
    pub fn cube_tag(self) -> String {
        format!("v{self}")
    }

    /// The Cargo feature selecting this release, e.g. `fw_1_17_1`.
    pub fn feature_name(self) -> String {
        format!("fw_{}_{}_{}", self.major, self.minor, self.patch)
    }

    /// Parse a `fw_<major>_<minor>_<patch>` Cargo feature name.
    pub fn from_feature_name(feature: &str) -> Option<Self> {
        let mut parts = feature.strip_prefix("fw_")?.split('_');
        let version = Self::new(
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        );
        parts.next().is_none().then_some(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        let digits = value.strip_prefix('v').unwrap_or(value);
        let mut parts = digits.split('.');
        let mut next = || {
            parts
                .next()
                .and_then(|part| part.parse::<u16>().ok())
                .ok_or_else(|| Error::parse(format!("invalid Cube release {value:?}")))
        };
        let version = Self::new(next()?, next()?, next()?);
        if parts.next().is_some() {
            return Err(Error::parse(format!("invalid Cube release {value:?}")));
        }
        Ok(version)
    }
}

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// An inclusive range of releases, interpreted against the catalog's ordered
/// release list. Ranges never imply releases the catalog does not declare.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleaseRange {
    pub first: Version,
    pub last: Version,
}

impl ReleaseRange {
    pub const fn single(version: Version) -> Self {
        Self {
            first: version,
            last: version,
        }
    }

    pub fn contains(self, version: Version) -> bool {
        self.first <= version && version <= self.last
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.first <= other.last && other.first <= self.last
    }
}

impl fmt::Display for ReleaseRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.first == self.last {
            write!(f, "{}", self.first)
        } else {
            write!(f, "{}..={}", self.first, self.last)
        }
    }
}

impl FromStr for ReleaseRange {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        let range = match value.split_once("..=") {
            Some((first, last)) => Self {
                first: first.trim().parse()?,
                last: last.trim().parse()?,
            },
            None => Self::single(value.trim().parse()?),
        };
        if range.first > range.last {
            return Err(Error::parse(format!("empty release range {value:?}")));
        }
        Ok(range)
    }
}

impl Serialize for ReleaseRange {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ReleaseRange {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_round_trip_through_tags_and_features() {
        let version: Version = "v1.17.1".parse().unwrap();
        assert_eq!(version, Version::new(1, 17, 1));
        assert_eq!(version.cube_tag(), "v1.17.1");
        assert_eq!(version.feature_name(), "fw_1_17_1");
        assert_eq!(Version::from_feature_name("fw_1_17_1"), Some(version));
        assert_eq!(Version::from_feature_name("fw_1_17"), None);
        assert!("1.17".parse::<Version>().is_err());
        assert!(Version::new(1, 9, 0) < Version::new(1, 17, 0));
    }

    #[test]
    fn ranges_round_trip() {
        let range: ReleaseRange = "1.15.0..=1.24.0".parse().unwrap();
        assert_eq!(range.to_string(), "1.15.0..=1.24.0");
        assert!(range.contains(Version::new(1, 17, 1)));
        assert!(!range.contains(Version::new(1, 14, 0)));
        let single: ReleaseRange = "1.17.1".parse().unwrap();
        assert_eq!(single.to_string(), "1.17.1");
        assert!("1.24.0..=1.15.0".parse::<ReleaseRange>().is_err());
    }
}
