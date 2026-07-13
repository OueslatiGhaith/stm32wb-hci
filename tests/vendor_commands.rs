extern crate stm32wb_hci as hci;

mod vendor;

use bt_hci::cmd::{AsyncCmd, SyncCmd};
use hci::vendor::{
    command::{
        gap::{
            CmdGapInit, GapAddDevicesToList, GapAdditionalBeaconStart, GapAdvSetEnable,
            GapConfigureWhitelist, GapGetBondedDevices, GapPassKeyResponse,
            GapPeripheralSecurityRequest, GapSendPairingRequest, GapSetAuthenticationRequirement,
            GapSetBroadcastMode, GapSetDirectConnectable, GapSetDiscoverable, GapSetIoCapability,
            GapSetLimitedDiscoverable, GapSetNonConnectable, GapSetNonDiscoverable, GapSetOobData,
            GapSetUnidirectedConnectable, GapTerminate, GapTerminateProcedure,
            GapUpdateAdvertisingData, IoCapability, PassKey, PowerAmplifierOutputLevel, Role,
            TerminationReason,
        },
        gatt::{
            AccessPermission, CharacteristicEvent, CharacteristicPermission,
            CharacteristicProperty, DescriptorPermission, DescriptorValueMaxLength,
            EncryptionKeySize, GattAddCharacteristic, GattAddCharacteristicDescriptor,
            GattAddService, GattCharacteristic, GattCharacteristicDescriptor,
            GattDiscoverCharacteristicsByUUID, GattDiscoverPrimaryServicesByUUID,
            GattFindByTypeValueRequest, GattHandleValue, GattIncludeService,
            GattReadByGroupTypeRequest, GattReadByTypeRequest, GattReadCharacteristicUsingUUID,
            GattReadHandleValue, GattReadMultipleVarCharValue, GattService, ServiceType, Uuid,
        },
        hal::{
            HalEventFlags, HalFirmwareRevision, HalGetFirmwareRevision, HalGetLinkStatus,
            HalGetPmDebugInfo, HalGetTxTestPacketCount, HalPmDebugInfo, HalRadioRegisterValue,
            HalRawRssi, HalReadRadioReg, HalReadRawRssi, HalReadRssi, HalRssi, HalRxStart,
            HalSetEventMask, HalSetPeripheralLatency, HalSetRadioActivityMask, HalSetTxPowerLevel,
            HalStartTone, HalTxTestPacketCount, HalWriteRadioReg, PowerLevel, RadioActivityFlags,
            ToneChannel,
        },
        l2cap::{L2CocConnectConfirm, L2CocReconfig, L2CocTxData},
    },
    event::AttributeHandle,
};
use vendor::RecordingSink;

#[tokio::test]
async fn declarative_gap_discoverable_encodes_local_name_and_advertising_counts() {
    let sink = RecordingSink::new();
    GapSetDiscoverable::try_new(0, 0x20, 0x30, 0, 0, &[0x09, b'X'], &[0xAA, 0xBB], 0, 0)
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(
        sink.written_data(),
        [
            1, 0x83, 0xFC, 17, 0, 0x20, 0, 0x30, 0, 0, 0, 2, 0x09, b'X', 2, 0xAA, 0xBB, 0, 0, 0, 0,
        ]
    );
}

#[tokio::test]
async fn declarative_gap_discoverable_rejects_an_oversized_aggregate() {
    let name = [0; 242];
    let result = GapSetDiscoverable::try_new(0, 0, 0, 0, 0, &name, &[0], 0, 0);

    assert!(result.is_err());
}

