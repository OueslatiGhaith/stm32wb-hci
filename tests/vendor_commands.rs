extern crate stm32wb_hci as hci;

mod vendor;

use hci::vendor::{
    command::{
        gap::{
            AddDeviceToListMode, AdvSetEnable, AdvertisingFilterPolicy, AdvertisingType,
            DiscoverableParameters, GapCommands, IoCapability, LocalName, OobDataType,
            OobDeviceType, OwnAddressType, PairingRequest, Role, SetOobDataParameters,
        },
        gatt::{
            AccessPermission, AddCharacteristicParameters, AddDescriptorParameters,
            AddServiceParameters, CharacteristicEvent, CharacteristicPermission,
            CharacteristicProperty, DescriptorPermission, EncryptionKeySize,
            FindByTypeValueParameters, GattCharacteristic, GattCharacteristicDescriptor,
            GattCommands, GattService, IncludeServiceParameters, ReadByTypeParameters, ServiceType,
            Uuid, Uuid16,
        },
        hal::{
            Error as HalError, HalCommands, HalEventFlags, HalFirmwareRevision, HalPmDebugInfo,
            HalRadioRegisterValue, HalRawRssi, HalRssi, HalTxTestPacketCount, PowerLevel,
            RadioActivityFlags,
        },
        l2cap::{
            Error as L2CapError, L2CapCocConnectConfirm, L2CapCocReconfig, L2CapCocTxData,
            L2capCommands,
        },
    },
    event::AttributeHandle,
};
use vendor::RecordingSink;

#[tokio::test]
async fn declarative_gap_discoverable_encodes_local_name_and_advertising_counts() {
    let sink = RecordingSink::new();
    let params = DiscoverableParameters {
        advertising_type: AdvertisingType::ConnectableUndirected,
        advertising_interval: Some((
            core::time::Duration::from_millis(20),
            core::time::Duration::from_millis(30),
        )),
        address_type: OwnAddressType::Public,
        filter_policy: AdvertisingFilterPolicy::AllowConnectionAndScan,
        local_name: Some(LocalName::Complete(b"X")),
        advertising_data: &[0xAA, 0xBB],
        conn_interval: (None, None),
    };

    let _ = sink.set_discoverable(&params).await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x83, 0xFC, 17, 0, 0x20, 0, 0x30, 0, 0, 0, 2, 0x09, b'X', 2, 0xAA, 0xBB, 0, 0, 0, 0,
        ]
    );
}

#[tokio::test]
async fn declarative_gap_discoverable_rejects_an_oversized_aggregate() {
    use hci::vendor::command::gap::Error;

    let sink = RecordingSink::new();
    let name = [0; 242];
    let params = DiscoverableParameters {
        advertising_type: AdvertisingType::ConnectableUndirected,
        advertising_interval: None,
        address_type: OwnAddressType::Public,
        filter_policy: AdvertisingFilterPolicy::AllowConnectionAndScan,
        local_name: Some(LocalName::Complete(&name)),
        advertising_data: &[],
        conn_interval: (None, None),
    };

    assert!(matches!(
        sink.set_discoverable(&params).await,
        Err(Error::IoError)
    ));
    assert!(sink.written_data().is_empty());
}

#[tokio::test]
async fn declarative_gap_adv_set_enable_derives_and_validates_the_set_count() {
    use hci::types::extended_advertisement::AdvSet;
    use hci::vendor::command::gap::Error;

    let sink = RecordingSink::new();
    let sets = [AdvSet {
        handle: hci::AdvertisingHandle(2),
        duration: 0x1234,
        max_extended_adv_events: 5,
    }];

    sink.adv_set_enable(&AdvSetEnable {
        enable: true,
        num_sets: 1,
        adv_set: &sets,
    })
    .await
    .unwrap();
    assert_eq!(
        sink.written_data(),
        [1, 0xC1, 0xFC, 6, 1, 1, 2, 0x34, 0x12, 5]
    );

    let mismatch = sink
        .adv_set_enable(&AdvSetEnable {
            enable: true,
            num_sets: 0,
            adv_set: &sets,
        })
        .await;
    assert!(matches!(mismatch, Err(Error::IoError)));
}

#[tokio::test]
async fn declarative_gap_set_oob_data_includes_type_and_length() {
    let sink = RecordingSink::new();
    let params = SetOobDataParameters {
        device_type: OobDeviceType::Remote,
        address: hci::BdAddrType::Public(hci::BdAddr([1, 2, 3, 4, 5, 6])),
        oob_data_type: OobDataType::Random,
        oob_data: [0xAA; 16],
    };

    let _ = sink.set_oob_data(&params).await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0xA8, 0xFC, 26, 1, 0, 1, 2, 3, 4, 5, 6, 1, 16, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
            0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        ]
    );
}

#[tokio::test]
async fn declarative_gap_pairing_request_includes_force_rebond() {
    let sink = RecordingSink::new();
    let params = PairingRequest {
        conn_handle: hci::ConnectionHandle(0x1234),
        force_rebond: true,
    };

    let _ = sink.send_pairing_request(&params).await;

    assert_eq!(sink.written_data(), [1, 0x9F, 0xFC, 3, 0x34, 0x12, 1]);
}

