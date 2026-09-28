//! GAP commands, in opcode order, and events, in code order.

#[allow(unused_imports, reason = "the HCI-layer profiles have no GAP commands")]
use bt_hci::param::{BdAddr, ConnHandle};
use stm32wb_hci_macros::{vendor_command, vendor_event, vendor_struct};

#[allow(unused_imports, reason = "the HCI-layer profiles have no GAP commands")]
use crate::aci::flags::{
    AdvChannelMap, AdvEventProperties, AdvMode, GapEventMask, InitiatingPhys, Role, ScanningPhys,
};
#[allow(unused_imports, reason = "the HCI-layer profiles have no GAP commands")]
use crate::aci::values::{
    AddressType, AdvertisingType, ConnectableOwnAddressType, IoCapability,
    NonConnectableAdvertisingType, OwnAddressType, Privacy, ScanType,
};
#[allow(unused_imports, reason = "the HCI-layer profiles have no GAP commands")]
use crate::wire::BoundedArray;

vendor_command! {
    /// Stop advertising.
    aci_gap_set_non_discoverable => GapSetNonDiscoverable {}
}

vendor_command! {
    /// Advertise in limited discoverable mode for about 180 seconds, with the
    /// local name and service UUIDs in the advertising data.
    aci_gap_set_limited_discoverable => GapSetLimitedDiscoverable {
        advertising_type: AdvertisingType,
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        own_address_type: OwnAddressType,
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
        advertising_type: AdvertisingType,
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        own_address_type: OwnAddressType,
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
        own_address_type: ConnectableOwnAddressType,
        directed_advertising_type: u8,
        direct_address_type: AddressType,
        direct_address: BdAddr,
        advertising_interval_min: u16,
        advertising_interval_max: u16,
    }
}

vendor_command! {
    /// Set the input and output capabilities used in pairing.
    aci_gap_set_io_capability => GapSetIoCapability {
        io_capability: IoCapability,
    }
}

vendor_command! {
    /// Set the bonding, MITM protection, Secure Connections, and key size
    /// requirements of pairing, and the fixed passkey if `use_fixed_pin` is 0.
    aci_gap_set_authentication_requirement => GapSetAuthenticationRequirement {
        bonding_mode: bool,
        mitm_mode: bool,
        sc_support: u8,
        key_press_notification_support: bool,
        min_encryption_key_size: u8,
        max_encryption_key_size: u8,
        use_fixed_pin: u8,
        fixed_pin: u32,
        identity_address_type: AddressType,
    }
}

vendor_command! {
    /// Require the host to authorize the peer before it accesses attributes.
    aci_gap_set_authorization_requirement => GapSetAuthorizationRequirement {
        connection_handle: ConnHandle,
        authorization_enable: bool,
    }
}

vendor_command! {
    /// Answer a passkey request with the passkey, 0 to 999999.
    aci_gap_pass_key_resp => GapPassKeyResp {
        connection_handle: ConnHandle,
        pass_key: u32,
    }
}

vendor_command! {
    /// Answer an authorization request: 1 authorizes, 2 rejects.
    aci_gap_authorization_resp => GapAuthorizationResp {
        connection_handle: ConnHandle,
        authorize: u8,
    }
}

vendor_command! {
    /// Initialize GAP in the given roles, adding the GAP service and its
    /// characteristics.
    aci_gap_init => GapInit {
        role: Role,
        privacy_enabled: Privacy,
        device_name_char_len: u8,
    } -> GapService {
        service_handle: u16,
        dev_name_char_handle: u16,
        appearance_char_handle: u16,
    }
}

vendor_command! {
    /// Advertise without accepting connections.
    aci_gap_set_non_connectable => GapSetNonConnectable {
        advertising_event_type: NonConnectableAdvertisingType,
        own_address_type: OwnAddressType,
    }
}

vendor_command! {
    /// Advertise connectable, undirected, without the discoverable flags.
    aci_gap_set_undirected_connectable => GapSetUndirectedConnectable {
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        own_address_type: ConnectableOwnAddressType,
        adv_filter_policy: u8,
    }
}

