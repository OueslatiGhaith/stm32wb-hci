//! The Bluetooth Core commands of the catalog, sent with bt-hci's own types.
//!
//! Each bt-hci type listed here, by its path in `bt_hci::cmd`, is checked at compile time against the
//! catalog on every target implementing the command: its opcode, whether it
//! completes with Command Complete (`SyncCmd`) or Command Status
//! (`AsyncCmd`), and its parameter and return widths wherever the catalog's
//! are fixed. It implements [`Supported`](crate::catalog::Supported) on
//! those targets only.
//!
//! bt-hci 0.10 declares some of the catalog's commands differently, and
//! lacks others; they are not listed:
//!
//! - `Disconnect` waits for Command Complete, but the binaries answer
//!   `hci_disconnect` with Command Status;
//! - `LeTransmitterTest` has opcode 0x201C, `hci_le_read_supported_states`,
//!   rather than 0x201E, and `LeTransmitterTestV3` has 0x205E,
//!   `hci_le_generate_dhkey_v2`;
//! - `LeSetExtScanParams` and `LeExtCreateConn` write one parameter set per
//!   PHY selected, where the binaries' wrappers always write two;
//! - nothing sends `hci_le_read_local_p256_public_key`,
//!   `hci_le_generate_dhkey`, `hci_le_read_peer_resolvable_address`,
//!   `hci_le_read_local_resolvable_address`, or
//!   `hci_le_set_resolvable_private_address_timeout_v2`.