#[tokio::test]
async fn declarative_gap_add_devices_to_list_counts_complete_records() {
    let sink = RecordingSink::new();
    let entries = [hci::BdAddrType::Public(hci::BdAddr([1, 2, 3, 4, 5, 6]))];

    let _ = sink
        .add_devices_to_list(&entries, AddDeviceToListMode::AppendBoth)
        .await;

    assert_eq!(
        sink.written_data(),
        [1, 0xAB, 0xFC, 9, 1, 0, 1, 2, 3, 4, 5, 6, 4]
    );
}

#[cfg(after_fw_0_17_1)]
use hci::vendor::command::gap::{ExtScanPhyParams, ExtStartScanParams, GapExtStartScan};

#[tokio::test]
async fn hal_get_link_status_uses_its_source_ocf() {
    let sink = RecordingSink::new();

    let _ = sink.get_link_status().await;

    // OGF 0x3f / OCF 0x017, as used by aci_hal_get_link_status in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x17, 0xFC, 0]);
}

#[tokio::test]
async fn hal_set_peripheral_latency_uses_its_own_opcode() {
    let sink = RecordingSink::new();

    let _ = sink.set_peripheral_latency(true).await;

    // OGF 0x3f / OCF 0x020, as used by aci_hal_set_*_latency in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x20, 0xFC, 1, 1]);
}

#[tokio::test]
async fn hal_write_radio_reg_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = sink.write_radio_reg(0xAA, 0x55).await;

    // OGF 0x3f / OCF 0x031, as used by aci_hal_write_radio_reg in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x31, 0xFC, 2, 0xAA, 0x55]);
}

#[tokio::test]
async fn declarative_hal_read_radio_reg_matches_cubewb() {
    let sink = RecordingSink::new();

    let value = sink.read_radio_reg(0xAA).await.unwrap();

    assert_eq!(value, 0);
    assert_eq!(sink.written_data(), [1, 0x30, 0xFC, 1, 0xAA]);
}

#[test]
fn declarative_hal_radio_reg_decodes_payload_without_status_byte() {
    use bt_hci::FromHciBytes;

    let value = HalRadioRegisterValue::from_hci_bytes_complete(&[0x55]).unwrap();

    assert_eq!(value.value, 0x55);
}

#[test]
fn declarative_hal_fixed_returns_decode_without_status_byte() {
    use bt_hci::{FromHciBytes, FromHciBytesError};

    let revision = HalFirmwareRevision::from_hci_bytes_complete(&[0x34, 0x12]).unwrap();
    assert_eq!(revision.revision, 0x1234);

    let count = HalTxTestPacketCount::from_hci_bytes_complete(&[0x78, 0x56, 0x34, 0x12]).unwrap();
    assert_eq!(count.packet_count, 0x1234_5678);

    let debug = HalPmDebugInfo::from_hci_bytes_complete(&[0x11, 0x22, 0x33]).unwrap();
    assert_eq!((debug.tx, debug.rx, debug.mblocks), (0x11, 0x22, 0x33));

    let rssi = HalRssi::from_hci_bytes_complete(&[0xA5]).unwrap();
    assert_eq!(rssi.value, 0xA5);

    let raw = HalRawRssi::from_hci_bytes_complete(&[0x11, 0x22, 0x33]).unwrap();
    assert_eq!(raw.value, [0x11, 0x22, 0x33]);

    assert!(matches!(
        HalFirmwareRevision::from_hci_bytes_complete(&[0x34]),
        Err(FromHciBytesError::InvalidSize)
    ));
    assert!(matches!(
        HalRssi::from_hci_bytes_complete(&[0xA5, 0x00]),
        Err(FromHciBytesError::InvalidSize)
    ));
}

#[test]
fn migrated_hal_event_types_keep_status_prefixed_try_from() {
    use hci::vendor::event::command::{
        HalFirmwareRevision as EventFirmwareRevision, HalPmDebugInfo as EventPmDebugInfo,
        HalTxTestPacketCount as EventTxTestPacketCount,
    };

    let revision = EventFirmwareRevision::try_from(&[0x00, 0x34, 0x12][..]).unwrap();
    assert_eq!(revision.revision, 0x1234);

    let count = EventTxTestPacketCount::try_from(&[0x00, 0x78, 0x56, 0x34, 0x12][..]).unwrap();
    assert_eq!(count.packet_count, 0x1234_5678);

    let debug = EventPmDebugInfo::try_from(&[0x00, 0x11, 0x22, 0x33][..]).unwrap();
    assert_eq!((debug.tx, debug.rx, debug.mblocks), (0x11, 0x22, 0x33));

    assert!(EventFirmwareRevision::try_from(&[0x00, 0x34][..]).is_err());
    assert!(EventPmDebugInfo::try_from(&[0x00, 0x11, 0x22, 0x33, 0x44][..]).is_err());
}

