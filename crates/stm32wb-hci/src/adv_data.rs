//! Advertising and scan response data, built from the AD structures of the
//! Core Specification Supplement.
//!
//! The data goes to [`GapUpdateAdvData`](crate::aci::gap::GapUpdateAdvData),
//! the extended advertising commands, or bt-hci's advertising commands:
//!
//! ```ignore
//! let data = AdvData::<31>::new()
//!     .flags(AdvFlags::LE_GENERAL_DISCOVERABLE | AdvFlags::BR_EDR_NOT_SUPPORTED)?
//!     .complete_local_name(b"STM32WB")?
//!     .complete_uuid16_list(&[0x180D])?;
//! controller.exec(&GapUpdateAdvData::try_new(data.as_bytes())?).await?;
//! ```
//!
//! `aci_gap_set_discoverable` takes its local name as an AD structure too,
//! whose type the stack reads from the first byte:
//! [`local_name_structure`] builds it.

use crate::aci::durations::ConnInterval;
use crate::wire::TooLong;
use crate::wire_flags;

wire_flags! {
    /// The flags AD structure: the discoverable mode and BR/EDR support.
    pub struct AdvFlags: u8 {
        /// LE limited discoverable mode.
        const LE_LIMITED_DISCOVERABLE = 0x01;
        /// LE general discoverable mode.
        const LE_GENERAL_DISCOVERABLE = 0x02;
        /// BR/EDR is not supported, as on STM32WB.
        const BR_EDR_NOT_SUPPORTED = 0x04;
    }
}

/// AD types of the Core Specification Supplement.
mod ad_type {
    pub const FLAGS: u8 = 0x01;
    pub const INCOMPLETE_UUID16: u8 = 0x02;
    pub const COMPLETE_UUID16: u8 = 0x03;
    pub const INCOMPLETE_UUID128: u8 = 0x06;
    pub const COMPLETE_UUID128: u8 = 0x07;
    pub const SHORTENED_LOCAL_NAME: u8 = 0x08;
    pub const COMPLETE_LOCAL_NAME: u8 = 0x09;
    pub const TX_POWER_LEVEL: u8 = 0x0A;
    pub const PERIPHERAL_CONN_INTERVAL_RANGE: u8 = 0x12;
    pub const SERVICE_DATA_UUID16: u8 = 0x16;
    pub const APPEARANCE: u8 = 0x19;
    pub const MANUFACTURER_SPECIFIC_DATA: u8 = 0xFF;
}

/// Advertising or scan response data of at most `N` bytes: 31 for legacy
/// advertising, more for extended advertising.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AdvData<const N: usize = 31> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> AdvData<N> {
    /// No AD structures.
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    /// The encoded AD structures.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    /// Append an AD structure of type `ad_type` whose data is the
    /// concatenation of `parts`, or fail if the data would exceed `N` bytes
    /// or the structure 255.
    pub fn structure(mut self, ad_type: u8, parts: &[&[u8]]) -> Result<Self, TooLong> {
        let data_len = parts.iter().map(|part| part.len()).sum::<usize>();
        let needed = self.len + 2 + data_len;
        if needed > N || data_len > 254 {
            return Err(TooLong {
                field: "advertising data",
                len: needed,
                capacity: N,
            });
        }
        self.bytes[self.len] = (data_len + 1) as u8;
        self.bytes[self.len + 1] = ad_type;
        let mut at = self.len + 2;
        for part in parts {
            self.bytes[at..at + part.len()].copy_from_slice(part);
            at += part.len();
        }
        self.len = at;
        Ok(self)
    }

    /// Append the flags.
    pub fn flags(self, flags: AdvFlags) -> Result<Self, TooLong> {
        self.structure(ad_type::FLAGS, &[&[flags.bits()]])
    }

    /// Append the complete local name.
    pub fn complete_local_name(self, name: &[u8]) -> Result<Self, TooLong> {
        self.structure(ad_type::COMPLETE_LOCAL_NAME, &[name])
    }

    /// Append the start of the local name.
    pub fn shortened_local_name(self, name: &[u8]) -> Result<Self, TooLong> {
        self.structure(ad_type::SHORTENED_LOCAL_NAME, &[name])
    }

    /// Append every 16-bit service UUID of the device.
    pub fn complete_uuid16_list(self, uuids: &[u16]) -> Result<Self, TooLong> {
        self.uuid16_list(ad_type::COMPLETE_UUID16, uuids)
    }

    /// Append some of the 16-bit service UUIDs of the device.
    pub fn incomplete_uuid16_list(self, uuids: &[u16]) -> Result<Self, TooLong> {
        self.uuid16_list(ad_type::INCOMPLETE_UUID16, uuids)
    }

    /// Append every 128-bit service UUID of the device, each least
    /// significant byte first.
    pub fn complete_uuid128_list(self, uuids: &[[u8; 16]]) -> Result<Self, TooLong> {
        self.structure(ad_type::COMPLETE_UUID128, &[uuids.as_flattened()])
    }

    /// Append some of the 128-bit service UUIDs of the device, each least
    /// significant byte first.
    pub fn incomplete_uuid128_list(self, uuids: &[[u8; 16]]) -> Result<Self, TooLong> {
        self.structure(ad_type::INCOMPLETE_UUID128, &[uuids.as_flattened()])
    }

    /// Append the transmit power, in dBm.
    pub fn tx_power_level(self, dbm: i8) -> Result<Self, TooLong> {
        self.structure(ad_type::TX_POWER_LEVEL, &[&dbm.to_le_bytes()])
    }

    /// Append the connection intervals the peripheral prefers.
    pub fn peripheral_conn_interval_range(
        self,
        min: ConnInterval,
        max: ConnInterval,
    ) -> Result<Self, TooLong> {
        self.structure(
            ad_type::PERIPHERAL_CONN_INTERVAL_RANGE,
            &[&min.units().to_le_bytes(), &max.units().to_le_bytes()],
        )
    }

    /// Append data for the service with the 16-bit UUID `uuid`.
    pub fn service_data_uuid16(self, uuid: u16, data: &[u8]) -> Result<Self, TooLong> {
        self.structure(ad_type::SERVICE_DATA_UUID16, &[&uuid.to_le_bytes(), data])
    }

    /// Append the external appearance of the device, from the Assigned
    /// Numbers.
    pub fn appearance(self, appearance: u16) -> Result<Self, TooLong> {
        self.structure(ad_type::APPEARANCE, &[&appearance.to_le_bytes()])
    }

    /// Append data specific to the manufacturer with the company identifier
    /// `company`.
    pub fn manufacturer_specific_data(self, company: u16, data: &[u8]) -> Result<Self, TooLong> {
        self.structure(
            ad_type::MANUFACTURER_SPECIFIC_DATA,
            &[&company.to_le_bytes(), data],
        )
    }

    fn uuid16_list(self, ad_type: u8, uuids: &[u16]) -> Result<Self, TooLong> {
        let needed = self.len + 2 + 2 * uuids.len();
        if needed > N || uuids.len() > 127 {
            return Err(TooLong {
                field: "advertising data",
                len: needed,
                capacity: N,
            });
        }
        let mut this = self.structure(ad_type, &[])?;
        this.bytes[this.len - 2] += 2 * uuids.len() as u8;
        for uuid in uuids {
            this.bytes[this.len..this.len + 2].copy_from_slice(&uuid.to_le_bytes());
            this.len += 2;
        }
        Ok(this)
    }
}

