use core::time::Duration;

pub(crate) fn duration_to_u16_ticks(duration: Duration, quantum_micros: u128) -> Option<u16> {
    let micros = duration.as_micros();
    if !micros.is_multiple_of(quantum_micros) {
        return None;
    }
    u16::try_from(micros / quantum_micros).ok()
}

pub(crate) fn duration_to_u32_ticks(duration: Duration, quantum_micros: u128) -> Option<u32> {
    let micros = duration.as_micros();
    if !micros.is_multiple_of(quantum_micros) {
        return None;
    }
    u32::try_from(micros / quantum_micros).ok()
}

pub(crate) fn duration_from_ticks(ticks: u32, quantum_micros: u64) -> Duration {
    Duration::from_micros(quantum_micros * u64::from(ticks))
}