#[tokio::test]
async fn declarative_hal_fixed_return_commands_have_no_wire_parameters() {
    let sink = RecordingSink::new();

    assert_eq!(sink.get_firmware_revision().await.unwrap(), 0);
    assert_eq!(
        sink.get_tx_test_packet_count().await.unwrap().packet_count,
        0
    );
    let debug = sink.get_pm_debug_info().await.unwrap();
    assert_eq!((debug.tx, debug.rx, debug.mblocks), (0, 0, 0));
    assert_eq!(sink.read_rssi().await.unwrap(), 0);
    assert_eq!(sink.read_raw_rssi().await.unwrap(), [0, 0, 0]);

    assert_eq!(
        sink.written_data(),
        [
            1, 0x00, 0xFC, 0, // firmware revision
            1, 0x14, 0xFC, 0, // TX test packet count
            1, 0x1C, 0xFC, 0, // PM debug info
            1, 0x22, 0xFC, 0, // RSSI
            1, 0x32, 0xFC, 0, // raw RSSI
        ]
    );
}

#[tokio::test]
async fn declarative_hal_fixed_setters_match_cubewb() {
    let sink = RecordingSink::new();

    sink.set_tx_power_level(PowerLevel::Plus3dBm).await.unwrap();
    sink.start_tone(0x27, 0xAA).await.unwrap();
    sink.set_radio_activity_mask(RadioActivityFlags::IDLE | RadioActivityFlags::CENTRAL_CONN)
        .await
        .unwrap();
    HalCommands::set_event_mask(&sink, HalEventFlags::SCAN_REQ_REPORT)
        .await
        .unwrap();
    sink.rx_start(0x27).await.unwrap();

    assert_eq!(
        sink.written_data(),
        [
            1, 0x0F, 0xFC, 2, 0x00, 0x1C, // set TX power
            1, 0x15, 0xFC, 2, 0x27, 0xAA, // start tone
            1, 0x18, 0xFC, 2, 0x21, 0x00, // radio activity mask
            1, 0x1A, 0xFC, 4, 0x01, 0x00, 0x00, 0x00, // HAL event mask
            1, 0x33, 0xFC, 1, 0x27, // RX start
        ]
    );
}

#[tokio::test]
async fn hal_start_tone_rejects_invalid_channel_before_writing() {
    let sink = RecordingSink::new();

    let result = sink.start_tone(40, 0).await;

    assert!(matches!(result, Err(HalError::InvalidChannel(40))));
    assert!(sink.written_data().is_empty());
}

#[tokio::test]
async fn gap_configure_whitelist_has_no_wire_parameters() {
    let sink = RecordingSink::new();

    let _ = sink.configure_white_list().await;

    // OGF 0x3f / OCF 0x092. CubeWB's generated wrapper takes `void`.
    assert_eq!(sink.written_data(), [1, 0x92, 0xFC, 0]);
}

#[tokio::test]
async fn declarative_gap_nondiscoverable_has_no_wire_parameters() {
    let sink = RecordingSink::new();

    let _ = sink.gap_set_nondiscoverable().await;

    assert_eq!(sink.written_data(), [1, 0x81, 0xFC, 0]);
}

#[tokio::test]
async fn declarative_gap_io_capability_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = sink.set_io_capability(IoCapability::KeyboardDisplay).await;

    assert_eq!(sink.written_data(), [1, 0x85, 0xFC, 1, 0x04]);
}

#[tokio::test]
async fn declarative_gap_init_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = GapCommands::init(&sink, Role::PERIPHERAL | Role::CENTRAL, true, 0x20).await;

    assert_eq!(sink.written_data(), [1, 0x8A, 0xFC, 3, 0x05, 0x01, 0x20]);
}

#[tokio::test]
async fn declarative_gap_command_status_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = sink
        .peripheral_security_request(&hci::ConnectionHandle(0x0123))
        .await;

    assert_eq!(sink.written_data(), [1, 0x8D, 0xFC, 2, 0x23, 0x01]);
}

#[tokio::test]
async fn declarative_counted_bytes_write_only_the_used_payload() {
    let sink = RecordingSink::new();

    let _ = sink.update_advertising_data(&[0xAA, 0xBB]).await;

    assert_eq!(sink.written_data(), [1, 0x8E, 0xFC, 3, 0x02, 0xAA, 0xBB]);
}

#[tokio::test]
async fn declarative_counted_bytes_reject_oversized_input() {
    let sink = RecordingSink::new();
    let data = [0; 32];

    let result = sink.update_advertising_data(&data).await;

    assert!(matches!(
        result,
        Err(hci::vendor::command::gap::Error::BadAdvertisingDataLength(
            32
        ))
    ));
    assert!(sink.written_data().is_empty());
}

#[test]
fn declarative_gap_init_decodes_payload_without_status_byte() {
    use bt_hci::FromHciBytes;
    use hci::vendor::event::command::GapInit;

    let value = GapInit::from_hci_bytes_complete(&[0x34, 0x12, 0x78, 0x56, 0xBC, 0x9A])
        .expect("valid GAP Init return payload");

    assert_eq!(value.service_handle, AttributeHandle(0x1234));
    assert_eq!(value.dev_name_handle, AttributeHandle(0x5678));
    assert_eq!(value.appearance_handle, AttributeHandle(0x9ABC));
}

