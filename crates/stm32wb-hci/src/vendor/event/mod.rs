//! Vendor-specific events for BlueNRG controllers.
//!
//! The BlueNRG implementation defines several additional events that are packaged as
//! vendor-specific events by the Bluetooth HCI. This module defines those events and functions to
//! deserialize buffers into them.

use byteorder::{ByteOrder, LittleEndian};
use core::cmp::PartialEq;
use core::convert::{TryFrom, TryInto};
use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::time::Duration;

use crate::types::PeerAddrType;
pub use crate::types::{BdAddrType, ConnectionInterval, ConnectionIntervalError};
use crate::vendor::command::gap::EventFlags;
pub use crate::vendor::command::{BoundedBytes, BoundedItems};
use bt_hci::param::{BdAddr, ConnHandle};

/// Enumeration of vendor-specific status codes.
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VendorStatus {
    /// The command cannot be executed due to the current state of the device.
    Failed = 0x41,
    /// Some parameters are invalid.
    InvalidParameters = 0x42,
    /// It is not allowed to start the procedure (e.g. another the procedure is ongoing or cannot be
    /// started on the given handle).
    NotAllowed = 0x46,
    /// Unexpected error.
    Error = 0x47,
    /// The address was not resolved.
    AddressNotResolved = 0x48,
    /// Failed to read from flash.
    FlashReadFailed = 0x49,
    /// Failed to write to flash.
    FlashWriteFailed = 0x4A,
    /// Failed to erase flash.
    FlashEraseFailed = 0x4B,
    /// Invalid CID
    InvalidCid = 0x50,
    /// Timer is not valid
    TimerNotValidLayer = 0x54,
    /// Insufficient resources to create the timer
    TimerInsufficientResources = 0x55,
    /// Connection signature resolving key (CSRK) is not found.
    CsrkNotFound = 0x5A,
    /// Identity resolving key (IRK) is not found
    IrkNotFound = 0x5B,
    /// The device is not in the security database.
    DeviceNotFoundInDatabase = 0x5C,
    /// The security database is full.
    SecurityDatabaseFull = 0x5D,
    /// The device is not bonded.
    DeviceNotBonded = 0x5E,
    /// The device is blacklisted.
    DeviceInBlacklist = 0x5F,
    /// The handle (service, characteristic, or descriptor) is invalid.
    InvalidHandle = 0x60,
    /// A parameter is invalid
    InvalidParameter = 0x61,
    /// The characteristic handle is not part of the service.
    OutOfHandle = 0x62,
    /// The operation is invalid
    InvalidOperation = 0x63,
    /// Insufficient resources to complete the operation.
    InsufficientResources = 0x64,
    /// The encryption key size is too small
    InsufficientEncryptionKeySize = 0x65,
    /// The characteristic already exists.
    CharacteristicAlreadyExists = 0x66,
    /// Returned when no valid slots are available (e.g. when there are no available state
    /// machines).
    NoValidSlot = 0x82,
    /// Returned when a scan window shorter than minimum allowed value has been requested
    /// (i.e. 2ms). The Rust API should prevent this error from occurring.
    ScanWindowTooShort = 0x83,
    /// Returned when the maximum requested interval to be allocated is shorter then the current
    /// anchor period and a there is no submultiple for the current anchor period that is between
    /// the minimum and the maximum requested intervals.
    NewIntervalFailed = 0x84,
    /// Returned when the maximum requested interval to be allocated is greater than the current
    /// anchor period and there is no multiple of the anchor period that is between the minimum and
    /// the maximum requested intervals.
    IntervalTooLarge = 0x85,
    /// Returned when the current anchor period or a new one can be found that is compatible to the
    /// interval range requested by the new slot but the maximum available length that can be
    /// allocated is less than the minimum requested slot length.
    LengthFailed = 0x86,
    /// MCU Library timed out.
    Timeout = 0xFF,
    /// MCU library: profile already initialized.
    ProfileAlreadyInitialized = 0xF0,
    /// MCU library: A parameter was null.
    NullParameter = 0xF1,
}

impl TryFrom<u8> for VendorStatus {
    type Error = BadVendorStatusError;

    fn try_from(value: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match value {
            0x41 => Ok(VendorStatus::Failed),
            0x42 => Ok(VendorStatus::InvalidParameters),
            0x46 => Ok(VendorStatus::NotAllowed),
            0x47 => Ok(VendorStatus::Error),
            0x48 => Ok(VendorStatus::AddressNotResolved),
            0x49 => Ok(VendorStatus::FlashReadFailed),
            0x4A => Ok(VendorStatus::FlashWriteFailed),
            0x4B => Ok(VendorStatus::FlashEraseFailed),
            0x50 => Ok(VendorStatus::InvalidCid),
            0x54 => Ok(VendorStatus::TimerNotValidLayer),
            0x55 => Ok(VendorStatus::TimerInsufficientResources),
            0x5A => Ok(VendorStatus::CsrkNotFound),
            0x5B => Ok(VendorStatus::IrkNotFound),
            0x5C => Ok(VendorStatus::DeviceNotFoundInDatabase),
            0x5D => Ok(VendorStatus::SecurityDatabaseFull),
            0x5E => Ok(VendorStatus::DeviceNotBonded),
            0x5F => Ok(VendorStatus::DeviceInBlacklist),
            0x60 => Ok(VendorStatus::InvalidHandle),
            0x61 => Ok(VendorStatus::InvalidParameter),
            0x62 => Ok(VendorStatus::OutOfHandle),
            0x63 => Ok(VendorStatus::InvalidOperation),
            0x64 => Ok(VendorStatus::InsufficientResources),
            0x65 => Ok(VendorStatus::InsufficientEncryptionKeySize),
            0x66 => Ok(VendorStatus::CharacteristicAlreadyExists),
            0x82 => Ok(VendorStatus::NoValidSlot),
            0x83 => Ok(VendorStatus::ScanWindowTooShort),
            0x84 => Ok(VendorStatus::NewIntervalFailed),
            0x85 => Ok(VendorStatus::IntervalTooLarge),
            0x86 => Ok(VendorStatus::LengthFailed),
            0xFF => Ok(VendorStatus::Timeout),
            0xF0 => Ok(VendorStatus::ProfileAlreadyInitialized),
            0xF1 => Ok(VendorStatus::NullParameter),
            _ => Err(BadVendorStatusError(value)),
        }
    }
}

/// A byte that does not identify an STM32WB vendor status.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BadVendorStatusError(pub u8);

impl From<VendorStatus> for u8 {
    fn from(val: VendorStatus) -> Self {
        val as u8
    }
}

/// Enumeration of potential errors when sending commands or deserializing events.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VendorError {
    /// The event is not recognized. Includes the unknown opcode.
    UnknownEvent(u16),

    /// For the [CoprocessorReady](VendorEvent::CoprocessorReady) event: the kind of firmware
    /// running on radio coprocessor is not recognized.
    UnknownFirmwareKind(u8),

    /// For the [GAP Pairing Complete](VendorEvent::GapPairingComplete) event: The status was not
    /// recognized. Includes the unrecognized byte.
    BadGapPairingStatus(u8),

    /// For the [GAP Pairing Complete](VendorEvent::GapPairingComplete) event: The error reason
    /// was not recognized. Includes the unrecognized byte.
    BadGapPairingErrorReason(u8),

    /// For the GAP Device Found event: the type of event was not
    /// recognized. Includes the unrecognized byte.
    BadGapDeviceFoundEvent(u8),

    /// For the GAP Device Found event: the type of BDADDR was not
    /// recognized. Includes the unrecognized byte.
    BadGapBdAddrType(u8),

    /// For the [GAP Procedure Complete](VendorEvent::GapProcedureComplete) event: The procedure
    /// code was not recognized. Includes the unrecognized byte.
    BadGapProcedure(u8),

    /// For the [GAP Procedure Complete](VendorEvent::GapProcedureComplete) event: The procedure
    /// status was not recognized. Includes the unrecognized byte.
    BadGapProcedureStatus(u8),

    /// For any L2CAP event: The event data length did not match the expected length. The first
    /// field is the required length, and the second is the actual length.
    BadL2CapDataLength(u8, u8),

    /// For any L2CAP event: The L2CAP length did not match the expected length. The first field is
    /// the required length, and the second is the actual length.
    BadL2CapLength(u16, u16),

    /// For any L2CAP response event: The L2CAP command was rejected, but the rejection reason was
    /// not recognized. Includes the unknown value.
    BadL2CapRejectionReason(u16),

    /// For the [L2CAP Connection Update Response](VendorEvent::L2CapConnectionUpdateResponse)
    /// event: The code byte did not indicate either Rejected or Updated. Includes the invalid byte.
    BadL2CapConnectionResponseCode(u8),

    /// For the [L2CAP Connection Update Response](VendorEvent::L2CapConnectionUpdateResponse)
    /// event: The command was accepted, but the result was not recognized. It did not indicate the
    /// parameters were either updated or rejected. Includes the unknown value.
    BadL2CapConnectionResponseResult(u16),

    /// For the [L2CAP Connection Update Request](VendorEvent::L2CapConnectionUpdateRequest) event:
    /// The provided connection interval is invalid. Includes the underlying error.
    BadConnectionInterval(ConnectionIntervalError),

    /// For the [L2CAP Connection Update Request](VendorEvent::L2CapConnectionUpdateRequest) event:
    /// The provided interval is invalid. Potential errors:
    /// - Either the minimum or maximum is out of range. The minimum value for either is 7.5 ms, and
    ///   the maximum is 4 s.
    /// - The min is greater than the max
    ///
    /// See the Bluetooth specification, Vol 3, Part A, Section 4.20. Versions 4.1, 4.2 and 5.0.
    ///
    /// Inclues the provided minimum and maximum, respectively.
    BadL2CapConnectionUpdateRequestInterval(Duration, Duration),

    /// For the [L2CAP Connection Update Request](VendorEvent::L2CapConnectionUpdateRequest) event:
    /// The provided connection latency is invalid. The maximum value for connection latency is
    /// defined in terms of the timeout and maximum connection interval.
    /// - `connIntervalMax = Interval Max`
    /// - `connSupervisionTimeout = Timeout`
    /// - `maxConnLatency = min(500, ((connSupervisionTimeout / (2 * connIntervalMax)) - 1))`
    ///
    /// See the Bluetooth specification, Vol 3, Part A, Section 4.20. Versions 4.1, 4.2 and 5.0.
    ///
    /// Inclues the provided value and maximum allowed value, respectively.
    BadL2CapConnectionUpdateRequestLatency(u16, u16),

    /// For the [L2CAP Connection Update Request](VendorEvent::L2CapConnectionUpdateRequest) event:
    /// The provided timeout is invalid. The timeout field shall have a value in the range of 100 ms
    /// to 32 seconds (inclusive).
    ///
    /// See the Bluetooth specification, Vol 3, Part A, Section 4.20. Versions 4.1, 4.2 and 5.0.
    ///
    /// Inclues the provided value.
    BadL2CapConnectionUpdateRequestTimeout(Duration),

    /// For the [ATT Find Information Response](VendorEvent::AttFindInformationResponse) event: The
    /// format code is invalid. Includes the unrecognized byte.
    BadAttFindInformationResponseFormat(u8),

    /// For the [ATT Find Information Response](VendorEvent::AttFindInformationResponse) event: The
    /// format code indicated 16-bit UUIDs, but the packet ends with a partial pair.
    AttFindInformationResponsePartialPair16,

    /// For the [ATT Find Information Response](VendorEvent::AttFindInformationResponse) event: The
    /// format code indicated 128-bit UUIDs, but the packet ends with a partial pair.
    AttFindInformationResponsePartialPair128,

    /// For the [ATT Find by Type Value Response](VendorEvent::AttFindByTypeValueResponse) event:
    /// The packet ends with a partial attribute pair.
    AttFindByTypeValuePartial,

    /// For the [ATT Read by Type Response](VendorEvent::AttReadByTypeResponse) event: The packet
    /// ends with a partial attribute handle-value pair.
    AttReadByTypeResponsePartial,

    /// For the [ATT Read by Group Type Response](VendorEvent::AttReadByGroupTypeResponse) event:
    /// The packet ends with a partial attribute data group.
    AttReadByGroupTypeResponsePartial,

    /// For the [GATT Procedure Complete](VendorEvent::GattProcedureComplete) event: The status
    /// code was not recognized. Includes the unrecognized byte.
    BadGattProcedureStatus(u8),

    /// For the [ATT Error Response](VendorEvent::AttErrorResponse) event: The request opcode was
    /// not recognized. Includes the unrecognized byte.
    BadAttRequestOpcode(u8),

    /// For the [ATT Error Response](VendorEvent::AttErrorResponse) event: The error code was not
    /// recognized. Includes the unrecognized byte.
    BadAttError(u8),

    /// For the [ATT Read Multiple Permit Request](VendorEvent::AttReadMultiplePermitRequest)
    /// event: The packet ends with a partial attribute handle.
    AttReadMultiplePermitRequestPartial,

    /// A field that is defined as a Boolean was neither 0 nor 1. The unknown
    /// value is provided.
    BadBooleanValue(u8),

    /// A vendor event contained an invalid Bluetooth address-type byte.
    BadBdAddrType(u8),

    /// For the [GATT EAT Bearer](crate::vendor::event::VendorEvent::GattEattBrearer) event: The EAB state was not recognized.
    BadEabState(u8),

    /// For the [HAL End Of Radio Activity](VendorEvent::HalEndOfRadioActivity) event: The Radio Event code was not recognized.
    BadRadioEvent(u8),

    /// For the [HAL Firmware Error](VendorEvent::HalFirmwareError) event: The Radio Event code was not recognized.
    BadFirmwareError(u8),
}

