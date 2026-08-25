use stm32wb_hci as hci;

use hci::types::{ExpectedConnectionLength, ExpectedConnectionLengthError};
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
    let range =
        ExpectedConnectionLength::new(Duration::from_millis(200), Duration::from_millis(500))
            .unwrap();
    let bytes: [u8; 4] = encode(&range);
    assert_eq!(bytes, [0x40, 0x01, 0x20, 0x03]);
    assert_eq!(
        range.range(),
        (Duration::from_millis(200), Duration::from_millis(500))
    );
    assert_eq!(core::mem::size_of::<ExpectedConnectionLength>(), 4);
}

#[test]
fn rejects_durations_between_hci_ticks() {
    let value = Duration::from_micros(200_001);
    let error = ExpectedConnectionLength::new(value, Duration::from_millis(500)).unwrap_err();
    assert_eq!(
        error,
        ExpectedConnectionLengthError::NotRepresentable(value)
    );
}

#[test]
fn interval_too_long() {
    let err = ExpectedConnectionLength::new(
        Duration::from_millis(200),
        Duration::from_micros(40_959_376),
    )
    .err()
    .unwrap();
    assert_eq!(
        err,
        ExpectedConnectionLengthError::TooLong(Duration::from_micros(40_959_376))
    );
}

#[test]
fn inverted() {
    let err = ExpectedConnectionLength::new(Duration::from_millis(400), Duration::from_millis(399))
        .err()
        .unwrap();
    assert_eq!(
        err,
        ExpectedConnectionLengthError::Inverted(
            Duration::from_millis(400),
            Duration::from_millis(399)
        )
    );
}