#[test]
fn declarative_bounded_return_decodes_counted_bytes() {
    use bt_hci::FromHciBytes;
    use hci::vendor::event::command::GattHandleValue;

    let value = GattHandleValue::from_hci_bytes_complete(&[
        0x34, 0x12, // total attribute length
        0x03, 0x00, // returned value length
        0xAA, 0xBB, 0xCC,
    ])
    .expect("valid GATT handle-value return payload");

    assert_eq!(value.total_length, 0x1234);
    assert_eq!(value.value(), [0xAA, 0xBB, 0xCC]);
}

#[test]
fn declarative_bounded_return_rejects_invalid_counts() {
    use bt_hci::{FromHciBytes, FromHciBytesError};
    use hci::vendor::event::command::GattHandleValue;

    let oversized = [0, 0, 250, 0];
    assert!(matches!(
        GattHandleValue::from_hci_bytes_complete(&oversized),
        Err(FromHciBytesError::InvalidValue)
    ));

    let truncated = [0, 0, 3, 0, 0xAA, 0xBB];
    assert!(matches!(
        GattHandleValue::from_hci_bytes_complete(&truncated),
        Err(FromHciBytesError::InvalidSize)
    ));
}

#[test]
fn declarative_bounded_items_decode_records_and_preserve_trailing_bytes() {
    use hci::vendor::command::{BoundedItems, decode_declarative_counted_items};

    let (items, rest) = decode_declarative_counted_items::<
        BoundedItems<AttributeHandle, 3>,
        AttributeHandle,
        u8,
        1,
        2,
        3,
    >(&[2, 0x34, 0x12, 0x78, 0x56, 0xAA])
    .unwrap();

    assert_eq!(
        items.as_slice(),
        [AttributeHandle(0x1234), AttributeHandle(0x5678)]
    );
    assert_eq!(rest, [0xAA]);
}

#[test]
fn declarative_trailing_bytes_consume_the_remaining_payload() {
    use hci::vendor::command::{BoundedBytes, decode_declarative_trailing_bytes};

    let (bytes, rest) =
        decode_declarative_trailing_bytes::<BoundedBytes<4>, 1, 4>(&[0xAA, 0xBB]).unwrap();

    assert_eq!(bytes.as_slice(), [0xAA, 0xBB]);
    assert!(rest.is_empty());
}

#[test]
fn declarative_trailing_bytes_enforce_the_declared_range() {
    use bt_hci::FromHciBytesError;
    use hci::vendor::command::{BoundedBytes, decode_declarative_trailing_bytes};

    type Bytes = BoundedBytes<4>;
    assert!(matches!(
        decode_declarative_trailing_bytes::<Bytes, 1, 4>(&[]),
        Err(FromHciBytesError::InvalidSize)
    ));
    assert!(matches!(
        decode_declarative_trailing_bytes::<Bytes, 1, 4>(&[0; 5]),
        Err(FromHciBytesError::InvalidSize)
    ));
}

#[test]
fn declarative_bounded_items_reject_excessive_truncated_and_invalid_records() {
    use bt_hci::FromHciBytesError;
    use hci::vendor::command::{BoundedItems, decode_declarative_counted_items};

    type Handles = BoundedItems<AttributeHandle, 3>;
    assert!(matches!(
        decode_declarative_counted_items::<Handles, AttributeHandle, u8, 1, 2, 3>(&[4]),
        Err(FromHciBytesError::InvalidValue)
    ));
    assert!(matches!(
        decode_declarative_counted_items::<Handles, AttributeHandle, u8, 1, 2, 3>(
            &[2, 0x34, 0x12,]
        ),
        Err(FromHciBytesError::InvalidSize)
    ));

    type Addresses = BoundedItems<hci::BdAddrType, 1>;
    assert!(matches!(
        decode_declarative_counted_items::<Addresses, hci::BdAddrType, u8, 1, 7, 1>(&[
            1, 0x02, 0, 0, 0, 0, 0, 0,
        ]),
        Err(FromHciBytesError::InvalidValue)
    ));
}

#[test]
fn declarative_bonded_devices_payload_decodes_counted_addresses() {
    use bt_hci::FromHciBytes;
    use hci::vendor::command::gap::GapBondedDevices;

    let devices = GapBondedDevices::from_hci_bytes_complete(&[
        2, // count
        0, 1, 2, 3, 4, 5, 6, // public
        1, 7, 8, 9, 10, 11, 12, // random
    ])
    .unwrap();

    assert_eq!(
        devices.bonded_addresses(),
        [
            hci::BdAddrType::Public(hci::BdAddr([1, 2, 3, 4, 5, 6])),
            hci::BdAddrType::Random(hci::BdAddr([7, 8, 9, 10, 11, 12])),
        ]
    );

    let event_devices =
        hci::vendor::event::command::GapBondedDevices::try_from(&[0, 1, 0, 1, 2, 3, 4, 5, 6][..])
            .unwrap();
    assert_eq!(
        event_devices.bonded_addresses(),
        [hci::BdAddrType::Public(hci::BdAddr([1, 2, 3, 4, 5, 6]))]
    );
}

