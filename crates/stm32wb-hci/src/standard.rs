//! The Bluetooth Core commands and events of the catalog, sent and decoded
//! with bt-hci's own types where bt-hci agrees with the catalog.
//!
//! Each bt-hci type listed here, by its path in `bt_hci::cmd`, is checked at compile time against the
//! catalog on every target implementing the command: its opcode, whether it
//! completes with Command Complete (`SyncCmd`) or Command Status
//! (`AsyncCmd`), and its parameter and return widths wherever the catalog's
//! are fixed. It implements [`Supported`](crate::catalog::Supported) on
//! those targets only.
//!
//! bt-hci's `LeSetExtScanParams`, `LeExtCreateConn`, and `LeExtCreateConnV2`
//! write one parameter set per PHY selected, where the wrappers always write
//! two and three, the catalogs' layout, so this module declares them with
//! [`standard_command!`](stm32wb_hci_macros::standard_command), checked like
//! the vendor commands. Their names follow bt-hci's, so importing both
//! modules' command of one name needs a rename.
//!
//! Every Core event is bt-hci's. Its event code, or LE meta subevent code, is
//! checked at compile time on every target emitting it, and a generated test
//! checks that bt-hci decodes exactly the catalog's width where that width is
//! fixed. The events carrying lists are tested here by hand. The transport
//! consumes Command Complete and Command Status, and ST's C decodes the LE
//! advertising report procedurally, so the catalog has no layout for them to
//! check.

use stm32wb_hci_macros::{standard_command, vendor_struct};

vendor_struct! {
    /// Scan parameters for one PHY.
    Scan_Param_Phy_t => ScanParamPhy {
        scan_type: crate::aci::values::ScanType,
        scan_interval: crate::aci::durations::ExtScanInterval,
        scan_window: crate::aci::durations::ExtScanWindow,
    }
}

standard_command! {
    /// Set the extended scan parameters. The wrapper always writes two PHY
    /// parameter sets, whatever `scanning_phys` selects.
    hci_le_set_extended_scan_parameters => LeSetExtScanParams {
        own_address_type: crate::aci::values::HciOwnAddressType,
        scanning_filter_policy: crate::aci::values::ScanningFilterPolicy,
        scanning_phys: crate::aci::flags::ScanningPhys,
        scan_param_phy: [ScanParamPhy; 2],
    }
}

vendor_struct! {
    /// Connection parameters for one PHY.
    Init_Param_Phy_t => InitParamPhy {
        scan_interval: crate::aci::durations::ExtScanInterval,
        scan_window: crate::aci::durations::ExtScanWindow,
        conn_interval_min: crate::aci::durations::ConnInterval,
        conn_interval_max: crate::aci::durations::ConnInterval,
        conn_latency: crate::aci::ranges::ConnLatency,
        supervision_timeout: crate::aci::durations::SupervisionTimeout,
        min_ce_length: crate::aci::durations::CeLength,
        max_ce_length: crate::aci::durations::CeLength,
    }
}

standard_command! {
    /// Create a connection with extended advertising. The wrapper always
    /// writes three PHY parameter sets, whatever `initiating_phys` selects.
    hci_le_extended_create_connection => LeExtCreateConn {
        initiator_filter_policy: crate::aci::values::InitiatorFilterPolicy,
        own_address_type: crate::aci::values::HciOwnAddressType,
        peer_address_type: crate::aci::values::AddressType,
        peer_address: bt_hci::param::BdAddr,
        initiating_phys: crate::aci::flags::InitiatingPhys,
        init_param_phy: [InitParamPhy; 3],
    }
}