#[test]
fn declarative_gap_and_hal_constraints_restore_legacy_guarantees() {
    let address = hci::types::BdAddrType::Public(hci::bt_hci::param::BdAddr([0; 6]));
    let pass_key = PassKey::try_new(999_999).unwrap();

    assert!(GapSetLimitedDiscoverable::try_new(0, 0x30, 0x20, 0, 0, &[], &[], 0, 0).is_err());
    assert!(GapSetDiscoverable::try_new(0x01, 0x20, 0x30, 0, 0, &[], &[], 0, 0).is_err());
    assert!(GapSetDirectConnectable::try_new(0, 0, address, 0x20, 0x30).is_err());
    assert!(GapSetDirectConnectable::try_new(0, 0x01, address, 0x001F, 0x0020).is_err());
    assert!(
        GapSetAuthenticationRequirement::try_new(false, false, 0, false, 16, 7, true, pass_key, 0,)
            .is_err()
    );
    assert!(PassKey::try_new(1_000_000).is_err());
    assert!(
        GapSetAuthenticationRequirement::try_new(false, false, 0, false, 7, 16, true, pass_key, 2,)
            .is_err()
    );
    let _ = GapPassKeyResponse::new(hci::bt_hci::param::ConnHandle(1), pass_key);
    assert!(GapSetNonConnectable::try_new(0, 0).is_err());
    assert!(GapSetUnidirectedConnectable::try_new(0x20, 0x30, 0, 1).is_err());
    let _ = GapTerminate::new(
        hci::bt_hci::param::ConnHandle(1),
        TerminationReason::AuthenticationFailure,
    );
    assert!(GapTerminateProcedure::try_new(0).is_err());
    assert!(GapSetBroadcastMode::try_new(0x20, 0x30, 0, 0, &[], &[]).is_err());
    assert!(GapSetBroadcastMode::try_new(0x30, 0x20, 2, 0, &[], &[]).is_err());
    assert!(GapSetBroadcastMode::try_new(0x1F, 0x20, 2, 0, &[], &[]).is_err());
    assert!(PowerAmplifierOutputLevel::try_new(0x24).is_err());
    let _ = GapAdditionalBeaconStart::new(
        0x20,
        0x30,
        7,
        address,
        PowerAmplifierOutputLevel::try_new(0x23).unwrap(),
    );
    assert!(ToneChannel::try_new(40).is_err());
}

#[tokio::test]
async fn declarative_gap_adv_set_enable_derives_and_validates_the_set_count() {
    use hci::types::extended_advertisement::AdvSet;

    let sink = RecordingSink::new();
    let sets = [AdvSet {
        handle: hci::bt_hci::param::AdvHandle(2),
        duration: 0x1234,
        max_extended_adv_events: 5,
    }];

    GapAdvSetEnable::try_new(true, &sets)
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();
    assert_eq!(
        sink.written_data(),
        [1, 0xC1, 0xFC, 6, 1, 1, 2, 0x34, 0x12, 5]
    );
}

#[tokio::test]
async fn declarative_gap_set_oob_data_includes_type_and_length() {
    let sink = RecordingSink::new();
    GapSetOobData::new(
        1,
        hci::types::BdAddrType::Public(hci::bt_hci::param::BdAddr([1, 2, 3, 4, 5, 6])),
        1,
        16,
        [0xAA; 16],
    )
    .exec(&sink)
    .await
    .unwrap();

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
    GapSendPairingRequest::new(hci::bt_hci::param::ConnHandle(0x1234), true)
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x9F, 0xFC, 3, 0x34, 0x12, 1]);
}

#[tokio::test]
async fn declarative_gap_add_devices_to_list_counts_complete_records() {
    let sink = RecordingSink::new();
    let entries = [hci::types::BdAddrType::Public(hci::bt_hci::param::BdAddr(
        [1, 2, 3, 4, 5, 6],
    ))];

    GapAddDevicesToList::try_new(&entries, 0x04)
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(
        sink.written_data(),
        [1, 0xAB, 0xFC, 9, 1, 0, 1, 2, 3, 4, 5, 6, 4]
    );
}

#[cfg(after_fw_0_17_1)]
use hci::vendor::command::gap::{ExtScanPhyParams, GapExtStartScan};

#[tokio::test]
async fn hal_get_link_status_uses_its_source_ocf() {
    let sink = RecordingSink::new();

    let _ = HalGetLinkStatus::new().exec(&sink).await;

    // OGF 0x3f / OCF 0x017, as used by aci_hal_get_link_status in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x17, 0xFC, 0]);
}

