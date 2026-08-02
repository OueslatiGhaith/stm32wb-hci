//! GATT commands and types needed for those commands.

use bt_hci::param::ConnHandle;

use crate::types::AttributeHandle;
use crate::vendor::command::BoundedBytes;
use crate::vendor::command::l2cap::L2CocChannelIndex;

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Number of attribute records reserved for a service.
    ///
    /// CubeWB leaves this as an application-selected byte capacity and does
    /// not publish a narrower intrinsic domain.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct GattAttributeRecordCapacity: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Maximum or resulting length of a Bluetooth attribute value.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct GattAttributeValueLength: u16 => 2 {
        minimum: 0,
        maximum: 512,
    }
}

impl From<GattAttributeValueLength> for usize {
    fn from(value: GattAttributeValueLength) -> Self {
        usize::from(value.value())
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Eight-bit offset into a local attribute value.
    ///
    /// Validity depends on the characteristic selected by the same command.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct GattAttributeOffset8: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Offset into an attribute value.
    ///
    /// CubeWB defines the complete wire width; the selected local or remote
    /// attribute supplies the context-dependent upper bound.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct GattAttributeOffset: u16 => 2;
}

impl From<GattAttributeOffset> for usize {
    fn from(value: GattAttributeOffset) -> Self {
        usize::from(value.value())
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Maximum number of local attribute bytes requested from the controller.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct GattRequestedValueLength: u16 => 2;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Raw 16-bit UUID used by the ATT Find By Type Value procedure.
    ///
    /// The Bluetooth Assigned Numbers registry is open, so every bit pattern
    /// remains representable.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct GattUuid16: u16 => 2;
}

/// Client selection for [`GattUpdateLongCharacteristicValue`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GattNotificationTarget(u16);

impl GattNotificationTarget {
    /// Notify all subscribed clients on their unenhanced ATT bearers.
    pub const ALL_UNENHANCED: Self = Self(0x0000);

    /// Construct a target from its documented disjoint wire domain.
    pub const fn try_new(value: u16) -> Result<Self, InvalidGattNotificationTarget> {
        if value == 0x0000
            || (value >= 0x0001 && value <= 0x0EFF)
            || (value >= 0xEA00 && value <= 0xEA3F)
        {
            Ok(Self(value))
        } else {
            Err(InvalidGattNotificationTarget { actual: value })
        }
    }

    /// Select one client on an unenhanced ATT bearer.
    pub const fn for_connection(handle: ConnHandle) -> Result<Self, InvalidGattNotificationTarget> {
        Self::try_new(handle.0)
    }

    /// Select one client on an enhanced ATT bearer.
    pub const fn for_enhanced_channel(
        index: L2CocChannelIndex,
    ) -> Result<Self, InvalidGattNotificationTarget> {
        Self::try_new(0xEA00 | index.value() as u16)
    }

