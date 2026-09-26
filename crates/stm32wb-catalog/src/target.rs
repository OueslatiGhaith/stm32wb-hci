//! Wireless-binary target dimensions.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Error;

/// A BLE stack profile, i.e. which variant of the CPU2 wireless binary runs.
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
}

impl Profile {
    pub const ALL: [Self; 6] = [
        Self::FullExtended,
        Self::Full,
        Self::Light,
        Self::HciLayerExtended,
        Self::HciLayer,
        Self::HciAdvScan,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::FullExtended => "full-extended",
            Self::Full => "full",
            Self::Light => "light",
            Self::HciLayerExtended => "hci-layer-extended",
            Self::HciLayer => "hci-layer",
            Self::HciAdvScan => "hci-adv-scan",
        }
    }

    /// The Cargo feature selecting this profile, e.g. `stack-light`.
    pub fn feature_name(self) -> String {
        format!("stack-{}", self.name())
    }

    pub fn from_feature_name(feature: &str) -> Option<Self> {
        feature.strip_prefix("stack-")?.parse().ok()
    }

    /// The availability column heading used by
    /// `STM32WB_BLE_Wireless_Interface.html`. The full-extended profile has no
    /// column: it supports the complete interface.
    pub const fn documentation_column(self) -> Option<&'static str> {
        match self {
            Self::FullExtended => None,
            Self::Full => Some("BF"),
            Self::Light => Some("PO"),
            Self::HciLayerExtended => Some("LO"),
            Self::HciLayer => Some("LB"),
            Self::HciAdvScan => Some("BO"),
        }
    }

    pub fn from_documentation_column(column: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.documentation_column() == Some(column))
    }

    /// The binary name component STMicroelectronics uses for this profile.
    pub const fn binary_stem(self) -> &'static str {
        match self {
            Self::FullExtended => "BLE_Stack_full_extended",
            Self::Full => "BLE_Stack_full",
            Self::Light => "BLE_Stack_light",
            Self::HciLayerExtended => "BLE_HCILayer_extended",
            Self::HciLayer => "BLE_HCILayer",
            Self::HciAdvScan => "BLE_HCI_AdvScan",
        }
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

    /// The CPU2 binary file name for a profile.
    pub fn binary_file_name(self, profile: Profile) -> String {
        format!("{}_{}_fw.bin", self.file_prefix(), profile.binary_stem())
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
            if let Some(column) = profile.documentation_column() {
                assert_eq!(Profile::from_documentation_column(column), Some(profile));
            }
        }
        assert_eq!(
            Family::Wb5x.binary_file_name(Profile::FullExtended),
            "stm32wb5x_BLE_Stack_full_extended_fw.bin"
        );
    }
}
