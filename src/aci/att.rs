//! ATT client commands, in opcode order.

#[allow(
    unused_imports,
    reason = "the HCI-layer and light profiles have no ATT client commands"
)]
use bt_hci::param::ConnHandle;
use stm32wb_hci_macros::vendor_command;

#[allow(
    unused_imports,
    reason = "the HCI-layer and light profiles have no ATT client commands"
)]
use crate::wire::Uuid;

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

#[cfg(all(test, any(feature = "stack-full-extended", feature = "stack-full")))]
mod tests {
    use bt_hci::WriteHci;
    use bt_hci::cmd::Cmd;

    use super::*;

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
}