standard_command! {
    /// Create a connection, possibly from a subevent of periodic advertising
    /// with responses. The wrapper always writes three PHY parameter sets,
    /// whatever `initiating_phys` selects.
    hci_le_extended_create_connection_v2 => LeExtCreateConnV2 {
        advertising_handle: u8,
        subevent: u8,
        initiator_filter_policy: u8,
        own_address_type: u8,
        peer_address_type: u8,
        peer_address: bt_hci::param::BdAddr,
        initiating_phys: u8,
        init_param_phy: [InitParamPhy; 3],
    }
}

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
    hci_set_event_mask_page_2 => controller_baseband::SetEventMaskPage2,
    hci_read_authenticated_payload_timeout => controller_baseband::ReadAuthenticatedPayloadTimeout,
    hci_write_authenticated_payload_timeout => controller_baseband::WriteAuthenticatedPayloadTimeout,
    hci_le_set_periodic_advertising_parameters => le::LeSetPeriodicAdvParams,
    hci_le_set_periodic_advertising_data => le::LeSetPeriodicAdvData<'a>,
    hci_le_set_periodic_advertising_enable => le::LeSetPeriodicAdvEnable,
    hci_le_periodic_advertising_create_sync => le::LePeriodicAdvCreateSync,
    hci_le_periodic_advertising_create_sync_cancel => le::LePeriodicAdvCreateSyncCancel,
    hci_le_periodic_advertising_terminate_sync => le::LePeriodicAdvTerminateSync,
    hci_le_add_device_to_periodic_advertiser_list => le::LeAddDeviceToPeriodicAdvList,
    hci_le_remove_device_from_periodic_advertiser_list => le::LeRemoveDeviceFromPeriodicAdvList,
    hci_le_clear_periodic_advertiser_list => le::LeClearPeriodicAdvList,
    hci_le_read_periodic_advertiser_list_size => le::LeReadPeriodicAdvListSize,
    hci_le_set_connectionless_cte_transmit_parameters => le::LeSetConnectionlessCteTransmitParams<'a>,
    hci_le_set_connectionless_cte_transmit_enable => le::LeSetConnectionlessCteTransmitEnable,
    hci_le_set_connection_cte_transmit_parameters => le::LeSetConnCteTransmitParams<'a>,
    hci_le_connection_cte_response_enable => le::LeConnCteResponseEnable,
    hci_le_read_antenna_information => le::LeReadAntennaInformation,
    hci_le_set_periodic_advertising_receive_enable => le::LeSetPeriodicAdvReceiveEnable,
    hci_le_periodic_advertising_sync_transfer => le::LePeriodicAdvSyncTransfer,
    hci_le_periodic_advertising_set_info_transfer => le::LePeriodicAdvSetInfoTransfer,
    hci_le_set_periodic_advertising_sync_transfer_parameters => le::LeSetPeriodicAdvSyncTransferParams,
    hci_le_set_default_periodic_advertising_sync_transfer_parameters => le::LeSetDefaultPeriodicAdvSyncTransferParams,
    hci_le_read_iso_tx_sync => le::LeReadIsoTxSync,
    hci_le_set_cig_parameters => le::LeSetCigParameters<'a>,
    hci_le_set_cig_parameters_test => le::LeSetCigParametersTest<'a>,
    hci_le_create_cis => le::LeCreateCis<'a>,
    hci_le_remove_cig => le::LeRemoveCig,
    hci_le_accept_cis_request => le::LeAcceptCisRequest,
    hci_le_reject_cis_request => le::LeRejectCisRequest,
    hci_le_create_big => le::LeCreateBig,
    hci_le_terminate_big => le::LeTerminateBig,
    hci_le_request_peer_sca => le::LeRequestPeerSca,
    hci_le_setup_iso_data_path => le::LeSetupIsoDataPath<'a>,
    hci_le_remove_iso_data_path => le::LeRemoveIsoDataPath,
    hci_le_iso_transmit_test => le::LeIsoTransmitTest,
    hci_le_iso_receive_test => le::LeIsoReceiveTest,
    hci_le_iso_read_test_counters => le::LeIsoReadTestCounters,
    hci_le_iso_test_end => le::LeIsoTestEnd,
    hci_le_set_host_feature => le::LeSetHostFeature,
    hci_le_read_iso_link_quality => le::LeReadIsoLinkQuality,
    hci_le_enhanced_read_transmit_power_level => le::LeEnhancedReadTransmitPowerLevel,
    hci_le_read_remote_transmit_power_level => le::LeReadRemoteTransmitPowerLevel,
    hci_le_set_path_loss_reporting_parameters => le::LeSetPathLossReportingParams,
    hci_le_set_path_loss_reporting_enable => le::LeSetPathLossReportingEnable,
    hci_le_set_transmit_power_reporting_enable => le::LeSetTransmitPowerReportingEnable,
    hci_le_set_data_related_address_changes => le::LeSetDataRelatedAddrChanges,
    hci_le_set_default_subrate => le::LeSetDefaultSubrate,
    hci_le_subrate_request => le::LeSubrateRequest,
    hci_le_set_periodic_advertising_response_data => le::LeSetPeriodicAdvResponseData<'a>,
    hci_le_set_periodic_advertising_subevent_data => le::LeSetPeriodicAdvSubeventData<'a>,
    hci_le_set_periodic_sync_subevent => le::LeSetPeriodicSyncSubevent<'a>,
    hci_le_set_periodic_advertising_parameters_v2 => le::LeSetPeriodicAdvParamsV2,
    hci_disconnect => link_control::Disconnect,
    hci_le_transmitter_test => le::LeTransmitterTest,
    hci_le_read_local_p256_public_key => le::LeReadLocalP256PublicKey,
    hci_le_generate_dhkey => le::LeGenerateDhkey,
    hci_le_read_peer_resolvable_address => le::LeReadPeerResolvableAddr,
    hci_le_read_local_resolvable_address => le::LeReadLocalResolvableAddr,
    hci_le_generate_dhkey_v2 => le::LeGenerateDhkeyV2,
    hci_le_set_resolvable_private_address_timeout_v2 => le::LeSetResolvablePrivateAddrTimeoutV2,
    hci_configure_data_path => controller_baseband::ConfigureDataPath<'a>,
    hci_le_add_device_to_monitored_advertisers_list => le::LeAddDeviceToMonitoredAdvList,
    hci_le_big_create_sync => le::LeBigCreateSync<'a>,
    hci_le_big_terminate_sync => le::LeBigTerminateSync,
    hci_le_clear_monitored_advertisers_list => le::LeClearMonitoredAdvList,
    hci_le_connection_cte_request_enable => le::LeConnCteRequestEnable,
    hci_le_create_big_test => le::LeCreateBigTest,
    hci_le_cs_create_config => le::LeCsCreateConfig,
    hci_le_cs_procedure_enable => le::LeCsProcedureEnable,
    hci_le_cs_read_local_supported_capabilities => le::LeCsReadLocalSupportedCapabilities,
    hci_le_cs_read_remote_fae_table => le::LeCsReadRemoteFaeTable,
    hci_le_cs_read_remote_supported_capabilities => le::LeCsReadRemoteSupportedCapabilities,
    hci_le_cs_remove_config => le::LeCsRemoveConfig,
    hci_le_cs_security_enable => le::LeCsSecurityEnableCommand,
    hci_le_cs_set_channel_classification => le::LeCsSetChannelClassification,
    hci_le_cs_set_default_settings => le::LeCsSetDefaultSettings,
    hci_le_cs_set_procedure_parameters => le::LeCsSetProcedureParams,
    hci_le_cs_test => le::LeCsTest<'a>,
    hci_le_cs_test_end => le::LeCsTestEnd,
    hci_le_cs_write_cached_remote_fae_table => le::LeCsWriteCachedRemoteFaeTable,
    hci_le_cs_write_cached_remote_supported_capabilities => le::LeCsWriteCachedRemoteSupportedCapabilities,
    hci_le_enable_monitoring_advertisers => le::LeEnableMonitoringAdv,
    hci_le_frame_space_update => le::LeFrameSpaceUpdate,
    hci_le_read_all_local_supported_features => le::LeReadAllLocalSupportedFeatures,
    hci_le_read_all_remote_features => le::LeReadAllRemoteFeatures,
    hci_le_read_buffer_size_v2 => le::LeReadBufferSizeV2,
    hci_le_read_monitored_advertisers_list_size => le::LeReadMonitoredAdvListSize,
    hci_le_receiver_test_v3 => le::LeReceiverTestV3<'a>,
    hci_le_remote_connection_parameter_request_negative_reply => le::LeRemoteConnectionParameterRequestNegativeReply,
    hci_le_remote_connection_parameter_request_reply => le::LeRemoteConnectionParameterRequestReply,
    hci_le_remove_device_from_monitored_advertisers_list => le::LeRemoveDeviceFromMonitoredAdvList,
    hci_le_set_connection_cte_receive_parameters => le::LeSetConnCteReceiveParams<'a>,
    hci_le_set_connectionless_iq_sampling_enable => le::LeSetConnectionlessIqSamplingEnable<'a>,
    hci_le_set_extended_advertising_parameters_v2 => le::LeSetExtAdvParamsV2,
    hci_le_transmitter_test_v3 => le::LeTransmitterTestV3<'a>,
    hci_le_transmitter_test_v4 => le::LeTransmitterTestV4<'a>,
    hci_read_afh_channel_assessment_mode => controller_baseband::ReadAfhChannelAssessmentMode,
    hci_read_connection_accept_timeout => controller_baseband::ReadConnectionAcceptTimeout,
    hci_read_local_supported_controller_delay => info::ReadLocalSupportedControllerDelay<'a>,
    hci_set_ecosystem_base_interval => controller_baseband::SetEcosystemBaseInterval,
    hci_write_afh_channel_assessment_mode => controller_baseband::WriteAfhChannelAssessmentMode,
    hci_write_connection_accept_timeout => controller_baseband::WriteConnectionAceeptTimeout,
}

