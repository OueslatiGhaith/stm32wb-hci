//! GATT commands, in opcode order, and events, in code order.

#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no GATT commands"
)]
use bt_hci::param::ConnHandle;
use stm32wb_hci_macros::{vendor_command, vendor_event, vendor_struct};

#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no GATT commands"
)]
use crate::aci::flags::{
    AccessPermissions, CharProperties, GattDescEventMask, GattEventMask, SecurityPermissions,
    UpdateType,
};
#[allow(unused_imports, reason = "the HCI-layer profiles have no GATT events")]
use crate::aci::values::EattBearerState;
#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no GATT commands"
)]
use crate::wire::{AttBearer, BoundedBytes, Elements, OrUnknown, Uuid};

vendor_command! {
    /// Initialize the GATT server, adding the GATT service and its service
    /// changed characteristic.
    aci_gatt_init => GattInit {}
}

vendor_command! {
    /// Add a service to the GATT server, reserving `max_attribute_records`
    /// attributes for its includes, characteristics, and descriptors.
    aci_gatt_add_service => GattAddService {
        service_uuid: Uuid,
        service_type: u8,
        max_attribute_records: u8,
    } -> GattService {
        service_handle: u16,
    }
}

vendor_command! {
    /// Include the service spanning `include_start_handle` to
    /// `include_end_handle` in the service at `service_handle`.
    aci_gatt_include_service => GattIncludeService {
        service_handle: u16,
        include_start_handle: u16,
        include_end_handle: u16,
        include_uuid: Uuid,
    } -> GattInclude {
        include_handle: u16,
    }
}

vendor_command! {
    /// Add a characteristic to a service. `char_value_length` is the largest
    /// value, or the only length if `is_variable` is false.
    aci_gatt_add_char => GattAddChar {
        service_handle: u16,
        char_uuid: Uuid,
        char_value_length: u16,
        char_properties: CharProperties,
        security_permissions: SecurityPermissions,
        gatt_evt_mask: GattEventMask,
        enc_key_size: u8,
        is_variable: bool,
    } -> GattChar {
        char_handle: u16,
    }
}

vendor_command! {
    /// Add a descriptor to a characteristic, with its initial value.
    aci_gatt_add_char_desc => GattAddCharDesc {
        service_handle: u16,
        char_handle: u16,
        char_desc_uuid: Uuid,
        char_desc_value_max_len: u8,
        char_desc_value: &'a [u8],
        security_permissions: SecurityPermissions,
        access_permissions: AccessPermissions,
        gatt_evt_mask: GattDescEventMask,
        enc_key_size: u8,
        is_variable: bool,
    } -> GattCharDesc {
        char_desc_handle: u16,
    }
}

vendor_command! {
    /// Update a characteristic value from `val_offset`, notifying or
    /// indicating the subscribed clients.
    aci_gatt_update_char_value => GattUpdateCharValue {
        service_handle: u16,
        char_handle: u16,
        val_offset: u8,
        char_value: &'a [u8],
    }
}

vendor_command! {
    /// Delete a characteristic from a service.
    aci_gatt_del_char => GattDelChar {
        serv_handle: u16,
        char_handle: u16,
    }
}

vendor_command! {
    /// Delete a service and its characteristics.
    aci_gatt_del_service => GattDelService {
        serv_handle: u16,
    }
}

vendor_command! {
    /// Delete an include definition from a service.
    aci_gatt_del_include_service => GattDelIncludeService {
        serv_handle: u16,
        include_handle: u16,
    }
}

vendor_command! {
    /// Enable or disable the GATT events.
    aci_gatt_set_event_mask => GattSetEventMask {
        gatt_evt_mask: u32,
    }
}

vendor_command! {
    /// Exchange the ATT MTU with the server.
    aci_gatt_exchange_config => GattExchangeConfig {
        connection_handle: ConnHandle,
    }
}

vendor_command! {
    /// Discover every primary service of the server.
    aci_gatt_disc_all_primary_services => GattDiscAllPrimaryServices {
        connection_handle: AttBearer,
    }
}