/// Errors produced while decoding an STM32WB vendor event.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// A buffer did not have the expected length.
    ///
    /// The first field is the observed length and the second is the required
    /// or maximum accepted length, depending on the payload being decoded.
    BadLength(usize, usize),
    /// The vendor event contained an invalid or unknown value.
    Vendor(VendorError),
}

impl From<VendorError> for Error {
    fn from(error: VendorError) -> Self {
        Self::Vendor(error)
    }
}

fn first_16<T>(buffer: &[T]) -> &[T] {
    if buffer.len() < 16 {
        buffer
    } else {
        &buffer[..16]
    }
}

/// A value decoded from an exact-width field in a vendor event.
///
/// `N` is part of the trait so `field: Type => N` only compiles when `Type`
/// explicitly supports that wire width. Implementations must decode the
/// protocol representation rather than relying on Rust layout or native
/// endianness.
///
/// An unsupported type/width pair is a compile-time error:
///
/// ```compile_fail
/// use stm32wb_hci::vendor::event::HciEventField;
///
/// fn requires_two_bytes<T: HciEventField<2>>() {}
/// requires_two_bytes::<bool>();
/// ```
pub trait HciEventField<const N: usize>: Sized {
    /// Decode one exact-width vendor-event field.
    fn from_hci_event_field(bytes: &[u8; N]) -> Result<Self, Error>;
}

macro_rules! impl_hci_event_integer_field {
    ($ty:ty, $len:literal) => {
        impl HciEventField<$len> for $ty {
            fn from_hci_event_field(bytes: &[u8; $len]) -> Result<Self, Error> {
                Ok(<$ty>::from_le_bytes(*bytes))
            }
        }
    };
}

impl_hci_event_integer_field!(u8, 1);
impl_hci_event_integer_field!(u16, 2);
impl_hci_event_integer_field!(u32, 4);

impl<const N: usize> HciEventField<N> for [u8; N] {
    fn from_hci_event_field(bytes: &[u8; N]) -> Result<Self, Error> {
        Ok(*bytes)
    }
}

impl HciEventField<1> for bool {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        match bytes[0] {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(Error::Vendor(VendorError::BadBooleanValue(value))),
        }
    }
}

impl HciEventField<2> for ConnHandle {
    fn from_hci_event_field(bytes: &[u8; 2]) -> Result<Self, Error> {
        Ok(Self(u16::from_le_bytes(*bytes)))
    }
}

impl HciEventField<2> for AttributeHandle {
    fn from_hci_event_field(bytes: &[u8; 2]) -> Result<Self, Error> {
        Ok(Self(u16::from_le_bytes(*bytes)))
    }
}

fn decode_hci_event_field<T, const N: usize>(
    data: &[u8],
    original_len: usize,
) -> Result<(T, &[u8]), Error>
where
    T: HciEventField<N>,
{
    if data.len() < N {
        return Err(Error::BadLength(
            original_len,
            original_len - data.len() + N,
        ));
    }
    let (field, rest) = data.split_at(N);
    let field = field
        .try_into()
        .expect("split_at returned the declared width");
    T::from_hci_event_field(field).map(|value| (value, rest))
}

fn decode_hci_event_counted_bytes<T, C, const COUNT_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
    original_len: usize,
) -> Result<(T, &[u8]), Error>
where
    T: crate::vendor::command::HciDecodeCountedBytes<C, COUNT_LEN, MAX_LEN>,
    C: HciEventField<COUNT_LEN>
        + crate::vendor::command::HciDecodeField<COUNT_LEN>
        + crate::vendor::command::HciCount<COUNT_LEN>,
{
    let (count, after_count) = decode_hci_event_field::<C, COUNT_LEN>(data, original_len)?;
    let len = crate::vendor::command::HciCount::to_usize(count);
    if len > MAX_LEN {
        return Err(Error::BadLength(len, MAX_LEN));
    }
    if after_count.len() < len {
        return Err(Error::BadLength(
            original_len,
            original_len - after_count.len() + len,
        ));
    }
    <T as crate::vendor::command::HciDecodeCountedBytes<C, COUNT_LEN, MAX_LEN>>::decode_counted_bytes(
        data,
    )
    .map_err(|_| Error::BadLength(original_len, COUNT_LEN + len))
}

fn decode_hci_event_counted_items<
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>(
    data: &[u8],
    original_len: usize,
) -> Result<(T, &[u8]), Error>
where
    T: crate::vendor::command::HciDecodeCountedItems<Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>,
    Item: Copy + crate::vendor::command::HciDecodeField<ITEM_LEN>,
    C: HciEventField<COUNT_LEN>
        + crate::vendor::command::HciDecodeField<COUNT_LEN>
        + crate::vendor::command::HciCount<COUNT_LEN>,
{
    let (count, after_count) = decode_hci_event_field::<C, COUNT_LEN>(data, original_len)?;
    let count = crate::vendor::command::HciCount::to_usize(count);
    if count > MAX_ITEMS {
        return Err(Error::BadLength(count, MAX_ITEMS));
    }
    let len = count
        .checked_mul(ITEM_LEN)
        .ok_or(Error::BadLength(count, MAX_ITEMS))?;
    if after_count.len() < len {
        return Err(Error::BadLength(
            original_len,
            original_len - after_count.len() + len,
        ));
    }
    <T as crate::vendor::command::HciDecodeCountedItems<
        Item,
        C,
        COUNT_LEN,
        ITEM_LEN,
        MAX_ITEMS,
    >>::decode_counted_items(data)
    .map_err(|_| Error::BadLength(original_len, COUNT_LEN + len))
}

#[allow(dead_code)]
fn decode_hci_event_trailing_bytes<T, const MIN_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
) -> Result<(T, &[u8]), Error>
where
    T: crate::vendor::command::HciDecodeTrailingBytes<MIN_LEN, MAX_LEN>,
{
    if !(MIN_LEN..=MAX_LEN).contains(&data.len()) {
        let expected = if data.len() < MIN_LEN {
            MIN_LEN
        } else {
            MAX_LEN
        };
        return Err(Error::BadLength(data.len(), expected));
    }
    <T as crate::vendor::command::HciDecodeTrailingBytes<MIN_LEN, MAX_LEN>>::decode_trailing_bytes(
        data,
    )
    .map_err(|_| Error::BadLength(data.len(), MAX_LEN))
}