stm32wb_hci_macros::standard_events! {
    hci_disconnection_complete_event => DisconnectionComplete,
    hci_encryption_change_event => EncryptionChangeV1,
    hci_read_remote_version_information_complete_event => ReadRemoteVersionInformationComplete,
    hci_command_complete_event => CommandComplete<'a>,
    hci_command_status_event => CommandStatus,
    hci_hardware_error_event => HardwareError,
    hci_number_of_completed_packets_event => NumberOfCompletedPackets<'a>,
    hci_encryption_key_refresh_complete_event => EncryptionKeyRefreshComplete,
    hci_le_connection_complete_event => le::LeConnectionComplete,
    hci_le_advertising_report_event => le::LeAdvertisingReport<'a>,
    hci_le_connection_update_complete_event => le::LeConnectionUpdateComplete,
    hci_le_read_remote_features_complete_event => le::LeReadRemoteFeaturesComplete,
    hci_le_long_term_key_request_event => le::LeLongTermKeyRequest,
    hci_le_data_length_change_event => le::LeDataLengthChange,
    hci_le_read_local_p256_public_key_complete_event => le::LeReadLocalP256PublicKeyComplete,
    hci_le_generate_dhkey_complete_event => le::LeGenerateDhkeyComplete,
    hci_le_enhanced_connection_complete_event => le::LeEnhancedConnectionComplete,
    hci_le_directed_advertising_report_event => le::LeDirectedAdvertisingReport<'a>,
    hci_le_phy_update_complete_event => le::LePhyUpdateComplete,
    hci_le_extended_advertising_report_event => le::LeExtendedAdvertisingReport<'a>,
    hci_le_scan_timeout_event => le::LeScanTimeout,
    hci_le_advertising_set_terminated_event => le::LeAdvertisingSetTerminated,
    hci_le_scan_request_received_event => le::LeScanRequestReceived,
    hci_le_channel_selection_algorithm_event => le::LeChannelSelectionAlgorithm,
    hci_data_buffer_overflow_event => DataBufferOverflow,
    hci_authenticated_payload_timeout_expired_event => AuthenticatedPayloadTimeoutExpired,
    hci_le_remote_connection_parameter_request_event => le::LeRemoteConnectionParameterRequest,
    hci_le_periodic_advertising_sync_established_event => le::LePeriodicAdvertisingSyncEstablished,
    hci_le_periodic_advertising_report_event => le::LePeriodicAdvertisingReport<'a>,
    hci_le_periodic_advertising_sync_lost_event => le::LePeriodicAdvertisingSyncLost,
    hci_le_connectionless_iq_report_event => le::LeConnectionlessIqReport<'a>,
    hci_le_connection_iq_report_event => le::LeConnectionIqReport<'a>,
    hci_le_cte_request_failed_event => le::LeCteRequestFailed,
    hci_le_periodic_advertising_sync_transfer_received_event => le::LePeriodicAdvertisingSyncTransferReceived,
    hci_le_cis_established_event => le::LeCisEstablished,
    hci_le_cis_request_event => le::LeCisRequest,
    hci_le_request_peer_sca_complete_event => le::LeRequestPeerScaComplete,
    hci_le_path_loss_threshold_event => le::LePathLossThreshold,
    hci_le_transmit_power_reporting_event => le::LeTransmitPowerReporting,
    hci_le_biginfo_advertising_report_event => le::LeBiginfoAdvertisingReport,
    hci_le_subrate_change_event => le::LeSubrateChange,
    hci_le_periodic_advertising_response_report_event => le::LePeriodicAdvertisingResponseReport<'a>,
    hci_le_cs_subevent_result_event => le::LeCsSubeventResult<'a>,
    hci_le_cs_subevent_result_continue_event => le::LeCsSubeventResultContinue<'a>,
    hci_le_frame_space_update_complete_event => le::LeFrameSpaceUpdateComplete,
    hci_le_create_big_complete_event => le::LeCreateBigComplete<'a>,
    hci_le_terminate_big_complete_event => le::LeTerminateBigComplete,
    hci_le_big_sync_established_event => le::LeBigSyncEstablished<'a>,
    hci_le_big_sync_lost_event => le::LeBigSyncLost,
    hci_le_cis_established_v2_event => le::LeCisEstablishedV2,
    hci_le_cs_config_complete_event => le::LeCsConfigComplete,
    hci_le_cs_procedure_enable_complete_event => le::LeCsProcedureEnableComplete,
    hci_le_cs_read_remote_fae_table_complete_event => le::LeCsReadRemoteFaeTableComplete<'a>,
    hci_le_cs_read_remote_supported_capabilities_complete_event => le::LeCsReadRemoteSupportedCapabilitiesComplete,
    hci_le_cs_security_enable_complete_event => le::LeCsSecurityEnableComplete,
    hci_le_cs_test_end_complete_event => le::LeCsTestEndComplete,
    hci_le_enhanced_connection_complete_v2_event => le::LeEnhancedConnectionCompleteV2,
    hci_le_monitored_advertisers_report_event => le::LeMonitoredAdvertisersReport,
    hci_le_periodic_advertising_report_v2_event => le::LePeriodicAdvertisingReportV2<'a>,
    hci_le_periodic_advertising_subevent_data_request_event => le::LePeriodicAdvertisingSubeventDataRequest,
    hci_le_periodic_advertising_sync_established_v2_event => le::LePeriodicAdvertisingSyncEstablishedV2,
    hci_le_periodic_advertising_sync_transfer_received_v2_event => le::LePeriodicAdvertisingSyncTransferReceivedV2,
    hci_le_read_all_remote_features_complete_event => le::LeReadAllRemoteFeaturesComplete<'a>,
}