impl<const N: usize> Default for AdvData<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> core::ops::Deref for AdvData<N> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<const N: usize> core::fmt::Debug for AdvData<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("AdvData").field(&self.as_bytes()).finish()
    }
}

#[cfg(feature = "defmt")]
impl<const N: usize> defmt::Format for AdvData<N> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "AdvData({=[u8]:x})", self.as_bytes())
    }
}

/// The local name as the AD structure `aci_gap_set_discoverable` and
/// `aci_gap_set_limited_discoverable` take: its type, complete or
/// shortened, then the name. The stack adds the length itself.
pub fn local_name_structure<const N: usize>(
    name: &[u8],
    complete: bool,
) -> Result<AdvData<N>, TooLong> {
    let needed = 1 + name.len();
    if needed > N {
        return Err(TooLong {
            field: "local name",
            len: needed,
            capacity: N,
        });
    }
    let mut data = AdvData::new();
    data.bytes[0] = if complete {
        ad_type::COMPLETE_LOCAL_NAME
    } else {
        ad_type::SHORTENED_LOCAL_NAME
    };
    data.bytes[1..needed].copy_from_slice(name);
    data.len = needed;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structures_are_length_type_and_data() {
        let data = AdvData::<31>::new()
            .flags(AdvFlags::LE_GENERAL_DISCOVERABLE | AdvFlags::BR_EDR_NOT_SUPPORTED)
            .unwrap()
            .complete_local_name(b"WB")
            .unwrap()
            .complete_uuid16_list(&[0x180D, 0x180F])
            .unwrap()
            .manufacturer_specific_data(0x0030, &[0xAA])
            .unwrap();
        assert_eq!(
            data.as_bytes(),
            [
                2, 0x01, 0x06, 3, 0x09, b'W', b'B', 5, 0x03, 0x0D, 0x18, 0x0F, 0x18, 4, 0xFF, 0x30,
                0x00, 0xAA
            ]
        );
    }

    #[test]
    fn data_past_the_capacity_is_rejected() {
        let data = AdvData::<8>::new().complete_local_name(b"ABCDEF").unwrap();
        assert_eq!(data.len(), 8);
        let error = data.flags(AdvFlags::empty()).unwrap_err();
        assert_eq!((error.len, error.capacity), (11, 8));
        assert!(
            AdvData::<8>::new()
                .complete_uuid16_list(&[1, 2, 3, 4])
                .is_err()
        );
        assert!(AdvData::<300>::new().structure(0xFF, &[&[0; 255]]).is_err());
    }

    #[test]
    fn intervals_and_uuid128s_are_little_endian() {
        let data = AdvData::<31>::new()
            .peripheral_conn_interval_range(
                ConnInterval::MIN,
                ConnInterval::from_millis(15).unwrap(),
            )
            .unwrap()
            .complete_uuid128_list(&[[1; 16]])
            .unwrap();
        assert_eq!(data[..6], [5, 0x12, 6, 0, 12, 0]);
        assert_eq!(data[6..8], [17, 0x07]);
        assert_eq!(data.len(), 6 + 18);
    }

    #[test]
    fn set_discoverable_names_start_with_their_type() {
        let name = local_name_structure::<32>(b"WB55", true).unwrap();
        assert_eq!(name.as_bytes(), [0x09, b'W', b'B', b'5', b'5']);
        assert!(local_name_structure::<4>(b"WB55", false).is_err());
    }
}
