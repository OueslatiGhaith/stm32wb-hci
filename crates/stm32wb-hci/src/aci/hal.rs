//! Hardware abstraction layer commands, in opcode order, and events, in code
//! order.
//!
//! The catalog also rules out commands other ST stacks define in this
//! group: no STM32WB wireless binary implements `aci_hal_get_link_status_v2`,
//! `aci_hal_set_sync_event_config`, or `aci_hal_continuous_tx_start`.

#[allow(unused_imports, reason = "some profiles do not report scan requests")]
use bt_hci::param::BdAddr;
use stm32wb_hci_macros::{vendor_command, vendor_event};

use crate::aci::flags::{HalEventMask, RadioActivityMask};
#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no warning event"
)]
use crate::aci::values::WarningType;
use crate::aci::values::{ConfigDataOffset, ReadableConfigDataOffset};
use crate::wire::BoundedBytes;
#[allow(
    unused_imports,
    reason = "the HCI-layer profiles have no warning event"
)]
use crate::wire::OrUnknown;

vendor_command! {
    /// Read the build number of the wireless stack.
    aci_hal_get_fw_build_number => HalGetFwBuildNumber {} -> HalFwBuildNumber {
        build_number: u16,
    }
}

vendor_command! {
    /// Write a value to the low-level configuration data at `offset`;
    /// [`entry`](Self::entry) also checks its length.
    aci_hal_write_config_data => HalWriteConfigData {
        offset: ConfigDataOffset,
        value: &'a [u8],
    }
}

impl<'a> HalWriteConfigData<'a> {
    /// Write `value` at `offset`, or `None` unless it is as long as the data
    /// the catalog documents there.
    pub fn entry(offset: ConfigDataOffset, value: &'a [u8]) -> Option<Self> {
        if value.len() != offset.length() {
            return None;
        }
        Self::try_new(offset, value).ok()
    }
}

vendor_command! {
    /// Read the low-level configuration data at `offset`.
    aci_hal_read_config_data => HalReadConfigData {
        offset: ReadableConfigDataOffset,
    } -> HalConfigData {
        data: BoundedBytes<250>,
    }
}

vendor_command! {
    /// Set the transmit power level. The level applies immediately and lasts
    /// until the next call or a reset; `en_high_power` is ignored on STM32WB.
    aci_hal_set_tx_power_level => HalSetTxPowerLevel {
        en_high_power: bool,
        pa_level: u8,
    }
}

vendor_command! {
    /// Read the number of packets sent by the last direct transmit test.
    aci_hal_le_tx_test_packet_number => HalLeTxTestPacketNumber {} -> HalTxTestPacketNumber {
        number_of_packets: u32,
    }
}

vendor_command! {
    /// Start transmitting a continuous tone on an RF channel (0 to 39), for
    /// debugging while no other radio activity is ongoing.
    aci_hal_tone_start => HalToneStart {
        rf_channel: u8,
        freq_offset: u8,
    }
}

vendor_command! {
    /// Stop a tone started with [`HalToneStart`].
    aci_hal_tone_stop => HalToneStop {}
}

vendor_command! {
    /// Read the state of each link and the connection handle it serves.
    aci_hal_get_link_status => HalGetLinkStatus {} -> HalLinkStatus {
        link_status: [u8; 8],
        link_connection_handle: [u16; 8],
    }
}

vendor_command! {
    /// Select the radio activities reported by the radio activity event.
    aci_hal_set_radio_activity_mask => HalSetRadioActivityMask {
        radio_activity_mask: RadioActivityMask,
    }
}

vendor_command! {
    /// Read the current anchor period and the largest free slot, in microseconds.
    aci_hal_get_anchor_period => HalGetAnchorPeriod {} -> HalAnchorPeriod {
        anchor_period: u32,
        max_free_slot: u32,
    }
}

vendor_command! {
    /// Enable or disable the HAL events.
    aci_hal_set_event_mask => HalSetEventMask {
        event_mask: HalEventMask,
    }
}

vendor_command! {
    /// Read how many buffers are allocated for ACL packets.
    aci_hal_get_pm_debug_info => HalGetPmDebugInfo {} -> HalPmDebugInfo {
        allocated_for_tx: u8,
        allocated_for_rx: u8,
        allocated_mblocks: u8,
    }
}

