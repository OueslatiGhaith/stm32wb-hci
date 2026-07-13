//! GAP commands and types needed for those commands.

pub use crate::types::BdAddrType;
use crate::types::PeerAddrType;
use crate::types::extended_advertisement::{AdvSet, ExtendedAdvertisingInterval};
pub use crate::types::{
    AdvertisingFilterPolicy, AdvertisingType, ConnectionInterval, ExpectedConnectionLength,
    OwnAddressType, ScanWindow,
};
use crate::vendor::command::BoundedItems;
use crate::vendor::event::AttributeHandle;
use bt_hci::param::{AdvHandle, BdAddr, ConnHandle};
#[cfg(after_fw_0_17_1)]
use byteorder::{ByteOrder, LittleEndian};

/// Six-digit GAP pass key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PassKey(u32);

impl PassKey {
    /// Create a pass key in the controller's accepted `0..=999_999` range.
    pub const fn try_new(value: u32) -> Result<Self, crate::vendor::command::HciValueError> {
        if value <= 999_999 {
            Ok(Self(value))
        } else {
            Err(crate::vendor::command::HciValueError::new(
                value as u64,
                0,
                999_999,
            ))
        }
    }

    /// Numeric pass-key value.
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl crate::vendor::command::HciEncodeField<4> for PassKey {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&self.0.to_le_bytes())
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&self.0.to_le_bytes()).await
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

/// Power-amplifier output level accepted by the additional-beacon command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PowerAmplifierOutputLevel(u8);

impl PowerAmplifierOutputLevel {
    /// Create an output level in the controller's accepted `0..=0x23` range.
    pub const fn try_new(value: u8) -> Result<Self, crate::vendor::command::HciValueError> {
        if value <= 0x23 {
            Ok(Self(value))
        } else {
            Err(crate::vendor::command::HciValueError::new(
                value as u64,
                0,
                0x23,
            ))
        }
    }

    /// Raw controller level.
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl crate::vendor::command::HciEncodeField<1> for PowerAmplifierOutputLevel {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[self.0])
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[self.0]).await
    }
}

/// Reasons accepted by [`GapTerminate`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TerminationReason {
    AuthenticationFailure = 0x05,
    RemoteUser = 0x13,
    RemoteLowResources = 0x14,
    RemotePowerOff = 0x15,
    UnsupportedRemoteFeature = 0x1A,
    PairingWithUnitKeyNotSupported = 0x29,
    UnacceptableConnectionParameters = 0x3B,
}

