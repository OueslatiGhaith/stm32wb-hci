//! ATT client commands, in opcode order, and events, in code order.

#[allow(
    unused_imports,
    reason = "the HCI-layer and light profiles have no ATT client commands"
)]
use bt_hci::param::ConnHandle;
use stm32wb_hci_macros::{vendor_command, vendor_event, vendor_struct};

#[allow(
    unused_imports,
    reason = "the HCI-layer and light profiles have no ATT client commands"
)]
use crate::wire::{Elements, Uuid};

vendor_command! {
    /// Find the handles and types of the attributes in a handle range.
    aci_att_find_info_req => AttFindInfoReq {
        connection_handle: ConnHandle,
        start_handle: u16,
        end_handle: u16,
    }
}

vendor_command! {
    /// Find the attributes of 16-bit type `uuid` with the given value in a
    /// handle range.
    aci_att_find_by_type_value_req => AttFindByTypeValueReq {
        connection_handle: ConnHandle,
        start_handle: u16,
        end_handle: u16,
        uuid: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Read the attributes of the given type in a handle range.
    aci_att_read_by_type_req => AttReadByTypeReq {
        connection_handle: ConnHandle,
        start_handle: u16,
        end_handle: u16,
        uuid: Uuid,
    }
}

vendor_command! {
    /// Read the attributes of the given grouping type in a handle range.
    aci_att_read_by_group_type_req => AttReadByGroupTypeReq {
        connection_handle: ConnHandle,
        start_handle: u16,
        end_handle: u16,
        uuid: Uuid,
    }
}

vendor_command! {
    /// Queue part of an attribute value to write from `val_offset`.
    aci_att_prepare_write_req => AttPrepareWriteReq {
        connection_handle: ConnHandle,
        attr_handle: u16,
        val_offset: u16,
        attribute_val: &'a [u8],
    }
}

vendor_command! {
    /// Write (`execute` 1) or cancel (`execute` 0) the queued writes.
    aci_att_execute_write_req => AttExecuteWriteReq {
        connection_handle: ConnHandle,
        execute: bool,
    }
}

vendor_event! {
    /// The server answered an MTU exchange with its receive MTU.
    aci_att_exchange_mtu_resp_event => AttExchangeMtuRespEvent {
        connection_handle: ConnHandle,
        server_rx_mtu: u16,
    }
}

vendor_event! {
    /// The server answered a find information request with handle and UUID
    /// pairs, whose UUIDs are 16-bit (`format` 1) or 128-bit (`format` 2).
    aci_att_find_info_resp_event => AttFindInfoRespEvent {
        connection_handle: ConnHandle,
        format: u8,
        handle_uuid_pair: &'a [u8],
    }
}

vendor_struct! {
    /// A found attribute and the end of its group.
    Attribute_Group_Handle_Pair_t => AttributeGroupHandlePair {
        found_attribute_handle: u16,
        group_end_handle: u16,
    }
}

vendor_event! {
    /// The server answered a find by type value request.
    aci_att_find_by_type_value_resp_event => AttFindByTypeValueRespEvent {
        connection_handle: ConnHandle,
        attribute_group_handle_pair: Elements<'a, AttributeGroupHandlePair>,
    }
}

vendor_event! {
    /// The server answered a read by type request with handle and value
    /// pairs of `handle_value_pair_length` bytes each.
    aci_att_read_by_type_resp_event => AttReadByTypeRespEvent {
        connection_handle: ConnHandle,
        handle_value_pair_length: u8,
        handle_value_pair_data: &'a [u8],
    }
}

vendor_event! {
    /// The server answered a read request.
    aci_att_read_resp_event => AttReadRespEvent {
        connection_handle: ConnHandle,
        attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// The server answered a read blob request.
    aci_att_read_blob_resp_event => AttReadBlobRespEvent {
        connection_handle: ConnHandle,
        attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// The server answered a read multiple request.
    aci_att_read_multiple_resp_event => AttReadMultipleRespEvent {
        connection_handle: ConnHandle,
        set_of_values: &'a [u8],
    }
}

vendor_event! {
    /// The server answered a read by group type request with attribute data
    /// of `attribute_data_length` bytes each.
    aci_att_read_by_group_type_resp_event => AttReadByGroupTypeRespEvent {
        connection_handle: ConnHandle,
        attribute_data_length: u8,
        attribute_data_list: &'a [u8],
    }
}

vendor_event! {
    /// The server queued part of a value to write, echoing it back.
    aci_att_prepare_write_resp_event => AttPrepareWriteRespEvent {
        connection_handle: ConnHandle,
        attribute_handle: u16,
        offset: u16,
        part_attribute_value: &'a [u8],
    }
}

vendor_event! {
    /// The server wrote or cancelled the queued writes.
    aci_att_exec_write_resp_event => AttExecWriteRespEvent {
        connection_handle: ConnHandle,
    }
}

#[cfg(all(test, any(feature = "stack-full-extended", feature = "stack-full")))]
mod tests {
    use bt_hci::cmd::Cmd;
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
    fn type_values_follow_the_16_bit_type() {
        let command =
            AttFindByTypeValueReq::try_new(ConnHandle::new(1), 1, 0xFFFF, 0x2800, &[0x0D, 0x18])
                .unwrap();
        assert_eq!(AttFindByTypeValueReq::OPCODE.to_raw(), 0xFD0D);
        let (bytes, len) = encode(&command);
        assert_eq!(
            bytes[..len],
            [
                0x0D, 0xFD, 11, 1, 0, 1, 0, 0xFF, 0xFF, 0x00, 0x28, 2, 0x0D, 0x18
            ]
        );
    }

    #[test]
    fn read_by_type_selects_the_uuid_width() {
        let command = AttReadByTypeReq::new(ConnHandle::new(1), 1, 0xFFFF, Uuid::from(0x2803u16));
        let (bytes, len) = encode(&command);
        assert_eq!(
            bytes[..len],
            [0x0E, 0xFD, 9, 1, 0, 1, 0, 0xFF, 0xFF, 1, 0x03, 0x28]
        );
    }

    #[test]
    fn handle_pairs_decode_as_they_are_read() {
        let params = [
            0x05, 0x0C, 0x01, 0x00, 2, 0x10, 0, 0x1F, 0, 0x20, 0, 0x2F, 0,
        ];
        let found = AttFindByTypeValueRespEvent::from_vendor_params(&params)
            .unwrap()
            .unwrap();
        let pairs = found.attribute_group_handle_pair;
        assert_eq!(pairs.len(), 2);
        assert_eq!(
            pairs.get(1),
            Some(AttributeGroupHandlePair {
                found_attribute_handle: 0x20,
                group_end_handle: 0x2F,
            })
        );
        assert_eq!(
            pairs.iter().map(|pair| pair.group_end_handle).sum::<u16>(),
            0x1F + 0x2F
        );
        assert!(
            AttFindByTypeValueRespEvent::from_vendor_params(&params[..12])
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn read_responses_borrow_the_value() {
        let read = AttReadRespEvent::from_hci_bytes_complete(&[0x01, 0x00, 2, 0xAA, 0xBB]).unwrap();
        assert_eq!(read.attribute_value, [0xAA, 0xBB]);
    }
}