vendor_command! {
    /// Ask the central to start security. Named `aci_gap_slave_security_req`
    /// in earlier releases.
    aci_gap_peripheral_security_req => GapPeripheralSecurityReq {
        connection_handle: ConnHandle,
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
    /// Read the security mode and level of a connection.
    aci_gap_get_security_level => GapGetSecurityLevel {
        connection_handle: ConnHandle,
    } -> GapSecurityLevel {
        security_mode: u8,
        security_level: u8,
    }
}

vendor_command! {
    /// Enable or disable the GAP events.
    aci_gap_set_event_mask => GapSetEventMask {
        gap_evt_mask: GapEventMask,
    }
}

vendor_command! {
    /// Put the addresses of the bonded devices in the controller's filter
    /// accept list. Named `aci_gap_configure_whitelist` in earlier releases.
    aci_gap_configure_filter_accept_list => GapConfigureFilterAcceptList {}
}

vendor_command! {
    /// Terminate a connection.
    aci_gap_terminate => GapTerminate {
        connection_handle: ConnHandle,
        reason: u8,
    }
}

vendor_command! {
    /// Remove every bonded device from the security database.
    aci_gap_clear_security_db => GapClearSecurityDb {}
}

vendor_command! {
    /// Allow pairing again with a bonded peer that lost its keys.
    aci_gap_allow_rebond => GapAllowRebond {
        connection_handle: ConnHandle,
    }
}

vendor_command! {
    /// Scan for devices in limited discoverable mode.
    aci_gap_start_limited_discovery_proc => GapStartLimitedDiscoveryProc {
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: OwnAddressType,
        filter_duplicates: bool,
    }
}

vendor_command! {
    /// Scan for devices in general or limited discoverable mode.
    aci_gap_start_general_discovery_proc => GapStartGeneralDiscoveryProc {
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: OwnAddressType,
        filter_duplicates: bool,
    }
}

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

vendor_command! {
    /// Connect to any of the listed peers as soon as it advertises.
    aci_gap_start_auto_connection_establish_proc => GapStartAutoConnectionEstablishProc {
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: ConnectableOwnAddressType,
        conn_interval_min: u16,
        conn_interval_max: u16,
        conn_latency: u16,
        supervision_timeout: u16,
        minimum_ce_length: u16,
        maximum_ce_length: u16,
        #[wire(before = "1.17.0")]
        whitelist_entry: &'a [WhitelistEntry],
        #[wire(since = "1.17.0")]
        peer_entry: &'a [PeerEntry],
    }
}

vendor_command! {
    /// Scan for connectable peers, reporting them for the host to select one.
    aci_gap_start_general_connection_establish_proc => GapStartGeneralConnectionEstablishProc {
        le_scan_type: ScanType,
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: OwnAddressType,
        scanning_filter_policy: u8,
        filter_duplicates: bool,
    }
}

vendor_command! {
    /// Scan for the listed peers and connect to the one the host selects.
    aci_gap_start_selective_connection_establish_proc => GapStartSelectiveConnectionEstablishProc {
        le_scan_type: ScanType,
        le_scan_interval: u16,
        le_scan_window: u16,
        own_address_type: OwnAddressType,
        scanning_filter_policy: u8,
        filter_duplicates: bool,
        #[wire(before = "1.17.0")]
        whitelist_entry: &'a [WhitelistEntry],
        #[wire(since = "1.17.0")]
        peer_entry: &'a [PeerEntry],
    }
}

vendor_command! {
    /// Connect to a peer.
    aci_gap_create_connection => GapCreateConnection {
        le_scan_interval: u16,
        le_scan_window: u16,
        peer_address_type: AddressType,
        peer_address: BdAddr,
        own_address_type: ConnectableOwnAddressType,
        conn_interval_min: u16,
        conn_interval_max: u16,
        conn_latency: u16,
        supervision_timeout: u16,
        minimum_ce_length: u16,
        maximum_ce_length: u16,
    }
}

vendor_command! {
    /// Terminate the GAP procedure `procedure_code`.
    aci_gap_terminate_gap_proc => GapTerminateGapProc {
        procedure_code: u8,
    }
}

vendor_command! {
    /// Update the parameters of a connection, as central.
    aci_gap_start_connection_update => GapStartConnectionUpdate {
        connection_handle: ConnHandle,
        conn_interval_min: u16,
        conn_interval_max: u16,
        conn_latency: u16,
        supervision_timeout: u16,
        minimum_ce_length: u16,
        maximum_ce_length: u16,
    }
}

vendor_command! {
    /// Start pairing, as central.
    aci_gap_send_pairing_req => GapSendPairingReq {
        connection_handle: ConnHandle,
        force_rebond: bool,
    }
}

vendor_command! {
    /// Resolve a private address with the stored identity resolving keys.
    aci_gap_resolve_private_addr => GapResolvePrivateAddr {
        address: BdAddr,
    } -> GapResolvedAddr {
        actual_address: BdAddr,
    }
}

vendor_command! {
    /// Broadcast `adv_data` without accepting connections, answering scan
    /// requests only from the listed peers.
    aci_gap_set_broadcast_mode => GapSetBroadcastMode {
        advertising_interval_min: u16,
        advertising_interval_max: u16,
        advertising_type: NonConnectableAdvertisingType,
        own_address_type: OwnAddressType,
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
        le_scan_type: ScanType,
        own_address_type: OwnAddressType,
        filter_duplicates: bool,
        scanning_filter_policy: u8,
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
    /// Read the addresses of the bonded devices.
    aci_gap_get_bonded_devices => GapGetBondedDevices {} -> GapBondedDevices {
        bonded_device_entry: BoundedArray<BondedDeviceEntry, 35>,
    }
}

vendor_command! {
    /// Check whether a peer is bonded. From 1.22.0 it also returns the
    /// peer's identity address. Named `aci_gap_is_device_bonded` in earlier
    /// releases.
    aci_gap_check_bonded_device => GapCheckBondedDevice {
        peer_address_type: AddressType,
        peer_address: BdAddr,
    } -> GapBondedIdentity {
        #[wire(since = "1.22.0")]
        id_address_type: u8,
        #[wire(since = "1.22.0")]
        id_address: BdAddr,
    }
}

vendor_command! {
    /// Confirm or reject the numeric comparison value.
    aci_gap_numeric_comparison_value_confirm_yesno => GapNumericComparisonValueConfirmYesno {
        connection_handle: ConnHandle,
        confirm_yes_no: bool,
    }
}

vendor_command! {
    /// Report a keypress while the user enters the passkey.
    aci_gap_passkey_input => GapPasskeyInput {
        connection_handle: ConnHandle,
        input_type: u8,
    }
}

vendor_command! {
    /// Read the local out-of-band pairing data of the given type.
    aci_gap_get_oob_data => GapGetOobData {
        oob_data_type: u8,
    } -> GapOobData {
        address_type: u8,
        address: BdAddr,
        oob_data_type: u8,
        oob_data_len: u8,
        oob_data: [u8; 16],
    }
}

vendor_command! {
    /// Set the local or remote out-of-band pairing data of the given type.
    aci_gap_set_oob_data => GapSetOobData {
        device_type: u8,
        address_type: AddressType,
        address: BdAddr,
        oob_data_type: u8,
        oob_data_len: u8,
        oob_data: [u8; 16],
    }
}

vendor_struct! {
    /// A peer identity address, before 1.17.0 renamed it [`IdentityEntry`].
    Whitelist_Identity_Entry_t => WhitelistIdentityEntry {
        peer_identity_address_type: u8,
        peer_identity_address: BdAddr,
    }
}

vendor_struct! {
    /// A peer identity address, named [`WhitelistIdentityEntry`] before 1.17.0.
    Identity_Entry_t => IdentityEntry {
        peer_identity_address_type: u8,
        peer_identity_address: BdAddr,
    }
}

vendor_command! {
    /// Add bonded devices to the controller's resolving list, clearing it
    /// first if `clear_resolving_list` is set.
    aci_gap_add_devices_to_resolving_list => GapAddDevicesToResolvingList {
        #[wire(before = "1.17.0")]
        whitelist_identity_entry: &'a [WhitelistIdentityEntry],
        #[wire(since = "1.17.0")]
        identity_entry: &'a [IdentityEntry],
        clear_resolving_list: bool,
    }
}

vendor_command! {
    /// Remove a bonded device from the security database.
    aci_gap_remove_bonded_device => GapRemoveBondedDevice {
        peer_identity_address_type: AddressType,
        peer_identity_address: BdAddr,
    }
}

vendor_struct! {
    /// A device address.
    List_Entry_t => ListEntry {
        address_type: u8,
        address: BdAddr,
    }
}

vendor_command! {
    /// Add devices to the controller's filter accept list, resolving list, or
    /// both, as `mode` selects. Before 1.17.0 the list is raw bytes.
    aci_gap_add_devices_to_list => GapAddDevicesToList {
        #[wire(before = "1.17.0")]
        list_entry: &'a [u8],
        #[wire(since = "1.17.0")]
        list_entry: &'a [ListEntry],
        mode: u8,
    }
}

vendor_command! {
    /// Accept or reject the pairing request of a peer.
    aci_gap_pairing_request_reply => GapPairingRequestReply {
        connection_handle: ConnHandle,
        accept: bool,
    }
}

vendor_command! {
    /// Start an additional non-connectable beacon, alongside the advertising
    /// the other GAP commands control.
    aci_gap_additional_beacon_start => GapAdditionalBeaconStart {
        adv_interval_min: u16,
        adv_interval_max: u16,
        adv_channel_map: AdvChannelMap,
        own_address_type: AddressType,
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
        adv_mode: AdvMode,
        advertising_handle: u8,
        adv_event_properties: AdvEventProperties,
        primary_adv_interval_min: u32,
        primary_adv_interval_max: u32,
        primary_adv_channel_map: AdvChannelMap,
        own_address_type: OwnAddressType,
        peer_address_type: AddressType,
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
        own_address_type: OwnAddressType,
        filter_duplicates: u8,
        duration: u16,
        period: u16,
        scanning_filter_policy: u8,
        scanning_phys: ScanningPhys,
        scan_param_phy: [ScanParamPhy; 2],
    }
}

vendor_struct! {
    /// Connection parameters for one PHY.
    Init_Param_Phy_t => InitParamPhy {
        scan_interval: u16,
        scan_window: u16,
        conn_interval_min: u16,
        conn_interval_max: u16,
        conn_latency: u16,
        supervision_timeout: u16,
        min_ce_length: u16,
        max_ce_length: u16,
    }
}

vendor_command! {
    /// Connect to a peer with extended scanning, with the parameters of the
    /// LE 1M, LE 2M, and LE Coded PHYs the `initiating_phys` bits select.
    aci_gap_ext_create_connection => GapExtCreateConnection {
        initiating_mode: u8,
        procedure: u8,
        own_address_type: ConnectableOwnAddressType,
        peer_address_type: AddressType,
        peer_address: BdAddr,
        advertising_handle: u8,
        subevent: u8,
        initiator_filter_policy: u8,
        initiating_phys: InitiatingPhys,
        init_param_phy: [InitParamPhy; 3],
    }
}

vendor_event! {
    /// Limited discoverable mode ended after its 180 seconds.
    aci_gap_limited_discoverable_event => GapLimitedDiscoverableEvent {}
}

vendor_event! {
    /// Pairing completed, failed (`status`, with `reason` the SMP error), or
    /// timed out.
    aci_gap_pairing_complete_event => GapPairingCompleteEvent {
        connection_handle: ConnHandle,
        status: u8,
        reason: u8,
    }
}

vendor_event! {
    /// Pairing needs a passkey; answer with [`GapPassKeyResp`].
    aci_gap_pass_key_req_event => GapPassKeyReqEvent {
        connection_handle: ConnHandle,
    }
}

vendor_event! {
    /// A peer needs authorization; answer with [`GapAuthorizationResp`].
    aci_gap_authorization_req_event => GapAuthorizationReqEvent {
        connection_handle: ConnHandle,
    }
}

vendor_event! {
    /// The security request sent with [`GapPeripheralSecurityReq`] went out.
    /// Named `aci_gap_slave_security_initiated_event` in earlier releases.
    aci_gap_peripheral_security_initiated_event => GapPeripheralSecurityInitiatedEvent {}
}

vendor_event! {
    /// A bonded peer asked to pair again, having lost its keys; allow it with
    /// [`GapAllowRebond`]. From 1.22.0 the event names the connection.
    aci_gap_bond_lost_event => GapBondLostEvent {
        #[wire(since = "1.22.0")]
        connection_handle: ConnHandle,
    }
}

vendor_event! {
    /// A GAP procedure completed, with data depending on the procedure.
    aci_gap_proc_complete_event => GapProcCompleteEvent {
        procedure_code: u8,
        status: u8,
        data: &'a [u8],
    }
}

vendor_event! {
    /// The resolving list has no key for the peer of a connection.
    aci_gap_addr_not_resolved_event => GapAddrNotResolvedEvent {
        connection_handle: ConnHandle,
    }
}

vendor_event! {
    /// Pairing needs the user to confirm `numeric_value`; answer with
    /// [`GapNumericComparisonValueConfirmYesno`].
    aci_gap_numeric_comparison_value_event => GapNumericComparisonValueEvent {
        connection_handle: ConnHandle,
        numeric_value: u32,
    }
}

vendor_event! {
    /// The peer reported a keypress while its user enters the passkey.
    aci_gap_keypress_notification_event => GapKeypressNotificationEvent {
        connection_handle: ConnHandle,
        notification_type: u8,
    }
}

vendor_event! {
    /// A peer asked to pair; answer with [`GapPairingRequestReply`].
    aci_gap_pairing_request_event => GapPairingRequestEvent {
        connection_handle: ConnHandle,
        bonded: bool,
        auth_req: u8,
    }
}

#[cfg(all(test, feature = "stack-full-extended"))]
mod tests {
    use bt_hci::cmd::{Cmd, CmdReturnBuf, SyncCmd};
    use bt_hci::{FromHciBytes, WriteHci};

    use super::*;
    use crate::wire::VendorEvent;

    fn encode(command: &impl WriteHci) -> ([u8; 300], usize) {
        let mut buffer = [0; 300];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 300 - writer.len();
        (buffer, len)
    }

    #[test]
    fn two_names_fit_their_capacities_but_not_the_parameters_together() {
        let command = GapSetDiscoverable::try_new(
            AdvertisingType::ConnectableUndirected,
            0x20,
            0x30,
            OwnAddressType::Public,
            0,
            b"ab",
            &[0x02, 0x0A, 0x18],
            6,
            7,
        )
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
        let error = GapSetDiscoverable::try_new(
            AdvertisingType::ConnectableUndirected,
            0x20,
            0x30,
            OwnAddressType::Public,
            0,
            &name,
            &name[..10],
            6,
            7,
        )
        .unwrap_err();
        assert_eq!(error.field, "parameters");
        assert_eq!(error.len, 265);
        assert!(
            GapSetDiscoverable::try_new(
                AdvertisingType::ConnectableUndirected,
                0x20,
                0x30,
                OwnAddressType::Public,
                0,
                &name,
                &[],
                6,
                7
            )
            .is_ok()
        );
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
            OwnAddressType::Public,
            0,
            0,
            0,
            0,
            ScanningPhys::LE_1M,
            [phy(1, 0x10, 0x20), phy(0, 0x30, 0x40)],
        );
        assert_eq!(GapExtStartScan::OPCODE.to_raw(), 0xFCD0);
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..3], [0xD0, 0xFC, 10 + 10]);
        assert_eq!(bytes[3..13], [0, 1, 0, 0, 0, 0, 0, 0, 0, 0x01]);
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

    #[test]
    fn init_returns_the_gap_service_handles() {
        let (bytes, len) = encode(&GapInit::new(Role::PERIPHERAL, Privacy::Disabled, 7));
        assert_eq!(bytes[..len], [0x8A, 0xFC, 3, 0x01, 0, 7]);
        let service = <GapInit as SyncCmd>::Return::from_hci_bytes_complete(&[
            0x01, 0x00, 0x02, 0x00, 0x03, 0x00,
        ])
        .unwrap();
        assert_eq!(
            (
                service.service_handle,
                service.dev_name_char_handle,
                service.appearance_char_handle
            ),
            (1, 2, 3)
        );
    }

    #[cfg(not(any(feature = "fw_1_15_0", feature = "fw_1_16_0")))]
    #[test]
    fn device_lists_hold_addresses() {
        let entry = ListEntry {
            address_type: 1,
            address: BdAddr::new([1, 2, 3, 4, 5, 6]),
        };
        let (bytes, len) = encode(&GapAddDevicesToList::try_new(&[entry], 2).unwrap());
        assert_eq!(bytes[..len], [0xAB, 0xFC, 9, 1, 1, 1, 2, 3, 4, 5, 6, 2]);
        assert!(GapAddDevicesToList::try_new(&[entry; 37], 2).is_err());
    }

    #[cfg(any(feature = "fw_1_15_0", feature = "fw_1_16_0"))]
    #[test]
    fn device_lists_hold_bytes() {
        let (bytes, len) = encode(&GapAddDevicesToList::try_new(&[1, 2, 3], 2).unwrap());
        assert_eq!(bytes[..len], [0xAB, 0xFC, 5, 3, 1, 2, 3, 2]);
        assert!(GapAddDevicesToList::try_new(&[0; 253], 2).is_err());
    }

    #[cfg(not(any(
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
        feature = "fw_1_21_0"
    )))]
    #[test]
    fn bonded_device_checks_return_the_identity_address() {
        let identity = <GapCheckBondedDevice as SyncCmd>::Return::from_hci_bytes_complete(&[
            1, 6, 5, 4, 3, 2, 1,
        ])
        .unwrap();
        assert_eq!(identity.id_address_type, 1);
        assert_eq!(identity.id_address, BdAddr::new([6, 5, 4, 3, 2, 1]));
    }

    #[cfg(any(
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
        feature = "fw_1_21_0"
    ))]
    #[test]
    fn bonded_device_checks_return_nothing_before_1_22_0() {
        let () = <GapCheckBondedDevice as SyncCmd>::Return::from_hci_bytes_complete(&[]).unwrap();
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
    fn extended_connections_encode_three_phys() {
        let phy = |n: u16| InitParamPhy {
            scan_interval: n,
            scan_window: n,
            conn_interval_min: n,
            conn_interval_max: n,
            conn_latency: n,
            supervision_timeout: n,
            min_ce_length: n,
            max_ce_length: n,
        };
        let command = GapExtCreateConnection::new(
            0,
            0,
            ConnectableOwnAddressType::Public,
            AddressType::Public,
            BdAddr::new([1, 2, 3, 4, 5, 6]),
            0xFF,
            0xFF,
            0,
            InitiatingPhys::LE_1M | InitiatingPhys::LE_2M,
            [phy(1), phy(2), phy(3)],
        );
        let (bytes, len) = encode(&command);
        assert_eq!(bytes[..3], [0xD1, 0xFC, 14 + 3 * 16]);
        assert_eq!(bytes[16], 0b011);
        assert_eq!(bytes[17..19], [1, 0]);
        assert_eq!(bytes[33..35], [2, 0]);
        assert_eq!(bytes[len - 2..len], [3, 0]);
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
        let command = GapStartSelectiveConnectionEstablishProc::try_new(
            ScanType::Active,
            0x10,
            0x10,
            OwnAddressType::Public,
            0,
            true,
            &peers,
        )
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
            GapStartSelectiveConnectionEstablishProc::try_new(
                ScanType::Active,
                0x10,
                0x10,
                OwnAddressType::Public,
                0,
                true,
                &too_many,
            )
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

    #[test]
    fn gap_events_decode_after_their_code() {
        let params = [0x07, 0x04, 0x02, 0x00, 0x02, 0xAA, 0xBB];
        let complete = GapProcCompleteEvent::from_vendor_params(&params)
            .unwrap()
            .unwrap();
        assert_eq!(GapProcCompleteEvent::CODE, 0x0407);
        assert_eq!(
            (complete.procedure_code, complete.status, complete.data),
            (0x02, 0x00, &[0xAA, 0xBB][..])
        );
        assert!(
            GapProcCompleteEvent::from_vendor_params(&params[..6])
                .unwrap()
                .is_err()
        );
        let limited = GapLimitedDiscoverableEvent::from_vendor_params(&[0x00, 0x04]).unwrap();
        assert_eq!(limited, Ok(GapLimitedDiscoverableEvent {}));
    }

    #[cfg(not(any(
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
        feature = "fw_1_21_0"
    )))]
    #[test]
    fn lost_bonds_name_the_connection_from_1_22_0() {
        let lost = GapBondLostEvent::from_hci_bytes_complete(&[0x01, 0x08]).unwrap();
        assert_eq!(lost.connection_handle, ConnHandle::new(0x0801));
        let pairing =
            GapPairingRequestEvent::from_hci_bytes_complete(&[0x01, 0x00, 1, 0x2D]).unwrap();
        assert!(pairing.bonded);
        assert_eq!(pairing.auth_req, 0x2D);
    }

    #[cfg(any(
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
        feature = "fw_1_21_0"
    ))]
    #[test]
    fn lost_bonds_carry_nothing_before_1_22_0() {
        assert_eq!(
            GapBondLostEvent::from_hci_bytes_complete(&[]),
            Ok(GapBondLostEvent {})
        );
        assert!(GapBondLostEvent::from_hci_bytes_complete(&[0x01, 0x08]).is_err());
    }
}