stm32wb_hci_macros::standard_commands! {
    hci_read_remote_version_information => link_control::ReadRemoteVersionInformation,
    hci_set_event_mask => controller_baseband::SetEventMask,
    hci_reset => controller_baseband::Reset,
    hci_read_transmit_power_level => controller_baseband::ReadTransmitPowerLevel,
    hci_set_controller_to_host_flow_control => controller_baseband::SetControllerToHostFlowControl,
    hci_host_buffer_size => controller_baseband::HostBufferSize,
    hci_host_number_of_completed_packets => controller_baseband::HostNumberOfCompletedPackets<'a>,
    hci_read_local_version_information => info::ReadLocalVersionInformation,
    hci_read_local_supported_commands => info::ReadLocalSupportedCmds,
    hci_read_local_supported_features => info::ReadLocalSupportedFeatures,
    hci_read_bd_addr => info::ReadBdAddr,
    hci_read_rssi => status::ReadRssi,
    hci_le_set_event_mask => le::LeSetEventMask,
    hci_le_read_buffer_size => le::LeReadBufferSize,
    hci_le_read_local_supported_features_page_0 => le::LeReadLocalSupportedFeatures,
    hci_le_set_random_address => le::LeSetRandomAddr,
    hci_le_set_advertising_parameters => le::LeSetAdvParams,
    hci_le_read_advertising_physical_channel_tx_power => le::LeReadAdvPhysicalChannelTxPower,
    hci_le_set_advertising_data => le::LeSetAdvData,
    hci_le_set_scan_response_data => le::LeSetScanResponseData,
    hci_le_set_advertising_enable => le::LeSetAdvEnable,
    hci_le_set_scan_parameters => le::LeSetScanParams,
    hci_le_set_scan_enable => le::LeSetScanEnable,
    hci_le_create_connection => le::LeCreateConn,
    hci_le_create_connection_cancel => le::LeCreateConnCancel,
    hci_le_read_filter_accept_list_size => le::LeReadFilterAcceptListSize,
    hci_le_clear_filter_accept_list => le::LeClearFilterAcceptList,
    hci_le_add_device_to_filter_accept_list => le::LeAddDeviceToFilterAcceptList,
    hci_le_remove_device_from_filter_accept_list => le::LeRemoveDeviceFromFilterAcceptList,
    hci_le_connection_update => le::LeConnUpdate,
    hci_le_set_host_channel_classification => le::LeSetHostChannelClassification,
    hci_le_read_channel_map => le::LeReadChannelMap,
    hci_le_read_remote_features_page_0 => le::LeReadRemoteFeatures,
    hci_le_encrypt => le::LeEncrypt,
    hci_le_rand => le::LeRand,
    hci_le_enable_encryption => le::LeEnableEncryption,
    hci_le_long_term_key_request_reply => le::LeLongTermKeyRequestReply,
    hci_le_long_term_key_request_negative_reply => le::LeLongTermKeyRequestNegativeReply,
    hci_le_read_supported_states => le::LeReadSupportedStates,
    hci_le_receiver_test => le::LeReceiverTest,
    hci_le_test_end => le::LeTestEnd,
    hci_le_set_data_length => le::LeSetDataLength,
    hci_le_read_suggested_default_data_length => le::LeReadSuggestedDefaultDataLength,
    hci_le_write_suggested_default_data_length => le::LeWriteSuggestedDefaultDataLength,
    hci_le_add_device_to_resolving_list => le::LeAddDeviceToResolvingList,
    hci_le_remove_device_from_resolving_list => le::LeRemoveDeviceFromResolvingList,
    hci_le_clear_resolving_list => le::LeClearResolvingList,
    hci_le_read_resolving_list_size => le::LeReadResolvingListSize,
    hci_le_set_address_resolution_enable => le::LeSetAddrResolutionEnable,
    hci_le_set_resolvable_private_address_timeout => le::LeSetResolvablePrivateAddrTimeout,
    hci_le_read_maximum_data_length => le::LeReadMaxDataLength,
    hci_le_read_phy => le::LeReadPhy,
    hci_le_set_default_phy => le::LeSetDefaultPhy,
    hci_le_set_phy => le::LeSetPhy,
    hci_le_receiver_test_v2 => le::LeReceiverTestV2,
    hci_le_transmitter_test_v2 => le::LeTransmitterTestV2,
    hci_le_set_advertising_set_random_address => le::LeSetAdvSetRandomAddr,
    hci_le_set_extended_advertising_parameters => le::LeSetExtAdvParams,
    hci_le_set_extended_advertising_data => le::LeSetExtAdvData<'a>,
    hci_le_set_extended_scan_response_data => le::LeSetExtScanResponseData<'a>,
    hci_le_set_extended_advertising_enable => le::LeSetExtAdvEnable<'a>,
    hci_le_read_maximum_advertising_data_length => le::LeReadMaxAdvDataLength,
    hci_le_read_number_of_supported_advertising_sets => le::LeReadNumberOfSupportedAdvSets,
    hci_le_remove_advertising_set => le::LeRemoveAdvSet,
    hci_le_clear_advertising_sets => le::LeClearAdvSets,
    hci_le_set_extended_scan_enable => le::LeSetExtScanEnable,
    hci_le_read_transmit_power => le::LeReadTransmitPower,
    hci_le_read_rf_path_compensation => le::LeReadRfPathCompensation,
    hci_le_write_rf_path_compensation => le::LeWriteRfPathCompensation,
    hci_le_set_privacy_mode => le::LeSetPrivacyMode,
}

#[cfg(test)]
mod tests {
    use bt_hci::cmd::{controller_baseband, le};

    use crate::catalog::Supported;

    fn supported<T: Supported>() {}

    #[test]
    fn listed_and_vendor_commands_are_supported() {
        supported::<controller_baseband::Reset>();
        supported::<le::LeSetEventMask>();
        supported::<crate::aci::hal::HalSetRadioActivityMask>();
    }

    /// Extended advertising exists in the extended profiles only.
    #[cfg(any(feature = "stack-full-extended", feature = "stack-hci-layer-extended"))]
    #[test]
    fn extended_profiles_support_extended_advertising() {
        supported::<le::LeSetExtAdvData<'static>>();
        supported::<le::LeSetExtAdvEnable<'static>>();
    }
}