#[tokio::test]
async fn hal_set_peripheral_latency_uses_its_own_opcode() {
    let sink = RecordingSink::new();

    HalSetPeripheralLatency::new(true)
        .exec(&sink)
        .await
        .unwrap();

    // OGF 0x3f / OCF 0x020, as used by aci_hal_set_*_latency in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x20, 0xFC, 1, 1]);
}

#[tokio::test]
async fn hal_write_radio_reg_matches_cubewb() {
    let sink = RecordingSink::new();

    HalWriteRadioReg::new(0xAA, 0x55).exec(&sink).await.unwrap();

    // OGF 0x3f / OCF 0x031, as used by aci_hal_write_radio_reg in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x31, 0xFC, 2, 0xAA, 0x55]);
}

#[tokio::test]
async fn declarative_hal_read_radio_reg_matches_cubewb() {
    let sink = RecordingSink::new();

    let value = HalReadRadioReg::new(0xAA).exec(&sink).await.unwrap();

    assert_eq!(value.value, 0);
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

#[tokio::test]
async fn declarative_hal_fixed_return_commands_have_no_wire_parameters() {
    let sink = RecordingSink::new();

    assert_eq!(
        HalGetFirmwareRevision::new()
            .exec(&sink)
            .await
            .unwrap()
            .revision,
        0
    );
    assert_eq!(
        HalGetTxTestPacketCount::new()
            .exec(&sink)
            .await
            .unwrap()
            .packet_count,
        0
    );
    let debug = HalGetPmDebugInfo::new().exec(&sink).await.unwrap();
    assert_eq!((debug.tx, debug.rx, debug.mblocks), (0, 0, 0));
    assert_eq!(HalReadRssi::new().exec(&sink).await.unwrap().value, 0);
    assert_eq!(
        HalReadRawRssi::new().exec(&sink).await.unwrap().value,
        [0, 0, 0]
    );

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

    HalSetTxPowerLevel::new(false, PowerLevel::Plus3dBm)
        .exec(&sink)
        .await
        .unwrap();
    HalStartTone::new(ToneChannel::try_new(0x27).unwrap(), 0xAA)
        .exec(&sink)
        .await
        .unwrap();
    HalSetRadioActivityMask::new(RadioActivityFlags::IDLE | RadioActivityFlags::CENTRAL_CONN)
        .exec(&sink)
        .await
        .unwrap();
    HalSetEventMask::new(HalEventFlags::SCAN_REQ_REPORT)
        .exec(&sink)
        .await
        .unwrap();
    HalRxStart::new(0x27).exec(&sink).await.unwrap();

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
async fn gap_configure_whitelist_has_no_wire_parameters() {
    let sink = RecordingSink::new();

    GapConfigureWhitelist::new().exec(&sink).await.unwrap();

    // OGF 0x3f / OCF 0x092. CubeWB's generated wrapper takes `void`.
    assert_eq!(sink.written_data(), [1, 0x92, 0xFC, 0]);
}

#[tokio::test]
async fn declarative_gap_nondiscoverable_has_no_wire_parameters() {
    let sink = RecordingSink::new();

    GapSetNonDiscoverable::new().exec(&sink).await.unwrap();

    assert_eq!(sink.written_data(), [1, 0x81, 0xFC, 0]);
}

#[tokio::test]
async fn declarative_gap_io_capability_matches_cubewb() {
    let sink = RecordingSink::new();

    GapSetIoCapability::new(IoCapability::KeyboardDisplay)
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x85, 0xFC, 1, 0x04]);
}

#[tokio::test]
async fn declarative_gap_init_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = CmdGapInit::new(Role::PERIPHERAL | Role::CENTRAL, true, 0x20)
        .exec(&sink)
        .await;

    assert_eq!(sink.written_data(), [1, 0x8A, 0xFC, 3, 0x05, 0x01, 0x20]);
}

