//! GATT server commands.

#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no GATT commands"
)]
use bt_hci::param::ConnHandle;
use stm32wb_hci_macros::{vendor_command, vendor_struct};

#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no GATT commands"
)]
use crate::wire::Uuid;

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

vendor_struct! {
    /// An attribute handle.
    Handle_Entry_t => HandleEntry {
        handle: u16,
    }
}

vendor_command! {
    /// Read the values of several characteristics at once.
    aci_gatt_read_multiple_char_value => GattReadMultipleCharValue {
        connection_handle: ConnHandle,
        handle_entry: &'a [HandleEntry],
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
    use bt_hci::cmd::{Cmd, SyncCmd};
    use bt_hci::{FromHciBytes, WriteHci};

    use super::*;

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
        let command = GattReadMultipleCharValue::try_new(ConnHandle::new(1), &handles).unwrap();
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..len], [0x1B, 0xFD, 7, 1, 0, 2, 0x10, 0, 0x03, 0x02]);
        let too_many = [HandleEntry::default(); 127];
        assert!(GattReadMultipleCharValue::try_new(ConnHandle::new(1), &too_many).is_err());
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
}