#[tokio::test]
async fn declarative_get_bonded_devices_has_no_request_parameters() {
    let sink = RecordingSink::new();

    let _ = sink.get_bonded_devices().await;

    assert_eq!(sink.written_data(), [1, 0xA3, 0xFC, 0]);
}

#[tokio::test]
async fn gatt_read_handle_value_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = sink
        .read_handle_value(AttributeHandle(0x0123), 0x4567, 0x89AB)
        .await;

    // OGF 0x3f / OCF 0x12a, as used by aci_gatt_read_handle_value in CubeWB.
    assert_eq!(
        sink.written_data(),
        [1, 0x2A, 0xFD, 6, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89]
    );
}

#[tokio::test]
async fn gatt_read_multiple_variable_value_uses_command_status_envelope() {
    let sink = RecordingSink::new();

    let _ = sink
        .read_multiple_variable_characteristic_value(
            hci::ConnectionHandle(0x0123),
            &[AttributeHandle(0x4567), AttributeHandle(0x89AB)],
        )
        .await;

    // OGF 0x3f / OCF 0x132. The command is asynchronous (Command Status),
    // so the test controller's async path is the relevant one.
    assert_eq!(
        sink.written_data(),
        [1, 0x32, 0xFD, 7, 0x23, 0x01, 2, 0x67, 0x45, 0xAB, 0x89,]
    );
}

#[tokio::test]
async fn declarative_counted_items_reject_oversized_input() {
    let sink = RecordingSink::new();
    let handles = [AttributeHandle(0); 127];

    let result = sink
        .read_multiple_variable_characteristic_value(hci::ConnectionHandle(0x0123), &handles)
        .await;

    assert!(matches!(
        result,
        Err(hci::vendor::command::gatt::Error::TooManyHandlesToRead)
    ));
    assert!(sink.written_data().is_empty());
}

#[tokio::test]
async fn declarative_tagged_uuid16_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = sink
        .discover_primary_services_by_uuid(hci::ConnectionHandle(0x0123), Uuid::Uuid16(0x4567))
        .await;

    assert_eq!(
        sink.written_data(),
        [1, 0x13, 0xFD, 5, 0x23, 0x01, 0x01, 0x67, 0x45]
    );
}

#[tokio::test]
async fn declarative_tagged_uuid128_matches_cubewb() {
    let sink = RecordingSink::new();
    let uuid = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xFF,
    ];

    let _ = sink
        .discover_primary_services_by_uuid(hci::ConnectionHandle(0x0123), Uuid::Uuid128(uuid))
        .await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x13, 0xFD, 19, 0x23, 0x01, 0x02, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
            0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
        ]
    );
}

#[tokio::test]
async fn reusable_uuid_payload_drives_characteristic_procedures() {
    let sink = RecordingSink::new();

    let _ = sink
        .discover_characteristics_by_uuid(
            hci::ConnectionHandle(0x0123),
            AttributeHandle(0x4567)..AttributeHandle(0x89AB),
            Uuid::Uuid16(0xCDEF),
        )
        .await;
    assert_eq!(
        sink.written_data(),
        [
            1, 0x16, 0xFD, 9, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x01, 0xEF, 0xCD,
        ]
    );

    let sink = RecordingSink::new();
    let uuid = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xFF,
    ];
    let _ = sink
        .read_characteristic_using_uuid(
            hci::ConnectionHandle(0x0123),
            AttributeHandle(0x4567)..AttributeHandle(0x89AB),
            Uuid::Uuid128(uuid),
        )
        .await;
    assert_eq!(
        sink.written_data(),
        [
            1, 0x19, 0xFD, 23, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x02, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
        ]
    );
}

#[tokio::test]
async fn reusable_uuid_payload_drives_add_service() {
    let sink = RecordingSink::new();
    let params = AddServiceParameters {
        uuid: Uuid::Uuid16(0x1234),
        service_type: ServiceType::Primary,
        max_attribute_records: 0x12,
    };

    let _ = sink.add_service(&params).await;

    assert_eq!(
        sink.written_data(),
        [1, 0x02, 0xFD, 5, 0x01, 0x34, 0x12, 0x01, 0x12]
    );
}

#[tokio::test]
async fn reusable_uuid_payload_drives_include_service() {
    let sink = RecordingSink::new();
    let params = IncludeServiceParameters {
        service_handle: AttributeHandle(0x0123),
        include_handle_range: AttributeHandle(0x4567)..AttributeHandle(0x89AB),
        include_uuid: Uuid::Uuid16(0xCDEF),
    };

    let _ = sink.include_service(&params).await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x03, 0xFD, 9, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x01, 0xEF, 0xCD,
        ]
    );
}

