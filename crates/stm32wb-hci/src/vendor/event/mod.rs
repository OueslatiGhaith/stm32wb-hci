//! Vendor-specific events for BlueNRG controllers.
//!
//! The BlueNRG implementation defines several additional events that are packaged as
//! vendor-specific events by the Bluetooth HCI. This module defines those events and functions to
//! deserialize buffers into them.

use core::cmp::PartialEq;
use core::convert::TryInto;
use core::fmt::{Debug, Formatter, Result as FmtResult};

use crate::types::{AttributeHandle, PeerAddrType, to_peer_addr_type};
pub use crate::types::{BdAddrType, ConnectionInterval, ConnectionIntervalError};
use crate::vendor::command::gap::EventFlags;
pub use crate::vendor::command::l2cap::{
    L2CocChannelIndex, L2CocConnectionResult, L2CocCreditIncrement, L2CocInitialCredits, L2CocMps,
    L2CocMtu, L2CocReconfigurationResult, L2CocRequestedChannelCount, L2CocSpsm,
    L2SignalIdentifier,
};
use crate::wire::HciCount;
pub use crate::wire::{EventBytes, EventItems, EventItemsIter, HciEventField, HciEventItem};
use bt_hci::param::{BdAddr, ConnHandle};

stm32wb_hci_macros::wire_type! {
    adapters: [conversion];
    closed
    /// Enumeration of vendor-specific status codes.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum VendorStatus: u8 {
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
    TryFromError = BadVendorStatusError => BadVendorStatusError;
}

/// A byte that does not identify an STM32WB vendor status.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BadVendorStatusError(pub u8);

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

    /// For the [GAP Procedure Complete](VendorEvent::GapProcedureComplete) event: The procedure
    /// code was not recognized. Includes the unrecognized byte.
    BadGapProcedure(u8),

    /// For the [GAP Procedure Complete](VendorEvent::GapProcedureComplete) event: The procedure
    /// status was not recognized. Includes the unrecognized byte.
    BadGapProcedureStatus(u8),

    /// For any L2CAP event: The event data length did not match the expected length. The first
    /// field is the required length, and the second is the actual length.
    BadL2CapDataLength(u8, u8),

    /// For any L2CAP response event: The L2CAP command was rejected, but the rejection reason was
    /// not recognized. Includes the unknown value.
    BadL2CapRejectionReason(u16),

    /// A credit-based event reported an MTU outside the documented domain.
    BadL2CocMtu(crate::vendor::command::HciValueError),

    /// A credit-based event reported an MPS outside the documented domain.
    BadL2CocMps(crate::vendor::command::HciValueError),

    /// A credit-based connection response used an undocumented result value.
    BadL2CocConnectionResult(crate::vendor::command::HciValueError),

    /// A credit-based connection request used an invalid SPSM.
    BadL2CocSpsm(crate::vendor::command::HciValueError),

    /// A credit-based connection request used an invalid channel count.
    BadL2CocRequestedChannelCount(crate::vendor::command::HciValueError),

    /// A credit-based reconfiguration response used an undocumented result value.
    BadL2CocReconfigurationResult(crate::vendor::command::HciValueError),

    /// A credit-based flow-control event reported a zero credit increment.
    BadL2CocCreditIncrement(crate::vendor::command::HciValueError),

    /// For the [L2CAP Connection Update Response](VendorEvent::L2CapConnectionUpdateResponse)
    /// event: The command was accepted, but the result was not recognized. It did not indicate the
    /// parameters were either updated or rejected. Includes the unknown value.
    BadL2CapConnectionResponseResult(u16),

    /// For the [L2CAP Connection Update Request](VendorEvent::L2CapConnectionUpdateRequest) event:
    /// The provided connection interval is invalid. Includes the underlying error.
    BadConnectionInterval(ConnectionIntervalError),

    /// For the [ATT Find Information Response](VendorEvent::AttFindInformationResponse) event: The
    /// format code is invalid. Includes the unrecognized byte.
    BadAttFindInformationResponseFormat(u8),

    /// For the [ATT Find Information Response](VendorEvent::AttFindInformationResponse) event: The
    /// format code indicated 16-bit UUIDs, but the packet ends with a partial pair.
    AttFindInformationResponsePartialPair16,

    /// For the [ATT Find Information Response](VendorEvent::AttFindInformationResponse) event: The
    /// format code indicated 128-bit UUIDs, but the packet ends with a partial pair.
    AttFindInformationResponsePartialPair128,

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

impl HciEventItem<1> for L2CocChannelIndex {
    fn from_validated_hci_event_field(bytes: &[u8; 1]) -> Self {
        Self::new(bytes[0])
    }
}

impl HciEventItem<2> for AttributeHandle {
    fn from_validated_hci_event_field(bytes: &[u8; 2]) -> Self {
        Self(u16::from_le_bytes(*bytes))
    }
}

/// Event-specific diagnostics for a count-prefixed byte-field target.
#[doc(hidden)]
pub trait HciEventCountedBytesTarget<
    'a,
    C,
    const COUNT_LEN: usize,
    const MIN_LEN: usize,
    const MAX_LEN: usize,
