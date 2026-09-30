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
//! bt-hci 0.10 declares some of the catalogs' commands differently, and
//! lacks others, so this module declares them with
//! [`standard_command!`](stm32wb_hci_macros::standard_command), checked like
//! the vendor commands:
//!
//! - bt-hci's `Disconnect` waits for Command Complete, but the binaries
//!   answer `hci_disconnect` with Command Status;
//! - bt-hci's `LeTransmitterTest` has opcode 0x201C,
//!   `hci_le_read_supported_states`, rather than 0x201E, and its
//!   `LeTransmitterTestV3` has 0x205E, `hci_le_generate_dhkey_v2`;
//! - bt-hci's `LeSetExtScanParams`, `LeExtCreateConn`, and
//!   `LeExtCreateConnV2` write one parameter set per PHY selected, where the
//!   wrappers always write two and three, the catalogs' layout;
//! - bt-hci's `LeRemoteConnectionParameterRequestReply` and
//!   `LeRemoteConnectionParameterRequestNegativeReply` wait for Command
//!   Status, but STM32WBA answers them with Command Complete, returning the
//!   connection handle;
//! - bt-hci's `LeReadBufferSizeV2` returns a 16-bit
//!   Total_Num_ISO_Data_Packets, the Core specification's one byte;
//! - bt-hci's `BigHandle` is two bytes, where the Core specification's
//!   BIG_Handle is one, so its `LeBigCreateSync` and `LeBigTerminateSync`
//!   misencode it;
//! - bt-hci's `LeCreateBigTest` and `LeSetExtAdvParamsV2` encode other
//!   widths than STM32WBA's, and its `LeFrameSpaceUpdate` waits for Command
//!   Complete, where STM32WBA answers with Command Status;
//! - bt-hci sends none of `hci_le_read_local_p256_public_key`,
//!   `hci_le_generate_dhkey`, `hci_le_read_peer_resolvable_address`,
//!   `hci_le_read_local_resolvable_address`,
//!   `hci_le_set_resolvable_private_address_timeout_v2`, or the STM32WBA
//!   commands declared after them.
//!
//! Their names follow bt-hci's, so importing both modules' command of one
//! name needs a rename.
//!
//! Every other Core event is bt-hci's. Its event code, or LE meta subevent
//! code, is checked at compile time on every target emitting it, and a
//! generated test checks that bt-hci decodes exactly the catalog's width
//! where that width is fixed. The events carrying a BIG_Handle, which bt-hci
//! decodes as two bytes, and the STM32WBA events bt-hci lacks are declared
//! with [`standard_event!`](stm32wb_hci_macros::standard_event), named as
//! bt-hci names its own. The events carrying lists are tested here by hand. The transport
//! consumes Command Complete and Command Status, and ST's C decodes the LE
//! advertising report procedurally, so the catalog has no layout for them to
//! check.

use stm32wb_hci_macros::{standard_command, standard_event, vendor_struct};

standard_command! {
    /// Terminate a connection with `reason`, completing with Command Status;
    /// a Disconnection Complete event follows.
    hci_disconnect => Disconnect {
        connection_handle: bt_hci::param::ConnHandle,
        reason: crate::aci::values::DisconnectReason,
    }
}

standard_command! {
    /// Start transmitting test packets on `tx_frequency`.
    hci_le_transmitter_test => LeTransmitterTest {
        tx_frequency: crate::aci::ranges::RfChannel,
        length_of_test_data: crate::aci::ranges::TestDataLength,
        packet_payload: crate::aci::values::TestPayload,
    }
}

standard_command! {
    /// Generate a P-256 key pair; an LE Read Local P-256 Public Key Complete
    /// event returns the public key.
    hci_le_read_local_p256_public_key => LeReadLocalP256PublicKey {}
}

standard_command! {
    /// Compute a Diffie-Hellman key with the peer's public key; an LE
    /// Generate DHKey Complete event returns it.
    hci_le_generate_dhkey => LeGenerateDhkey {
        remote_p256_public_key: [u8; 64],
    }
}

standard_command! {
    /// Read the resolvable private address the peer uses.
    hci_le_read_peer_resolvable_address => LeReadPeerResolvableAddr {
        peer_identity_address_type: crate::aci::values::AddressType,
        peer_identity_address: bt_hci::param::BdAddr,
    } -> LePeerResolvableAddr {
        peer_resolvable_address: bt_hci::param::BdAddr,
    }
}