stm32wb_hci_macros::vendor_event! {
    /// When the radio coprocessor firmware is started normally, it gives this event to the user to
    /// indicate the system has started.
    CoprocessorReady(0x9200) {
        Payload = { kind: FirmwareKind => 1, };
    }
    /// This event is generated when teh device completes a radio activity and provide information when
    /// a new radio activity will be performed.
    ///
    /// Information provided includes type of radio activity and absolute time in system ticks when a
    /// radio acitivity is scheduled, if any. The application can use this information to schedule user
    /// activities synchronous to selected radio activities. A command
    /// [Set Radio Activity Mask](crate::vendor::command::hal::HalSetRadioActivityMask) is
    /// provided to enable radio activity events of user interests, by default no events are enabled.
    ///
    /// The user should take into account that enabling radio events in an application with intense
    /// radio activity could lead to a fairly high rate of events generated.
    ///
    /// Application use cases indlude synchronizing notifications with connection intervals, switching
    /// antenna at the end of advertising or performing flash erase while radio is idle.
    HalEndOfRadioActivity(0x0004) {
        Payload = {
            last_state: RadioEvent => 1,
            next_state: RadioEvent => 1,
            next_state_sys_time: u32 => 4,
            last_state_slot: u8 => 1,
            next_state_slot: u8 => 1,
        };
    }
    /// This event is reported to the application after a scan request is received and a scan response is
    /// scheduled to be transmitted.
    ///
    /// Note: RSSI in this event is valid only when privacy is not used
    HalScanReqReport(0x0005) {
        Payload = {
            rssi: u8 => 1,
            peer_addr: PeerAddrType => 7,
        };
    }
    /// This event is generated to report firmware error information
    HalFirmwareError(0x0006) {
        Payload = {
            fw_error_type: FirmwareError => 1,
            data: BoundedBytes<251> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 251,
            },
        };
    }
    /// This event is generated by the controller when the limited discoverable mode ends due to
    /// timeout (180 seconds).
    GapLimitedDiscoverableTimeout(0x0400) {
        Payload = ();
    }
    /// This event is generated when the pairing process has completed successfully or a pairing
    /// procedure timeout has occurred or the pairing has failed.  This is to notify the application
    /// that we have paired with a remote device so that it can take further actions or to notify
    /// that a timeout has occurred so that the upper layer can decide to disconnect the link.
    GapPairingComplete(0x0401) {
        Payload = {
            conn_handle: ConnHandle => 2,
            status: GapPairingStatus => 2,
        };
    }
    /// This event is generated by the Security manager to the application when a pass key is
    /// required for pairing.  When this event is received, the application has to respond with the
    /// `gap_pass_key_response` command.
    GapPassKeyRequest(0x0402) {
        Payload = { conn_handle: ConnHandle => 2, };
    }
    /// This event is generated by the Security manager to the application when the application has
    /// set that authorization is required for reading/writing of attributes. This event will be
    /// generated as soon as the pairing is complete. When this event is received,
    /// `gap_authorization_response` command should be used by the application.
    GapAuthorizationRequest(0x0403) {
        Payload = { conn_handle: ConnHandle => 2, };
    }
    /// This event is generated when the peripheral security request is successfully sent to the
    /// central device.
    GapPeripheralSecurityInitiated(0x0404) {
        Payload = ();
    }
    /// This event is generated on the peripheral when a `gap_peripheral_security_request` is called
    /// to reestablish the bond with the central device but the central device has lost the
    /// bond. When this event is received, the upper layer has to issue the command
    /// `gap_allow_rebond` in order to allow the peripheral to continue the pairing process with the
    /// central device. On the central device, this event is raised when `gap_send_pairing_request`
    /// is called to reestablish a bond with a peripheral but the peripheral has lost the bond. In
    /// order to create a new bond the central device has to launch `gap_send_pairing_request` with
    /// `force_rebond` set to `true`.
    GapBondLost(0x0405) {
        Payload = ();
    }
    /// This event is sent by the GAP to the upper layers when a procedure previously started has
    /// been terminated by the upper layer or has completed for any other reason
    GapProcedureComplete(0x0407) {
        Payload = {
            procedure: GapProcedureKind => 1,
            status: GapProcedureStatus => 1,
            data: BoundedBytes<250> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
    }
    /// This event is sent only by a privacy enabled peripheral. with a non-empty bonded device list
    ///  The event is sent to the application when the peripheral is unsuccessful in resolving
    /// the resolvable address of the peer device after connecting to it.
    GapAddressNotResolved(0x0408) {
        Payload = { conn_handle: ConnHandle => 2, };
    }
    /// This event is sent only during SC Pairing, when Numeric Comparison
    /// Association model is selected, in order to show the Numeric Value generated,
    /// and to ask for Confirmation to the User. When this event is received, the
    /// application has to respond with the
    /// [numeric_comparison_value_confirm_yes_no](super::command::gap::GapConfirmNumericComparisonValue)
    /// command.
    GapNumericComparisonValue(0x0409) {
        Payload = {
            connection_handle: ConnHandle => 2,
            numeric_value: u32 => 4,
        };
    }
    /// This event is sent only during SC Pairing, when Keypress Notifications are
    /// supported, in order to show the input type signaled by the peer device,
    /// having Keyboard only I/O capabilities. When this event is received, no
    /// action is required to the User.
    GapKeypressNotification(0x040A) {
        Payload = {
            connection_handle: ConnHandle => 2,
            notification_type: KeypressNotificationType => 1,
        };
    }
    /// This event is generated when SMP mode is configured to surface pairing
    /// requests to the host.
    ///
    /// The host should answer using the GAP pairing-request-reply command.
    #[cfg(since_fw_0_17_1)]
    GapPairingRequest(0x040B) {
        Payload = {
            connection_handle: ConnHandle => 2,
            bonded: bool => 1,
            auth_req: u8 => 1,
        };
    }
    /// This event is generated when the central device responds to the L2CAP connection update
    /// request packet. For more info see
    /// [L2ConnectionParameterUpdateResponse](crate::vendor::command::l2cap::L2ConnectionParameterUpdateResponse)
    /// and CommandReject in Bluetooth Core v4.0 spec.
    L2CapConnectionUpdateResponse(0x0800) {
        Payload = {
            conn_handle: ConnHandle => 2,
            result: L2CapConnectionUpdateResult => 2,
        };
    }
    /// This event is generated when the central device does not respond to the connection update
    /// request within 30 seconds.
    L2CapProcedureTimeout(0x0801) {
        Payload = {
            conn_handle: ConnHandle => 2,
            _data: EmptyL2CapData => {
                kind: payload,
                min_len: 1,
                max_len: 251,
            },
        };
    }
    /// The event is given by the L2CAP layer when a connection update request is received from the
    /// peripheral. The application has to respond by calling
    /// [l2cap_connection_parameter_update_response](crate::vendor::command::l2cap::L2ConnectionParameterUpdateResponse).
    L2CapConnectionUpdateRequest(0x0802) {
        Payload = {
            conn_handle: ConnHandle => 2,
            identifier: u8 => 1,
            l2cap_length: u16 => 2,
            conn_interval: ConnectionInterval => 8,
        };
    }
    /// This event is generated upon receipt of a valid Command Reject packet (e.g.
    /// when the Central responds to the Connection Update Request packet with a
    /// Command Reject packet).
    L2CapCommandReject(0x080A) {
        Payload = {
            conn_handle: ConnHandle => 2,
            identifier: u8 => 1,
            reason: u16 => 2,
            data: BoundedBytes<247> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 247,
            },
        };
    }
    /// This event is generated when receiving a valid Credit Based Connection Request packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocConnect(0x0810) {
        Payload = {
            conn_handle: ConnHandle => 2,
            spsm: u16 => 2,
            mtu: u16 => 2,
            mps: u16 => 2,
            initial_credits: u16 => 2,
            channel_number: u8 => 1,
        };
    }
    /// This event is generated when receiving a valid Credit Based Connection Response packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocConnectConfirm(0x0811) {
        Payload = {
            conn_handle: ConnHandle => 2,
            mtu: u16 => 2,
            mps: u16 => 2,
            initial_credits: u16 => 2,
            result: u16 => 2,
            channel_indices: L2CapChannelIndices<0, 242> => {
                kind: payload,
                min_len: 1,
                max_len: 243,
            },
        };
    }
    /// This event is generated when receiving a valid Credit Based Reconfigure Request packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocReconfig(0x0812) {
        Payload = {
            conn_handle: ConnHandle => 2,
            mtu: u16 => 2,
            mps: u16 => 2,
            channel_indices: L2CapChannelIndices<1, 246> => {
                kind: payload,
                min_len: 2,
                max_len: 247,
            },
        };
    }
    /// This event is generated when receiving a valid Credit Based Reconfigure Response packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocReconfigConfirm(0x0813) {
        Payload = {
            conn_handle: ConnHandle => 2,
            result: u16 => 2,
        };
    }
    /// This event is generated when a connection-oriented channel is disconnected following an
    /// L2CAP channel termination procedure.
    ///
    /// Includes the channel index of the connection oriented channel for which the primitive applies
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocDisconnect(0x0814) {
        Payload = { channel_index: u8 => 1, };
    }
    /// This event is generated when receiving a valid Flow Control Credit signaling packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocFlowControl(0x0815) {
        Payload = {
            channel_index: u8 => 1,
            credits: u16 => 2,
        };
    }
    /// This event is generated when receiving a valid K-frame packet on a connection-oriented channel
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    ///
    /// # Note:
    /// For the first K-frame of the SDU, the information data contains the L2CAP SDU length coded in
    /// two octets followed by the K-frame information payload. For the next K-frames of the SDU, the
    /// information data only contains the K-frame information payload.
    L2CapCocRxData(0x0816) {
        Payload = {
            channel_index: u8 => 1,
            data: BoundedBytes<250> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 250,
            },
        };
    }
    /// Each time the [L2CAO COC Tx Data](crate::vendor::command::l2cap::L2CocTxData) command
    /// raises the error code [Insufficient Resources](VendorStatus::InsufficientResources) (0x64), this event
    /// is generated as soon as there is a free buffer available for sending K-frames.
    L2CapCocTxPoolAvailable(0x0817) {
        Payload = ();
    }
    /// This event is generated to the application by the ATT server when a client modifies any
    /// attribute on the server, as consequence of one of the following ATT procedures:
    /// - write without response
    /// - signed write without response
    /// - write characteristic value
    /// - write long characteristic value
    /// - reliable write
    GattAttributeModified(0x0C01) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attr_handle: AttributeHandle => 2,
            offset: u16 => 2,
            data: BoundedBytes<245> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 245,
            },
        };
    }
    /// This event is generated when a ATT client procedure completes either with error or
    /// successfully.
    GattProcedureTimeout(0x0C02) {
        Payload = { conn_handle: ConnHandle => 2, };
    }
    /// This event is generated in response to an Exchange MTU request.
    AttExchangeMtuResponse(0x0C03) {
        Payload = {
            conn_handle: ConnHandle => 2,
            server_rx_mtu: usize => 2,
        };
    }
    /// This event is generated in response to a Find Information Request. See Find Information
    /// Response in Bluetooth Core v4.0 spec.
    AttFindInformationResponse(0x0C04) {
        Payload = {
            conn_handle: ConnHandle => 2,
            handle_uuid_pairs: HandleUuidPairs => {
                kind: payload,
                min_len: 2,
                max_len: 251,
            },
        };
    }
    /// This event is generated in response to a Find By Type Value Request.
    AttFindByTypeValueResponse(0x0C05) {
        Payload = {
            conn_handle: ConnHandle => 2,
            handles: BoundedItems<HandleInfoPair, 62> => {
                kind: counted_items,
                count: u8 => 1,
                item: HandleInfoPair => 4,
                max_items: 62,
            },
        };
    }
    /// This event is generated in response to a Read by Type Request.
    AttReadByTypeResponse(0x0C06) {
        Payload = {
            conn_handle: ConnHandle => 2,
            pairs: HandleValuePairs => {
                kind: payload,
                min_len: 2,
                max_len: 251,
            },
        };
    }
    /// This event is generated in response to a Read Request.
    AttReadResponse(0x0C07) {
        Payload = {
            conn_handle: ConnHandle => 2,
            value: BoundedBytes<250> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
    }
    /// This event is generated in response to a Read Blob Request. The value in the response is the
    /// partial value starting from the offset in the request. See the Bluetooth Core v4.1 spec, Vol
    /// 3, section 3.4.4.5 and 3.4.4.6.
    AttReadBlobResponse(0x0C08) {
        Payload = {
            conn_handle: ConnHandle => 2,
            value: BoundedBytes<250> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
    }
    /// This event is generated in response to a Read Multiple Request. The value in the response is
    /// the set of values requested from the request. See the Bluetooth Core v4.1 spec, Vol 3,
    /// section 3.4.4.7 and 3.4.4.8.
    AttReadMultipleResponse(0x0C09) {
        Payload = {
            conn_handle: ConnHandle => 2,
            value: BoundedBytes<250> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
    }
    /// This event is generated in response to a Read By Group Type Request. See the Bluetooth Core
    /// v4.1 spec, Vol 3, section 3.4.4.9 and 3.4.4.10.
    AttReadByGroupTypeResponse(0x0C0A) {
        Payload = {
            conn_handle: ConnHandle => 2,
            groups: AttributeGroups => {
                kind: payload,
                min_len: 2,
                max_len: 251,
            },
        };
    }
    /// This event is generated in response to a Prepare Write Request. See the Bluetooth Core v4.1
    /// spec, Vol 3, Part F, section 3.4.6.1 and 3.4.6.2
    AttPrepareWriteResponse(0x0C0C) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: usize => 2,
            value: BoundedBytes<246> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 246,
            },
        };
    }
    /// This event is generated in response to an Execute Write Request. See the Bluetooth Core v4.1
    /// spec, Vol 3, Part F, section 3.4.6.3 and 3.4.6.4
    AttExecuteWriteResponse(0x0C0D) {
        Payload = { conn_handle: ConnHandle => 2, };
    }
    /// This event is generated when an indication is received from the server.
    GattIndication(0x0C0E) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            value: BoundedBytes<248> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
    }
    /// This event is generated when an notification is received from the server.
    GattNotification(0x0C0F) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            value: BoundedBytes<248> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
    }
    /// This event is generated when a GATT client procedure completes either with error or
    /// successfully.
    GattProcedureComplete(0x0C10) {
        Payload = {
            conn_handle: ConnHandle => 2,
            status: GattProcedureStatus => 1,
        };
    }
    /// This event is generated when an Error Response is received from the server. The error
    /// response can be given by the server at the end of one of the GATT discovery procedures. This
    /// does not mean that the procedure ended with an error, but this error event is part of the
    /// procedure itself.
    AttErrorResponse(0x0C11) {
        Payload = {
            conn_handle: ConnHandle => 2,
            request: AttRequest => 1,
            attribute_handle: AttributeHandle => 2,
            error: AttError => 1,
        };
    }
    /// This event can be generated during a "Discover Characteristics by UUID" procedure or a "Read
    /// using Characteristic UUID" procedure. The attribute value will be a service declaration as
    /// defined in Bluetooth Core v4.0 spec, Vol 3, Part G, section 3.3.1), when a "Discover
    /// Characteristics By UUID" has been started. It will be the value of the Characteristic if a
    /// "Read using Characteristic UUID" has been performed.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part G, section 4.6.2 (discover characteristics by
    /// UUID), and section 4.8.2 (read using characteristic using UUID).
    GattDiscoverOrReadCharacteristicByUuidResponse(0x0C12) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            value: BoundedBytes<248> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
    }
    /// This event is given to the application when a write request, write command or signed write
    /// command is received by the server from the client. This event will be given to the
    /// application only if the event bit for this event generation is set when the characteristic
    /// was added. When this event is received, the application has to check whether the value being
    /// requested for write is allowed to be written and respond with a GATT Write Response. If the
    /// write is rejected by the application, then the value of the attribute will not be
    /// modified. In case of a write request, an error response will be sent to the client, with the
    /// error code as specified by the application. In case of write/signed write commands, no
    /// response is sent to the client but the attribute is not modified.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part F, section 3.4.5.
    AttWritePermitRequest(0x0C13) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            value: BoundedBytes<248> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
    }
    /// This event is given to the application when a read request or read blob request is received
    /// by the server from the client. This event will be given to the application only if the event
    /// bit for this event generation is set when the characteristic was added. On receiving this
    /// event, the application can update the value of the handle if it desires and when done it has
    /// to use the [`allow_read`](crate::vendor::command::gatt::GattAllowRead) command to indicate to the
    /// stack that it can send the response to the client.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part F, section 3.4.4.
    AttReadPermitRequest(0x0C14) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: usize => 2,
        };
    }
    /// This event is given to the application when a read multiple request or read by type request
    /// is received by the server from the client. This event will be given to the application only
    /// if the event bit for this event generation is set when the characteristic was added.  On
    /// receiving this event, the application can update the values of the handles if it desires and
    /// when done it has to send the [`allow_read`](crate::vendor::command::gatt::GattAllowRead) command to
    /// indicate to the stack that it can send the response to the client.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part F, section 3.4.4.
    AttReadMultiplePermitRequest(0x0C15) {
        Payload = {
            conn_handle: ConnHandle => 2,
            handles: BoundedItems<AttributeHandle, 125> => {
                kind: counted_items,
                count: u8 => 1,
                item: AttributeHandle => 2,
                max_items: 125,
            },
        };
    }
    /// This event is raised when the number of available TX buffers is above a threshold TH (TH =
    /// 2).  The event will be given only if a previous ACI command returned with
    /// [InsufficientResources](AttError::InsufficientResources).  On receiving this event, the
    /// application can continue to send notifications by calling `gatt_update_char_value`.
    GattTxPoolAvailable(0x0C16) {
        Payload = {
            conn_handle: ConnHandle => 2,
            available_buffers: usize => 2,
        };
    }
    /// This event is raised on the server when the client confirms the reception of an indication.
    GattServerConfirmation(0x0C17) {
        Payload = { conn_handle: ConnHandle => 2, };
    }
    /// This event is given to the application when a prepare write request is received by the
    /// server from the client. This event will be given to the application only if the event bit
    /// for this event generation is set when the characteristic was added.  When this event is
    /// received, the application has to check whether the value being requested for write is
    /// allowed to be written and respond with the command `gatt_write_response`.  Based on the
    /// response from the application, the attribute value will be modified by the stack.  If the
    /// write is rejected by the application, then the value of the attribute will not be modified
    /// and an error response will be sent to the client, with the error code as specified by the
    /// application.
    AttPrepareWritePermitRequest(0x0C18) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: usize => 2,
            value: BoundedBytes<246> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 246,
            },
        };
    }
    /// This event informs the application of a change in status of the enhanced ATT bearer handled
    /// by the special L2CAP channel.
    GattEattBrearer(0x0C19) {
        Payload = {
            channel_index: u8 => 1,
            eab_state: EabState => 1,
            status: GattProcedureStatus => 1,
        };
    }
    /// This event is generated when a Multiple Handle Value Notification is received from the server.
    GattMultiNotification(0x0C1A) {
        Payload = {
            conn_handle: ConnHandle => 2,
            offset: u16 => 2,
            data: BoundedBytes<247> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 247,
            },
        };
    }
    /// This event is generated on server side after the transmission of all notifications linked with
    /// the a local update of a characteristic value (if it is enabled at the creation of the characteristic
    /// with [GATT Notify Notification Completion](crate::vendor::command::gatt::CharacteristicEvent) mask
    /// and if the characteristic supports notifications).
    #[cfg(since_fw_0_17_0)]
    GattNotificationComplete(0x0C1B) {
        Payload = { attr_handle: AttributeHandle => 2, };
    }
    /// When it is enabled with [set_event_mast](crate::vendor::command::gatt::GattSetEventMask),
    /// this event is generated instead of [ATT Read Response](VendorEvent::AttReadResponse) /
    /// [ATT Read Blob Response](VendorEvent::AttReadBlobResponse) /
    /// [ATT Read Multiple Response](VendorEvent::AttReadMultipleResponse).
    ///
    /// This event should be used instead of those events when `ATT_MTU >
    /// (BLE_EVT_MAX_PARAM_LEN - 4)` i.e. `ATT_MTU > 251` for `BLE_EVT_MAX_PARAM_LEN`
    /// default value.
    GattReadExt(0x0C1D) {
        Payload = {
            conn_handle: ConnHandle => 2,
            offset: u16 => 2,
            value: BoundedBytes<247> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 247,
            },
        };
    }
    /// When it is enabled with [set_event_mast](crate::vendor::command::gatt::GattSetEventMask),
    /// this event is generated instead of [GATT Indication](VendorEvent::GattIndication) event.
    ///
    /// This event should be used instead of `ACI_GATT_INDICATION_EVENT` when
    /// `ATT_MTU > (BLE_EVT_MAX_PARAM_LEN - 4)` i.e. `ATT_MTU > 251` for `BLE_EVT_MAX_PARAM_LEN`
    /// default value.
    GattIndicationExt(0x0C1E) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: BoundedBytes<245> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 245,
            },
        };
    }
    /// When it is enabled with [set_event_mast](crate::vendor::command::gatt::GattSetEventMask),
    /// this event is generated instead of [GATT Notification](VendorEvent::GattNotification) event.
    ///
    /// This event should be used instead of `ACI_GATT_INDICATION_EVENT` when
    /// `ATT_MTU > (BLE_EVT_MAX_PARAM_LEN - 4)` i.e. `ATT_MTU > 251` for `BLE_EVT_MAX_PARAM_LEN`
    /// default value.
    GattNotificationExt(0x0C1F) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: BoundedBytes<245> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 245,
            },
        };
    }
}