>: Sized
{
    fn from_event_counted_bytes(bytes: &'a [u8]) -> Self;

    fn truncated_counted_bytes_error(
        _declared_len: Option<usize>,
        _actual: usize,
        _required: usize,
    ) -> Option<Error> {
        None
    }

    fn counted_bytes_bound_error(_actual: usize, _bound: usize) -> Option<Error> {
        None
    }

    fn validate_counted_bytes_len(_len: usize) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, C, const COUNT_LEN: usize, const MIN_LEN: usize, const MAX_LEN: usize>
    HciEventCountedBytesTarget<'a, C, COUNT_LEN, MIN_LEN, MAX_LEN> for EventBytes<'a, MAX_LEN>
{
    fn from_event_counted_bytes(bytes: &'a [u8]) -> Self {
        Self::from_bytes(bytes)
    }
}

/// Borrowing target for a counted sequence of fixed-width event items.
#[doc(hidden)]
pub trait HciEventCountedItemsTarget<
    'a,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>: Sized
{
    fn from_event_counted_items(bytes: &'a [u8]) -> Self;
}

impl<'a, Item, C, const COUNT_LEN: usize, const ITEM_LEN: usize, const MAX_ITEMS: usize>
    HciEventCountedItemsTarget<'a, Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>
    for EventItems<'a, Item, ITEM_LEN, MAX_ITEMS>
{
    fn from_event_counted_items(bytes: &'a [u8]) -> Self {
        Self::from_bytes(bytes)
    }
}

/// Borrowing target for a record-width and byte-length prefixed event field.
#[doc(hidden)]
pub trait HciEventRecordTarget<'a, const MIN_RECORD_LEN: usize, const MAX_LEN: usize>:
    Sized
{
    fn invalid_record_layout() -> Error;

    fn prefixed_record_length_error(_actual: usize, _required: usize) -> Option<Error> {
        None
    }

    fn from_event_records(record_len: usize, records: &'a [u8]) -> Self;
}

/// Target that reports an unknown tag in a `tagged_items` event field.
#[doc(hidden)]
pub trait HciEventTaggedItemsTarget<Tag>: Sized {
    fn unknown_tag(tag: Tag) -> Error;

    fn truncated_tagged_items_error(_actual: usize, _required: usize) -> Option<Error> {
        None
    }
}

/// One tag-selected fixed-width item representation for a borrowed event field.
#[doc(hidden)]
pub trait HciEventTaggedItemsVariant<
    'a,
    Tag,
    Item: Copy,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>: Sized
{
    fn invalid_items(tag: Tag) -> Error;

    fn from_tagged_items(tag: Tag, records: &'a [u8]) -> Self;
}

/// Borrowing target for a field that consumes the remaining event bytes.
#[doc(hidden)]
pub trait HciEventTrailingBytesTarget<'a, const MIN_LEN: usize, const MAX_LEN: usize>:
    Sized
{
    fn from_event_trailing_bytes(bytes: &'a [u8]) -> Self;
}

impl<'a, const MIN_LEN: usize, const MAX_LEN: usize>
    HciEventTrailingBytesTarget<'a, MIN_LEN, MAX_LEN> for EventBytes<'a, MAX_LEN>
{
    fn from_event_trailing_bytes(bytes: &'a [u8]) -> Self {
        Self::from_bytes(bytes)
    }
}

fn decode_hci_event_field<T, const N: usize>(
    data: &[u8],
    original_len: usize,
) -> Result<(T, &[u8]), Error>
where
    T: HciEventField<N>,
{
    crate::wire::decode_fixed_field(data, T::from_hci_event_field)
        .map_err(|error| map_event_decode_error(error, data.len(), original_len))
}

fn map_event_decode_error(
    error: crate::wire::DecodeError<Error>,
    field_input_len: usize,
    original_len: usize,
) -> Error {
    match error {
        crate::wire::DecodeError::Field(error) => error,
        crate::wire::DecodeError::Truncated { required } => Error::BadLength(
            original_len,
            (original_len - field_input_len).saturating_add(required),
        ),
        crate::wire::DecodeError::CountTooLarge { actual, maximum }
        | crate::wire::DecodeError::SizeOverflow { actual, maximum } => {
            Error::BadLength(actual, maximum)
        }
        crate::wire::DecodeError::CountTooSmall { actual, minimum } => {
            Error::BadLength(actual, minimum)
        }
        crate::wire::DecodeError::LengthOutOfRange {
            actual,
            minimum,
            maximum,
        } => Error::BadLength(actual, if actual < minimum { minimum } else { maximum }),
    }
}

fn decode_hci_event_counted_bytes<
    'a,
    T,
    C,
    const COUNT_LEN: usize,
    const MIN_LEN: usize,
    const MAX_LEN: usize,
>(
    data: &'a [u8],
    original_len: usize,
) -> Result<(T, &'a [u8]), Error>
where
    T: HciEventCountedBytesTarget<'a, C, COUNT_LEN, MIN_LEN, MAX_LEN>,
    C: HciEventField<COUNT_LEN> + HciCount<COUNT_LEN>,
{
    let (value, rest) = crate::wire::decode_counted_bytes::<T, _, COUNT_LEN, MIN_LEN, MAX_LEN>(
        data,
        |bytes| C::from_hci_event_field(bytes).map(HciCount::to_usize),
        T::from_event_counted_bytes,
    )
    .map_err(|error| {
        match &error {
            crate::wire::DecodeError::Truncated { required } => {
                let prefix_len = if data.len() >= COUNT_LEN {
                    COUNT_LEN
                } else {
                    0
                };
                let actual = data.len().saturating_sub(prefix_len);
                let required = required.saturating_sub(prefix_len);
                let declared_len = (prefix_len == COUNT_LEN).then_some(required);
                if let Some(error) =
                    T::truncated_counted_bytes_error(declared_len, actual, required)
                {
                    return error;
                }
            }
            crate::wire::DecodeError::CountTooLarge { actual, maximum } => {
                if let Some(error) = T::counted_bytes_bound_error(*actual, *maximum) {
                    return error;
                }
            }
            crate::wire::DecodeError::CountTooSmall { actual, minimum } => {
                if let Some(error) = T::counted_bytes_bound_error(*actual, *minimum) {
                    return error;
                }
            }
            _ => {}
        }
        map_event_decode_error(error, data.len(), original_len)
    })?;
    let len = data.len() - rest.len() - COUNT_LEN;
    T::validate_counted_bytes_len(len)?;
    Ok((value, rest))
}

fn decode_hci_event_counted_items<
    'a,
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MIN_ITEMS: usize,
    const MAX_ITEMS: usize,
>(
    data: &'a [u8],
    original_len: usize,
) -> Result<(T, &'a [u8]), Error>
where
    T: HciEventCountedItemsTarget<'a, Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>,
    Item: Copy + HciEventField<ITEM_LEN>,
    C: HciEventField<COUNT_LEN> + HciCount<COUNT_LEN>,
{
    let (len, remaining) = crate::wire::decode_fixed_field(data, |bytes| {
        C::from_hci_event_field(bytes).map(HciCount::to_usize)
    })
    .map_err(|error| map_event_decode_error(error, data.len(), original_len))?;
    if len < MIN_ITEMS {
        return Err(Error::BadLength(len, MIN_ITEMS));
    }
    if len > MAX_ITEMS {
        return Err(Error::BadLength(len, MAX_ITEMS));
    }
    let required_items = ITEM_LEN
        .checked_mul(len)
        .ok_or(Error::BadLength(len, MAX_ITEMS))?;
    if remaining.len() < required_items {
        let actual = if ITEM_LEN == 0 {
            0
        } else {
            remaining.len() / ITEM_LEN
        };
        return Err(Error::BadLength(actual, len));
    }
    let (records, rest) = remaining.split_at(required_items);
    for item in records.chunks_exact(ITEM_LEN) {
        let item = <&[u8; ITEM_LEN]>::try_from(item)
            .map_err(|_| Error::BadLength(records.len(), ITEM_LEN))?;
        Item::from_hci_event_field(item)?;
    }
    Ok((T::from_event_counted_items(records), rest))
}

fn decode_hci_event_length_prefixed_records<
    'a,
    T,
    RecordLen,
    Length,
    const RECORD_LEN_WIDTH: usize,
    const LENGTH_WIDTH: usize,
    const MIN_RECORD_LEN: usize,
    const MAX_LEN: usize,
>(
    data: &'a [u8],
    original_len: usize,
) -> Result<(T, &'a [u8]), Error>
where
    T: HciEventRecordTarget<'a, MIN_RECORD_LEN, MAX_LEN>,
    RecordLen: HciEventField<RECORD_LEN_WIDTH> + HciCount<RECORD_LEN_WIDTH>,
    Length: HciEventField<LENGTH_WIDTH> + HciCount<LENGTH_WIDTH>,
{
    let (record_len, records, rest) =
        crate::wire::decode_prefixed_bytes::<usize, _, RECORD_LEN_WIDTH, LENGTH_WIDTH, MAX_LEN>(
            data,
            |bytes| RecordLen::from_hci_event_field(bytes).map(HciCount::to_usize),
            |bytes| Length::from_hci_event_field(bytes).map(HciCount::to_usize),
        )
        .map_err(|error| {
            let required = match &error {
                crate::wire::DecodeError::Truncated { required } => Some(*required),
                crate::wire::DecodeError::CountTooLarge { actual, .. } => RECORD_LEN_WIDTH
                    .checked_add(LENGTH_WIDTH)
                    .and_then(|prefix| prefix.checked_add(*actual)),
                _ => None,
            };
            if let Some(error) =
                required.and_then(|required| T::prefixed_record_length_error(data.len(), required))
            {
                return error;
            }
            map_event_decode_error(error, data.len(), original_len)
        })?;

    if record_len < MIN_RECORD_LEN || !records.len().is_multiple_of(record_len) {
        return Err(T::invalid_record_layout());
    }
    Ok((T::from_event_records(record_len, records), rest))
}

fn decode_hci_event_prefixed_bytes<
    T,
    Tag,
    Length,
    const TAG_WIDTH: usize,
    const LENGTH_WIDTH: usize,
    const MAX_LEN: usize,
>(
    data: &[u8],
    original_len: usize,
) -> Result<(Tag, &[u8], &[u8]), Error>
where
    T: HciEventTaggedItemsTarget<Tag>,
    Tag: HciEventField<TAG_WIDTH>,
    Length: HciEventField<LENGTH_WIDTH> + HciCount<LENGTH_WIDTH>,
{
    crate::wire::decode_prefixed_bytes::<Tag, _, TAG_WIDTH, LENGTH_WIDTH, MAX_LEN>(
        data,
        Tag::from_hci_event_field,
        |bytes| Length::from_hci_event_field(bytes).map(HciCount::to_usize),
    )
    .map_err(|error| {
        let custom_error = match &error {
            crate::wire::DecodeError::Truncated { required } => {
                T::truncated_tagged_items_error(data.len(), *required)
            }
            _ => None,
        };
        if let Some(error) = custom_error {
            return error;
        }
        map_event_decode_error(error, data.len(), original_len)
    })
}

fn decode_hci_event_tagged_items_variant<
    'a,
    T,
    Tag: Copy,
    Item,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>(
    tag: Tag,
    records: &'a [u8],
) -> Result<T, Error>
where
    T: HciEventTaggedItemsVariant<'a, Tag, Item, ITEM_LEN, MAX_ITEMS>,
    Item: Copy + HciEventField<ITEM_LEN>,
{
    if ITEM_LEN == 0
        || !records.len().is_multiple_of(ITEM_LEN)
        || records.len() / ITEM_LEN > MAX_ITEMS
    {
        return Err(T::invalid_items(tag));
    }
    for item in records.chunks_exact(ITEM_LEN) {
        let item = <&[u8; ITEM_LEN]>::try_from(item).map_err(|_| T::invalid_items(tag))?;
        Item::from_hci_event_field(item)?;
    }
    Ok(T::from_tagged_items(tag, records))
}

#[allow(dead_code)]
fn decode_hci_event_trailing_bytes<'a, T, const MIN_LEN: usize, const MAX_LEN: usize>(
    data: &'a [u8],
) -> Result<(T, &'a [u8]), Error>
where
    T: HciEventTrailingBytesTarget<'a, MIN_LEN, MAX_LEN>,
{
    crate::wire::decode_trailing_bytes::<T, Error, MIN_LEN, MAX_LEN>(
        data,
        T::from_event_trailing_bytes,
    )
    .map_err(|error| map_event_decode_error(error, data.len(), data.len()))
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
    #[cfg(before_fw_0_24_0)]
    HalEndOfRadioActivity(0x0004) {
        Payload = {
            last_state: RadioEvent => 1,
            next_state: RadioEvent => 1,
            next_state_sys_time: u32 => 4,
            last_state_slot: u8 => 1,
            next_state_slot: u8 => 1,
        };
    }
    /// End-of-radio-activity event code used by STM32CubeWB 1.24 and newer.
    #[cfg(since_fw_0_24_0)]
    HalEndOfRadioActivity(0x1804) {
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
    #[cfg(before_fw_0_24_0)]
    HalScanReqReport(0x0005) {
        Payload = {
            rssi: u8 => 1,
            peer_addr: PeerAddrType => 7,
        };
    }
    /// Scan-request report event code used by STM32CubeWB 1.24 and newer.
    #[cfg(since_fw_0_24_0)]
    HalScanReqReport(0x1805) {
        Payload = {
            rssi: u8 => 1,
            peer_addr: PeerAddrType => 7,
        };
    }
    /// This event is generated to report firmware error information
    HalFirmwareError(0x0006) {
        Payload = {
            fw_error_type: FirmwareError => 1,
            data: EventBytes<'a, 251> => {
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
    #[cfg(before_fw_0_22_0)]
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
    #[cfg(before_fw_0_22_0)]
    GapBondLost(0x0405) {
        Payload = ();
    }
    /// Bond-lost payload used by STM32CubeWB 1.22 and newer.
    #[cfg(since_fw_0_22_0)]
    GapBondLost(0x0405) {
        Payload = { conn_handle: ConnHandle => 2, };
    }
    /// This event is sent by the GAP to the upper layers when a procedure previously started has
    /// been terminated by the upper layer or has completed for any other reason
    GapProcedureComplete(0x0407) {
        Payload = {
            procedure: GapProcedureKind => 1,
            status: GapProcedureStatus => 1,
            data: EventBytes<'a, 250> => {
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
    /// This event asks the application to accept or reject an incoming pairing request.
    #[cfg(since_fw_0_21_0)]
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
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
    }
    /// The event is given by the L2CAP layer when a connection update request is received from the
    /// peripheral. The application has to respond by calling
    /// [l2cap_connection_parameter_update_response](crate::vendor::command::l2cap::L2ConnectionParameterUpdateResponse).
    L2CapConnectionUpdateRequest(0x0802) {
        Payload = {
            conn_handle: ConnHandle => 2,
            identifier: L2SignalIdentifier => 1,
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
            identifier: L2SignalIdentifier => 1,
            reason: L2CapRejectionReason => 2,
            data: EventBytes<'a, 247> => {
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
            spsm: L2CocSpsm => 2,
            mtu: L2CocMtu => 2,
            mps: L2CocMps => 2,
            initial_credits: L2CocInitialCredits => 2,
            channel_number: L2CocRequestedChannelCount => 1,
        };
    }
    /// This event is generated when receiving a valid Credit Based Connection Response packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocConnectConfirm(0x0811) {
        Payload = {
            conn_handle: ConnHandle => 2,
            mtu: L2CocMtu => 2,
            mps: L2CocMps => 2,
            initial_credits: L2CocInitialCredits => 2,
            result: L2CocConnectionResult => 2,
            channel_indices: EventItems<'a, L2CocChannelIndex, 1, 242> => {
                kind: counted_items,
                count: u8 => 1,
                item: L2CocChannelIndex => 1,
                max_items: 242,
            },
        };
    }
    /// This event is generated when receiving a valid Credit Based Reconfigure Request packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocReconfig(0x0812) {
        Payload = {
            conn_handle: ConnHandle => 2,
            mtu: L2CocMtu => 2,
            mps: L2CocMps => 2,
            channel_indices: EventItems<'a, L2CocChannelIndex, 1, 246> => {
                kind: counted_items,
                count: u8 => 1,
                item: L2CocChannelIndex => 1,
                min_items: 1,
                max_items: 246,
                storage_min_len: 1,
            },
        };
    }
    /// This event is generated when receiving a valid Credit Based Reconfigure Response packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocReconfigConfirm(0x0813) {
        Payload = {
            conn_handle: ConnHandle => 2,
            result: L2CocReconfigurationResult => 2,
        };
    }
    /// This event is generated when a connection-oriented channel is disconnected following an
    /// L2CAP channel termination procedure.
    ///
    /// Includes the channel index of the connection oriented channel for which the primitive applies
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocDisconnect(0x0814) {
        Payload = { channel_index: L2CocChannelIndex => 1, };
    }
    /// This event is generated when receiving a valid Flow Control Credit signaling packet.
    ///
    /// See Bluetooth spec. v.5.4 [Vol 3, Part A].
    L2CapCocFlowControl(0x0815) {
        Payload = {
            channel_index: L2CocChannelIndex => 1,
            credits: L2CocCreditIncrement => 2,
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
            channel_index: L2CocChannelIndex => 1,
            data: EventBytes<'a, 250> => {
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
            data: EventBytes<'a, 245> => {
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
            server_rx_mtu: u16 => 2,
        };
    }
    /// This event is generated in response to a Find Information Request. See Find Information
    /// Response in Bluetooth Core v4.0 spec.
    AttFindInformationResponse(0x0C04) {
        Payload = {
            conn_handle: ConnHandle => 2,
            handle_uuid_pairs: HandleUuidPairs<'a> => {
                kind: tagged_items,
                tag: u8 => 1,
                length: u8 => 1,
                variants: {
                    0x01 => {
                        item: HandleUuid16Pair => 4,
                        max_items: 62,
                    },
                    0x02 => {
                        item: HandleUuid128Pair => 18,
                        max_items: 13,
                    },
                },
                max_len: 249,
            },
        };
    }
    /// This event is generated in response to a Find By Type Value Request.
    AttFindByTypeValueResponse(0x0C05) {
        Payload = {
            conn_handle: ConnHandle => 2,
            handles: EventItems<'a, HandleInfoPair, 4, 62> => {
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
            pairs: HandleValuePairs<'a> => {
                kind: length_prefixed_records,
                record_len: u8 => 1,
                length: u8 => 1,
                min_record_len: 2,
                max_len: 249,
            },
        };
    }
    /// This event is generated in response to a Read Request.
    AttReadResponse(0x0C07) {
        Payload = {
            conn_handle: ConnHandle => 2,
            value: EventBytes<'a, 250> => {
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
            value: EventBytes<'a, 250> => {
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
            value: EventBytes<'a, 250> => {
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
            groups: AttributeGroups<'a> => {
                kind: length_prefixed_records,
                record_len: u8 => 1,
                length: u8 => 1,
                min_record_len: 4,
                max_len: 249,
            },
        };
    }
    /// This event is generated in response to a Prepare Write Request. See the Bluetooth Core v4.1
    /// spec, Vol 3, Part F, section 3.4.6.1 and 3.4.6.2
    AttPrepareWriteResponse(0x0C0C) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: EventBytes<'a, 246> => {
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
            value: EventBytes<'a, 248> => {
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
            value: EventBytes<'a, 248> => {
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
            value: EventBytes<'a, 248> => {
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
            value: EventBytes<'a, 248> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
    }
    /// This event is given to the application when a read request or read blob request is received
    /// by the server from the client. This event will be given to the application only if the event
    /// bit for this event generation is set when the characteristic was added. On receiving this
    /// event, the application can update the value of the handle if it desires and then use the
    /// firmware's read-permission response command to tell the stack it can respond to the client.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part F, section 3.4.4.
    AttReadPermitRequest(0x0C14) {
        Payload = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: u16 => 2,
        };
    }
    /// This event is given to the application when a read multiple request or read by type request
    /// is received by the server from the client. This event will be given to the application only
    /// if the event bit for this event generation is set when the characteristic was added.  On
    /// receiving this event, the application can update the values of the handles if it desires and
    /// then use the firmware's read-permission response command to tell the stack it can respond to
    /// the client.
    ///
    /// See the Bluetooth Core v4.1 spec, Vol 3, Part F, section 3.4.4.
    AttReadMultiplePermitRequest(0x0C15) {
        Payload = {
            conn_handle: ConnHandle => 2,
            handles: EventItems<'a, AttributeHandle, 2, 125> => {
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
            available_buffers: u16 => 2,
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
            offset: u16 => 2,
            value: EventBytes<'a, 246> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 246,
            },
        };
    }
    /// This event informs the application of a change in status of the enhanced ATT bearer handled
    /// by the special L2CAP channel.
    #[cfg(before_fw_0_23_0)]
    GattEattBrearer(0x0C19) {
        Payload = {
            channel_index: L2CocChannelIndex => 1,
            eab_state: EabState => 1,
            status: GattProcedureStatus => 1,
        };
    }
    /// Enhanced ATT bearer payload used by STM32CubeWB 1.23 and newer.
    #[cfg(since_fw_0_23_0)]
    GattEattBrearer(0x0C19) {
        Payload = {
            conn_handle: ConnHandle => 2,
            channel_index: L2CocChannelIndex => 1,
            eab_state: EabState => 1,
            mtu: L2CocMtu => 2,
        };
    }
    /// This event is generated when a Multiple Handle Value Notification is received from the server.
    GattMultiNotification(0x0C1A) {
        Payload = {
            conn_handle: ConnHandle => 2,
            offset: u16 => 2,
            data: EventBytes<'a, 247> => {
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
            value: EventBytes<'a, 247> => {
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
            value: EventBytes<'a, 245> => {
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
            value: EventBytes<'a, 245> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 245,
            },
        };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Potential firmware kinds for [`CoprocessorReady`](VendorEvent::CoprocessorReady)
    /// event.
    #[derive(Clone, Copy, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum FirmwareKind: u8 => 1 {
        /// Wireless firmware (BLE, Thread, etc.)
        Wireless = 0,
        /// RCC firmware.
        Rcc = 1,
    }
    TryFromError = VendorError => VendorError::UnknownFirmwareKind;
    EventError = Error::Vendor;
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Reasons why an L2CAP command was rejected. See the Bluetooth specification, v4.1, Vol 3,
    /// Part A, Section 4.1.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum L2CapRejectionReason: u16 => 2 {
        /// The controller sent an unknown command.
        CommandNotUnderstood = 0,
        /// When multiple commands are included in an L2CAP packet and the packet exceeds the
        /// signaling MTU (MTUsig) of the receiver, a single Command Reject packet shall be sent in
        /// response.
        SignalingMtuExceeded = 1,
        /// Invalid CID in request
        InvalidCid = 2,
    }
    TryFromError = VendorError => VendorError::BadL2CapRejectionReason;
    EventError = Error::Vendor;
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Results reported by the L2CAP connection update response event.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum L2CapConnectionUpdateResult: u16 => 2 {
        /// The connection parameters were accepted and updated.
        ParametersUpdated = 0x0000,
        /// The connection parameters were rejected.
        ParametersRejected = 0x0001,
    }
    TryFromError = VendorError => VendorError::BadL2CapConnectionResponseResult;
    EventError = Error::Vendor;
}

