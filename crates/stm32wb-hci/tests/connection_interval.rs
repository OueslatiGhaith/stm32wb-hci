extern crate stm32wb_hci as hci;

use hci::types::{
    ConnectionInterval, ConnectionIntervalBuilder, ConnectionIntervalError, FixedConnectionInterval,
};
use hci::vendor::command::HciEncodeField;
use std::time::Duration;

fn encode<T, const N: usize>(value: &T) -> [u8; N]
where
    T: HciEncodeField<N>,
{
    let mut bytes = [0; N];
    let mut writer = bytes.as_mut_slice();
    value.write_hci_field(&mut writer).unwrap();
    assert!(writer.is_empty());
    bytes
}

#[test]
fn valid() {
    let interval = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(50), Duration::from_millis(500))
        .with_latency(10)
        .with_supervision_timeout(Duration::from_secs(15))
        .build()
        .unwrap();
    let bytes: [u8; 8] = encode(&interval);

    // 50 ms / 1.25 ms = 40 = 0x0028
    // 500 ms / 1.25 ms = 400 = 0x0190
    // 15000 ms / 10 ms = 1500 = 0x05DC
    assert_eq!(bytes, [0x28, 0x00, 0x90, 0x01, 0x0A, 0x00, 0xDC, 0x05]);
}

#[test]
fn stores_canonical_ticks_and_encodes_fractional_milliseconds_exactly() {
    let interval = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_micros(7_500), Duration::from_micros(7_500))
        .with_latency(0)
        .with_supervision_timeout(Duration::from_millis(110))
        .build()
        .unwrap();

    assert_eq!(core::mem::size_of::<ConnectionInterval>(), 8);
    assert_eq!(core::mem::size_of::<FixedConnectionInterval>(), 6);
    assert_eq!(interval.interval().0, Duration::from_micros(7_500));
    assert_eq!(
        encode::<_, 8>(&interval),
        [0x06, 0x00, 0x06, 0x00, 0x00, 0x00, 0x0B, 0x00]
    );
}

#[test]
fn rejects_durations_between_hci_ticks() {
    let interval_error = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_micros(7_501), Duration::from_millis(10))
        .with_latency(0)
        .with_supervision_timeout(Duration::from_millis(110))
        .build()
        .unwrap_err();
    assert_eq!(
        interval_error,
        ConnectionIntervalError::NotRepresentable(Duration::from_micros(7_501))
    );

    let timeout_error = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_micros(7_500), Duration::from_micros(7_500))
        .with_latency(0)
        .with_supervision_timeout(Duration::from_millis(101))
        .build()
        .unwrap_err();
    assert_eq!(
        timeout_error,
        ConnectionIntervalError::NotRepresentable(Duration::from_millis(101))
    );
}

#[test]
fn incomplete() {
    assert_eq!(
        ConnectionIntervalBuilder::new()
            .with_latency(10)
            .with_supervision_timeout(Duration::from_secs(15))
            .build()
            .err()
            .unwrap(),
        ConnectionIntervalError::Incomplete
    );
    assert_eq!(
        ConnectionIntervalBuilder::new()
            .with_range(Duration::from_millis(50), Duration::from_millis(500))
            .with_supervision_timeout(Duration::from_secs(15))
            .build()
            .err()
            .unwrap(),
        ConnectionIntervalError::Incomplete
    );
    assert_eq!(
        ConnectionIntervalBuilder::new()
            .with_range(Duration::from_millis(50), Duration::from_millis(500))
            .with_latency(10)
            .build()
            .err()
            .unwrap(),
        ConnectionIntervalError::Incomplete
    );
}

#[test]
fn too_short() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(4), Duration::from_millis(1000))
        .with_latency(10)
        .with_supervision_timeout(Duration::from_secs(15))
        .build()
        .err()
        .unwrap();
    assert_eq!(
        err,
        ConnectionIntervalError::IntervalTooShort(Duration::from_millis(4))
    );
}

#[test]
fn too_long() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(100), Duration::from_millis(4001))
        .with_latency(10)
        .with_supervision_timeout(Duration::from_secs(15))
        .build()
        .err()
        .unwrap();
    assert_eq!(
        err,
        ConnectionIntervalError::IntervalTooLong(Duration::from_millis(4001))
    );
}