#[cfg(test)]
mod tests {
    use bt_hci::cmd::{Cmd, controller_baseband, le};

    use crate::catalog::Supported;

    fn supported<T: Supported>() {}

    /// The BIS handles of a BIG follow its one-byte handle and parameters,
    /// counted by Num_BIS, whether ST declares them as integers or as
    /// `Connection_Handle_t`.
    #[cfg(any(feature = "stack-wba-full", feature = "stack-wba-link-layer-only"))]
    #[test]
    fn big_events_list_their_bis_handles() {
        use bt_hci::FromHciBytes;
        use bt_hci::event::le::{LeBigSyncLost, LeCreateBigComplete};
        use bt_hci::param::{BigHandle, Status};

        let params = [
            0x00, 0x05, 1, 0, 0, 2, 0, 0, 0x01, 2, 1, 0, 1, 0xFB, 0, 0x10,
            0, // up to ISO_Interval
            2, 0x20, 0x00, 0x21, 0x00, // Num_BIS, then the handles
        ];
        let event = LeCreateBigComplete::from_hci_bytes_complete(&params).unwrap();
        assert_eq!(event.big_handle, BigHandle(0x05));
        assert_eq!(event.iso_interval.as_u16(), 0x0010);
        assert_eq!(event.bis_handles.len(), 2);
        assert_eq!(event.bis_handles[1].handle().unwrap().raw(), 0x0021);
        assert!(LeCreateBigComplete::from_hci_bytes_complete(&params[..params.len() - 1]).is_err());

        let event = LeBigSyncLost::from_hci_bytes_complete(&[0x05, 0x13]).unwrap();
        assert_eq!(
            (event.big_handle, event.reason),
            (BigHandle(0x05), Status::new(0x13))
        );
    }