/// Zero-length L2CAP event data, including its required wire count.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EmptyL2CapData;

impl<'a> HciEventCountedBytesTarget<'a, u8, 1, 0, 250> for EmptyL2CapData {
    fn from_event_counted_bytes(_bytes: &'a [u8]) -> Self {
        Self
    }

    fn truncated_counted_bytes_error(
        declared_len: Option<usize>,
        actual: usize,
        required: usize,
    ) -> Option<Error> {
        Some(if let Some(declared_len) = declared_len {
            Error::Vendor(VendorError::BadL2CapDataLength(
                declared_len
                    .try_into()
                    .expect("the declared u8 count cannot exceed u8::MAX"),
                0,
            ))
        } else {
            Error::BadLength(actual, required)
        })
    }

    fn counted_bytes_bound_error(actual: usize, _bound: usize) -> Option<Error> {
        Some(Error::Vendor(VendorError::BadL2CapDataLength(
            actual
                .try_into()
                .expect("the declared u8 count cannot exceed u8::MAX"),
            0,
        )))
    }

    fn validate_counted_bytes_len(len: usize) -> Result<(), Error> {
        if len == 0 {
            Ok(())
        } else {
            Err(Error::Vendor(VendorError::BadL2CapDataLength(
                len.try_into()
                    .expect("the declared u8 count cannot exceed u8::MAX"),
                0,
            )))
        }
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

stm32wb_hci_macros::wire_type! {
    adapters: [conversion];
    closed
    /// Reasons the [GAP Pairing Complete](VendorEvent::GapPairingComplete) event failed.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum GapPairingReason: u8 {
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
    TryFromError = VendorError => VendorError::BadGapPairingErrorReason;
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// GAP procedure discriminator carried by the procedure-complete event.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum GapProcedureKind: u8 => 1 {
        LimitedDiscovery = 0x01,
        GeneralDiscovery = 0x02,
        NameDiscovery = 0x04,
        AutoConnectionEstablishment = 0x08,
        GeneralConnectionEstablishment = 0x10,
        SelectiveConnectionEstablishment = 0x20,
        DirectConnectionEstablishment = 0x40,
        Observation = 0x80,
    }
    TryFromError = VendorError => VendorError::BadGapProcedure;
    EventError = Error::Vendor;
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Possible results of a [GAP procedure](VendorEvent::GapProcedureComplete).
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum GapProcedureStatus: u8 => 1 {
        /// BLE Status Success.
        Success = 0x00,
        /// BLE Status Failed.
        Failed = 0x41,
        /// Procedure failed due to authentication requirements.
        AuthFailure = 0x05,
    }
    TryFromError = VendorError => VendorError::BadGapProcedureStatus;
    EventError = Error::Vendor;
}

impl GattAttributeModified<'_> {
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

impl AttFindInformationResponse<'_> {
    /// The Find Information Response shall have complete handle-UUID pairs. Such pairs shall not be
    /// split across response packets; this also implies that a handleUUID pair shall fit into a
    /// single response packet. The handle-UUID pairs shall be returned in ascending order of
    /// attribute handles.
    pub fn handle_uuid_pair_iter(&self) -> HandleUuidPairIterator<'_> {
        match self.handle_uuid_pairs {
            HandleUuidPairs::Format16(data) => {
                HandleUuidPairIterator::Format16(HandleUuid16PairIterator {
                    chunks: data.chunks_exact(4),
                })
            }
            HandleUuidPairs::Format128(data) => {
                HandleUuidPairIterator::Format128(HandleUuid128PairIterator {
                    chunks: data.chunks_exact(18),
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

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    composite
    HandleUuid16Pair => 4 {
        Fields = {
            handle: AttributeHandle => 2,
            uuid: u16 => 2,
        };
        Decode = {
            Ok(Self {
                handle,
                uuid: Uuid16(uuid),
            })
        };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    composite
    HandleUuid128Pair => 18 {
        Fields = {
            handle: AttributeHandle => 2,
            uuid: [u8; 16] => 16,
        };
        Decode = {
            Ok(Self {
                handle,
                uuid: Uuid128(uuid),
            })
        };
    }
}

/// Newtype for the 16-bit UUID buffer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Uuid16(pub u16);

/// Newtype for the 128-bit UUID buffer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Uuid128(pub [u8; 16]);

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(missing_docs)]
pub enum HandleUuidPairs<'a> {
    Format16(&'a [u8]),
    Format128(&'a [u8]),
}

impl HciEventTaggedItemsTarget<u8> for HandleUuidPairs<'_> {
    fn unknown_tag(tag: u8) -> Error {
        Error::Vendor(VendorError::BadAttFindInformationResponseFormat(tag))
    }

    fn truncated_tagged_items_error(actual: usize, required: usize) -> Option<Error> {
        Some(Error::BadLength(actual, required))
    }
}

impl<'a> HciEventTaggedItemsVariant<'a, u8, HandleUuid16Pair, 4, MAX_FORMAT16_PAIR_COUNT>
    for HandleUuidPairs<'a>
{
    fn invalid_items(_tag: u8) -> Error {
        Error::Vendor(VendorError::AttFindInformationResponsePartialPair16)
    }

    fn from_tagged_items(_tag: u8, records: &'a [u8]) -> Self {
        Self::Format16(records)
    }
}

impl<'a> HciEventTaggedItemsVariant<'a, u8, HandleUuid128Pair, 18, MAX_FORMAT128_PAIR_COUNT>
    for HandleUuidPairs<'a>
{
    fn invalid_items(_tag: u8) -> Error {
        Error::Vendor(VendorError::AttFindInformationResponsePartialPair128)
    }

    fn from_tagged_items(_tag: u8, records: &'a [u8]) -> Self {
        Self::Format128(records)
    }
}

impl Debug for HandleUuidPairs<'_> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            HandleUuidPairs::Format16(bytes) => f.debug_tuple("Format16").field(bytes).finish(),
            HandleUuidPairs::Format128(bytes) => f.debug_tuple("Format128").field(bytes).finish(),
        }
    }
}

/// Possible iterators over handle-UUID pairs that can be returnedby the
/// [ATT find information response](AttFindInformationResponse). All pairs from the same event have the same format.
pub enum HandleUuidPairIterator<'a> {
    /// The event contains 16-bit UUIDs.
    Format16(HandleUuid16PairIterator<'a>),
    /// The event contains 128-bit UUIDs.
    Format128(HandleUuid128PairIterator<'a>),
}

/// Iterator over handle-UUID pairs for 16-bit UUIDs.
pub struct HandleUuid16PairIterator<'a> {
    chunks: core::slice::ChunksExact<'a, u8>,
}

impl<'a> Iterator for HandleUuid16PairIterator<'a> {
    type Item = HandleUuid16Pair;
    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.chunks.next()?;
        Some(HandleUuid16Pair {
            handle: AttributeHandle(u16::from_le_bytes([bytes[0], bytes[1]])),
            uuid: Uuid16(u16::from_le_bytes([bytes[2], bytes[3]])),
        })
    }
}

