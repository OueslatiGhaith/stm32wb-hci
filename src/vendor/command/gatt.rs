//! GATT commands and types needed for those commands.

use core::ops::Range;

use crate::{
    BadStatusError, ConnectionHandle, Status,
    vendor::{command::BoundedBytes, event::AttributeHandle},
};
vendor_cmd! {
    GattInit(cgid = 0x2, cid = 0x01) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattAddService(cgid = 0x2, cid = 0x02) {
        Params<'a> = {
            uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
            service_type: u8 => 1,
            max_attribute_records: u8 => 1,
        };
        Completion = CommandComplete;
        Return = GattService {
            service_handle: AttributeHandle => 2,
        };
    }
}

vendor_cmd! {
    GattIncludeService(cgid = 0x2, cid = 0x03) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            include_handle_start: AttributeHandle => 2,
            include_handle_end: AttributeHandle => 2,
            include_uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandComplete;
        Return = GattIncludedService {
            service_handle: AttributeHandle => 2,
        };
    }
}

vendor_cmd! {
    GattAddCharacteristic(cgid = 0x2, cid = 0x04) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            characteristic_uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
            characteristic_value_len: u16 => 2,
            characteristic_properties: u8 => 1,
            security_permissions: u8 => 1,
            gatt_event_mask: u8 => 1,
            encryption_key_size: u8 => 1,
            is_variable: bool => 1,
        };
        Completion = CommandComplete;
        Return = GattCharacteristic {
            characteristic_handle: AttributeHandle => 2,
        };
    }
}

vendor_cmd! {
    GattAddCharacteristicDescriptor(cgid = 0x2, cid = 0x05) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            descriptor_uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
            descriptor_value_max_len: u8 => 1,
            descriptor_value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 227,
            },
            security_permissions: u8 => 1,
            access_permissions: u8 => 1,
            gatt_event_mask: u8 => 1,
            encryption_key_size: u8 => 1,
            is_variable: bool => 1,
        };
        Completion = CommandComplete;
        Return = GattCharacteristicDescriptor {
            descriptor_handle: AttributeHandle => 2,
        };
    }
}

