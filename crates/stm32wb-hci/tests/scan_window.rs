use stm32wb_hci as hci;

use hci::types::{ScanWindow, ScanWindowError};
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
    let scan_window = ScanWindow::start_every(Duration::from_millis(10))
        .unwrap()
        .open_for(Duration::from_millis(5))
        .unwrap();
    assert_eq!(scan_window.interval(), Duration::from_millis(10));
    assert_eq!(scan_window.window(), Duration::from_millis(5));
    assert_eq!(core::mem::size_of::<ScanWindow>(), 4);
    let bytes: [u8; 4] = encode(&scan_window);
    assert_eq!(bytes, [0x10, 0x00, 0x08, 0x00]);
}

#[test]
fn rejects_durations_between_hci_ticks() {
    let value = Duration::from_micros(10_001);
    let error = ScanWindow::start_every(value).err().unwrap();
    assert_eq!(error, ScanWindowError::NotRepresentable(value));
}

#[test]
fn interval_too_short() {
    let err = ScanWindow::start_every(Duration::from_millis(2))
        .err()
        .unwrap();
    assert_eq!(err, ScanWindowError::TooShort(Duration::from_millis(2)));
}

#[test]
fn interval_too_long() {
    let err = ScanWindow::start_every(Duration::from_millis(10241))
        .err()
        .unwrap();
    assert_eq!(err, ScanWindowError::TooLong(Duration::from_millis(10241)));
}

#[test]
fn window_too_short() {
    let err = ScanWindow::start_every(Duration::from_millis(10))
        .unwrap()
        .open_for(Duration::from_millis(2))
        .err()
        .unwrap();
    assert_eq!(err, ScanWindowError::TooShort(Duration::from_millis(2)));
}

#[test]
fn inverted() {
    let err = ScanWindow::start_every(Duration::from_millis(100))
        .unwrap()
        .open_for(Duration::from_millis(101))
        .err()
        .unwrap();
    assert_eq!(
        err,
        ScanWindowError::Inverted {
            interval: Duration::from_millis(100),
            window: Duration::from_millis(101),
        }
    );
}
