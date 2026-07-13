extern crate stm32wb_hci as hci;

use hci::event::command::*;
use hci::event::*;
use hci::vendor::event::command::VendorReturnParameters;
use std::convert::TryFrom;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[allow(dead_code)]
struct VendorEvent;
#[allow(dead_code)]
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct VendorError;

#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VendorStatus {
    FourFive,
    FiveZero,
}

impl TryFrom<u8> for VendorStatus {
    type Error = hci::BadStatusError;

    fn try_from(value: u8) -> Result<VendorStatus, Self::Error> {
        match value {
            0x45 => Ok(VendorStatus::FourFive),
            0x50 => Ok(VendorStatus::FiveZero),
            _ => Err(hci::BadStatusError::BadValue(value)),
        }
    }
}

#[test]
fn command_complete_failed() {
    let buffer = [0x0E, 3, 1, 0x67, 0x43];
    match Event::new(Packet(&buffer)) {
        Err(Error::UnknownOpcode(opcode)) => assert_eq!(opcode.0, 0x4367),
        other => panic!("Did not get unknown opcode: {:?}", other),
    }
}

#[test]
fn unsolicited_command_complete() {
    let buffer = [0x0E, 3, 1, 0x00, 0x00];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::Spontaneous => (),
                other => panic!("Got return parameters: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

macro_rules! status_only {
    {
        $($(#[$inner:ident $($args:tt)*])*
        $fn:ident($oc0:expr, $oc1:expr, $return:path);)*
    } => {
        $(
            $(#[$inner $($args)*])*
            #[test]
            fn $fn() {
                let buffer = [0x0E, 4, 8, $oc0, $oc1, 0];
                match Event::new(Packet(&buffer)) {
                    Ok(Event::CommandComplete(event)) => {
                        assert_eq!(event.num_hci_command_packets, 8);
                        match event.return_params {
                            $return(status) => {
                                assert_eq!(status, hci::Status::Success);
                            }
                            other => panic!("Wrong return parameters: {:?}", other),
                        }
                    }
                    other => panic!("Did not get command complete event: {:?}", other),
                }
            }
        )*
    }
}

status_only! {
    set_event_mask(0x01, 0x0C, ReturnParameters::SetEventMask);
    reset(0x03, 0x0C, ReturnParameters::Reset);
    le_set_event_mask(0x01, 0x20, ReturnParameters::LeSetEventMask);
    le_set_random_address(0x05, 0x20, ReturnParameters::LeSetRandomAddress);
    le_set_advertising_parameters(0x06, 0x20, ReturnParameters::LeSetAdvertisingParameters);
    le_set_advertising_data(0x08, 0x20, ReturnParameters::LeSetAdvertisingData);
    le_set_scan_response_data(0x09, 0x20, ReturnParameters::LeSetScanResponseData);
    le_set_advertising_enable(0x0A, 0x20, ReturnParameters::LeSetAdvertisingEnable);
    le_set_scan_parameters(0x0B, 0x20, ReturnParameters::LeSetScanParameters);
    le_set_scan_enable(0x0C, 0x20, ReturnParameters::LeSetScanEnable);
    le_create_connection_cancel(0x0E, 0x20, ReturnParameters::LeCreateConnectionCancel);
    le_clear_white_list(0x10, 0x20, ReturnParameters::LeClearWhiteList);
    le_add_device_to_whitelist(0x11, 0x20, ReturnParameters::LeAddDeviceToWhiteList);
    le_remove_device_from_whitelist(0x12, 0x20, ReturnParameters::LeRemoveDeviceFromWhiteList);
    le_set_host_channel_classification(0x14, 0x20,
                                       ReturnParameters::LeSetHostChannelClassification);
    le_receiver_test(0x1D, 0x20, ReturnParameters::LeReceiverTest);
    le_transmitter_test(0x1E, 0x20, ReturnParameters::LeTransmitterTest);
}

#[test]
fn read_tx_power_level() {
    let buffer = [0x0E, 7, 6, 0x2D, 0x0C, 0x00, 0x01, 0x02, 0x03];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 6);
            match event.return_params {
                ReturnParameters::ReadTxPowerLevel(params) => {
                    assert_eq!(params.conn_handle, hci::ConnectionHandle(0x0201));
                    assert_eq!(params.tx_power_level_dbm, 0x03);
                }
                other => panic!("Got return parameters: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn read_local_version_information() {
    let buffer = [
        0x0E, 12, 0x01, 0x01, 0x10, 0x00, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::ReadLocalVersionInformation(local_version_info) => {
                    assert_eq!(local_version_info.hci_version, 2);
                    assert_eq!(local_version_info.hci_revision, 0x0403);
                    assert_eq!(local_version_info.lmp_version, 5);
                    assert_eq!(local_version_info.manufacturer_name, 0x0706);
                    assert_eq!(local_version_info.lmp_subversion, 0x0908);
                }
                other => panic!(
                    "Did not get Read Local Version Info return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn read_local_supported_commands() {
    let buffer = [
        0x0E, 68, 1, 0x02, 0x10, 0x00, 0x01, 0x02, 0x04, 0x00, 0x10, 0x20, 0x40, 0x80, 0x01, 0x02,
        0x04, 0x08, 0x10, 0x00, 0x40, 0x80, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x01,
        0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::ReadLocalSupportedCommands(params) => {
                    assert_eq!(
                        params.supported_commands,
                        CommandFlags::INQUIRY
                            | CommandFlags::REJECT_CONNECTION_REQUEST
                            | CommandFlags::MASTER_LINK_KEY
                            | CommandFlags::PARK_STATE
                            | CommandFlags::FLOW_SPECIFICATION
                            | CommandFlags::WRITE_STORED_LINK_KEY
                            | CommandFlags::WRITE_SCAN_ENABLE
                            | CommandFlags::READ_PAGE_SCAN_ACTIVITY
                            | CommandFlags::WRITE_CLASS_OF_DEVICE
                            | CommandFlags::READ_TRANSMIT_POWER_LEVEL
                            | CommandFlags::READ_CURRENT_IAC_LAP
                            | CommandFlags::READ_INQUIRY_SCAN_TYPE
                            | CommandFlags::READ_LOCAL_EXTENDED_FEATURES
                            | CommandFlags::READ_CLOCK
                            | CommandFlags::READ_LOOPBACK_MODE
                            | CommandFlags::WRITE_EXTENDED_INQUIRY_RESPONSE
                            | CommandFlags::READ_DEFAULT_ERRONEOUS_DATA_REPORTING
                            | CommandFlags::USER_PASSKEY_REQUEST_NEGATIVE_REPLY
                            | CommandFlags::READ_ENCRYPTION_KEY_SIZE
                            | CommandFlags::DISCONNECT_LOGICAL_LINK
                            | CommandFlags::READ_LOCAL_AMP_ASSOC
                            | CommandFlags::AMP_TEST
                            | CommandFlags::READ_ENHANCED_TRANSMIT_POWER_LEVEL
                            | CommandFlags::LE_READ_BUFFER_SIZE
                            | CommandFlags::LE_SET_SCAN_PARAMETERS
                            | CommandFlags::LE_SET_HOST_CHANNEL_CLASSIFICATION
                            | CommandFlags::LE_RECEIVER_TEST
                            | CommandFlags::READ_LOCAL_SUPPORTED_CODECS
                            | CommandFlags::TRUNCATED_PAGE
                            | CommandFlags::READ_SYNCHRONIZATION_TRAIN_PARAMETERS
                            | CommandFlags::WRITE_SYNCHRONIZATION_TRAIN_PARAMETERS
                            | CommandFlags::WRITE_EXTENDED_PAGE_TIMEOUT
                            | CommandFlags::LE_GENERATE_DH_KEY
                            | CommandFlags::LE_READ_MAXIMUM_DATA_LENGTH
                            | CommandFlags::LE_SET_EXTENDED_SCAN_RESPONSE_DATA_COMMAND
                            | CommandFlags::LE_SET_EXTENDED_SCAN_PARAMETERS_COMMAND
                            | CommandFlags::LE_READ_PERIODIC_ADVERTISER_LIST_SIZE_COMMAND
                    );
                    assert!(params.supported_commands.is_set(CommandFlags::INQUIRY));
                    assert!(
                        params.supported_commands.contains(
                            CommandFlags::INQUIRY | CommandFlags::REJECT_CONNECTION_REQUEST
                        )
                    );
                }
                other => panic!(
                    "Did not get Read Supported Commands return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn read_local_supported_commands_failed_bad_command_flag() {
    let buffer = [
        0x0E, 68, 1, 0x02, 0x10, 0x00, 0x01, 0x02, 0x04, 0x00, 0x10, 0x20, 0x40, 0x80, 0x01, 0x02,
        0x04, 0x08, 0x10, 0x00, 0x40, 0x80, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x01,
        0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    match Event::new(Packet(&buffer)) {
        Err(Error::BadCommandFlag) => (),
        other => panic!("Did not get Bad Command Flag: {:?}", other),
    }
}

#[test]
fn read_local_supported_features() {
    let buffer = [
        0x0E, 12, 1, 0x03, 0x10, 0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::ReadLocalSupportedFeatures(params) => {
                    assert_eq!(
                        params.supported_features,
                        LmpFeatures::THREE_SLOT_PACKETS
                            | LmpFeatures::POWER_CONTROL_REQUESTS
                            | LmpFeatures::POWER_CONTROL
                            | LmpFeatures::ENHANCED_INQUIRY_SCAN
                            | LmpFeatures::AFH_CLASSIFICATION_PERIPHERAL
                            | LmpFeatures::ENHANCED_DATA_RATE_ESCO_2_MB_PER_S_MODE
                            | LmpFeatures::NON_FLUSHABLE_PACKET_BOUNDARY_FLAG
                            | LmpFeatures::EXTENDED_FEATURES
                    );
                }
                other => panic!(
                    "Did not get Read Supported Features return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn read_bd_addr() {
    let buffer = [
        0x0E, 10, 1, 0x09, 0x10, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::ReadBdAddr(params) => {
                    assert_eq!(
                        params.bd_addr,
                        hci::BdAddr([0x01, 0x02, 0x03, 0x04, 0x05, 0x06])
                    );
                }
                other => panic!("Did not get Read BDADDR return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn read_rssi() {
    let buffer = [0x0E, 7, 1, 0x05, 0x14, 0x00, 0x01, 0x02, 0x03];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::ReadRssi(params) => {
                    assert_eq!(params.conn_handle, hci::ConnectionHandle(0x0201));
                    assert_eq!(params.rssi, 0x03);
                }
                other => panic!("Did not get Read RSSI return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_buffer_size() {
    let buffer = [0x0E, 7, 2, 0x02, 0x20, 0x00, 0x01, 0x02, 0x03];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 2);
            match event.return_params {
                ReturnParameters::LeReadBufferSize(event) => {
                    assert_eq!(event.data_packet_length, 0x0201);
                    assert_eq!(event.data_packet_count, 0x03);
                }
                other => panic!("Did not get LE Read Buffer Size return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_local_supported_features() {
    let buffer = [
        0x0E, 12, 1, 0x03, 0x20, 0x00, 0x04, 0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeReadLocalSupportedFeatures(event) => {
                    assert_eq!(
                        event.supported_features,
                        LeFeatures::EXTENDED_REJECT_INDICATION
                            | LeFeatures::STABLE_MODULATION_INDEX_TX
                            | LeFeatures::MINIMUM_NUMBER_OF_USED_CHANNELS_PROCEDURE
                    );
                }
                other => panic!("Did not get LE Read Buffer Size return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_advertising_channel_tx_power() {
    let buffer = [0x0E, 5, 1, 0x07, 0x20, 0x00, 0x01];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeReadAdvertisingChannelTxPower(params) => {
                    assert_eq!(params.power, 0x01);
                }
                other => panic!(
                    "Did not get LE Read Advertising Channel TX Power return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_white_list_size() {
    let buffer = [0x0E, 5, 1, 0x0F, 0x20, 0x00, 0x16];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeReadWhiteListSize(status, white_list_size) => {
                    assert_eq!(status, hci::Status::Success);
                    assert_eq!(white_list_size, 0x16);
                }
                other => panic!(
                    "Did not get LE Read White List Size return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_channel_map() {
    let buffer = [
        0x0E, 11, 1, 0x15, 0x20, 0x00, 0x01, 0x02, 0x11, 0x11, 0x11, 0x11, 0x11,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeReadChannelMap(params) => {
                    assert_eq!(params.conn_handle, hci::ConnectionHandle(0x0201));
                    assert_eq!(
                        params.channel_map,
                        hci::ChannelClassification::CH_0
                            | hci::ChannelClassification::CH_4
                            | hci::ChannelClassification::CH_8
                            | hci::ChannelClassification::CH_12
                            | hci::ChannelClassification::CH_16
                            | hci::ChannelClassification::CH_20
                            | hci::ChannelClassification::CH_24
                            | hci::ChannelClassification::CH_28
                            | hci::ChannelClassification::CH_32
                            | hci::ChannelClassification::CH_36
                    );
                }
                other => panic!(
                    "Did not get LE Read White List Size return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_channel_map_failed_reserved() {
    let buffer = [
        0x0E, 11, 1, 0x15, 0x20, 0x00, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x20,
    ];
    match Event::new(Packet(&buffer)) {
        Err(Error::InvalidChannelMap(bytes)) => {
            assert_eq!(bytes, [0x00, 0x00, 0x00, 0x00, 0x20]);
        }
        other => panic!("Did not get invalid channel map: {:?}", other),
    }
}

#[test]
fn le_encrypt() {
    let buffer = [
        0x0E, 20, 1, 0x17, 0x20, 0x00, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeEncrypt(params) => {
                    assert_eq!(
                        params.encrypted_data.0,
                        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
                    );
                }
                other => panic!("Did not get LE Encrypt return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_rand() {
    let buffer = [
        0x0E, 12, 1, 0x18, 0x20, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeRand(params) => {
                    assert_eq!(params.random_number, 0x0807_0605_0403_0201);
                }
                other => panic!("Did not get LE Rand return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_long_term_key_request_reply() {
    let buffer = [0x0E, 6, 1, 0x1A, 0x20, 0x00, 0x01, 0x02];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeLongTermKeyRequestReply(params) => {
                    assert_eq!(params.conn_handle, hci::ConnectionHandle(0x0201));
                }
                other => panic!(
                    "Did not get LE LTK Request Reply return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_long_term_key_request_negative_reply() {
    let buffer = [0x0E, 6, 1, 0x1B, 0x20, 0x00, 0x01, 0x02];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeLongTermKeyRequestNegativeReply(params) => {
                    assert_eq!(params.conn_handle, hci::ConnectionHandle(0x0201));
                }
                other => panic!(
                    "Did not get LE LTK Request Negative Reply return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_supported_states() {
    let buffer = [
        0x0E, 12, 1, 0x1C, 0x20, 0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x02, 0x00, 0x00,
    ];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeReadSupportedStates(params) => {
                    assert_eq!(
                        params.supported_states,
                        LeStates::NON_CONNECTABLE_ADVERTISING
                            | LeStates::SCAN_AD_AND_PASS_SCAN
                            | LeStates::NONCONN_AD_AND_CENTRAL_CONN
                            | LeStates::ACT_SCAN_AND_PERIPH_CONN
                            | LeStates::DIR_AD_HDC_AND_CENTRAL_CONN
                            | LeStates::INITIATING_AND_PERIPH_CONN
                    );
                }
                other => panic!(
                    "Did not get LE Read Supported States return params: {:?}",
                    other
                ),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn le_read_supported_states_failed_reserved_flag() {
    let buffer = [
        0x0E, 12, 1, 0x1C, 0x20, 0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x04, 0x00, 0x00,
    ];
    match Event::new(Packet(&buffer)) {
        Err(Error::InvalidLeStates(bitfield)) => {
            assert_eq!(bitfield, 0x0000_0410_0804_0201);
        }
        other => panic!("Did not get bad LE State flags: {:?}", other),
    }
}

#[test]
fn le_test_end() {
    let buffer = [0x0E, 6, 1, 0x1F, 0x20, 0x00, 0x01, 0x02];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::LeTestEnd(params) => {
                    assert_eq!(params.number_of_packets, 0x0201);
                }
                other => panic!("Did not get LE Test End return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:?}", other),
    }
}

#[test]
fn vendor_command() {
    let buffer = [0x0E, 6, 1, 0x00, 0xFC, 0x00, 0x34, 0x12];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => {
            assert_eq!(event.num_hci_command_packets, 1);
            match event.return_params {
                ReturnParameters::Vendor(params) => match params {
                    VendorReturnParameters::HalGetFirmwareRevision(revision) => {
                        assert_eq!(revision.revision, 0x1234);
                    }
                    other => panic!(
                        "Did not get a Get Firmware Revision return params: {:?}",
                        other
                    ),
                },
                other => panic!("Did not get Vendor command return params: {:?}", other),
            }
        }
        other => panic!("Did not get command complete event: {:04X?}", other),
    }
}

#[test]
fn vendor_tx_test_packet_count_decodes_typed_payload() {
    let buffer = [0x0E, 8, 1, 0x14, 0xFC, 0x00, 0x78, 0x56, 0x34, 0x12];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => match event.return_params {
            ReturnParameters::Vendor(VendorReturnParameters::HalGetTxTestPacketCount(value)) => {
                assert_eq!(value.packet_count, 0x1234_5678);
            }
            other => panic!("Did not get a TX packet-count vendor response: {other:?}"),
        },
        other => panic!("Did not get a command complete event: {other:?}"),
    }
}

#[test]
fn vendor_pm_debug_info_decodes_typed_payload() {
    let buffer = [0x0E, 7, 1, 0x1C, 0xFC, 0x00, 0x11, 0x22, 0x33];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => match event.return_params {
            ReturnParameters::Vendor(VendorReturnParameters::HalGetPmDebugInfo(value)) => {
                assert_eq!((value.tx, value.rx, value.mblocks), (0x11, 0x22, 0x33));
            }
            other => panic!("Did not get a PM debug-info vendor response: {other:?}"),
        },
        other => panic!("Did not get a command complete event: {other:?}"),
    }
}

#[test]
fn vendor_rssi_uses_payload_after_status() {
    let buffer = [0x0E, 5, 1, 0x22, 0xFC, 0x00, 0xA5];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => match event.return_params {
            ReturnParameters::Vendor(VendorReturnParameters::HalReadRssi(value)) => {
                assert_eq!(value, 0xA5);
            }
            other => panic!("Did not get an RSSI vendor response: {other:?}"),
        },
        other => panic!("Did not get a command complete event: {other:?}"),
    }
}

#[test]
fn vendor_raw_rssi_preserves_all_three_response_bytes() {
    // Command Complete: one command credit, OGF 0x3f / OCF 0x032, status,
    // then the three-byte raw RSSI value from `aci_hal_read_raw_rssi_rp0`.
    let buffer = [0x0E, 7, 1, 0x32, 0xFC, 0x00, 0x11, 0x22, 0x33];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => match event.return_params {
            ReturnParameters::Vendor(VendorReturnParameters::HalReadRawRssi(value)) => {
                assert_eq!(value, [0x11, 0x22, 0x33]);
            }
            other => panic!("Did not get a raw RSSI vendor response: {other:?}"),
        },
        other => panic!("Did not get a command complete event: {other:?}"),
    }
}

#[test]
fn vendor_security_level_uses_mode_and_level_bytes() {
    // Command Complete: OGF 0x3f / OCF 0x090, status, Security_Mode, and
    // Security_Level as declared by `aci_gap_get_security_level_rp0`.
    let buffer = [0x0E, 6, 1, 0x90, 0xFC, 0x00, 0x01, 0x04];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => match event.return_params {
            ReturnParameters::Vendor(VendorReturnParameters::GapGetSecurityLevel(level)) => {
                assert_eq!(level.security_mode, 1);
                assert_eq!(level.security_level, 4);
            }
            other => panic!("Did not get a GAP security-level response: {other:?}"),
        },
        other => panic!("Did not get a command complete event: {other:?}"),
    }
}

#[test]
fn vendor_l2cap_coc_connect_confirm_preserves_created_channel_indices() {
    // Command Complete: OGF 0x3f / OCF 0x189, status, Channel_Number, then
    // one index for each created channel.
    let buffer = [0x0E, 7, 1, 0x89, 0xFD, 0x00, 2, 0x0A, 0x0B];
    match Event::new(Packet(&buffer)) {
        Ok(Event::CommandComplete(event)) => match event.return_params {
            ReturnParameters::Vendor(VendorReturnParameters::L2CapCocConnectConfirm(response)) => {
                assert_eq!(response.channel_number, 2);
                assert_eq!(response.channel_indices(), [0x0A, 0x0B]);
            }
            other => panic!("Did not get an L2CAP CoC Connect Confirm response: {other:?}"),
        },
        other => panic!("Did not get a command complete event: {other:?}"),
    }
}
