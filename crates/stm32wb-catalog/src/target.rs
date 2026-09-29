//! Wireless-binary target dimensions.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Error, Platform, Version};

/// A BLE stack profile, i.e. which variant of the BLE stack runs: on
/// STM32WB, the CPU2 wireless binary; on STM32WBA, the configuration of the
/// stack library linked into the application.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// The complete interface (`BLE_Stack_full_extended`).
    FullExtended,
    /// Basic Features (`BLE_Stack_full`), column BF.
    Full,
    /// Peripheral Only (`BLE_Stack_light`), column PO.
    Light,
    /// Link Layer Only (`BLE_HCILayer_extended`), column LO.
    HciLayerExtended,
    /// Link Layer Only Basic (`BLE_HCILayer`), column LB.
    HciLayer,
    /// Beacon Only (`BLE_HCI_AdvScan`), column BO.
    HciAdvScan,
    /// STM32WBA's Full configuration: the complete interface.
    WbaFull,
    /// STM32WBA's Basic Plus configuration, column BP.
    WbaBasicPlus,
    /// STM32WBA's Basic Features configuration, column BF.
    WbaBasicFeatures,
    /// STM32WBA's Peripheral Only configuration, column PO.
    WbaPeripheralOnly,
    /// STM32WBA's Link Layer Only configuration, column LO.
    WbaLinkLayerOnly,
}

impl Profile {
    pub const ALL: [Self; 11] = [
        Self::FullExtended,
        Self::Full,
        Self::Light,
        Self::HciLayerExtended,
        Self::HciLayer,
        Self::HciAdvScan,
        Self::WbaFull,
        Self::WbaBasicPlus,
        Self::WbaBasicFeatures,
        Self::WbaPeripheralOnly,
        Self::WbaLinkLayerOnly,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::FullExtended => "full-extended",
            Self::Full => "full",
            Self::Light => "light",
            Self::HciLayerExtended => "hci-layer-extended",
            Self::HciLayer => "hci-layer",
            Self::HciAdvScan => "hci-adv-scan",
            Self::WbaFull => "wba-full",
            Self::WbaBasicPlus => "wba-basic-plus",
            Self::WbaBasicFeatures => "wba-basic-features",
            Self::WbaPeripheralOnly => "wba-peripheral-only",
            Self::WbaLinkLayerOnly => "wba-link-layer-only",
        }
    }

    pub const fn platform(self) -> Platform {
        match self {
            Self::FullExtended
            | Self::Full
            | Self::Light
            | Self::HciLayerExtended
            | Self::HciLayer
            | Self::HciAdvScan => Platform::Stm32wb,
            Self::WbaFull
            | Self::WbaBasicPlus
            | Self::WbaBasicFeatures
            | Self::WbaPeripheralOnly
            | Self::WbaLinkLayerOnly => Platform::Stm32wba,
        }
    }

    /// The Cargo feature selecting this profile, e.g. `stack-light`.
    pub fn feature_name(self) -> String {
        format!("stack-{}", self.name())
    }

    pub fn from_feature_name(feature: &str) -> Option<Self> {
        feature.strip_prefix("stack-")?.parse().ok()
    }

    /// The availability column heading of the platform's interface document
    /// (`STM32WB_BLE_Wireless_Interface.html` or
    /// `STM32WBA_BLE_Wireless_Interface.html`). The complete profile has no
    /// column: it supports the whole interface.
    pub const fn documentation_column(self) -> Option<&'static str> {
        match self {
            Self::FullExtended | Self::WbaFull => None,
            Self::Full | Self::WbaBasicFeatures => Some("BF"),
            Self::Light | Self::WbaPeripheralOnly => Some("PO"),
            Self::HciLayerExtended | Self::WbaLinkLayerOnly => Some("LO"),
            Self::HciLayer => Some("LB"),
            Self::HciAdvScan => Some("BO"),
            Self::WbaBasicPlus => Some("BP"),
        }
    }

    /// The STM32WB CPU2 binary name component for this profile; STM32WBA
    /// ships no binaries.
    pub const fn binary_stem(self) -> Option<&'static str> {
        match self {
            Self::FullExtended => Some("BLE_Stack_full_extended"),
            Self::Full => Some("BLE_Stack_full"),
            Self::Light => Some("BLE_Stack_light"),
            Self::HciLayerExtended => Some("BLE_HCILayer_extended"),
            Self::HciLayer => Some("BLE_HCILayer"),
            Self::HciAdvScan => Some("BLE_HCI_AdvScan"),
            _ => None,
        }
    }
}

