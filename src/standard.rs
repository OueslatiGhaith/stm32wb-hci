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
//! lacks others, so this module declares them with
//! [`standard_command!`](stm32wb_hci_macros::standard_command), checked like
//! the vendor commands:
//!
//! - bt-hci's `Disconnect` waits for Command Complete, but the binaries
//!   answer `hci_disconnect` with Command Status;
//! - bt-hci's `LeTransmitterTest` has opcode 0x201C,
//!   `hci_le_read_supported_states`, rather than 0x201E, and its
//!   `LeTransmitterTestV3` has 0x205E, `hci_le_generate_dhkey_v2`;
//! - bt-hci's `LeSetExtScanParams` and `LeExtCreateConn` write one parameter
//!   set per PHY selected, where the binaries' wrappers always write two and
//!   three, the catalog's layout;
//! - bt-hci sends none of `hci_le_read_local_p256_public_key`,
//!   `hci_le_generate_dhkey`, `hci_le_read_peer_resolvable_address`,
//!   `hci_le_read_local_resolvable_address`, or
//!   `hci_le_set_resolvable_private_address_timeout_v2`.
//!
//! Their names follow bt-hci's, so importing both modules' command of one
//! name needs a rename.

use stm32wb_hci_macros::{standard_command, vendor_struct};

standard_command! {
    /// Terminate a connection with `reason`, completing with Command Status;
    /// a Disconnection Complete event follows.
    hci_disconnect => Disconnect {
        connection_handle: bt_hci::param::ConnHandle,
        reason: u8,
    }
}

standard_command! {
    /// Start transmitting test packets on `tx_frequency`.
    hci_le_transmitter_test => LeTransmitterTest {
        tx_frequency: u8,
        length_of_test_data: u8,
        packet_payload: u8,
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
        peer_identity_address_type: u8,
        peer_identity_address: bt_hci::param::BdAddr,
    } -> LePeerResolvableAddr {
        peer_resolvable_address: bt_hci::param::BdAddr,
    }
}

standard_command! {
    /// Read the resolvable private address the controller uses with a peer.
    hci_le_read_local_resolvable_address => LeReadLocalResolvableAddr {
        peer_identity_address_type: u8,
        peer_identity_address: bt_hci::param::BdAddr,
    } -> LeLocalResolvableAddr {
        local_resolvable_address: bt_hci::param::BdAddr,
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

standard_command! {
    /// Set the extended scan parameters. The wrapper always writes two PHY
    /// parameter sets, whatever `scanning_phys` selects.
    hci_le_set_extended_scan_parameters => LeSetExtScanParams {
        own_address_type: u8,
        scanning_filter_policy: u8,
        scanning_phys: u8,
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

standard_command! {
    /// Create a connection with extended advertising. The wrapper always
    /// writes three PHY parameter sets, whatever `initiating_phys` selects.
    hci_le_extended_create_connection => LeExtCreateConn {
        initiator_filter_policy: u8,
        own_address_type: u8,
        peer_address_type: u8,
        peer_address: bt_hci::param::BdAddr,
        initiating_phys: u8,
        init_param_phy: [InitParamPhy; 3],
    }
}

standard_command! {
    /// Compute a Diffie-Hellman key with the peer's public key, or the debug
    /// key as `key_type` selects.
    hci_le_generate_dhkey_v2 => LeGenerateDhkeyV2 {
        remote_p256_public_key: [u8; 64],
        key_type: u8,
    }
}

standard_command! {
    /// Bound the random timeout between resolvable private address changes.
    hci_le_set_resolvable_private_address_timeout_v2 => LeSetResolvablePrivateAddrTimeoutV2 {
        rpa_timeout_min: u16,
        rpa_timeout_max: u16,
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
}

#[cfg(test)]
mod tests {
    use bt_hci::cmd::{Cmd, controller_baseband, le};

    use super::*;
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
            command_status::<Disconnect>();
            supported::<Disconnect>();
            let (bytes, len) = encode(&Disconnect::new(
                bt_hci::param::ConnHandle::new(0x0801),
                0x13,
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
            let (bytes, len) = encode(&LeReadPeerResolvableAddr::new(0, address));
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
            let phy = ScanParamPhy {
                scan_type: 1,
                scan_interval: 0x10,
                scan_window: 0x08,
            };
            let (bytes, len) = encode(&LeSetExtScanParams::new(0, 0, 0x01, [phy; 2]));
            assert_eq!(bytes[2], 13);
            assert_eq!(bytes[6..len], [1, 0x10, 0, 0x08, 0, 1, 0x10, 0, 0x08, 0]);

            let phy = InitParamPhy {
                scan_interval: 0x10,
                scan_window: 0x10,
                conn_interval_min: 6,
                conn_interval_max: 6,
                conn_latency: 0,
                supervision_timeout: 100,
                min_ce_length: 0,
                max_ce_length: 0,
            };
            let command = LeExtCreateConn::new(0, 0, 0, BdAddr::new([0; 6]), 0x01, [phy; 3]);
            let (bytes, len) = encode(&command);
            assert_eq!(bytes[..3], [0x43, 0x20, 58]);
            assert_eq!(len, 61);
        }
    }
}
