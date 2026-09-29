//! The status codes the BLE stack returns besides the Bluetooth Core's.
//!
//! bt-hci names only the Core's error codes, so it shows ST's as unknown.
//! [`BleStatus`] names them, and is checked at compile time to stand for
//! exactly the codes `ble_defs.h` defines in the selected release.
//! [`StatusError`] names a failed command's status either way:
//!
//! ```ignore
//! match controller.exec(&command).await {
//!     Err(bt_hci::cmd::Error::Hci(error)) => match StatusError::from(error) {
//!         StatusError::Stack(BleStatus::InsufficientResources) => retry(),
//!         other => return Err(other),
//!     },
//!     ...
//! }
//! ```

use core::fmt;

use bt_hci::param::{Error, Status};

use crate::wire::WireValue;
use crate::wire_values;

wire_values! {
    /// A failure the BLE stack reports with one of its own status codes.
    pub enum BleStatus: u8 {
        /// The peer is in the blacklist, so the pairing it requested cannot
        /// be performed.
        DeviceInBlacklist = 0x59,
        /// No CSRK was found to validate an incoming signed packet.
        CsrkNotFound = 0x5A,
        /// No IRK was found.
        IrkNotFound = 0x5B,
        /// No bonded or volatile record of the device exists.
        DeviceNotFound = 0x5C,
        /// The security database is full, before 1.17.1.
        #[cfg(any(feature = "fw_1_15_0", feature = "fw_1_16_0", feature = "fw_1_17_0"))]
        SecurityDatabaseFull = 0x5D,
        /// The peer is not bonded, so no operation on bonded devices applies.
        DeviceNotBonded = 0x5E,
        /// The encryption key is too short, before 1.17.1.
        #[cfg(any(feature = "fw_1_15_0", feature = "fw_1_16_0", feature = "fw_1_17_0"))]
        InsufficientEncryptionKeySize = 0x5F,
        /// The attribute handle is invalid.
        InvalidHandle = 0x60,
        /// No attribute handles are left for a service, characteristic or
        /// descriptor.
        OutOfHandles = 0x61,
        /// The GATT rules do not allow the operation here.
        InvalidOperation = 0x62,
        /// The characteristic already exists, before 1.17.1.
        #[cfg(any(feature = "fw_1_15_0", feature = "fw_1_16_0", feature = "fw_1_17_0"))]
        CharacteristicAlreadyExists = 0x63,
        /// Resources such as packets or timers are short for now; the
        /// operation may be retried.
        InsufficientResources = 0x64,
        /// The peer lacks the security a notification or indication needs.
        SecurityPermissionError = 0x65,
        /// The address could not be resolved with the stored IRKs.
        AddressNotResolved = 0x70,
        /// No radio slot is free.
        NoValidSlot = 0x82,
        /// The only free slot is shorter than the scan window.
        ScanWindowShort = 0x83,
        /// No interval in the requested range fits the anchor period.
        NewIntervalFailed = 0x84,
        /// The requested interval is too large for the anchor period.
        IntervalTooLarge = 0x85,
        /// The longest free slot is shorter than the requested length.
        LengthFailed = 0x86,
        /// The host failed to perform the operation.
        Failed = 0x91,
        /// A host command parameter is invalid.
        InvalidParameters = 0x92,
        /// The host is processing another request.
        Busy = 0x93,
        /// The operation cannot complete now and is put on hold.
        Pending = 0x95,
        /// The operation breaks the logic of the layer or the format of its
        /// data.
        LogicError = 0x97,
        /// Memory is exhausted for good, as for the ATT database.
        OutOfMemory = 0x98,
        /// A timeout occurred at the BLE application interface.
        Timeout = 0xFF,
    }
}

stm32wb_hci_macros::check_statuses!(BleStatus);

impl BleStatus {
    /// The BLE stack's status `status` carries, or `None` for success or a
    /// Bluetooth Core error.
    pub fn from_status(status: Status) -> Option<Self> {
        Self::from_raw(status.into())
    }
}

impl From<BleStatus> for Status {
    fn from(status: BleStatus) -> Self {
        Status::new(status.into())
    }
}

impl fmt::Display for BleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Why a command failed: one of the BLE stack's status codes, or a
/// Bluetooth Core error, which bt-hci names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum StatusError {
    /// A code of the BLE stack.
    Stack(BleStatus),
    /// A Bluetooth Core error code.
    Core(Error),
}

impl From<Error> for StatusError {
    fn from(error: Error) -> Self {
        match BleStatus::from_status(error.to_status()) {
            Some(status) => Self::Stack(status),
            None => Self::Core(error),
        }
    }
}

impl From<StatusError> for Status {
    fn from(error: StatusError) -> Self {
        match error {
            StatusError::Stack(status) => status.into(),
            StatusError::Core(error) => error.to_status(),
        }
    }
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stack(status) => write!(f, "{status}"),
            Self::Core(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for StatusError {}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::format;

    use super::*;
    use crate::wire::values_exactly;

    #[test]
    fn stack_codes_are_named() {
        let failed = Status::new(0x91).to_result().unwrap_err();
        assert_eq!(
            StatusError::from(failed),
            StatusError::Stack(BleStatus::Failed)
        );
        assert_eq!(format!("{}", StatusError::from(failed)).as_str(), "Failed");
        assert_eq!(Status::from(StatusError::from(failed)), Status::new(0x91));
        assert_eq!(BleStatus::from_status(Status::SUCCESS), None);
    }

    #[test]
    fn core_codes_keep_their_bt_hci_names() {
        let error = Status::UNKNOWN_CMD.to_result().unwrap_err();
        assert_eq!(StatusError::from(error), StatusError::Core(error));
        assert_eq!(
            format!("{}", StatusError::from(error)).as_str(),
            "Unknown HCI Command"
        );
    }

    #[test]
    fn statuses_must_be_exactly_the_documented_codes() {
        assert!(
            !values_exactly::<BleStatus>(&[0x91]),
            "a value is undocumented"
        );
        #[cfg(feature = "fw_1_24_0")]
        {
            let documented = [
                0x59, 0x5A, 0x5B, 0x5C, 0x5E, 0x60, 0x61, 0x62, 0x64, 0x65, 0x70, 0x82, 0x83, 0x84,
                0x85, 0x86, 0x91, 0x92, 0x93, 0x95, 0x97, 0x98, 0xFF,
            ];
            assert!(values_exactly::<BleStatus>(&documented));
            assert!(
                !values_exactly::<BleStatus>(&[&documented[..], &[0x5D]].concat()),
                "a documented code is missing"
            );
        }
    }
}
