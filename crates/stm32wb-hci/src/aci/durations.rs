//! Types standing for the durations the vendor commands document for their
//! parameters, each counted in its member's unit.
//!
//! Each is declared with [`wire_duration!`](crate::wire_duration), and every
//! command using one checks at compile time, on every target, that the
//! catalog documents that parameter on STM32WB in the same unit, and each of
//! the type's values. A range only some commands or releases document is
//! left out, and its type says so.

use crate::wire_duration;

/// The longest scan interval and window when scanning for extended
/// advertising: 15 s on STM32WB, 40.96 s on STM32WBA.
#[cfg(feature = "_stm32wb")]
const MAX_EXT_SCAN_UNITS: u16 = 0x5DC0;
#[cfg(feature = "_stm32wba")]
const MAX_EXT_SCAN_UNITS: u16 = 0xFFFF;

wire_duration! {
    /// The interval between legacy advertising events, from 20 ms to
    /// 10.24 s. The 3.75 ms interval of high duty cycle directed advertising
    /// is left out.
    pub struct AdvInterval: u16 {
        unit_us = 625;
        units = 0x0020..=0x4000;
    }
}

wire_duration! {
    /// The interval between directed advertising events: low duty cycle,
    /// from 20 ms to 10.24 s, or high duty cycle.
    pub struct DirectAdvInterval: u16 {
        unit_us = 625;
        units = 0x0020..=0x4000;
        /// High duty cycle directed advertising, at most 3.75 ms apart for
        /// at most 1.28 s.
        const HIGH_DUTY_CYCLE = 0x0006;
    }
}

wire_duration! {
    /// The interval between the primary advertising events of an advertising
    /// set, from 20 ms to about 10486 s.
    pub struct ExtAdvInterval: u32 {
        unit_us = 625;
        units = 0x0000_0020..=0x00FF_FFFF;
    }
}

wire_duration! {
    /// The interval between the starts of two scan windows when scanning only
    /// for legacy advertising, from 2.5 ms to 10.24 s.
    pub struct ScanInterval: u16 {
        unit_us = 625;
        units = 0x0004..=0x4000;
    }
}

wire_duration! {
    /// How long each scan lasts when scanning only for legacy advertising,
    /// from 2.5 ms to 10.24 s, and at most the scan interval.
    pub struct ScanWindow: u16 {
        unit_us = 625;
        units = 0x0004..=0x4000;
    }
}

wire_duration! {
    /// The interval between the starts of two scan windows when scanning for
    /// extended advertising, from 2.5 ms to 15 s on STM32WB and 40.96 s on
    /// STM32WBA.
    pub struct ExtScanInterval: u16 {
        unit_us = 625;
        units = 0x0004..=MAX_EXT_SCAN_UNITS;
    }
}

wire_duration! {
    /// How long each scan lasts when scanning for extended advertising, from
    /// 2.5 ms to 15 s on STM32WB and 40.96 s on STM32WBA, and at most the
    /// scan interval.
    pub struct ExtScanWindow: u16 {
        unit_us = 625;
        units = 0x0004..=MAX_EXT_SCAN_UNITS;
    }
}

/// The scan interval of the general discovery procedure: an
/// [`ExtScanInterval`] before STM32WB 1.22.0 and STM32WBA 1.6.0, which then
/// limit it to legacy advertising's [`ScanInterval`].
#[cfg(any(
    feature = "fw_1_15_0",
    feature = "fw_1_16_0",
    feature = "fw_1_17_0",
    feature = "fw_1_17_1",
    feature = "fw_1_17_2",
    feature = "fw_1_17_3",
    feature = "fw_1_18_0",
    feature = "fw_1_19_0",
    feature = "fw_1_19_1",
    feature = "fw_1_20_0",
    feature = "fw_1_21_0",
    feature = "wba_1_0_0",
    feature = "wba_1_1_0",
    feature = "wba_1_1_1",
    feature = "wba_1_2_0",
    feature = "wba_1_3_1",
    feature = "wba_1_4_0",
    feature = "wba_1_4_1",
    feature = "wba_1_5_0"
))]
pub type DiscoveryScanInterval = ExtScanInterval;
/// The scan interval of the general discovery procedure: an
/// [`ExtScanInterval`] before STM32WB 1.22.0 and STM32WBA 1.6.0, which then
/// limit it to legacy advertising's [`ScanInterval`].
#[cfg(not(any(
    feature = "fw_1_15_0",
    feature = "fw_1_16_0",
    feature = "fw_1_17_0",
    feature = "fw_1_17_1",
    feature = "fw_1_17_2",
    feature = "fw_1_17_3",
    feature = "fw_1_18_0",
    feature = "fw_1_19_0",
    feature = "fw_1_19_1",
    feature = "fw_1_20_0",
    feature = "fw_1_21_0",
    feature = "wba_1_0_0",
    feature = "wba_1_1_0",
    feature = "wba_1_1_1",
    feature = "wba_1_2_0",
    feature = "wba_1_3_1",
    feature = "wba_1_4_0",
    feature = "wba_1_4_1",
    feature = "wba_1_5_0"
)))]
pub type DiscoveryScanInterval = ScanInterval;