    /// Each Number Of Completed Packets entry is a handle and a count.
    #[cfg(not(feature = "stack-hci-adv-scan"))]
    #[test]
    fn completed_packets_list_handle_and_count_pairs() {
        use bt_hci::FromHciBytes;
        use bt_hci::event::NumberOfCompletedPackets;

        let event = NumberOfCompletedPackets::from_hci_bytes_complete(&[2, 1, 8, 3, 0, 2, 8, 1, 0])
            .unwrap();
        assert_eq!(event.completed_packets.len(), 2);
        assert_eq!(event.completed_packets[1].handle().unwrap().raw(), 0x0802);
        assert_eq!(
            event.completed_packets[1].num_completed_packets().unwrap(),
            1
        );
    }

    /// Each directed advertising report is 16 bytes, as
    /// `Direct_Advertising_Report_t`.
    #[cfg(not(feature = "stack-hci-adv-scan"))]
    #[test]
    fn directed_reports_are_16_bytes_each() {
        use bt_hci::FromHciBytes;
        use bt_hci::event::le::LeDirectedAdvertisingReport;

        let mut params = [1; 33];
        params[0] = 2;
        let report = LeDirectedAdvertisingReport::from_hci_bytes_complete(&params).unwrap();
        assert_eq!(report.reports.len(), 2);
        assert!(LeDirectedAdvertisingReport::from_hci_bytes_complete(&params[..32]).is_err());
    }