vendor_command! {
    /// Discover the primary services of the server with the given UUID.
    aci_gatt_disc_primary_service_by_uuid => GattDiscPrimaryServiceByUuid {
        connection_handle: AttBearer,
        uuid: Uuid,
    }
}

vendor_command! {
    /// Find the services a service includes, within its handle range.
    aci_gatt_find_included_services => GattFindIncludedServices {
        connection_handle: AttBearer,
        start_handle: u16,
        end_handle: u16,
    }
}

vendor_command! {
    /// Discover every characteristic of a service, within its handle range.
    aci_gatt_disc_all_char_of_service => GattDiscAllCharOfService {
        connection_handle: AttBearer,
        start_handle: u16,
        end_handle: u16,
    }
}

vendor_command! {
    /// Discover the characteristics with the given UUID in a handle range.
    aci_gatt_disc_char_by_uuid => GattDiscCharByUuid {
        connection_handle: AttBearer,
        start_handle: u16,
        end_handle: u16,
        uuid: Uuid,
    }
}

vendor_command! {
    /// Discover the descriptors of a characteristic, up to `end_handle`.
    aci_gatt_disc_all_char_desc => GattDiscAllCharDesc {
        connection_handle: AttBearer,
        char_handle: u16,
        end_handle: u16,
    }
}

vendor_command! {
    /// Read a characteristic value.
    aci_gatt_read_char_value => GattReadCharValue {
        connection_handle: AttBearer,
        attr_handle: u16,
    }
}

vendor_command! {
    /// Read the values of the characteristics with the given UUID in a handle
    /// range.
    aci_gatt_read_using_char_uuid => GattReadUsingCharUuid {
        connection_handle: AttBearer,
        start_handle: u16,
        end_handle: u16,
        uuid: Uuid,
    }
}

vendor_command! {
    /// Read a characteristic value from `val_offset`, in as many requests as
    /// it takes.
    aci_gatt_read_long_char_value => GattReadLongCharValue {
        connection_handle: AttBearer,
        attr_handle: u16,
        val_offset: u16,
    }
}

vendor_struct! {
    /// An attribute handle.
    Handle_Entry_t => HandleEntry {
        handle: u16,
    }
}

vendor_command! {
    /// Read the values of several characteristics at once.
    aci_gatt_read_multiple_char_value => GattReadMultipleCharValue {
        connection_handle: AttBearer,
        handle_entry: &'a [HandleEntry],
    }
}