#[tokio::test]
async fn declarative_add_characteristic_includes_is_variable_byte() {
    let sink = RecordingSink::new();
    let params = AddCharacteristicParameters {
        service_handle: AttributeHandle(0x0123),
        characteristic_uuid: Uuid::Uuid16(0x4567),
        characteristic_value_len: 0x89AB,
        characteristic_properties: CharacteristicProperty::READ | CharacteristicProperty::WRITE,
        security_permissions: CharacteristicPermission::ENCRYPTED_READ,
        gatt_event_mask: CharacteristicEvent::CONFIRM_READ,
        encryption_key_size: EncryptionKeySize::with_value(16).unwrap(),
        is_variable: true,
    };

    let _ = sink.add_characteristic(&params).await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x04, 0xFD, 12, 0x23, 0x01, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x0A, 0x04, 0x04, 0x10,
            0x01,
        ]
    );
}

#[tokio::test]
async fn reusable_uuid_payload_and_counted_value_drive_add_descriptor() {
    let sink = RecordingSink::new();
    let params = AddDescriptorParameters {
        service_handle: AttributeHandle(0x0123),
        characteristic_handle: AttributeHandle(0x4567),
        descriptor_uuid: Uuid::Uuid16(0x2902),
        descriptor_value_max_len: 3,
        descriptor_value: &[0xAA, 0xBB],
        security_permissions: DescriptorPermission::ENCRYPTED,
        access_permissions: AccessPermission::READ_WRITE,
        gatt_event_mask: CharacteristicEvent::ATTRIBUTE_WRITE,
        encryption_key_size: EncryptionKeySize::with_value(7).unwrap(),
        is_variable: false,
    };

    let _ = sink.add_characteristic_descriptor(&params).await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x05, 0xFD, 16, 0x23, 0x01, 0x67, 0x45, 0x01, 0x02, 0x29, 0x03, 0x02, 0xAA, 0xBB,
            0x04, 0x03, 0x01, 0x07, 0x00,
        ]
    );
}

#[tokio::test]
async fn reusable_uuid_payload_drives_read_by_type_commands() {
    let params = ReadByTypeParameters {
        conn_handle: hci::ConnectionHandle(0x0123),
        attribute_handle_range: AttributeHandle(0x4567)..AttributeHandle(0x89AB),
        uuid: Uuid::Uuid16(0xCDEF),
    };

    let sink = RecordingSink::new();
    let _ = sink.read_by_type_request(&params).await;
    assert_eq!(
        sink.written_data(),
        [
            1, 0x0E, 0xFD, 9, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x01, 0xEF, 0xCD,
        ]
    );

    let sink = RecordingSink::new();
    let _ = sink.read_by_group_type_request(&params).await;
    assert_eq!(
        sink.written_data(),
        [
            1, 0x0F, 0xFD, 9, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x01, 0xEF, 0xCD,
        ]
    );
}

#[tokio::test]
async fn declarative_find_by_type_value_uses_raw_uuid16_and_counted_value() {
    let sink = RecordingSink::new();
    let params = FindByTypeValueParameters {
        conn_handle: hci::ConnectionHandle(0x0123),
        attribute_handle_range: AttributeHandle(0x4567)..AttributeHandle(0x89AB),
        uuid: Uuid16(0xCDEF),
        value: &[0xAA, 0xBB],
    };

    let _ = sink.find_by_type_value_request(&params).await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x0D, 0xFD, 11, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0xEF, 0xCD, 0x02, 0xAA, 0xBB,
        ]
    );
}

#[tokio::test]
async fn migrated_uuid_commands_reject_invalid_lengths_before_writing() {
    let sink = RecordingSink::new();
    let oversized_value = [0; 247];
    let find = FindByTypeValueParameters {
        conn_handle: hci::ConnectionHandle(0x0123),
        attribute_handle_range: AttributeHandle(0x4567)..AttributeHandle(0x89AB),
        uuid: Uuid16(0xCDEF),
        value: &oversized_value,
    };
    let result = sink.find_by_type_value_request(&find).await;
    assert!(matches!(
        result,
        Err(hci::vendor::command::gatt::Error::ValueBufferTooLong)
    ));
    assert!(sink.written_data().is_empty());

    let sink = RecordingSink::new();
    let descriptor = AddDescriptorParameters {
        service_handle: AttributeHandle(0x0123),
        characteristic_handle: AttributeHandle(0x4567),
        descriptor_uuid: Uuid::Uuid16(0x2902),
        descriptor_value_max_len: 1,
        descriptor_value: &[0xAA, 0xBB],
        security_permissions: DescriptorPermission::empty(),
        access_permissions: AccessPermission::READ,
        gatt_event_mask: CharacteristicEvent::empty(),
        encryption_key_size: EncryptionKeySize::with_value(7).unwrap(),
        is_variable: false,
    };
    let result = sink.add_characteristic_descriptor(&descriptor).await;
    assert!(matches!(
        result,
        Err(hci::vendor::command::gatt::Error::DescriptorTooLong)
    ));
    assert!(sink.written_data().is_empty());
}