    /// ST's C reads each legacy advertising report whole, one after the
    /// other: 9 bytes up to the data length, the data, then the RSSI. The
    /// catalog leaves the layout unresolved, since its C has no structure
    /// for it.
    #[test]
    fn advertising_reports_follow_each_other() {
        use bt_hci::FromHciBytes;
        use bt_hci::event::le::LeAdvertisingReport;
        use bt_hci::param::BdAddr;

        let params = [
            2, // Num_Reports
            0x00, 0x00, 1, 2, 3, 4, 5, 6, 3, 0x02, 0x01, 0x06, 0xC4, // ADV_IND, public
            0x04, 0x01, 7, 8, 9, 10, 11, 12, 0, 0xB0, // SCAN_RSP, random, no data
        ];
        let event = LeAdvertisingReport::from_hci_bytes_complete(&params).unwrap();
        let mut reports = event.reports.iter();
        assert_eq!(reports.len(), 2);
        let first = reports.next().unwrap().unwrap();
        assert_eq!(first.addr, BdAddr::new([1, 2, 3, 4, 5, 6]));
        assert_eq!(first.data, [0x02, 0x01, 0x06]);
        assert_eq!(first.rssi, -60);
        let second = reports.next().unwrap().unwrap();
        assert_eq!(second.addr, BdAddr::new([7, 8, 9, 10, 11, 12]));
        assert!(second.data.is_empty());
        assert_eq!(second.rssi, -80);
        assert!(reports.next().is_none());
        assert!(
            LeAdvertisingReport::from_hci_bytes_complete(&params[..params.len() - 1])
                .unwrap()
                .reports
                .iter()
                .nth(1)
                .unwrap()
                .is_err(),
            "the second report lacks its RSSI"
        );
    }