vendor_command! {
    /// Enable or disable peripheral latency on connections, which is enabled
    /// by default. Named `aci_hal_set_slave_latency` before 1.17.0.
    aci_hal_set_peripheral_latency => HalSetPeripheralLatency {
        enable: bool,
    }
}

vendor_command! {
    /// Read the RSSI of the last received packet, in dBm.
    aci_hal_read_rssi => HalReadRssi {} -> HalRssi {
        rssi: i8,
    }
}

vendor_command! {
    /// Encrypt (`mode` 0) or decrypt (`mode` 1) data with the Encrypted
    /// Advertising Data scheme.
    aci_hal_ead_encrypt_decrypt => HalEadEncryptDecrypt {
        mode: u8,
        key: [u8; 16],
        iv: [u8; 8],
        in_data: &'a [u8],
    } -> HalEadData {
        out_data: BoundedBytes<249>,
    }
}

vendor_command! {
    /// Read a register of the RF module.
    aci_hal_read_radio_reg => HalReadRadioReg { register_address: u8 } -> HalRadioReg {
        #[wire(name = "reg_val")]
        register_value: u8,
    }
}

vendor_command! {
    /// Write a register of the RF module.
    aci_hal_write_radio_reg => HalWriteRadioReg {
        register_address: u8,
        register_value: u8,
    }
}

vendor_command! {
    /// Read the raw RSSI value.
    aci_hal_read_raw_rssi => HalReadRawRssi {} -> HalRawRssi {
        value: [u8; 3],
    }
}

vendor_command! {
    /// Start receiving on an RF channel (0 to 39) until [`HalRxStop`].
    aci_hal_rx_start => HalRxStart {
        rf_channel: u8,
    }
}

vendor_command! {
    /// Stop receiving started with [`HalRxStart`].
    aci_hal_rx_stop => HalRxStop {}
}

vendor_command! {
    /// Reset the BLE stack, entering sleep mode as soon as it completes.
    aci_hal_stack_reset => HalStackReset {}
}

vendor_event! {
    /// The radio finished one activity and scheduled the next, as selected by
    /// [`HalSetRadioActivityMask`]. `next_state_sys_time` is in units of
    /// 625/256 µs. The code is 0x1804 from 1.24.0.
    aci_hal_end_of_radio_activity_event => HalEndOfRadioActivityEvent {
        last_state: u8,
        next_state: u8,
        next_state_sys_time: u32,
        last_state_slot: u8,
        next_state_slot: u8,
    }
}

vendor_event! {
    /// A peer sent a scan request, reported with its RSSI in dBm. The code is
    /// 0x1805 from 1.24.0.
    aci_hal_scan_req_report_event => HalScanReqReportEvent {
        rssi: i8,
        peer_address_type: u8,
        peer_address: BdAddr,
    }
}

vendor_event! {
    /// A warning from the wireless stack, with data depending on its type.
    /// Named `aci_hal_fw_error_event` before 1.22.0.
    aci_warning_event => HalWarningEvent {
        warning_type: OrUnknown<WarningType>,
        data: &'a [u8],
    }
}

#[cfg(test)]
mod tests {
    use bt_hci::cmd::{Cmd, CmdReturnBuf, SyncCmd};
    use bt_hci::{FromHciBytes, ReadHci, WriteHci};

    use super::*;
    use crate::wire::VendorEvent;

    fn encode(command: &impl WriteHci) -> ([u8; 64], usize) {
        let mut buffer = [0; 64];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 64 - writer.len();
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
        let (bytes, len) = encode(&HalSetRadioActivityMask::new(
            RadioActivityMask::ADVERTISING | RadioActivityMask::CENTRAL_CONNECTION,
        ));
        assert_eq!(bytes[..len], [0x18, 0xFC, 2, 0x22, 0x00]);
        let (bytes, len) = encode(&HalToneStop::new());
        assert_eq!(bytes[..len], [0x16, 0xFC, 0]);
    }