vendor_cmd! {
    GattUpdateCharacteristicValue(cgid = 0x2, cid = 0x06) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            offset: u8 => 1,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 249,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattDeleteCharacterisitic(cgid = 0x2, cid = 0x07) {
        Params = {
            service: AttributeHandle => 2,
            characteristic: AttributeHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattDeleteService(cgid = 0x2, cid = 0x08) {
        Params = {
            service: AttributeHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattDeleteIncludedService(cgid = 0x2, cid = 0x09) {
        Params = {
            service: AttributeHandle => 2,
            included_service: AttributeHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattSetEventMask(cgid = 0x2, cid = 0x0A) {
        Params = {
            event_mask: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattExchageConfiguration(cgid = 0x2, cid = 0x0B) {
        Params = {
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattFindInformationRequest(cgid = 0x2, cid = 0x0C) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattFindByTypeValueRequest(cgid = 0x2, cid = 0x0D) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: u16 => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 246,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadByTypeRequest(cgid = 0x2, cid = 0x0E) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadByGroupTypeRequest(cgid = 0x2, cid = 0x0F) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattPrepareWriteRequest(cgid = 0x2, cid = 0x10) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattExecuteWriteRequest(cgid = 0x2, cid = 0x11) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            execute: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattDiscoverAllPrimaryServices(cgid = 0x2, cid = 0x12) {
        Params = {
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattDiscoverPrimaryServicesByUUID(cgid = 0x2, cid = 0x13) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattFindIncludedServices(cgid = 0x2, cid = 0x14) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            service_handle_start: AttributeHandle => 2,
            service_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattDiscoverAllCharacteristicsOfService(cgid = 0x2, cid = 0x15) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattDiscoverCharacteristicsByUUID(cgid = 0x2, cid = 0x16) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattDiscoverAllCharacteristicDescriptors(cgid = 0x2, cid = 0x17) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            characteristic_handle_start: AttributeHandle => 2,
            characteristic_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadCharacteristicValue(cgid = 0x2, cid = 0x18) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            characteristic_handle: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadCharacteristicUsingUUID(cgid = 0x2, cid = 0x19) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: payload,
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadLongCharacteristicValue(cgid = 0x2, cid = 0x1A) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            attribute: AttributeHandle => 2,
            offset: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadMultipleCharacteristicValues(cgid = 0x2, cid = 0x1B) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            handles: &'a [AttributeHandle] => {
                kind: counted_items,
                count: u8 => 1,
                item: AttributeHandle => 2,
                max_items: 126,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattWriteCharacteristicValue(cgid = 0x2, cid = 0x1C) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattWriteLongCharacteristicValue(cgid = 0x2, cid = 0x1D) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattWriteCharacteristicValueReliably(cgid = 0x2, cid = 0x1E) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattWriteLongCharacteristicDescriptor(cgid = 0x2, cid = 0x1F) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            descriptor_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadLongCharacteristicDescriptor(cgid = 0x2, cid = 0x20) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            attribute: AttributeHandle => 2,
            offset: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattWriteCharacteristicDescriptor(cgid = 0x2, cid = 0x21) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            descriptor_handle: AttributeHandle => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattReadCharacteristicDescriptor(cgid = 0x2, cid = 0x22) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            descriptor_handle: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GattWriteWithoutResponse(cgid = 0x2, cid = 0x23) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattSignedWriteWithoutResponse(cgid = 0x2, cid = 0x24) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattConfirmIndication(cgid = 0x2, cid = 0x25) {
        Params = {
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattWriteResponse(cgid = 0x2, cid = 0x26) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            attribute_handle: AttributeHandle => 2,
            write_status: u8 => 1,
            error_code: u8 => 1,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 248,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattAllowRead(cgid = 0x2, cid = 0x27) {
        Params = {
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattSetSecurityPermission(cgid = 0x2, cid = 0x28) {
        Params = {
            service_handle: AttributeHandle => 2,
            attribute_handle: AttributeHandle => 2,
            permission: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattSetDescriptorValue(cgid = 0x2, cid = 0x29) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            descriptor_handle: AttributeHandle => 2,
            offset: u16 => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 246,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattReadHandleValue(cgid = 0x2, cid = 0x2A) {
        Params = {
            handle: AttributeHandle => 2,
            offset: u16 => 2,
            value_length_requested: u16 => 2,
        };
        Completion = CommandComplete;
        Return = GattHandleValue {
            total_length: u16 => 2,
            value: BoundedBytes<249> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 249,
            },
        };
    }
}

impl GattHandleValue {
    /// Maximum number of value bytes that fit in the response envelope.
    pub const MAX_VALUE_LEN: usize = 249;

    /// Return the handle value bytes present in this response.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GattReadHandleValueOffset(cgid = 0x2, cid = 0x2B) {
        Params = {
            handle: AttributeHandle => 2,
            offset: u8 => 1,
        };
        Completion = CommandComplete;
        Return = GattHandleValueOffset {
            value: BoundedBytes<128> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 128,
            },
        };
    }
}

vendor_cmd! {
    GattUpdateLongCharacteristicValue(cgid = 0x2, cid = 0x2C) {
        Params<'a> = {
            conn_handle_to_notify: u16 => 2,
            service_handle: AttributeHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            update_type: u8 => 1,
            total_len: u16 => 2,
            offset: u16 => 2,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 243,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattDenyRead(cgid = 0x2, cid = 0x2D) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            error_code: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattSetAccessPermission(cgid = 0x2, cid = 0x2E) {
        Params = {
            service_handle: AttributeHandle => 2,
            attribute_handle: AttributeHandle => 2,
            permissions: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattStoreDatabase(cgid = 0x2, cid = 0x30) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattSendMultipleNotification(cgid = 0x2, cid = 0x31) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            handles: &'a [AttributeHandle] => {
                kind: counted_items,
                count: u8 => 1,
                item: AttributeHandle => 2,
                max_items: 126,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GattReadMultipleVarCharValue(cgid = 0x2, cid = 0x32) {
        Params<'a> = {
            conn_handle: ConnectionHandle => 2,
            handles: &'a [AttributeHandle] => {
                kind: counted_items,
                count: u8 => 1,
                item: AttributeHandle => 2,
                max_items: 126,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GattWriteWithoutRespExt(cgid = 0x2, cid = 0x40) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            attr_handle: u16 => 2,
            signed_mode: bool => 1,
            data_len: u16 => 2,
            data_pointer: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GattWriteWithRespExt(cgid = 0x2, cid = 0x41) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            attr_handle: u16 => 2,
            write_mode: u8 => 1,
            val_offset: u16 => 2,
            data_len: u16 => 2,
            data_pointer: u32 => 4,
        };
        Completion = CommandStatus;
    }
}

/// Potential errors from parameter validation.
///
/// Before some commands are sent to the controller, the parameters are validated. This type
/// enumerates the potential validation errors. Must be specialized on the types of communication
/// errors.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// For the [Add Characteristic Descriptor](GattAddCharacteristicDescriptor) command:
    /// the [descriptor value](AddDescriptorParameters::descriptor_value) is longer than the
    /// [maximum descriptor value length](AddDescriptorParameters::descriptor_value_max_len).
    DescriptorTooLong,

    /// For the [Add Characteristic Descriptor](GattAddCharacteristicDescriptor) command:
    /// the [descriptor value maximum length](AddDescriptorParameters::descriptor_value_max_len) is
    /// so large that the serialized structure may be more than 255 bytes. The maximum size is 227.
    DescriptorBufferTooLong,

    /// For the [Update Characteristir Value](GattUpdateCharacteristicValue) command: the
    /// length of the [characteristic value](UpdateCharacteristicValueParameters::value) is so large
    /// that the serialized structure would be more than 255 bytes. The maximum size is 249.
    ValueBufferTooLong,

    /// For the [Read Multiple Characteristic Values](GattReadMultipleCharacteristicValues)
    /// command: the number of [handles](MultipleCharacteristicReadParameters::handles) would cause
    /// the serialized command to be more than 255 bytes. The maximum length is 126 handles.
    TooManyHandlesToRead,

    /// Event Parsing Error
    ParseError(crate::event::Error),

    /// An error occurred during execution of the command
    HciError(Status),

    /// An error occurred during execution of the command
    UnknownHciError(u8),

    /// An internal error occurred during execution of the controller. This is a bug.
    IoError,
}

impl<T> From<bt_hci::cmd::Error<T>> for Error {
    fn from(err: bt_hci::cmd::Error<T>) -> Self {
        match err {
            bt_hci::cmd::Error::Io(_) => Self::IoError,
            bt_hci::cmd::Error::Hci(err) => match Status::try_from(err.to_status().into_inner()) {
                Ok(status) => Self::HciError(status),
                Err(BadStatusError::BadValue(status)) => Self::UnknownHciError(status),
            },
        }
    }
}

impl From<crate::event::Error> for Error {
    fn from(e: crate::event::Error) -> Self {
        Self::ParseError(e)
    }
}

/// Parameters for the [GATT Add Service](GattAddService) command.
pub struct AddServiceParameters {
    /// UUID of the service
    pub uuid: Uuid,

    /// Type of service
    pub service_type: ServiceType,

    /// The maximum number of attribute records that can be added to this service (including the
    /// service attribute, include attribute, characteristic attribute, characteristic value
    /// attribute and characteristic descriptor attribute).
    pub max_attribute_records: u8,
}

vendor_payload! {
    /// Types of UUID.
    pub enum Uuid {
        Tag = u8 => 1;

        /// 16-bit UUID.
        Uuid16(value: u16 => 2) = 0x01;

        /// 128-bit UUID.
        Uuid128(value: [u8; 16] => 16) = 0x02;
    }
}

/// Types of GATT services
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ServiceType {
    /// Primary service
    Primary = 0x01,
    /// Secondary service
    Secondary = 0x02,
}

/// Parameters for the [GATT Include Service](GattIncludeService) command.
pub struct IncludeServiceParameters {
    /// Handle of the service to which another service has to be included
    pub service_handle: AttributeHandle,

    /// Range of handles of the service which has to be included in the service.
    pub include_handle_range: Range<AttributeHandle>,

    /// UUID of the included service
    pub include_uuid: Uuid,
}

/// Parameters for the [GATT Add Characteristic](GattAddCharacteristic) command.
pub struct AddCharacteristicParameters {
    /// Handle of the service to which the characteristic has to be added
    pub service_handle: AttributeHandle,

    /// UUID of the characteristic
    pub characteristic_uuid: Uuid,

    /// Maximum length of the characteristic value
    pub characteristic_value_len: u16,

    /// Properties of the characteristic (defined in Volume 3, Part G, Section 3.3.3.1 of Bluetooth
    /// Specification 4.1)
    pub characteristic_properties: CharacteristicProperty,

    /// Security requirements of the characteristic
    pub security_permissions: CharacteristicPermission,

    /// Which types of events will be generated when the attribute is accessed.
    pub gatt_event_mask: CharacteristicEvent,

    /// The minimum encryption key size requirement for this attribute.
    pub encryption_key_size: EncryptionKeySize,

    /// If true, the attribute has a variable length value field. Otherwise, the value field length
    /// is fixed.
    pub is_variable: bool,
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Available [properties](AddCharacteristicParameters::characteristic_properties) for
    /// characteristics. Defined in Volume 3, Part G, Section 3.3.3.1 of Bluetooth Specification
    /// 4.1.
    pub struct CharacteristicProperty: u8 {
        /// If set, permits broadcasts of the Characteristic Value using Server Characteristic
        /// Configuration Descriptor. If set, the Server Characteristic Configuration Descriptor
        /// shall exist.
        const BROADCAST = 0x01;

        /// If set, permits reads of the Characteristic Value using procedures defined in Volume 3,
        /// Part G, Section 4.8 of the Bluetooth specification 4.1.
        const READ = 0x02;

        /// If set, permit writes of the Characteristic Value without response using procedures
        /// defined in Volume 3, Part G, Section 4.9.1 of the Bluetooth specification 4.1.
        const WRITE_WITHOUT_RESPONSE = 0x04;

        /// If set, permits writes of the Characteristic Value with response using procedures
        /// defined in Volume 3, Part Section 4.9.3 or Section 4.9.4 of the Bluetooth
        /// specification 4.1.
        const WRITE = 0x08;

        /// If set, permits notifications of a Characteristic Value without acknowledgement using
        /// the procedure defined in Volume 3, Part G, Section 4.10 of the Bluetooth specification
        /// 4.1. If set, the Client Characteristic Configuration Descriptor shall exist.
        const NOTIFY = 0x10;

        /// If set, permits indications of a Characteristic Value with acknowledgement using the
        /// procedure defined in Volume 3, Part G, Section 4.11 of the Bluetooth specification
        /// 4.1. If set, the Client Characteristic Configuration Descriptor shall exist.
        const INDICATE = 0x20;

        /// If set, permits signed writes to the Characteristic Value using the Signed Writes
        /// procedure defined in Volume 3, Part G, Section 4.9.2 of the Bluetooth specification
        /// 4.1.
        const AUTHENTICATED = 0x40;

        /// If set, additional characteristic properties are defined in the Characteristic Extended
        /// Properties Descriptor defined in Volume 3, Part G, Section 3.3.3.1 of the Bluetooth
        /// specification 4.1. If set, the Characteristic Extended Properties Descriptor shall
        /// exist.
        const EXTENDED_PROPERTIES = 0x80;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Available [properties](AddCharacteristicParameters::characteristic_properties) for
    /// characteristics. Defined in Volume 3, Part G, Section 3.3.3.1 of Bluetooth Specification
    /// 4.1.
    pub struct CharacteristicProperty: u8 {
        /// If set, permits broadcasts of the Characteristic Value using Server Characteristic
        /// Configuration Descriptor. If set, the Server Characteristic Configuration Descriptor
        /// shall exist.
        const BROADCAST = 0x01;

        /// If set, permits reads of the Characteristic Value using procedures defined in Volume 3,
        /// Part G, Section 4.8 of the Bluetooth specification 4.1.
        const READ = 0x02;

        /// If set, permit writes of the Characteristic Value without response using procedures
        /// defined in Volume 3, Part G, Section 4.9.1 of the Bluetooth specification 4.1.
        const WRITE_WITHOUT_RESPONSE = 0x04;

        /// If set, permits writes of the Characteristic Value with response using procedures
        /// defined in Volume 3, Part Section 4.9.3 or Section 4.9.4 of the Bluetooth
        /// specification 4.1.
        const WRITE = 0x08;

        /// If set, permits notifications of a Characteristic Value without acknowledgement using
        /// the procedure defined in Volume 3, Part G, Section 4.10 of the Bluetooth specification
        /// 4.1. If set, the Client Characteristic Configuration Descriptor shall exist.
        const NOTIFY = 0x10;

        /// If set, permits indications of a Characteristic Value with acknowledgement using the
        /// procedure defined in Volume 3, Part G, Section 4.11 of the Bluetooth specification
        /// 4.1. If set, the Client Characteristic Configuration Descriptor shall exist.
        const INDICATE = 0x20;

        /// If set, permits signed writes to the Characteristic Value using the Signed Writes
        /// procedure defined in Volume 3, Part G, Section 4.9.2 of the Bluetooth specification
        /// 4.1.
        const AUTHENTICATED = 0x40;

        /// If set, additional characteristic properties are defined in the Characteristic Extended
        /// Properties Descriptor defined in Volume 3, Part G, Section 3.3.3.1 of the Bluetooth
        /// specification 4.1. If set, the Characteristic Extended Properties Descriptor shall
        /// exist.
        const EXTENDED_PROPERTIES = 0x80;
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// [Permissions](AddCharacteristicParameter::security_permissions) available for
    /// characteristics.
    pub struct CharacteristicPermission: u8 {
        /// Need authentication to read.
        const AUTHENTICATED_READ = 0x01;

        /// Need authorization to read.
        const AUTHORIZED_READ = 0x02;

        /// Link should be encrypted to read.
        const ENCRYPTED_READ = 0x04;

        /// Need authentication to write.
        const AUTHENTICATED_WRITE = 0x08;

        /// Need authorization to write.
        const AUTHORIZED_WRITE = 0x10;

        /// Link should be encrypted for write.
        const ENCRYPTED_WRITE = 0x20;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// [Permissions](AddCharacteristicParameter::security_permissions) available for
    /// characteristics.
    pub struct CharacteristicPermission: u8 {
        /// Need authentication to read.
        const AUTHENTICATED_READ = 0x01;

        /// Need authorization to read.
        const AUTHORIZED_READ = 0x02;

        /// Link should be encrypted to read.
        const ENCRYPTED_READ = 0x04;

        /// Need authentication to write.
        const AUTHENTICATED_WRITE = 0x08;

        /// Need authorization to write.
        const AUTHORIZED_WRITE = 0x10;

        /// Link should be encrypted for write.
        const ENCRYPTED_WRITE = 0x20;
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Which events may be generated when a characteristic is accessed.
    pub struct CharacteristicEvent: u8 {
        /// The application will be notified when a client writes to this attribute.
        const ATTRIBUTE_WRITE = 0x01;

        /// The application will be notified when a write request/write command/signed write command
        /// is received by the server for this attribute.
        const CONFIRM_WRITE = 0x02;

        /// The application will be notified when a read request of any type is got for this
        /// attribute.
        const CONFIRM_READ = 0x04;

        #[cfg(any(only_fw_0_17_0, after_fw_0_17_0))]
        /// The application will be notified when a notification is complete.
        const NOTIFY_NOTIFICATION_COMPLETE = 0x08;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Which events may be generated when a characteristic is accessed.
    pub struct CharacteristicEvent: u8 {
        /// The application will be notified when a client writes to this attribute.
        const ATTRIBUTE_WRITE = 0x01;

        /// The application will be notified when a write request/write command/signed write command
        /// is received by the server for this attribute.
        const CONFIRM_WRITE = 0x02;

        /// The application will be notified when a read request of any type is got for this
        /// attribute.
        const CONFIRM_READ = 0x04;

        #[cfg(any(only_fw_0_17_0, after_fw_0_17_0))]
        /// The application will be notified when a notification is complete.
        const NOTIFY_NOTIFICATION_COMPLETE = 0x08;
    }
}

/// Encryption key size, in bytes.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EncryptionKeySize(u8);

impl EncryptionKeySize {
    /// Validate the size as a valid encryption key size. Valid range is 7 to 16, inclusive.
    ///
    /// # Errors
    ///
    /// - [TooShort](EncryptionKeySizeError::TooShort) if the provided size is less than 7.
    /// - [TooLong](EncryptionKeySizeError::TooLong) if the provided size is greater than 16.
    pub fn with_value(sz: usize) -> Result<Self, EncryptionKeySizeError> {
        const MIN: usize = 7;
        const MAX: usize = 16;

        if sz < MIN {
            return Err(EncryptionKeySizeError::TooShort);
        }

        if sz > MAX {
            return Err(EncryptionKeySizeError::TooLong);
        }

        Ok(Self(sz as u8))
    }

    /// Retrieve the key size.
    pub fn value(&self) -> usize {
        self.0 as usize
    }
}

/// Errors that can occur when creating an [`EncryptionKeySize`].
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum EncryptionKeySizeError {
    /// The provided size was less than the minimum allowed size.
    TooShort,
    /// The provided size was greater than the maximum allowed size.
    TooLong,
}

/// Parameters for the [GATT Add Characteristic Descriptor](GattAddCharacteristicDescriptor)
/// command.
pub struct AddDescriptorParameters<'a> {
    /// Handle of the service to which characteristic belongs.
    pub service_handle: AttributeHandle,

    /// Handle of the characteristic to which description is to be added.
    pub characteristic_handle: AttributeHandle,

    /// UUID of the characteristic descriptor.
    ///
    /// See [KnownDescriptor] for some useful descriptors. This value is not restricted to the known
    /// descriptors, however.
    pub descriptor_uuid: Uuid,

    /// The maximum length of the descriptor value.
    pub descriptor_value_max_len: usize,

    /// Current Length of the characteristic descriptor value.
    pub descriptor_value: &'a [u8],

    /// What security requirements must be met before the descriptor can be accessed.
    pub security_permissions: DescriptorPermission,

    /// What types of access are allowed for the descriptor.
    pub access_permissions: AccessPermission,

    /// Which types of events will be generated when the attribute is accessed.
    pub gatt_event_mask: CharacteristicEvent,

    /// The minimum encryption key size requirement for this attribute.
    pub encryption_key_size: EncryptionKeySize,

    /// If true, the attribute has a variable length value field. Otherwise, the value field length
    /// is fixed.
    pub is_variable: bool,
}

/// Common characteristic descriptor UUIDs.
#[repr(u16)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KnownDescriptor {
    /// Characteristic Extended Properties Descriptor
    CharacteristicExtendedProperties = 0x2900,
    /// Characteristic User Descriptor
    CharacteristicUser = 0x2901,
    /// Client configuration descriptor
    ClientConfiguration = 0x2902,
    /// Server configuration descriptor
    ServerConfiguration = 0x2903,
    /// Characteristic presentation format
    CharacteristicPresentationFormat = 0x2904,
    /// Characteristic aggregated format
    CharacteristicAggregatedFormat = 0x2905,
}

impl From<KnownDescriptor> for Uuid {
    fn from(value: KnownDescriptor) -> Self {
        Uuid::Uuid16(value as u16)
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Permissions available for characteristic descriptors.
    pub struct DescriptorPermission: u8 {
        /// Authentication required.
        const AUTHENTICATED = 0x01;

        /// Authorization required.
        const AUTHORIZED = 0x02;

        /// Encryption required.
        const ENCRYPTED = 0x04;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Permissions available for characteristic descriptors.
    pub struct DescriptorPermission: u8 {
        /// Authentication required.
        const AUTHENTICATED = 0x01;

        /// Authorization required.
        const AUTHORIZED = 0x02;

        /// Encryption required.
        const ENCRYPTED = 0x04;
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Types of access for characteristic descriptors
    pub struct AccessPermission: u8 {
        /// Readable
        const READ = 0x01;
        /// Writable
        const WRITE = 0x02;
        /// Readable and writeable
        const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();
        /// Writeable without response
        const WRITE_NO_RESP = 0x04;
        /// Signed writeable
        const SIGNED_WRITE = 0x08;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Types of access for characteristic descriptors
    pub struct AccessPermission: u8 {
        /// Readable
        const READ = 0x01;
        /// Writable
        const WRITE = 0x02;
        /// Readable and writeable
        const READ_WRITE = Self::READ.bits() | Self::WRITE.bits();
        /// Writeable without responseconst
        const WRITE_NO_RESP = 0x04;
        /// Signed writeable
        const SIGNED_WRITE = 0x08;
    }
}

/// Parameters for the [Update Characteristic Value](GattUpdateCharacteristicValue)
/// command.
pub struct UpdateCharacteristicValueParameters<'a> {
    /// Handle of the service to which characteristic belongs.
    pub service_handle: AttributeHandle,

    /// Handle of the characteristic.
    pub characteristic_handle: AttributeHandle,

    /// The offset from which the attribute value has to be updated. If this is set to 0, and the
    /// attribute value is of [variable length](AddCharacteristicParameters::is_variable), then the
    /// length of the attribute will be set to the length of
    /// [value](UpdateCharacteristicValueParameters::value). If the offset is set to a value greater
    /// than 0, then the length of the attribute will be set to the
    /// [maximum length](AddCharacteristicParameters::characteristic_value_len) as specified for the
    /// attribute while adding the characteristic.
    pub offset: usize,

    /// The new characteristic value.
    pub value: &'a [u8],
}

/// Parameters for the [GATT Delete Included Service](GattDeleteIncludedService) command.
pub struct DeleteIncludedServiceParameters {
    /// Handle of the service to which Include definition belongs
    pub service: AttributeHandle,

    /// Handle of the Included definition to be deleted.
    pub included_service: AttributeHandle,
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Flags for individual events that can be masked by the
    /// [GATT Set Event Mask](GattSetEventMask) command.
    pub struct Event: u32 {
        /// [GATT Attribute Modified](crate::vendor::event::VendorEvent::GattAttributeModified).
        const ATTRIBUTE_MODIFIED = 0x0000_0001;
        /// [GATT Procedure Timeout](crate::vendor::event::VendorEvent::GattProcedureTimeout).
        const PROCEDURE_TIMEOUT = 0x0000_0002;
        /// [ATT Exchange MTU Response](crate::vendor::event::VendorEvent::AttExchangeMtuResponse).
        const EXCHANGE_MTU_RESPONSE = 0x0000_0004;
        /// [ATT Find Information Response](crate::vendor::event::VendorEvent::AttFindInformationResponse).
        const FIND_INFORMATION_RESPONSE = 0x0000_0008;
        /// [ATT Find By Type Value Response](crate::vendor::event::VendorEvent::AttFindByTypeValueResponse).
        const FIND_BY_TYPE_VALUE_RESPONSE = 0x0000_0010;
        /// [ATT Find By Type Response](crate::vendor::event::VendorEvent::AttFindByTypeResponse).
        const READ_BY_TYPE_RESPONSE = 0x0000_0020;
        /// [ATT Read Response](crate::vendor::event::VendorEvent::AttReadResponse).
        const READ_RESPONSE = 0x0000_0040;
        /// [ATT Read Blob Response](crate::vendor::event::VendorEvent::AttReadBlobResponse).
        const READ_BLOB_RESPONSE = 0x0000_0080;
        /// [ATT Read Multiple Response](crate::vendor::event::VendorEvent::AttReadMultipleResponse).
        const READ_MULTIPLE_RESPONSE = 0x0000_0100;
        /// [ATT Read By Group](crate::vendor::event::VendorEvent::AttReadByGroupTypeResponse).
        const READ_BY_GROUP_RESPONSE = 0x0000_0200;
        /// [ATT Prepare Write Response](crate::vendor::event::VendorEvent::AttPrepareWriteResponse).
        const PREPARE_WRITE_RESPONSE = 0x0000_0800;
        /// [ATT Execute Write Response](crate::vendor::event::VendorEvent::AttExecuteWriteResponse).
        const EXECUTE_WRITE_RESPONSE = 0x0000_1000;
        /// [GATT Indication](crate::vendor::event::VendorEvent::GattIndication).
        const INDICATION = 0x0000_2000;
        /// [GATT Notification](crate::vendor::event::VendorEvent::GattNotification).
        const NOTIFICATION = 0x0000_4000;
        /// [GATT Error Response](crate::vendor::event::VendorEvent::AttErrorResponse).
        const ERROR_RESPONSE = 0x0000_8000;
        /// [GATT Procedure Complete](crate::vendor::event::VendorEvent::GattProcedureComplete).
        const PROCEDURE_COMPLETE = 0x0001_0000;
        /// [GATT Discover Characteristic by UUID or Read Using Characteristic UUID](crate::vendor::event::VendorEvent::GattDiscoverOrReadCharacteristicByUuidResponse).
        const DISCOVER_OR_READ_CHARACTERISTIC_BY_UUID_RESPONSE = 0x0002_0000;
        /// [GATT Tx Pool Available](crate::vendor::event::VendorEvent::GattTxPoolAvailable)
        const TX_POOL_AVAILABLE = 0x0004_0000;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Flags for individual events that can be masked by the [GATT Set Event Mask](GattSetEventMask) command.
    pub struct Event: u32 {
        /// [GATT Attribute Modified](crate::vendor::event::VendorEvent::GattAttributeModified).
        const ATTRIBUTE_MODIFIED = 0x0000_0001;
        /// [GATT Procedure Timeout](crate::vendor::event::VendorEvent::GattProcedureTimeout).
        const PROCEDURE_TIMEOUT = 0x0000_0002;
        /// [ATT Exchange MTU Response](crate::vendor::event::VendorEvent::AttExchangeMtuResponse).
        const EXCHANGE_MTU_RESPONSE = 0x0000_0004;
        /// [ATT Find Information Response](crate::vendor::event::VendorEvent::AttFindInformationResponse).
        const FIND_INFORMATION_RESPONSE = 0x0000_0008;
        /// [ATT Find By Type Value Response](crate::vendor::event::VendorEvent::AttFindByTypeValueResponse).
        const FIND_BY_TYPE_VALUE_RESPONSE = 0x0000_0010;
        /// [ATT Find By Type Response](crate::vendor::event::VendorEvent::AttFindByTypeResponse).
        const READ_BY_TYPE_RESPONSE = 0x0000_0020;
        /// [ATT Read Response](crate::vendor::event::VendorEvent::AttReadResponse).
        const READ_RESPONSE = 0x0000_0040;
        /// [ATT Read Blob Response](crate::vendor::event::VendorEvent::AttReadBlobResponse).
        const READ_BLOB_RESPONSE = 0x0000_0080;
        /// [ATT Read Multiple Response](crate::vendor::event::VendorEvent::AttReadMultipleResponse).
        const READ_MULTIPLE_RESPONSE = 0x0000_0100;
        /// [ATT Read By Group](crate::vendor::event::VendorEvent::AttReadByGroupTypeResponse).
        const READ_BY_GROUP_RESPONSE = 0x0000_0200;
        /// [ATT Prepare Write Response](crate::vendor::event::VendorEvent::AttPrepareWriteResponse).
        const PREPARE_WRITE_RESPONSE = 0x0000_0800;
        /// [ATT Execute Write Response](crate::vendor::event::VendorEvent::AttExecuteWriteResponse).
        const EXECUTE_WRITE_RESPONSE = 0x0000_1000;
        /// [GATT Indication](crate::vendor::event::VendorEvent::GattIndication).
        const INDICATION = 0x0000_2000;
        /// [GATT Notification](crate::vendor::event::VendorEvent::GattNotification).
        const NOTIFICATION = 0x0000_4000;
        /// [GATT Error Response](crate::vendor::event::VendorEvent::AttErrorResponse).
        const ERROR_RESPONSE = 0x0000_8000;
        /// [GATT Procedure Complete](crate::vendor::event::VendorEvent::GattProcedureComplete).
        const PROCEDURE_COMPLETE = 0x0001_0000;
        /// [GATT Discover Characteristic by UUID or Read Using Characteristic UUID](crate::vendor::event::VendorEvent::GattDiscoverOrReadCharacteristicByUuidResponse).
        const DISCOVER_OR_READ_CHARACTERISTIC_BY_UUID_RESPONSE = 0x0002_0000;
        /// [GATT Tx Pool Available](crate::vendor::event::VendorEvent::GattTxPoolAvailable)
        const TX_POOL_AVAILABLE = 0x0004_0000;
    }
}

/// Parameters for the [GATT Find by Type Value Request](GattFindByTypeValueRequest)
/// command.
pub struct FindByTypeValueParameters<'a> {
    /// Connection handle for which the command is given.
    pub conn_handle: crate::ConnectionHandle,

    /// Range of attributes to be discovered on the server.
    pub attribute_handle_range: Range<AttributeHandle>,

    /// UUID to find.
    pub uuid: Uuid16,

    /// Attribute value to find.
    ///
    /// Note: Though the max attribute value that is allowed according to the spec is 512 octets,
    /// due to the limitation of the transport layer (command packet max length is 255 bytes) the
    /// value is limited to 246 bytes.
    pub value: &'a [u8],
}

/// 16-bit UUID
pub struct Uuid16(pub u16);

/// Parameters for the [Read by Group Type Request](GattReadByGroupTypeRequest) command.
pub struct ReadByTypeParameters {
    /// Connection handle for which the command is given.
    pub conn_handle: crate::ConnectionHandle,

    /// Range of values to be read on the server.
    pub attribute_handle_range: Range<AttributeHandle>,

    /// UUID of the attribute.
    pub uuid: Uuid,
}

/// Parameters for the [Prepare Write Request](GattPrepareWriteRequest) command.
pub struct WriteRequest<'a> {
    /// Connection handle for which the command is given.
    pub conn_handle: crate::ConnectionHandle,

    /// Handle of the attribute whose value has to be written
    pub attribute_handle: AttributeHandle,

    /// The offset at which value has to be written
    pub offset: usize,

    /// Value of the attribute to be written
    pub value: &'a [u8],
}

/// Parameters for the [Read long characteristic value](GattReadLongCharacteristicValue)
/// command.
pub struct LongCharacteristicReadParameters {
    /// Connection handle for which the command is given.
    pub conn_handle: crate::ConnectionHandle,

    /// Handle of the characteristic to be read
    pub attribute: AttributeHandle,

    /// Offset from which the value needs to be read.
    pub offset: usize,
}

/// Parameters for the [Read Multiple Characteristic Values](GattReadMultipleCharacteristicValues)
/// command.
pub struct MultipleCharacteristicReadParameters<'a> {
    /// Connection handle for which the command is given.
    pub conn_handle: crate::ConnectionHandle,

    /// The handles for which the attribute value has to be read.
    ///
    /// The maximum length is 126 handles.
    pub handles: &'a [AttributeHandle],
}

/// Parameters for the [Write Characteristic Value](GattWriteCharacteristicValue) command.
pub struct CharacteristicValue<'a> {
    /// Connection handle for which the command is given.
    pub conn_handle: crate::ConnectionHandle,

    /// Handle of the characteristic to be written.
    pub characteristic_handle: AttributeHandle,

    /// Value to be written. The maximum length is 250 bytes.
    pub value: &'a [u8],
}

/// Parameters for the [Write Long Characteristic Value](GattWriteLongCharacteristicValue)
/// command.
pub struct LongCharacteristicValue<'a> {
    /// Connection handle for which the command is given.
    pub conn_handle: crate::ConnectionHandle,

    /// Handle of the characteristic to be written.
    pub characteristic_handle: AttributeHandle,

    /// Offset at which the attribute has to be written.
    pub offset: usize,

    /// Value to be written. The maximum length is 248 bytes.
    pub value: &'a [u8],
}

/// Parameters for the [Write Response](GattWriteResponse) command.
pub struct WriteResponseParameters<'a> {
    /// Connection handle for which the command is given
    pub conn_handle: crate::ConnectionHandle,

    /// Handle of the attribute that was passed in the
    /// [Write Permit Request](crate::vendor::event::VendorEvent::AttWritePermitRequest) event.
    pub attribute_handle: AttributeHandle,

    /// Is the command rejected, and if so, why?
    pub status: Result<(), crate::Status>,

    /// Value as passed in the
    /// [Write Permit Request](crate::vendor::event::VendorEvent::AttWritePermitRequest) event.
    pub value: &'a [u8],
}

/// Parameters for the [Set Security Permission](GattSetSecurityPermission) command.
pub struct SecurityPermissionParameters {
    /// Handle of the service which contains the attribute whose security permission has to be
    /// modified.
    pub service_handle: AttributeHandle,

    /// Handle of the attribute whose security permission has to be modified.
    pub attribute_handle: AttributeHandle,

    /// Security requirements for the attribute.
    pub permission: CharacteristicPermission,
}

/// Parameters for the [Set Descriptor Value](GattSetDescriptorValue) command.
pub struct DescriptorValueParameters<'a> {
    /// Handle of the service which contains the descriptor.
    pub service_handle: AttributeHandle,

    /// Handle of the characteristic which contains the descriptor.
    pub characteristic_handle: AttributeHandle,

    /// Handle of the descriptor whose value has to be set.
    pub descriptor_handle: AttributeHandle,

    /// Offset from which the descriptor value has to be updated.
    pub offset: usize,

    /// Descriptor value
    pub value: &'a [u8],
}

/// Parameters for the
/// [Update Long Characteristic Value](GattUpdateLongCharacteristicValue) command.
pub struct UpdateCharacteristicValueExt<'a> {
    /// Specifies the client(s) to be notified
    pub conn_handle_to_notify: ConnectionHandleToNotify,

    /// Handle of the service to which characteristic belongs.
    pub service_handle: AttributeHandle,

    /// Handle of the characteristic.
    pub characteristic_handle: AttributeHandle,

    /// Controls whether an indication, notification, both, or neither is generated by the attribute
    /// update.
    pub update_type: UpdateType,

    /// Total length of the Attribute value after the update. In case of a
    /// [variable size](AddCharacteristicParameters::is_variable) characteristic, this field specifies the new
    /// length of the characteristic value after the update; in case of fixed length characteristic
    /// this field is ignored.
    pub total_len: usize,

    /// The offset from which the Attribute value has to be updated.
    pub offset: usize,

    /// Updated value of the characteristic.
    pub value: &'a [u8],
}

#[derive(Clone, Copy)]
pub enum ConnectionHandleToNotify {
    /// Notify all subscribed clients on their unenhanced ATT bearer
    NotifyAll,
    /// Notify one client on the specified unenhanced ATT bearer (the parameter us the
    /// connection handle) (0x0001 .. 0x0EFF)
    NotifyOneUnenhanced(ConnectionHandle),
    /// Notfiy one client on the specified enhanced ATT bearer (the LST-byte of the
    /// parameter is the connection-oriented channel index) (0xEA00 .. 0xEA1F)
    NotifyOneEnhanced(ConnectionHandle),
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Flags for types of updates that the controller should signal when a characteristic value is
    /// [updated](GattUpdateLongCharacteristicValue).
    pub struct UpdateType: u8 {
        /// A notification can be sent if enabled in the client characteristic configuration
        /// descriptor.
        const NOTIFICATION = 0x01;
        /// An indication can be sent if enabled in the client characteristic configuration
        /// descriptor.
        const INDICATION = 0x02;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Flags for types of updates that the controller should signal when a characteristic value is
    /// [updated](GattUpdateLongCharacteristicValue).
    pub struct UpdateType: u8 {
        /// A notification can be sent if enabled in the client characteristic configuration
        /// descriptor.
        const NOTIFICATION = 0x01;
        /// An indication can be sent if enabled in the client characteristic configuration
        /// descriptor.
        const INDICATION = 0x02;
    }
}
