extern crate stm32wb_hci as hci;

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