/// Potential firmware kinds for [`CoprocessorReady`](VendorEvent::CoprocessorReady)
/// event.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FirmwareKind {
    /// Wireless firmware (BLE, Thread, etc.)
    Wireless,

    /// RCC firmware.
    Rcc,
}

impl TryFrom<u8> for FirmwareKind {
    type Error = VendorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FirmwareKind::Wireless),
            1 => Ok(FirmwareKind::Rcc),
            _ => Err(VendorError::UnknownFirmwareKind(value)),
        }
    }
}

/// Reasons why an L2CAP command was rejected. see the Bluetooth specification, v4.1, Vol 3, Part A,
/// Section 4.1.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum L2CapRejectionReason {
    /// The controller sent an unknown command.
    CommandNotUnderstood,
    /// When multiple commands are included in an L2CAP packet and the packet exceeds the signaling
    /// MTU (MTUsig) of the receiver, a single Command Reject packet shall be sent in response.
    SignalingMtuExceeded,
    /// Invalid CID in request
    InvalidCid,
}

impl TryFrom<u16> for L2CapRejectionReason {
    type Error = VendorError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(L2CapRejectionReason::CommandNotUnderstood),
            1 => Ok(L2CapRejectionReason::SignalingMtuExceeded),
            2 => Ok(L2CapRejectionReason::InvalidCid),
            _ => Err(VendorError::BadL2CapRejectionReason(value)),
        }
    }
}

/// Potential results that can be used in the L2CAP connection update response.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum L2CapConnectionUpdateResult {
    /// The update request was rejected. The code indicates the reason for the rejection.
    CommandRejected(L2CapRejectionReason),

    /// The L2CAP connection update response is valid. The code indicates if the parameters were
    /// rejected.
    ParametersRejected,

    /// The L2CAP connection update response is valid. The code indicates if the parameters were
    /// updated.
    ParametersUpdated,
}

fn to_l2cap_connection_update_accepted_result(
    value: u16,
) -> Result<L2CapConnectionUpdateResult, VendorError> {
    match value {
        0x0000 => Ok(L2CapConnectionUpdateResult::ParametersUpdated),
        0x0001 => Ok(L2CapConnectionUpdateResult::ParametersRejected),
        _ => Err(VendorError::BadL2CapConnectionResponseResult(value)),
    }
}

/// Zero-length L2CAP event data, including its required wire count.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EmptyL2CapData;

impl EmptyL2CapData {
    fn decode_hci_event_payload(data: &[u8]) -> Result<(Self, &[u8]), Error> {
        let Some((&count, rest)) = data.split_first() else {
            return Err(Error::BadLength(0, 1));
        };
        if count != 0 {
            return Err(Error::Vendor(VendorError::BadL2CapDataLength(count, 0)));
        }
        Ok((Self, rest))
    }
}

/// Channel indices prefixed by the controller-provided channel count.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct L2CapChannelIndices<const MIN: usize, const MAX: usize> {
    indices: [u8; MAX],
    len: usize,
}

impl<const MIN: usize, const MAX: usize> L2CapChannelIndices<MIN, MAX> {
    /// Returns the channel indices present on the wire.
    pub fn as_slice(&self) -> &[u8] {
        &self.indices[..self.len]
    }
}

impl<const MIN: usize, const MAX: usize> L2CapChannelIndices<MIN, MAX> {
    fn decode_hci_event_payload(data: &[u8]) -> Result<(Self, &[u8]), Error> {
        let Some((&count, data)) = data.split_first() else {
            return Err(Error::BadLength(0, 1));
        };
        let len = usize::from(count);
        if !(MIN..=MAX).contains(&len) {
            return Err(Error::BadLength(len, if len < MIN { MIN } else { MAX }));
        }
        if data.len() < len {
            return Err(Error::BadLength(data.len(), len));
        }
        let (value, rest) = data.split_at(len);
        let mut indices = [0; MAX];
        indices[..len].copy_from_slice(value);
        Ok((Self { indices, len }, rest))
    }
}

/// Reasons the [GAP Pairing Complete](VendorEvent::GapPairingComplete) event was generated.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GapPairingStatus {
    /// Pairing with a remote device was successful.
    Success,
    /// The SMP timeout has elapsed and no further SMP commands will be processed until
    /// reconnection.
    Timeout(GapPairingReason),
    /// The pairing failed with the remote device.
    Failed(GapPairingReason),
    /// Encryption failed
    EncryptionFailed(GapPairingReason),
}

fn to_gap_pairing_status(
    status: u8,
    reason: Result<GapPairingReason, VendorError>,
) -> Result<GapPairingStatus, VendorError> {
    match status {
        0 => Ok(GapPairingStatus::Success),
        1 => Ok(GapPairingStatus::Timeout(reason?)),
        2 => Ok(GapPairingStatus::Failed(reason?)),
        3 => Ok(GapPairingStatus::EncryptionFailed(reason?)),
        _ => Err(VendorError::BadGapPairingStatus(status)),
    }
}