    #[cfg(feature = "stack-full-extended")]
    #[test]
    fn profile_specific_commands_follow_the_catalog() {
        let (bytes, len) = encode(&HalSetEventMask::new(HalEventMask::SCAN_REQUEST_REPORT));
        assert_eq!(bytes[..len], [0x1A, 0xFC, 4, 1, 0, 0, 0]);
        let (bytes, len) = encode(&HalWriteRadioReg::new(0x12, 0x34));
        assert_eq!(bytes[..len], [0x31, 0xFC, 2, 0x12, 0x34]);
        let (bytes, len) = encode(&HalRxStart::new(19));
        assert_eq!(bytes[..len], [0x33, 0xFC, 1, 19]);
        assert_eq!(HalRxStop::OPCODE.to_raw(), 0xFC34);
        assert_eq!(decode::<HalReadRadioReg>(&[0x5A]).register_value, 0x5A);
        assert_eq!(decode::<HalReadRawRssi>(&[1, 2, 3]).value, [1, 2, 3]);
    }

    #[test]
    fn flags_encode_as_one_byte() {
        let (bytes, len) = encode(&HalSetTxPowerLevel::new(true, 0x19));
        assert_eq!(bytes[..len], [0x0F, 0xFC, 2, 1, 0x19]);
    }

    #[cfg(not(feature = "stack-hci-adv-scan"))]
    #[test]
    fn renamed_commands_keep_their_opcode() {
        let (bytes, len) = encode(&HalSetPeripheralLatency::new(false));
        assert_eq!(bytes[..len], [0x20, 0xFC, 1, 0]);
        let count = decode::<HalLeTxTestPacketNumber>(&[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(count.number_of_packets, 0x0403_0201);
    }

    #[cfg(all(feature = "fw_1_24_0", feature = "stack-full-extended"))]
    #[test]
    fn two_byte_counts_encode_and_decode() {
        let command = HalEadEncryptDecrypt::try_new(0, [0xAA; 16], [0xBB; 8], &[1, 2, 3]).unwrap();
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..4], [0x2F, 0xFC, 30, 0]);
        assert_eq!(bytes[4..20], [0xAA; 16]);
        assert_eq!(bytes[20..28], [0xBB; 8]);
        assert_eq!(bytes[28..len], [3, 0, 1, 2, 3]);
        assert!(HalEadEncryptDecrypt::try_new(0, [0; 16], [0; 8], &[0; 229]).is_err());

        let data = decode::<HalEadEncryptDecrypt>(&[2, 0, 0xCC, 0xDD]);
        assert_eq!(data.out_data.as_slice(), [0xCC, 0xDD]);
        assert_eq!(<HalEadEncryptDecrypt as SyncCmd>::ReturnBuf::LEN, 2 + 249);
    }