#[tokio::test]
async fn declarative_gap_command_status_matches_cubewb() {
    let sink = RecordingSink::new();

    GapPeripheralSecurityRequest::new(hci::bt_hci::param::ConnHandle(0x0123))
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x8D, 0xFC, 2, 0x23, 0x01]);
}

#[tokio::test]
async fn declarative_counted_bytes_write_only_the_used_payload() {
    let sink = RecordingSink::new();

    GapUpdateAdvertisingData::try_new(&[0xAA, 0xBB])
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x8E, 0xFC, 3, 0x02, 0xAA, 0xBB]);
}

#[test]
fn declarative_counted_bytes_reject_oversized_input() {
    let data = [0; 32];

    let result = GapUpdateAdvertisingData::try_new(&data);

    assert!(result.is_err());
}

#[test]
fn declarative_gap_init_decodes_payload_without_status_byte() {
    use bt_hci::FromHciBytes;
    use hci::vendor::command::gap::GapInit;

    let value = GapInit::from_hci_bytes_complete(&[0x34, 0x12, 0x78, 0x56, 0xBC, 0x9A])
        .expect("valid GAP Init return payload");

    assert_eq!(value.service_handle, AttributeHandle(0x1234));
    assert_eq!(value.dev_name_handle, AttributeHandle(0x5678));
    assert_eq!(value.appearance_handle, AttributeHandle(0x9ABC));
}

#[test]
fn declarative_bounded_return_decodes_counted_bytes() {
    use bt_hci::FromHciBytes;

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

    type Addresses = BoundedItems<hci::types::BdAddrType, 1>;
    assert!(matches!(
        decode_declarative_counted_items::<Addresses, hci::types::BdAddrType, u8, 1, 7, 1>(&[
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
            hci::types::BdAddrType::Public(hci::bt_hci::param::BdAddr([1, 2, 3, 4, 5, 6])),
            hci::types::BdAddrType::Random(hci::bt_hci::param::BdAddr([7, 8, 9, 10, 11, 12])),
        ]
    );
}

#[tokio::test]
async fn declarative_get_bonded_devices_has_no_request_parameters() {
    let sink = RecordingSink::new();

    let _ = GapGetBondedDevices::new().exec(&sink).await;

    assert_eq!(sink.written_data(), [1, 0xA3, 0xFC, 0]);
}

#[tokio::test]
async fn gatt_read_handle_value_matches_cubewb() {
    let sink = RecordingSink::new();

    let _ = GattReadHandleValue::new(AttributeHandle(0x0123), 0x4567, 0x89AB)
        .exec(&sink)
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

    GattReadMultipleVarCharValue::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        &[AttributeHandle(0x4567), AttributeHandle(0x89AB)],
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();

    // OGF 0x3f / OCF 0x132. The command is asynchronous (Command Status),
    // so the test controller's async path is the relevant one.
    assert_eq!(
        sink.written_data(),
        [1, 0x32, 0xFD, 7, 0x23, 0x01, 2, 0x67, 0x45, 0xAB, 0x89,]
    );
}

#[test]
fn declarative_counted_items_reject_oversized_input() {
    let handles = [AttributeHandle(0); 127];

    let result =
        GattReadMultipleVarCharValue::try_new(hci::bt_hci::param::ConnHandle(0x0123), &handles);

    assert!(result.is_err());
}

#[tokio::test]
async fn declarative_tagged_uuid16_matches_cubewb() {
    let sink = RecordingSink::new();

    let uuid = Uuid::Uuid16(0x4567);
    GattDiscoverPrimaryServicesByUUID::try_new(hci::bt_hci::param::ConnHandle(0x0123), &uuid)
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

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

    let uuid = Uuid::Uuid128(uuid);
    GattDiscoverPrimaryServicesByUUID::try_new(hci::bt_hci::param::ConnHandle(0x0123), &uuid)
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(
        sink.written_data(),
        [
            1, 0x13, 0xFD, 19, 0x23, 0x01, 0x02, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
            0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
        ]
    );
}

#[tokio::test]
async fn inline_uuid_shape_drives_characteristic_procedures() {
    let sink = RecordingSink::new();

    let uuid = Uuid::Uuid16(0xCDEF);
    GattDiscoverCharacteristicsByUUID::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        &uuid,
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();
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
    let uuid = Uuid::Uuid128(uuid);
    GattReadCharacteristicUsingUUID::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        &uuid,
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();
    assert_eq!(
        sink.written_data(),
        [
            1, 0x19, 0xFD, 23, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x02, 0x00, 0x11, 0x22, 0x33,
            0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
        ]
    );
}

#[tokio::test]
async fn inline_uuid_shape_drives_add_service() {
    let sink = RecordingSink::new();
    let uuid = Uuid::Uuid16(0x1234);
    let _ = GattAddService::try_new(&uuid, ServiceType::Primary as u8, 0x12)
        .unwrap()
        .exec(&sink)
        .await;

    assert_eq!(
        sink.written_data(),
        [1, 0x02, 0xFD, 5, 0x01, 0x34, 0x12, 0x01, 0x12]
    );
}

#[tokio::test]
async fn inline_uuid_shape_drives_include_service() {
    let sink = RecordingSink::new();
    let uuid = Uuid::Uuid16(0xCDEF);
    let _ = GattIncludeService::try_new(
        AttributeHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        &uuid,
    )
    .unwrap()
    .exec(&sink)
    .await;

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
    let uuid = Uuid::Uuid16(0x4567);
    let _ = GattAddCharacteristic::try_new(
        AttributeHandle(0x0123),
        &uuid,
        0x89AB,
        (CharacteristicProperty::READ | CharacteristicProperty::WRITE).bits(),
        CharacteristicPermission::ENCRYPTED_READ.bits(),
        CharacteristicEvent::CONFIRM_READ.bits(),
        EncryptionKeySize::with_value(16).unwrap(),
        true,
    )
    .unwrap()
    .exec(&sink)
    .await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x04, 0xFD, 12, 0x23, 0x01, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x0A, 0x04, 0x04, 0x10,
            0x01,
        ]
    );
}