#[test]
fn declarative_uuid_payload_decodes_all_variants() {
    use hci::vendor::command::{HciDecodePayload, HciEncodePayload, decode_declarative_payload};

    assert_eq!(<Uuid as HciEncodePayload>::MIN_LEN, 3);
    assert_eq!(<Uuid as HciEncodePayload>::MAX_LEN, 17);

    let (uuid16, rest) = Uuid::from_hci_payload(&[0x01, 0x67, 0x45, 0xAA]).unwrap();
    assert_eq!(uuid16, Uuid::Uuid16(0x4567));
    assert_eq!(rest, [0xAA]);

    let bytes = [
        0x02, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
        0xEE, 0xFF,
    ];
    let (uuid128, rest) = Uuid::from_hci_payload(&bytes).unwrap();
    assert_eq!(
        uuid128,
        Uuid::Uuid128([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ])
    );
    assert!(rest.is_empty());

    let (return_field, rest) =
        decode_declarative_payload::<Uuid, 3, 17>(&[0x01, 0xCD, 0xAB, 0xFE]).unwrap();
    assert_eq!(return_field, Uuid::Uuid16(0xABCD));
    assert_eq!(rest, [0xFE]);
}

#[test]
fn declarative_uuid_payload_rejects_unknown_and_truncated_variants() {
    use bt_hci::FromHciBytesError;
    use hci::vendor::command::HciDecodePayload;

    assert!(matches!(
        Uuid::from_hci_payload(&[0x03]),
        Err(FromHciBytesError::InvalidValue)
    ));
    assert!(matches!(
        Uuid::from_hci_payload(&[0x01, 0xAA]),
        Err(FromHciBytesError::InvalidSize)
    ));
}

#[test]
fn reusable_fixed_gatt_returns_decode_without_transport_status() {
    use bt_hci::FromHciBytes;

    let service = GattService::from_hci_bytes_complete(&[0x34, 0x12]).unwrap();
    assert_eq!(service.service_handle, AttributeHandle(0x1234));

    let characteristic = GattCharacteristic::from_hci_bytes_complete(&[0x78, 0x56]).unwrap();
    assert_eq!(
        characteristic.characteristic_handle,
        AttributeHandle(0x5678)
    );

    let descriptor = GattCharacteristicDescriptor::from_hci_bytes_complete(&[0xBC, 0x9A]).unwrap();
    assert_eq!(descriptor.descriptor_handle, AttributeHandle(0x9ABC));
}

#[test]
fn fixed_gatt_return_event_reexports_keep_status_aware_try_from() {
    use hci::vendor::event::command::GattService as EventGattService;

    let service = EventGattService::try_from(&[0x00, 0x34, 0x12][..]).unwrap();
    assert_eq!(service.service_handle, AttributeHandle(0x1234));
}

#[cfg(after_fw_0_17_1)]
#[tokio::test]
async fn declarative_bitmap_selected_phy_items_match_cubewb() {
    let sink = RecordingSink::new();
    let params = ExtStartScanParams {
        scan_mode: 1,
        procedure: 2,
        own_address_type: 3,
        filter_duplicates: 4,
        duration: 0x1122,
        period: 0x3344,
        scanning_filter_policy: 5,
        scanning_phys: 0x05,
        phy_params: [
            ExtScanPhyParams {
                scan_type: 6,
                scan_interval: 0x5566,
                scan_window: 0x7788,
            },
            ExtScanPhyParams {
                scan_type: 7,
                scan_interval: 0x99AA,
                scan_window: 0xBBCC,
            },
        ],
        num_phys: 2,
    };

    let _ = sink.ext_start_scan(&params).await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0xD0, 0xFC, 20, 1, 2, 3, 4, 0x22, 0x11, 0x44, 0x33, 5, 0x05, 6, 0x66, 0x55, 0x88,
            0x77, 7, 0xAA, 0x99, 0xCC, 0xBB,
        ]
    );
}

#[cfg(after_fw_0_17_1)]
#[tokio::test]
async fn declarative_bitmap_selected_phy_items_reject_mismatch() {
    let sink = RecordingSink::new();
    let params = ExtStartScanParams {
        scan_mode: 0,
        procedure: 0,
        own_address_type: 0,
        filter_duplicates: 0,
        duration: 0,
        period: 0,
        scanning_filter_policy: 0,
        scanning_phys: 0x05,
        phy_params: [
            ExtScanPhyParams {
                scan_type: 0,
                scan_interval: 0,
                scan_window: 0,
            },
            ExtScanPhyParams {
                scan_type: 0,
                scan_interval: 0,
                scan_window: 0,
            },
        ],
        num_phys: 1,
    };

    let result = sink.ext_start_scan(&params).await;

    let Err(hci::vendor::command::gap::Error::BadExtendedScanParameters(error)) = result else {
        panic!("mismatched PHY record count was not rejected");
    };
    assert_eq!(error.actual(), 1);
    assert_eq!(error.minimum(), 2);
    assert_eq!(error.maximum(), 2);
    assert!(sink.written_data().is_empty());
}