#[test]
fn inverted() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(500), Duration::from_millis(499))
        .with_latency(10)
        .with_supervision_timeout(Duration::from_secs(15))
        .build()
        .err()
        .unwrap();
    assert_eq!(
        err,
        ConnectionIntervalError::IntervalInverted(
            Duration::from_millis(500),
            Duration::from_millis(499)
        )
    );
}

#[test]
fn bad_conn_latency() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(50), Duration::from_millis(500))
        .with_latency(500)
        .with_supervision_timeout(Duration::from_secs(15))
        .build()
        .err()
        .unwrap();
    assert_eq!(err, ConnectionIntervalError::BadConnectionLatency(500));
}

#[test]
fn supervision_timeout_too_short_absolute() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_micros(7500), Duration::from_micros(7500))
        .with_latency(0)
        .with_supervision_timeout(Duration::from_millis(99))
        .build()
        .err()
        .unwrap();

    // The relative minimum supervision timeout here would be 15 ms (7.5 ms * (1 + 0) * 2), so our
    // timeout would meet that requirement. However, it is lower than the absolute minimum.
    assert_eq!(
        err,
        ConnectionIntervalError::SupervisionTimeoutTooShort(
            Duration::from_millis(99),
            Duration::from_millis(100)
        )
    );
}

#[test]
fn supervision_timeout_too_short_relative() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(50), Duration::from_millis(500))
        .with_latency(10)
        .with_supervision_timeout(Duration::from_millis(10999))
        .build()
        .err()
        .unwrap();

    // The relative minimum supervision timeout here is be 11 s (500 ms * (1 + 10) * 2).
    assert_eq!(
        err,
        ConnectionIntervalError::SupervisionTimeoutTooShort(
            Duration::from_millis(10999),
            Duration::from_secs(11)
        )
    );
}

#[test]
fn supervision_timeout_too_long() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(50), Duration::from_millis(500))
        .with_latency(10)
        .with_supervision_timeout(Duration::from_millis(32001))
        .build()
        .err()
        .unwrap();
    assert_eq!(
        err,
        ConnectionIntervalError::SupervisionTimeoutTooLong(Duration::from_millis(32001))
    );
}

#[test]
fn impossible_supervision_timeout() {
    let err = ConnectionIntervalBuilder::new()
        .with_range(Duration::from_millis(50), Duration::from_secs(4))
        .with_latency(4)
        .with_supervision_timeout(Duration::from_secs(32))
        .build()
        .err()
        .unwrap();
    assert_eq!(
        err,
        ConnectionIntervalError::ImpossibleSupervisionTimeout(Duration::from_secs(40))
    );
}

#[test]
fn from_bytes_valid() {
    let valid_bytes = [0x90, 0x00, 0x90, 0x01, 0x0A, 0x00, 0xDC, 0x05];
    let interval = ConnectionInterval::from_bytes(&valid_bytes).unwrap();
    assert_eq!(
        interval.interval(),
        (
            Duration::from_millis(0x90 * 5 / 4),
            Duration::from_millis(0x190 * 5 / 4),
        )
    );
    assert_eq!(interval.conn_latency(), 0x0A);
    assert_eq!(
        interval.supervision_timeout(),
        Duration::from_millis(10 * 0x05DC)
    );
}

#[test]
fn fixed_from_bytes_valid() {
    let valid_bytes = [0x90, 0x01, 0x0A, 0x00, 0xDC, 0x05];
    let interval = FixedConnectionInterval::from_bytes(&valid_bytes).unwrap();
    assert_eq!(interval.interval(), Duration::from_millis(0x190 * 5 / 4));
    assert_eq!(interval.conn_latency(), 0x0A);
    assert_eq!(
        interval.supervision_timeout(),
        Duration::from_millis(10 * 0x05DC)
    );
}

#[test]
fn from_bytes_validates_decoded_fields() {
    let bytes = [0x05, 0x00, 0x09, 0x00, 0x0A, 0x00, 0xDC, 0x05];
    let err = ConnectionInterval::from_bytes(&bytes).err().unwrap();
    assert_eq!(
        err,
        ConnectionIntervalError::IntervalTooShort(Duration::from_micros(6250))
    );
}
