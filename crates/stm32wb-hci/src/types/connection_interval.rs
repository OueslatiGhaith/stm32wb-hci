//! Types related to the connection interval.

use core::cmp;
use core::time::Duration;

use super::time_units::{duration_from_ticks, duration_to_u16_ticks};

const INTERVAL_QUANTUM_MICROS: u64 = 1_250;
const TIMEOUT_QUANTUM_MICROS: u64 = 10_000;

/// Define a connection interval range with its latency and supervision timeout. This value is
/// passed to the controller, which determines the [actual connection interval](crate::types::FixedConnectionInterval).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionInterval {
    interval_: (u16, u16),
    conn_latency_: u16,
    supervision_timeout_: u16,
}

impl ConnectionInterval {
    /// Returns the connection interval.
    pub fn interval(&self) -> (Duration, Duration) {
        (
            interval_duration(self.interval_.0),
            interval_duration(self.interval_.1),
        )
    }

    /// Returns the connection latency, in number of events.
    pub fn conn_latency(&self) -> u16 {
        self.conn_latency_
    }

    /// Returns the supervision timeout.
    pub fn supervision_timeout(&self) -> Duration {
        timeout_duration(self.supervision_timeout_)
    }

    /// Deserializes the connection interval from the given byte buffer.
    ///
    /// - The minimum interval value, appropriately converted (2 bytes)
    /// - The maximum interval value, appropriately converted (2 bytes)
    /// - The connection latency (2 bytes)
    /// - The supervision timeout, appropriately converted (2 bytes)
    ///
    /// # Panics
    ///
    /// The provided buffer must be at least 8 bytes long.
    ///
    /// # Errors
    ///
    /// Any of the errors from the [builder](ConnectionIntervalBuilder::build) except for
    /// Incomplete.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ConnectionIntervalError> {
        assert!(bytes.len() >= 8);

        Self::from_hci_fields(
            u16::from_le_bytes([bytes[0], bytes[1]]),
            u16::from_le_bytes([bytes[2], bytes[3]]),
            u16::from_le_bytes([bytes[4], bytes[5]]),
            u16::from_le_bytes([bytes[6], bytes[7]]),
        )
    }

    pub(crate) fn from_hci_fields(
        interval_min: u16,
        interval_max: u16,
        latency: u16,
        timeout: u16,
    ) -> Result<Self, ConnectionIntervalError> {
        ConnectionIntervalBuilder::new()
            .with_range(
                interval_duration(interval_min),
                interval_duration(interval_max),
            )
            .with_latency(latency)
            .with_supervision_timeout(timeout_duration(timeout))
            .build()
    }

    fn hci_fields(&self) -> (u16, u16, u16, u16) {
        (
            self.interval_.0,
            self.interval_.1,
            self.conn_latency_,
            self.supervision_timeout_,
        )
    }
}

fn interval_duration(ticks: u16) -> Duration {
    duration_from_ticks(u32::from(ticks), INTERVAL_QUANTUM_MICROS)
}

fn timeout_duration(ticks: u16) -> Duration {
    duration_from_ticks(u32::from(ticks), TIMEOUT_QUANTUM_MICROS)
}

stm32wb_hci_macros::wire_type! {
    adapters: [command, event];
    composite
    ConnectionInterval => 8 {
        Fields = {
            interval_min: u16,
            interval_max: u16,
            latency: u16,
            timeout: u16,
        };
        Encode = |value| { value.hci_fields() };
        Decode = {
            ConnectionInterval::from_hci_fields(interval_min, interval_max, latency, timeout)
                .map_err(crate::vendor::event::VendorError::BadConnectionInterval)
                .map_err(crate::vendor::event::Error::Vendor)
        };
    }
}

/// Intermediate builder for the [`ConnectionInterval`].
#[derive(Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionIntervalBuilder {
    interval: Option<(Duration, Duration)>,
    conn_latency: Option<u16>,
    supervision_timeout: Option<Duration>,
}

impl ConnectionIntervalBuilder {
    /// Initializes a new builder.
    pub fn new() -> ConnectionIntervalBuilder {
        ConnectionIntervalBuilder {
            interval: None,
            conn_latency: None,
            supervision_timeout: None,
        }
    }

    /// Sets the connection interval range.
    ///
    /// # Errors
    ///
    /// There are no errors from this function, but it may cause errors in
    /// [build](ConnectionIntervalBuilder::build) if:
    /// - `min` is greater than `max`
    /// - Either `min` or `max` is less than 7.5 ms or more than 4 seconds.
    /// - `max` leads to an invalid relative supervision timeout.
    pub fn with_range(&mut self, min: Duration, max: Duration) -> &mut ConnectionIntervalBuilder {
        self.interval = Some((min, max));
        self
    }