    /// Return the encoded client selector.
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// Value outside the client-selector domain documented by CubeWB.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct InvalidGattNotificationTarget {
    actual: u16,
}

impl InvalidGattNotificationTarget {
    /// Return the rejected wire value.
    pub const fn actual(self) -> u16 {
        self.actual
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    GattNotificationTarget => 2 {
        Fields = { value: u16, };
        Encode = |target| { (target.value(),) };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Maximum characteristic-descriptor value length accepted by
    /// [`GattAddCharacteristicDescriptor`].
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct DescriptorValueMaxLength: u8 => 1 {
        minimum: 0,
        maximum: 227,
    }
}

impl From<DescriptorValueMaxLength> for usize {
    fn from(value: DescriptorValueMaxLength) -> Self {
        usize::from(value.value())
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattInit(cgid = 0x2, cid = 0x01) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattAddService(cgid = 0x2, cid = 0x02) {
        Params<'a> = {
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
            service_type: ServiceType,
            max_attribute_records: GattAttributeRecordCapacity,
        };
        Completion = CommandComplete;
        Return = GattService {
            service_handle: AttributeHandle,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattIncludeService(cgid = 0x2, cid = 0x03) {
        Params<'a> = {
            service_handle: AttributeHandle,
            include_handle_start: AttributeHandle,
            include_handle_end: AttributeHandle,
            include_uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandComplete;
        Return = GattIncludedService {
            service_handle: AttributeHandle,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattAddCharacteristic(cgid = 0x2, cid = 0x04) {
        Params<'a> = {
            service_handle: AttributeHandle,
            characteristic_uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
            characteristic_value_len: GattAttributeValueLength,
            characteristic_properties: CharacteristicProperty,
            security_permissions: CharacteristicPermission,
            gatt_event_mask: CharacteristicEvent,
            encryption_key_size: EncryptionKeySize,
            is_variable: bool,
        };
        Completion = CommandComplete;
        Return = GattCharacteristic {
            characteristic_handle: AttributeHandle,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattAddCharacteristicDescriptor(cgid = 0x2, cid = 0x05) {
        Params<'a> = {
            service_handle: AttributeHandle,
            characteristic_handle: AttributeHandle,
            descriptor_uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
            descriptor_value_max_len: DescriptorValueMaxLength,
            descriptor_value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 227,
            },
            security_permissions: DescriptorPermission,
            access_permissions: AccessPermission,
            gatt_event_mask: CharacteristicEvent,
            encryption_key_size: EncryptionKeySize,
            is_variable: bool,
        };
        Constraints = {
            len_at_most(descriptor_value, descriptor_value_max_len);
        };
        Completion = CommandComplete;
        Return = GattCharacteristicDescriptor {
            descriptor_handle: AttributeHandle,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattUpdateCharacteristicValue(cgid = 0x2, cid = 0x06) {
        Params<'a> = {
            service_handle: AttributeHandle,
            characteristic_handle: AttributeHandle,
            offset: GattAttributeOffset8,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 249,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDeleteCharacterisitic(cgid = 0x2, cid = 0x07) {
        Params = {
            service: AttributeHandle,
            characteristic: AttributeHandle,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDeleteService(cgid = 0x2, cid = 0x08) {
        Params = {
            service: AttributeHandle,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDeleteIncludedService(cgid = 0x2, cid = 0x09) {
        Params = {
            service: AttributeHandle,
            included_service: AttributeHandle,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSetEventMask(cgid = 0x2, cid = 0x0A) {
        Params = {
            event_mask: Event,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattExchageConfiguration(cgid = 0x2, cid = 0x0B) {
        Params = {
            conn_handle: ConnHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattFindInformationRequest(cgid = 0x2, cid = 0x0C) {
        Params = {
            conn_handle: ConnHandle,
            attribute_handle_start: AttributeHandle,
            attribute_handle_end: AttributeHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattFindByTypeValueRequest(cgid = 0x2, cid = 0x0D) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle_start: AttributeHandle,
            attribute_handle_end: AttributeHandle,
            uuid: GattUuid16,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 246,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadByTypeRequest(cgid = 0x2, cid = 0x0E) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle_start: AttributeHandle,
            attribute_handle_end: AttributeHandle,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadByGroupTypeRequest(cgid = 0x2, cid = 0x0F) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle_start: AttributeHandle,
            attribute_handle_end: AttributeHandle,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattPrepareWriteRequest(cgid = 0x2, cid = 0x10) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle: AttributeHandle,
            offset: GattAttributeOffset,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattExecuteWriteRequest(cgid = 0x2, cid = 0x11) {
        Params = {
            conn_handle: ConnHandle,
            execute: bool,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverAllPrimaryServices(cgid = 0x2, cid = 0x12) {
        Params = {
            conn_handle: ConnHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverPrimaryServicesByUUID(cgid = 0x2, cid = 0x13) {
        Params<'a> = {
            conn_handle: ConnHandle,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattFindIncludedServices(cgid = 0x2, cid = 0x14) {
        Params = {
            conn_handle: ConnHandle,
            service_handle_start: AttributeHandle,
            service_handle_end: AttributeHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverAllCharacteristicsOfService(cgid = 0x2, cid = 0x15) {
        Params = {
            conn_handle: ConnHandle,
            attribute_handle_start: AttributeHandle,
            attribute_handle_end: AttributeHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverCharacteristicsByUUID(cgid = 0x2, cid = 0x16) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle_start: AttributeHandle,
            attribute_handle_end: AttributeHandle,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverAllCharacteristicDescriptors(cgid = 0x2, cid = 0x17) {
        Params = {
            conn_handle: ConnHandle,
            characteristic_handle_start: AttributeHandle,
            characteristic_handle_end: AttributeHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadCharacteristicValue(cgid = 0x2, cid = 0x18) {
        Params = {
            conn_handle: ConnHandle,
            characteristic_handle: AttributeHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadCharacteristicUsingUUID(cgid = 0x2, cid = 0x19) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle_start: AttributeHandle,
            attribute_handle_end: AttributeHandle,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16], },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadLongCharacteristicValue(cgid = 0x2, cid = 0x1A) {
        Params = {
            conn_handle: ConnHandle,
            attribute: AttributeHandle,
            offset: GattAttributeOffset,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadMultipleCharacteristicValues(cgid = 0x2, cid = 0x1B) {
        Params<'a> = {
            conn_handle: ConnHandle,
            handles: &'a [AttributeHandle] => {
                kind: counted_items,
                count: u8,
                item: AttributeHandle,
                max_items: 126,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattWriteCharacteristicValue(cgid = 0x2, cid = 0x1C) {
        Params<'a> = {
            conn_handle: ConnHandle,
            characteristic_handle: AttributeHandle,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 250,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattWriteLongCharacteristicValue(cgid = 0x2, cid = 0x1D) {
        Params<'a> = {
            conn_handle: ConnHandle,
            characteristic_handle: AttributeHandle,
            offset: GattAttributeOffset,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattWriteCharacteristicValueReliably(cgid = 0x2, cid = 0x1E) {
        Params<'a> = {
            conn_handle: ConnHandle,
            characteristic_handle: AttributeHandle,
            offset: GattAttributeOffset,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(before_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattWriteLongCharacteristicDescriptor(cgid = 0x2, cid = 0x1F) {
        Params<'a> = {
            conn_handle: ConnHandle,
            descriptor_handle: AttributeHandle,
            offset: GattAttributeOffset,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 248,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(before_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattReadLongCharacteristicDescriptor(cgid = 0x2, cid = 0x20) {
        Params = {
            conn_handle: ConnHandle,
            attribute: AttributeHandle,
            offset: GattAttributeOffset,
        };
        Completion = CommandStatus;
    }
}

#[cfg(before_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattWriteCharacteristicDescriptor(cgid = 0x2, cid = 0x21) {
        Params<'a> = {
            conn_handle: ConnHandle,
            descriptor_handle: AttributeHandle,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 250,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(before_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattReadCharacteristicDescriptor(cgid = 0x2, cid = 0x22) {
        Params = {
            conn_handle: ConnHandle,
            descriptor_handle: AttributeHandle,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattWriteWithoutResponse(cgid = 0x2, cid = 0x23) {
        Params<'a> = {
            conn_handle: ConnHandle,
            characteristic_handle: AttributeHandle,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 250,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSignedWriteWithoutResponse(cgid = 0x2, cid = 0x24) {
        Params<'a> = {
            conn_handle: ConnHandle,
            characteristic_handle: AttributeHandle,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 250,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattConfirmIndication(cgid = 0x2, cid = 0x25) {
        Params = {
            conn_handle: ConnHandle,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(before_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattWriteResponse(cgid = 0x2, cid = 0x26) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle: AttributeHandle,
            write_status: WriteStatus,
            error_code: u8,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 248,
            },
        };
        Constraints = {
            implies_eq(write_status, WriteStatus::Allowed, error_code, 0);
            implies_range(write_status, WriteStatus::Rejected, error_code, 1, u8::MAX);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattPermitWrite(cgid = 0x2, cid = 0x26) {
        Params<'a> = {
            conn_handle: ConnHandle,
            attribute_handle: AttributeHandle,
            write_status: WriteStatus,
            error_code: u8,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 248,
            },
        };
        Constraints = {
            implies_eq(write_status, WriteStatus::Allowed, error_code, 0);
            implies_one_of_or_range(
                write_status,
                WriteStatus::Rejected,
                error_code,
                [0x08],
                0x80,
                0x9F
            );
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(before_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattAllowRead(cgid = 0x2, cid = 0x27) {
        Params = {
            conn_handle: ConnHandle,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattPermitRead(cgid = 0x2, cid = 0x27) {
        Params = {
            conn_handle: ConnHandle,
            read_status: ReadStatus,
            error_code: u8,
            attribute_handle: AttributeHandle,
        };
        Constraints = {
            implies_eq(read_status, ReadStatus::Allowed, error_code, 0);
            implies_eq(
                read_status,
                ReadStatus::Allowed,
                attribute_handle,
                AttributeHandle(0)
            );
            implies_one_of_or_range(
                read_status,
                ReadStatus::Rejected,
                error_code,
                [0x08],
                0x80,
                0x9F
            );
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSetSecurityPermission(cgid = 0x2, cid = 0x28) {
        Params = {
            service_handle: AttributeHandle,
            attribute_handle: AttributeHandle,
            permission: CharacteristicPermission,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSetDescriptorValue(cgid = 0x2, cid = 0x29) {
        Params<'a> = {
            service_handle: AttributeHandle,
            characteristic_handle: AttributeHandle,
            descriptor_handle: AttributeHandle,
            offset: GattAttributeOffset,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 246,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadHandleValue(cgid = 0x2, cid = 0x2A) {
        Params = {
            handle: AttributeHandle,
            offset: GattAttributeOffset,
            value_length_requested: GattRequestedValueLength,
        };
        Completion = CommandComplete;
        Return = GattHandleValue {
            total_length: u16,
            value: BoundedBytes<247> => {
                kind: counted_bytes,
                count: u16,
                max_len: 247,
            },
        };
    }
}

impl GattHandleValue {
    /// Maximum number of value bytes that fit in the response envelope.
    pub const MAX_VALUE_LEN: usize = 247;

    /// Return the handle value bytes present in this response.
    pub fn value(&self) -> &[u8] {
        self.value.as_slice()
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattUpdateLongCharacteristicValue(cgid = 0x2, cid = 0x2C) {
        Params<'a> = {
            conn_handle_to_notify: GattNotificationTarget,
            service_handle: AttributeHandle,
            characteristic_handle: AttributeHandle,
            update_type: UpdateType,
            total_len: GattAttributeValueLength,
            offset: GattAttributeOffset,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 243,
            },
        };
        Constraints = {
            offset_len_at_most(offset, value, total_len);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(before_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattDenyRead(cgid = 0x2, cid = 0x2D) {
        Params = {
            conn_handle: ConnHandle,
            error_code: u8,
        };
        Constraints = {
            one_of_or_range(error_code, [0x08], 0x80, 0x9F);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSetAccessPermission(cgid = 0x2, cid = 0x2E) {
        Params = {
            service_handle: AttributeHandle,
            attribute_handle: AttributeHandle,
            permissions: AccessPermission,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattStoreDatabase(cgid = 0x2, cid = 0x30) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSendMultipleNotification(cgid = 0x2, cid = 0x31) {
        Params<'a> = {
            conn_handle: ConnHandle,
            handles: &'a [AttributeHandle] => {
                kind: counted_items,
                count: u8,
                item: AttributeHandle,
                max_items: 126,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadMultipleVarCharValue(cgid = 0x2, cid = 0x32) {
        Params<'a> = {
            conn_handle: ConnHandle,
            handles: &'a [AttributeHandle] => {
                kind: counted_items,
                count: u8,
                item: AttributeHandle,
                max_items: 126,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(since_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattWriteWithoutRespExt(cgid = 0x2, cid = 0x40) {
        Params = {
            conn_handle: ConnHandle,
            attribute_handle: AttributeHandle,
            signed_mode: bool,
            data: ExtraDataReference,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_1_24_0)]
stm32wb_hci_macros::vendor_cmd! {
    GattWriteWithRespExt(cgid = 0x2, cid = 0x41) {
        Params = {
            conn_handle: ConnHandle,
            attribute_handle: AttributeHandle,
            write_mode: WriteMode,
            value_offset: GattAttributeOffset,
            data: ExtraDataReference,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Application decision returned for a pending attribute write.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum WriteStatus: u8 => 1 {
        /// Allow the requested attribute write.
        Allowed = 0x00,
        /// Reject the requested attribute write and return its ATT error code.
        Rejected = 0x01,
    }
}

#[cfg(since_fw_1_24_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Application decision returned for a pending attribute read.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ReadStatus: u8 => 1 {
        /// Allow all reads described by the permission event.
        Allowed = 0x00,
        /// Reject one or all reads described by the permission event.
        Rejected = 0x01,
    }
}

/// Invalid range into the controller's extra-data buffer.
#[cfg(since_fw_1_24_0)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ExtraDataRangeError {
    /// The range ended before it started.
    Inverted { start: u32, end: u32 },
    /// The range length cannot be represented by the command's 16-bit field.
    TooLong { length: u32 },
}

/// A validated range in the controller's pre-filled extra-data buffer.
#[cfg(since_fw_1_24_0)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ExtraDataReference {
    length: u16,
    offset: u32,
}

#[cfg(since_fw_1_24_0)]
impl ExtraDataReference {
    /// Construct a reference from the occupied byte range in the extra-data buffer.
    pub fn try_new(range: core::ops::Range<u32>) -> Result<Self, ExtraDataRangeError> {
        let Some(length) = range.end.checked_sub(range.start) else {
            return Err(ExtraDataRangeError::Inverted {
                start: range.start,
                end: range.end,
            });
        };
        let length = u16::try_from(length).map_err(|_| ExtraDataRangeError::TooLong { length })?;
        Ok(Self {
            length,
            offset: range.start,
        })
    }

    /// Byte offset from the start of the extra-data buffer.
    pub const fn offset(self) -> u32 {
        self.offset
    }

    /// Number of bytes occupied by this reference.
    pub const fn length(self) -> u16 {
        self.length
    }
}

#[cfg(since_fw_1_24_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    ExtraDataReference => 6 {
        Fields = {
            length: u16,
            offset: u32,
        };
        Encode = |value| {
            (value.length, value.offset)
        };
    }
}

#[cfg(since_fw_1_24_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// GATT write-with-response procedure used by [`GattWriteWithRespExt`].
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum WriteMode: u8 => 1 {
        /// Write a characteristic value or descriptor.
        CharacteristicOrDescriptor = 0x00,
        /// Write a long characteristic value or descriptor.
        LongCharacteristicOrDescriptor = 0x01,
        /// Reliably write a characteristic value.
        ReliableCharacteristic = 0x02,
    }
}

/// Types of UUID.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uuid {
    /// 16-bit UUID.
    Uuid16(u16),

    /// 128-bit UUID.
    Uuid128([u8; 16]),
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Types of GATT services.
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ServiceType: u8 => 1 {
        /// Primary service
        Primary = 0x01,
        /// Secondary service
        Secondary = 0x02,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Available characteristic properties. Defined in Volume 3, Part G,
    /// Section 3.3.3.1 of Bluetooth Specification 4.1.
    pub struct CharacteristicProperty: u8 => 1 {
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Security permissions available for characteristics.
    pub struct CharacteristicPermission: u8 => 1 {
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Which events may be generated when a characteristic is accessed.
    pub struct CharacteristicEvent: u8 => 1 {
        /// The application will be notified when a client writes to this attribute.
        const ATTRIBUTE_WRITE = 0x01;

        /// The application will be notified when a write request/write command/signed write command
        /// is received by the server for this attribute.
        const CONFIRM_WRITE = 0x02;

        /// The application will be notified when a read request of any type is got for this
        /// attribute.
        const CONFIRM_READ = 0x04;

        #[cfg(since_fw_1_17_0)]
        /// The application will be notified when a notification is complete.
        const NOTIFY_NOTIFICATION_COMPLETE = 0x08;
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Encryption key size, in bytes.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct EncryptionKeySize: u8 => 1 {
        minimum: 7,
        maximum: 16,
    }
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Permissions available for characteristic descriptors.
    pub struct DescriptorPermission: u8 => 1 {
        /// Authentication required.
        const AUTHENTICATED = 0x01;

        /// Authorization required.
        const AUTHORIZED = 0x02;

        /// Encryption required.
        const ENCRYPTED = 0x04;
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Types of access for characteristic descriptors
    pub struct AccessPermission: u8 => 1 {
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Flags for individual events that can be masked by the
    /// [GATT Set Event Mask](GattSetEventMask) command.
    pub struct Event: u32 => 4 {
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
        /// [ATT Read By Type Response](crate::vendor::event::VendorEvent::AttReadByTypeResponse).
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
        /// [GATT Extended Read](crate::vendor::event::VendorEvent::GattReadExt)
        const READ_EXT = 0x0010_0000;
        /// [GATT Extended Indication](crate::vendor::event::VendorEvent::GattIndicationExt)
        const INDICATION_EXT = 0x0020_0000;
        /// [GATT Extended Notification](crate::vendor::event::VendorEvent::GattNotificationExt)
        const NOTIFICATION_EXT = 0x0040_0000;
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Flags for types of updates that the controller should signal when a characteristic value is
    /// [updated](GattUpdateLongCharacteristicValue).
    pub struct UpdateType: u8 => 1 {
        /// A notification can be sent if enabled in the client characteristic configuration
        /// descriptor.
        const NOTIFICATION = 0x01;
        /// An indication can be sent if enabled in the client characteristic configuration
        /// descriptor.
        const INDICATION = 0x02;
    }
}
