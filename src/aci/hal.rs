//! Hardware abstraction layer commands.

use stm32wb_hci_macros::vendor_command;

use crate::wire::BoundedBytes;

vendor_command! {
    /// Write a value to the low-level configuration data at `offset`.
    aci_hal_write_config_data => HalWriteConfigData {
        offset: u8,
        value: &'a [u8],
    }
}

vendor_command! {
    /// Read the low-level configuration data at `offset`.
    aci_hal_read_config_data => HalReadConfigData { offset: u8 } -> HalConfigData {
        data: BoundedBytes<250>,
    }
}

vendor_command! {
    /// Stop a tone started with [`HalToneStart`].
    aci_hal_tone_stop => HalToneStop {}
}

vendor_command! {
    /// Start transmitting a continuous tone on an RF channel.
    aci_hal_tone_start => HalToneStart {
        rf_channel: u8,
        freq_offset: u8,
    }
}

vendor_command! {
    /// Select the radio activities reported by the radio activity event.
    aci_hal_set_radio_activity_mask => HalSetRadioActivityMask {
        radio_activity_mask: u16,
    }
}

vendor_command! {
    /// Enable or disable the HAL events.
    aci_hal_set_event_mask => HalSetEventMask {
        event_mask: u32,
    }
}

vendor_command! {
    /// Reset the BLE stack.
    aci_hal_stack_reset => HalStackReset {}
}

vendor_command! {
    /// Read the current anchor period and the largest free slot, in microseconds.
    aci_hal_get_anchor_period => HalGetAnchorPeriod {} -> HalAnchorPeriod {
        anchor_period: u32,
        max_free_slot: u32,
    }
}

vendor_command! {
    /// Read the RSSI of the last received packet, in dBm.
    aci_hal_read_rssi => HalReadRssi {} -> HalRssi {
        rssi: i8,
    }
}

vendor_command! {
    /// Read the state of each link and the connection handle it serves.
    aci_hal_get_link_status => HalGetLinkStatus {} -> HalLinkStatus {
        link_status: [u8; 8],
        link_connection_handle: [u16; 8],
    }
}

#[cfg(test)]
mod tests {
    use bt_hci::cmd::{Cmd, CmdReturnBuf, SyncCmd};
    use bt_hci::{FromHciBytes, ReadHci, WriteHci};

    use super::*;

    fn encode(command: &impl WriteHci) -> ([u8; 16], usize) {
        let mut buffer = [0; 16];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 16 - writer.len();
        (buffer, len)
    }

    #[test]
    fn opcodes_come_from_the_catalog() {
        assert_eq!(HalToneStop::OPCODE.to_raw(), 0xFC16);
        assert_eq!(HalToneStart::OPCODE.to_raw(), 0xFC15);
        assert_eq!(HalSetRadioActivityMask::OPCODE.to_raw(), 0xFC18);
    }

    #[test]
    fn parameters_encode_in_catalog_order() {
        let (bytes, len) = encode(&HalToneStart::new(39, 2));
        assert_eq!(bytes[..len], [0x15, 0xFC, 2, 39, 2]);
        let (bytes, len) = encode(&HalSetRadioActivityMask::new(0x0102));
        assert_eq!(bytes[..len], [0x18, 0xFC, 2, 0x02, 0x01]);
        let (bytes, len) = encode(&HalToneStop::new());
        assert_eq!(bytes[..len], [0x16, 0xFC, 0]);
    }

    #[cfg(feature = "stack-full-extended")]
    #[test]
    fn profile_specific_commands_follow_the_catalog() {
        let (bytes, len) = encode(&HalSetEventMask::new(0x0403_0201));
        assert_eq!(bytes[..len], [0x1A, 0xFC, 4, 1, 2, 3, 4]);
    }

    /// Decode the return parameters bt-hci hands over after the status.
    fn decode<C: SyncCmd>(bytes: &[u8]) -> C::Return {
        C::Return::from_hci_bytes_complete(bytes).unwrap()
    }

    #[test]
    fn return_parameters_decode_after_the_status() {
        assert_eq!(HalGetAnchorPeriod::OPCODE.to_raw(), 0xFC19);
        let period = decode::<HalGetAnchorPeriod>(&[0x10, 0x27, 0, 0, 0xE8, 0x03, 0, 0]);
        assert_eq!(period.anchor_period, 10_000);
        assert_eq!(period.max_free_slot, 1_000);

        assert_eq!(decode::<HalReadRssi>(&[0xC4]).rssi, -60);

        assert!(<HalGetAnchorPeriod as SyncCmd>::Return::from_hci_bytes_complete(&[0; 7]).is_err());
    }