standard_command! {
    /// Read the resolvable private address the controller uses with a peer.
    hci_le_read_local_resolvable_address => LeReadLocalResolvableAddr {
        peer_identity_address_type: crate::aci::values::AddressType,
        peer_identity_address: bt_hci::param::BdAddr,
    } -> LeLocalResolvableAddr {
        local_resolvable_address: bt_hci::param::BdAddr,
    }
}

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
    /// Compute a Diffie-Hellman key with the peer's public key, or the debug
    /// key as `key_type` selects.
    hci_le_generate_dhkey_v2 => LeGenerateDhkeyV2 {
        remote_p256_public_key: [u8; 64],
        key_type: crate::aci::values::DhkeyPrivateKey,
    }
}

standard_command! {
    /// Bound the random timeout between resolvable private address changes.
    hci_le_set_resolvable_private_address_timeout_v2 => LeSetResolvablePrivateAddrTimeoutV2 {
        rpa_timeout_min: crate::aci::ranges::RpaTimeout,
        rpa_timeout_max: crate::aci::ranges::RpaTimeout,
    }
}

standard_command! {
    /// Configure the data path between the Host and the Controller in one
    /// direction.
    hci_configure_data_path => ConfigureDataPath {
        data_path_direction: u8,
        data_path_id: u8,
        vendor_specific_config: &'a [u8],
    }
}

standard_command! {
    /// Add a device to the Monitored Advertisers List, with the RSSI
    /// thresholds to report.
    hci_le_add_device_to_monitored_advertisers_list => LeAddDeviceToMonitoredAdvertisersList {
        address_type: u8,
        address: bt_hci::param::BdAddr,
        rssi_threshold_low: u8,
        rssi_threshold_high: u8,
        timeout: u8,
    }
}

standard_command! {
    /// Synchronize to a BIG, receiving the BISes `bis` lists.
    hci_le_big_create_sync => LeBigCreateSync {
        big_handle: u8,
        sync_handle: u16,
        encryption: u8,
        broadcast_code: [u8; 16],
        mse: u8,
        big_sync_timeout: u16,
        bis: &'a [u8],
    }
}

standard_command! {
    /// Stop synchronizing, or stop trying to synchronize, to a BIG.
    hci_le_big_terminate_sync => LeBigTerminateSync {
        big_handle: u8,
    } -> LeBigTerminateSyncReturn {
        big_handle: u8,
    }
}

standard_command! {
    /// Remove every device from the Monitored Advertisers List.
    hci_le_clear_monitored_advertisers_list => LeClearMonitoredAdvertisersList {}
}

standard_command! {
    /// Start or stop requesting Constant Tone Extensions from the peer of a
    /// connection.
    hci_le_connection_cte_request_enable => LeConnectionCteRequestEnable {
        connection_handle: bt_hci::param::ConnHandle,
        enable: u8,
        cte_request_interval: u16,
        requested_cte_length: u8,
        requested_cte_type: u8,
    } -> LeConnectionCteRequestEnableReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Create a BIG with test parameters.
    hci_le_create_big_test => LeCreateBigTest {
        big_handle: u8,
        advertising_handle: u8,
        num_bis: u8,
        sdu_interval: [u8; 3],
        iso_interval: u16,
        nse: u8,
        max_sdu: u16,
        max_pdu: u16,
        phy: u8,
        packing: u8,
        framing: u8,
        bn: u8,
        irc: u8,
        pto: u8,
        encryption: u8,
        broadcast_code: [u8; 16],
    }
}

standard_command! {
    /// Create or update the Channel Sounding configuration `config_id` of a
    /// connection.
    hci_le_cs_create_config => LeCsCreateConfig {
        connection_handle: bt_hci::param::ConnHandle,
        config_id: u8,
        create_context: u8,
        main_mode_type: u8,
        sub_mode_type: u8,
        min_main_mode_steps: u8,
        max_main_mode_steps: u8,
        main_mode_repetition: u8,
        mode_0_steps: u8,
        role: u8,
        rtt_type: u8,
        cs_sync_phy: u8,
        channel_map: [u8; 10],
        channel_map_repetition: u8,
        channel_selection_type: u8,
        ch3c_shape: u8,
        ch3c_jump: u8,
        reserved: u8,
    }
}

standard_command! {
    /// Enable or disable the Channel Sounding procedures of a configuration.
    hci_le_cs_procedure_enable => LeCsProcedureEnable {
        connection_handle: bt_hci::param::ConnHandle,
        config_id: u8,
        enable: u8,
    }
}