/// The scan window of the general discovery procedure: an [`ExtScanWindow`]
/// before STM32WB 1.22.0 and STM32WBA 1.6.0, which then limit it to legacy
/// advertising's [`ScanWindow`].
#[cfg(any(
    feature = "fw_1_15_0",
    feature = "fw_1_16_0",
    feature = "fw_1_17_0",
    feature = "fw_1_17_1",
    feature = "fw_1_17_2",
    feature = "fw_1_17_3",
    feature = "fw_1_18_0",
    feature = "fw_1_19_0",
    feature = "fw_1_19_1",
    feature = "fw_1_20_0",
    feature = "fw_1_21_0",
    feature = "wba_1_0_0",
    feature = "wba_1_1_0",
    feature = "wba_1_1_1",
    feature = "wba_1_2_0",
    feature = "wba_1_3_1",
    feature = "wba_1_4_0",
    feature = "wba_1_4_1",
    feature = "wba_1_5_0"
))]
pub type DiscoveryScanWindow = ExtScanWindow;
/// The scan window of the general discovery procedure: an [`ExtScanWindow`]
/// before STM32WB 1.22.0 and STM32WBA 1.6.0, which then limit it to legacy
/// advertising's [`ScanWindow`].
#[cfg(not(any(
    feature = "fw_1_15_0",
    feature = "fw_1_16_0",
    feature = "fw_1_17_0",
    feature = "fw_1_17_1",
    feature = "fw_1_17_2",
    feature = "fw_1_17_3",
    feature = "fw_1_18_0",
    feature = "fw_1_19_0",
    feature = "fw_1_19_1",
    feature = "fw_1_20_0",
    feature = "fw_1_21_0",
    feature = "wba_1_0_0",
    feature = "wba_1_1_0",
    feature = "wba_1_1_1",
    feature = "wba_1_2_0",
    feature = "wba_1_3_1",
    feature = "wba_1_4_0",
    feature = "wba_1_4_1",
    feature = "wba_1_5_0"
)))]
pub type DiscoveryScanWindow = ScanWindow;

wire_duration! {
    /// How long an extended scan lasts, from 10 ms to 655.35 s.
    pub struct ScanDuration: u16 {
        unit_us = 10_000;
        units = 0x0001..=0xFFFF;
        /// Scan until scanning is disabled.
        const CONTINUOUS = 0x0000;
    }
}

wire_duration! {
    /// The time from the start of one extended scan to the start of the
    /// next, from 1.28 s to about 83884 s.
    pub struct ScanPeriod: u16 {
        unit_us = 1_280_000;
        units = 0x0001..=0xFFFF;
        /// Scan continuously, without repeating.
        const CONTINUOUS = 0x0000;
    }
}

wire_duration! {
    /// How long an advertising set advertises, from 10 ms to 655.35 s.
    pub struct AdvDuration: u16 {
        unit_us = 10_000;
        units = 0x0001..=0xFFFF;
        /// Advertise until advertising is disabled.
        const UNLIMITED = 0x0000;
    }
}

wire_duration! {
    /// A connection interval, from 7.5 ms to 4 s.
    pub struct ConnInterval: u16 {
        unit_us = 1250;
        units = 0x0006..=0x0C80;
    }
}

wire_duration! {
    /// A bound of the connection intervals the peripheral prefers, advertised
    /// in its advertising data, from 7.5 ms to 4 s.
    pub struct PreferredConnInterval: u16 {
        unit_us = 1250;
        units = 0x0006..=0x0C80;
        /// Advertise no preferred connection intervals; both bounds must be
        /// omitted.
        const OMITTED = 0x0000;
        /// No specific minimum or maximum.
        const UNSPECIFIED = 0xFFFF;
    }
}

wire_duration! {
    /// How long a connection may go without a valid packet before it is
    /// lost, from 100 ms to 32 s.
    pub struct SupervisionTimeout: u16 {
        unit_us = 10_000;
        units = 0x000A..=0x0C80;
    }
}