impl Platform {
    /// The prefix of the Cargo features selecting the platform's releases:
    /// STM32WB names the release of its CPU2 wireless binary, and STM32WBA
    /// that of its BLE stack library.
    pub const fn release_feature_prefix(self) -> &'static str {
        match self {
            Self::Stm32wb => "fw_",
            Self::Stm32wba => "wba_",
        }
    }

    /// The Cargo feature selecting a release of the platform, e.g.
    /// `fw_1_24_0` or `wba_1_10_0`.
    pub fn release_feature(self, release: Version) -> String {
        release.feature_name(self.release_feature_prefix())
    }

    /// The release a Cargo feature of the platform selects.
    pub fn release_from_feature(self, feature: &str) -> Option<Version> {
        Version::from_feature_name(feature, self.release_feature_prefix())
    }

    /// The stack profiles of the platform, the complete one first.
    pub fn profiles(self) -> &'static [Profile] {
        match self {
            Self::Stm32wb => &Profile::ALL[..6],
            Self::Stm32wba => &Profile::ALL[6..],
        }
    }

    /// The profile supporting the complete interface.
    pub fn complete_profile(self) -> Profile {
        self.profiles()[0]
    }

    /// The profile an availability column of the platform's interface
    /// document names.
    pub fn profile_for_column(self, column: &str) -> Option<Profile> {
        self.profiles()
            .iter()
            .copied()
            .find(|profile| profile.documentation_column() == Some(column))
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Profile {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.name() == value)
            .ok_or_else(|| Error::parse(format!("unknown stack profile {value:?}")))
    }
}

/// An STM32WB MCU family with its own set of CPU2 binaries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    Wb1x,
    Wb3x,
    Wb5x,
}

impl Family {
    pub const ALL: [Self; 3] = [Self::Wb1x, Self::Wb3x, Self::Wb5x];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Wb1x => "wb1x",
            Self::Wb3x => "wb3x",
            Self::Wb5x => "wb5x",
        }
    }

    /// Directory below `Projects/STM32WB_Copro_Wireless_Binaries`.
    pub const fn directory(self) -> &'static str {
        match self {
            Self::Wb1x => "STM32WB1x",
            Self::Wb3x => "STM32WB3x",
            Self::Wb5x => "STM32WB5x",
        }
    }

    /// Prefix of every binary file name for this family.
    pub const fn file_prefix(self) -> &'static str {
        match self {
            Self::Wb1x => "stm32wb1x",
            Self::Wb3x => "stm32wb3x",
            Self::Wb5x => "stm32wb5x",
        }
    }

    /// The CPU2 binary file name for a profile, if it has binaries.
    pub fn binary_file_name(self, profile: Profile) -> Option<String> {
        let stem = profile.binary_stem()?;
        Some(format!("{}_{stem}_fw.bin", self.file_prefix()))
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Family {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|family| family.name() == value)
            .ok_or_else(|| Error::parse(format!("unknown MCU family {value:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names_round_trip() {
        for profile in Profile::ALL {
            assert_eq!(profile.name().parse::<Profile>().unwrap(), profile);
            assert_eq!(
                Profile::from_feature_name(&profile.feature_name()),
                Some(profile)
            );
            let platform = profile.platform();
            assert!(platform.profiles().contains(&profile));
            match profile.documentation_column() {
                Some(column) => assert_eq!(platform.profile_for_column(column), Some(profile)),
                None => assert_eq!(platform.complete_profile(), profile),
            }
        }
        assert_eq!(
            Platform::Stm32wb.profile_for_column("BF"),
            Some(Profile::Full)
        );
        assert_eq!(
            Platform::Stm32wba.profile_for_column("BF"),
            Some(Profile::WbaBasicFeatures)
        );
        assert_eq!(Platform::Stm32wb.profile_for_column("BP"), None);
        let release = Version::new(1, 10, 0);
        assert_eq!(Platform::Stm32wba.release_feature(release), "wba_1_10_0");
        assert_eq!(
            Platform::Stm32wba.release_from_feature("wba_1_10_0"),
            Some(release)
        );
        assert_eq!(Platform::Stm32wb.release_from_feature("wba_1_10_0"), None);
        assert_eq!(
            Family::Wb5x
                .binary_file_name(Profile::FullExtended)
                .as_deref(),
            Some("stm32wb5x_BLE_Stack_full_extended_fw.bin")
        );
        assert_eq!(Family::Wb5x.binary_file_name(Profile::WbaFull), None);
    }
}
