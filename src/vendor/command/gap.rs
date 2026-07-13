//! GAP commands and types needed for those commands.

pub use crate::types::BdAddrType;
use crate::types::PeerAddrType;
pub use crate::types::extended_advertisement::AdvertisingHandle;
use crate::types::extended_advertisement::{
    AdvSet, AdvertisingEvent, AdvertisingMode, AdvertisingOperation, AdvertisingPhy,
    ExtendedAdvertisingInterval,
};
pub use crate::types::{
    AdvertisingFilterPolicy, AdvertisingType, ConnectionInterval, ExpectedConnectionLength,
    OwnAddressType, ScanWindow,
};
use crate::vendor::command::BoundedItems;
use crate::vendor::event::AttributeHandle;
use bt_hci::param::{BdAddr, ConnHandle};

hci_ranged! {
    /// Six-digit GAP pass key.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PassKey: u32 => 4 {
        minimum: 0,
        maximum: 999_999,
    }
}

impl crate::vendor::command::HciEncodeField<4> for ScanWindow {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

hci_ranged! {
    /// Power-amplifier output level accepted by the additional-beacon command.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PowerAmplifierOutputLevel: u8 => 1 {
        minimum: 0,
        maximum: 0x23,
    }
}

hci_enum! {
    /// Reasons accepted by [`GapTerminate`].
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum TerminationReason: u8 => 1 {
        AuthenticationFailure = 0x05,
        RemoteUser = 0x13,
        RemoteLowResources = 0x14,
        RemotePowerOff = 0x15,
        UnsupportedRemoteFeature = 0x1A,
        PairingWithUnitKeyNotSupported = 0x29,
        UnacceptableConnectionParameters = 0x3B,
    }
}