wire_duration! {
    /// The length of a connection event, from 0 to about 41 s.
    pub struct CeLength: u16 {
        unit_us = 625;
        units = 0x0000..=0xFFFF;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::format;

    use bt_hci::{FromHciBytes, FromHciBytesError, WriteHci};

    use super::*;
    use crate::aci::flags::Role;
    use crate::aci::values::Privacy;
    use crate::wire::{
        HciWireType, flags_documented, is_opaque, unit_documented, values_documented,
    };

    #[test]
    fn durations_count_units() {
        let interval = AdvInterval::from_millis(100).unwrap();
        assert_eq!(interval.units(), 160);
        assert_eq!(interval.as_micros(), Some(100_000));
        assert_eq!(AdvInterval::from_micros(100_001), None, "not whole units");
        assert_eq!(AdvInterval::from_millis(19), None, "below the range");
        assert_eq!(AdvInterval::from_units(0x4001), None, "above the range");
        assert_eq!(ConnInterval::from_micros(7_500), Some(ConnInterval::MIN));
        assert_eq!(ScanPeriod::from_millis(u64::MAX), None);
        assert_eq!(ScanDuration::CONTINUOUS.as_micros(), None);
        assert_eq!(ScanDuration::from_units(0), None, "a special value");
        assert_eq!(
            DirectAdvInterval::HIGH_DUTY_CYCLE.as_micros(),
            None,
            "the controller picks the interval"
        );

        let mut buffer = [0u8; 4];
        interval.write_hci(&mut buffer[..2]).unwrap();
        assert_eq!(buffer[..2], [0xA0, 0x00]);
        ExtAdvInterval::MAX.write_hci(&mut buffer[..]).unwrap();
        assert_eq!(buffer, [0xFF, 0xFF, 0xFF, 0x00]);
        assert_eq!(<ExtAdvInterval as HciWireType>::WIDTH, 4);
        assert_eq!(
            ScanDuration::from_hci_bytes_complete(&[0, 0]).unwrap(),
            ScanDuration::CONTINUOUS
        );
        assert_eq!(
            AdvInterval::from_hci_bytes_complete(&[0x1F, 0x00]),
            Err(FromHciBytesError::InvalidValue)
        );
        assert_eq!(format!("{interval:?}").as_str(), "AdvInterval(100000 µs)");
        assert_eq!(
            format!("{:?}", ScanDuration::CONTINUOUS).as_str(),
            "ScanDuration(CONTINUOUS)"
        );
    }

    #[test]
    fn durations_must_be_documented() {
        assert!(values_documented::<AdvInterval>(&[(0x20, 0x4000)]));
        assert!(values_documented::<AdvInterval>(&[
            (0x06, 0x06),
            (0x20, 0x4000)
        ]));
        assert!(!values_documented::<AdvInterval>(&[(0x20, 0x3FFF)]));
        assert!(
            values_documented::<ScanDuration>(&[(0, 0), (1, 0xFFFF)]),
            "adjacent items cover the range"
        );
        assert!(
            !values_documented::<ScanDuration>(&[(1, 0xFFFF)]),
            "the special value is undocumented"
        );
        assert!(values_documented::<PreferredConnInterval>(&[
            (0, 0),
            (6, 0xC80),
            (0xFFFF, 0xFFFF)
        ]));
        assert!(unit_documented::<AdvInterval>(Some(625)));
        assert!(!unit_documented::<AdvInterval>(Some(1250)), "another unit");
        assert!(!unit_documented::<AdvInterval>(None), "no time member");
        assert!(!unit_documented::<Privacy>(Some(625)), "not a duration");
        assert!(unit_documented::<Role>(None));
        assert!(!flags_documented::<CeLength>(0xFFFF), "a range is no flags");
        assert!(!is_opaque::<CeLength>());
    }

    #[cfg(feature = "_stm32wb")]
    #[test]
    fn stm32wb_scans_extended_advertising_for_up_to_15_s() {
        assert_eq!(ExtScanInterval::MAX.units(), 0x5DC0);
        assert_eq!(
            ExtScanWindow::from_micros(15_000_000),
            Some(ExtScanWindow::MAX)
        );
    }

    #[cfg(feature = "_stm32wba")]
    #[test]
    fn stm32wba_scans_extended_advertising_for_up_to_40_96_s() {
        assert_eq!(ExtScanInterval::MAX.units(), 0xFFFF);
        assert_eq!(ExtScanWindow::MAX.as_micros(), Some(40_959_375));
    }

    #[cfg(feature = "fw_1_21_0")]
    #[test]
    fn general_discovery_scans_extended_advertising_before_1_22_0() {
        assert_eq!(DiscoveryScanInterval::MAX, ExtScanInterval::MAX);
    }

    #[cfg(feature = "fw_1_22_0")]
    #[test]
    fn general_discovery_scans_legacy_advertising_from_1_22_0() {
        assert_eq!(DiscoveryScanWindow::MAX, ScanWindow::MAX);
    }
}