/// Reasons the [GAP Pairing Complete](VendorEvent::GapPairingComplete) event failed.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GapPairingReason {
    PasskeyEntryFailed = 0x01,
    OobNotAvailable = 0x02,
    AuthRequirements = 0x03,
    ConfirmValueFailed = 0x04,
    PairingNotSupported = 0x05,
    EncryptionKeySize = 0x06,
    CommandNotSupported = 0x07,
    Unspecified = 0x08,
    RepeatedAttemptes = 0x09,
    InvalidParams = 0x0A,
    DHKeyCheckFailed = 0x0B,
    NumericComparisonFailed = 0x0C,
    KeyRejected = 0x0F,
}

impl TryFrom<u8> for GapPairingReason {
    type Error = VendorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(GapPairingReason::PasskeyEntryFailed),
            0x02 => Ok(GapPairingReason::OobNotAvailable),
            0x03 => Ok(GapPairingReason::AuthRequirements),
            0x04 => Ok(GapPairingReason::ConfirmValueFailed),
            0x05 => Ok(GapPairingReason::PairingNotSupported),
            0x06 => Ok(GapPairingReason::EncryptionKeySize),
            0x07 => Ok(GapPairingReason::CommandNotSupported),
            0x08 => Ok(GapPairingReason::Unspecified),
            0x09 => Ok(GapPairingReason::RepeatedAttemptes),
            0x0A => Ok(GapPairingReason::InvalidParams),
            0x0B => Ok(GapPairingReason::DHKeyCheckFailed),
            0x0C => Ok(GapPairingReason::NumericComparisonFailed),
            0x0F => Ok(GapPairingReason::KeyRejected),
            _ => Err(VendorError::BadGapPairingErrorReason(value)),
        }
    }
}

/// Maximum length of the name returned in the [`NameDiscovery`](GapProcedure::NameDiscovery)
/// procedure.
pub const MAX_NAME_LEN: usize = 248;

/// Newtype for the name buffer returned after successful
/// [`NameDiscovery`](GapProcedure::NameDiscovery).
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NameBuffer(pub [u8; MAX_NAME_LEN]);

impl Debug for NameBuffer {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        first_16(&self.0).fmt(f)
    }
}

impl PartialEq<NameBuffer> for NameBuffer {
    fn eq(&self, other: &Self) -> bool {
        if self.0.len() != other.0.len() {
            return false;
        }

        for (a, b) in self.0.iter().zip(other.0.iter()) {
            if a != b {
                return false;
            }
        }

        true
    }
}

/// Procedures whose completion may be reported by
/// [`GapProcedureComplete`](VendorEvent::GapProcedureComplete).
#[allow(clippy::large_enum_variant)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GapProcedure {
    /// See Vol 3, Part C, section 9.2.5.
    LimitedDiscovery,
    /// See Vol 3, Part C, section 9.2.6.
    GeneralDiscovery,
    /// See Vol 3, Part C, section 9.2.7. Contains the number of valid bytes and buffer with enough
    /// space for the maximum length of the name that can be retuned.
    NameDiscovery(usize, NameBuffer),
    /// See Vol 3, Part C, section 9.3.5.
    AutoConnectionEstablishment,
    /// See Vol 3, Part C, section 9.3.6. Contains the reconnection address.
    GeneralConnectionEstablishment,
    /// See Vol 3, Part C, section 9.3.7.
    SelectiveConnectionEstablishment,
    /// See Vol 3, Part C, section 9.3.8.
    DirectConnectionEstablishment,
    Observation,
}

/// GAP procedure discriminator carried by the procedure-complete event.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GapProcedureKind {
    LimitedDiscovery,
    GeneralDiscovery,
    NameDiscovery,
    AutoConnectionEstablishment,
    GeneralConnectionEstablishment,
    SelectiveConnectionEstablishment,
    DirectConnectionEstablishment,
    Observation,
}

impl HciEventField<1> for GapProcedureKind {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        match bytes[0] {
            0x01 => Ok(Self::LimitedDiscovery),
            0x02 => Ok(Self::GeneralDiscovery),
            0x04 => Ok(Self::NameDiscovery),
            0x08 => Ok(Self::AutoConnectionEstablishment),
            0x10 => Ok(Self::GeneralConnectionEstablishment),
            0x20 => Ok(Self::SelectiveConnectionEstablishment),
            0x40 => Ok(Self::DirectConnectionEstablishment),
            0x80 => Ok(Self::Observation),
            value => Err(Error::Vendor(VendorError::BadGapProcedure(value))),
        }
    }
}

impl GapProcedureComplete {
    /// Converts the declarative discriminator and data field to the legacy combined value.
    pub fn legacy_procedure(&self) -> Result<GapProcedure, Error> {
        Ok(match self.procedure {
            GapProcedureKind::LimitedDiscovery => GapProcedure::LimitedDiscovery,
            GapProcedureKind::GeneralDiscovery => GapProcedure::GeneralDiscovery,
            GapProcedureKind::NameDiscovery => {
                let data = self.data.as_slice();
                if data.len() > MAX_NAME_LEN {
                    return Err(Error::BadLength(data.len(), MAX_NAME_LEN));
                }
                let mut name = NameBuffer([0; MAX_NAME_LEN]);
                name.0[..data.len()].copy_from_slice(data);
                GapProcedure::NameDiscovery(data.len(), name)
            }
            GapProcedureKind::AutoConnectionEstablishment => {
                GapProcedure::AutoConnectionEstablishment
            }
            GapProcedureKind::GeneralConnectionEstablishment => {
                GapProcedure::GeneralConnectionEstablishment
            }
            GapProcedureKind::SelectiveConnectionEstablishment => {
                GapProcedure::SelectiveConnectionEstablishment
            }
            GapProcedureKind::DirectConnectionEstablishment => {
                GapProcedure::DirectConnectionEstablishment
            }
            GapProcedureKind::Observation => GapProcedure::Observation,
        })
    }
}

/// Possible results of a [GAP procedure](VendorEvent::GapProcedureComplete).
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GapProcedureStatus {
    /// BLE Status Success.
    Success,
    /// BLE Status Failed.
    Failed,
    /// Procedure failed due to authentication requirements.
    AuthFailure,
}

impl TryFrom<u8> for GapProcedureStatus {
    type Error = VendorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(GapProcedureStatus::Success),
            0x41 => Ok(GapProcedureStatus::Failed),
            0x05 => Ok(GapProcedureStatus::AuthFailure),
            _ => Err(VendorError::BadGapProcedureStatus(value)),
        }
    }
}

impl GattAttributeModified {
    /// Returns the attribute offset from which data has been written.
    pub fn offset(&self) -> usize {
        usize::from(self.offset & 0x7FFF)
    }

    /// Returns the valid attribute data returned by the ATT attribute modified event as a slice of
    /// bytes.
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
}

/// Newtype for an attribute handle. These handles are IDs, not general integers, and should not be
/// manipulated as such.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttributeHandle(pub u16);

impl AttFindInformationResponse {
    /// The Find Information Response shall have complete handle-UUID pairs. Such pairs shall not be
    /// split across response packets; this also implies that a handleUUID pair shall fit into a
    /// single response packet. The handle-UUID pairs shall be returned in ascending order of
    /// attribute handles.
    pub fn handle_uuid_pair_iter(&self) -> HandleUuidPairIterator<'_> {
        match self.handle_uuid_pairs {
            HandleUuidPairs::Format16(count, ref data) => {
                HandleUuidPairIterator::Format16(HandleUuid16PairIterator {
                    data,
                    count,
                    next_index: 0,
                })
            }
            HandleUuidPairs::Format128(count, ref data) => {
                HandleUuidPairIterator::Format128(HandleUuid128PairIterator {
                    data,
                    count,
                    next_index: 0,
                })
            }
        }
    }
}

// Assuming a maximum HCI packet size of 255, these are the maximum number of handle-UUID pairs for
// each format that can be in one packet.  Formats cannot be mixed in a single packet.
//
// Packets have 6 other bytes of data preceding the handle-UUID pairs.
//
// max = floor((255 - 6) / pair_length)
const MAX_FORMAT16_PAIR_COUNT: usize = 62;
const MAX_FORMAT128_PAIR_COUNT: usize = 13;

/// One format of the handle-UUID pairs in the [`AttFindInformationResponse`] event. The UUIDs are
/// 16 bits.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleUuid16Pair {
    /// Attribute handle
    pub handle: AttributeHandle,
    /// Attribute UUID
    pub uuid: Uuid16,
}

/// One format of the handle-UUID pairs in the [`AttFindInformationResponse`] event. The UUIDs are
/// 128 bits.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleUuid128Pair {
    /// Attribute handle
    pub handle: AttributeHandle,
    /// Attribute UUID
    pub uuid: Uuid128,
}

/// Newtype for the 16-bit UUID buffer.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Uuid16(pub u16);

/// Newtype for the 128-bit UUID buffer.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Uuid128(pub [u8; 16]);

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(missing_docs)]
pub enum HandleUuidPairs {
    Format16(usize, [HandleUuid16Pair; MAX_FORMAT16_PAIR_COUNT]),
    Format128(usize, [HandleUuid128Pair; MAX_FORMAT128_PAIR_COUNT]),
}

impl HandleUuidPairs {
    fn decode_hci_event_payload(data: &[u8]) -> Result<(Self, &[u8]), Error> {
        if data.len() < 2 {
            return Err(Error::BadLength(data.len(), 2));
        }
        let format = data[0];
        let len = usize::from(data[1]);
        if len > 249 {
            return Err(Error::BadLength(len, 249));
        }
        if data.len() < 2 + len {
            return Err(Error::BadLength(data.len(), 2 + len));
        }
        let (pairs, rest) = data[2..].split_at(len);
        let value = match format {
            1 => to_handle_uuid16_pairs(pairs),
            2 => to_handle_uuid128_pairs(pairs),
            value => Err(VendorError::BadAttFindInformationResponseFormat(value)),
        }
        .map_err(Error::Vendor)?;
        Ok((value, rest))
    }
}

impl Debug for HandleUuidPairs {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{{")?;
        match *self {
            HandleUuidPairs::Format16(count, pairs) => {
                for handle_uuid_pair in &pairs[..count] {
                    write!(
                        f,
                        "{{{:?}, {:?}}}",
                        handle_uuid_pair.handle, handle_uuid_pair.uuid
                    )?
                }
            }
            HandleUuidPairs::Format128(count, pairs) => {
                for handle_uuid_pair in &pairs[..count] {
                    write!(
                        f,
                        "{{{:?}, {:?}}}",
                        handle_uuid_pair.handle, handle_uuid_pair.uuid
                    )?
                }
            }
        }
        write!(f, "}}")
    }
}

/// Possible iterators over handle-UUID pairs that can be returnedby the
/// [ATT find information response](AttFindInformationResponse). All pairs from the same event have the same format.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HandleUuidPairIterator<'a> {
    /// The event contains 16-bit UUIDs.
    Format16(HandleUuid16PairIterator<'a>),
    /// The event contains 128-bit UUIDs.
    Format128(HandleUuid128PairIterator<'a>),
}

/// Iterator over handle-UUID pairs for 16-bit UUIDs.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleUuid16PairIterator<'a> {
    data: &'a [HandleUuid16Pair; MAX_FORMAT16_PAIR_COUNT],
    count: usize,
    next_index: usize,
}

impl<'a> Iterator for HandleUuid16PairIterator<'a> {
    type Item = HandleUuid16Pair;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.count {
            return None;
        }

        let index = self.next_index;
        self.next_index += 1;
        Some(self.data[index])
    }
}

/// Iterator over handle-UUID pairs for 128-bit UUIDs.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleUuid128PairIterator<'a> {
    data: &'a [HandleUuid128Pair; MAX_FORMAT128_PAIR_COUNT],
    count: usize,
    next_index: usize,
}

impl<'a> Iterator for HandleUuid128PairIterator<'a> {
    type Item = HandleUuid128Pair;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.count {
            return None;
        }

        let index = self.next_index;
        self.next_index += 1;
        Some(self.data[index])
    }
}

