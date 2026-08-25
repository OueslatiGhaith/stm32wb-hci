use core::time::Duration;

use super::time_units::{duration_from_ticks, duration_to_u32_ticks};

const ADVERTISING_INTERVAL_QUANTUM_MICROS: u64 = 625;

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Extended advertising modes
    pub struct AdvertisingMode: u8 => 1 {
        /// Use specific random address
        const SPECIFIC = 0x01;
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Advertising-set handle accepted by STM32WB extended-advertising commands.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct AdvertisingHandle: u8 => 1 {
        minimum: 0x00,
        maximum: 0xEF,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Advertising event types
    pub struct AdvertisingEvent: u16 => 2 {
        /// Connectable advertising
        const CONNECTABLE = 0x0001;
        /// Scannable advertising
        const SCANNABLE = 0x0002;
        /// Directed advertising
        const DIRECTED = 0x0004;
        /// High duty cycle directed connectable advertising
        const HIGH_DUTY_DIRECTED = 0x0008;
        /// Use legacy advertising PDUs
        const LEGACY = 0x0010;
        /// Anonymous advertising
        const ANONYMOUS = 0x0020;
        /// Include Tx power in at least one advertising PDU
        const INCLUDE_TX_POWER = 0x0040;
    }
}

/// Define an extended advertising interval range.
///
/// The advertising interval min shall be less than or equal to the advertising interval
/// max. The advertising interval min and advertising interval max should not be the same
/// values to enable the Controller to determine the best advertising interval given other
/// activities, though this implementation allows them to be equal.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ExtendedAdvertisingInterval {
    /// The first field is the min, the second is the max
    interval: (u32, u32),
}

impl ExtendedAdvertisingInterval {
    /// Creates an advertising interval with the provided minimum and maximum values.
    ///
    /// # Errors
    ///
    /// - [TooShort](ExtendedAdvertisingIntervalError::TooShort) if the minimum value is too small. For
    ///   Bluetooth specifications v4.x, if the advertising type is
    ///   [ScannableUndirected](crate::types::AdvertisingType::ScannableUndirected), then the
    ///   minimum value is 100 ms. In all other cases, the minimum value is 20 ms.
    /// - [TooLong](ExtendedAdvertisingIntervalError::TooLong) if the maximum value is too large. The
    ///   maximum value is 10,485.759375 seconds.
    /// - [Inverted](ExtendedAdvertisingIntervalError::Inverted) if the minimum is greater than the
    ///   maximum.
    /// - [NotRepresentable](ExtendedAdvertisingIntervalError::NotRepresentable) if either value
    ///   is not an exact multiple of 0.625 ms.
    pub fn with_range(
        min: Duration,
        max: Duration,
    ) -> Result<Self, ExtendedAdvertisingIntervalError> {
        const MIN: Duration = Duration::from_millis(20);
        const MAX: Duration = Duration::from_micros(10485759375);

        if min < MIN {
            return Err(ExtendedAdvertisingIntervalError::TooShort(min));
        }
        if max > MAX {
            return Err(ExtendedAdvertisingIntervalError::TooLong(max));
        }
        if min > max {
            return Err(ExtendedAdvertisingIntervalError::Inverted(min, max));
        }

        let minimum = duration_to_u32_ticks(min, u128::from(ADVERTISING_INTERVAL_QUANTUM_MICROS))
            .ok_or(ExtendedAdvertisingIntervalError::NotRepresentable(min))?;
        let maximum = duration_to_u32_ticks(max, u128::from(ADVERTISING_INTERVAL_QUANTUM_MICROS))
            .ok_or(ExtendedAdvertisingIntervalError::NotRepresentable(max))?;

        Ok(Self {
            interval: (minimum, maximum),
        })
    }

    /// Returns the minimum and maximum advertising intervals.
    pub fn interval(&self) -> (Duration, Duration) {
        (
            duration_from_ticks(self.interval.0, ADVERTISING_INTERVAL_QUANTUM_MICROS),
            duration_from_ticks(self.interval.1, ADVERTISING_INTERVAL_QUANTUM_MICROS),
        )
    }

    fn hci_fields(&self) -> (u32, u32) {
        self.interval
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    ExtendedAdvertisingInterval => 8 {
        Fields = {
            minimum: u32,
            maximum: u32,
        };
        Encode = |value| { value.hci_fields() };
    }
}

/// Potential errors that can occur when specifying an [`ExtendedAdvertisingInterval`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ExtendedAdvertisingIntervalError {
    /// The minimum value was too short. Includes the invalid value.
    TooShort(Duration),
    /// The maximum value was too long. Includes the invalid value.
    TooLong(Duration),
    /// The minimum value was greater than the maximum value. Includes the provided minimum and
    /// value, respectively.
    Inverted(Duration, Duration),
    /// A duration is not an exact multiple of the 0.625 ms HCI unit.
    NotRepresentable(Duration),
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Advertising PHY
    #[derive(Clone, Copy, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AdvertisingPhy: u8 => 1 {
        /// Advertisement PHY is LE 1M
        Le1M = 0x01,
        /// Advertisement PHY is LE 2M
        Le2M = 0x02,
        /// Advertisement PHY is LE Coded
        LeCoded = 0x03,
    }
}

/// Advertising set
pub struct AdvSet {
    /// Used to identify an advertising set
    pub handle: AdvertisingHandle,
    /// Duration of advertising set.
    ///
    /// Values:
    /// - 0x0000 (0 ms) : No advertising duration.
    /// - 0x0001 (10 ms)  ... 0xFFFF (655350 ms) : Advertising duration
    pub duration: u16,
    /// Maximum number of advertising events.
    ///
    /// Values:
    /// - 0x00: No maximum number of advertising events
    /// - 0x01 .. 0xFF: Maximum number of extended advertising events the
    ///   Controller shall attempt to send prior to terminating the extended
    ///   advertising
    pub max_extended_adv_events: u8,
}

impl AdvSet {
    fn hci_fields(&self) -> (AdvertisingHandle, u16, u8) {
        (self.handle, self.duration, self.max_extended_adv_events)
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    AdvSet => 4 {
        Fields = {
            handle: AdvertisingHandle,
            duration: u16,
            max_extended_adv_events: u8,
        };
        Encode = |value| { value.hci_fields() };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Advertising Operation
    #[derive(Clone, Copy, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AdvertisingOperation: u8 => 1 {
        /// Intermediate fragment of fragmented extended advertising data
        IntermediateFragment = 0x00,
        /// First fragment of fragmented extended advertising data
        FirstFragment = 0x01,
        /// Last fragment of fragmented extended advertising data
        LastFragment = 0x02,
        /// Complete extended advertising data
        CompleteData = 0x03,
        /// Unchanged data (just update the advertising DID)
        UnchangedData = 0x04,
    }
}