vendor_cmd! {
    GapSetNonDiscoverable(cgid = 0x1, cid = 0x01) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetLimitedDiscoverable(cgid = 0x1, cid = 0x02) {
        Params<'a> = {
            advertising_type: AdvertisingType => 1,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: AddressType => 1,
            filter_policy: AdvertisingFilterPolicy => 1,
            local_name: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 242,
            },
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
        };
        Constraints = {
            one_of(advertising_type, [
                AdvertisingType::ConnectableUndirected,
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ]);
            one_of_or_range(advertising_interval_min, [0], 0x0020, 0x4000);
            one_of_or_range(advertising_interval_max, [0], 0x0020, 0x4000);
            paired_value(advertising_interval_min, advertising_interval_max, 0);
            ordered(advertising_interval_min, advertising_interval_max);
            one_of_or_range(conn_interval_min, [0, 0xFFFF], 0x0006, 0x0C80);
            one_of_or_range(conn_interval_max, [0, 0xFFFF], 0x0006, 0x0C80);
            ordered_when_in_range(conn_interval_min, conn_interval_max, 0x0006, 0x0C80);
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapSetDiscoverable(cgid = 0x1, cid = 0x03) {
        Params<'a> = {
            advertising_type: AdvertisingType => 1,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: AddressType => 1,
            filter_policy: AdvertisingFilterPolicy => 1,
            local_name: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 242,
            },
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
        };
        Constraints = {
            one_of(advertising_type, [
                AdvertisingType::ConnectableUndirected,
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ]);
            one_of_or_range(advertising_interval_min, [0], 0x0020, 0x4000);
            one_of_or_range(advertising_interval_max, [0], 0x0020, 0x4000);
            paired_value(advertising_interval_min, advertising_interval_max, 0);
            ordered(advertising_interval_min, advertising_interval_max);
            one_of_or_range(conn_interval_min, [0, 0xFFFF], 0x0006, 0x0C80);
            one_of_or_range(conn_interval_max, [0, 0xFFFF], 0x0006, 0x0C80);
            ordered_when_in_range(conn_interval_min, conn_interval_max, 0x0006, 0x0C80);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetDirectConnectable(cgid = 0x1, cid = 0x04) {
        Params = {
            own_address_type: AddressType => 1,
            advertising_type: AdvertisingType => 1,
            initiator_address: BdAddrType => 7,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
        };
        Constraints = {
            one_of(advertising_type, [
                AdvertisingType::ConnectableDirectedHighDutyCycle,
                AdvertisingType::ConnectableDirectedLowDutyCycle,
            ]);
            implies_eq(
                advertising_type,
                AdvertisingType::ConnectableDirectedHighDutyCycle,
                advertising_interval_min,
                0x0006
            );
            implies_eq(
                advertising_type,
                AdvertisingType::ConnectableDirectedHighDutyCycle,
                advertising_interval_max,
                0x0006
            );
            implies_range(
                advertising_type,
                AdvertisingType::ConnectableDirectedLowDutyCycle,
                advertising_interval_min,
                0x0020,
                0x4000
            );
            implies_range(
                advertising_type,
                AdvertisingType::ConnectableDirectedLowDutyCycle,
                advertising_interval_max,
                0x0020,
                0x4000
            );
            ordered(advertising_interval_min, advertising_interval_max);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetIoCapability(cgid = 0x1, cid = 0x05) {
        Params = {
            io_capability: IoCapability => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetAuthenticationRequirement(cgid = 0x1, cid = 0x06) {
        Params = {
            bonding_required: bool => 1,
            mitm_protection_required: bool => 1,
            secure_connection_support: SecureConnectionSupport => 1,
            keypress_notification_support: bool => 1,
            encryption_key_size_min: u8 => 1,
            encryption_key_size_max: u8 => 1,
            pass_key_required: bool => 1,
            fixed_pin: PassKey => 4,
            identity_address_type: AddressType => 1,
        };
        Constraints = {
            range(encryption_key_size_min, 7, 16);
            range(encryption_key_size_max, 7, 16);
            ordered(encryption_key_size_min, encryption_key_size_max);
            one_of(identity_address_type, [AddressType::Public, AddressType::Random]);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetAuthorizationRequirement(cgid = 0x1, cid = 0x07) {
        Params = {
            conn_handle: ConnHandle => 2,
            authorization_required: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPassKeyResponse(cgid = 0x1, cid = 0x08) {
        Params = {
            conn_handle: ConnHandle => 2,
            pin: PassKey => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAuthorizationResponse(cgid = 0x1, cid = 0x09) {
        Params = {
            conn_handle: ConnHandle => 2,
            authorization: Authorization => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

// TODO: verify these return parameters

vendor_cmd! {
    CmdGapInit(cgid = 0x1, cid = 0x0A) {
        Params = {
            role: Role => 1,
            privacy_enabled: bool => 1,
            dev_name_characteristic_len: u8 => 1,
        };
        Completion = CommandComplete;
        Return = GapInit {
            service_handle: AttributeHandle => 2,
            dev_name_handle: AttributeHandle => 2,
            appearance_handle: AttributeHandle => 2,
        };
    }
}

vendor_cmd! {
    GapSetNonConnectable(cgid = 0x1, cid = 0x0B) {
        Params = {
            advertising_type: AdvertisingType => 1,
            address_type: AddressType => 1,
        };
        Constraints = {
            one_of(advertising_type, [
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ]);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetUnidirectedConnectable(cgid = 0x1, cid = 0x0C) {
        Params = {
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: AddressType => 1,
            filter_policy: AdvertisingFilterPolicy => 1,
        };
        Constraints = {
            range(advertising_interval_min, 0x0020, 0x4000);
            range(advertising_interval_max, 0x0020, 0x4000);
            ordered(advertising_interval_min, advertising_interval_max);
            one_of(filter_policy, [
                AdvertisingFilterPolicy::AllowConnectionAndScan,
                AdvertisingFilterPolicy::WhiteListConnectionAndScan,
            ]);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPeripheralSecurityRequest(cgid = 0x1, cid = 0x0D) {
        Params = {
            conn_handle: ConnHandle => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapUpdateAdvertisingData(cgid = 0x1, cid = 0x0E) {
        Params<'a> = {
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapDeleteAdType(cgid = 0x1, cid = 0x0F) {
        Params = {
            // Bluetooth AD types are an open registry, so this remains a raw
            // byte rather than pretending the legacy enum is exhaustive.
            ad_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapGetSecurityLevel(cgid = 0x1, cid = 0x10) {
        Params = {
            conn_handle: ConnHandle => 2,
        };
        Completion = CommandComplete;
        Return = GapSecurityLevelReturn {
            security_mode: u8 => 1,
            security_level: u8 => 1,
        };
    }
}

vendor_cmd! {
    GapSetEventMask(cgid = 0x1, cid = 0x11) {
        Params = {
            flags: EventFlags => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapConfigureWhitelist(cgid = 0x1, cid = 0x12) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapTerminate(cgid = 0x1, cid = 0x13) {
        Params = {
            conn_handle: ConnHandle => 2,
            reason: TerminationReason => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapClearSecurityDatabase(cgid = 0x1, cid = 0x14) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAllowRebond(cgid = 0x1, cid = 0x15) {
        Params = {
            conn_handle: ConnHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartLimitedDiscoveryProcedure(cgid = 0x1, cid = 0x16) {
        Params = {
            scan_window: ScanWindow => 4,
            own_address_type: AddressType => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartGeneralDiscoveryProcedure(cgid = 0x1, cid = 0x17) {
        Params = {
            scan_window: ScanWindow => 4,
            own_address_type: AddressType => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartAutoConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x19) {
        Params<'a> = {
            scan_window: ScanWindow => 4,
            own_address_type: AddressType => 1,
            conn_interval: ConnectionInterval => 8,
            expected_connection_length: ExpectedConnectionLength => 4,
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 33,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartGeneralConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x1A) {
        Params = {
            scan_type: ScanType => 1,
            scan_window: ScanWindow => 4,
            filter_policy: ScanningFilterPolicy => 1,
            own_address_type: AddressType => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartSelectiveConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x1B) {
        Params<'a> = {
            scan_type: ScanType => 1,
            scan_window: ScanWindow => 4,
            own_address_type: AddressType => 1,
            filter_policy: ScanningFilterPolicy => 1,
            filter_duplicates: bool => 1,
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 35,
            },
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapCreateConnection(cgid = 0x1, cid = 0x1C) {
        Params = {
            scan_window: ScanWindow => 4,
            peer_address: PeerAddrType => 7,
            own_address_type: AddressType => 1,
            conn_interval: ConnectionInterval => 8,
            expected_connection_length: ExpectedConnectionLength => 4,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapTerminateProcedure(cgid = 0x1, cid = 0x1D) {
        Params = {
            procedure: Procedure => 1,
        };
        Constraints = {
            non_empty(procedure);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartConnectionUpdate(cgid = 0x1, cid = 0x1E) {
        Params = {
            conn_handle: ConnHandle => 2,
            conn_interval: ConnectionInterval => 8,
            expected_connection_length: ExpectedConnectionLength => 4,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapSendPairingRequest(cgid = 0x1, cid = 0x1F) {
        Params = {
            conn_handle: ConnHandle => 2,
            force_rebond: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    CmdGapResolvePrivateAddress(cgid = 0x1, cid = 0x20) {
        Params = {
            address: BdAddr => 6,
        };
        Completion = CommandComplete;
        Return = GapResolvedPrivateAddress {
            address: BdAddr => 6,
        };
    }
}

vendor_cmd! {
    GapSetBroadcastMode(cgid = 0x1, cid = 0x21) {
        Params<'a> = {
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            advertising_type: AdvertisingType => 1,
            own_address_type: AddressType => 1,
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 31,
            },
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 35,
            },
        };
        Constraints = {
            range(advertising_interval_min, 0x0020, 0x4000);
            range(advertising_interval_max, 0x0020, 0x4000);
            ordered(advertising_interval_min, advertising_interval_max);
            one_of(advertising_type, [
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ]);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartObservationProcedure(cgid = 0x1, cid = 0x22) {
        Params = {
            scan_window: ScanWindow => 4,
            scan_type: ScanType => 1,
            own_address_type: AddressType => 1,
            filter_duplicates: bool => 1,
            filter_policy: ScanningFilterPolicy => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapGetBondedDevices(cgid = 0x1, cid = 0x23) {
        Params = ();
        Completion = CommandComplete;
        Return = GapBondedDevices {
            addresses: BoundedItems<BdAddrType, 35> => {
                kind: counted_items,
                count: u8 => 1,
                item: BdAddrType => 7,
                max_items: 35,
            },
        };
    }
}

impl GapBondedDevices {
    /// Addresses reported by the controller.
    pub fn bonded_addresses(&self) -> &[BdAddrType] {
        self.addresses.as_slice()
    }
}

vendor_cmd! {
    GapIsDeviceBonded(cgid = 0x1, cid = 0x24) {
        Params = {
            address: PeerAddrType => 7,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapConfirmNumericComparisonValue(cgid = 0x1, cid = 0x25) {
        Params = {
            conn_handle: ConnHandle => 2,
            confirm_yes_no: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPasskeyInput(cgid = 0x1, cid = 0x26) {
        Params = {
            conn_handle: ConnHandle => 2,
            input_type: InputType => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}
vendor_cmd! {
    GapGetOobData(cgid = 0x1, cid = 0x27) {
        Params = {
            oob_data_type: OobDataType => 1,
        };
        Completion = CommandComplete;
        Return = GapOobData {
            address_type: u8 => 1,
            address: BdAddr => 6,
            oob_data_type: u8 => 1,
            oob_data_len: u8 => 1,
            oob_data: [u8; 16] => 16,
        };
    }
}

vendor_cmd! {
    GapSetOobData(cgid = 0x1, cid = 0x28) {
        Params = {
            device_type: OobDeviceType => 1,
            address: BdAddrType => 7,
            oob_data_type: OobDataType => 1,
            oob_data_len: OobDataLength => 1,
            oob_data: [u8; 16] => 16,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAddDevicesToResolvingList(cgid = 0x1, cid = 0x29) {
        Params<'a> = {
            whitelist_identities: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: PeerAddrType => 7,
                max_items: 36,
            },
            clear_resolving_list: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapRemoveBondedDevice(cgid = 0x1, cid = 0x2A) {
        Params = {
            address: BdAddrType => 7,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAddDevicesToList(cgid = 0x1, cid = 0x2B) {
        Params<'a> = {
            list_entries: &'a [BdAddrType] => {
                kind: counted_items,
                count: u8 => 1,
                item: BdAddrType => 7,
                max_items: 36,
            },
            mode: AddDeviceToListMode => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdditionalBeaconStart(cgid = 0x1, cid = 0x30) {
        Params = {
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            advertising_channel_map: AdvertisingChannelMap => 1,
            own_address_type: BdAddrType => 7,
            pa_level: PowerAmplifierOutputLevel => 1,
        };
        Constraints = {
            range(advertising_interval_min, 0x0020, 0x4000);
            range(advertising_interval_max, 0x0020, 0x4000);
            ordered(advertising_interval_min, advertising_interval_max);
            non_empty(advertising_channel_map);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdditionalBeaconStop(cgid = 0x1, cid = 0x31) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdditionalBeaconSetData(cgid = 0x1, cid = 0x32) {
        Params<'a> = {
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 254,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetConfig(cgid = 0x1, cid = 0x40) {
        Params<'a> = {
            adv_mode: AdvertisingMode => 1,
            adv_handle: AdvertisingHandle => 1,
            adv_event_properties: AdvertisingEvent => 2,
            adv_interval: &'a ExtendedAdvertisingInterval => 8,
            primary_adv_channel_map: AdvertisingChannelMap => 1,
            own_addr_type: AddressType => 1,
            peer_addr: BdAddrType => 7,
            adv_filter_policy: AdvertisingFilterPolicy => 1,
            adv_tx_power: i8 => 1,
            secondary_adv_max_skip: u8 => 1,
            secondary_adv_phy: AdvertisingPhy => 1,
            adv_sid: u8 => 1,
            scan_req_notification_enable: bool => 1,
        };
        Constraints = {
            non_empty(primary_adv_channel_map);
            range(adv_tx_power, -127, 20);
            range(adv_sid, 0, 0x0F);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetEnable(cgid = 0x1, cid = 0x41) {
        Params<'a> = {
            enable: bool => 1,
            adv_set: &'a [AdvSet] => {
                kind: counted_items,
                count: u8 => 1,
                item: AdvSet => 4,
                max_items: 63,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetAdvertisingData(cgid = 0x1, cid = 0x42) {
        Params<'a> = {
            adv_handle: AdvertisingHandle => 1,
            operation: AdvertisingOperation => 1,
            fragment_preference: bool => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 251,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetScanResponseData(cgid = 0x1, cid = 0x43) {
        Params<'a> = {
            adv_handle: AdvertisingHandle => 1,
            operation: AdvertisingOperation => 1,
            fragment_preference: bool => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 251,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvRemoveSet(cgid = 0x1, cid = 0x44) {
        Params = {
            handle: AdvertisingHandle => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvClearSets(cgid = 0x1, cid = 0x45) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAdvSetRandomAddress(cgid = 0x1, cid = 0x46) {
        Params = {
            handle: AdvertisingHandle => 1,
            address: BdAddr => 6,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapPairingRequestReply(cgid = 0x1, cid = 0x2D) {
        Params = {
            conn_handle: ConnHandle => 2,
            accept: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapAdvSetPeriodicParameters(cgid = 0x1, cid = 0x47) {
        Params = {
            advertising_handle: AdvertisingHandle => 1,
            periodic_adv_interval_min: PeriodicAdvertisingInterval => 2,
            periodic_adv_interval_max: PeriodicAdvertisingInterval => 2,
            periodic_adv_properties: PeriodicAdvertisingProperties => 2,
            num_subevents: PeriodicAdvertisingSubeventCount => 1,
            subevent_interval: PeriodicAdvertisingSubeventInterval => 1,
            response_slot_delay: PeriodicAdvertisingResponseSlotDelay => 1,
            response_slot_spacing: PeriodicAdvertisingResponseSlotSpacing => 1,
            num_response_slots: u8 => 1,
        };
        Constraints = {
            ordered(periodic_adv_interval_min, periodic_adv_interval_max);
            pawr_subevents_fit(
                periodic_adv_interval_min,
                num_subevents,
                subevent_interval
            );
            pawr_response_slots_fit(
                num_subevents,
                subevent_interval,
                response_slot_delay,
                response_slot_spacing,
                num_response_slots
            );
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapAdvSetPeriodicData(cgid = 0x1, cid = 0x48) {
        Params<'a> = {
            advertising_handle: AdvertisingHandle => 1,
            operation: AdvertisingOperation => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 252,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapAdvSetPeriodicEnable(cgid = 0x1, cid = 0x49) {
        Params = {
            enable: PeriodicAdvertisingEnable => 1,
            handle: AdvertisingHandle => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapAdvSetConfigurationV2(cgid = 0x1, cid = 0x4D) {
        Params = {
            adv_mode: AdvertisingMode => 1,
            adv_handle: AdvertisingHandle => 1,
            adv_event_properties: AdvertisingEvent => 2,
            primary_adv_interval_min: u32 => 4,
            primary_adv_interval_max: u32 => 4,
            primary_adv_channel_map: AdvertisingChannelMap => 1,
            own_addr_type: AddressType => 1,
            peer_addr: BdAddrType => 7,
            adv_filter_policy: AdvertisingFilterPolicy => 1,
            adv_tx_power: i8 => 1,
            primary_adv_phy: AdvertisingPhy => 1,
            secondary_adv_max_skip: u8 => 1,
            secondary_adv_phy: AdvertisingPhy => 1,
            adv_sid: u8 => 1,
            scan_req_notification_enable: bool => 1,
            primary_adv_phy_options: u8 => 1,
        };
        Constraints = {
            range(primary_adv_interval_min, 0x0000_0020, 0x00FF_FFFF);
            range(primary_adv_interval_max, 0x0000_0020, 0x00FF_FFFF);
            ordered(primary_adv_interval_min, primary_adv_interval_max);
            non_empty(primary_adv_channel_map);
            range(adv_tx_power, -127, 20);
            range(adv_sid, 0, 0x0F);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapExtStartScan(cgid = 0x1, cid = 0x50) {
        Params<'a> = {
            scan_mode: ExtScanMode => 1,
            procedure: Procedure => 1,
            own_address_type: AddressType => 1,
            filter_duplicates: bool => 1,
            duration: u16 => 2,
            period: u16 => 2,
            scanning_filter_policy: ScanningFilterPolicy => 1,
            scanning_phys: ScanningPhy => 1,
            phy_params: &'a [ExtScanPhyParams] => {
                kind: bitmap_items,
                bitmap: scanning_phys,
                mask: 0x05,
                item: ExtScanPhyParams => 5,
                max_items: 2,
            },
        };
        Constraints = {
            one_of(procedure, [
                Procedure::LIMITED_DISCOVERY,
                Procedure::GENERAL_DISCOVERY,
                Procedure::GENERAL_CONNECTION_ESTABLISHMENT,
                Procedure::SELECTIVE_CONNECTION_ESTABLISHMENT,
                Procedure::OBSERVATION,
            ]);
            non_empty(scanning_phys);
        };
        Completion = CommandStatus;
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapExtCreateConnection(cgid = 0x1, cid = 0x51) {
        Params<'a> = {
            initiating_mode: ExtInitiatingMode => 1,
            procedure: Procedure => 1,
            own_address_type: AddressType => 1,
            peer_address: BdAddrType => 7,
            advertising_handle: InitiatingAdvertisingHandle => 1,
            subevent: InitiatingSubevent => 1,
            initiator_filter_policy: InitiatorFilterPolicy => 1,
            initiating_phys: InitiatingPhy => 1,
            phy_params: &'a [ExtConnectionPhyParams] => {
                kind: bitmap_items,
                bitmap: initiating_phys,
                mask: 0x07,
                item: ExtConnectionPhyParams => 16,
                max_items: 3,
            },
        };
        Constraints = {
            one_of(procedure, [
                Procedure::AUTO_CONNECTION_ESTABLISHMENT,
                Procedure::DIRECT_CONNECTION_ESTABLISHMENT,
            ]);
            one_of(own_address_type, [
                AddressType::Public,
                AddressType::Random,
                AddressType::ResolvablePrivate,
            ]);
            non_empty(initiating_phys);
        };
        Completion = CommandStatus;
    }
}

impl crate::vendor::command::HciEncodeField<8> for ExtendedAdvertisingInterval {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 8];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 8];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

impl crate::vendor::command::HciEncodeField<7> for PeerAddrType {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 7];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 7];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

impl crate::vendor::command::HciEncodeField<4> for AdvSet {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        let mut bytes = [0; 4];
        self.copy_into_slice(&mut bytes);
        writer.write_all(&bytes).await
    }
}

hci_enum! {
    /// I/O capabilities available for the [GAP Set I/O Capability](GapSetIoCapability) command.
    #[derive(Copy, Clone, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum IoCapability: u8 => 1 {
        /// Display Only
        Display = 0x00,
        /// Display yes/no
        DisplayConfirm = 0x01,
        /// Keyboard Only
        Keyboard = 0x02,
        /// No Input, no output
        None = 0x03,
        /// Keyboard display
        KeyboardDisplay = 0x04,
    }
}

/// Options for out-of-band authentication.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OutOfBandAuthentication {
    /// Out Of Band authentication not enabled
    Disabled,
    /// Out Of Band authentication enabled; includes the OOB data.
    Enabled([u8; 16]),
}

hci_enum! {
    /// Secure Connection support mode for [`GapSetAuthenticationRequirement`].
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SecureConnectionSupport: u8 => 1 {
        NotSupported = 0x00,
        Optional = 0x01,
        Mandatory = 0x02,
    }
}

/// Fixed-PIN behavior for [`GapSetAuthenticationRequirement`].
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Pin {
    /// Do not use fixed pin during the pairing process.  In this case, GAP will generate a
    /// [GAP Pass Key Request](crate::vendor::event::VendorEvent::GapPassKeyRequest) event to the host.
    Requested,

    /// Use a fixed pin during pairing. The provided value is used as the PIN, and must be 999999 or
    /// less.
    Fixed(u32),
}

hci_enum! {
    /// Options for the [GAP Authorization Response](GapAuthorizationResponse).
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum Authorization: u8 => 1 {
        /// Accept the connection.
        Authorized = 0x01,
        /// Reject the connection.
        Rejected = 0x02,
    }
}

hci_bitflags! {
    /// Roles for a [GAP service](CmdGapInit).
    pub struct Role: u8 => 1 {
        /// Peripheral
        const PERIPHERAL = 0x01;
        /// Broadcaster
        const BROADCASTER = 0x02;
        /// Central Device
        const CENTRAL = 0x04;
        /// Observer
        const OBSERVER = 0x08;
    }
}

hci_enum! {
    /// Indicates the type of address being used in the advertising packets, for
    /// [`GapSetNonConnectable`].
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AddressType: u8 => 1 {
        /// Public device address.
        Public = 0x00,
        /// Static random device address.
        Random = 0x01,
        /// Controller generates Resolvable Private Address.
        ResolvablePrivate = 0x02,
        /// Controller generates Resolvable Private Address based on the local IRK from resolving
        /// list.
        NonResolvablePrivate = 0x03,
    }
}

hci_enum! {
    /// Available types of advertising data.
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AdvertisingDataType: u8 => 1 {
        /// Flags
        Flags = 0x01,
        /// 16-bit service UUID
        Uuid16 = 0x02,
        /// Complete list of 16-bit service UUIDs
        UuidCompleteList16 = 0x03,
        /// 32-bit service UUID
        Uuid32 = 0x04,
        /// Complete list of 32-bit service UUIDs
        UuidCompleteList32 = 0x05,
        /// 128-bit service UUID
        Uuid128 = 0x06,
        /// Complete list of 128-bit service UUIDs.
        UuidCompleteList128 = 0x07,
        /// Shortened local name
        ShortenedLocalName = 0x08,
        /// Complete local name
        CompleteLocalName = 0x09,
        /// Transmitter power level
        TxPowerLevel = 0x0A,
        /// Security Manager TK Value
        SecurityManagerTkValue = 0x10,
        /// Security Manager out-of-band flags
        SecurityManagerOutOfBandFlags = 0x11,
        /// Connection interval
        PeripheralConnectionInterval = 0x12,
        /// Service solicitation list, 16-bit UUIDs
        SolicitUuidList16 = 0x14,
        /// Service solicitation list, 32-bit UUIDs
        SolicitUuidList32 = 0x15,
        /// Service data
        ServiceData = 0x16,
        /// Manufacturer-specific data
        ManufacturerSpecificData = 0xFF,
    }
}

hci_bitflags! {
    /// Event types for [GAP Set Event Mask](GapSetEventMask).
    pub struct EventFlags: u16 => 2 {
        /// [Limited Discoverable](crate::vendor::event::VendorEvent::GapLimitedDiscoverableTimeout)
        const LIMITED_DISCOVERABLE_TIMEOUT = 0x0001;
        /// [Pairing Complete](crate::vendor::event::VendorEvent::GapPairingComplete)
        const PAIRING_COMPLETE = 0x0002;
        /// [Pass Key Request](crate::vendor::event::VendorEvent::GapPassKeyRequest)
        const PASS_KEY_REQUEST = 0x0004;
        /// [Authorization Request](crate::vendor::event::VendorEvent::GapAuthorizationRequest)
        const AUTHORIZATION_REQUEST = 0x0008;
        /// [Peripheral Security Initiated](crate::vendor::event::VendorEvent::GapPeripheralSecurityInitiated).
        const PERIPHERAL_SECURITY_INITIATED = 0x0010;
        /// [Bond Lost](crate::vendor::event::VendorEvent::GapBondLost)
        const BOND_LOST = 0x0020;
        /// [GAP Procedure Complete](crate::vendor::event::VendorEvent::GapProcedureComplete)
        const PROCEDURE_COMPLETE = 0x0080;
        /// [L2CAP Connection Update Request](crate::vendor::event::VendorEvent::L2CapConnectionUpdateRequest)
        const L2CAP_CONNECTION_UPDATE_REQUEST = 0x0100;
        /// [L2CAP Connection Update Response](crate::vendor::event::VendorEvent::L2CapConnectionUpdateResponse)
        const L2CAP_CONNECTION_UPDATE_RESPONSE = 0x0200;
        /// [L2CAP Procedure Timeout](crate::vendor::event::VendorEvent::L2CapProcedureTimeout)
        const L2CAP_PROCEDURE_TIMEOUT = 0x0400;
        /// [GAP Address Not Resolved](crate::vendor::event::VendorEvent::GapAddressNotResolved)
        const ADDRESS_NOT_RESOLVED = 0x0800;
    }
}

hci_bitflags! {
    /// GAP procedures accepted by [`GapTerminateProcedure`].
    pub struct Procedure: u8 => 1 {
        /// [Limited Discovery](GapStartLimitedDiscoveryProcedure) procedure.
        const LIMITED_DISCOVERY = 0x01;
        /// [General Discovery](GapStartGeneralDiscoveryProcedure) procedure.
        const GENERAL_DISCOVERY = 0x02;
        /// Name Discovery procedure.
        const NAME_DISCOVERY = 0x04;
        /// [Auto Connection Establishment](GapStartAutoConnectionEstablishmentProcedure).
        const AUTO_CONNECTION_ESTABLISHMENT = 0x08;
        /// [General Connection Establishment](GapStartGeneralConnectionEstablishmentProcedure).
        const GENERAL_CONNECTION_ESTABLISHMENT = 0x10;
        /// [Selective Connection Establishment](GapStartSelectiveConnectionEstablishmentProcedure).
        const SELECTIVE_CONNECTION_ESTABLISHMENT = 0x20;
        /// Direct Connection Establishment.
        const DIRECT_CONNECTION_ESTABLISHMENT = 0x40;
        /// [Observation](GapStartObservationProcedure) procedure.
        const OBSERVATION = 0x80;
    }
}

hci_enum! {
    /// Parameter for [GAP Passkey Input](GapPasskeyInput).
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum InputType: u8 => 1 {
        EntryStarted = 0x00,
        DigitEntered = 0x01,
        DigitErased = 0x02,
        Cleared = 0x03,
        EntryCompleted = 0x04,
    }
}

hci_enum! {
    /// Kind of GAP out-of-band pairing data.
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum OobDataType: u8 => 1 {
        /// TK (LP v.4.1)
        TK = 0x00,
        /// Random (SC)
        Random = 0x01,
        /// Confirm (SC)
        Confirm = 0x02,
    }
}

hci_enum! {
    /// Length modes accepted when supplying GAP out-of-band pairing data.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum OobDataLength: u8 => 1 {
        /// Ask the stack to generate Secure Connections random/confirm data.
        Generate = 0x00,
        /// Supply the complete 16-byte out-of-band value.
        Present = 0x10,
    }
}

hci_enum! {
    /// Device whose GAP out-of-band data is being supplied.
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum OobDeviceType: u8 => 1 {
        Local = 0x00,
        Remote = 0x01,
    }
}

hci_enum! {
    /// Parameter for [GAP Add Devices to List](GapAddDevicesToList).
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum AddDeviceToListMode: u8 => 1 {
        /// Append to the resolving list only
        AppendResoling = 0x00,
        /// Clear and set the resolving list only
        ClearAndSetResolving = 0x01,
        /// Append to the whitelist only
        AppendWhitelist = 0x02,
        /// Clear and set the whitelist only
        ClearAndSetWhitelist = 0x03,
        /// Append to both resolving and white lists
        AppendBoth = 0x04,
        /// Clear and set both resolving and white lists
        ClearAndSetBoth = 0x05,
    }
}

hci_enum! {
    /// Type of Link Layer scan performed by GAP discovery procedures.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ScanType: u8 => 1 {
        /// Listen for advertisements without sending scan requests.
        Passive = 0x00,
        /// Send scan requests to scannable advertisers.
        Active = 0x01,
    }
}

hci_enum! {
    /// Policy used to filter advertising and scan-response packets while scanning.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ScanningFilterPolicy: u8 => 1 {
        /// Process packets from every advertiser.
        BasicUnfiltered = 0x00,
        /// Process packets only from devices in the Filter Accept List.
        BasicFiltered = 0x01,
        /// Process all packets, including directed advertisements with a resolvable target.
        ExtendedUnfiltered = 0x02,
        /// Apply the Filter Accept List while accepting resolvable directed targets.
        ExtendedFiltered = 0x03,
    }
}

hci_bitflags! {
    /// Primary advertising channels selected for an advertising procedure.
    pub struct AdvertisingChannelMap: u8 => 1 {
        /// Use primary advertising channel 37.
        const CHANNEL_37 = 0x01;
        /// Use primary advertising channel 38.
        const CHANNEL_38 = 0x02;
        /// Use primary advertising channel 39.
        const CHANNEL_39 = 0x04;
    }
}

#[cfg(after_fw_0_17_1)]
hci_ranged! {
    /// Periodic-advertising interval in 1.25 ms units.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PeriodicAdvertisingInterval: u16 => 2 {
        minimum: 0x0006,
        maximum: 0xFFFF,
    }
}

#[cfg(after_fw_0_17_1)]
hci_bitflags! {
    /// Fields included in periodic advertising packets.
    pub struct PeriodicAdvertisingProperties: u16 => 2 {
        /// Include transmit power in periodic advertising packets.
        const INCLUDE_TX_POWER = 0x0040;
    }
}

#[cfg(after_fw_0_17_1)]
hci_ranged! {
    /// Number of PAwR subevents transmitted in each periodic advertising event.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PeriodicAdvertisingSubeventCount: u8 => 1 {
        minimum: 0x00,
        maximum: 0x80,
    }
}

#[cfg(after_fw_0_17_1)]
hci_ranged! {
    /// Interval between PAwR subevents in 1.25 ms units.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PeriodicAdvertisingSubeventInterval: u8 => 1 {
        minimum: 0x06,
        maximum: 0xFF,
    }
}

#[cfg(after_fw_0_17_1)]
hci_ranged! {
    /// Delay before the first PAwR response slot in 1.25 ms units.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PeriodicAdvertisingResponseSlotDelay: u8 => 1 {
        minimum: 0x00,
        maximum: 0xFE,
    }
}

#[cfg(after_fw_0_17_1)]
hci_ranged! {
    /// Spacing between PAwR response slots in 0.125 ms units.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PeriodicAdvertisingResponseSlotSpacing: u8 => 1 {
        minimum: 0x02,
        maximum: 0xFF,
        sentinel: NO_RESPONSE_SLOTS = 0x00,
    }
}

#[cfg(after_fw_0_17_1)]
hci_bitflags! {
    /// Controls periodic advertising and the optional ADI field.
    pub struct PeriodicAdvertisingEnable: u8 => 1 {
        /// Enable periodic advertising for the selected advertising set.
        const ENABLE_PERIODIC_ADVERTISING = 0x01;
        /// Include the ADI field in AUX_SYNC_IND packets.
        const INCLUDE_ADI = 0x02;
    }
}

#[cfg(after_fw_0_17_1)]
hci_ranged! {
    /// Periodic-advertising handle used while initiating an extended connection.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct InitiatingAdvertisingHandle: u8 => 1 {
        minimum: 0x00,
        maximum: 0xEF,
        sentinel: UNUSED = 0xFF,
    }
}

#[cfg(after_fw_0_17_1)]
hci_ranged! {
    /// Periodic-advertising subevent used while initiating an extended connection.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct InitiatingSubevent: u8 => 1 {
        minimum: 0x00,
        maximum: 0x7F,
        sentinel: UNUSED = 0xFF,
    }
}

#[cfg(after_fw_0_17_1)]
hci_enum! {
    /// Mode field for the extended GAP scan command.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ExtScanMode: u8 => 1 {
        /// Reserved value required by STM32CubeWB.
        Default = 0x00,
    }
}

#[cfg(after_fw_0_17_1)]
hci_bitflags! {
    /// PHYs on which an extended scan is performed.
    pub struct ScanningPhy: u8 => 1 {
        /// Scan advertisements on the LE 1M PHY.
        const LE_1M = 0x01;
        /// Scan advertisements on the LE Coded PHY.
        const LE_CODED = 0x04;
    }
}

#[cfg(after_fw_0_17_1)]
hci_enum! {
    /// Mode field for the extended GAP connection command.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ExtInitiatingMode: u8 => 1 {
        /// Reserved value required by STM32CubeWB.
        Default = 0x00,
    }
}

#[cfg(after_fw_0_17_1)]
hci_enum! {
    /// Selects how an advertiser is chosen during connection initiation.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum InitiatorFilterPolicy: u8 => 1 {
        /// Connect to the explicitly supplied peer address.
        UsePeerAddress = 0x00,
        /// Choose an advertiser from the Filter Accept List.
        UseFilterAcceptList = 0x01,
    }
}

#[cfg(after_fw_0_17_1)]
hci_bitflags! {
    /// PHY-specific parameter records supplied while initiating a connection.
    pub struct InitiatingPhy: u8 => 1 {
        /// Scan and provide connection parameters for the LE 1M PHY.
        const LE_1M = 0x01;
        /// Provide connection parameters for the LE 2M PHY.
        const LE_2M = 0x02;
        /// Scan and provide connection parameters for the LE Coded PHY.
        const LE_CODED = 0x04;
    }
}

/// One record in the extended-scan PHY parameter list.
#[cfg(after_fw_0_17_1)]
pub struct ExtScanPhyParams {
    /// Passive or active scanning for this PHY.
    pub scan_type: ScanType,
    /// Scan interval in 0.625 ms units.
    pub scan_interval: u16,
    /// Scan window in 0.625 ms units.
    pub scan_window: u16,
}

#[cfg(after_fw_0_17_1)]
impl crate::vendor::command::HciEncodeField<5> for ExtScanPhyParams {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        self.scan_type.write_hci_field(&mut writer)?;
        writer.write_all(&self.scan_interval.to_le_bytes())?;
        writer.write_all(&self.scan_window.to_le_bytes())
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        self.scan_type.write_hci_field_async(&mut writer).await?;
        writer.write_all(&self.scan_interval.to_le_bytes()).await?;
        writer.write_all(&self.scan_window.to_le_bytes()).await
    }
}

/// One record in the extended-connection PHY parameter list.
#[cfg(after_fw_0_17_1)]
pub struct ExtConnectionPhyParams {
    /// Scan interval in 0.625 ms units.
    pub scan_interval: u16,
    /// Scan window in 0.625 ms units.
    pub scan_window: u16,
    /// Minimum connection interval in 1.25 ms units.
    pub connection_interval_min: u16,
    /// Maximum connection interval in 1.25 ms units.
    pub connection_interval_max: u16,
    /// Maximum connection latency in connection events.
    pub max_latency: u16,
    /// Supervision timeout in 10 ms units.
    pub supervision_timeout: u16,
    /// Minimum connection-event length in 0.625 ms units.
    pub min_ce_length: u16,
    /// Maximum connection-event length in 0.625 ms units.
    pub max_ce_length: u16,
}

#[cfg(after_fw_0_17_1)]
impl crate::vendor::command::HciEncodeField<16> for ExtConnectionPhyParams {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&self.scan_interval.to_le_bytes())?;
        writer.write_all(&self.scan_window.to_le_bytes())?;
        writer.write_all(&self.connection_interval_min.to_le_bytes())?;
        writer.write_all(&self.connection_interval_max.to_le_bytes())?;
        writer.write_all(&self.max_latency.to_le_bytes())?;
        writer.write_all(&self.supervision_timeout.to_le_bytes())?;
        writer.write_all(&self.min_ce_length.to_le_bytes())?;
        writer.write_all(&self.max_ce_length.to_le_bytes())
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&self.scan_interval.to_le_bytes()).await?;
        writer.write_all(&self.scan_window.to_le_bytes()).await?;
        writer
            .write_all(&self.connection_interval_min.to_le_bytes())
            .await?;
        writer
            .write_all(&self.connection_interval_max.to_le_bytes())
            .await?;
        writer.write_all(&self.max_latency.to_le_bytes()).await?;
        writer
            .write_all(&self.supervision_timeout.to_le_bytes())
            .await?;
        writer.write_all(&self.min_ce_length.to_le_bytes()).await?;
        writer.write_all(&self.max_ce_length.to_le_bytes()).await
    }
}