// [0x4, 0xc, 0x1, 0x8, 0x1, 0x8, 0x12, 0x0, 0x3, 0x5, 0x13, 0x0, 0x2, 0x29]

fn to_handle_uuid16_pairs(buffer: &[u8]) -> Result<HandleUuidPairs, VendorError> {
    const PAIR_LEN: usize = 4;
    if !buffer.len().is_multiple_of(PAIR_LEN) {
        return Err(VendorError::AttFindInformationResponsePartialPair16);
    }

    let count = buffer.len() / PAIR_LEN;
    let mut pairs = [HandleUuid16Pair {
        handle: AttributeHandle(0),
        uuid: Uuid16(0),
    }; MAX_FORMAT16_PAIR_COUNT];
    for (i, pair) in pairs.iter_mut().enumerate().take(count) {
        let index = i * PAIR_LEN;
        pair.handle = AttributeHandle(LittleEndian::read_u16(&buffer[index..]));
        pair.uuid = Uuid16(LittleEndian::read_u16(&buffer[2 + index..]));
    }

    Ok(HandleUuidPairs::Format16(count, pairs))
}

fn to_handle_uuid128_pairs(buffer: &[u8]) -> Result<HandleUuidPairs, VendorError> {
    const PAIR_LEN: usize = 18;
    if !buffer.len().is_multiple_of(PAIR_LEN) {
        return Err(VendorError::AttFindInformationResponsePartialPair128);
    }

    let count = buffer.len() / PAIR_LEN;
    let mut pairs = [HandleUuid128Pair {
        handle: AttributeHandle(0),
        uuid: Uuid128([0; 16]),
    }; MAX_FORMAT128_PAIR_COUNT];
    for (i, pair) in pairs.iter_mut().enumerate().take(count) {
        let index = i * PAIR_LEN;
        let next_index = (i + 1) * PAIR_LEN;
        pair.handle = AttributeHandle(LittleEndian::read_u16(&buffer[index..]));
        pair.uuid.0.copy_from_slice(&buffer[2 + index..next_index]);
    }

    Ok(HandleUuidPairs::Format128(count, pairs))
}

impl AttFindByTypeValueResponse {
    /// Returns an iterator over the Handles Information List as defined in Bluetooth Core v4.1
    /// spec.
    pub fn handle_pairs_iter(&self) -> HandleInfoPairIterator<'_> {
        HandleInfoPairIterator {
            event: self,
            next_index: 0,
        }
    }
}

/// Simple container for the handle information returned in [`AttFindByTypeValueResponse`].
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleInfoPair {
    /// Attribute handle
    pub attribute: AttributeHandle,
    /// Group End handle
    pub group_end: GroupEndHandle,
}

impl crate::vendor::command::HciDecodeField<4> for HandleInfoPair {
    fn from_hci_field(bytes: &[u8; 4]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(Self {
            attribute: AttributeHandle(u16::from_le_bytes([bytes[0], bytes[1]])),
            group_end: GroupEndHandle(u16::from_le_bytes([bytes[2], bytes[3]])),
        })
    }
}

/// Newtype for Group End handles
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GroupEndHandle(pub u16);

/// Iterator into valid [`HandleInfoPair`] structs returned in the
/// [ATT Find By Type Value Response](AttFindByTypeValueResponse) event.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleInfoPairIterator<'a> {
    event: &'a AttFindByTypeValueResponse,
    next_index: usize,
}

impl<'a> Iterator for HandleInfoPairIterator<'a> {
    type Item = HandleInfoPair;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.event.handles.len() {
            return None;
        }

        let index = self.next_index;
        self.next_index += 1;
        Some(self.event.handles.as_slice()[index])
    }
}

/// Owned ATT handle-value records decoded from a Read By Type response.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleValuePairs {
    data: [u8; 249],
    len: usize,
    value_len: usize,
}

impl HandleValuePairs {
    fn decode_hci_event_payload(data: &[u8]) -> Result<(Self, &[u8]), Error> {
        if data.len() < 2 {
            return Err(Error::BadLength(data.len(), 2));
        }
        let pair_len = usize::from(data[0]);
        let len = usize::from(data[1]);
        if len > 249 || data.len() < 2 + len {
            return Err(Error::BadLength(data.len(), 2 + len));
        }
        if pair_len < 2 || !len.is_multiple_of(pair_len) {
            return Err(Error::Vendor(VendorError::AttReadByTypeResponsePartial));
        }
        let (records, rest) = data[2..].split_at(len);
        let mut value = [0; 249];
        value[..len].copy_from_slice(records);
        Ok((
            Self {
                data: value,
                len,
                value_len: pair_len - 2,
            },
            rest,
        ))
    }
}

impl AttReadByTypeResponse {
    /// Return an iterator over all valid handle-value pairs returned with the response.
    pub fn handle_value_pair_iter(&self) -> HandleValuePairIterator<'_> {
        HandleValuePairIterator {
            event: self,
            index: 0,
        }
    }
}

/// Iterator over the valid handle-value pairs returned with the
/// [ATT Read by Type response](AttReadByTypeResponse).
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleValuePairIterator<'a> {
    event: &'a AttReadByTypeResponse,
    index: usize,
}

impl<'a> Iterator for HandleValuePairIterator<'a> {
    type Item = HandleValuePair<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.event.pairs.len {
            return None;
        }

        let handle_index = self.index;
        let value_index = self.index + 2;
        self.index += 2 + self.event.pairs.value_len;
        let next_index = self.index;
        Some(HandleValuePair {
            handle: AttributeHandle(LittleEndian::read_u16(
                &self.event.pairs.data[handle_index..],
            )),
            value: &self.event.pairs.data[value_index..next_index],
        })
    }
}

/// A single handle-value pair returned by the [ATT Read by Type response](AttReadByTypeResponse).
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleValuePair<'a> {
    /// Attribute handle
    pub handle: AttributeHandle,
    /// Attribute value. The caller must interpret the value correctly, depending on the expected
    /// type of the attribute.
    pub value: &'a [u8],
}

impl<'a> HandleValuePair<'a> {
    pub fn uuid(&self) -> u16 {
        LittleEndian::read_u16(&self.value[3..])
    }
}

impl AttReadResponse {
    /// Returns the valid part of the value data.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

impl AttReadBlobResponse {
    /// Returns the valid part of the value data.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

impl AttReadMultipleResponse {
    /// Returns the valid part of the value data.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

/// Owned ATT attribute groups decoded from a Read By Group Type response.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttributeGroups {
    data: [u8; 249],
    len: usize,
    group_len: usize,
}

impl AttributeGroups {
    fn decode_hci_event_payload(data: &[u8]) -> Result<(Self, &[u8]), Error> {
        if data.len() < 2 {
            return Err(Error::BadLength(data.len(), 2));
        }
        let group_len = usize::from(data[0]);
        let len = usize::from(data[1]);
        if len > 249 || data.len() < 2 + len {
            return Err(Error::BadLength(data.len(), 2 + len));
        }
        if group_len < 4 || !len.is_multiple_of(group_len) {
            return Err(Error::Vendor(
                VendorError::AttReadByGroupTypeResponsePartial,
            ));
        }
        let (records, rest) = data[2..].split_at(len);
        let mut value = [0; 249];
        value[..len].copy_from_slice(records);
        Ok((
            Self {
                data: value,
                len,
                group_len,
            },
            rest,
        ))
    }
}

impl AttReadByGroupTypeResponse {
    /// Create and return an iterator for the attribute data returned with the response.
    pub fn attribute_data_iter(&self) -> AttributeDataIterator<'_> {
        AttributeDataIterator {
            event: self,
            next_index: 0,
        }
    }
}

/// Iterator over the attribute data returned in the [`AttReadByGroupTypeResponse`].
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttributeDataIterator<'a> {
    event: &'a AttReadByGroupTypeResponse,
    next_index: usize,
}

impl<'a> Iterator for AttributeDataIterator<'a> {
    type Item = AttributeData<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.event.groups.len {
            return None;
        }

        let attr_handle_index = self.next_index;
        let group_end_index = 2 + attr_handle_index;
        let value_index = 2 + group_end_index;
        self.next_index += self.event.groups.group_len;
        Some(AttributeData {
            attribute_handle: AttributeHandle(LittleEndian::read_u16(
                &self.event.groups.data[attr_handle_index..],
            )),
            attribute_end_handle: AttributeHandle(LittleEndian::read_u16(
                &self.event.groups.data[group_end_index..],
            )),
            value: &self.event.groups.data[value_index..self.next_index],
        })
    }
}

/// Attribute data returned in the [`AttReadByGroupTypeResponse`] event.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttributeData<'a> {
    /// Attribute handle
    pub attribute_handle: AttributeHandle,
    /// Group end handle
    pub attribute_end_handle: AttributeHandle,
    /// Attribute value
    pub value: &'a [u8],
}

impl<'a> AttributeData<'a> {
    pub fn uuid(&self) -> u16 {
        LittleEndian::read_u16(&self.value[0..])
    }
}

impl AttPrepareWriteResponse {
    /// Returns the partial value of the attribute to be written.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

macro_rules! impl_attribute_value_accessor {
    ($($ty:ty),* $(,)?) => {
        $(
            impl $ty {
                /// Returns the current value of the attribute.
                pub fn value(&self) -> &[u8] {
                    self.value.as_slice()
                }
            }
        )*
    };
}

impl_attribute_value_accessor!(
    GattIndication,
    GattNotification,
    GattDiscoverOrReadCharacteristicByUuidResponse,
    AttWritePermitRequest,
    GattReadExt,
    GattIndicationExt,
    GattNotificationExt,
);

/// Allowed status codes for the [GATT Procedure Complete](VendorEvent::GattProcedureComplete)
/// event.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GattProcedureStatus {
    /// BLE Status Success
    Success,
    /// BLE Status Failed
    Failed,
}

impl TryFrom<u8> for GattProcedureStatus {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(GattProcedureStatus::Success),
            0x41 => Ok(GattProcedureStatus::Failed),
            _ => Err(Error::Vendor(VendorError::BadGattProcedureStatus(value))),
        }
    }
}

