//! Types related to the LE scanning window.

use core::time::Duration;

use super::time_units::{duration_from_ticks, duration_to_u16_ticks};

const QUANTUM_MICROS: u64 = 625;

/// Define a scanning window.
///
/// The controller runs LE scans every [`interval`](ScanWindow::interval), with scanning active
/// during the [`window`](ScanWindow::window) in every interval.
///
/// The minimum time range is 2.5 ms, and the maximum is 10.24 s. The window must be shorter than or
/// equal to the interval.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ScanWindow {
    interval_width: u16,
    window_width: u16,
}

impl ScanWindow {
    /// Returns the interval for the scanning window. The controller starts an LE scan every
    /// interval.
    pub fn interval(&self) -> Duration {
        ticks_to_duration(self.interval_width)
    }

    /// Returns the amount of time the controller is scanning every interval.
    pub fn window(&self) -> Duration {
        ticks_to_duration(self.window_width)
    }

    /// Begins building a [ScanWindow]. The scan window has the given interval. Returns a
    /// [builder](ScanWindowBuilder) that can be used to set the window duration.
    ///
    /// # Errors
    ///
    /// - [ScanWindowError::TooShort] if the provided interval is too short. It must be at least 2.5
    ///   ms.
    /// - [ScanWindowError::TooLong] if the provided interval is too long. It must be 10.24 seconds
    ///   or less.
    /// - [ScanWindowError::NotRepresentable] if the interval is not an exact multiple of 0.625 ms.
    pub fn start_every(interval: Duration) -> Result<ScanWindowBuilder, ScanWindowError> {
        Ok(ScanWindowBuilder {
            interval: ScanWindow::validate(interval)?,
        })
    }

    fn validate(d: Duration) -> Result<u16, ScanWindowError> {
        const MIN: Duration = Duration::from_micros(2500);
        if d < MIN {
            return Err(ScanWindowError::TooShort(d));
        }

        const MAX: Duration = Duration::from_millis(10240);
        if d > MAX {
            return Err(ScanWindowError::TooLong(d));
        }

        duration_to_u16_ticks(d, u128::from(QUANTUM_MICROS))
            .ok_or(ScanWindowError::NotRepresentable(d))
    }

    fn hci_fields(&self) -> (u16, u16) {
        (self.interval_width, self.window_width)
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    ScanWindow => 4 {
        Fields = {
            interval: u16 => 2,
            window: u16 => 2,
        };
        Encode = |value| { value.hci_fields() };
    }
}

/// Intermediate builder for the [`ScanWindow`].
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ScanWindowBuilder {
    interval: u16,
}

impl ScanWindowBuilder {
    /// Completes building a [ScanWindow]. The scan window has the given window.
    ///
    /// # Errors
    ///
    /// - [ScanWindowError::TooShort] if the provided interval is too short. It must be at least 2.5
    ///   ms.
    /// - [ScanWindowError::TooLong] if the provided interval is too long. It must be 10.24 seconds
    ///   or less.
    /// - [ScanWindowError::Inverted] if the window is longer than the interval.
    /// - [ScanWindowError::NotRepresentable] if the window is not an exact multiple of 0.625 ms.
    pub fn open_for(&self, window: Duration) -> Result<ScanWindow, ScanWindowError> {
        let interval = ticks_to_duration(self.interval);
        if window > interval {
            return Err(ScanWindowError::Inverted { interval, window });
        }

        Ok(ScanWindow {
            interval_width: self.interval,
            window_width: ScanWindow::validate(window)?,
        })
    }
}

/// Types of errors that can occur when creating a [`ScanWindow`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ScanWindowError {
    /// The duration is too short. Both the interval and duration must be at least 2.5 ms. Includes
    /// the invalid duration.
    TooShort(Duration),
    /// The duration is too long. Both the interval and duration must be no more than 10.24
    /// seconds. Includes the invalid duration.
    TooLong(Duration),
    /// The duration is not an exact multiple of the 0.625 ms HCI unit.
    NotRepresentable(Duration),
    /// The interval and window are inverted. That is, the interval is shorter than the window.
    Inverted {
        /// The provided interval, which is shorter than the window.
        interval: Duration,
        /// The provided window, which is longer than the interval.
        window: Duration,
    },
}

fn ticks_to_duration(ticks: u16) -> Duration {
    duration_from_ticks(u32::from(ticks), QUANTUM_MICROS)
}
