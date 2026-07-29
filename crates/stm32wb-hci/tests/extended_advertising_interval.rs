use stm32wb_hci as hci;

use hci::types::extended_advertisement::{
    ExtendedAdvertisingInterval, ExtendedAdvertisingIntervalError,
};
use std::time::Duration;

#[test]
fn stores_canonical_ticks() {
    let interval = ExtendedAdvertisingInterval::with_range(
        Duration::from_millis(20),
        Duration::from_millis(100),
    )
    .unwrap();

    assert_eq!(core::mem::size_of::<ExtendedAdvertisingInterval>(), 8);
    assert_eq!(
        interval.interval(),
        (Duration::from_millis(20), Duration::from_millis(100))
    );
}

#[test]
fn rejects_durations_between_hci_ticks() {
    let value = Duration::from_micros(20_001);
    let error =
        ExtendedAdvertisingInterval::with_range(value, Duration::from_millis(100)).unwrap_err();
    assert_eq!(
        error,
        ExtendedAdvertisingIntervalError::NotRepresentable(value)
    );
}
