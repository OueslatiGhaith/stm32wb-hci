//! Types standing for the ranges of values the vendor commands document for
//! their parameters, other than durations.
//!
//! Each is declared with [`wire_range!`](crate::wire_range), and every
//! command using one checks at compile time, on every target, that the
//! catalog documents each of the type's values for that parameter. A range
//! only some commands document is left out, and its type says so.

use crate::wire_range;

/// The highest PA level: 1.15.0 documents levels up to +6 dBm, later
/// releases four more.
#[cfg(feature = "fw_1_15_0")]
const MAX_PA_LEVEL: u8 = 0x1F;
#[cfg(not(feature = "fw_1_15_0"))]
const MAX_PA_LEVEL: u8 = 0x23;

/// The largest L2CAP PDU payload a credit based channel accepts: 1.17.1
/// lowered it to the 248 bytes a link layer packet carries.
#[cfg(any(feature = "fw_1_15_0", feature = "fw_1_16_0", feature = "fw_1_17_0"))]
const MAX_COC_MPS: u16 = 0xFFFD;
#[cfg(not(any(feature = "fw_1_15_0", feature = "fw_1_16_0", feature = "fw_1_17_0")))]
const MAX_COC_MPS: u16 = 0x00F8;

wire_range! {
    /// How many connection events the peripheral may skip, from 0 to 499.
    pub struct ConnLatency: u16 {
        values = 0x0000..=0x01F3;
    }
}

wire_range! {
    /// A six-digit passkey, from 0 to 999999.
    pub struct Passkey: u32 {
        values = 0..=999_999;
    }
}

wire_range! {
    /// The handle of an advertising set, from 0 to 0xEF. The 0xFF some
    /// commands accept for no advertising set is left out.
    pub struct AdvHandle: u8 {
        values = 0x00..=0xEF;
    }
}

wire_range! {
    /// The advertising set identifier sent in extended advertising, from 0
    /// to 15.
    pub struct AdvSid: u8 {
        values = 0x00..=0x0F;
    }
}

wire_range! {
    /// The size of the encryption key an attribute requires, from 7 to 16
    /// bytes.
    pub struct EncKeySize: u8 {
        values = 7..=16;
    }
}

wire_range! {
    /// An RF channel, from 0 (2402 MHz) to 39 (2480 MHz).
    pub struct RfChannel: u8 {
        values = 0x00..=0x27;
    }
}

wire_range! {
    /// A transmit power, from -127 to +20 dBm. The 0x7F 1.18.0 adds for no
    /// preference is left out.
    pub struct TxPower: i8 {
        values = -127..=20;
    }
}

wire_range! {
    /// A received signal strength, from -127 to +20 dBm.
    pub struct Rssi: i8 {
        values = -127..=20;
        /// The strength is not available.
        const UNAVAILABLE = 127;
    }
}

wire_range! {
    /// A power amplifier level, from 0 (-40 dBm) to 0x1F (+6 dBm) in
    /// 1.15.0, and up to 0x23 from 1.16.0; 0x19 is 0 dBm.
    pub struct PaLevel: u8 {
        values = 0x00..=MAX_PA_LEVEL;
    }
}

wire_range! {
    /// A simplified protocol/service multiplexer of an LE credit based
    /// channel, from 1 to 0xFF.
    pub struct Spsm: u16 {
        values = 0x0001..=0x00FF;
    }
}

wire_range! {
    /// The largest SDU a credit based channel accepts, from 23 bytes.
    pub struct CocMtu: u16 {
        values = 0x0017..=0xFFFF;
    }
}

wire_range! {
    /// The largest PDU payload a credit based channel accepts, from 23
    /// bytes to 0xFFFD before 1.17.1 and to 248 from 1.17.1.
    pub struct CocMps: u16 {
        values = 0x0017..=MAX_COC_MPS;
    }
}

wire_range! {
    /// The credits granted to the peer of a credit based channel, from 1.
    pub struct CocCredits: u16 {
        values = 0x0001..=0xFFFF;
    }
}

wire_range! {
    /// How long a resolvable private address lasts, from 1 second to 1 hour.
    pub struct RpaTimeout: u16 {
        values = 0x0001..=0x0E10;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::format;

    use bt_hci::{FromHciBytes, FromHciBytesError, WriteHci};

    use super::*;
    use crate::wire::{OrUnknown, values_documented};

    #[test]
    fn ranges_hold_their_values() {
        assert_eq!(ConnLatency::new(499), Some(ConnLatency::MAX));
        assert_eq!(ConnLatency::new(500), None);
        assert_eq!(EncKeySize::new(6), None, "below the range");
        assert_eq!(TxPower::new(-127), Some(TxPower::MIN));
        assert_eq!(Rssi::new(127), None, "a special value");

        let mut buffer = [0u8; 4];
        Passkey::MAX.write_hci(&mut buffer[..]).unwrap();
        assert_eq!(buffer, [0x3F, 0x42, 0x0F, 0x00]);
        assert_eq!(
            Rssi::from_hci_bytes_complete(&[0xC4]).unwrap(),
            Rssi::new(-60).unwrap()
        );
        assert_eq!(
            Rssi::from_hci_bytes_complete(&[0x7F]).unwrap(),
            Rssi::UNAVAILABLE
        );
        assert_eq!(
            RfChannel::from_hci_bytes_complete(&[0x28]),
            Err(FromHciBytesError::InvalidValue)
        );
        assert_eq!(
            OrUnknown::<RfChannel>::from_hci_bytes_complete(&[0x28]).unwrap(),
            OrUnknown::Unknown(0x28)
        );
        assert_eq!(
            format!("{:?}", Rssi::new(-60).unwrap()).as_str(),
            "Rssi(-60)"
        );
        assert_eq!(
            format!("{:?}", Rssi::UNAVAILABLE).as_str(),
            "Rssi(UNAVAILABLE)"
        );
    }

    #[test]
    fn ranges_must_be_documented() {
        assert!(values_documented::<ConnLatency>(&[(0, 0x1F3)]));
        assert!(!values_documented::<ConnLatency>(&[(0, 0x1F2)]));
        assert!(
            !values_documented::<[ConnLatency; 2]>(&[(0, 0x1F2)]),
            "an array stands for the values of its elements"
        );
        assert!(values_documented::<Rssi>(&[(-127, 20), (127, 127)]));
        assert!(
            !values_documented::<Rssi>(&[(-127, 20)]),
            "the special value is undocumented"
        );
    }

    #[cfg(feature = "fw_1_15_0")]
    #[test]
    fn the_first_release_has_fewer_pa_levels() {
        assert_eq!(PaLevel::MAX.get(), 0x1F);
        assert_eq!(CocMps::MAX.get(), 0xFFFD);
    }

    #[cfg(feature = "fw_1_24_0")]
    #[test]
    fn later_releases_have_more_pa_levels() {
        assert_eq!(PaLevel::MAX.get(), 0x23);
        assert_eq!(CocMps::MAX.get(), 248);
    }
}