    #[cfg(feature = "stack-full-extended")]
    #[test]
    fn array_return_parameters_decode_element_wise() {
        let mut bytes = [0; 24];
        bytes[..8].copy_from_slice(&[1, 2, 0, 0, 0, 0, 0, 0]);
        bytes[8..12].copy_from_slice(&[0x01, 0x08, 0x02, 0x08]);
        let links = decode::<HalGetLinkStatus>(&bytes);
        assert_eq!(links.link_status[..2], [1, 2]);
        assert_eq!(links.link_connection_handle[..2], [0x0801, 0x0802]);
    }

    #[test]
    fn counted_return_parameters_read_their_count_first() {
        assert_eq!(HalReadConfigData::OPCODE.to_raw(), 0xFC0D);
        let config = decode::<HalReadConfigData>(&[3, 0xAA, 0xBB, 0xCC]);
        assert_eq!(config.data.as_slice(), [0xAA, 0xBB, 0xCC]);
        assert!(decode::<HalReadConfigData>(&[0]).data.is_empty());

        let mut too_long = [0; 252];
        too_long[0] = 251;
        let parse = <HalReadConfigData as SyncCmd>::Return::from_hci_bytes_complete;
        assert!(parse(&too_long).is_err(), "longer than the capacity");
        assert!(parse(&[3, 0xAA, 0xBB]).is_err(), "shorter than the count");
        assert!(parse(&[1, 0xAA, 0xBB]).is_err(), "longer than the count");
        assert_eq!(<HalReadConfigData as SyncCmd>::ReturnBuf::LEN, 251);
    }

    #[test]
    fn return_parameters_read_one_member_at_a_time() {
        let mut buffer = [0; 251];
        let config = <HalReadConfigData as SyncCmd>::Return::read_hci(
            &[2, 0xAA, 0xBB, 0xFF][..],
            &mut buffer,
        )
        .unwrap();
        assert_eq!(config.data.as_slice(), [0xAA, 0xBB]);

        let mut buffer = [0; 8];
        let period = <HalGetAnchorPeriod as SyncCmd>::Return::read_hci(
            &[1, 0, 0, 0, 2, 0, 0, 0][..],
            &mut buffer,
        )
        .unwrap();
        assert_eq!((period.anchor_period, period.max_free_slot), (1, 2));
        assert!(
            <HalGetAnchorPeriod as SyncCmd>::Return::read_hci(&[0; 8][..], &mut [0; 7]).is_err()
        );
    }

    #[tokio::test]
    async fn asynchronous_reads_match_synchronous() {
        let mut buffer = [0; 251];
        let config = <HalReadConfigData as SyncCmd>::Return::read_hci_async(
            &[2, 0xAA, 0xBB, 0xFF][..],
            &mut buffer,
        )
        .await
        .unwrap();
        assert_eq!(config.data.as_slice(), [0xAA, 0xBB]);
    }

    #[test]
    fn counts_are_written_from_the_counted_field() {
        let command = HalWriteConfigData::try_new(0x2E, &[1, 2, 3]).unwrap();
        assert_eq!(HalWriteConfigData::OPCODE.to_raw(), 0xFC0C);
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..len], [0x0C, 0xFC, 5, 0x2E, 3, 1, 2, 3]);

        assert_eq!(
            HalWriteConfigData::try_new(0, &[0; 254]),
            Err(crate::wire::TooLong {
                field: "value",
                len: 254,
                capacity: 253,
            })
        );
        assert!(HalWriteConfigData::try_new(0, &[0; 253]).is_ok());
    }

    #[tokio::test]
    async fn asynchronous_encoding_matches_synchronous() {
        let command = HalWriteConfigData::try_new(0x2E, &[1, 2, 3]).unwrap();
        let mut buffer = [0; 16];
        let mut writer = &mut buffer[..];
        command.write_hci_async(&mut writer).await.unwrap();
        let len = 16 - writer.len();
        let (expected, expected_len) = encode(&command);
        assert_eq!(buffer[..len], expected[..expected_len]);
    }
}