#[tokio::test]
async fn inline_uuid_shape_and_counted_value_drive_add_descriptor() {
    let sink = RecordingSink::new();
    let uuid = Uuid::Uuid16(0x2902);
    let _ = GattAddCharacteristicDescriptor::try_new(
        AttributeHandle(0x0123),
        AttributeHandle(0x4567),
        &uuid,
        DescriptorValueMaxLength::try_new(3).unwrap(),
        &[0xAA, 0xBB],
        DescriptorPermission::ENCRYPTED.bits(),
        AccessPermission::READ_WRITE.bits(),
        CharacteristicEvent::ATTRIBUTE_WRITE.bits(),
        EncryptionKeySize::with_value(7).unwrap(),
        false,
    )
    .unwrap()
    .exec(&sink)
    .await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x05, 0xFD, 16, 0x23, 0x01, 0x67, 0x45, 0x01, 0x02, 0x29, 0x03, 0x02, 0xAA, 0xBB,
            0x04, 0x03, 0x01, 0x07, 0x00,
        ]
    );
}

#[tokio::test]
async fn inline_uuid_shape_drives_read_by_type_commands() {
    let uuid = Uuid::Uuid16(0xCDEF);

    let sink = RecordingSink::new();
    GattReadByTypeRequest::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        &uuid,
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();
    assert_eq!(
        sink.written_data(),
        [
            1, 0x0E, 0xFD, 9, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0x01, 0xEF, 0xCD,
        ]
    );

    let sink = RecordingSink::new();
    GattReadByGroupTypeRequest::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        &uuid,
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();
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
    GattFindByTypeValueRequest::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        0xCDEF,
        &[0xAA, 0xBB],
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();

    assert_eq!(
        sink.written_data(),
        [
            1, 0x0D, 0xFD, 11, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89, 0xEF, 0xCD, 0x02, 0xAA, 0xBB,
        ]
    );
}

