//! GAP commands.

#[allow(unused_imports, reason = "the HCI-layer profiles have no GAP commands")]
use bt_hci::param::BdAddr;
use stm32wb_hci_macros::{vendor_command, vendor_struct};

#[allow(unused_imports, reason = "the HCI-layer profiles have no GAP commands")]
use crate::wire::BoundedArray;

vendor_struct! {
    /// A peer device, before 1.17.0 renamed it [`PeerEntry`].
    Whitelist_Entry_t => WhitelistEntry {
        peer_address_type: u8,
        peer_address: BdAddr,
    }
}

vendor_struct! {
    /// A peer device, named [`WhitelistEntry`] before 1.17.0.
    Peer_Entry_t => PeerEntry {
        peer_address_type: u8,
        peer_address: BdAddr,
    }
}

vendor_struct! {
    /// A bonded device.
    Bonded_Device_Entry_t => BondedDeviceEntry {
        address_type: u8,
        address: BdAddr,
    }
}

vendor_command! {
    /// Scan for the listed peers and connect to the one the host selects.
    aci_gap_start_selective_connection_establish_proc => GapStartSelectiveConnectionEstablishProc {
        le_scan_type: u8,
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: u8,
        scanning_filter_policy: u8,
        filter_duplicates: bool,
        #[wire(before = "1.17.0")]
        whitelist_entry: &'a [WhitelistEntry],
        #[wire(since = "1.17.0")]
        peer_entry: &'a [PeerEntry],
    }
}

vendor_command! {
    /// Read the addresses of the bonded devices.
    aci_gap_get_bonded_devices => GapGetBondedDevices {} -> GapBondedDevices {
        bonded_device_entry: BoundedArray<BondedDeviceEntry, 35>,
    }
}

#[cfg(all(test, feature = "stack-full-extended"))]
mod tests {
    use bt_hci::cmd::{Cmd, CmdReturnBuf, SyncCmd};
    use bt_hci::{FromHciBytes, WriteHci};

    use super::*;

    fn encode(command: &impl WriteHci) -> ([u8; 300], usize) {
        let mut buffer = [0; 300];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 300 - writer.len();
        (buffer, len)
    }

    #[cfg(not(any(feature = "fw_1_15_0", feature = "fw_1_16_0")))]
    fn peers() -> [PeerEntry; 2] {
        [
            PeerEntry {
                peer_address_type: 0,
                peer_address: BdAddr::new([1, 2, 3, 4, 5, 6]),
            },
            PeerEntry {
                peer_address_type: 1,
                peer_address: BdAddr::new([7, 8, 9, 10, 11, 12]),
            },
        ]
    }

    #[cfg(any(feature = "fw_1_15_0", feature = "fw_1_16_0"))]
    fn peers() -> [WhitelistEntry; 2] {
        [
            WhitelistEntry {
                peer_address_type: 0,
                peer_address: BdAddr::new([1, 2, 3, 4, 5, 6]),
            },
            WhitelistEntry {
                peer_address_type: 1,
                peer_address: BdAddr::new([7, 8, 9, 10, 11, 12]),
            },
        ]
    }

    #[test]
    fn counted_structures_encode_element_by_element() {
        let peers = peers();
        let command =
            GapStartSelectiveConnectionEstablishProc::try_new(1, 0x10, 0x10, 0, 0, true, &peers)
                .unwrap();
        assert_eq!(
            GapStartSelectiveConnectionEstablishProc::OPCODE.to_raw(),
            0xFC9B
        );
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..3], [0x9B, 0xFC, 9 + 14]);
        assert_eq!(bytes[3..12], [1, 0x10, 0, 0x10, 0, 0, 0, 1, 2]);
        assert_eq!(
            bytes[12..len],
            [0, 1, 2, 3, 4, 5, 6, 1, 7, 8, 9, 10, 11, 12]
        );
        let too_many = [peers[0]; 36];
        assert!(
            GapStartSelectiveConnectionEstablishProc::try_new(1, 0x10, 0x10, 0, 0, true, &too_many)
                .is_err()
        );
    }

    #[test]
    fn counted_return_structures_decode_element_by_element() {
        let devices = <GapGetBondedDevices as SyncCmd>::Return::from_hci_bytes_complete(&[
            2, 0, 1, 2, 3, 4, 5, 6, 1, 7, 8, 9, 10, 11, 12,
        ])
        .unwrap();
        assert_eq!(
            devices.bonded_device_entry.as_slice(),
            [
                BondedDeviceEntry {
                    address_type: 0,
                    address: BdAddr::new([1, 2, 3, 4, 5, 6]),
                },
                BondedDeviceEntry {
                    address_type: 1,
                    address: BdAddr::new([7, 8, 9, 10, 11, 12]),
                },
            ]
        );
        assert!(
            <GapGetBondedDevices as SyncCmd>::Return::from_hci_bytes_complete(&[2, 0, 1, 2])
                .is_err()
        );
        assert_eq!(<GapGetBondedDevices as SyncCmd>::ReturnBuf::LEN, 1 + 35 * 7);
    }
}
