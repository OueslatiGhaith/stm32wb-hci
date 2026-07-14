//! Types related to the expected connection length range.

use core::time::Duration;

/// Define an expected connection length range
///
/// There is no minimum. The maximum is bounded by what is representable as a u16 at T = N * 0.625
/// ms, so max = 65535 * 0.625 ms = 40.959375 seconds.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ExpectedConnectionLength {
    pub range: (Duration, Duration),
}

impl ExpectedConnectionLength {
    /// Creates a new ExpectedConnectionLength, or returns an error if the duration is invalid.
    ///
    /// # Errors
    ///
    /// - [Inverted](ExpectedConnectionLengthError::Inverted) if `min` is greater than `max`
    /// - [TooLong](ExpectedConnectionLengthError::TooLong) if `max` is longer than 40.959375
    ///   seconds.
    pub fn new(
        min: Duration,
        max: Duration,
    ) -> Result<ExpectedConnectionLength, ExpectedConnectionLengthError> {
        if min > max {
            return Err(ExpectedConnectionLengthError::Inverted(min, max));
        }

        const ABSOLUTE_MAX: Duration = Duration::from_micros(40_959_375);
        assert_eq!(Self::duration_as_u16(ABSOLUTE_MAX), 0xFFFF);
        if max > ABSOLUTE_MAX {
            return Err(ExpectedConnectionLengthError::TooLong(max));
        }

        Ok(ExpectedConnectionLength { range: (min, max) })
    }

    fn duration_as_u16(d: Duration) -> u16 {
        // T = 0.625 ms * N
        // so N = T / 0.625 ms
        //      = T / 625 us
        //
        // Note: 1600 = 1_000_000 / 625
        (1600 * d.as_secs() as u32 + (d.subsec_micros() / 625)) as u16
    }

    fn hci_fields(&self) -> (u16, u16) {
        (
            Self::duration_as_u16(self.range.0),
            Self::duration_as_u16(self.range.1),
        )
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    ExpectedConnectionLength => 4 {
        Fields = {
            minimum: u16 => 2,
            maximum: u16 => 2,
        };
        Encode = |value| { value.hci_fields() };
    }
}

/// Types of errors that can occure when creating a [`ExpectedConnectionLength`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ExpectedConnectionLengthError {
    /// The maximum expected length is too long. The maximum is 40.959375, because nothing higher
    /// can be represented as a u16.
    TooLong(Duration),
    /// The min is greater than the max. Returns the min and max, respectively.
    Inverted(Duration, Duration),
}