/// Iterator over handle-UUID pairs for 128-bit UUIDs.
pub struct HandleUuid128PairIterator<'a> {
    chunks: core::slice::ChunksExact<'a, u8>,
}

impl<'a> Iterator for HandleUuid128PairIterator<'a> {
    type Item = HandleUuid128Pair;
    fn next(&mut self) -> Option<Self::Item> {
        let bytes = self.chunks.next()?;
        let mut uuid = [0; 16];
        uuid.copy_from_slice(&bytes[2..]);
        Some(HandleUuid128Pair {
            handle: AttributeHandle(u16::from_le_bytes([bytes[0], bytes[1]])),
            uuid: Uuid128(uuid),
        })
    }
}

impl AttFindByTypeValueResponse<'_> {
    /// Returns an iterator over the Handles Information List as defined in Bluetooth Core v4.1
    /// spec.
    pub fn handle_pairs_iter(&self) -> EventItemsIter<'_, HandleInfoPair, 4> {
        self.handles.iter()
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

impl HandleInfoPair {
    fn from_wire_bytes(bytes: &[u8; 4]) -> Self {
        Self {
            attribute: AttributeHandle(u16::from_le_bytes([bytes[0], bytes[1]])),
            group_end: GroupEndHandle(u16::from_le_bytes([bytes[2], bytes[3]])),
        }
    }
}

