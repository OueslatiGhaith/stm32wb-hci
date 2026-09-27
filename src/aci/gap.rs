//! GAP commands, in opcode order.

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
    /// Stop advertising.
    aci_gap_set_non_discoverable => GapSetNonDiscoverable {}
}

vendor_command! {
    /// Advertise in limited discoverable mode for about 180 seconds, with the
    /// local name and service UUIDs in the advertising data.
    aci_gap_set_limited_discoverable => GapSetLimitedDiscoverable {
        advertising_type: u8,
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        own_address_type: u8,
        advertising_filter_policy: u8,
        local_name: &'a [u8],
        service_uuid_list: &'a [u8],
        conn_interval_min: u16,
        conn_interval_max: u16,
    }
}

vendor_command! {
    /// Advertise in general discoverable mode, with the local name and service
    /// UUIDs in the advertising data.
    aci_gap_set_discoverable => GapSetDiscoverable {
        advertising_type: u8,
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        own_address_type: u8,
        advertising_filter_policy: u8,
        local_name: &'a [u8],
        service_uuid_list: &'a [u8],
        conn_interval_min: u16,
        conn_interval_max: u16,
    }
}

vendor_command! {
    /// Advertise to a single peer with directed connectable advertising.
    aci_gap_set_direct_connectable => GapSetDirectConnectable {
        own_address_type: u8,
        directed_advertising_type: u8,
        direct_address_type: u8,
        direct_address: BdAddr,
        advertising_interval_min: u16,
        advertising_interval_max: u16,
    }
}

vendor_command! {
    /// Advertise without accepting connections.
    aci_gap_set_non_connectable => GapSetNonConnectable {
        advertising_event_type: u8,
        own_address_type: u8,
    }
}

vendor_command! {
    /// Advertise connectable, undirected, without the discoverable flags.
    aci_gap_set_undirected_connectable => GapSetUndirectedConnectable {
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        own_address_type: u8,
        adv_filter_policy: u8,
    }
}

vendor_command! {
    /// Add or replace AD structures in the advertising data.
    aci_gap_update_adv_data => GapUpdateAdvData {
        adv_data: &'a [u8],
    }
}

vendor_command! {
    /// Remove the AD structure of type `ad_type` from the advertising data.
    aci_gap_delete_ad_type => GapDeleteAdType {
        #[wire(name = "ADType")]
        ad_type: u8,
    }
}

vendor_command! {
    /// Scan for devices in limited discoverable mode.
    aci_gap_start_limited_discovery_proc => GapStartLimitedDiscoveryProc {
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: u8,
        filter_duplicates: bool,
    }
}