/// Potential error codes for the [ATT Error Response](VendorEvent::AttErrorResponse). See Table
/// 3.3 in the Bluetooth Core Specification, v4.1, Vol 3, Part F, Section 3.4.1.1 and The Bluetooth
/// Core Specification Supplement, Table 1.1.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AttError {
    /// The attribute handle given was not valid on this server.
    InvalidHandle = 0x01,
    /// The attribute cannot be read.
    ReadNotPermitted = 0x02,
    /// The attribute cannot be written.
    WriteNotPermitted = 0x03,
    /// The attribute PDU was invalid.
    InvalidPdu = 0x04,
    /// The attribute requires authentication before it can be read or written.
    InsufficientAuthentication = 0x05,
    /// Attribute server does not support the request received from the client.
    RequestNotSupported = 0x06,
    /// Offset specified was past the end of the attribute.
    InvalidOffset = 0x07,
    /// The attribute requires authorization before it can be read or written.
    InsufficientAuthorization = 0x08,
    /// Too many prepare writes have been queued.
    PrepareQueueFull = 0x09,
    /// No attribute found within the given attribute handle range.
    AttributeNotFound = 0x0A,
    /// The attribute cannot be read or written using the Read Blob Request.
    AttributeNotLong = 0x0B,
    /// The Encryption Key Size used for encrypting this link is insufficient.
    InsufficientEncryptionKeySize = 0x0C,
    /// The attribute value length is invalid for the operation.
    InvalidAttributeValueLength = 0x0D,
    /// The attribute request that was requested has encountered an error that was unlikely, and
    /// therefore could not be completed as requested.
    UnlikelyError = 0x0E,
    /// The attribute requires encryption before it can be read or written.
    InsufficientEncryption = 0x0F,
    /// The attribute type is not a supported grouping attribute as defined by a higher layer
    /// specification.
    UnsupportedGroupType = 0x10,
    /// Insufficient Resources to complete the request.
    InsufficientResources = 0x11,
    /// Database out of sync
    DatabaseOutOfSync = 0x12,
    /// Value not allowed
    ValueNotAllowed = 0x13,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x80 = 0x80,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x81 = 0x81,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x82 = 0x82,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x83 = 0x83,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x84 = 0x84,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x85 = 0x85,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x86 = 0x86,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x87 = 0x87,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x88 = 0x88,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x89 = 0x89,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8A = 0x8A,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8B = 0x8B,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8C = 0x8C,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8D = 0x8D,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8E = 0x8E,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x8F = 0x8F,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x90 = 0x90,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x91 = 0x91,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x92 = 0x92,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x93 = 0x93,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x94 = 0x94,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x95 = 0x95,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x96 = 0x96,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x97 = 0x97,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x98 = 0x98,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x99 = 0x99,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9A = 0x9A,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9B = 0x9B,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9C = 0x9C,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9D = 0x9D,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9E = 0x9E,
    /// Application error code defined by a higher layer specification.
    ApplicationError0x9F = 0x9F,
    /// The requested write operation cannot be fulfilled for reasons other than permissions.
    WriteRequestRejected = 0xFC,
    /// A Client Characteristic Configuration descriptor is not configured according to the
    /// requirements of the profile or service.
    ClientCharacteristicConfigurationDescriptorImproperlyConfigured = 0xFD,
    /// A profile or service request cannot be serviced because an operation that has been
    /// previously triggered is still in progress.
    ProcedureAlreadyInProgress = 0xFE,
    /// An attribute value is out of range as defined by a profile or service specification.
    OutOfRange = 0xFF,
}

impl TryFrom<u8> for AttError {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(AttError::InvalidHandle),
            0x02 => Ok(AttError::ReadNotPermitted),
            0x03 => Ok(AttError::WriteNotPermitted),
            0x04 => Ok(AttError::InvalidPdu),
            0x05 => Ok(AttError::InsufficientAuthentication),
            0x06 => Ok(AttError::RequestNotSupported),
            0x07 => Ok(AttError::InvalidOffset),
            0x08 => Ok(AttError::InsufficientAuthorization),
            0x09 => Ok(AttError::PrepareQueueFull),
            0x0A => Ok(AttError::AttributeNotFound),
            0x0B => Ok(AttError::AttributeNotLong),
            0x0C => Ok(AttError::InsufficientEncryptionKeySize),
            0x0D => Ok(AttError::InvalidAttributeValueLength),
            0x0E => Ok(AttError::UnlikelyError),
            0x0F => Ok(AttError::InsufficientEncryption),
            0x10 => Ok(AttError::UnsupportedGroupType),
            0x11 => Ok(AttError::InsufficientResources),
            0x80 => Ok(AttError::ApplicationError0x80),
            0x81 => Ok(AttError::ApplicationError0x81),
            0x82 => Ok(AttError::ApplicationError0x82),
            0x83 => Ok(AttError::ApplicationError0x83),
            0x84 => Ok(AttError::ApplicationError0x84),
            0x85 => Ok(AttError::ApplicationError0x85),
            0x86 => Ok(AttError::ApplicationError0x86),
            0x87 => Ok(AttError::ApplicationError0x87),
            0x88 => Ok(AttError::ApplicationError0x88),
            0x89 => Ok(AttError::ApplicationError0x89),
            0x8A => Ok(AttError::ApplicationError0x8A),
            0x8B => Ok(AttError::ApplicationError0x8B),
            0x8C => Ok(AttError::ApplicationError0x8C),
            0x8D => Ok(AttError::ApplicationError0x8D),
            0x8E => Ok(AttError::ApplicationError0x8E),
            0x8F => Ok(AttError::ApplicationError0x8F),
            0x90 => Ok(AttError::ApplicationError0x90),
            0x91 => Ok(AttError::ApplicationError0x91),
            0x92 => Ok(AttError::ApplicationError0x92),
            0x93 => Ok(AttError::ApplicationError0x93),
            0x94 => Ok(AttError::ApplicationError0x94),
            0x95 => Ok(AttError::ApplicationError0x95),
            0x96 => Ok(AttError::ApplicationError0x96),
            0x97 => Ok(AttError::ApplicationError0x97),
            0x98 => Ok(AttError::ApplicationError0x98),
            0x99 => Ok(AttError::ApplicationError0x99),
            0x9A => Ok(AttError::ApplicationError0x9A),
            0x9B => Ok(AttError::ApplicationError0x9B),
            0x9C => Ok(AttError::ApplicationError0x9C),
            0x9D => Ok(AttError::ApplicationError0x9D),
            0x9E => Ok(AttError::ApplicationError0x9E),
            0x9F => Ok(AttError::ApplicationError0x9F),
            0xFC => Ok(AttError::WriteRequestRejected),
            0xFD => Ok(AttError::ClientCharacteristicConfigurationDescriptorImproperlyConfigured),
            0xFE => Ok(AttError::ProcedureAlreadyInProgress),
            0xFF => Ok(AttError::OutOfRange),
            _ => Err(value),
        }
    }
}

/// Possible ATT requests.  See Table 3.37 in the Bluetooth Core Spec v4.1, Vol 3, Part F, Section
/// 3.4.8.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AttRequest {
    /// Section 3.4.1.1
    ErrorResponse = 0x01,
    /// Section 3.4.2.1
    ExchangeMtuRequest = 0x02,
    /// Section 3.4.2.2
    ExchangeMtuResponse = 0x03,
    /// Section 3.4.3.1
    FindInformationRequest = 0x04,
    /// Section 3.4.3.2
    FindInformationResponse = 0x05,
    /// Section 3.4.3.3
    FindByTypeValueRequest = 0x06,
    /// Section 3.4.3.4
    FindByTypeValueResponse = 0x07,
    /// Section 3.4.4.1
    ReadByTypeRequest = 0x08,
    /// Section 3.4.4.2
    ReadByTypeResponse = 0x09,
    /// Section 3.4.4.3
    ReadRequest = 0x0A,
    /// Section 3.4.4.4
    ReadResponse = 0x0B,
    /// Section 3.4.4.5
    ReadBlobRequest = 0x0C,
    /// Section 3.4.4.6
    ReadBlobResponse = 0x0D,
    /// Section 3.4.4.7
    ReadMultipleRequest = 0x0E,
    /// Section 3.4.4.8
    ReadMultipleResponse = 0x0F,
    /// Section 3.4.4.9
    ReadByGroupTypeRequest = 0x10,
    /// Section 3.4.4.10
    ReadByGroupTypeResponse = 0x11,
    /// Section 3.4.5.1
    WriteRequest = 0x12,
    /// Section 3.4.5.2
    WriteResponse = 0x13,
    /// Section 3.4.5.3
    WriteCommand = 0x52,
    /// Section 3.4.5.4
    SignedWriteCommand = 0xD2,
    /// Section 3.4.6.1
    PrepareWriteRequest = 0x16,
    /// Section 3.4.6.2
    PrepareWriteResponse = 0x17,
    /// Section 3.4.6.3
    ExecuteWriteRequest = 0x18,
    /// Section 3.4.6.4
    ExecuteWriteResponse = 0x19,
    /// Section 3.4.7.1
    HandleValueNotification = 0x1B,
    /// Section 3.4.7.2
    HandleValueIndication = 0x1D,
    /// Section 3.4.7.3
    HandleValueConfirmation = 0x1E,
}

impl TryFrom<u8> for AttRequest {
    type Error = VendorError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(AttRequest::ErrorResponse),
            0x02 => Ok(AttRequest::ExchangeMtuRequest),
            0x03 => Ok(AttRequest::ExchangeMtuResponse),
            0x04 => Ok(AttRequest::FindInformationRequest),
            0x05 => Ok(AttRequest::FindInformationResponse),
            0x06 => Ok(AttRequest::FindByTypeValueRequest),
            0x07 => Ok(AttRequest::FindByTypeValueResponse),
            0x08 => Ok(AttRequest::ReadByTypeRequest),
            0x09 => Ok(AttRequest::ReadByTypeResponse),
            0x0A => Ok(AttRequest::ReadRequest),
            0x0B => Ok(AttRequest::ReadResponse),
            0x0C => Ok(AttRequest::ReadBlobRequest),
            0x0D => Ok(AttRequest::ReadBlobResponse),
            0x0E => Ok(AttRequest::ReadMultipleRequest),
            0x0F => Ok(AttRequest::ReadMultipleResponse),
            0x10 => Ok(AttRequest::ReadByGroupTypeRequest),
            0x11 => Ok(AttRequest::ReadByGroupTypeResponse),
            0x12 => Ok(AttRequest::WriteRequest),
            0x13 => Ok(AttRequest::WriteResponse),
            0x52 => Ok(AttRequest::WriteCommand),
            0xD2 => Ok(AttRequest::SignedWriteCommand),
            0x16 => Ok(AttRequest::PrepareWriteRequest),
            0x17 => Ok(AttRequest::PrepareWriteResponse),
            0x18 => Ok(AttRequest::ExecuteWriteRequest),
            0x19 => Ok(AttRequest::ExecuteWriteResponse),
            0x1B => Ok(AttRequest::HandleValueNotification),
            0x1D => Ok(AttRequest::HandleValueIndication),
            0x1E => Ok(AttRequest::HandleValueConfirmation),
            _ => Err(VendorError::BadAttRequestOpcode(value)),
        }
    }
}

impl AttReadMultiplePermitRequest {
    /// Returns the valid attribute handles returned by the ATT Read Multiple Permit Request event.
    pub fn handles(&self) -> &[AttributeHandle] {
        self.handles.as_slice()
    }
}

impl AttPrepareWritePermitRequest {
    /// Returns the data to be written.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Type of Keypress input notified/signaled by peer device
/// (having Keyboard only I/O capabilities.
pub enum KeypressNotificationType {
    EntryStarted = 0x00,
    DigitEntered = 0x01,
    DigitErased = 0x02,
    PasskeyCleared = 0x03,
    EntryCompleted = 0x04,
    Reserved,
}

impl From<u8> for KeypressNotificationType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => KeypressNotificationType::EntryStarted,
            0x01 => KeypressNotificationType::DigitEntered,
            0x02 => KeypressNotificationType::DigitErased,
            0x03 => KeypressNotificationType::PasskeyCleared,
            0x04 => KeypressNotificationType::EntryCompleted,
            _ => KeypressNotificationType::Reserved,
        }
    }
}

impl HciEventField<1> for KeypressNotificationType {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        Ok(bytes[0].into())
    }
}

/// Preferred spelling alias kept for API ergonomics.
pub type GattEattBearer = GattEattBrearer;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Enhanced ATT bearer state.
pub enum EabState {
    AttBearerCreated = 0x00,
    AttBearerTerminated = 0x01,
}