impl HciEventItem<4> for HandleInfoPair {
    fn from_validated_hci_event_field(bytes: &[u8; 4]) -> Self {
        Self::from_wire_bytes(bytes)
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    composite
    HandleInfoPair => 4 {
        Fields = {
            attribute: AttributeHandle => 2,
            group_end: u16 => 2,
        };
        Decode = {
            Ok(Self {
                attribute,
                group_end: GroupEndHandle(group_end),
            })
        };
    }
}

impl crate::vendor::command::HciDecodeField<4> for HandleInfoPair {
    fn from_hci_field(bytes: &[u8; 4]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(Self::from_wire_bytes(bytes))
    }
}

/// Newtype for Group End handles
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GroupEndHandle(pub u16);

/// Borrowed ATT handle-value records decoded from a Read By Type response.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HandleValuePairs<'a> {
    data: &'a [u8],
    value_len: usize,
}

impl<'a> HciEventRecordTarget<'a, 2, 249> for HandleValuePairs<'a> {
    fn invalid_record_layout() -> Error {
        Error::Vendor(VendorError::AttReadByTypeResponsePartial)
    }

    fn prefixed_record_length_error(actual: usize, required: usize) -> Option<Error> {
        Some(Error::BadLength(actual, required))
    }

    fn from_event_records(pair_len: usize, records: &'a [u8]) -> Self {
        Self {
            data: records,
            value_len: pair_len - 2,
        }
    }
}

impl AttReadByTypeResponse<'_> {
    /// Return an iterator over all valid handle-value pairs returned with the response.
    pub fn handle_value_pair_iter(&self) -> impl Iterator<Item = HandleValuePair<'_>> {
        let record_len = self.pairs.value_len + 2;
        self.pairs
            .data
            .chunks_exact(record_len)
            .map(|record| HandleValuePair {
                handle: AttributeHandle(u16::from_le_bytes([record[0], record[1]])),
                value: &record[2..],
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

impl AttReadResponse<'_> {
    /// Returns the valid part of the value data.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

impl AttReadBlobResponse<'_> {
    /// Returns the valid part of the value data.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

impl AttReadMultipleResponse<'_> {
    /// Returns the valid part of the value data.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

/// Borrowed ATT attribute groups decoded from a Read By Group Type response.
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttributeGroups<'a> {
    data: &'a [u8],
    group_len: usize,
}

impl<'a> HciEventRecordTarget<'a, 4, 249> for AttributeGroups<'a> {
    fn invalid_record_layout() -> Error {
        Error::Vendor(VendorError::AttReadByGroupTypeResponsePartial)
    }

    fn prefixed_record_length_error(actual: usize, required: usize) -> Option<Error> {
        Some(Error::BadLength(actual, required))
    }

    fn from_event_records(group_len: usize, records: &'a [u8]) -> Self {
        Self {
            data: records,
            group_len,
        }
    }
}

impl AttReadByGroupTypeResponse<'_> {
    /// Create and return an iterator for the attribute data returned with the response.
    pub fn attribute_data_iter(&self) -> impl Iterator<Item = AttributeData<'_>> {
        self.groups
            .data
            .chunks_exact(self.groups.group_len)
            .map(|record| AttributeData {
                attribute_handle: AttributeHandle(u16::from_le_bytes([record[0], record[1]])),
                attribute_end_handle: AttributeHandle(u16::from_le_bytes([record[2], record[3]])),
                value: &record[4..],
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

/// UUID carried by an ATT attribute-group record.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AttributeUuid {
    /// Bluetooth 16-bit UUID.
    Uuid16(Uuid16),
    /// Bluetooth 128-bit UUID.
    Uuid128(Uuid128),
}

/// An ATT attribute-group value did not contain a complete UUID.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttributeUuidError {
    actual_length: usize,
}

impl AttributeUuidError {
    /// Number of UUID bytes present in the attribute-group value.
    pub const fn actual_length(self) -> usize {
        self.actual_length
    }
}

impl<'a> AttributeData<'a> {
    /// Decode the service UUID carried by this attribute-group value.
    ///
    /// ATT Read By Group Type responses carry either a 16-bit or 128-bit UUID.
    /// Other value lengths are reported instead of indexing beyond the record.
    pub fn uuid(&self) -> Result<AttributeUuid, AttributeUuidError> {
        match self.value {
            [low, high] => Ok(AttributeUuid::Uuid16(Uuid16(u16::from_le_bytes([
                *low, *high,
            ])))),
            bytes if bytes.len() == 16 => {
                let mut uuid = [0; 16];
                uuid.copy_from_slice(bytes);
                Ok(AttributeUuid::Uuid128(Uuid128(uuid)))
            }
            bytes => Err(AttributeUuidError {
                actual_length: bytes.len(),
            }),
        }
    }
}

impl AttPrepareWriteResponse<'_> {
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
    GattIndication<'_>,
    GattNotification<'_>,
    GattDiscoverOrReadCharacteristicByUuidResponse<'_>,
    AttWritePermitRequest<'_>,
    GattReadExt<'_>,
    GattIndicationExt<'_>,
    GattNotificationExt<'_>,
);

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Allowed status codes for the [GATT Procedure Complete](VendorEvent::GattProcedureComplete)
    /// event.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum GattProcedureStatus: u8 => 1 {
        /// BLE Status Success
        Success = 0x00,
        /// BLE Status Failed
        Failed = 0x41,
    }
    TryFromError = Error => |value| Error::Vendor(VendorError::BadGattProcedureStatus(value));
    EventError = core::convert::identity;
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Potential error codes for the [ATT Error Response](VendorEvent::AttErrorResponse). See
    /// Table 3.3 in the Bluetooth Core Specification, v4.1, Vol 3, Part F, Section 3.4.1.1 and
    /// The Bluetooth Core Specification Supplement, Table 1.1.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AttError: u8 => 1 {
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
    TryFromError = u8 => core::convert::identity;
    EventError = |value| Error::Vendor(VendorError::BadAttError(value));
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Possible ATT requests. See Table 3.37 in the Bluetooth Core Spec v4.1, Vol 3, Part F,
    /// Section 3.4.8.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AttRequest: u8 => 1 {
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
    TryFromError = VendorError => VendorError::BadAttRequestOpcode;
    EventError = Error::Vendor;
}