    /// ST's layout is one extended advertising report with its data.
    #[cfg(any(feature = "stack-full-extended", feature = "stack-hci-layer-extended"))]
    #[test]
    fn extended_reports_follow_the_catalog_layout() {
        use bt_hci::FromHciBytes;
        use bt_hci::event::le::LeExtendedAdvertisingReport;

        // Num_Reports, 23 bytes of report up to Data_Length, then the data.
        let mut params = [0u8; 28];
        params[0] = 1;
        params[10] = 1; // Primary_PHY: LE 1M
        params[24] = 3; // Data_Length
        params[25..].copy_from_slice(&[0xAA, 0xBB, 0xCC]);
        let event = LeExtendedAdvertisingReport::from_hci_bytes_complete(&params).unwrap();
        let report = event.reports.iter().next().unwrap().unwrap();
        assert_eq!(report.data, [0xAA, 0xBB, 0xCC]);
    }

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

    #[test]
    fn transmitter_test_uses_the_catalog_opcode() {
        assert_eq!(le::LeTransmitterTest::OPCODE.to_raw(), 0x201E);
        supported::<le::LeTransmitterTest>();
    }

    /// The commands the advertising and scanning profile lacks.
    #[cfg(not(feature = "stack-hci-adv-scan"))]
    mod connected {
        use bt_hci::cmd::{AsyncCmd, Cmd, SyncCmd, le};
        use bt_hci::param::{AddrKind, BdAddr};
        use bt_hci::{FromHciBytes, WriteHci};

        use super::supported;

        fn encode(command: &impl WriteHci) -> ([u8; 128], usize) {
            let mut buffer = [0; 128];
            let mut writer = &mut buffer[..];
            command.write_hci(&mut writer).unwrap();
            let len = 128 - writer.len();
            (buffer, len)
        }

        fn command_status<T: AsyncCmd>() {}