impl TryFrom<u8> for EabState {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(EabState::AttBearerCreated),
            0x01 => Ok(EabState::AttBearerTerminated),
            err => Err(Error::Vendor(VendorError::BadEabState(err))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RadioEvent {
    Idle = 0x00,
    Advertising = 0x01,
    PeripheralConnection = 0x02,
    Scanning = 0x03,
    CentralConnection = 0x05,
    TxTestMode = 0x06,
    RxTestMode = 0x07,
}

impl TryFrom<u8> for RadioEvent {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(RadioEvent::Idle),
            0x01 => Ok(RadioEvent::Advertising),
            0x02 => Ok(RadioEvent::PeripheralConnection),
            0x03 => Ok(RadioEvent::Scanning),
            0x05 => Ok(RadioEvent::CentralConnection),
            0x06 => Ok(RadioEvent::TxTestMode),
            0x07 => Ok(RadioEvent::RxTestMode),
            x => Err(Error::Vendor(VendorError::BadRadioEvent(x))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Defines error types returned by [HAL Firmware Error](VendorEvent::HalFirmwareError) event
pub enum FirmwareError {
    /// L2CAP recombination failure
    L2capRecombination = 0x01,
    /// GATT unexpected peer message
    GattUnexpectedPeerMsg = 0x02,
    /// NVM level warning
    NvmLevelWarning = 0x03,
    /// COC Rx data length too large
    CocRxDataTooLarge = 0x04,
    /// COC already assigned DCID
    COCAlreadyAssignedDCID = 0x05,
    /// SMP unexpected LTK request
    SmpUnexpectedLTKRequest = 0x06,
    /// GATT bearer not allocated
    GattBearerNotAllocated = 0x07,
}

impl TryFrom<u8> for FirmwareError {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(FirmwareError::L2capRecombination),
            0x02 => Ok(FirmwareError::GattUnexpectedPeerMsg),
            0x03 => Ok(FirmwareError::NvmLevelWarning),
            0x04 => Ok(FirmwareError::CocRxDataTooLarge),
            0x05 => Ok(FirmwareError::COCAlreadyAssignedDCID),
            0x06 => Ok(FirmwareError::SmpUnexpectedLTKRequest),
            0x07 => Ok(FirmwareError::GattBearerNotAllocated),
            x => Err(Error::Vendor(VendorError::BadFirmwareError(x))),
        }
    }
}

impl HalFirmwareError {
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
}

impl HciEventField<2> for usize {
    fn from_hci_event_field(bytes: &[u8; 2]) -> Result<Self, Error> {
        Ok(usize::from(u16::from_le_bytes(*bytes)))
    }
}

impl HciEventField<1> for FirmwareKind {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0].try_into().map_err(Error::Vendor)
    }
}

impl HciEventField<1> for RadioEvent {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0].try_into()
    }
}

impl HciEventField<7> for PeerAddrType {
    fn from_hci_event_field(bytes: &[u8; 7]) -> Result<Self, Error> {
        let address = BdAddr(bytes[1..].try_into().expect("six-byte address"));
        match bytes[0] {
            0x00 | 0x02 => Ok(Self::PublicDeviceAddress(address)),
            0x01 => Ok(Self::RandomDeviceAddress(address)),
            0x03 => Ok(Self::RandomIdentityAddress(address)),
            value => Err(Error::Vendor(VendorError::BadBdAddrType(value))),
        }
    }
}

impl HciEventField<1> for FirmwareError {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0].try_into()
    }
}

impl HciEventField<2> for GapPairingStatus {
    fn from_hci_event_field(bytes: &[u8; 2]) -> Result<Self, Error> {
        to_gap_pairing_status(bytes[0], bytes[1].try_into()).map_err(Error::Vendor)
    }
}

impl HciEventField<1> for GapProcedureStatus {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0].try_into().map_err(Error::Vendor)
    }
}

impl HciEventField<2> for L2CapConnectionUpdateResult {
    fn from_hci_event_field(bytes: &[u8; 2]) -> Result<Self, Error> {
        to_l2cap_connection_update_accepted_result(u16::from_le_bytes(*bytes))
            .map_err(Error::Vendor)
    }
}

impl HciEventField<8> for ConnectionInterval {
    fn from_hci_event_field(bytes: &[u8; 8]) -> Result<Self, Error> {
        Self::from_bytes(bytes)
            .map_err(VendorError::BadConnectionInterval)
            .map_err(Error::Vendor)
    }
}

impl HciEventField<1> for GattProcedureStatus {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0].try_into()
    }
}

impl HciEventField<1> for AttRequest {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0].try_into().map_err(Error::Vendor)
    }
}

impl HciEventField<1> for AttError {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0]
            .try_into()
            .map_err(VendorError::BadAttError)
            .map_err(Error::Vendor)
    }
}

impl HciEventField<1> for EabState {
    fn from_hci_event_field(bytes: &[u8; 1]) -> Result<Self, Error> {
        bytes[0].try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(since_fw_0_17_1)]
    #[test]
    fn parses_gap_pairing_request_event() {
        // 0x040B + conn_handle(0x0123) + bonded(1) + auth_req(0x2D)
        let bytes = [0x0B, 0x04, 0x23, 0x01, 0x01, 0x2D];
        let event = VendorEvent::new(&bytes).expect("parse pairing request");

        match event {
            VendorEvent::GapPairingRequest(e) => {
                assert_eq!(e.connection_handle.0, 0x0123);
                assert!(e.bonded);
                assert_eq!(e.auth_req, 0x2D);
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[cfg(since_fw_0_17_1)]
    #[test]
    fn rejects_short_gap_pairing_request_event() {
        let bytes = [0x0B, 0x04, 0x23, 0x01, 0x01];
        let err = VendorEvent::new(&bytes).expect_err("must reject short payload");

        assert!(matches!(err, Error::BadLength(_, _)));
    }

    #[cfg(not(since_fw_0_17_1))]
    #[test]
    fn rejects_gap_pairing_request_event_before_its_supported_firmware() {
        let bytes = [0x0B, 0x04, 0x23, 0x01, 0x01, 0x2D];
        let err =
            VendorEvent::new(&bytes).expect_err("event is not defined by supported Cube tags");

        assert!(matches!(
            err,
            Error::Vendor(VendorError::UnknownEvent(0x040B))
        ));
    }

    #[cfg(since_fw_0_17_0)]
    #[test]
    fn parses_gatt_notification_complete_event() {
        let bytes = [0x1B, 0x0C, 0x23, 0x01];
        let event = VendorEvent::new(&bytes).expect("parse notification complete");

        assert!(matches!(
            event,
            VendorEvent::GattNotificationComplete(GattNotificationComplete {
                attr_handle: AttributeHandle(0x0123)
            })
        ));
    }

    #[cfg(before_fw_0_17_0)]
    #[test]
    fn rejects_gatt_notification_complete_before_fw_0_17_0() {
        let bytes = [0x1B, 0x0C, 0x23, 0x01];
        let err = VendorEvent::new(&bytes).expect_err("event was introduced in Cube v1.17.0");

        assert!(matches!(
            err,
            Error::Vendor(VendorError::UnknownEvent(0x0C1B))
        ));
    }

    #[test]
    fn parses_gatt_eatt_bearer_event() {
        // 0x0C19 + channel_index(2) + eab_state(created) + status(success)
        let bytes = [0x19, 0x0C, 0x02, 0x00, 0x00];
        let event = VendorEvent::new(&bytes).expect("parse eatt bearer");

        match event {
            VendorEvent::GattEattBrearer(e) => {
                assert_eq!(e.channel_index, 2);
                assert!(matches!(e.eab_state, EabState::AttBearerCreated));
                assert_eq!(e.status, GattProcedureStatus::Success);
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn fixed_event_rejects_trailing_payload_bytes() {
        let bytes = [0x02, 0x0C, 0x23, 0x01, 0xFF];
        let error = VendorEvent::new(&bytes).expect_err("fixed payload must be exact");
        assert_eq!(error, Error::BadLength(3, 2));
    }

    #[test]
    fn counted_event_rejects_a_truncated_value() {
        let bytes = [0x07, 0x0C, 0x23, 0x01, 0x02, 0xAA];
        let error = VendorEvent::new(&bytes).expect_err("count requires two value bytes");
        assert_eq!(error, Error::BadLength(4, 5));
    }

    #[test]
    fn counted_items_reject_a_count_above_the_declared_maximum() {
        let bytes = [
            0x05, 0x0C, // event code
            0x23, 0x01, // connection handle
            63,   // handle-pair count; the declaration allows at most 62
        ];
        let error = VendorEvent::new(&bytes).expect_err("count exceeds the schema maximum");
        assert_eq!(error, Error::BadLength(63, 62));
    }

    #[test]
    fn semantic_payload_decodes_within_its_declared_bounds() {
        let bytes = [
            0x12, 0x08, // event code
            0x23, 0x01, // connection handle
            0x40, 0x00, // MTU
            0x20, 0x00, // MPS
            0x01, // channel count
            0x07, // channel index
        ];
        let event = VendorEvent::new(&bytes).expect("valid semantic payload");
        let VendorEvent::L2CapCocReconfig(event) = event else {
            panic!("unexpected event variant");
        };
        assert_eq!(event.channel_indices.as_slice(), &[0x07]);
    }

    #[test]
    fn semantic_payload_enforces_its_declared_count_bounds() {
        let mut bytes = [
            0x12, 0x08, // event code
            0x23, 0x01, // connection handle
            0x40, 0x00, // MTU
            0x20, 0x00, // MPS
            0x00, // channel count
        ];
        let error = VendorEvent::new(&bytes).expect_err("at least one channel is required");
        assert_eq!(error, Error::BadLength(0, 1));

        bytes[8] = 247;
        let error = VendorEvent::new(&bytes).expect_err("at most 246 channels are allowed");
        assert_eq!(error, Error::BadLength(247, 246));
    }

    #[test]
    fn semantic_empty_payload_rejects_a_nonzero_count() {
        let bytes = [
            0x01, 0x08, // event code
            0x23, 0x01, // connection handle
            0x01, // the timeout event requires an empty data list
        ];
        let error = VendorEvent::new(&bytes).expect_err("timeout data count must be zero");
        assert_eq!(error, Error::Vendor(VendorError::BadL2CapDataLength(1, 0)));
    }

    #[test]
    fn read_multiple_count_is_a_count_of_handles() {
        let bytes = [0x15, 0x0C, 0x23, 0x01, 0x02, 0x34, 0x12, 0x78, 0x56];
        let event = VendorEvent::new(&bytes).expect("two counted handles");
        let VendorEvent::AttReadMultiplePermitRequest(event) = event else {
            panic!("unexpected event variant");
        };
        assert_eq!(
            event.handles(),
            &[AttributeHandle(0x1234), AttributeHandle(0x5678)]
        );
    }

    #[test]
    fn trailing_channel_indices_must_match_the_declared_count() {
        let bytes = [
            0x11, 0x08, // event code
            0x23, 0x01, // connection handle
            0x40, 0x00, // MTU
            0x40, 0x00, // MPS
            0x01, 0x00, // credits
            0x00, 0x00, // result
            0x02, // channel count
            0x07, // only one channel index
        ];
        let error = VendorEvent::new(&bytes).expect_err("count and trailing list disagree");
        assert_eq!(error, Error::BadLength(1, 2));
    }

    #[test]
    fn bond_lost_has_no_payload() {
        assert!(matches!(
            VendorEvent::new(&[0x05, 0x04]).unwrap(),
            VendorEvent::GapBondLost
        ));
        assert_eq!(
            VendorEvent::new(&[0x05, 0x04, 0x23, 0x01]).unwrap_err(),
            Error::BadLength(2, 0)
        );
    }

    #[test]
    fn read_by_type_rejects_a_record_shorter_than_its_handle() {
        let bytes = [0x06, 0x0C, 0x23, 0x01, 0x01, 0x01, 0xAA];
        let error = VendorEvent::new(&bytes).expect_err("one-byte records have no handle");
        assert_eq!(
            error,
            Error::Vendor(VendorError::AttReadByTypeResponsePartial)
        );
    }

    #[test]
    fn read_by_group_rejects_a_record_shorter_than_two_handles() {
        let bytes = [0x0A, 0x0C, 0x23, 0x01, 0x03, 0x03, 0x01, 0x02, 0x03];
        let error = VendorEvent::new(&bytes).expect_err("group record needs two handles");
        assert_eq!(
            error,
            Error::Vendor(VendorError::AttReadByGroupTypeResponsePartial)
        );
    }
}