impl AttReadMultiplePermitRequest<'_> {
    /// Iterates over the attribute handles in the ATT Read Multiple Permit Request event.
    pub fn handles(&self) -> EventItemsIter<'_, AttributeHandle, 2> {
        self.handles.iter()
    }
}

impl AttPrepareWritePermitRequest<'_> {
    /// Returns the data to be written.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    open_enum
    /// Type of keypress input notified by a peer with keyboard I/O capabilities.
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum KeypressNotificationType: u8 => 1 {
        EntryStarted = 0x00,
        DigitEntered = 0x01,
        DigitErased = 0x02,
        PasskeyCleared = 0x03,
        EntryCompleted = 0x04,
        _ => Reserved,
    }
}

/// Preferred spelling alias kept for API ergonomics.
pub type GattEattBearer = GattEattBrearer;

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Enhanced ATT bearer state.
    #[derive(Debug, Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum EabState: u8 => 1 {
        AttBearerCreated = 0x00,
        AttBearerTerminated = 0x01,
        /// The bearer MTU was reconfigured.
        AttBearerReconfigured = 0x02,
    }
    TryFromError = Error => |value| Error::Vendor(VendorError::BadEabState(value));
    EventError = core::convert::identity;
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Radio state reported by the end-of-radio-activity event.
    #[derive(Debug, Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum RadioEvent: u8 => 1 {
        Idle = 0x00,
        Advertising = 0x01,
        PeripheralConnection = 0x02,
        Scanning = 0x03,
        CentralConnection = 0x05,
        TxTestMode = 0x06,
        RxTestMode = 0x07,
    }
    TryFromError = Error => |value| Error::Vendor(VendorError::BadRadioEvent(value));
    EventError = core::convert::identity;
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    closed
    /// Defines error types returned by [HAL Firmware Error](VendorEvent::HalFirmwareError) event.
    #[derive(Debug, Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum FirmwareError: u8 => 1 {
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
    TryFromError = Error => |value| Error::Vendor(VendorError::BadFirmwareError(value));
    EventError = core::convert::identity;
}