vendor_command! {
    /// Scan for devices in general or limited discoverable mode.
    aci_gap_start_general_discovery_proc => GapStartGeneralDiscoveryProc {
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: u8,
        filter_duplicates: bool,
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
    /// Broadcast `adv_data` without accepting connections, answering scan
    /// requests only from the listed peers.
    aci_gap_set_broadcast_mode => GapSetBroadcastMode {
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        advertising_type: u8,
        own_address_type: u8,
        adv_data: &'a [u8],
        #[wire(before = "1.17.0")]
        whitelist_entry: &'a [WhitelistEntry],
        #[wire(since = "1.17.0")]
        peer_entry: &'a [PeerEntry],
    }
}

vendor_command! {
    /// Scan and report every advertising packet, without GAP filtering.
    aci_gap_start_observation_proc => GapStartObservationProc {
        le_scan_interval: u16,
        le_scan_window: u16,
        le_scan_type: u8,
        own_address_type: u8,
        filter_duplicates: bool,
        scanning_filter_policy: u8,
    }
}

vendor_command! {
    /// Read the addresses of the bonded devices.
    aci_gap_get_bonded_devices => GapGetBondedDevices {} -> GapBondedDevices {
        bonded_device_entry: BoundedArray<BondedDeviceEntry, 35>,
    }
}

vendor_command! {
    /// Start an additional non-connectable beacon, alongside the advertising
    /// the other GAP commands control.
    aci_gap_additional_beacon_start => GapAdditionalBeaconStart {
        adv_interval_min: u16,
        adv_interval_max: u16,
        adv_channel_map: u8,
        own_address_type: u8,
        own_address: BdAddr,
        pa_level: u8,
    }
}

vendor_command! {
    /// Stop the beacon started with [`GapAdditionalBeaconStart`].
    aci_gap_additional_beacon_stop => GapAdditionalBeaconStop {}
}

vendor_command! {
    /// Set the advertising data of the additional beacon.
    aci_gap_additional_beacon_set_data => GapAdditionalBeaconSetData {
        adv_data: &'a [u8],
    }
}

vendor_command! {
    /// Configure the advertising set `advertising_handle`.
    aci_gap_adv_set_configuration => GapAdvSetConfiguration {
        adv_mode: u8,
        advertising_handle: u8,
        adv_event_properties: u16,
        primary_adv_interval_min: u32,
        primary_adv_interval_max: u32,
        primary_adv_channel_map: u8,
        own_address_type: u8,
        peer_address_type: u8,
        peer_address: BdAddr,
        adv_filter_policy: u8,
        adv_tx_power: i8,
        secondary_adv_max_skip: u8,
        secondary_adv_phy: u8,
        adv_sid: u8,
        scan_req_notification_enable: bool,
    }
}

vendor_struct! {
    /// An advertising set to enable, and when to stop it.
    Adv_Set_t => AdvSet {
        advertising_handle: u8,
        duration: u16,
        max_extended_advertising_events: u8,
    }
}

vendor_command! {
    /// Enable or disable the listed advertising sets.
    aci_gap_adv_set_enable => GapAdvSetEnable {
        enable: bool,
        adv_set: &'a [AdvSet],
    }
}

vendor_command! {
    /// Set the advertising data of an advertising set, or a fragment of it.
    aci_gap_adv_set_adv_data => GapAdvSetAdvData {
        advertising_handle: u8,
        operation: u8,
        fragment_preference: u8,
        advertising_data: &'a [u8],
    }
}

vendor_command! {
    /// Set the scan response data of an advertising set, or a fragment of it.
    aci_gap_adv_set_scan_resp_data => GapAdvSetScanRespData {
        advertising_handle: u8,
        operation: u8,
        fragment_preference: u8,
        scan_response_data: &'a [u8],
    }
}

vendor_command! {
    /// Remove an advertising set.
    aci_gap_adv_remove_set => GapAdvRemoveSet {
        advertising_handle: u8,
    }
}

vendor_command! {
    /// Remove every advertising set.
    aci_gap_adv_clear_sets => GapAdvClearSets {}
}

vendor_command! {
    /// Set the random address of an advertising set.
    aci_gap_adv_set_random_address => GapAdvSetRandomAddress {
        advertising_handle: u8,
        random_address: BdAddr,
    }
}

vendor_struct! {
    /// Scan parameters for one PHY.
    Scan_Param_Phy_t => ScanParamPhy {
        scan_type: u8,
        scan_interval: u16,
        scan_window: u16,
    }
}

vendor_command! {
    /// Start an extended scan or discovery procedure, with the parameters of
    /// the LE 1M and LE Coded PHYs the `scanning_phys` bits select.
    aci_gap_ext_start_scan => GapExtStartScan {
        scan_mode: u8,
        procedure: u8,
        own_address_type: u8,
        filter_duplicates: u8,
        duration: u16,
        period: u16,
        scanning_filter_policy: u8,
        scanning_phys: u8,
        scan_param_phy: [ScanParamPhy; 2],
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

    #[test]
    fn two_names_fit_their_capacities_but_not_the_parameters_together() {
        let command =
            GapSetDiscoverable::try_new(0, 0x20, 0x30, 0, 0, b"ab", &[0x02, 0x0A, 0x18], 6, 7)
                .unwrap();
        assert_eq!(GapSetDiscoverable::OPCODE.to_raw(), 0xFC83);
        let (bytes, len) = encode(&command);
        assert_eq!(
            bytes[..len],
            [
                0x83, 0xFC, 18, 0, 0x20, 0, 0x30, 0, 0, 0, 2, b'a', b'b', 3, 0x02, 0x0A, 0x18, 6,
                0, 7, 0
            ]
        );
        let name = [0; 242];
        let error =
            GapSetDiscoverable::try_new(0, 0x20, 0x30, 0, 0, &name, &name[..10], 6, 7).unwrap_err();
        assert_eq!(error.field, "parameters");
        assert_eq!(error.len, 265);
        assert!(GapSetDiscoverable::try_new(0, 0x20, 0x30, 0, 0, &name, &[], 6, 7).is_ok());
    }

    #[cfg(not(any(
        feature = "fw_1_15_0",
        feature = "fw_1_16_0",
        feature = "fw_1_17_0",
        feature = "fw_1_17_1",
        feature = "fw_1_17_2",
        feature = "fw_1_17_3"
    )))]
    #[test]
    fn fixed_structure_arrays_encode_element_by_element() {
        let phy = |scan_type, scan_interval, scan_window| ScanParamPhy {
            scan_type,
            scan_interval,
            scan_window,
        };
        let command = GapExtStartScan::new(
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0b101,
            [phy(1, 0x10, 0x20), phy(0, 0x30, 0x40)],
        );
        assert_eq!(GapExtStartScan::OPCODE.to_raw(), 0xFCD0);
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..3], [0xD0, 0xFC, 10 + 10]);
        assert_eq!(bytes[3..13], [0, 1, 0, 0, 0, 0, 0, 0, 0, 0b101]);
        assert_eq!(bytes[13..len], [1, 0x10, 0, 0x20, 0, 0, 0x30, 0, 0x40, 0]);
    }

    #[test]
    fn advertising_sets_encode_their_count() {
        let sets = [AdvSet {
            advertising_handle: 3,
            duration: 0x0102,
            max_extended_advertising_events: 4,
        }];
        let (bytes, len) = encode(&GapAdvSetEnable::try_new(true, &sets).unwrap());
        assert_eq!(bytes[..len], [0xC1, 0xFC, 6, 1, 1, 3, 0x02, 0x01, 4]);
        assert!(GapAdvSetEnable::try_new(true, &[sets[0]; 64]).is_err());
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