#[cfg(after_fw_0_17_1)]
#[tokio::test]
async fn declarative_bitmap_selected_phy_items_reject_unknown_bits() {
    let sink = RecordingSink::new();
    let params = ExtStartScanParams {
        scan_mode: 0,
        procedure: 0,
        own_address_type: 0,
        filter_duplicates: 0,
        duration: 0,
        period: 0,
        scanning_filter_policy: 0,
        scanning_phys: 0x02,
        phy_params: [
            ExtScanPhyParams {
                scan_type: 0,
                scan_interval: 0,
                scan_window: 0,
            },
            ExtScanPhyParams {
                scan_type: 0,
                scan_interval: 0,
                scan_window: 0,
            },
        ],
        num_phys: 0,
    };

    let result = sink.ext_start_scan(&params).await;

    let Err(hci::vendor::command::gap::Error::BadExtendedScanParameters(error)) = result else {
        panic!("unsupported PHY bit was not rejected");
    };
    assert_eq!(error.actual(), 0x02);
    assert_eq!(error.maximum(), 0x05);
    assert!(sink.written_data().is_empty());
}

#[cfg(after_fw_0_17_1)]
#[test]
fn bitmap_schema_constructor_enforces_sparse_mask_and_cardinality() {
    let phy = [ExtScanPhyParams {
        scan_type: 0,
        scan_interval: 0,
        scan_window: 0,
    }];

    let unknown_bit = match GapExtStartScan::try_new(0, 0, 0, 0, 0, 0, 0, 0x02, &phy[..0]) {
        Ok(_) => panic!("unsupported PHY bit was accepted"),
        Err(error) => error,
    };
    assert_eq!(unknown_bit.actual(), 0x02);
    assert_eq!(unknown_bit.maximum(), 0x05);

    let wrong_count = match GapExtStartScan::try_new(0, 0, 0, 0, 0, 0, 0, 0x05, &phy) {
        Ok(_) => panic!("mismatched PHY record count was accepted"),
        Err(error) => error,
    };
    assert_eq!(wrong_count.actual(), 1);
    assert_eq!(wrong_count.minimum(), 2);
    assert_eq!(wrong_count.maximum(), 2);
}

#[tokio::test]
async fn l2cap_coc_connect_confirm_uses_only_its_five_cubewb_inputs() {
    let sink = RecordingSink::new();
    let params = L2CapCocConnectConfirm {
        conn_handle: hci::ConnectionHandle(0x0123),
        mtu: 0x4567,
        mps: 0x0089,
        initial_credits: 0xABCD,
        result: 0x0002,
        channel_number: 0,
        channel_index_list: [0; 246],
    };

    let _ = sink.coc_connect_confirm(&params).await;

    // OGF 0x3f / OCF 0x189. The generated wrapper takes five 16-bit input
    // fields; its Channel_Number and Channel_Index_List are response values.
    assert_eq!(
        sink.written_data(),
        [
            1, 0x89, 0xFD, 10, 0x23, 0x01, 0x67, 0x45, 0x89, 0x00, 0xCD, 0xAB, 0x02, 0x00,
        ]
    );
}

#[tokio::test]
async fn declarative_l2cap_reconfig_writes_only_the_declared_channel_indices() {
    let sink = RecordingSink::new();
    let mut channel_index_list = [0; 246];
    channel_index_list[..2].copy_from_slice(&[0xAA, 0xBB]);
    let params = L2CapCocReconfig {
        conn_handle: hci::ConnectionHandle(0x0123),
        mtu: 0x4567,
        mps: 0x0089,
        channel_number: 2,
        channel_index_list,
    };

    sink.coc_reconfig(&params).await.unwrap();

    assert_eq!(
        sink.written_data(),
        [
            1, 0x8A, 0xFD, 9, 0x23, 0x01, 0x67, 0x45, 0x89, 0x00, 2, 0xAA, 0xBB,
        ]
    );
}

#[tokio::test]
async fn declarative_l2cap_tx_data_writes_only_the_declared_data() {
    let sink = RecordingSink::new();
    let mut data = [0; 252];
    data[..2].copy_from_slice(&[0xAA, 0xBB]);
    let params = L2CapCocTxData {
        channel_index: 3,
        length: 2,
        data,
    };

    sink.coc_tx_data(&params).await.unwrap();

    assert_eq!(sink.written_data(), [1, 0x8E, 0xFD, 5, 3, 2, 0, 0xAA, 0xBB]);
}

#[tokio::test]
async fn declarative_l2cap_rejects_lengths_beyond_the_public_backing_arrays() {
    let sink = RecordingSink::new();
    let reconfig = L2CapCocReconfig {
        conn_handle: hci::ConnectionHandle(0),
        mtu: 0,
        mps: 0,
        channel_number: 247,
        channel_index_list: [0; 246],
    };
    let tx = L2CapCocTxData {
        channel_index: 0,
        length: 253,
        data: [0; 252],
    };

    assert!(matches!(
        sink.coc_reconfig(&reconfig).await,
        Err(L2CapError::InvalidChannelCount(247))
    ));
    assert!(matches!(
        sink.coc_tx_data(&tx).await,
        Err(L2CapError::InvalidDataLength(253))
    ));
    assert!(sink.written_data().is_empty());
}