impl crate::vendor::command::HciEncodeField<1> for TerminationReason {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8])
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8]).await
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
            advertising_type: u8 => 1,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
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
            one_of(advertising_type, [0x00, 0x02, 0x03]);
            ordered(advertising_interval_min, advertising_interval_max);
            ordered(conn_interval_min, conn_interval_max);
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapSetDiscoverable(cgid = 0x1, cid = 0x03) {
        Params<'a> = {
            advertising_type: u8 => 1,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
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
            one_of(advertising_type, [0x00, 0x02, 0x03]);
            ordered(advertising_interval_min, advertising_interval_max);
            ordered(conn_interval_min, conn_interval_max);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetDirectConnectable(cgid = 0x1, cid = 0x04) {
        Params = {
            own_address_type: u8 => 1,
            advertising_type: u8 => 1,
            initiator_address: BdAddrType => 7,
            advertising_interval_min: u16 => 2,
            advertising_interval_max: u16 => 2,
        };
        Constraints = {
            one_of(advertising_type, [0x01, 0x04]);
            range(advertising_interval_min, 0x0020, 0x4000);
            range(advertising_interval_max, 0x0020, 0x4000);
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
            secure_connection_support: u8 => 1,
            keypress_notification_support: bool => 1,
            encryption_key_size_min: u8 => 1,
            encryption_key_size_max: u8 => 1,
            pass_key_required: bool => 1,
            fixed_pin: PassKey => 4,
            identity_address_type: u8 => 1,
        };
        Constraints = {
            ordered(encryption_key_size_min, encryption_key_size_max);
            one_of(identity_address_type, [0x00, 0x01]);
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
            authorization: u8 => 1,
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
            advertising_type: u8 => 1,
            address_type: u8 => 1,
        };
        Constraints = {
            one_of(advertising_type, [0x02, 0x03]);
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
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
        };
        Constraints = {
            range(advertising_interval_min, 0x0020, 0x4000);
            range(advertising_interval_max, 0x0020, 0x4000);
            ordered(advertising_interval_min, advertising_interval_max);
            one_of(filter_policy, [0x00, 0x03]);
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
            flags: u16 => 2,
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
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartGeneralDiscoveryProcedure(cgid = 0x1, cid = 0x17) {
        Params = {
            scan_window: ScanWindow => 4,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartAutoConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x19) {
        Params<'a> = {
            scan_window: ScanWindow => 4,
            own_address_type: u8 => 1,
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
            scan_type: u8 => 1,
            scan_window: ScanWindow => 4,
            filter_policy: u8 => 1,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartSelectiveConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x1B) {
        Params<'a> = {
            scan_type: u8 => 1,
            scan_window: ScanWindow => 4,
            own_address_type: u8 => 1,
            filter_policy: u8 => 1,
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
            own_address_type: u8 => 1,
            conn_interval: ConnectionInterval => 8,
            expected_connection_length: ExpectedConnectionLength => 4,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapTerminateProcedure(cgid = 0x1, cid = 0x1D) {
        Params = {
            procedure: u8 => 1,
        };
        Constraints = {
            range(procedure, 1, u8::MAX);
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
            advertising_type: u8 => 1,
            own_address_type: u8 => 1,
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
            one_of(advertising_type, [0x02, 0x03]);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartObservationProcedure(cgid = 0x1, cid = 0x22) {
        Params = {
            scan_window: ScanWindow => 4,
            scan_type: u8 => 1,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
            filter_policy: u8 => 1,
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
            input_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}
vendor_cmd! {
    GapGetOobData(cgid = 0x1, cid = 0x27) {
        Params = {
            oob_data_type: u8 => 1,
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
            device_type: u8 => 1,
            address: BdAddrType => 7,
            oob_data_type: u8 => 1,
            oob_data_len: u8 => 1,
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
            mode: u8 => 1,
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
            advertising_channel_map: u8 => 1,
            own_address_type: BdAddrType => 7,
            pa_level: PowerAmplifierOutputLevel => 1,
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
            adv_mode: u8 => 1,
            adv_handle: AdvHandle => 1,
            adv_event_properties: u16 => 2,
            adv_interval: &'a ExtendedAdvertisingInterval => 8,
            primary_adv_channel_map: u8 => 1,
            own_addr_type: u8 => 1,
            peer_addr: BdAddrType => 7,
            adv_filter_policy: u8 => 1,
            adv_tx_power: u8 => 1,
            secondary_adv_max_skip: u8 => 1,
            secondary_adv_phy: u8 => 1,
            adv_sid: u8 => 1,
            scan_req_notification_enable: bool => 1,
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
            adv_handle: AdvHandle => 1,
            operation: u8 => 1,
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
            adv_handle: AdvHandle => 1,
            operation: u8 => 1,
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
            handle: AdvHandle => 1,
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
            handle: AdvHandle => 1,
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
            advertising_handle: AdvHandle => 1,
            periodic_adv_interval_min: u16 => 2,
            periodic_adv_interval_max: u16 => 2,
            periodic_adv_properties: u16 => 2,
            num_subevents: u8 => 1,
            subevent_interval: u8 => 1,
            response_slot_delay: u8 => 1,
            response_slot_spacing: u8 => 1,
            num_response_slots: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapAdvSetPeriodicData(cgid = 0x1, cid = 0x48) {
        Params<'a> = {
            advertising_handle: AdvHandle => 1,
            operation: u8 => 1,
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
            enable: u8 => 1,
            handle: AdvHandle => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapAdvSetConfigurationV2(cgid = 0x1, cid = 0x4D) {
        Params = {
            adv_mode: u8 => 1,
            adv_handle: AdvHandle => 1,
            adv_event_properties: u16 => 2,
            primary_adv_interval_min: u32 => 4,
            primary_adv_interval_max: u32 => 4,
            primary_adv_channel_map: u8 => 1,
            own_addr_type: u8 => 1,
            peer_addr: BdAddrType => 7,
            adv_filter_policy: u8 => 1,
            adv_tx_power: u8 => 1,
            primary_adv_phy: u8 => 1,
            secondary_adv_max_skip: u8 => 1,
            secondary_adv_phy: u8 => 1,
            adv_sid: u8 => 1,
            scan_req_notification_enable: bool => 1,
            primary_adv_phy_options: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapExtStartScan(cgid = 0x1, cid = 0x50) {
        Params<'a> = {
            scan_mode: u8 => 1,
            procedure: u8 => 1,
            own_address_type: u8 => 1,
            filter_duplicates: u8 => 1,
            duration: u16 => 2,
            period: u16 => 2,
            scanning_filter_policy: u8 => 1,
            scanning_phys: u8 => 1,
            phy_params: &'a [ExtScanPhyParams] => {
                kind: bitmap_items,
                bitmap: scanning_phys,
                mask: 0x05,
                item: ExtScanPhyParams => 5,
                max_items: 2,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    GapExtCreateConnection(cgid = 0x1, cid = 0x51) {
        Params<'a> = {
            initiating_mode: u8 => 1,
            procedure: u8 => 1,
            own_address_type: u8 => 1,
            peer_address_type: u8 => 1,
            peer_address: BdAddr => 6,
            advertising_handle: u8 => 1,
            subevent: u8 => 1,
            initiator_filter_policy: u8 => 1,
            initiating_phys: u8 => 1,
            phy_params: &'a [[u8; 16]] => {
                kind: bitmap_items,
                bitmap: initiating_phys,
                mask: 0x07,
                item: [u8; 16] => 16,
                max_items: 3,
            },
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

/// I/O capabilities available for the [GAP Set I/O Capability](GapSetIoCapability) command.
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum IoCapability {
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

impl crate::vendor::command::HciEncodeField<1> for IoCapability {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field(&(*self as u8), writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field_async(
            &(*self as u8),
            writer,
        )
        .await
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

/// Secure Connection support mode for [`GapSetAuthenticationRequirement`].
#[derive(Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SecureConnectionSupport {
    NotSupported = 0x00,
    Optional = 0x01,
    Mandatory = 0x02,
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

/// Options for the [GAP Authorization Response](GapAuthorizationResponse).
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Authorization {
    /// Accept the connection.
    Authorized = 0x01,
    /// Reject the connection.
    Rejected = 0x02,
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Roles for a [GAP service](CmdGapInit).
    pub struct Role: u8 {
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

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Roles for a [GAP service](CmdGapInit).
    pub struct Role: u8 {
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

impl crate::vendor::command::HciEncodeField<1> for Role {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field(&self.bits(), writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        <u8 as crate::vendor::command::HciEncodeField<1>>::write_hci_field_async(
            &self.bits(),
            writer,
        )
        .await
    }
}

/// Indicates the type of address being used in the advertising packets, for the
/// [`set_nonconnectable`](GapSetNonConnectable).
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AddressType {
    /// Public device address.
    Public = 0x00,
    /// Static random device address.
    Random = 0x01,
    /// Controller generates Resolvable Private Address.
    ResolvablePrivate = 0x02,
    /// Controller generates Resolvable Private Address. based on the local IRK from resolving
    /// list.
    NonResolvablePrivate = 0x03,
}

/// Available types of advertising data.
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AdvertisingDataType {
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
    /// Serurity Manager TK Value
    SecurityManagerTkValue = 0x10,
    /// Serurity Manager out-of-band flags
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

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Event types for [GAP Set Event Mask](GapSetEventMask).
    #[derive(Debug, Clone, Copy)]
    pub struct EventFlags: u16 {
        /// [Limited Discoverable](::event::VendorEvent::GapLimitedDiscoverableTimeout)
        const LIMITED_DISCOVERABLE_TIMEOUT = 0x0001;
        /// [Pairing Complete](::event::VendorEvent::GapPairingComplete)
        const PAIRING_COMPLETE = 0x0002;
        /// [Pass Key Request](::event::VendorEvent::GapPassKeyRequest)
        const PASS_KEY_REQUEST = 0x0004;
        /// [Authorization Request](::event::VendorEvent::GapAuthorizationRequest)
        const AUTHORIZATION_REQUEST = 0x0008;
        /// [Peripheral Security Initiated](::event::VendorEvent::GapPeripheralSecurityInitiated).
        const PERIPHERAL_SECURITY_INITIATED = 0x0010;
        /// [Bond Lost](::event::VendorEvent::GapBondLost)
        const BOND_LOST = 0x0020;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Event types for [GAP Set Event Mask](GapSetEventMask).
    pub struct EventFlags: u16 {
        /// [Limited Discoverable](::event::VendorEvent::GapLimitedDiscoverableTimeout)
        const LIMITED_DISCOVERABLE_TIMEOUT = 0x0001;
        /// [Pairing Complete](::event::VendorEvent::GapPairingComplete)
        const PAIRING_COMPLETE = 0x0002;
        /// [Pass Key Request](::event::VendorEvent::GapPassKeyRequest)
        const PASS_KEY_REQUEST = 0x0004;
        /// [Authorization Request](::event::VendorEvent::GapAuthorizationRequest)
        const AUTHORIZATION_REQUEST = 0x0008;
        /// [Peripheral Security Initiated](::event::VendorEvent::GapPeripheralSecurityInitiated).
        const PERIPHERAL_SECURITY_INITIATED = 0x0010;
        /// [Bond Lost](::event::VendorEvent::GapBondLost)
        const BOND_LOST = 0x0020;
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    /// Roles for a [GAP service](CmdGapInit).
    pub struct Procedure: u8 {
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

#[cfg(feature = "defmt")]
defmt::bitflags! {
    /// Roles for a [GAP service](CmdGapInit).
    pub struct Procedure: u8 {
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

/// Parameter for [GAP Passkey Input](GapPasskeyInput)
pub enum InputType {
    EntryStarted = 0x00,
    DigitEntered = 0x01,
    DigitErased = 0x02,
    Cleared = 0x03,
    EntryCompleted = 0x04,
}

#[derive(Clone, Copy)]
pub enum OobDataType {
    /// TK (LP v.4.1)
    TK,
    /// Random (SC)
    Random,
    /// Confirm (SC)
    Confirm,
}

#[derive(Clone, Copy)]
pub enum OobDeviceType {
    Local = 0x00,
    Remote = 0x01,
}

/// Parameter for [GAP Add Devices to List](GapAddDevicesToList)
pub enum AddDeviceToListMode {
    /// Append to the resolving list only
    AppendResoling = 0x00,
    /// clear and set the resolving list only
    ClearAndSetResolving = 0x01,
    /// append to the whitelist only
    AppendWhitelist = 0x02,
    /// clear and set the whitelist only
    ClearAndSetWhitelist = 0x03,
    /// apppend to both resolving and white lists
    AppendBoth = 0x04,
    /// clear and set both resolving and white lists
    ClearAndSetBoth = 0x05,
}

/// One record in the extended-scan PHY parameter list.
pub struct ExtScanPhyParams {
    pub scan_type: u8,
    pub scan_interval: u16,
    pub scan_window: u16,
}

impl crate::vendor::command::HciEncodeField<5> for ExtScanPhyParams {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[self.scan_type])?;
        writer.write_all(&self.scan_interval.to_le_bytes())?;
        writer.write_all(&self.scan_window.to_le_bytes())
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[self.scan_type]).await?;
        writer.write_all(&self.scan_interval.to_le_bytes()).await?;
        writer.write_all(&self.scan_window.to_le_bytes()).await
    }
}
