//! Serialize protocol codes as `0x`-prefixed hexadecimal strings, the
//! spelling used by the Bluetooth Core and ST documentation.

use serde::{Deserialize, Deserializer, Serializer};

/// Serialize a one-byte code as `0x`-prefixed hexadecimal.
pub(crate) mod byte {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(crate) fn serialize<S: Serializer>(value: &u8, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&format_args!("0x{value:02X}"))
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
        let value = String::deserialize(deserializer)?;
        value
            .strip_prefix("0x")
            .filter(|digits| digits.len() == 2)
            .and_then(|digits| u8::from_str_radix(digits, 16).ok())
            .ok_or_else(|| {
                serde::de::Error::custom(format!("expected a 0x-prefixed u8, got {value:?}"))
            })
    }
}

pub(crate) fn serialize<S: Serializer>(value: &u16, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&format_args!("0x{value:04X}"))
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    let value = String::deserialize(deserializer)?;
    value
        .strip_prefix("0x")
        .and_then(|digits| u16::from_str_radix(digits, 16).ok())
        .ok_or_else(|| {
            serde::de::Error::custom(format!("expected a 0x-prefixed u16, got {value:?}"))
        })
}
