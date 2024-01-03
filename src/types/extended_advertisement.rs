use core::time::Duration;

use byteorder::{ByteOrder, LittleEndian};

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Extended advertising modes
    pub struct AdvertisingMode: u8 {
        /// Use specific random address
        const SPECIFIC = 0x01;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Extended advertising modes
    pub struct AdvertisingMode: u8 {
        /// Use specific random address
        const SPECIFIC = 0x01;
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Advertising event types
    pub struct AdvertisingEvent: u16 {
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

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Advertising event types
    pub struct AdvertisingEvent: u16 {
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
/// adctivities, through this implementation allows them to be equal.
pub struct ExtendedAdvertisingInterval {
    /// The first field is the min, the second is the max
    interval: (Duration, Duration),
}

impl ExtendedAdvertisingInterval {
    /// Creates an advertising interval with the provided minimum and maximum values.
    ///
    /// # Errors
    ///
    /// - [TooShort](ExtendedAdvertisingIntervalError::TooShort) if the minimum value is too small. For
    ///   Bluetooth specifications v4.x, if the advertising type is
    ///   [ScannableUndirected](AdvertisingType::ScannableUndirected), then the minimum value is 100
    ///   ms. In all other cases, the minimum value is 20 ms.
    /// - [TooLong](ExtendedAdvertisingIntervalError::TooLong) if the maximum value is too large. The
    ///   maximum value is 10.24 seconds.
    /// - [Inverted](ExtendedAdvertisingIntervalError::Inverted) if the minimum is greater than the
    ///   maximum.
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

        Ok(Self {
            interval: (min, max),
        })
    }

    fn duration_as_u32(d: Duration) -> u32 {
        // T = 0.625 ms * N
        // so N = T / 0.625 ms
        //      = T / 625 us
        //
        // Note: 1600 = 1_000_000 / 625
        1600 * d.as_secs() as u32 + (d.subsec_micros() / 625)
    }

    /// Serialize the interval into the given buffer.
    ///
    /// Serializes the minimum range of the interval (4 bytes), the maximum range of the
    /// interval (4 bytees)
    ///
    /// # Panics
    ///
    /// - If the provided buffer is not at least 8 bytes long.
    pub fn copy_into_slice(&self, bytes: &mut [u8]) {
        LittleEndian::write_u32(&mut bytes[0..], Self::duration_as_u32(self.interval.0));
        LittleEndian::write_u32(&mut bytes[4..], Self::duration_as_u32(self.interval.1));
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
}

/// Advertising PHY
pub enum AdvertisingPhy {
    /// Advertisement PHY is LE 1M
    Le1M = 0x01,
    /// Advertisement PHY is LE 2M
    Le2M = 0x02,
}