    /// Sets the connection latency.
    ///
    /// # Errors
    ///
    /// There are no errors from this function, but it may cause errors in
    /// [build](ConnectionIntervalBuilder::build) if:
    /// - `latency` is 500 or greater.
    /// - `latency` leads to an invalid relative supervision timeout.
    pub fn with_latency(&mut self, latency: u16) -> &mut ConnectionIntervalBuilder {
        self.conn_latency = Some(latency);
        self
    }

    /// Sets the supervision timeout.
    ///
    /// # Errors
    ///
    /// There are no errors from this function, but it may cause errors in
    /// [build](ConnectionIntervalBuilder::build) if:
    /// - `timeout` less than 100 ms or greater than 32 seconds
    /// - `timeout` results in an invalid relative supervision timeout.
    pub fn with_supervision_timeout(
        &mut self,
        timeout: Duration,
    ) -> &mut ConnectionIntervalBuilder {
        self.supervision_timeout = Some(timeout);
        self
    }

    /// Builds the connection interval if all parameters are valid.
    ///
    /// # Errors
    ///
    /// - [Incomplete](ConnectionIntervalError::Incomplete) if any of
    ///   [`with_range`](ConnectionIntervalBuilder::with_range),
    ///   [`with_latency`](ConnectionIntervalBuilder::with_latency), or
    ///   [`with_supervision_timeout`](ConnectionIntervalBuilder::with_supervision_timeout) have not
    ///   been called.
    /// - [IntervalTooShort](ConnectionIntervalError::IntervalTooShort) if the minimum range value
    ///   is less than 7.5 ms.
    /// - [IntervalTooLong](ConnectionIntervalError::IntervalTooLong) if the maximum range value
    ///   is greater than 4 seconds.
    /// - [IntervalInverted](ConnectionIntervalError::IntervalInverted) if the minimum range value
    ///   is greater than the maximum.
    /// - [NotRepresentable](ConnectionIntervalError::NotRepresentable) if an interval is not an
    ///   exact multiple of 1.25 ms or the timeout is not an exact multiple of 10 ms.
    /// - [BadConnectionLatency](ConnectionIntervalError::BadConnectionLatency) if the connection
    ///   latency is 500 or more.
    /// - [SupervisionTimeoutTooShort](ConnectionIntervalError::SupervisionTimeoutTooShort) if the
    ///   supervision timeout is less than 100 ms, or if it is less than the computed minimum: (1 +
    ///   latency) * interval max * 2.
    /// - [SupervisionTimeoutTooLong](ConnectionIntervalError::SupervisionTimeoutTooLong) if the
    ///   supervision timeout is more than 32 seconds.
    /// - [ImpossibleSupervisionTimeout](ConnectionIntervalError::ImpossibleSupervisionTimeout) if
    ///   the computed minimum supervision timeout ((1 + latency) * interval max * 2) is 32 seconds
    ///   or more.
    pub fn build(&self) -> Result<ConnectionInterval, ConnectionIntervalError> {
        if self.interval.is_none()
            || self.conn_latency.is_none()
            || self.supervision_timeout.is_none()
        {
            return Err(ConnectionIntervalError::Incomplete);
        }

        let interval = self.interval.unwrap();
        const INTERVAL_MIN: Duration = Duration::from_micros(7500);
        if interval.0 < INTERVAL_MIN {
            return Err(ConnectionIntervalError::IntervalTooShort(interval.0));
        }

        const INTERVAL_MAX: Duration = Duration::from_secs(4);
        if interval.1 > INTERVAL_MAX {
            return Err(ConnectionIntervalError::IntervalTooLong(interval.1));
        }

        if interval.0 > interval.1 {
            return Err(ConnectionIntervalError::IntervalInverted(
                interval.0, interval.1,
            ));
        }
        let interval_ticks = (
            duration_to_u16_ticks(interval.0, u128::from(INTERVAL_QUANTUM_MICROS))
                .ok_or(ConnectionIntervalError::NotRepresentable(interval.0))?,
            duration_to_u16_ticks(interval.1, u128::from(INTERVAL_QUANTUM_MICROS))
                .ok_or(ConnectionIntervalError::NotRepresentable(interval.1))?,
        );
        let interval = (
            interval_duration(interval_ticks.0),
            interval_duration(interval_ticks.1),
        );

        let conn_latency = self.conn_latency.unwrap();
        const LATENCY_MAX: u16 = 0x1F3;
        if conn_latency > LATENCY_MAX {
            return Err(ConnectionIntervalError::BadConnectionLatency(conn_latency));
        }

        let supervision_timeout = self.supervision_timeout.unwrap();
        const TIMEOUT_MAX: Duration = Duration::from_secs(32);
        if supervision_timeout > TIMEOUT_MAX {
            return Err(ConnectionIntervalError::SupervisionTimeoutTooLong(
                supervision_timeout,
            ));
        }
        let computed_timeout_min = interval.1 * (1 + u32::from(conn_latency)) * 2;
        if computed_timeout_min >= TIMEOUT_MAX {
            return Err(ConnectionIntervalError::ImpossibleSupervisionTimeout(
                computed_timeout_min,
            ));
        }

        const TIMEOUT_ABS_MIN: Duration = Duration::from_millis(100);
        let timeout_min = cmp::max(computed_timeout_min, TIMEOUT_ABS_MIN);
        if supervision_timeout <= timeout_min {
            return Err(ConnectionIntervalError::SupervisionTimeoutTooShort(
                supervision_timeout,
                timeout_min,
            ));
        }
        let supervision_timeout_ticks =
            duration_to_u16_ticks(supervision_timeout, u128::from(TIMEOUT_QUANTUM_MICROS)).ok_or(
                ConnectionIntervalError::NotRepresentable(supervision_timeout),
            )?;

        Ok(ConnectionInterval {
            interval_: interval_ticks,
            conn_latency_: conn_latency,
            supervision_timeout_: supervision_timeout_ticks,
        })
    }
}