        /// The full and light profiles terminate connections with
        /// `aci_gap_terminate` from 1.21.0.
        #[cfg(any(
            feature = "stack-full-extended",
            feature = "stack-hci-layer-extended",
            feature = "stack-hci-layer"
        ))]
        #[test]
        fn disconnect_completes_with_command_status() {
            use bt_hci::cmd::link_control::Disconnect;
            use bt_hci::param::{ConnHandle, DisconnectReason};

            command_status::<Disconnect>();
            supported::<Disconnect>();
            let (bytes, len) = encode(&Disconnect::new(
                ConnHandle::new(0x0801),
                DisconnectReason::RemoteUserTerminatedConn,
            ));
            assert_eq!(bytes[..len], [0x06, 0x04, 3, 0x01, 0x08, 0x13]);
        }

        /// Generating a DHKey reaches the full and light profiles in 1.21.0.
        #[cfg(any(
            feature = "stack-full-extended",
            feature = "stack-hci-layer-extended",
            feature = "stack-hci-layer"
        ))]
        #[test]
        fn dhkeys_are_generated_from_the_peer_key() {
            let (bytes, len) = encode(&le::LeGenerateDhkey::new([7; 64]));
            assert_eq!(bytes[..3], [0x26, 0x20, 64]);
            assert_eq!(len, 67);
        }

        #[test]
        fn key_and_address_commands_use_the_catalog_opcodes() {
            use le::{LeReadLocalP256PublicKey, LeReadPeerResolvableAddr};

            assert_eq!(LeReadLocalP256PublicKey::OPCODE.to_raw(), 0x2025);
            supported::<LeReadLocalP256PublicKey>();
            command_status::<LeReadLocalP256PublicKey>();

            let address = BdAddr::new([1, 2, 3, 4, 5, 6]);
            let (bytes, len) = encode(&LeReadPeerResolvableAddr::new(AddrKind::PUBLIC, address));
            assert_eq!(bytes[..len], [0x2B, 0x20, 7, 0, 1, 2, 3, 4, 5, 6]);
            let returned =
                <LeReadPeerResolvableAddr as SyncCmd>::Return::from_hci_bytes_complete(&[9; 6])
                    .unwrap();
            assert_eq!(returned, BdAddr::new([9; 6]));
        }

        /// The binaries' wrappers write every PHY's parameters.
        #[cfg(any(feature = "stack-full-extended", feature = "stack-hci-layer-extended"))]
        #[test]
        fn extended_phy_commands_write_every_phy() {
            use crate::aci::durations::{
                CeLength, ConnInterval, ExtScanInterval, ExtScanWindow, SupervisionTimeout,
            };
            use crate::aci::flags::{InitiatingPhys, ScanningPhys};
            use crate::aci::ranges::ConnLatency;
            use crate::aci::values::{
                AddressType, HciOwnAddressType, InitiatorFilterPolicy, ScanType,
                ScanningFilterPolicy,
            };
            use crate::standard::{
                InitParamPhy, LeExtCreateConn, LeSetExtScanParams, ScanParamPhy,
            };

            let phy = ScanParamPhy {
                scan_type: ScanType::Active,
                scan_interval: ExtScanInterval::from_units(0x10).unwrap(),
                scan_window: ExtScanWindow::from_units(0x08).unwrap(),
            };
            let (bytes, len) = encode(&LeSetExtScanParams::new(
                HciOwnAddressType::Public,
                ScanningFilterPolicy::BasicUnfiltered,
                ScanningPhys::LE_1M,
                [phy; 2],
            ));
            assert_eq!(bytes[2], 13);
            assert_eq!(bytes[6..len], [1, 0x10, 0, 0x08, 0, 1, 0x10, 0, 0x08, 0]);

            let phy = InitParamPhy {
                scan_interval: ExtScanInterval::from_units(0x10).unwrap(),
                scan_window: ExtScanWindow::from_units(0x10).unwrap(),
                conn_interval_min: ConnInterval::MIN,
                conn_interval_max: ConnInterval::MIN,
                conn_latency: ConnLatency::MIN,
                supervision_timeout: SupervisionTimeout::from_millis(1000).unwrap(),
                min_ce_length: CeLength::MIN,
                max_ce_length: CeLength::MIN,
            };
            let command = LeExtCreateConn::new(
                InitiatorFilterPolicy::PeerAddress,
                HciOwnAddressType::Public,
                AddressType::Public,
                BdAddr::new([0; 6]),
                InitiatingPhys::LE_1M,
                [phy; 3],
            );
            let (bytes, len) = encode(&command);
            assert_eq!(bytes[..3], [0x43, 0x20, 58]);
            assert_eq!(len, 61);
        }
    }
}
