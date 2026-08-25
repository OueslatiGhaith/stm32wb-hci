//! GAP commands and types needed for those commands.

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
use crate::types::AttributeHandle;
pub use crate::types::BdAddrType;
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    all(feature = "stack-light", before_fw_1_23_0),
))]
use crate::types::PeerAddrType;
#[cfg(feature = "stack-full-extended")]
pub use crate::types::extended_advertisement::AdvertisingHandle;
#[cfg(feature = "stack-full-extended")]
use crate::types::extended_advertisement::{
    AdvSet, AdvertisingEvent, AdvertisingMode, AdvertisingOperation, AdvertisingPhy,
    ExtendedAdvertisingInterval,
};
pub use crate::types::{
    AdvertisingFilterPolicy, AdvertisingType, ConnectionInterval, ExpectedConnectionLength,
    OwnAddressType, ScanWindow,
};
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
use crate::vendor::command::BoundedItems;
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
use bt_hci::param::{BdAddr, ConnHandle};

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Six-digit GAP pass key.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PassKey: u32 => 4 {
        minimum: 0,
        maximum: 999_999,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Maximum number of consecutive auxiliary advertising events the
    /// controller may skip.
    ///
    /// The Bluetooth command assigns meaning to the complete byte domain.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct SecondaryAdvertisingMaximumSkip: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Legacy advertising-interval bound in 0.625 ms units.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct AdvertisingIntervalBound: u16 => 2 {
        minimum: 0x0020,
        maximum: 0x4000,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Optional legacy advertising-interval bound in 0.625 ms units.
    ///
    /// Discoverable procedures accept zero for both bounds to let the
    /// controller select an interval. Whether both fields use that sentinel is
    /// a command-level relationship and remains declarative in `vendor_cmd!`.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct OptionalAdvertisingIntervalBound: u16 => 2 {
        minimum: 0x0020,
        maximum: 0x4000,
        sentinel: CONTROLLER_SELECTED = 0x0000,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Advertising set identifier carried in the four-bit SID field.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct AdvertisingSid: u8 => 1 {
        minimum: 0x00,
        maximum: 0x0F,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Power-amplifier output level accepted by the additional-beacon command.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct PowerAmplifierOutputLevel: u8 => 1 {
        minimum: 0,
        maximum: 0x23,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Privacy behavior selected while initializing the GAP service.
    ///
    /// CubeWB uses `0x02`, rather than the usual Boolean `0x01`, to enable
    /// privacy for this command.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum PrivacyMode: u8 => 1 {
        Disabled = 0x00,
        Enabled = 0x02,
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetNonDiscoverable(cgid = 0x1, cid = 0x01) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetLimitedDiscoverable(cgid = 0x1, cid = 0x02) {
        Params<'a> = {
            advertising_type: AdvertisingType,
            advertising_interval_min: OptionalAdvertisingIntervalBound,
            advertising_interval_max: OptionalAdvertisingIntervalBound,
            own_address_type: AddressType,
            filter_policy: AdvertisingFilterPolicy,
            local_name: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 242,
            },
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 31,
                storage_max_len: 243,
            },
            conn_interval_min: u16,
            conn_interval_max: u16,
        };
        Constraints = {
            self.advertising_type in [
                AdvertisingType::ConnectableUndirected,
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ];
            (self.advertising_interval_min
                == OptionalAdvertisingIntervalBound::CONTROLLER_SELECTED)
                iff (self.advertising_interval_max
                    == OptionalAdvertisingIntervalBound::CONTROLLER_SELECTED);
            self.advertising_interval_min <= self.advertising_interval_max;
            self.conn_interval_min in [0, 0xFFFF]
                || self.conn_interval_min in 0x0006..=0x0C80;
            self.conn_interval_max in [0, 0xFFFF]
                || self.conn_interval_max in 0x0006..=0x0C80;
            (self.conn_interval_min in 0x0006..=0x0C80
                && self.conn_interval_max in 0x0006..=0x0C80)
                implies self.conn_interval_min <= self.conn_interval_max;
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetDiscoverable(cgid = 0x1, cid = 0x03) {
        Params<'a> = {
            advertising_type: AdvertisingType,
            advertising_interval_min: OptionalAdvertisingIntervalBound,
            advertising_interval_max: OptionalAdvertisingIntervalBound,
            own_address_type: AddressType,
            filter_policy: AdvertisingFilterPolicy,
            local_name: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 242,
            },
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 31,
                storage_max_len: 243,
            },
            conn_interval_min: u16,
            conn_interval_max: u16,
        };
        Constraints = {
            self.advertising_type in [
                AdvertisingType::ConnectableUndirected,
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ];
            (self.advertising_interval_min
                == OptionalAdvertisingIntervalBound::CONTROLLER_SELECTED)
                iff (self.advertising_interval_max
                    == OptionalAdvertisingIntervalBound::CONTROLLER_SELECTED);
            self.advertising_interval_min <= self.advertising_interval_max;
            self.conn_interval_min in [0, 0xFFFF]
                || self.conn_interval_min in 0x0006..=0x0C80;
            self.conn_interval_max in [0, 0xFFFF]
                || self.conn_interval_max in 0x0006..=0x0C80;
            (self.conn_interval_min in 0x0006..=0x0C80
                && self.conn_interval_max in 0x0006..=0x0C80)
                implies self.conn_interval_min <= self.conn_interval_max;
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetDirectConnectable(cgid = 0x1, cid = 0x04) {
        Params = {
            own_address_type: AddressType,
            advertising_type: AdvertisingType,
            initiator_address: BdAddrType,
            advertising_interval_min: u16,
            advertising_interval_max: u16,
        };
        Constraints = {
            self.advertising_type in [
                AdvertisingType::ConnectableDirectedHighDutyCycle,
                AdvertisingType::ConnectableDirectedLowDutyCycle,
            ];
            self.advertising_type
                == AdvertisingType::ConnectableDirectedHighDutyCycle
                implies self.advertising_interval_min == 0x0006;
            self.advertising_type
                == AdvertisingType::ConnectableDirectedHighDutyCycle
                implies self.advertising_interval_max == 0x0006;
            self.advertising_type
                == AdvertisingType::ConnectableDirectedLowDutyCycle
                implies self.advertising_interval_min in 0x0020..=0x4000;
            self.advertising_type
                == AdvertisingType::ConnectableDirectedLowDutyCycle
                implies self.advertising_interval_max in 0x0020..=0x4000;
            self.advertising_interval_min <= self.advertising_interval_max;
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetIoCapability(cgid = 0x1, cid = 0x05) {
        Params = {
            io_capability: IoCapability,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetAuthenticationRequirement(cgid = 0x1, cid = 0x06) {
        Params = {
            bonding_required: bool,
            mitm_protection_required: bool,
            secure_connection_support: SecureConnectionSupport,
            keypress_notification_support: bool,
            encryption_key_size_min: u8,
            encryption_key_size_max: u8,
            pass_key_required: bool,
            fixed_pin: PassKey,
            identity_address_type: AddressType,
        };
        Constraints = {
            self.encryption_key_size_min in 7..=16;
            self.encryption_key_size_max in 7..=16;
            self.encryption_key_size_min <= self.encryption_key_size_max;
            self.identity_address_type in [
                AddressType::Public,
                AddressType::Random,
            ];
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetAuthorizationRequirement(cgid = 0x1, cid = 0x07) {
        Params = {
            conn_handle: ConnHandle,
            authorization_required: bool,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapPassKeyResponse(cgid = 0x1, cid = 0x08) {
        Params = {
            conn_handle: ConnHandle,
            pin: PassKey,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapAuthorizationResponse(cgid = 0x1, cid = 0x09) {
        Params = {
            conn_handle: ConnHandle,
            authorization: Authorization,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    CmdGapInit(cgid = 0x1, cid = 0x0A) {
        Params = {
            role: Role,
            privacy: PrivacyMode,
            // CubeWB defines no narrower numeric domain for this capacity.
            dev_name_characteristic_len: u8,
        };
        Constraints = {
            !self.role.is_empty();
        };
        Completion = CommandComplete;
        Return = GapInit {
            service_handle: AttributeHandle,
            dev_name_handle: AttributeHandle,
            appearance_handle: AttributeHandle,
        };
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetNonConnectable(cgid = 0x1, cid = 0x0B) {
        Params = {
            advertising_type: AdvertisingType,
            address_type: AddressType,
        };
        Constraints = {
            self.advertising_type in [
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ];
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetUnidirectedConnectable(cgid = 0x1, cid = 0x0C) {
        Params = {
            advertising_interval_min: AdvertisingIntervalBound,
            advertising_interval_max: AdvertisingIntervalBound,
            own_address_type: AddressType,
            filter_policy: AdvertisingFilterPolicy,
        };
        Constraints = {
            self.advertising_interval_min <= self.advertising_interval_max;
            self.filter_policy in [
                AdvertisingFilterPolicy::AllowConnectionAndScan,
                AdvertisingFilterPolicy::WhiteListConnectionAndScan,
            ];
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapPeripheralSecurityRequest(cgid = 0x1, cid = 0x0D) {
        Params = {
            conn_handle: ConnHandle,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapUpdateAdvertisingData(cgid = 0x1, cid = 0x0E) {
        Params<'a> = {
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 31,
                storage_max_len: 255,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapDeleteAdType(cgid = 0x1, cid = 0x0F) {
        Params = {
            // Bluetooth AD types are an open registry, so this remains a raw
            // byte rather than pretending the legacy enum is exhaustive.
            ad_type: u8,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapGetSecurityLevel(cgid = 0x1, cid = 0x10) {
        Params = {
            conn_handle: ConnHandle,
        };
        Completion = CommandComplete;
        Return = GapSecurityLevelReturn {
            security_mode: u8,
            security_level: u8,
        };
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetEventMask(cgid = 0x1, cid = 0x11) {
        Params = {
            flags: EventFlags,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapConfigureWhitelist(cgid = 0x1, cid = 0x12) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapTerminate(cgid = 0x1, cid = 0x13) {
        Params = {
            conn_handle: ConnHandle,
            reason: TerminationReason,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapClearSecurityDatabase(cgid = 0x1, cid = 0x14) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapAllowRebond(cgid = 0x1, cid = 0x15) {
        Params = {
            conn_handle: ConnHandle,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapStartLimitedDiscoveryProcedure(cgid = 0x1, cid = 0x16) {
        Params = {
            scan_window: ScanWindow,
            own_address_type: AddressType,
            filter_duplicates: bool,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapStartGeneralDiscoveryProcedure(cgid = 0x1, cid = 0x17) {
        Params = {
            scan_window: ScanWindow,
            own_address_type: AddressType,
            filter_duplicates: bool,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapStartAutoConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x19) {
        Params<'a> = {
            scan_window: ScanWindow,
            own_address_type: AddressType,
            conn_interval: ConnectionInterval,
            expected_connection_length: ExpectedConnectionLength,
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8,
                item: PeerAddrType,
                max_items: 33,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapStartGeneralConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x1A) {
        Params = {
            scan_type: ScanType,
            scan_window: ScanWindow,
            filter_policy: ScanningFilterPolicy,
            own_address_type: AddressType,
            filter_duplicates: bool,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapStartSelectiveConnectionEstablishmentProcedure(cgid = 0x1, cid = 0x1B) {
        Params<'a> = {
            scan_type: ScanType,
            scan_window: ScanWindow,
            own_address_type: AddressType,
            filter_policy: ScanningFilterPolicy,
            filter_duplicates: bool,
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8,
                item: PeerAddrType,
                max_items: 35,
            },
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapCreateConnection(cgid = 0x1, cid = 0x1C) {
        Params = {
            scan_window: ScanWindow,
            peer_address: PeerAddrType,
            own_address_type: AddressType,
            conn_interval: ConnectionInterval,
            expected_connection_length: ExpectedConnectionLength,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapTerminateProcedure(cgid = 0x1, cid = 0x1D) {
        Params = {
            procedure: Procedure,
        };
        Constraints = {
            !self.procedure.is_empty();
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapStartConnectionUpdate(cgid = 0x1, cid = 0x1E) {
        Params = {
            conn_handle: ConnHandle,
            conn_interval: ConnectionInterval,
            expected_connection_length: ExpectedConnectionLength,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapSendPairingRequest(cgid = 0x1, cid = 0x1F) {
        Params = {
            conn_handle: ConnHandle,
            force_rebond: bool,
        };
        Completion = CommandStatus;
    }
}

#[cfg(before_fw_1_22_0)]
#[cfg(any(feature = "stack-full-extended", feature = "stack-full"))]
stm32wb_hci_macros::vendor_cmd! {
    CmdGapResolvePrivateAddress(cgid = 0x1, cid = 0x20) {
        Params = {
            address: BdAddr,
        };
        Completion = CommandComplete;
        Return = GapResolvedPrivateAddress {
            address: BdAddr,
        };
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetBroadcastMode(cgid = 0x1, cid = 0x21) {
        Params<'a> = {
            advertising_interval_min: AdvertisingIntervalBound,
            advertising_interval_max: AdvertisingIntervalBound,
            advertising_type: AdvertisingType,
            own_address_type: AddressType,
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 31,
                storage_max_len: 248,
            },
            white_list: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8,
                item: PeerAddrType,
                max_items: 35,
            },
        };
        Constraints = {
            self.advertising_interval_min <= self.advertising_interval_max;
            self.advertising_type in [
                AdvertisingType::ScannableUndirected,
                AdvertisingType::NonConnectableUndirected,
            ];
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(feature = "stack-full-extended", feature = "stack-full",))]
stm32wb_hci_macros::vendor_cmd! {
    GapStartObservationProcedure(cgid = 0x1, cid = 0x22) {
        Params = {
            scan_window: ScanWindow,
            scan_type: ScanType,
            own_address_type: AddressType,
            filter_duplicates: bool,
            filter_policy: ScanningFilterPolicy,
        };
        Completion = CommandStatus;
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapGetBondedDevices(cgid = 0x1, cid = 0x23) {
        Params = ();
        Completion = CommandComplete;
        Return = GapBondedDevices {
            addresses: BoundedItems<BdAddrType, 35> => {
                kind: counted_items,
                count: u8,
                item: BdAddrType,
                max_items: 35,
            },
        };
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
impl GapBondedDevices {
    /// Addresses reported by the controller.
    pub fn bonded_addresses(&self) -> &[BdAddrType] {
        self.addresses.as_slice()
    }
}

#[cfg(before_fw_1_22_0)]
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light"
))]
stm32wb_hci_macros::vendor_cmd! {
    GapIsDeviceBonded(cgid = 0x1, cid = 0x24) {
        Params = {
            address: PeerAddrType,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_1_22_0)]
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapCheckBondedDevice(cgid = 0x1, cid = 0x24) {
        Params = {
            address: BdAddrType,
        };
        Completion = CommandComplete;
        Return = GapCheckBondedDeviceReturn {
            identity_address: BdAddrType,
        };
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapConfirmNumericComparisonValue(cgid = 0x1, cid = 0x25) {
        Params = {
            conn_handle: ConnHandle,
            confirm_yes_no: bool,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapPasskeyInput(cgid = 0x1, cid = 0x26) {
        Params = {
            conn_handle: ConnHandle,
            input_type: InputType,
        };
        Completion = CommandComplete;
        Return = ();
    }
}
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapGetOobData(cgid = 0x1, cid = 0x27) {
        Params = {
            oob_data_type: OobDataType,
        };
        Completion = CommandComplete;
        Return = GapOobData {
            address_type: u8,
            address: BdAddr,
            oob_data_type: u8,
            oob_data_len: u8,
            oob_data: [u8; 16],
        };
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapSetOobData(cgid = 0x1, cid = 0x28) {
        Params = {
            device_type: OobDeviceType,
            address: BdAddrType,
            oob_data_type: OobDataType,
            oob_data_len: OobDataLength,
            oob_data: [u8; 16],
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(before_fw_1_23_0)]
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light"
))]
stm32wb_hci_macros::vendor_cmd! {
    GapAddDevicesToResolvingList(cgid = 0x1, cid = 0x29) {
        Params<'a> = {
            whitelist_identities: &'a [PeerAddrType] => {
                kind: counted_items,
                count: u8,
                item: PeerAddrType,
                max_items: 36,
            },
            clear_resolving_list: bool,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapRemoveBondedDevice(cgid = 0x1, cid = 0x2A) {
        Params = {
            address: BdAddrType,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapAddDevicesToList(cgid = 0x1, cid = 0x2B) {
        Params<'a> = {
            list_entries: &'a [BdAddrType] => {
                kind: counted_items,
                count: u8,
                item: BdAddrType,
                max_items: 36,
            },
            mode: AddDeviceToListMode,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapAdditionalBeaconStart(cgid = 0x1, cid = 0x30) {
        Params = {
            advertising_interval_min: AdvertisingIntervalBound,
            advertising_interval_max: AdvertisingIntervalBound,
            advertising_channel_map: AdvertisingChannelMap,
            own_address_type: BdAddrType,
            pa_level: PowerAmplifierOutputLevel,
        };
        Constraints = {
            self.advertising_interval_min <= self.advertising_interval_max;
            !self.advertising_channel_map.is_empty();
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapAdditionalBeaconStop(cgid = 0x1, cid = 0x31) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapAdditionalBeaconSetData(cgid = 0x1, cid = 0x32) {
        Params<'a> = {
            advertising_data: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 254,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapAdvSetConfig(cgid = 0x1, cid = 0x40) {
        Params<'a> = {
            adv_mode: AdvertisingMode,
            adv_handle: AdvertisingHandle,
            adv_event_properties: AdvertisingEvent,
            adv_interval: &'a ExtendedAdvertisingInterval,
            primary_adv_channel_map: AdvertisingChannelMap,
            own_addr_type: AddressType,
            peer_addr: BdAddrType,
            adv_filter_policy: AdvertisingFilterPolicy,
            adv_tx_power: i8,
            secondary_adv_max_skip: SecondaryAdvertisingMaximumSkip,
            secondary_adv_phy: AdvertisingPhy,
            adv_sid: AdvertisingSid,
            scan_req_notification_enable: bool,
        };
        Constraints = {
            !self.primary_adv_channel_map.is_empty();
            self.adv_tx_power in [127] || self.adv_tx_power in -127..=20;
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapAdvSetEnable(cgid = 0x1, cid = 0x41) {
        Params<'a> = {
            enable: bool,
            adv_set: &'a [AdvSet] => {
                kind: counted_items,
                count: u8,
                item: AdvSet,
                max_items: 63,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapAdvSetAdvertisingData(cgid = 0x1, cid = 0x42) {
        Params<'a> = {
            adv_handle: AdvertisingHandle,
            operation: AdvertisingOperation,
            fragment_preference: bool,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 251,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapAdvSetScanResponseData(cgid = 0x1, cid = 0x43) {
        Params<'a> = {
            adv_handle: AdvertisingHandle,
            operation: AdvertisingOperation,
            fragment_preference: bool,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8,
                max_len: 251,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapAdvRemoveSet(cgid = 0x1, cid = 0x44) {
        Params = {
            handle: AdvertisingHandle,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapAdvClearSets(cgid = 0x1, cid = 0x45) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapAdvSetRandomAddress(cgid = 0x1, cid = 0x46) {
        Params = {
            handle: AdvertisingHandle,
            address: BdAddr,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_1_21_0)]
#[cfg(any(
    feature = "stack-full-extended",
    feature = "stack-full",
    feature = "stack-light",
))]
stm32wb_hci_macros::vendor_cmd! {
    GapPairingRequestReply(cgid = 0x1, cid = 0x2D) {
        Params = {
            conn_handle: ConnHandle,
            accept: bool,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_1_18_0)]
#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapExtStartScan(cgid = 0x1, cid = 0x50) {
        Params = {
            scan_mode: ExtScanMode,
            procedure: Procedure,
            own_address_type: AddressType,
            filter_duplicates: ExtendedDuplicateFiltering,
            duration: ExtendedScanDuration,
            period: ExtendedScanPeriod,
            scanning_filter_policy: ScanningFilterPolicy,
            scanning_phys: ScanningPhy,
            le_1m_params: ExtScanPhyParams,
            le_coded_params: ExtScanPhyParams,
        };
        Constraints = {
            self.procedure in [
                Procedure::LIMITED_DISCOVERY,
                Procedure::GENERAL_DISCOVERY,
                Procedure::GENERAL_CONNECTION_ESTABLISHMENT,
                Procedure::SELECTIVE_CONNECTION_ESTABLISHMENT,
                Procedure::OBSERVATION,
            ];
            self.scanning_phys in [
                ScanningPhy::LE_1M,
                ScanningPhy::LE_CODED,
                ScanningPhy::LE_1M | ScanningPhy::LE_CODED,
            ];
        };
        Completion = CommandStatus;
    }
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Duplicate filtering behavior for extended scanning.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ExtendedDuplicateFiltering: u8 => 1 {
        Disabled = 0x00,
        Enabled = 0x01,
        /// Enable filtering and reset the duplicate list each scan period.
        EnabledWithPeriodicReset = 0x02,
    }
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Extended-scan duration in 10 ms units; zero scans until stopped.
    ///
    /// CubeWB assigns meaning to every `u16` value, including the zero
    /// sentinel, so construction is intentionally infallible.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct ExtendedScanDuration: u16 => 2;
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Extended-scan period in 1.28 s units; zero disables periodic scanning.
    ///
    /// CubeWB assigns meaning to every `u16` value, including the zero
    /// sentinel, so construction is intentionally infallible.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct ExtendedScanPeriod: u16 => 2;
}

#[cfg(since_fw_1_18_0)]
#[cfg(feature = "stack-full-extended")]
stm32wb_hci_macros::vendor_cmd! {
    GapExtCreateConnection(cgid = 0x1, cid = 0x51) {
        Params = {
            initiating_mode: ExtInitiatingMode,
            procedure: Procedure,
            own_address_type: AddressType,
            peer_address: BdAddrType,
            advertising_handle: InitiatingAdvertisingHandle,
            subevent: InitiatingSubevent,
            initiator_filter_policy: InitiatorFilterPolicy,
            initiating_phys: InitiatingPhy,
            le_1m_params: ExtConnectionPhyParams,
            le_2m_params: ExtConnectionPhyParams,
            le_coded_params: ExtConnectionPhyParams,
        };
        Constraints = {
            self.procedure in [
                Procedure::AUTO_CONNECTION_ESTABLISHMENT,
                Procedure::DIRECT_CONNECTION_ESTABLISHMENT,
            ];
            self.own_address_type in [
                AddressType::Public,
                AddressType::Random,
                AddressType::ResolvablePrivate,
            ];
            self.initiating_phys in [
                InitiatingPhy::LE_1M,
                InitiatingPhy::LE_2M,
                InitiatingPhy::LE_CODED,
                InitiatingPhy::LE_1M | InitiatingPhy::LE_2M,
                InitiatingPhy::LE_1M | InitiatingPhy::LE_CODED,
                InitiatingPhy::LE_2M | InitiatingPhy::LE_CODED,
                InitiatingPhy::LE_1M | InitiatingPhy::LE_2M | InitiatingPhy::LE_CODED,
            ];
        };
        Completion = CommandStatus;
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Device whose GAP out-of-band data is being supplied.
    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum OobDeviceType: u8 => 1 {
        Local = 0x00,
        Remote = 0x01,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Reserved mode field used by the extended GAP scan command.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ExtScanMode: u8 => 1 {
        /// Reserved value required by STM32CubeWB.
        Default = 0x00,
    }
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// PHYs on which an extended scan is performed.
    pub struct ScanningPhy: u8 => 1 {
        /// Scan on the LE 1M PHY.
        const LE_1M = 0x01;
        /// Scan on the LE Coded PHY.
        const LE_CODED = 0x04;
    }
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Reserved mode field used by the extended GAP connection command.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ExtInitiatingMode: u8 => 1 {
        /// Reserved value required by STM32CubeWB.
        Default = 0x00,
    }
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
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

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// PHY-specific records supplied while initiating a connection.
    pub struct InitiatingPhy: u8 => 1 {
        /// Supply parameters for the LE 1M PHY.
        const LE_1M = 0x01;
        /// Supply parameters for the LE 2M PHY.
        const LE_2M = 0x02;
        /// Supply parameters for the LE Coded PHY.
        const LE_CODED = 0x04;
    }
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Periodic-advertising handle used while initiating a connection.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct InitiatingAdvertisingHandle: u8 => 1 {
        minimum: 0x00,
        maximum: 0xEF,
        sentinel: UNUSED = 0xFF,
    }
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Periodic-advertising subevent used while initiating a connection.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct InitiatingSubevent: u8 => 1 {
        minimum: 0x00,
        maximum: 0x7F,
        sentinel: UNUSED = 0xFF,
    }
}

/// One of the two fixed extended-scan PHY parameter records.
#[cfg(since_fw_1_18_0)]
pub struct ExtScanPhyParams {
    /// Passive or active scanning for this PHY.
    pub scan_type: ScanType,
    /// Validated scan interval and window for this PHY.
    pub scan_window: ScanWindow,
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    ExtScanPhyParams => 5 {
        Fields = {
            scan_type: ScanType,
            scan_window: ScanWindow,
        };
        Encode = |value| {
            (value.scan_type, value.scan_window)
        };
    }
}

/// One of the three fixed extended-connection PHY parameter records.
#[cfg(since_fw_1_18_0)]
pub struct ExtConnectionPhyParams {
    /// Validated scan interval and window for this PHY.
    pub scan_window: ScanWindow,
    /// Validated connection interval, latency, and supervision timeout.
    pub connection_interval: ConnectionInterval,
    /// Validated expected connection-event length range.
    pub expected_connection_length: ExpectedConnectionLength,
}

#[cfg(since_fw_1_18_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    composite
    ExtConnectionPhyParams => 16 {
        Fields = {
            scan_window: ScanWindow,
            connection_interval: ConnectionInterval,
            expected_connection_length: ExpectedConnectionLength,
        };
        Encode = |value| {
            (
                value.scan_window,
                value.connection_interval,
                value.expected_connection_length,
            )
        };
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
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
