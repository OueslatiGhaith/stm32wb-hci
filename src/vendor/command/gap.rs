//! GAP commands and types needed for those commands.

extern crate byteorder;

pub use crate::host::{AdvertisingFilterPolicy, AdvertisingType, OwnAddressType};
use crate::types::extended_advertisement::{
    AdvSet, AdvertisingEvent, AdvertisingOperation, AdvertisingPhy, ExtendedAdvertisingInterval,
};
pub use crate::types::{ConnectionInterval, ExpectedConnectionLength, ScanWindow};
use crate::vendor::command::BoundedItems;
#[cfg(after_fw_0_17_1)]
use crate::vendor::command::HciLengthError;
use crate::vendor::event::AttributeHandle;
use crate::{AdvertisingHandle, BadStatusError, ConnectionHandle, Status};
pub use crate::{BdAddr, BdAddrType};
use crate::{
    host::{Channels, PeerAddrType, ScanFilterPolicy, ScanType},
    types::extended_advertisement::AdvertisingMode,
};
#[cfg(after_fw_0_17_1)]
use byteorder::{ByteOrder, LittleEndian};
use core::time::Duration;

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
            fixed_pin: u32 => 4,
            identity_address_type: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapSetAuthorizationRequirement(cgid = 0x1, cid = 0x07) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            authorization_required: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPassKeyResponse(cgid = 0x1, cid = 0x08) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            pin: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapAuthorizationResponse(cgid = 0x1, cid = 0x09) {
        Params = {
            conn_handle: ConnectionHandle => 2,
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
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPeripheralSecurityRequest(cgid = 0x1, cid = 0x0D) {
        Params = {
            conn_handle: ConnectionHandle => 2,
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
            conn_handle: ConnectionHandle => 2,
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
            conn_handle: ConnectionHandle => 2,
            reason: u8 => 1,
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
            conn_handle: ConnectionHandle => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartLimitedDiscoveryProcedure(cgid = 0x1, cid = 0x16) {
        Params = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartGeneralDiscoveryProcedure(cgid = 0x1, cid = 0x17) {
        Params = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            own_address_type: u8 => 1,
            filter_duplicates: bool => 1,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapStartAutoConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x19) {
        Params<'a> = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            own_address_type: u8 => 1,
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
            conn_latency: u16 => 2,
            supervision_timeout: u16 => 2,
            expected_connection_length_min: u16 => 2,
            expected_connection_length_max: u16 => 2,
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
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
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
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
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
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
            peer_address: PeerAddrType => 7,
            own_address_type: u8 => 1,
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
            conn_latency: u16 => 2,
            supervision_timeout: u16 => 2,
            expected_connection_length_min: u16 => 2,
            expected_connection_length_max: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapTerminateProcedure(cgid = 0x1, cid = 0x1D) {
        Params = {
            procedure: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartConnectionUpdate(cgid = 0x1, cid = 0x1E) {
        Params = {
            conn_handle: ConnectionHandle => 2,
            conn_interval_min: u16 => 2,
            conn_interval_max: u16 => 2,
            conn_latency: u16 => 2,
            supervision_timeout: u16 => 2,
            expected_connection_length_min: u16 => 2,
            expected_connection_length_max: u16 => 2,
        };
        Completion = CommandStatus;
    }
}

vendor_cmd! {
    GapSendPairingRequest(cgid = 0x1, cid = 0x1F) {
        Params = {
            conn_handle: ConnectionHandle => 2,
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
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapStartObservationProcedure(cgid = 0x1, cid = 0x22) {
        Params = {
            scan_interval: u16 => 2,
            scan_window: u16 => 2,
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
    pub(crate) const MAX_ADDRESSES: usize = 35;

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
            conn_handle: ConnectionHandle => 2,
            confirm_yes_no: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    GapPasskeyInput(cgid = 0x1, cid = 0x26) {
        Params = {
            conn_handle: ConnectionHandle => 2,
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
            pa_level: u8 => 1,
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
                kind: trailing_bytes,
                min_len: 0,
                max_len: 255,
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
            adv_handle: AdvertisingHandle => 1,
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
            adv_handle: AdvertisingHandle => 1,
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
            adv_handle: AdvertisingHandle => 1,
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
            conn_handle: ConnectionHandle => 2,
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
            advertising_handle: AdvertisingHandle => 1,
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
            adv_mode: u8 => 1,
            adv_handle: AdvertisingHandle => 1,
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

/// Potential errors from parameter validation.
///
/// Before some commands are sent to the controller, the parameters are validated. This type
/// enumerates the potential validation errors. Must be specialized on the types of communication
/// errors.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// For the [GAP Set Limited Discoverable](GapSetLimitedDiscoverable) and
    /// [GAP Set Discoverable](GapSetDiscoverable) commands, the connection
    /// interval is inverted (the min is greater than the max).  Return the provided min as the
    /// first element, max as the second.
    BadConnectionInterval(Duration, Duration),

    /// For the [GAP Set Limited Discoverable](GapSetLimitedDiscoverable) and
    /// [GAP Set Broadcast Mode](GapSetBroadcastMode) commands, the advertising
    /// type is disallowed.  Returns the invalid advertising type.
    BadAdvertisingType(crate::types::AdvertisingType),

    /// For the [GAP Set Limited Discoverable](GapSetLimitedDiscoverable)
    /// command, the advertising interval is inverted (that is, the max is less than the
    /// min). Includes the provided range.
    BadAdvertisingInterval(Duration, Duration),

    /// For the [GAP Set Authentication Requirement](GapSetAuthenticationRequirement)
    /// command, the encryption key size range is inverted (the max is less than the min). Includes the provided range.
    BadEncryptionKeySizeRange(u8, u8),

    /// For the [GAP Set Authentication Requirement](GapSetAuthenticationRequirement)
    /// command, the address type must be either Public or Random
    BadAddressType(AddressType),

    BadPowerAmplifierLevel(u8),

    /// For the [GAP Set Authentication Requirement](GapSetAuthenticationRequirement) and
    /// [GAP Pass Key Response](GapPassKeyResponse) commands, the provided fixed pin is out of
    /// range (must be less than or equal to 999999).  Includes the provided PIN.
    BadFixedPin(u32),

    /// For the [GAP Set Undirected Connectable](GapSetUnidirectedConnectable) command, the
    /// advertising filter policy is not one of the allowed values. Only
    /// [AllowConnectionAndScan](crate::host::AdvertisingFilterPolicy::AllowConnectionAndScan) and
    /// [WhiteListConnectionAndScan](crate::host::AdvertisingFilterPolicy::WhiteListConnectionAndScan) are
    /// allowed.
    BadAdvertisingFilterPolicy(crate::host::AdvertisingFilterPolicy),

    /// For the [GAP Update Advertising Data](GapUpdateAdvertisingData) and
    /// [GAP Set Broadcast Mode](GapSetBroadcastMode) commands, the advertising data
    /// is too long. It must be 31 bytes or less. The length of the provided data is returned.
    BadAdvertisingDataLength(usize),

    /// For extended scanning, the PHY bitmap selects an unsupported bit, or
    /// the number of per-PHY records differs from the selected-bit count.
    #[cfg(after_fw_0_17_1)]
    BadExtendedScanParameters(HciLengthError),

    /// For the [GAP Terminate](GapTerminate) command, the termination reason was
    /// not one of the allowed reason. The reason is returned.
    BadTerminationReason(crate::Status),

    /// For the [GAP Start Auto Connection Establishment](GapStartAutoConnectionEstablishmentProcedure) or
    /// [GAP Start Selective Connection Establishment](GapStartSelectiveConnectionEstablishmentProcedure) commands, the
    /// provided [white list](AutoConnectionEstablishmentParameters::white_list) has more than 33
    /// or 35 entries, respectively, which would cause the command to be longer than 255 bytes.
    ///
    /// For the [GAP Set Broadcast Mode](GapSetBroadcastMode), the provided
    /// [white list](BroadcastModeParameters::white_list) the maximum number of entries ranges
    /// from 31 to 35, depending on the length of the advertising data.
    WhiteListTooLong,

    /// For the [GAP Terminate Procedure](GapTerminateProcedure) command, the
    /// provided bitfield had no bits set.
    NoProcedure,

    /// Event Parsing Error
    ParseError(crate::event::Error),

    /// An error occurred during execution of the command
    HciError(Status),

    /// An error occurred during execution of the command
    UnknownHciError(u8),

    /// An internal error occurred during execution of the controller. This is a bug.
    IoError,
}

impl<T> From<bt_hci::cmd::Error<T>> for Error {
    fn from(err: bt_hci::cmd::Error<T>) -> Self {
        match err {
            bt_hci::cmd::Error::Io(_) => Self::IoError,
            bt_hci::cmd::Error::Hci(err) => match Status::try_from(err.to_status().into_inner()) {
                Ok(status) => Self::HciError(status),
                Err(BadStatusError::BadValue(status)) => Self::UnknownHciError(status),
            },
        }
    }
}

impl From<crate::event::Error> for Error {
    fn from(e: crate::event::Error) -> Self {
        Self::ParseError(e)
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

/// Parameters for the
/// [`set_limited_discoverable`](GapSetLimitedDiscoverable) and
/// [`set_discoverable`](GapSetDiscoverable) commands.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DiscoverableParameters<'a, 'b> {
    /// Advertising method for the device.
    ///
    /// Must be
    /// [ConnectableUndirected](crate::host::AdvertisingType::ConnectableUndirected),
    /// [ScannableUndirected](crate::host::AdvertisingType::ScannableUndirected), or
    /// [NonConnectableUndirected](crate::host::AdvertisingType::NonConnectableUndirected).
    pub advertising_type: AdvertisingType,

    /// Range of advertising for non-directed advertising.
    ///
    /// If not provided, the GAP will use default values (1.28 seconds).
    ///
    /// Range for both limits: 20 ms to 10.24 seconds.  The second value must be greater than or
    /// equal to the first.
    pub advertising_interval: Option<(Duration, Duration)>,

    /// Address type for this device.
    pub address_type: OwnAddressType,

    /// Filter policy for this device.
    pub filter_policy: AdvertisingFilterPolicy,

    /// Name of the device.
    pub local_name: Option<LocalName<'a>>,

    /// Service UUID list as defined in the Bluetooth spec, v4.1, Vol 3, Part C, Section 11.
    ///
    /// Must be 31 bytes or fewer.
    pub advertising_data: &'b [u8],

    /// Expected length of the connection to the peripheral.
    pub conn_interval: (Option<Duration>, Option<Duration>),
}

/// Allowed types for the local name.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LocalName<'a> {
    /// The shortened local name.
    Shortened(&'a [u8]),

    /// The complete local name.
    Complete(&'a [u8]),
}

/// Parameters for the
/// [`set_undirected_connectable`](GapSetUnidirectedConnectable) command.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct UndirectedConnectableParameters {
    /// Range of advertising interval for advertising.
    ///
    /// Range for both limits: 20 ms to 10.24 seconds.  The second value must be greater than or
    /// equal to the first.
    pub advertising_interval: (Duration, Duration),

    /// Address type of this device.
    pub own_address_type: OwnAddressType,

    /// filter policy for this device
    pub filter_policy: AdvertisingFilterPolicy,
}

/// Parameters for the
/// [`set_direct_connectable`](GapSetDirectConnectable) command.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DirectConnectableParameters {
    /// Address type of this device.
    pub own_address_type: OwnAddressType,

    /// Advertising method for the device.
    ///
    /// Must be
    /// [ConnectableDirectedHighDutyCycle](crate::host::AdvertisingType::ConnectableDirectedHighDutyCycle),
    /// or
    /// [ConnectableDirectedLowDutyCycle](crate::host::AdvertisingType::ConnectableDirectedLowDutyCycle).
    pub advertising_type: AdvertisingType,

    /// Initiator's Bluetooth address.
    pub initiator_address: BdAddrType,

    /// Range of advertising interval for advertising.
    ///
    /// Range for both limits: 20 ms to 10.24 seconds.  The second value must be greater than or
    /// equal to the first.
    pub advertising_interval: (Duration, Duration),
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

/// Parameters for the [GAP Set Authentication Requirement](GapSetAuthenticationRequirement) command.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AuthenticationRequirements {
    /// Is bonding required?
    pub bonding_required: bool,

    /// Is MITM (man-in-the-middle) protection required?
    pub mitm_protection_required: bool,

    /// is secure connection support required
    pub secure_connection_support: SecureConnectionSupport,

    /// is keypress notification support required
    pub keypress_notification_support: bool,

    /// Minimum and maximum size of the encryption key.
    pub encryption_key_size_range: (u8, u8),

    /// Pin to use during the pairing process.
    pub fixed_pin: Pin,

    /// identity address type.
    pub identity_address_type: AddressType,
}

/// Options for out-of-band authentication.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OutOfBandAuthentication {
    /// Out Of Band authentication not enabled
    Disabled,
    /// Out Of Band authentication enabled; includes the OOB data.
    Enabled([u8; 16]),
}

/// Options for [`secure_connection_support`](AuthenticationRequirements)
#[derive(Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SecureConnectionSupport {
    NotSupported = 0x00,
    Optional = 0x01,
    Mandatory = 0x02,
}

/// Options for [`fixed_pin`](AuthenticationRequirements).
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

/// Parameters for the [GAP Limited Discovery](GapStartLimitedDiscoveryProcedure) and
/// [GAP General Discovery](GapStartGeneralDiscoveryProcedure) procedures.
pub struct DiscoveryProcedureParameters {
    /// Scanning window for the discovery procedure.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// If true, duplicate devices are filtered out.
    pub filter_duplicates: bool,
}

/// Parameters for the GAP Name Discovery
/// procedure.
pub struct NameDiscoveryProcedureParameters {
    /// Scanning window for the discovery procedure.
    pub scan_window: ScanWindow,

    /// Address of the connected device
    pub peer_address: crate::host::PeerAddrType,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Connection interval parameters.
    pub conn_interval: ConnectionInterval,

    /// Expected connection length
    pub expected_connection_length: ExpectedConnectionLength,
}

/// Parameters for the
/// [GAP Start Auto Connection Establishment](GapStartAutoConnectionEstablishmentProcedure) command.
pub struct AutoConnectionEstablishmentParameters<'a> {
    /// Scanning window for connection establishment.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Connection interval parameters.
    pub conn_interval: ConnectionInterval,

    /// Expected connection length
    pub expected_connection_length: ExpectedConnectionLength,

    /// Addresses to white-list for automatic connection.
    pub white_list: &'a [crate::host::PeerAddrType],
}

/// Parameters for the
/// [GAP Start General Connection Establishment](GapStartGeneralConnectionEstablishmentProcedure) command.
pub struct GeneralConnectionEstablishmentParameters {
    /// passive or active scanning. With passive scanning, no scan request PDUs are sent
    pub scan_type: ScanType,

    /// Scanning window for connection establishment.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Scanning filter policy.
    ///
    /// # Note
    /// if privacy is enabled, filter policy can only assume values
    /// [Accept All](ScanFilterPolicy::AcceptAll) or
    /// [Addressed To This Device](ScanFilterPolicy::AddressedToThisDevice)
    pub filter_policy: ScanFilterPolicy,

    /// If true, only report unique devices.
    pub filter_duplicates: bool,
}

/// Parameters for the
/// [GAP Start Selective Connection Establishment](GapStartSelectiveConnectionEstablishmentProcedure) command.
pub struct SelectiveConnectionEstablishmentParameters<'a> {
    /// Type of scanning
    pub scan_type: crate::host::ScanType,

    /// Scanning window for connection establishment.
    pub scan_window: ScanWindow,

    /// Address type of this device.
    pub own_address_type: crate::host::OwnAddressType,

    /// Scanning filter policy.
    ///
    /// # Note
    /// if privacy is enabled, filter policy can only assume values
    /// [Accept All](ScanFilterPolicy::AcceptAll) or
    /// [Whitelist Addressed to this Device](ScanFilterPolicy::WhiteListAddressedToThisDevice)
    pub filter_policy: ScanFilterPolicy,

    /// If true, only report unique devices.
    pub filter_duplicates: bool,

    /// Addresses to white-list for automatic connection.
    pub white_list: &'a [crate::host::PeerAddrType],
}

/// The parameters for the GAP Name Discovery
/// and [GAP Create Connection](GapCreateConnection) commands are identical.
pub type ConnectionParameters = NameDiscoveryProcedureParameters;

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

/// Parameters for the [`start_connection_update`](GapStartConnectionUpdate)
/// command.
pub struct ConnectionUpdateParameters {
    /// Handle of the connection for which the update procedure has to be started.
    pub conn_handle: crate::ConnectionHandle,

    /// Updated connection interval for the connection.
    pub conn_interval: ConnectionInterval,

    /// Expected length of connection event needed for this connection.
    pub expected_connection_length: ExpectedConnectionLength,
}

/// Parameters for the [`send_pairing_request`](GapSendPairingRequest)
/// command.
pub struct PairingRequest {
    /// Handle of the connection for which the pairing request has to be sent.
    pub conn_handle: crate::ConnectionHandle,

    /// Whether pairing request has to be sent if the device is previously bonded or not. If false,
    /// the pairing request is sent only if the device has not previously bonded.
    pub force_rebond: bool,
}

/// Parameters for the [GAP Set Broadcast Mode](GapSetBroadcastMode) command.
pub struct BroadcastModeParameters<'a, 'b> {
    /// Advertising type and interval.
    ///
    /// Only the [ScannableUndirected](crate::types::AdvertisingType::ScannableUndirected) and
    /// [NonConnectableUndirected](crate::types::AdvertisingType::NonConnectableUndirected).
    pub advertising_interval: crate::types::AdvertisingInterval,

    /// Type of this device's address.
    ///
    /// A privacy enabled device uses either a
    /// [resolvable private address](AddressType::ResolvablePrivate) or a
    /// [non-resolvable private](AddressType::NonResolvablePrivate) address.
    pub own_address_type: AddressType,

    /// Advertising data used by the device when advertising.
    ///
    /// Must be 31 bytes or fewer.
    pub advertising_data: &'a [u8],

    /// Addresses to add to the white list.
    ///
    /// Each address takes up 7 bytes (1 byte for the type, 6 for the address). The full length of
    /// this packet must not exceed 255 bytes. The white list must be less than a maximum of between
    /// 31 and 35 entries, depending on the length of
    /// [`advertising_data`](BroadcastModeParameters::advertising_data). Shorter advertising data
    /// allows more white list entries.
    pub white_list: &'b [crate::host::PeerAddrType],
}

/// Parameters for the [GAP Start Observation Procedure](GapStartObservationProcedure)
/// command.
pub struct ObservationProcedureParameters {
    /// Scanning window.
    pub scan_window: crate::types::ScanWindow,

    /// Active or passive scanning
    pub scan_type: crate::host::ScanType,

    /// Address type of this device.
    pub own_address_type: AddressType,

    /// If true, do not report duplicate events in the
    /// [advertising report](crate::event::Event::LeAdvertisingReport).
    pub filter_duplicates: bool,

    /// Scanning filter policy
    pub filter_policy: ScanFilterPolicy,
}

/// Parameters for [GAP Numeric Comparison Confirm Yes or No](crate::vendor::command::gap::GapConfirmNumericComparisonValue)
pub struct NumericComparisonValueConfirmYesNoParameters {
    /// Connection handle for which the command applies.
    pub conn_handle: ConnectionHandle,

    /// Indicates if the numeric values shown on both local and peer device are different or equal.
    pub confirm_yes_no: bool,
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

/// Parameters for [GAP Set OOB Data](GapSetOobData)
pub struct SetOobDataParameters {
    /// OOB Device type
    pub device_type: OobDeviceType,
    /// Identity address
    pub address: BdAddrType,
    /// OOB Data type
    pub oob_data_type: OobDataType,
    /// Pairing Data received through OOB from remote device
    pub oob_data: [u8; 16],
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

/// Parameters for [GAP Additional Beacon Start](GapAdditionalBeaconStart)
pub struct AdditonalBeaconStartParameters {
    /// Advertising interval
    pub advertising_interval: (Duration, Duration),
    /// advertising channel map
    pub advertising_channel_map: Channels,
    /// Own address type
    pub own_address_type: BdAddrType,
    /// Power amplifier output level. Range: 0x00 .. 0x23
    pub pa_level: u8,
}

/// Params for the [adv_set_config](GapAdvSetConfig) command
pub struct AdvSetConfig {
    /// Bitmap of extended advertising modes
    pub adv_mode: AdvertisingMode,
    /// Used to identify an advertising set
    pub adv_handle: AdvertisingHandle,
    /// Type of advertising event
    pub adv_event_properties: AdvertisingEvent,
    /// Advertising interval
    pub adv_interval: ExtendedAdvertisingInterval,
    /// Advertising channel map
    pub primary_adv_channel_map: Channels,
    /// Own address type.
    ///
    /// If privacy is disabled, the address can be public or static random, otherwise,
    /// it can be a resolvable private address or a non-resolvabble private address.
    pub own_addr_type: OwnAddressType,
    /// Public device address, random device addressm public identity address, or random
    /// (static) identity address of the device to be connected.
    pub peer_addr: BdAddrType,
    /// Advertising filter policy
    pub adv_filter_policy: AdvertisingFilterPolicy,
    /// Advertising TX power. Units; dBm.
    ///
    /// Values;
    /// - -127 .. 20
    pub adv_tx_power: u8,
    /// Secondary advertising maximum skip.
    ///
    /// Values:
    /// - 0x00: `AUX_QDV_IND` shall be sent prior to the next advertising event
    /// - 0x01 .. 0xFF: Maximum advertising events to the Controller can skip
    ///   before sending the `AUX_QDV_IND` packets on the secondary physical channel.
    pub secondary_adv_max_skip: u8,
    /// Secondary advertising PHY
    pub secondary_adv_phy: AdvertisingPhy,
    /// Value of advertising SID subfield in the ADI field of the PDU.
    ///
    /// Values:
    /// - 0x00 .. 0x0F
    pub adv_sid: u8,
    /// Scan request notifications
    pub scan_req_notification_enable: bool,
}

/// Params for the [adv_set_enable](GapAdvSetEnable) command
pub struct AdvSetEnable<'a> {
    /// Enable/Disable advertising
    pub enable: bool,
    /// Number of advertising sets.
    ///
    /// Values
    /// - 0x00: disable all advertising sets
    /// - 0x01 .. 0x3F: Number of advertising sets to enable or disable
    pub num_sets: u8,
    /// Advertising sets
    pub adv_set: &'a [AdvSet],
}

/// Params for the [adv_set_advertising_data](GapAdvSetAdvertisingData) command
pub struct AdvSetAdvertisingData<'a> {
    /// Used to identify an advertising set
    pub adv_handle: AdvertisingHandle,
    /// Advertising operation
    pub operation: AdvertisingOperation,
    /// Fragment preference. If set to `true`, the Controller may fragment all data, else
    /// the Controller should not fragment or should minimize fragmentation of data
    pub fragment: bool,
    /// Data formatted as defined in Bluetooth spec. v.5.4 [Vol 3, Part C, 11].
    pub data: &'a [u8],
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [adv_set_periodic_parameters](GapAdvSetPeriodicParameters).
pub struct AdvSetPeriodicParameters {
    pub advertising_handle: AdvertisingHandle,
    pub periodic_adv_interval_min: u16,
    pub periodic_adv_interval_max: u16,
    pub periodic_adv_properties: u16,
    pub num_subevents: u8,
    pub subevent_interval: u8,
    pub response_slot_delay: u8,
    pub response_slot_spacing: u8,
    pub num_response_slots: u8,
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [adv_set_periodic_data](GapAdvSetPeriodicData).
pub struct AdvSetPeriodicData<'a> {
    pub advertising_handle: AdvertisingHandle,
    pub operation: AdvertisingOperation,
    pub data: &'a [u8],
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [adv_set_configuration_v2](GapAdvSetConfigurationV2).
///
/// Like [AdvSetConfig] but uses 4-byte primary advertising intervals and adds PHY fields.
pub struct AdvSetConfigV2 {
    pub adv_mode: AdvertisingMode,
    pub adv_handle: AdvertisingHandle,
    pub adv_event_properties: AdvertisingEvent,
    /// Minimum primary advertising interval (N * 0.625 ms).
    pub primary_adv_interval_min: u32,
    /// Maximum primary advertising interval (N * 0.625 ms).
    pub primary_adv_interval_max: u32,
    pub primary_adv_channel_map: Channels,
    pub own_addr_type: OwnAddressType,
    pub peer_addr: BdAddrType,
    pub adv_filter_policy: AdvertisingFilterPolicy,
    pub adv_tx_power: u8,
    pub primary_adv_phy: AdvertisingPhy,
    pub secondary_adv_max_skip: u8,
    pub secondary_adv_phy: AdvertisingPhy,
    pub adv_sid: u8,
    pub scan_req_notification_enable: bool,
    pub primary_adv_phy_options: u8,
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

#[cfg(after_fw_0_17_1)]
/// Parameters for [ext_start_scan](GapExtStartScan).
pub struct ExtStartScanParams {
    pub scan_mode: u8,
    pub procedure: u8,
    pub own_address_type: u8,
    pub filter_duplicates: u8,
    pub duration: u16,
    pub period: u16,
    pub scanning_filter_policy: u8,
    pub scanning_phys: u8,
    /// Per-PHY parameters (one entry per set bit in scanning_phys, max 2).
    pub phy_params: [ExtScanPhyParams; 2],
    pub num_phys: usize,
}

#[cfg(after_fw_0_17_1)]
/// Per-PHY connection parameters for [ExtCreateConnectionParams].
pub struct ExtConnPhyParams {
    pub scan_interval: u16,
    pub scan_window: u16,
    pub conn_interval_min: u16,
    pub conn_interval_max: u16,
    pub conn_latency: u16,
    pub supervision_timeout: u16,
    pub min_ce_length: u16,
    pub max_ce_length: u16,
}

#[cfg(after_fw_0_17_1)]
/// Parameters for [ext_create_connection](GapExtCreateConnection).
pub struct ExtCreateConnectionParams {
    pub initiating_mode: u8,
    pub procedure: u8,
    pub own_address_type: u8,
    pub peer_address_type: u8,
    pub peer_address: BdAddr,
    pub advertising_handle: u8,
    pub subevent: u8,
    pub initiator_filter_policy: u8,
    pub initiating_phys: u8,
    /// Per-PHY parameters (one entry per set bit in initiating_phys, max 3).
    pub phy_params: [ExtConnPhyParams; 3],
    pub num_phys: usize,
}
