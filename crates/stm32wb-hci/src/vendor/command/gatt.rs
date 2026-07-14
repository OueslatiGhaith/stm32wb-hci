//! GATT commands and types needed for those commands.

use bt_hci::param::ConnHandle;

use crate::vendor::{command::BoundedBytes, event::AttributeHandle};

hci_ranged! {
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
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
            service_type: ServiceType => 1,
            max_attribute_records: u8 => 1,
        };
        Completion = CommandComplete;
        Return = GattService {
            service_handle: AttributeHandle => 2,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattIncludeService(cgid = 0x2, cid = 0x03) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            include_handle_start: AttributeHandle => 2,
            include_handle_end: AttributeHandle => 2,
            include_uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
                    },
                },
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

stm32wb_hci_macros::vendor_cmd! {
    GattAddCharacteristic(cgid = 0x2, cid = 0x04) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            characteristic_uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
            characteristic_value_len: u16 => 2,
            characteristic_properties: CharacteristicProperty => 1,
            security_permissions: CharacteristicPermission => 1,
            gatt_event_mask: CharacteristicEvent => 1,
            encryption_key_size: EncryptionKeySize => 1,
            is_variable: bool => 1,
        };
        Completion = CommandComplete;
        Return = GattCharacteristic {
            characteristic_handle: AttributeHandle => 2,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattAddCharacteristicDescriptor(cgid = 0x2, cid = 0x05) {
        Params<'a> = {
            service_handle: AttributeHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            descriptor_uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
                    },
                },
                min_len: 3,
                max_len: 17,
            },
            descriptor_value_max_len: DescriptorValueMaxLength => 1,
            descriptor_value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 227,
            },
            security_permissions: DescriptorPermission => 1,
            access_permissions: AccessPermission => 1,
            gatt_event_mask: CharacteristicEvent => 1,
            encryption_key_size: EncryptionKeySize => 1,
            is_variable: bool => 1,
        };
        Constraints = {
            len_at_most(descriptor_value, descriptor_value_max_len);
        };
        Completion = CommandComplete;
        Return = GattCharacteristicDescriptor {
            descriptor_handle: AttributeHandle => 2,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
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

stm32wb_hci_macros::vendor_cmd! {
    GattDeleteCharacterisitic(cgid = 0x2, cid = 0x07) {
        Params = {
            service: AttributeHandle => 2,
            characteristic: AttributeHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDeleteService(cgid = 0x2, cid = 0x08) {
        Params = {
            service: AttributeHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDeleteIncludedService(cgid = 0x2, cid = 0x09) {
        Params = {
            service: AttributeHandle => 2,
            included_service: AttributeHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSetEventMask(cgid = 0x2, cid = 0x0A) {
        Params = {
            event_mask: Event => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattExchageConfiguration(cgid = 0x2, cid = 0x0B) {
        Params = {
            conn_handle: ConnHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattFindInformationRequest(cgid = 0x2, cid = 0x0C) {
        Params = {
            conn_handle: ConnHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattFindByTypeValueRequest(cgid = 0x2, cid = 0x0D) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattReadByTypeRequest(cgid = 0x2, cid = 0x0E) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
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
            conn_handle: ConnHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
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
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattExecuteWriteRequest(cgid = 0x2, cid = 0x11) {
        Params = {
            conn_handle: ConnHandle => 2,
            execute: bool => 1,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverAllPrimaryServices(cgid = 0x2, cid = 0x12) {
        Params = {
            conn_handle: ConnHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverPrimaryServicesByUUID(cgid = 0x2, cid = 0x13) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
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
            conn_handle: ConnHandle => 2,
            service_handle_start: AttributeHandle => 2,
            service_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverAllCharacteristicsOfService(cgid = 0x2, cid = 0x15) {
        Params = {
            conn_handle: ConnHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattDiscoverCharacteristicsByUUID(cgid = 0x2, cid = 0x16) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
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
            conn_handle: ConnHandle => 2,
            characteristic_handle_start: AttributeHandle => 2,
            characteristic_handle_end: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadCharacteristicValue(cgid = 0x2, cid = 0x18) {
        Params = {
            conn_handle: ConnHandle => 2,
            characteristic_handle: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadCharacteristicUsingUUID(cgid = 0x2, cid = 0x19) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
            attribute_handle_start: AttributeHandle => 2,
            attribute_handle_end: AttributeHandle => 2,
            uuid: &'a Uuid => {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    Uuid::Uuid16(value) => {
                        tag: 0x01,
                        fields: { value: u16 => 2, },
                    },
                    Uuid::Uuid128(value) => {
                        tag: 0x02,
                        fields: { value: [u8; 16] => 16, },
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
            conn_handle: ConnHandle => 2,
            attribute: AttributeHandle => 2,
            offset: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattReadMultipleCharacteristicValues(cgid = 0x2, cid = 0x1B) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattWriteCharacteristicValue(cgid = 0x2, cid = 0x1C) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattWriteLongCharacteristicValue(cgid = 0x2, cid = 0x1D) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattWriteCharacteristicValueReliably(cgid = 0x2, cid = 0x1E) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattWriteLongCharacteristicDescriptor(cgid = 0x2, cid = 0x1F) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattReadLongCharacteristicDescriptor(cgid = 0x2, cid = 0x20) {
        Params = {
            conn_handle: ConnHandle => 2,
            attribute: AttributeHandle => 2,
            offset: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattWriteCharacteristicDescriptor(cgid = 0x2, cid = 0x21) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattReadCharacteristicDescriptor(cgid = 0x2, cid = 0x22) {
        Params = {
            conn_handle: ConnHandle => 2,
            descriptor_handle: AttributeHandle => 2,
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattWriteWithoutResponse(cgid = 0x2, cid = 0x23) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattSignedWriteWithoutResponse(cgid = 0x2, cid = 0x24) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattConfirmIndication(cgid = 0x2, cid = 0x25) {
        Params = {
            conn_handle: ConnHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattWriteResponse(cgid = 0x2, cid = 0x26) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
            attribute_handle: AttributeHandle => 2,
            write_status: WriteStatus => 1,
            error_code: u8 => 1,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
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

stm32wb_hci_macros::vendor_cmd! {
    GattAllowRead(cgid = 0x2, cid = 0x27) {
        Params = {
            conn_handle: ConnHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSetSecurityPermission(cgid = 0x2, cid = 0x28) {
        Params = {
            service_handle: AttributeHandle => 2,
            attribute_handle: AttributeHandle => 2,
            permission: CharacteristicPermission => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
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

stm32wb_hci_macros::vendor_cmd! {
    GattReadHandleValue(cgid = 0x2, cid = 0x2A) {
        Params = {
            handle: AttributeHandle => 2,
            offset: u16 => 2,
            value_length_requested: u16 => 2,
        };
        Completion = CommandComplete;
        Return = GattHandleValue {
            total_length: u16 => 2,
            value: BoundedBytes<247> => {
                kind: counted_bytes,
                count: u16 => 2,
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

#[cfg(since_fw_0_17_1)]
stm32wb_hci_macros::vendor_cmd! {
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

stm32wb_hci_macros::vendor_cmd! {
    GattUpdateLongCharacteristicValue(cgid = 0x2, cid = 0x2C) {
        Params<'a> = {
            conn_handle_to_notify: u16 => 2,
            service_handle: AttributeHandle => 2,
            characteristic_handle: AttributeHandle => 2,
            update_type: UpdateType => 1,
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

stm32wb_hci_macros::vendor_cmd! {
    GattDenyRead(cgid = 0x2, cid = 0x2D) {
        Params = {
            conn_handle: ConnHandle => 2,
            error_code: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    GattSetAccessPermission(cgid = 0x2, cid = 0x2E) {
        Params = {
            service_handle: AttributeHandle => 2,
            attribute_handle: AttributeHandle => 2,
            permissions: AccessPermission => 1,
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
            conn_handle: ConnHandle => 2,
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

stm32wb_hci_macros::vendor_cmd! {
    GattReadMultipleVarCharValue(cgid = 0x2, cid = 0x32) {
        Params<'a> = {
            conn_handle: ConnHandle => 2,
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

#[cfg(since_fw_0_17_1)]
stm32wb_hci_macros::vendor_cmd! {
    GattWriteWithoutRespExt(cgid = 0x2, cid = 0x40) {
        Params = {
            conn_handle: ConnHandle => 2,
            attr_handle: u16 => 2,
            signed_mode: bool => 1,
            data_len: u16 => 2,
            data_pointer: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_0_17_1)]
stm32wb_hci_macros::vendor_cmd! {
    GattWriteWithRespExt(cgid = 0x2, cid = 0x41) {
        Params = {
            conn_handle: ConnHandle => 2,
            attr_handle: u16 => 2,
            write_mode: WriteMode => 1,
            val_offset: u16 => 2,
            data_len: u16 => 2,
            data_pointer: u32 => 4,
        };
        Completion = CommandStatus;
    }
}

hci_enum! {
    /// Application decision returned by [`GattWriteResponse`].
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum WriteStatus: u8 => 1 {
        /// Allow the requested attribute write.
        Allowed = 0x00,
        /// Reject the requested attribute write and return its ATT error code.
        Rejected = 0x01,
    }
}

#[cfg(since_fw_0_17_1)]
hci_enum! {
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

hci_enum! {
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

hci_bitflags! {
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

hci_bitflags! {
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

hci_bitflags! {
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

        #[cfg(since_fw_0_17_0)]
        /// The application will be notified when a notification is complete.
        const NOTIFY_NOTIFICATION_COMPLETE = 0x08;
    }
}

hci_ranged! {
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

hci_bitflags! {
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

hci_bitflags! {
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

hci_bitflags! {
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

hci_bitflags! {
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