    #[cfg(all(
        feature = "stack-full-extended",
        any(
            feature = "fw_1_15_0",
            feature = "fw_1_16_0",
            feature = "fw_1_17_0",
            feature = "fw_1_17_1",
            feature = "fw_1_17_2",
            feature = "fw_1_17_3",
            feature = "fw_1_18_0",
            feature = "fw_1_19_0",
            feature = "fw_1_19_1",
            feature = "fw_1_20_0",
            feature = "fw_1_21_0",
            feature = "fw_1_22_0",
            feature = "fw_1_22_1"
        )
    ))]
    #[test]
    fn retired_commands_exist_in_their_releases() {
        assert_eq!(
            decode::<HalGetFwBuildNumber>(&[0x34, 0x12]).build_number,
            0x1234
        );
        let info = decode::<HalGetPmDebugInfo>(&[1, 2, 3]);
        assert_eq!(
            (
                info.allocated_for_tx,
                info.allocated_for_rx,
                info.allocated_mblocks
            ),
            (1, 2, 3)
        );
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
        let command =
            HalWriteConfigData::try_new(ConfigDataOffset::StaticRandomAddress, &[1, 2, 3]).unwrap();
        assert_eq!(HalWriteConfigData::OPCODE.to_raw(), 0xFC0C);
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..len], [0x0C, 0xFC, 5, 0x2E, 3, 1, 2, 3]);

        assert_eq!(
            HalWriteConfigData::try_new(ConfigDataOffset::PublicAddress, &[0; 254]),
            Err(crate::wire::TooLong {
                field: "value",
                len: 254,
                capacity: 253,
            })
        );
        assert!(HalWriteConfigData::try_new(ConfigDataOffset::PublicAddress, &[0; 253]).is_ok());
    }

    #[test]
    fn entries_are_as_long_as_their_offset() {
        let address = [1, 2, 3, 4, 5, 6];
        let command =
            HalWriteConfigData::entry(ConfigDataOffset::StaticRandomAddress, &address).unwrap();
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..len], [0x0C, 0xFC, 8, 0x2E, 6, 1, 2, 3, 4, 5, 6]);
        assert!(HalWriteConfigData::entry(ConfigDataOffset::SmpMode, &address).is_none());
        assert!(HalWriteConfigData::entry(ConfigDataOffset::SmpMode, &[0x01]).is_some());
    }

    #[tokio::test]
    async fn asynchronous_encoding_matches_synchronous() {
        let command =
            HalWriteConfigData::try_new(ConfigDataOffset::StaticRandomAddress, &[1, 2, 3]).unwrap();
        let mut buffer = [0; 16];
        let mut writer = &mut buffer[..];
        command.write_hci_async(&mut writer).await.unwrap();
        let len = 16 - writer.len();
        let (expected, expected_len) = encode(&command);
        assert_eq!(buffer[..len], expected[..expected_len]);
    }

    #[test]
    fn events_decode_after_their_code() {
        let code = if cfg!(feature = "fw_1_24_0") {
            0x1804
        } else {
            0x0004
        };
        assert_eq!(HalEndOfRadioActivityEvent::CODE, code);
        let [low, high] = code.to_le_bytes();
        let params = [low, high, 1, 2, 0x10, 0x20, 0x30, 0x40, 3, 4];
        let event = HalEndOfRadioActivityEvent::from_vendor_params(&params)
            .unwrap()
            .unwrap();
        assert_eq!(
            event,
            HalEndOfRadioActivityEvent {
                last_state: 1,
                next_state: 2,
                next_state_sys_time: 0x4030_2010,
                last_state_slot: 3,
                next_state_slot: 4,
            }
        );
        assert!(HalEndOfRadioActivityEvent::from_vendor_params(&[low ^ 1, high, 0]).is_none());

        let mut packet = [0xFF, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        packet[2..].copy_from_slice(&params);
        let bt_hci::event::Event::Vendor(vendor) =
            bt_hci::event::Event::from_hci_bytes_complete(&packet).unwrap()
        else {
            panic!("0xFF is the vendor event code");
        };
        assert_eq!(
            HalEndOfRadioActivityEvent::from_vendor(&vendor)
                .unwrap()
                .unwrap(),
            event
        );
        assert!(
            HalEndOfRadioActivityEvent::from_vendor_params(&params[..9])
                .unwrap()
                .is_err()
        );
        let mut longer = [0; 11];
        longer[..10].copy_from_slice(&params);
        assert!(
            HalEndOfRadioActivityEvent::from_vendor_params(&longer)
                .unwrap()
                .is_err()
        );
    }

    #[cfg(any(
        feature = "stack-full-extended",
        feature = "stack-full",
        feature = "stack-light"
    ))]
    #[test]
    fn counted_event_data_borrows_the_event() {
        let params = [0x06, 0x00, 0x03, 3, 0xAA, 0xBB, 0xCC];
        let warning = HalWarningEvent::from_vendor_params(&params)
            .unwrap()
            .unwrap();
        assert_eq!(
            warning.warning_type,
            OrUnknown::Known(WarningType::NvmAlmostFull)
        );
        assert_eq!(warning.data, [0xAA, 0xBB, 0xCC]);
        assert!(
            HalWarningEvent::from_vendor_params(&params[..6])
                .unwrap()
                .is_err()
        );
    }

    #[cfg(feature = "stack-full-extended")]
    #[test]
    fn scan_requests_report_the_peer() {
        let report =
            HalScanReqReportEvent::from_hci_bytes_complete(&[0xC4, 1, 1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!(report.rssi, -60);
        assert_eq!(report.peer_address, BdAddr::new([1, 2, 3, 4, 5, 6]));
    }
}