impl HalFirmwareError<'_> {
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    composite
    PeerAddrType => 7 {
        Fields = {
            address_type: u8 => 1,
            address: [u8; 6] => 6,
        };
        Decode = {
            to_peer_addr_type(address_type, BdAddr(address))
                .map_err(|error| Error::Vendor(VendorError::BadBdAddrType(error.0)))
        };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    composite
    GapPairingStatus => 2 {
        Fields = {
            status: u8 => 1,
            reason: u8 => 1,
        };
        Decode = {
            match status {
                0 => Ok(GapPairingStatus::Success),
                1 => reason
                    .try_into()
                    .map(GapPairingStatus::Timeout)
                    .map_err(Error::Vendor),
                2 => reason
                    .try_into()
                    .map(GapPairingStatus::Failed)
                    .map_err(Error::Vendor),
                3 => reason
                    .try_into()
                    .map(GapPairingStatus::EncryptionFailed)
                    .map_err(Error::Vendor),
                _ => Err(Error::Vendor(VendorError::BadGapPairingStatus(status))),
            }
        };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [event];
    composite
    ConnectionInterval => 8 {
        Fields = {
            interval_min: u16 => 2,
            interval_max: u16 => 2,
            latency: u16 => 2,
            timeout: u16 => 2,
        };
        Decode = {
            ConnectionInterval::from_hci_fields(interval_min, interval_max, latency, timeout)
                .map_err(VendorError::BadConnectionInterval)
                .map_err(Error::Vendor)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_event_is_a_compact_packet_view() {
        let bytes = [0x07, 0x0C, 0x23, 0x01, 0x02, 0xAA, 0xBB];
        let VendorEvent::AttReadResponse(event) =
            VendorEvent::new(&bytes).expect("valid read response")
        else {
            panic!("unexpected event variant");
        };

        assert_eq!(event.value.as_slice(), &[0xAA, 0xBB]);
        assert_eq!(event.value.as_slice().as_ptr(), bytes[5..].as_ptr());
        assert!(core::mem::size_of::<VendorEvent<'static>>() <= 64);
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

    #[cfg(before_fw_0_23_0)]
    #[test]
    fn parses_gatt_eatt_bearer_event() {
        // 0x0C19 + channel_index(2) + eab_state(created) + status(success)
        let bytes = [0x19, 0x0C, 0x02, 0x00, 0x00];
        let event = VendorEvent::new(&bytes).expect("parse eatt bearer");

        match event {
            VendorEvent::GattEattBrearer(e) => {
                assert_eq!(e.channel_index, L2CocChannelIndex::new(2));
                assert!(matches!(e.eab_state, EabState::AttBearerCreated));
                assert_eq!(e.status, GattProcedureStatus::Success);
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[cfg(since_fw_0_23_0)]
    #[test]
    fn parses_current_gatt_eatt_bearer_event() {
        let bytes = [0x19, 0x0C, 0x23, 0x01, 0x02, 0x02, 0x40, 0x00];
        let event = VendorEvent::new(&bytes).expect("parse current eatt bearer");

        match event {
            VendorEvent::GattEattBrearer(e) => {
                assert_eq!(e.conn_handle, ConnHandle::new(0x0123));
                assert_eq!(e.channel_index, L2CocChannelIndex::new(2));
                assert!(matches!(e.eab_state, EabState::AttBearerReconfigured));
                assert_eq!(e.mtu.value(), 64);
            }
            _ => panic!("unexpected event variant"),
        }
    }

    #[test]
    fn declarative_event_enums_preserve_their_invalid_value_errors() {
        assert_eq!(
            FirmwareKind::try_from(2).unwrap_err(),
            VendorError::UnknownFirmwareKind(2)
        );
        assert_eq!(
            GapProcedureKind::try_from(3).unwrap_err(),
            VendorError::BadGapProcedure(3)
        );
        assert_eq!(
            GapProcedureStatus::try_from(0x42).unwrap_err(),
            VendorError::BadGapProcedureStatus(0x42)
        );
        assert_eq!(
            GattProcedureStatus::try_from(0x42).unwrap_err(),
            Error::Vendor(VendorError::BadGattProcedureStatus(0x42))
        );
        assert_eq!(
            AttRequest::try_from(0).unwrap_err(),
            VendorError::BadAttRequestOpcode(0)
        );
        assert_eq!(AttError::try_from(0x14).unwrap_err(), 0x14);
        assert_eq!(
            EabState::try_from(3).unwrap_err(),
            Error::Vendor(VendorError::BadEabState(3))
        );
        assert_eq!(
            RadioEvent::try_from(4).unwrap_err(),
            Error::Vendor(VendorError::BadRadioEvent(4))
        );
        assert_eq!(
            FirmwareError::try_from(0).unwrap_err(),
            Error::Vendor(VendorError::BadFirmwareError(0))
        );
        assert_eq!(
            <AttError as HciEventField<1>>::from_hci_event_field(&[0x14]).unwrap_err(),
            Error::Vendor(VendorError::BadAttError(0x14))
        );
    }

    #[test]
    fn declarative_conversion_enums_are_bidirectional_and_closed() {
        assert_eq!(
            VendorStatus::try_from(0).unwrap_err(),
            BadVendorStatusError(0)
        );
        assert_eq!(u8::from(VendorStatus::InsufficientResources), 0x64);

        assert_eq!(
            L2CapRejectionReason::try_from(3).unwrap_err(),
            VendorError::BadL2CapRejectionReason(3)
        );
        assert_eq!(u16::from(L2CapRejectionReason::InvalidCid), 2);

        assert_eq!(
            GapPairingReason::try_from(0x0D).unwrap_err(),
            VendorError::BadGapPairingErrorReason(0x0D)
        );
        assert_eq!(u8::from(GapPairingReason::KeyRejected), 0x0F);
    }

    #[test]
    fn declarative_open_event_enums_retain_unknown_wire_values() {
        assert_eq!(
            <KeypressNotificationType as HciEventField<1>>::from_hci_event_field(&[0x04]).unwrap(),
            KeypressNotificationType::EntryCompleted
        );

        let reserved =
            <KeypressNotificationType as HciEventField<1>>::from_hci_event_field(&[0xA5]).unwrap();
        assert_eq!(reserved, KeypressNotificationType::Reserved(0xA5));
        assert_eq!(u8::from(reserved), 0xA5);
    }

    #[test]
    fn declarative_composite_fields_preserve_contextual_semantics() {
        let public_identity =
            <PeerAddrType as HciEventField<7>>::from_hci_event_field(&[2, 1, 2, 3, 4, 5, 6])
                .expect("public identity address");
        assert!(matches!(
            public_identity,
            PeerAddrType::PublicIdentityAddress(BdAddr([1, 2, 3, 4, 5, 6]))
        ));
        assert_eq!(
            <PeerAddrType as HciEventField<7>>::from_hci_event_field(&[4, 0, 0, 0, 0, 0, 0])
                .unwrap_err(),
            Error::Vendor(VendorError::BadBdAddrType(4))
        );

        assert_eq!(
            <GapPairingStatus as HciEventField<2>>::from_hci_event_field(&[0, 0xFF])
                .expect("success ignores the reason byte"),
            GapPairingStatus::Success
        );
        assert_eq!(
            <GapPairingStatus as HciEventField<2>>::from_hci_event_field(&[1, 0x01])
                .expect("timeout reason"),
            GapPairingStatus::Timeout(GapPairingReason::PasskeyEntryFailed)
        );
        assert_eq!(
            <GapPairingStatus as HciEventField<2>>::from_hci_event_field(&[1, 0x0D]).unwrap_err(),
            Error::Vendor(VendorError::BadGapPairingErrorReason(0x0D))
        );
        assert_eq!(
            <GapPairingStatus as HciEventField<2>>::from_hci_event_field(&[4, 0x0D]).unwrap_err(),
            Error::Vendor(VendorError::BadGapPairingStatus(4))
        );

        assert_eq!(
            <L2CapConnectionUpdateResult as HciEventField<2>>::from_hci_event_field(&[0, 0])
                .expect("accepted connection update"),
            L2CapConnectionUpdateResult::ParametersUpdated
        );
        assert_eq!(
            <L2CapConnectionUpdateResult as HciEventField<2>>::from_hci_event_field(&[2, 0])
                .unwrap_err(),
            Error::Vendor(VendorError::BadL2CapConnectionResponseResult(2))
        );

        let interval = <ConnectionInterval as HciEventField<8>>::from_hci_event_field(&[
            6, 0, // 7.5 ms minimum
            6, 0, // 7.5 ms maximum
            0, 0, // zero connection latency
            11, 0, // 110 ms supervision timeout
        ])
        .expect("valid connection interval");
        assert_eq!(
            interval.interval(),
            (
                core::time::Duration::from_micros(7_500),
                core::time::Duration::from_micros(7_500)
            )
        );
        assert_eq!(
            <ConnectionInterval as HciEventField<8>>::from_hci_event_field(&[0; 8]).unwrap_err(),
            Error::Vendor(VendorError::BadConnectionInterval(
                ConnectionIntervalError::IntervalTooShort(core::time::Duration::ZERO)
            ))
        );
    }

    #[test]
    fn fixed_two_byte_event_scalars_use_their_wire_type() {
        let VendorEvent::AttExchangeMtuResponse(event) =
            VendorEvent::new(&[0x03, 0x0C, 0x23, 0x01, 0x00, 0x02]).expect("exchange MTU response")
        else {
            panic!("unexpected event variant");
        };
        let server_rx_mtu: u16 = event.server_rx_mtu;
        assert_eq!(server_rx_mtu, 0x0200);

        let VendorEvent::AttPrepareWriteResponse(event) = VendorEvent::new(&[
            0x0C, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x34, 0x12, // attribute handle
            0x78, 0x56, // offset
            0x00, // empty value
        ])
        .expect("prepare write response") else {
            panic!("unexpected event variant");
        };
        let offset: u16 = event.offset;
        assert_eq!(offset, 0x5678);

        let VendorEvent::AttReadPermitRequest(event) =
            VendorEvent::new(&[0x14, 0x0C, 0x23, 0x01, 0x34, 0x12, 0x78, 0x56])
                .expect("read permit request")
        else {
            panic!("unexpected event variant");
        };
        let offset: u16 = event.offset;
        assert_eq!(offset, 0x5678);

        let VendorEvent::GattTxPoolAvailable(event) =
            VendorEvent::new(&[0x16, 0x0C, 0x23, 0x01, 0x78, 0x56]).expect("TX pool available")
        else {
            panic!("unexpected event variant");
        };
        let available_buffers: u16 = event.available_buffers;
        assert_eq!(available_buffers, 0x5678);

        let VendorEvent::AttPrepareWritePermitRequest(event) = VendorEvent::new(&[
            0x18, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x34, 0x12, // attribute handle
            0x78, 0x56, // offset
            0x00, // empty value
        ])
        .expect("prepare write permit request") else {
            panic!("unexpected event variant");
        };
        let offset: u16 = event.offset;
        assert_eq!(offset, 0x5678);
    }

    #[test]
    fn l2cap_command_reject_uses_the_declared_reason_type() {
        let bytes = [
            0x0A, 0x08, // event code
            0x23, 0x01, // connection handle
            0x07, // identifier
            0x02, 0x00, // invalid CID
            0x00, // no reason-specific data
        ];
        let VendorEvent::L2CapCommandReject(event) =
            VendorEvent::new(&bytes).expect("typed command rejection")
        else {
            panic!("unexpected event variant");
        };
        assert_eq!(event.identifier, L2SignalIdentifier::new(0x07));
        assert_eq!(event.reason, L2CapRejectionReason::InvalidCid);
    }

    #[test]
    fn att_error_response_accepts_every_declared_core_error() {
        for (value, expected) in [
            (0x12, AttError::DatabaseOutOfSync),
            (0x13, AttError::ValueNotAllowed),
        ] {
            let bytes = [
                0x11, 0x0C, // event code
                0x23, 0x01, // connection handle
                0x0A, // read request
                0x34, 0x12, // attribute handle
                value,
            ];
            let VendorEvent::AttErrorResponse(event) =
                VendorEvent::new(&bytes).expect("declared ATT error")
            else {
                panic!("unexpected event variant");
            };
            assert_eq!(event.error, expected);
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
    fn ranged_counted_items_decode_semantic_l2cap_values() {
        let bytes = [
            0x12, 0x08, // event code
            0x23, 0x01, // connection handle
            0x40, 0x00, // MTU
            0x20, 0x00, // MPS
            0x01, // channel count
            0x07, // channel index
        ];
        let event = VendorEvent::new(&bytes).expect("valid ranged counted bytes");
        let VendorEvent::L2CapCocReconfig(event) = event else {
            panic!("unexpected event variant");
        };
        assert_eq!(event.mtu.value(), 64);
        assert_eq!(event.mps.value(), 32);
        let mut channels = event.channel_indices.iter();
        assert_eq!(channels.next(), Some(L2CocChannelIndex::new(0x07)));
        assert!(channels.next().is_none());
    }

    #[test]
    fn ranged_counted_items_enforce_their_declared_bounds() {
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
    fn semantic_l2cap_event_values_reject_invalid_controller_data() {
        let bytes = [
            0x12, 0x08, // event code
            0x23, 0x01, // connection handle
            0x16, 0x00, // MTU below the documented minimum
            0x20, 0x00, // MPS
            0x01, // channel count
            0x07, // channel index
        ];
        let error = VendorEvent::new(&bytes).expect_err("invalid MTU must be rejected");
        let Error::Vendor(VendorError::BadL2CocMtu(value)) = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(value.actual(), 22);
        assert_eq!(value.minimum(), 23);
        assert_eq!(value.maximum(), u16::MAX as u64);
    }

    #[test]
    fn empty_l2cap_data_rejects_a_nonzero_count() {
        let bytes = [
            0x01, 0x08, // event code
            0x23, 0x01, // connection handle
            0x01, // the timeout event requires an empty data list
        ];
        let error = VendorEvent::new(&bytes).expect_err("timeout data count must be zero");
        assert_eq!(error, Error::Vendor(VendorError::BadL2CapDataLength(1, 0)));
    }

    #[test]
    fn empty_l2cap_data_preserves_its_missing_count_diagnostic() {
        let bytes = [
            0x01, 0x08, // event code
            0x23, 0x01, // connection handle
        ];
        let error = VendorEvent::new(&bytes).expect_err("timeout data count is required");
        assert_eq!(error, Error::BadLength(0, 1));
    }

    #[test]
    fn read_multiple_count_is_a_count_of_handles() {
        let bytes = [0x15, 0x0C, 0x23, 0x01, 0x02, 0x34, 0x12, 0x78, 0x56];
        let event = VendorEvent::new(&bytes).expect("two counted handles");
        let VendorEvent::AttReadMultiplePermitRequest(event) = event else {
            panic!("unexpected event variant");
        };
        let mut handles = event.handles();
        assert_eq!(handles.next(), Some(AttributeHandle(0x1234)));
        assert_eq!(handles.next(), Some(AttributeHandle(0x5678)));
        assert!(handles.next().is_none());
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
    fn tagged_items_decode_both_find_information_formats() {
        let format16 = [
            0x04, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x01, 0x08, // 16-bit UUID format and byte length
            0x34, 0x12, 0x0D, 0x18, // first handle and UUID
            0x78, 0x56, 0x0F, 0x18, // second handle and UUID
        ];
        let VendorEvent::AttFindInformationResponse(event) =
            VendorEvent::new(&format16).expect("two 16-bit handle-UUID pairs")
        else {
            panic!("unexpected event variant");
        };
        let HandleUuidPairIterator::Format16(mut pairs) = event.handle_uuid_pair_iter() else {
            panic!("unexpected UUID format");
        };
        let first = pairs.next().expect("first pair");
        assert_eq!(first.handle, AttributeHandle(0x1234));
        assert_eq!(first.uuid, Uuid16(0x180D));
        let second = pairs.next().expect("second pair");
        assert_eq!(second.handle, AttributeHandle(0x5678));
        assert_eq!(second.uuid, Uuid16(0x180F));
        assert!(pairs.next().is_none());

        let format128 = [
            0x04, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x02, 0x12, // 128-bit UUID format and byte length
            0xBC, 0x9A, // handle
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // UUID
            0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        ];
        let VendorEvent::AttFindInformationResponse(event) =
            VendorEvent::new(&format128).expect("one 128-bit handle-UUID pair")
        else {
            panic!("unexpected event variant");
        };
        let HandleUuidPairIterator::Format128(mut pairs) = event.handle_uuid_pair_iter() else {
            panic!("unexpected UUID format");
        };
        let pair = pairs.next().expect("128-bit pair");
        assert_eq!(pair.handle, AttributeHandle(0x9ABC));
        assert_eq!(
            pair.uuid,
            Uuid128([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
        );
        assert!(pairs.next().is_none());
    }

    #[test]
    fn tagged_items_preserve_find_information_diagnostics() {
        let missing_length = [0x04, 0x0C, 0x23, 0x01, 0x01];
        assert_eq!(
            VendorEvent::new(&missing_length).unwrap_err(),
            Error::BadLength(1, 2)
        );

        let unknown_format = [0x04, 0x0C, 0x23, 0x01, 0x03, 0x00];
        assert_eq!(
            VendorEvent::new(&unknown_format).unwrap_err(),
            Error::Vendor(VendorError::BadAttFindInformationResponseFormat(0x03))
        );

        let partial16 = [0x04, 0x0C, 0x23, 0x01, 0x01, 0x03, 0x34, 0x12, 0x0D];
        assert_eq!(
            VendorEvent::new(&partial16).unwrap_err(),
            Error::Vendor(VendorError::AttFindInformationResponsePartialPair16)
        );

        let mut partial128 = [0; 6 + 17];
        partial128[..6].copy_from_slice(&[0x04, 0x0C, 0x23, 0x01, 0x02, 17]);
        assert_eq!(
            VendorEvent::new(&partial128).unwrap_err(),
            Error::Vendor(VendorError::AttFindInformationResponsePartialPair128)
        );
    }

    #[test]
    fn counted_items_decode_handle_information_records() {
        let bytes = [
            0x05, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x02, // pair count
            0x01, 0x00, 0x05, 0x00, // first attribute and group-end handles
            0x06, 0x00, 0x09, 0x00, // second attribute and group-end handles
        ];
        let VendorEvent::AttFindByTypeValueResponse(event) =
            VendorEvent::new(&bytes).expect("two handle-information records")
        else {
            panic!("unexpected event variant");
        };
        let mut pairs = event.handle_pairs_iter();
        let first = pairs.next().expect("first pair");
        assert_eq!(first.attribute, AttributeHandle(0x0001));
        assert_eq!(first.group_end, GroupEndHandle(0x0005));
        let second = pairs.next().expect("second pair");
        assert_eq!(second.attribute, AttributeHandle(0x0006));
        assert_eq!(second.group_end, GroupEndHandle(0x0009));
        assert!(pairs.next().is_none());
    }

    #[test]
    fn length_prefixed_records_decode_read_by_type_values() {
        let bytes = [
            0x06, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x04, 0x08, // record length and total byte length
            0x34, 0x12, 0xAA, 0xBB, // first handle-value record
            0x78, 0x56, 0xCC, 0xDD, // second handle-value record
        ];
        let VendorEvent::AttReadByTypeResponse(event) =
            VendorEvent::new(&bytes).expect("two handle-value records")
        else {
            panic!("unexpected event variant");
        };
        let mut pairs = event.handle_value_pair_iter();
        let first = pairs.next().expect("first pair");
        assert_eq!(first.handle, AttributeHandle(0x1234));
        assert_eq!(first.value, &[0xAA, 0xBB]);
        let second = pairs.next().expect("second pair");
        assert_eq!(second.handle, AttributeHandle(0x5678));
        assert_eq!(second.value, &[0xCC, 0xDD]);
        assert!(pairs.next().is_none());
    }

    #[test]
    fn length_prefixed_records_decode_read_by_group_values() {
        let bytes = [
            0x0A, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x06, 0x0C, // record length and total byte length
            0x01, 0x00, 0x05, 0x00, 0x0D, 0x18, // first group
            0x06, 0x00, 0x09, 0x00, 0x0F, 0x18, // second group
        ];
        let VendorEvent::AttReadByGroupTypeResponse(event) =
            VendorEvent::new(&bytes).expect("two attribute groups")
        else {
            panic!("unexpected event variant");
        };
        let mut groups = event.attribute_data_iter();
        let first = groups.next().expect("first group");
        assert_eq!(first.attribute_handle, AttributeHandle(0x0001));
        assert_eq!(first.attribute_end_handle, AttributeHandle(0x0005));
        assert_eq!(first.value, &[0x0D, 0x18]);
        assert_eq!(first.uuid(), Ok(AttributeUuid::Uuid16(Uuid16(0x180D))));
        let second = groups.next().expect("second group");
        assert_eq!(second.attribute_handle, AttributeHandle(0x0006));
        assert_eq!(second.attribute_end_handle, AttributeHandle(0x0009));
        assert_eq!(second.value, &[0x0F, 0x18]);
        assert_eq!(second.uuid(), Ok(AttributeUuid::Uuid16(Uuid16(0x180F))));
        assert!(groups.next().is_none());
    }

    #[test]
    fn short_att_records_expose_total_accessors() {
        let read_by_type = [
            0x06, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x02, 0x02, // handle-only record and total byte length
            0x34, 0x12, // attribute handle
        ];
        let VendorEvent::AttReadByTypeResponse(event) =
            VendorEvent::new(&read_by_type).expect("valid handle-only record")
        else {
            panic!("unexpected event variant");
        };
        let pair = event
            .handle_value_pair_iter()
            .next()
            .expect("one handle-value pair");
        assert_eq!(pair.handle, AttributeHandle(0x1234));
        assert!(pair.value.is_empty());

        let read_by_group = [
            0x0A, 0x0C, // event code
            0x23, 0x01, // connection handle
            0x04, 0x04, // handle-only group and total byte length
            0x01, 0x00, 0x05, 0x00, // start and end handles
        ];
        let VendorEvent::AttReadByGroupTypeResponse(event) =
            VendorEvent::new(&read_by_group).expect("valid handle-only group")
        else {
            panic!("unexpected event variant");
        };
        let group = event
            .attribute_data_iter()
            .next()
            .expect("one attribute group");
        assert_eq!(group.uuid().unwrap_err().actual_length(), 0);
    }

    #[test]
    fn attribute_group_uuid_accepts_the_full_128_bit_form() {
        let uuid = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let group = AttributeData {
            attribute_handle: AttributeHandle(1),
            attribute_end_handle: AttributeHandle(5),
            value: &uuid,
        };
        assert_eq!(group.uuid(), Ok(AttributeUuid::Uuid128(Uuid128(uuid))));
    }

    #[cfg(before_fw_0_22_0)]
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

    #[cfg(since_fw_0_22_0)]
    #[test]
    fn bond_lost_carries_its_connection_handle() {
        let VendorEvent::GapBondLost(event) = VendorEvent::new(&[0x05, 0x04, 0x23, 0x01]).unwrap()
        else {
            panic!("unexpected event variant");
        };
        assert_eq!(event.conn_handle, ConnHandle::new(0x0123));
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
    fn length_prefixed_records_preserve_their_length_diagnostics() {
        let missing_length = [0x06, 0x0C, 0x23, 0x01, 0x02];
        assert_eq!(
            VendorEvent::new(&missing_length).unwrap_err(),
            Error::BadLength(1, 2)
        );

        let oversized_length = [0x06, 0x0C, 0x23, 0x01, 0x02, 250];
        assert_eq!(
            VendorEvent::new(&oversized_length).unwrap_err(),
            Error::BadLength(2, 252)
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