#[test]
fn migrated_uuid_commands_reject_invalid_lengths_before_writing() {
    let oversized_value = [0; 247];
    let result = GattFindByTypeValueRequest::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        0xCDEF,
        &oversized_value,
    );
    assert!(result.is_err());

    let uuid = Uuid::Uuid16(0x2902);
    let oversized_descriptor = [0; 228];
    let result = GattAddCharacteristicDescriptor::try_new(
        AttributeHandle(0x0123),
        AttributeHandle(0x4567),
        &uuid,
        DescriptorValueMaxLength::try_new(227).unwrap(),
        &oversized_descriptor,
        DescriptorPermission::empty().bits(),
        AccessPermission::READ.bits(),
        CharacteristicEvent::empty().bits(),
        EncryptionKeySize::with_value(7).unwrap(),
        false,
    );
    assert!(result.is_err());

    let descriptor_value = [0; 4];
    let result = GattAddCharacteristicDescriptor::try_new(
        AttributeHandle(0x0123),
        AttributeHandle(0x4567),
        &uuid,
        DescriptorValueMaxLength::try_new(3).unwrap(),
        &descriptor_value,
        DescriptorPermission::empty().bits(),
        AccessPermission::READ.bits(),
        CharacteristicEvent::empty().bits(),
        EncryptionKeySize::with_value(7).unwrap(),
        false,
    );
    assert!(result.is_err());

    assert!(DescriptorValueMaxLength::try_new(228).is_err());
    assert!(EncryptionKeySize::with_value(6).is_err());
    assert!(EncryptionKeySize::with_value(17).is_err());
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

#[cfg(after_fw_0_17_1)]
#[tokio::test]
async fn declarative_bitmap_selected_phy_items_match_cubewb() {
    let sink = RecordingSink::new();
    let phy_params = [
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
    ];

    GapExtStartScan::try_new(1, 2, 3, 4, 0x1122, 0x3344, 5, 0x05, &phy_params)
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(
        sink.written_data(),
        [
            1, 0xD0, 0xFC, 20, 1, 2, 3, 4, 0x22, 0x11, 0x44, 0x33, 5, 0x05, 6, 0x66, 0x55, 0x88,
            0x77, 7, 0xAA, 0x99, 0xCC, 0xBB,
        ]
    );
}

#[cfg(after_fw_0_17_1)]
#[test]
fn declarative_bitmap_selected_phy_items_reject_mismatch() {
    let phy_params = [ExtScanPhyParams {
        scan_type: 0,
        scan_interval: 0,
        scan_window: 0,
    }];

    let Err(error) = GapExtStartScan::try_new(0, 0, 0, 0, 0, 0, 0, 0x05, &phy_params) else {
        panic!("mismatched PHY record count was not rejected");
    };
    assert_eq!(error.actual(), 1);
    assert_eq!(error.minimum(), 2);
    assert_eq!(error.maximum(), 2);
}

#[cfg(after_fw_0_17_1)]
#[test]
fn declarative_bitmap_selected_phy_items_reject_unknown_bits() {
    let Err(error) = GapExtStartScan::try_new(0, 0, 0, 0, 0, 0, 0, 0x02, &[]) else {
        panic!("unsupported PHY bit was not rejected");
    };
    assert_eq!(error.actual(), 0x02);
    assert_eq!(error.maximum(), 0x05);
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
    let _ = L2CocConnectConfirm::new(
        hci::bt_hci::param::ConnHandle(0x0123),
        0x4567,
        0x0089,
        0xABCD,
        0x0002,
    )
    .exec(&sink)
    .await;

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
    L2CocReconfig::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        0x4567,
        0x0089,
        &[0xAA, 0xBB],
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();

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
    L2CocTxData::try_new(3, &[0xAA, 0xBB])
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x8E, 0xFD, 5, 3, 2, 0, 0xAA, 0xBB]);
}

#[test]
fn declarative_l2cap_rejects_lengths_beyond_the_wire_bounds() {
    let reconfig = [0; 247];
    let tx = [0; 253];

    assert!(L2CocReconfig::try_new(hci::bt_hci::param::ConnHandle(0), 0, 0, &reconfig).is_err());
    assert!(L2CocTxData::try_new(0, &tx).is_err());
}