/// Types of errors that can occur when creating a [`ConnectionInterval`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectionIntervalError {
    /// At least one of any of [`with_range`](ConnectionIntervalBuilder::with_range),
    /// [`with_latency`](ConnectionIntervalBuilder::with_latency), or
    /// [`with_supervision_timeout`](ConnectionIntervalBuilder::with_supervision_timeout) has not
    /// been called.
    Incomplete,
    /// The minimum range value is less than 7.5 ms. Includes the invalid value.
    IntervalTooShort(Duration),
    /// The maximum range value is greater than 4 seconds. Includes the invalid value.
    IntervalTooLong(Duration),
    /// The minimum range value is greater than the maximum. Includes the provided minimum and
    /// maximum, respectively.
    IntervalInverted(Duration, Duration),
    /// A duration is not an exact multiple of its HCI time unit.
    NotRepresentable(Duration),
    /// The connection latency is 500 or more. Includes the provided value.
    BadConnectionLatency(u16),
    /// The supervision timeout is less than 100 ms, or it is less than the computed minimum: (1 +
    /// latency) * interval max * 2. The first value is the provided timeout; the second is the
    /// required minimum.
    SupervisionTimeoutTooShort(Duration, Duration),
    /// The supervision timeout is more than 32 seconds. Includes the provided timeout.
    SupervisionTimeoutTooLong(Duration),
    /// The computed minimum supervision timeout ((1 + latency) * interval max * 2) is 32 seconds
    /// or more. Includes the computed minimum.
    ImpossibleSupervisionTimeout(Duration),
}

/// Define a connection interval with its latency and supervision timeout. This value is
/// returned from the controller.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct FixedConnectionInterval {
    interval_: u16,
    conn_latency_: u16,
    supervision_timeout_: u16,
}

impl FixedConnectionInterval {
    /// Deserializes the connection interval from the given byte buffer.
    ///
    /// - The interval value, appropriately converted (2 bytes)
    /// - The connection latency (2 bytes)
    /// - The supervision timeout, appropriately converted (2 bytes)
    ///
    /// # Panics
    ///
    /// The provided buffer must be at least 6 bytes long.
    ///
    /// # Errors
    ///
    /// Any of the errors from the [builder](ConnectionIntervalBuilder::build) except for
    /// Incomplete.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ConnectionIntervalError> {
        assert!(bytes.len() >= 6);

        let interval = u16::from_le_bytes([bytes[0], bytes[1]]);
        let latency = u16::from_le_bytes([bytes[2], bytes[3]]);
        let timeout = u16::from_le_bytes([bytes[4], bytes[5]]);
        ConnectionInterval::from_hci_fields(interval, interval, latency, timeout)?;

        Ok(FixedConnectionInterval {
            interval_: interval,
            conn_latency_: latency,
            supervision_timeout_: timeout,
        })
    }

    /// Returns the connection interval.
    pub fn interval(&self) -> Duration {
        interval_duration(self.interval_)
    }

    /// Returns the connection latency, in number of events.
    pub fn conn_latency(&self) -> u16 {
        self.conn_latency_
    }

    /// Returns the supervision timeout.
    pub fn supervision_timeout(&self) -> Duration {
        timeout_duration(self.supervision_timeout_)
    }
}