standard_command! {
    /// Read the Channel Sounding capabilities of the local Controller.
    hci_le_cs_read_local_supported_capabilities => LeCsReadLocalSupportedCapabilities {} -> LeCsLocalSupportedCapabilities {
        num_config_supported: u8,
        max_consecutive_procedures_supported: u16,
        num_antennas_supported: u8,
        max_antenna_paths_supported: u8,
        roles_supported: u8,
        optional_modes_supported: u8,
        rtt_capability: u8,
        rtt_aa_only_n: u8,
        rtt_sounding_n: u8,
        rtt_random_payload_n: u8,
        nadm_sounding_capability: u16,
        nadm_random_capability: u16,
        cs_sync_phys_supported: u8,
        subfeatures_supported: u16,
        t_ip1_times_supported: u16,
        t_ip2_times_supported: u16,
        t_fcs_times_supported: u16,
        t_pm_times_supported: u16,
        t_sw_time_supported: u8,
        tx_snr_capability: u8,
    }
}

standard_command! {
    /// Read the mode 0 Frequency Actuation Error table of the peer.
    hci_le_cs_read_remote_fae_table => LeCsReadRemoteFaeTable {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Read the Channel Sounding capabilities of the peer.
    hci_le_cs_read_remote_supported_capabilities => LeCsReadRemoteSupportedCapabilities {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Remove the Channel Sounding configuration `config_id` of a connection.
    hci_le_cs_remove_config => LeCsRemoveConfig {
        connection_handle: bt_hci::param::ConnHandle,
        config_id: u8,
    }
}

standard_command! {
    /// Start the Channel Sounding Security Start procedure on a connection.
    hci_le_cs_security_enable => LeCsSecurityEnable {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Set the channels Channel Sounding may use.
    hci_le_cs_set_channel_classification => LeCsSetChannelClassification {
        channel_classification: [u8; 10],
    }
}

standard_command! {
    /// Set the default Channel Sounding settings of a connection.
    hci_le_cs_set_default_settings => LeCsSetDefaultSettings {
        connection_handle: bt_hci::param::ConnHandle,
        role_enable: u8,
        cs_sync_antenna_selection: u8,
        max_tx_power: u8,
    } -> LeCsSetDefaultSettingsReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Set the scheduling parameters of the Channel Sounding procedures of a
    /// configuration.
    hci_le_cs_set_procedure_parameters => LeCsSetProcedureParameters {
        connection_handle: bt_hci::param::ConnHandle,
        config_id: u8,
        max_procedure_len: u16,
        min_procedure_interval: u16,
        max_procedure_interval: u16,
        max_procedure_count: u16,
        min_subevent_len: [u8; 3],
        max_subevent_len: [u8; 3],
        tone_antenna_config_selection: u8,
        phy: u8,
        tx_power_delta: u8,
        preferred_peer_antenna: u8,
        snr_control_initiator: u8,
        snr_control_reflector: u8,
    } -> LeCsSetProcedureParametersReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Start a Channel Sounding test.
    hci_le_cs_test => LeCsTest {
        main_mode_type: u8,
        sub_mode_type: u8,
        main_mode_repetition: u8,
        mode_0_steps: u8,
        role: u8,
        rtt_type: u8,
        cs_sync_phy: u8,
        cs_sync_antenna_selection: u8,
        subevent_len: [u8; 3],
        subevent_interval: u16,
        max_num_subevents: u8,
        transmit_power_level: u8,
        t_ip1_time: u8,
        t_ip2_time: u8,
        t_fcs_time: u8,
        t_pm_time: u8,
        t_sw_time: u8,
        tone_antenna_config_selection: u8,
        reserved: u8,
        snr_control_initiator: u8,
        snr_control_reflector: u8,
        drbg_nonce: u16,
        channel_map_repetition: u8,
        override_config: u16,
        override_parameters_data: &'a [u8],
    }
}

standard_command! {
    /// Stop the Channel Sounding test in progress.
    hci_le_cs_test_end => LeCsTestEnd {}
}

standard_command! {
    /// Write the cached mode 0 Frequency Actuation Error table of the peer.
    hci_le_cs_write_cached_remote_fae_table => LeCsWriteCachedRemoteFaeTable {
        connection_handle: bt_hci::param::ConnHandle,
        remote_fae_table: [u8; 72],
    } -> LeCsWriteCachedRemoteFaeTableReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Write the cached Channel Sounding capabilities of the peer.
    hci_le_cs_write_cached_remote_supported_capabilities => LeCsWriteCachedRemoteSupportedCapabilities {
        connection_handle: bt_hci::param::ConnHandle,
        num_config_supported: u8,
        max_consecutive_procedures_supported: u16,
        num_antennas_supported: u8,
        max_antenna_paths_supported: u8,
        roles_supported: u8,
        modes_supported: u8,
        rtt_capability: u8,
        rtt_aa_only_n: u8,
        rtt_sounding_n: u8,
        rtt_random_payload_n: u8,
        nadm_sounding_capability: u16,
        nadm_random_capability: u16,
        cs_sync_phys_supported: u8,
        subfeatures_supported: u16,
        t_ip1_times_supported: u16,
        t_ip2_times_supported: u16,
        t_fcs_times_supported: u16,
        t_pm_times_supported: u16,
        t_sw_time_supported: u8,
        tx_snr_capability: u8,
    } -> LeCsWriteCachedRemoteSupportedCapabilitiesReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Enable or disable monitoring the Monitored Advertisers List.
    hci_le_enable_monitoring_advertisers => LeEnableMonitoringAdvertisers {
        enable: u8,
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

standard_command! {
    /// Request new frame space values on a connection.
    hci_le_frame_space_update => LeFrameSpaceUpdate {
        connection_handle: bt_hci::param::ConnHandle,
        frame_space_min: u16,
        frame_space_max: u16,
        phys: u8,
        spacing_types: u16,
    }
}

standard_command! {
    /// Read every page of the LE features of the Controller.
    hci_le_read_all_local_supported_features => LeReadAllLocalSupportedFeatures {} -> LeAllLocalSupportedFeatures {
        max_page: u8,
        le_features: [u8; 248],
    }
}

standard_command! {
    /// Read the LE feature pages of the peer, up to `pages_requested`.
    hci_le_read_all_remote_features => LeReadAllRemoteFeatures {
        connection_handle: bt_hci::param::ConnHandle,
        pages_requested: u8,
    }
}

standard_command! {
    /// Read the size and number of the ACL and ISO data buffers of the
    /// Controller.
    hci_le_read_buffer_size_v2 => LeReadBufferSizeV2 {} -> LeBufferSizeV2 {
        le_acl_data_packet_length: u16,
        total_num_le_acl_data_packets: u8,
        iso_data_packet_length: u16,
        total_num_iso_data_packets: u8,
    }
}

standard_command! {
    /// Read how many entries the Monitored Advertisers List holds.
    hci_le_read_monitored_advertisers_list_size => LeReadMonitoredAdvertisersListSize {} -> LeMonitoredAdvertisersListSize {
        number: u8,
    }
}

standard_command! {
    /// Start receiving test packets, with Constant Tone Extensions.
    hci_le_receiver_test_v3 => LeReceiverTestV3 {
        rx_frequency: u8,
        phy: u8,
        modulation_index: u8,
        expected_cte_length: u8,
        expected_cte_type: u8,
        slot_durations: u8,
        antenna_ids: &'a [u8],
    }
}

standard_command! {
    /// Reject the connection parameters the peer requested.
    hci_le_remote_connection_parameter_request_negative_reply => LeRemoteConnectionParameterRequestNegativeReply {
        connection_handle: bt_hci::param::ConnHandle,
        reason: u8,
    } -> LeRemoteConnectionParameterRequestNegativeReplyReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Accept the connection parameters the peer requested, with the given
    /// ones.
    hci_le_remote_connection_parameter_request_reply => LeRemoteConnectionParameterRequestReply {
        connection_handle: bt_hci::param::ConnHandle,
        interval_min: u16,
        interval_max: u16,
        max_latency: u16,
        timeout: u16,
        min_ce_length: u16,
        max_ce_length: u16,
    } -> LeRemoteConnectionParameterRequestReplyReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Remove a device from the Monitored Advertisers List.
    hci_le_remove_device_from_monitored_advertisers_list => LeRemoveDeviceFromMonitoredAdvertisersList {
        address_type: u8,
        address: bt_hci::param::BdAddr,
    }
}

standard_command! {
    /// Enable or disable sampling the Constant Tone Extensions received on a
    /// connection.
    hci_le_set_connection_cte_receive_parameters => LeSetConnectionCteReceiveParameters {
        connection_handle: bt_hci::param::ConnHandle,
        sampling_enable: u8,
        slot_durations: u8,
        antenna_ids: &'a [u8],
    } -> LeSetConnectionCteReceiveParametersReturn {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_command! {
    /// Enable or disable sampling the Constant Tone Extensions of a periodic
    /// advertising train.
    hci_le_set_connectionless_iq_sampling_enable => LeSetConnectionlessIqSamplingEnable {
        sync_handle: u16,
        sampling_enable: u8,
        slot_durations: u8,
        max_sampled_ctes: u8,
        antenna_ids: &'a [u8],
    } -> LeSetConnectionlessIqSamplingEnableReturn {
        sync_handle: u16,
    }
}

standard_command! {
    /// Set the parameters of an advertising set, with its PHY options.
    hci_le_set_extended_advertising_parameters_v2 => LeSetExtAdvParamsV2 {
        advertising_handle: u8,
        adv_event_properties: u16,
        primary_adv_interval_min: [u8; 3],
        primary_adv_interval_max: [u8; 3],
        primary_adv_channel_map: u8,
        own_address_type: u8,
        peer_address_type: u8,
        peer_address: bt_hci::param::BdAddr,
        adv_filter_policy: u8,
        adv_tx_power: u8,
        primary_adv_phy: u8,
        secondary_adv_max_skip: u8,
        secondary_adv_phy: u8,
        adv_sid: u8,
        scan_req_notification_enable: u8,
        primary_adv_phy_options: u8,
        secondary_adv_phy_options: u8,
    } -> LeSetExtAdvParamsV2Return {
        selected_tx_power: u8,
    }
}

standard_command! {
    /// Start transmitting test packets, with Constant Tone Extensions.
    hci_le_transmitter_test_v3 => LeTransmitterTestV3 {
        tx_frequency: u8,
        length_of_test_data: u8,
        packet_payload: u8,
        phy: u8,
        cte_length: u8,
        cte_type: u8,
        antenna_ids: &'a [u8],
    }
}

standard_command! {
    /// Start transmitting test packets, with Constant Tone Extensions, at a
    /// transmit power level.
    hci_le_transmitter_test_v4 => LeTransmitterTestV4 {
        tx_frequency: u8,
        length_of_test_data: u8,
        packet_payload: u8,
        phy: u8,
        cte_length: u8,
        cte_type: u8,
        antenna_ids: &'a [u8],
        tx_power_level: u8,
    }
}

standard_command! {
    /// Read whether the Controller assesses channels.
    hci_read_afh_channel_assessment_mode => ReadAfhChannelAssessmentMode {} -> AfhChannelAssessmentMode {
        afh_channel_assessment_mode: u8,
    }
}

standard_command! {
    /// Read the Connection Accept Timeout.
    hci_read_connection_accept_timeout => ReadConnectionAcceptTimeout {} -> ConnectionAcceptTimeout {
        connection_accept_timeout: u16,
    }
}

standard_command! {
    /// Read the range of Controller delays a codec supports on a transport
    /// and direction.
    hci_read_local_supported_controller_delay => ReadLocalSupportedControllerDelay {
        codec_id: [u8; 5],
        logical_transport_type: u8,
        direction: u8,
        codec_configuration: &'a [u8],
    } -> LocalSupportedControllerDelay {
        min_controller_delay: [u8; 3],
        max_controller_delay: [u8; 3],
    }
}

standard_command! {
    /// Hint the base interval of the communications the Controller can
    /// expect.
    hci_set_ecosystem_base_interval => SetEcosystemBaseInterval {
        interval: u16,
    }
}

standard_command! {
    /// Set whether the Controller assesses channels.
    hci_write_afh_channel_assessment_mode => WriteAfhChannelAssessmentMode {
        afh_channel_assessment_mode: u8,
    }
}

standard_command! {
    /// Set the Connection Accept Timeout.
    hci_write_connection_accept_timeout => WriteConnectionAcceptTimeout {
        connection_accept_timeout: u16,
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
}

vendor_struct! {
    /// The connection handle of one BIS.
    Connection_Handle_t => BisConnHandle {
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_event! {
    /// A BIG was created, or creating it failed. Before STM32CubeWBA 1.2.0
    /// the BIS handles are plain integers, the same bytes as later.
    hci_le_create_big_complete_event => LeCreateBigComplete {
        status: u8,
        big_handle: u8,
        big_sync_delay: [u8; 3],
        transport_latency_big: [u8; 3],
        phy: u8,
        nse: u8,
        bn: u8,
        pto: u8,
        irc: u8,
        max_pdu: u16,
        iso_interval: u16,
        #[wire(wba_before = "1.2.0")]
        connection_handle: crate::wire::Elements<'a, bt_hci::param::ConnHandle>,
        #[wire(wba_since = "1.2.0")]
        connection_handle: crate::wire::Elements<'a, BisConnHandle>,
    }
}

standard_event! {
    /// A BIG was terminated.
    hci_le_terminate_big_complete_event => LeTerminateBigComplete {
        big_handle: u8,
        reason: u8,
    }
}

standard_event! {
    /// Synchronizing to a BIG succeeded or failed. Before STM32CubeWBA 1.2.0
    /// the BIS handles are plain integers, the same bytes as later.
    hci_le_big_sync_established_event => LeBigSyncEstablished {
        status: u8,
        big_handle: u8,
        transport_latency_big: [u8; 3],
        nse: u8,
        bn: u8,
        pto: u8,
        irc: u8,
        max_pdu: u16,
        iso_interval: u16,
        #[wire(wba_before = "1.2.0")]
        connection_handle: crate::wire::Elements<'a, bt_hci::param::ConnHandle>,
        #[wire(wba_since = "1.2.0")]
        connection_handle: crate::wire::Elements<'a, BisConnHandle>,
    }
}

standard_event! {
    /// Synchronization to a BIG was lost or terminated.
    hci_le_big_sync_lost_event => LeBigSyncLost {
        big_handle: u8,
        reason: u8,
    }
}

standard_event! {
    /// A CIS was established, lost before being established, or rejected by
    /// the Peripheral.
    hci_le_cis_established_v2_event => LeCisEstablishedV2 {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        cig_sync_delay: [u8; 3],
        cis_sync_delay: [u8; 3],
        transport_latency_c_to_p: [u8; 3],
        transport_latency_p_to_c: [u8; 3],
        phy_c_to_p: u8,
        phy_p_to_c: u8,
        nse: u8,
        bn_c_to_p: u8,
        bn_p_to_c: u8,
        ft_c_to_p: u8,
        ft_p_to_c: u8,
        max_pdu_c_to_p: u16,
        max_pdu_p_to_c: u16,
        iso_interval: u16,
        sub_interval: [u8; 3],
        max_sdu_c_to_p: u16,
        max_sdu_p_to_c: u16,
        sdu_interval_c_to_p: [u8; 3],
        sdu_interval_p_to_c: [u8; 3],
        framing: u8,
    }
}

standard_event! {
    /// A Channel Sounding configuration was created, updated, or removed.
    hci_le_cs_config_complete_event => LeCsConfigComplete {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        config_id: u8,
        action: u8,
        main_mode_type: u8,
        sub_mode_type: u8,
        min_main_mode_steps: u8,
        max_main_mode_steps: u8,
        main_mode_repetition: u8,
        mode_0_steps: u8,
        role: u8,
        rtt_type: u8,
        cs_sync_phy: u8,
        channel_map: [u8; 10],
        channel_map_repetition: u8,
        channel_selection_type: u8,
        ch3c_shape: u8,
        ch3c_jump: u8,
        reserved: u8,
        t_ip1_time: u8,
        t_ip2_time: u8,
        t_fcs_time: u8,
        t_pm_time: u8,
    }
}

standard_event! {
    /// Channel Sounding procedures were scheduled or disabled.
    hci_le_cs_procedure_enable_complete_event => LeCsProcedureEnableComplete {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        config_id: u8,
        state: u8,
        tone_antenna_config_selection: u8,
        selected_tx_power: u8,
        subevent_len: [u8; 3],
        subevents_per_event: u8,
        subevent_interval: u16,
        event_interval: u16,
        procedure_interval: u16,
        procedure_count: u16,
        max_procedure_len: u16,
    }
}

standard_event! {
    /// The peer's mode 0 Frequency Actuation Error table was read.
    hci_le_cs_read_remote_fae_table_complete_event => LeCsReadRemoteFaeTableComplete {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        remote_fae_table: [u8; 72],
    }
}

standard_event! {
    /// The Channel Sounding capabilities were exchanged with the peer.
    hci_le_cs_read_remote_supported_capabilities_complete_event => LeCsReadRemoteSupportedCapabilitiesComplete {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        num_config_supported: u8,
        max_consecutive_procedures_supported: u16,
        num_antennas_supported: u8,
        max_antenna_paths_supported: u8,
        roles_supported: u8,
        optional_modes_supported: u8,
        rtt_capability: u8,
        rtt_aa_only_n: u8,
        rtt_sounding_n: u8,
        rtt_random_payload_n: u8,
        nadm_sounding_capability: u16,
        nadm_random_capability: u16,
        cs_sync_phys_supported: u8,
        subfeatures_supported: u16,
        t_ip1_times_supported: u16,
        t_ip2_times_supported: u16,
        t_fcs_times_supported: u16,
        t_pm_times_supported: u16,
        t_sw_time_supported: u8,
        tx_snr_capability: u8,
    }
}

standard_event! {
    /// The Channel Sounding Security Start procedure completed.
    hci_le_cs_security_enable_complete_event => LeCsSecurityEnableComplete {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
    }
}

standard_event! {
    /// A Channel Sounding test ended.
    hci_le_cs_test_end_complete_event => LeCsTestEndComplete {
        status: u8,
    }
}

standard_event! {
    /// A connection was created, possibly from an advertising set or a
    /// periodic advertising train.
    hci_le_enhanced_connection_complete_v2_event => LeEnhancedConnectionCompleteV2 {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        role: u8,
        peer_address_type: u8,
        peer_address: bt_hci::param::BdAddr,
        local_resolvable_private_address: bt_hci::param::BdAddr,
        peer_resolvable_private_address: bt_hci::param::BdAddr,
        conn_interval: u16,
        conn_latency: u16,
        supervision_timeout: u16,
        central_clock_accuracy: u8,
        advertising_handle: u8,
        sync_handle: u16,
    }
}

standard_event! {
    /// A monitored advertiser crossed an RSSI threshold.
    hci_le_monitored_advertisers_report_event => LeMonitoredAdvertisersReport {
        address_type: u8,
        address: bt_hci::param::BdAddr,
        condition: u8,
    }
}

standard_event! {
    /// A periodic advertisement, or a subevent of one, was received, or not.
    hci_le_periodic_advertising_report_v2_event => LePeriodicAdvertisingReportV2 {
        sync_handle: u16,
        tx_power: u8,
        rssi: u8,
        cte_type: u8,
        periodic_event_counter: u16,
        subevent: u8,
        data_status: u8,
        data: &'a [u8],
    }
}

standard_event! {
    /// The Controller requests the data of the subevents it is about to
    /// transmit.
    hci_le_periodic_advertising_subevent_data_request_event => LePeriodicAdvertisingSubeventDataRequest {
        advertising_handle: u8,
        subevent_start: u8,
        subevent_data_count: u8,
    }
}

standard_event! {
    /// The first periodic advertisement of a train was received, or
    /// synchronizing failed.
    hci_le_periodic_advertising_sync_established_v2_event => LePeriodicAdvertisingSyncEstablishedV2 {
        status: u8,
        sync_handle: u16,
        advertising_sid: u8,
        advertiser_address_type: u8,
        advertiser_address: bt_hci::param::BdAddr,
        advertiser_phy: u8,
        periodic_advertising_interval: u16,
        advertiser_clock_accuracy: u8,
        num_subevents: u8,
        subevent_interval: u8,
        response_slot_delay: u8,
        response_slot_spacing: u8,
    }
}

standard_event! {
    /// Synchronization information received from a peer led to synchronizing
    /// to a periodic advertising train, or not.
    hci_le_periodic_advertising_sync_transfer_received_v2_event => LePeriodicAdvertisingSyncTransferReceivedV2 {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        service_data: u16,
        sync_handle: u16,
        advertising_sid: u8,
        advertiser_address_type: u8,
        advertiser_address: bt_hci::param::BdAddr,
        advertiser_phy: u8,
        periodic_advertising_interval: u16,
        advertiser_clock_accuracy: u8,
        num_subevents: u8,
        subevent_interval: u8,
        response_slot_delay: u8,
        response_slot_spacing: u8,
    }
}

standard_event! {
    /// The LE feature pages of the peer were read.
    hci_le_read_all_remote_features_complete_event => LeReadAllRemoteFeaturesComplete {
        status: u8,
        connection_handle: bt_hci::param::ConnHandle,
        max_remote_page: u8,
        max_valid_page: u8,
        le_features: [u8; 248],
    }
}

stm32wb_hci_macros::standard_events_enum! {
    /// Every Core event the selected target emits that this module declares,
    /// rather than bt-hci. Decode one from an event packet with
    /// [`StandardEvent::from_packet`].
    pub enum StandardEvent<'a> {
        hci_le_create_big_complete_event => LeCreateBigComplete(LeCreateBigComplete<'a>),
        hci_le_terminate_big_complete_event => LeTerminateBigComplete(LeTerminateBigComplete),
        hci_le_big_sync_established_event => LeBigSyncEstablished(LeBigSyncEstablished<'a>),
        hci_le_big_sync_lost_event => LeBigSyncLost(LeBigSyncLost),
        hci_le_cis_established_v2_event => LeCisEstablishedV2(LeCisEstablishedV2),
        hci_le_cs_config_complete_event => LeCsConfigComplete(LeCsConfigComplete),
        hci_le_cs_procedure_enable_complete_event => LeCsProcedureEnableComplete(LeCsProcedureEnableComplete),
        hci_le_cs_read_remote_fae_table_complete_event => LeCsReadRemoteFaeTableComplete(LeCsReadRemoteFaeTableComplete),
        hci_le_cs_read_remote_supported_capabilities_complete_event => LeCsReadRemoteSupportedCapabilitiesComplete(LeCsReadRemoteSupportedCapabilitiesComplete),
        hci_le_cs_security_enable_complete_event => LeCsSecurityEnableComplete(LeCsSecurityEnableComplete),
        hci_le_cs_test_end_complete_event => LeCsTestEndComplete(LeCsTestEndComplete),
        hci_le_enhanced_connection_complete_v2_event => LeEnhancedConnectionCompleteV2(LeEnhancedConnectionCompleteV2),
        hci_le_monitored_advertisers_report_event => LeMonitoredAdvertisersReport(LeMonitoredAdvertisersReport),
        hci_le_periodic_advertising_report_v2_event => LePeriodicAdvertisingReportV2(LePeriodicAdvertisingReportV2<'a>),
        hci_le_periodic_advertising_subevent_data_request_event => LePeriodicAdvertisingSubeventDataRequest(LePeriodicAdvertisingSubeventDataRequest),
        hci_le_periodic_advertising_sync_established_v2_event => LePeriodicAdvertisingSyncEstablishedV2(LePeriodicAdvertisingSyncEstablishedV2),
        hci_le_periodic_advertising_sync_transfer_received_v2_event => LePeriodicAdvertisingSyncTransferReceivedV2(LePeriodicAdvertisingSyncTransferReceivedV2),
        hci_le_read_all_remote_features_complete_event => LeReadAllRemoteFeaturesComplete(LeReadAllRemoteFeaturesComplete),
    }
}

#[cfg(test)]
mod tests {
    use bt_hci::cmd::{Cmd, controller_baseband, le};

    use super::*;
    use crate::catalog::Supported;

    fn supported<T: Supported>() {}

    /// The BIS handles of a BIG follow its one-byte handle and parameters,
    /// counted by Num_BIS, whether ST declares them as integers or as
    /// `Connection_Handle_t`.
    #[cfg(any(feature = "stack-wba-full", feature = "stack-wba-link-layer-only"))]
    #[test]
    fn big_events_list_their_bis_handles() {
        use bt_hci::FromHciBytes;

        let params = [
            0x00, 0x05, 1, 0, 0, 2, 0, 0, 0x01, 2, 1, 0, 1, 0xFB, 0, 0x10,
            0, // up to ISO_Interval
            2, 0x20, 0x00, 0x21, 0x00, // Num_BIS, then the handles
        ];
        let event = LeCreateBigComplete::from_hci_bytes_complete(&params).unwrap();
        assert_eq!(event.big_handle, 0x05);
        assert_eq!(event.iso_interval, 0x0010);
        assert_eq!(event.connection_handle.len(), 2);
        assert_eq!(event.connection_handle.as_bytes(), [0x20, 0x00, 0x21, 0x00]);
        assert!(LeCreateBigComplete::from_hci_bytes_complete(&params[..params.len() - 1]).is_err());

        let event = LeBigSyncLost::from_hci_bytes_complete(&[0x05, 0x13]).unwrap();
        assert_eq!((event.big_handle, event.reason), (0x05, 0x13));
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
        assert_eq!(LeTransmitterTest::OPCODE.to_raw(), 0x201E);
        supported::<LeTransmitterTest>();
    }

    /// The commands the advertising and scanning profile lacks.
    #[cfg(not(feature = "stack-hci-adv-scan"))]
    mod connected {
        use bt_hci::cmd::{AsyncCmd, Cmd, SyncCmd};
        use bt_hci::param::BdAddr;
        use bt_hci::{FromHciBytes, WriteHci};

        use super::super::*;
        use super::supported;
        use crate::aci::values::AddressType;

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
            use crate::aci::values::DisconnectReason;

            command_status::<Disconnect>();
            supported::<Disconnect>();
            let (bytes, len) = encode(&Disconnect::new(
                bt_hci::param::ConnHandle::new(0x0801),
                DisconnectReason::RemoteUserTerminated,
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
            let (bytes, len) = encode(&LeGenerateDhkey::new([7; 64]));
            assert_eq!(bytes[..3], [0x26, 0x20, 64]);
            assert_eq!(len, 67);
        }

        #[test]
        fn core_commands_bt_hci_lacks_use_the_catalog_opcodes() {
            assert_eq!(LeReadLocalP256PublicKey::OPCODE.to_raw(), 0x2025);
            supported::<LeReadLocalP256PublicKey>();
            command_status::<LeReadLocalP256PublicKey>();

            let address = BdAddr::new([1, 2, 3, 4, 5, 6]);
            let (bytes, len) = encode(&LeReadPeerResolvableAddr::new(AddressType::Public, address));
            assert_eq!(bytes[..len], [0x2B, 0x20, 7, 0, 1, 2, 3, 4, 5, 6]);
            let returned =
                <LeReadPeerResolvableAddr as SyncCmd>::Return::from_hci_bytes_complete(&[9; 6])
                    .unwrap();
            assert_eq!(returned.peer_resolvable_address, BdAddr::new([9; 6]));
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
                HciOwnAddressType, InitiatorFilterPolicy, ScanType, ScanningFilterPolicy,
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
