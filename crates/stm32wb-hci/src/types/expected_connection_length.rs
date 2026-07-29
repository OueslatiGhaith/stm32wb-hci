//! Types related to the expected connection length range.

use core::time::Duration;

use super::time_units::{duration_from_ticks, duration_to_u16_ticks};

const QUANTUM_MICROS: u64 = 625;

/// Define an expected connection length range
///
/// There is no minimum. The maximum is bounded by what is representable as a u16 at T = N * 0.625
/// ms, so max = 65535 * 0.625 ms = 40.959375 seconds.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ExpectedConnectionLength {
    range: (u16, u16),
}

impl ExpectedConnectionLength {
    /// Creates a new ExpectedConnectionLength, or returns an error if the duration is invalid.
    ///
    /// # Errors
    ///
    /// - [Inverted](ExpectedConnectionLengthError::Inverted) if `min` is greater than `max`
    /// - [TooLong](ExpectedConnectionLengthError::TooLong) if `max` is longer than 40.959375
    ///   seconds.
    /// - [NotRepresentable](ExpectedConnectionLengthError::NotRepresentable) if either duration
    ///   is not an exact multiple of 0.625 ms.
    pub fn new(
        min: Duration,
        max: Duration,
    ) -> Result<ExpectedConnectionLength, ExpectedConnectionLengthError> {
        if min > max {
            return Err(ExpectedConnectionLengthError::Inverted(min, max));
        }

        const ABSOLUTE_MAX: Duration = Duration::from_micros(40_959_375);
        if max > ABSOLUTE_MAX {
            return Err(ExpectedConnectionLengthError::TooLong(max));
        }
        let minimum = duration_to_u16_ticks(min, u128::from(QUANTUM_MICROS))
            .ok_or(ExpectedConnectionLengthError::NotRepresentable(min))?;
        let maximum = duration_to_u16_ticks(max, u128::from(QUANTUM_MICROS))
            .ok_or(ExpectedConnectionLengthError::NotRepresentable(max))?;

        Ok(ExpectedConnectionLength {
            range: (minimum, maximum),
        })
    }

    /// Returns the minimum and maximum expected connection lengths.
    pub fn range(&self) -> (Duration, Duration) {
        (
            duration_from_ticks(u32::from(self.range.0), QUANTUM_MICROS),
            duration_from_ticks(u32::from(self.range.1), QUANTUM_MICROS),
        )
    }

    fn hci_fields(&self) -> (u16, u16) {
        self.range
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

/// Types of errors that can occur when creating an [`ExpectedConnectionLength`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ExpectedConnectionLengthError {
    /// The maximum expected length is too long. The maximum is 40.959375, because nothing higher
    /// can be represented as a u16.
    TooLong(Duration),
    /// The min is greater than the max. Returns the min and max, respectively.
    Inverted(Duration, Duration),
    /// A duration is not an exact multiple of the 0.625 ms HCI unit.
    NotRepresentable(Duration),
}
