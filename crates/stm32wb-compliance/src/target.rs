//! Identity of the wireless coprocessor binary being checked.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::CubeRelease;

/// STM32WB device family used to select a family-specific CPU2 binary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McuFamily {
    Wb1x,
    Wb3x,
    Wb5x,
}

impl McuFamily {
    pub const ALL: [Self; 3] = [Self::Wb1x, Self::Wb3x, Self::Wb5x];

    pub const fn directory(self) -> &'static str {
        match self {
            Self::Wb1x => "STM32WB1x",
            Self::Wb3x => "STM32WB3x",
            Self::Wb5x => "STM32WB5x",
        }
    }

    const fn file_prefix(self) -> &'static str {
        match self {
            Self::Wb1x => "stm32wb1x",
            Self::Wb3x => "stm32wb3x",
            Self::Wb5x => "stm32wb5x",
        }
    }
}

impl fmt::Display for McuFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Wb1x => "wb1x",
            Self::Wb3x => "wb3x",
            Self::Wb5x => "wb5x",
        })
    }
}

impl FromStr for McuFamily {
    type Err = TargetParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "wb1x" | "stm32wb1x" => Ok(Self::Wb1x),
            "wb3x" | "stm32wb3x" => Ok(Self::Wb3x),
            "wb5x" | "stm32wb5x" => Ok(Self::Wb5x),
            _ => Err(TargetParseError(format!(
                "unknown STM32WB MCU family {value:?}; expected wb1x, wb3x, or wb5x"
            ))),
        }
    }
}

/// BLE stack profile encoded by one family-specific CPU2 binary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StackProfile {
    FullExtended,
    Full,
    Light,
    HciLayerExtended,
    HciLayer,
    HciAdvScan,
}

impl StackProfile {
    pub const ALL: [Self; 6] = [
        Self::FullExtended,
        Self::Full,
        Self::Light,
        Self::HciLayerExtended,
        Self::HciLayer,
        Self::HciAdvScan,
    ];

    const fn binary_stem(self) -> &'static str {
        match self {
            Self::FullExtended => "BLE_Stack_full_extended_fw.bin",
            Self::Full => "BLE_Stack_full_fw.bin",
            Self::Light => "BLE_Stack_light_fw.bin",
            Self::HciLayerExtended => "BLE_HCILayer_extended_fw.bin",
            Self::HciLayer => "BLE_HCILayer_fw.bin",
            Self::HciAdvScan => "BLE_HCI_AdvScan_fw.bin",
        }
    }

    /// Cargo feature selecting this stack profile in `stm32wb-hci`.
    pub const fn feature_name(self) -> &'static str {
        match self {
            Self::FullExtended => "stack-full-extended",
            Self::Full => "stack-full",
            Self::Light => "stack-light",
            Self::HciLayerExtended => "stack-hci-layer-extended",
            Self::HciLayer => "stack-hci-layer",
            Self::HciAdvScan => "stack-hci-adv-scan",
        }
    }

    pub(crate) fn from_documentation_column(value: &str) -> Option<Self> {
        match value.trim() {
            "BF" => Some(Self::Full),
            "PO" => Some(Self::Light),
            "LO" => Some(Self::HciLayerExtended),
            "LB" => Some(Self::HciLayer),
            "BO" => Some(Self::HciAdvScan),
            _ => None,
        }
    }
}

impl fmt::Display for StackProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FullExtended => "full-extended",
            Self::Full => "full",
            Self::Light => "light",
            Self::HciLayerExtended => "hci-layer-extended",
            Self::HciLayer => "hci-layer",
            Self::HciAdvScan => "hci-adv-scan",
        })
    }
}

impl FromStr for StackProfile {
    type Err = TargetParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full-extended" | "full_extended" => Ok(Self::FullExtended),
            "full" => Ok(Self::Full),
            "light" => Ok(Self::Light),
            "hci-layer-extended" | "hci_layer_extended" => Ok(Self::HciLayerExtended),
            "hci-layer" | "hci_layer" => Ok(Self::HciLayer),
            "hci-adv-scan" | "hci_adv_scan" => Ok(Self::HciAdvScan),
            _ => Err(TargetParseError(format!(
                "unknown BLE stack profile {value:?}; expected full-extended, full, light, hci-layer-extended, hci-layer, or hci-adv-scan"
            ))),
        }
    }
}

/// Complete identity of one Cube release and family-specific CPU2 binary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ComplianceTarget {
    pub release: CubeRelease,
    pub family: McuFamily,
    pub profile: StackProfile,
}

impl ComplianceTarget {
    pub const fn new(release: CubeRelease, family: McuFamily, profile: StackProfile) -> Self {
        Self {
            release,
            family,
            profile,
        }
    }

    pub fn binary_file_name(self) -> String {
        format!(
            "{}_{}",
            self.family.file_prefix(),
            self.profile.binary_stem()
        )
    }

    pub fn binary_path(self) -> PathBuf {
        PathBuf::from("Projects/STM32WB_Copro_Wireless_Binaries")
            .join(self.family.directory())
            .join(self.binary_file_name())
    }

    pub fn release_notes_path(self) -> PathBuf {
        PathBuf::from("Projects/STM32WB_Copro_Wireless_Binaries")
            .join(self.family.directory())
            .join("Release_Notes.html")
    }
}

impl fmt::Display for ComplianceTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "CubeWB {} / {} / {}",
            self.release,
            self.family,
            self.binary_file_name()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetParseError(String);

impl fmt::Display for TargetParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TargetParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_the_exact_family_binary_path() {
        let target = ComplianceTarget::new(
            CubeRelease::new(1, 24, 0),
            McuFamily::Wb5x,
            StackProfile::FullExtended,
        );
        assert_eq!(
            target.binary_path(),
            PathBuf::from(
                "Projects/STM32WB_Copro_Wireless_Binaries/STM32WB5x/stm32wb5x_BLE_Stack_full_extended_fw.bin"
            )
        );
    }

    #[test]
    fn parses_cli_names_without_losing_profile_identity() {
        assert_eq!("stm32wb3x".parse(), Ok(McuFamily::Wb3x));
        assert_eq!("hci-layer".parse(), Ok(StackProfile::HciLayer));
        assert_eq!(StackProfile::HciLayer.feature_name(), "stack-hci-layer");
        assert!("extended".parse::<StackProfile>().is_err());
    }
}