vendor_command! {
    /// Write a characteristic value, waiting for the server's response.
    aci_gatt_write_char_value => GattWriteCharValue {
        connection_handle: AttBearer,
        attr_handle: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Write a characteristic value from `val_offset`, in as many prepared
    /// writes as it takes.
    aci_gatt_write_long_char_value => GattWriteLongCharValue {
        connection_handle: AttBearer,
        attr_handle: u16,
        val_offset: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Write a characteristic value with reliable writes, which the server
    /// echoes back for checking.
    aci_gatt_write_char_reliable => GattWriteCharReliable {
        connection_handle: AttBearer,
        attr_handle: u16,
        val_offset: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Write a long characteristic descriptor from `val_offset`.
    aci_gatt_write_long_char_desc => GattWriteLongCharDesc {
        connection_handle: AttBearer,
        attr_handle: u16,
        val_offset: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Read a long characteristic descriptor from `val_offset`.
    aci_gatt_read_long_char_desc => GattReadLongCharDesc {
        connection_handle: AttBearer,
        attr_handle: u16,
        val_offset: u16,
    }
}

vendor_command! {
    /// Write a characteristic descriptor.
    aci_gatt_write_char_desc => GattWriteCharDesc {
        connection_handle: AttBearer,
        attr_handle: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Read a characteristic descriptor.
    aci_gatt_read_char_desc => GattReadCharDesc {
        connection_handle: AttBearer,
        attr_handle: u16,
    }
}

vendor_command! {
    /// Write a characteristic value without waiting for a response.
    aci_gatt_write_without_resp => GattWriteWithoutResp {
        connection_handle: AttBearer,
        attr_handle: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Write a characteristic value with a signature, without waiting for a
    /// response.
    aci_gatt_signed_write_without_resp => GattSignedWriteWithoutResp {
        connection_handle: ConnHandle,
        attr_handle: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Confirm an indication.
    aci_gatt_confirm_indication => GattConfirmIndication {
        #[wire(before = "1.16.0")]
        connection_handle: ConnHandle,
        #[wire(since = "1.16.0")]
        connection_handle: AttBearer,
    }
}

vendor_command! {
    /// Answer a write the server asked the host to approve. Named
    /// `aci_gatt_write_resp` before 1.24.0.
    aci_gatt_permit_write => GattPermitWrite {
        connection_handle: AttBearer,
        attr_handle: u16,
        write_status: u8,
        error_code: u8,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Answer a read the server asked the host to approve. From 1.24.0 it can
    /// also reject the read of `attr_handle` with `error_code`. Named
    /// `aci_gatt_allow_read` before 1.24.0.
    aci_gatt_permit_read => GattPermitRead {
        connection_handle: AttBearer,
        #[wire(since = "1.24.0")]
        read_status: u8,
        #[wire(since = "1.24.0")]
        error_code: u8,
        #[wire(since = "1.24.0")]
        attr_handle: u16,
    }
}

vendor_command! {
    /// Set the security permissions of an attribute.
    aci_gatt_set_security_permission => GattSetSecurityPermission {
        serv_handle: u16,
        attr_handle: u16,
        security_permissions: SecurityPermissions,
    }
}

vendor_command! {
    /// Set a characteristic descriptor value from `val_offset`.
    aci_gatt_set_desc_value => GattSetDescValue {
        serv_handle: u16,
        char_handle: u16,
        char_desc_handle: u16,
        val_offset: u16,
        char_desc_value: &'a [u8],
    }
}

vendor_command! {
    /// Read up to `value_length_requested` bytes of a local attribute value
    /// from `offset`, and the full length of the value.
    aci_gatt_read_handle_value => GattReadHandleValue {
        attr_handle: u16,
        offset: u16,
        value_length_requested: u16,
    } -> GattHandleValue {
        length: u16,
        value: BoundedBytes<247>,
    }
}

vendor_command! {
    /// Update a characteristic value of `char_length` bytes from
    /// `value_offset`, notifying or indicating the client
    /// `conn_handle_to_notify` as `update_type` selects. The unenhanced
    /// bearer of connection handle 0x0000 notifies every subscribed client.
    aci_gatt_update_char_value_ext => GattUpdateCharValueExt {
        #[wire(before = "1.16.0")]
        conn_handle_to_notify: ConnHandle,
        #[wire(since = "1.16.0")]
        conn_handle_to_notify: AttBearer,
        service_handle: u16,
        char_handle: u16,
        update_type: UpdateType,
        char_length: u16,
        value_offset: u16,
        value: &'a [u8],
    }
}

vendor_command! {
    /// Reject a read the server asked the host to approve.
    aci_gatt_deny_read => GattDenyRead {
        connection_handle: AttBearer,
        error_code: u8,
    }
}

vendor_command! {
    /// Set the access permissions of an attribute.
    aci_gatt_set_access_permission => GattSetAccessPermission {
        serv_handle: u16,
        attr_handle: u16,
        access_permissions: AccessPermissions,
    }
}

vendor_command! {
    /// Store the GATT database in flash.
    aci_gatt_store_db => GattStoreDb {}
}

vendor_command! {
    /// Notify the values of several characteristics at once.
    aci_gatt_send_mult_notification => GattSendMultNotification {
        connection_handle: AttBearer,
        handle_entry: &'a [HandleEntry],
    }
}

vendor_command! {
    /// Read the values of several variable-length characteristics at once.
    aci_gatt_read_multiple_var_char_value => GattReadMultipleVarCharValue {
        connection_handle: AttBearer,
        handle_entry: &'a [HandleEntry],
    }
}

vendor_command! {
    /// Write `data_length` bytes at `data_pointer` without waiting for a
    /// response. The wireless CPU reads the data from that address, so it
    /// must stay valid until the write completes.
    aci_gatt_write_without_resp_ext => GattWriteWithoutRespExt {
        connection_handle: ConnHandle,
        attr_handle: u16,
        signed_mode: u8,
        data_length: u16,
        data_pointer: u32,
    }
}

vendor_command! {
    /// Write `data_length` bytes at `data_pointer` from `val_offset`, waiting
    /// for the server's response. The wireless CPU reads the data from that
    /// address, so it must stay valid until the procedure completes.
    aci_gatt_write_with_resp_ext => GattWriteWithRespExt {
        connection_handle: ConnHandle,
        attr_handle: u16,
        write_mode: u8,
        val_offset: u16,
        data_length: u16,
        data_pointer: u32,
    }
}

vendor_event! {
    /// A client modified a local attribute from `offset`.
    aci_gatt_attribute_modified_event => GattAttributeModifiedEvent {
        connection_handle: AttBearer,
        attr_handle: u16,
        offset: u16,
        attr_data: &'a [u8],
    }
}

vendor_event! {
    /// A GATT procedure timed out.
    aci_gatt_proc_timeout_event => GattProcTimeoutEvent {
        connection_handle: AttBearer,
    }
}

vendor_event! {
    /// The server indicated a characteristic value; confirm it with
    /// [`GattConfirmIndication`].
    aci_gatt_indication_event => GattIndicationEvent {
        #[wire(before = "1.16.0")]
        connection_handle: ConnHandle,
        #[wire(since = "1.16.0")]
        connection_handle: AttBearer,
        attribute_handle: u16,
        attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// The server notified a characteristic value.
    aci_gatt_notification_event => GattNotificationEvent {
        #[wire(before = "1.16.0")]
        connection_handle: ConnHandle,
        #[wire(since = "1.16.0")]
        connection_handle: AttBearer,
        attribute_handle: u16,
        attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// A GATT client procedure completed, with `error_code` 0 on success.
    aci_gatt_proc_complete_event => GattProcCompleteEvent {
        connection_handle: AttBearer,
        error_code: u8,
    }
}

vendor_event! {
    /// The server rejected the request with opcode `req_opcode` on an
    /// attribute.
    aci_gatt_error_resp_event => GattErrorRespEvent {
        connection_handle: AttBearer,
        req_opcode: u8,
        attribute_handle: u16,
        error_code: u8,
    }
}

vendor_event! {
    /// A characteristic found by UUID, with its value.
    aci_gatt_disc_read_char_by_uuid_resp_event => GattDiscReadCharByUuidRespEvent {
        connection_handle: AttBearer,
        attribute_handle: u16,
        attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// A client wants to write an attribute that needs the host's approval;
    /// answer with [`GattPermitWrite`].
    aci_gatt_write_permit_req_event => GattWritePermitReqEvent {
        connection_handle: AttBearer,
        attribute_handle: u16,
        data: &'a [u8],
    }
}

vendor_event! {
    /// A client wants to read an attribute that needs the host's approval;
    /// answer with [`GattPermitRead`].
    aci_gatt_read_permit_req_event => GattReadPermitReqEvent {
        connection_handle: AttBearer,
        attribute_handle: u16,
        offset: u16,
    }
}

vendor_struct! {
    /// An attribute handle in a read multiple request.
    Handle_Item_t => HandleItem {
        handle: u16,
    }
}

vendor_event! {
    /// A client wants to read several attributes that need the host's
    /// approval; answer with [`GattPermitRead`].
    aci_gatt_read_multi_permit_req_event => GattReadMultiPermitReqEvent {
        connection_handle: AttBearer,
        handle_item: Elements<'a, HandleItem>,
    }
}

vendor_event! {
    /// Buffers freed up after a send failed for lack of them.
    aci_gatt_tx_pool_available_event => GattTxPoolAvailableEvent {
        connection_handle: ConnHandle,
        available_buffers: u16,
    }
}

vendor_event! {
    /// The client confirmed an indication.
    aci_gatt_server_confirmation_event => GattServerConfirmationEvent {
        #[wire(before = "1.16.0")]
        connection_handle: ConnHandle,
        #[wire(since = "1.16.0")]
        connection_handle: AttBearer,
    }
}

vendor_event! {
    /// A client wants to queue a write that needs the host's approval;
    /// answer with [`GattPermitWrite`].
    aci_gatt_prepare_write_permit_req_event => GattPrepareWritePermitReqEvent {
        connection_handle: AttBearer,
        attribute_handle: u16,
        offset: u16,
        data: &'a [u8],
    }
}

vendor_event! {
    /// An enhanced ATT bearer changed state. From 1.23.0 the event names the
    /// connection and the bearer's MTU instead of a status.
    aci_gatt_eatt_bearer_event => GattEattBearerEvent {
        #[wire(since = "1.23.0")]
        connection_handle: ConnHandle,
        channel_index: u8,
        eab_state: OrUnknown<EattBearerState>,
        #[wire(before = "1.23.0")]
        status: u8,
        #[wire(since = "1.23.0")]
        mtu: u16,
    }
}

vendor_event! {
    /// The server notified several characteristic values at once.
    aci_gatt_mult_notification_event => GattMultNotificationEvent {
        connection_handle: AttBearer,
        offset: u16,
        data: &'a [u8],
    }
}

vendor_event! {
    /// A notification of the attribute went out.
    aci_gatt_notification_complete_event => GattNotificationCompleteEvent {
        attr_handle: u16,
    }
}

vendor_event! {
    /// Part of a long read response, from `offset`.
    aci_gatt_read_ext_event => GattReadExtEvent {
        connection_handle: AttBearer,
        offset: u16,
        attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// Part of a long indicated value, from `offset`.
    aci_gatt_indication_ext_event => GattIndicationExtEvent {
        #[wire(before = "1.16.0")]
        connection_handle: ConnHandle,
        #[wire(since = "1.16.0")]
        connection_handle: AttBearer,
        attribute_handle: u16,
        offset: u16,
        attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// Part of a long notified value, from `offset`.
    aci_gatt_notification_ext_event => GattNotificationExtEvent {
        connection_handle: AttBearer,
        attribute_handle: u16,
        offset: u16,
        attribute_value: &'a [u8],
    }
}

#[cfg(all(
    test,
    any(
        feature = "stack-full-extended",
        feature = "stack-full",
        feature = "stack-light"
    )
))]
mod tests {
    use bt_hci::cmd::{Cmd, CmdReturnBuf, SyncCmd};
    use bt_hci::{FromHciBytes, WriteHci};

    use super::*;
    use crate::wire::VendorEvent;

    fn encode(command: &impl WriteHci) -> ([u8; 32], usize) {
        let mut buffer = [0; 32];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 32 - writer.len();
        (buffer, len)
    }

    #[test]
    fn union_selectors_are_written_from_the_alternative() {
        assert_eq!(GattAddService::OPCODE.to_raw(), 0xFD02);
        let (bytes, len) = encode(&GattAddService::new(Uuid::from(0x180Du16), 1, 4));
        assert_eq!(bytes[..len], [0x02, 0xFD, 5, 1, 0x0D, 0x18, 1, 4]);

        let uuid = core::array::from_fn(|index| index as u8);
        let (bytes, len) = encode(&GattAddService::new(Uuid::Uuid128(uuid), 2, 8));
        assert_eq!(bytes[..3], [0x02, 0xFD, 19]);
        assert_eq!(bytes[3], 2);
        assert_eq!(bytes[4..20], uuid);
        assert_eq!(bytes[20..len], [2, 8]);
    }

    #[test]
    fn unions_may_follow_fixed_parameters() {
        let command = GattIncludeService::new(0x0010, 0x0020, 0x0028, Uuid::from(0x1801u16));
        let (bytes, len) = encode(&command);
        assert_eq!(
            bytes[..len],
            [0x03, 0xFD, 9, 0x10, 0, 0x20, 0, 0x28, 0, 1, 0x01, 0x18]
        );
        let include =
            <GattIncludeService as SyncCmd>::Return::from_hci_bytes_complete(&[0x29, 0]).unwrap();
        assert_eq!(include.include_handle, 0x0029);
    }

    #[cfg(any(feature = "stack-full-extended", feature = "stack-full"))]
    #[test]
    fn handle_lists_encode_their_count() {
        let handles = [
            HandleEntry { handle: 0x0010 },
            HandleEntry { handle: 0x0203 },
        ];
        let command =
            GattReadMultipleCharValue::try_new(ConnHandle::new(1).into(), &handles).unwrap();
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..len], [0x1B, 0xFD, 7, 1, 0, 2, 0x10, 0, 0x03, 0x02]);
        let too_many = [HandleEntry::default(); 127];
        assert!(GattReadMultipleCharValue::try_new(ConnHandle::new(1).into(), &too_many).is_err());
    }

    #[test]
    fn descriptors_carry_a_union_and_a_counted_value() {
        let command = GattAddCharDesc::try_new(
            0x0010,
            0x0012,
            Uuid::from(0x2901u16),
            8,
            b"hi",
            SecurityPermissions::empty(),
            AccessPermissions::READ,
            GattDescEventMask::empty(),
            7,
            true,
        )
        .unwrap();
        assert_eq!(GattAddCharDesc::OPCODE.to_raw(), 0xFD05);
        let (bytes, len) = encode(&command);
        assert_eq!(
            bytes[..len],
            [
                0x05, 0xFD, 16, 0x10, 0, 0x12, 0, 1, 0x01, 0x29, 8, 2, b'h', b'i', 0, 1, 0, 7, 1
            ]
        );
        let descriptor =
            <GattAddCharDesc as SyncCmd>::Return::from_hci_bytes_complete(&[0x13, 0]).unwrap();
        assert_eq!(descriptor.char_desc_handle, 0x0013);
    }

    #[test]
    fn handle_values_decode_a_16_bit_count() {
        let value = <GattReadHandleValue as SyncCmd>::Return::from_hci_bytes_complete(&[
            5, 0, 3, 0, 1, 2, 3,
        ])
        .unwrap();
        assert_eq!(value.length, 5);
        assert_eq!(value.value.as_slice(), [1, 2, 3]);
        assert!(
            <GattReadHandleValue as SyncCmd>::Return::from_hci_bytes_complete(&[5, 0, 3, 0, 1])
                .is_err()
        );
        assert_eq!(
            <GattReadHandleValue as SyncCmd>::ReturnBuf::LEN,
            2 + 2 + 247
        );
    }

    #[cfg(feature = "fw_1_24_0")]
    #[test]
    fn read_permissions_answer_for_an_attribute_from_1_24_0() {
        let (bytes, len) = encode(&GattPermitRead::new(
            ConnHandle::new(1).into(),
            1,
            0x0E,
            0x0020,
        ));
        assert_eq!(bytes[..len], [0x27, 0xFD, 6, 1, 0, 1, 0x0E, 0x20, 0]);
    }

    #[cfg(not(feature = "fw_1_24_0"))]
    #[test]
    fn read_permissions_name_only_the_connection_before_1_24_0() {
        let (bytes, len) = encode(&GattPermitRead::new(ConnHandle::new(1).into()));
        assert_eq!(bytes[..len], [0x27, 0xFD, 2, 1, 0]);
    }

    #[tokio::test]
    async fn asynchronous_encoding_matches_synchronous() {
        let command = GattAddService::new(Uuid::Uuid128([7; 16]), 1, 3);
        let mut buffer = [0; 32];
        let mut writer = &mut buffer[..];
        command.write_hci_async(&mut writer).await.unwrap();
        let len = 32 - writer.len();
        let (expected, expected_len) = encode(&command);
        assert_eq!(buffer[..len], expected[..expected_len]);
    }

    #[test]
    fn modified_attributes_follow_a_16_bit_length() {
        let params = [0x01, 0x0C, 0x01, 0x00, 0x12, 0x00, 0x00, 0x00, 2, 0, 1, 0];
        let modified = GattAttributeModifiedEvent::from_vendor_params(&params)
            .unwrap()
            .unwrap();
        assert_eq!(modified.attr_handle, 0x0012);
        assert_eq!(modified.attr_data, [1, 0]);
    }

    #[test]
    fn read_permit_requests_list_their_handles() {
        let request = GattReadMultiPermitReqEvent::from_hci_bytes_complete(&[
            0x01, 0x00, 3, 0x10, 0, 0x11, 0, 0x12, 0,
        ])
        .unwrap();
        assert!(
            request
                .handle_item
                .iter()
                .map(|item| item.handle)
                .eq([0x10, 0x11, 0x12])
        );
    }

    #[cfg(all(feature = "stack-full-extended", feature = "fw_1_24_0"))]
    #[test]
    fn enhanced_bearers_report_their_mtu_from_1_23_0() {
        let bearer =
            GattEattBearerEvent::from_hci_bytes_complete(&[0x01, 0x08, 1, 0, 0x40, 0]).unwrap();
        assert_eq!(bearer.connection_handle.raw(), 0x0801);
        assert_eq!(
            (bearer.channel_index, bearer.eab_state, bearer.mtu),
            (1, OrUnknown::Known(EattBearerState::Created), 64)
        );
    }

    #[cfg(all(feature = "stack-full-extended", feature = "fw_1_22_1"))]
    #[test]
    fn enhanced_bearers_report_a_status_before_1_23_0() {
        let bearer = GattEattBearerEvent::from_hci_bytes_complete(&[1, 0, 0x12]).unwrap();
        assert_eq!(
            (bearer.channel_index, bearer.eab_state, bearer.status),
            (1, OrUnknown::Known(EattBearerState::Created), 0x12)
        );
    }

    #[cfg(feature = "stack-full-extended")]
    #[test]
    fn client_procedures_run_on_enhanced_bearers() {
        let bearer = AttBearer::enhanced(5).unwrap();
        let (bytes, len) = encode(&GattReadCharValue::new(bearer, 0x0012));
        assert_eq!(bytes[3..len], [0x05, 0xEA, 0x12, 0x00]);
        let (bytes, len) = encode(&GattReadCharValue::new(
            ConnHandle::new(0x0801).into(),
            0x0012,
        ));
        assert_eq!(bytes[3..len], [0x01, 0x08, 0x12, 0x00]);

        let response = GattProcCompleteEvent::from_hci_bytes_complete(&[0x05, 0xEA, 0]).unwrap();
        assert_eq!(response.connection_handle.channel_index(), Some(5));
        // Past the last channel of every release.
        assert!(GattProcCompleteEvent::from_hci_bytes_complete(&[0x40, 0xEA, 0]).is_err());
    }

    /// The server side takes enhanced bearers from 1.16.0 only.
    #[cfg(all(feature = "stack-full-extended", feature = "fw_1_15_0"))]
    #[test]
    fn server_notifications_use_connections_in_1_15_0() {
        let event = GattNotificationEvent::from_hci_bytes_complete(&[0x01, 0x08, 0x12, 0, 1, 0xAA])
            .unwrap();
        let handle: ConnHandle = event.connection_handle;
        assert_eq!(handle.raw(), 0x0801);
        let _ = GattConfirmIndication::new(ConnHandle::new(1));
    }
}
