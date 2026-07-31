use stm32wb_hci as hci;

mod vendor;

use bt_hci::cmd::{AsyncCmd, SyncCmd};
use hci::types::{AdvertisingFilterPolicy, AdvertisingType, AttributeHandle};
#[cfg(since_fw_1_18_0)]
use hci::vendor::command::gap::Procedure;
#[cfg(since_fw_1_24_0)]
use hci::vendor::command::gatt::GattPermitWrite as GattWriteResponse;
#[cfg(before_fw_1_24_0)]
use hci::vendor::command::gatt::GattWriteResponse;
#[cfg(before_fw_1_23_0)]
use hci::vendor::command::hal::{HalFirmwareRevision, HalGetFirmwareRevision};
use hci::vendor::command::{gap::EventFlags, gatt::Event as GattEventFlags};
use hci::vendor::command::{
    gap::{
        AddDeviceToListMode, AddressType, AdvertisingChannelMap, AdvertisingHandle, AdvertisingSid,
        CmdGapInit, GapAddDevicesToList, GapAdditionalBeaconSetData, GapAdvSetConfig,
        GapAdvSetEnable, GapInit, GapPeripheralSecurityRequest, GapSendPairingRequest,
        GapSetDiscoverable, GapSetEventMask, GapSetIoCapability, GapSetOobData,
        GapUpdateAdvertisingData, IoCapability, OobDataLength, OobDataType, OobDeviceType,
        OptionalAdvertisingIntervalBound, PrivacyMode, Role, ScanType, ScanningFilterPolicy,
        SecondaryAdvertisingMaximumSkip,
    },
    gatt::{
        AccessPermission, CharacteristicEvent, CharacteristicPermission, CharacteristicProperty,
        DescriptorPermission, DescriptorValueMaxLength, EncryptionKeySize, GattAddCharacteristic,
        GattAddCharacteristicDescriptor, GattAddService, GattAttributeOffset,
        GattAttributeRecordCapacity, GattAttributeValueLength, GattDiscoverCharacteristicsByUUID,
        GattDiscoverPrimaryServicesByUUID, GattFindByTypeValueRequest, GattHandleValue,
        GattIncludeService, GattNotificationTarget, GattReadByGroupTypeRequest,
        GattReadByTypeRequest, GattReadCharacteristicUsingUUID, GattReadHandleValue,
        GattReadMultipleVarCharValue, GattRequestedValueLength, GattSetEventMask,
        GattUpdateLongCharacteristicValue, GattUuid16, ServiceType, UpdateType, Uuid, WriteStatus,
    },
    hal::{
        HalEventFlags, HalRadioRegisterValue, HalReadRadioReg, HalRxStart, HalSetEventMask,
        HalSetPeripheralLatency, HalSetRadioActivityMask, HalSetTxPowerLevel, HalStartTone,
        HalWriteRadioReg, PowerLevel, RadioActivityFlags, RadioRegisterAddress, RadioRegisterValue,
        ToneChannel, ToneFrequencyOffset,
    },
    l2cap::{
        L2CapCocConnectConfirmWire, L2CocChannelIndex, L2CocConnectConfirm, L2CocConnectionResult,
        L2CocCreditIncrement, L2CocInitialCredits, L2CocMaximumChannelCount, L2CocMps, L2CocMtu,
        L2CocReconfig, L2CocReconfigurationResult, L2CocRequestedChannelCount, L2CocSpsm,
        L2CocTxData,
    },
};
use vendor::RecordingSink;

#[test]
fn declarative_gap_discoverable_encodes_local_name_and_advertising_counts() {
    pollster::block_on(
        declarative_gap_discoverable_encodes_local_name_and_advertising_counts_async(),
    );
}
async fn declarative_gap_discoverable_encodes_local_name_and_advertising_counts_async() {
    let sink = RecordingSink::new();
    GapSetDiscoverable::try_new(
        AdvertisingType::ConnectableUndirected,
        OptionalAdvertisingIntervalBound::try_new(0x20).unwrap(),
        OptionalAdvertisingIntervalBound::try_new(0x30).unwrap(),
        AddressType::Public,
        AdvertisingFilterPolicy::AllowConnectionAndScan,
        &[0x09, b'X'],
        &[0xAA, 0xBB],
        0,
        0,
    )
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

#[test]
fn declarative_gap_discoverable_rejects_an_oversized_aggregate() {
    let name = [0; 242];
    let result = GapSetDiscoverable::try_new(
        AdvertisingType::ConnectableUndirected,
        OptionalAdvertisingIntervalBound::CONTROLLER_SELECTED,
        OptionalAdvertisingIntervalBound::CONTROLLER_SELECTED,
        AddressType::Public,
        AdvertisingFilterPolicy::AllowConnectionAndScan,
        &name,
        &[0],
        0,
        0,
    );

    assert!(result.is_err());
}

#[test]
fn semantic_scan_domains_encode_their_declared_values() {
    pollster::block_on(semantic_scan_domains_encode_their_declared_values_async());
}
async fn semantic_scan_domains_encode_their_declared_values_async() {
    use core::time::Duration;
    use hci::vendor::command::gap::GapStartGeneralConnectionEstablishmentProcedure;

    let scan_window = hci::types::ScanWindow::start_every(Duration::from_millis(10))
        .unwrap()
        .open_for(Duration::from_millis(5))
        .unwrap();
    let sink = RecordingSink::new();

    GapStartGeneralConnectionEstablishmentProcedure::new(
        ScanType::Active,
        scan_window,
        ScanningFilterPolicy::ExtendedFiltered,
        AddressType::ResolvablePrivate,
        true,
    )
    .exec(&sink)
    .await
    .unwrap();

    assert_eq!(
        sink.written_data(),
        [1, 0x9A, 0xFC, 8, 1, 0x10, 0, 0x08, 0, 3, 2, 1]
    );
    assert!(
        <ScanType as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[0x02]).is_err()
    );
    assert!(
        <ScanningFilterPolicy as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[0x04])
            .is_err()
    );
}

#[cfg(since_fw_1_18_0)]
#[test]
fn extended_scan_requires_at_least_one_declared_phy() {
    use core::time::Duration;
    use hci::vendor::command::gap::{
        ExtScanMode, ExtScanPhyParams, ExtendedDuplicateFiltering, ExtendedScanDuration,
        ExtendedScanPeriod, GapExtStartScan, ScanningPhy,
    };

    let phy_params = || ExtScanPhyParams {
        scan_type: ScanType::Passive,
        scan_window: hci::types::ScanWindow::start_every(Duration::from_millis(10))
            .unwrap()
            .open_for(Duration::from_millis(5))
            .unwrap(),
    };

    assert!(ScanningPhy::from_bits(0x02).is_none());
    assert!(
        GapExtStartScan::try_new(
            ExtScanMode::Default,
            Procedure::OBSERVATION,
            AddressType::Public,
            ExtendedDuplicateFiltering::Disabled,
            ExtendedScanDuration::new(0),
            ExtendedScanPeriod::new(0),
            ScanningFilterPolicy::BasicUnfiltered,
            ScanningPhy::empty(),
            phy_params(),
            phy_params(),
        )
        .is_err()
    );
}

#[test]
fn gatt_write_response_uses_a_closed_write_status() {
    pollster::block_on(gatt_write_response_uses_a_closed_write_status_async());
}
async fn gatt_write_response_uses_a_closed_write_status_async() {
    let sink = RecordingSink::new();

    GattWriteResponse::try_new(
        hci::bt_hci::param::ConnHandle(0x1234),
        AttributeHandle(0x5678),
        WriteStatus::Rejected,
        0x08,
        &[0xAA],
    )
    .unwrap()
    .exec(&sink)
    .await
    .unwrap();

    assert_eq!(
        sink.written_data(),
        [1, 0x26, 0xFD, 8, 0x34, 0x12, 0x78, 0x56, 1, 0x08, 1, 0xAA,]
    );
    assert!(
        <WriteStatus as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[0x02]).is_err()
    );
    assert!(
        GattWriteResponse::try_new(
            hci::bt_hci::param::ConnHandle(1),
            AttributeHandle(2),
            WriteStatus::Allowed,
            1,
            &[],
        )
        .is_err()
    );
    assert!(
        GattWriteResponse::try_new(
            hci::bt_hci::param::ConnHandle(1),
            AttributeHandle(2),
            WriteStatus::Rejected,
            0,
            &[],
        )
        .is_err()
    );
    GattWriteResponse::try_new(
        hci::bt_hci::param::ConnHandle(1),
        AttributeHandle(2),
        WriteStatus::Allowed,
        0,
        &[],
    )
    .unwrap();
}

#[test]
fn additional_beacon_data_includes_its_cubewb_length_prefix() {
    pollster::block_on(additional_beacon_data_includes_its_cubewb_length_prefix_async());
}
async fn additional_beacon_data_includes_its_cubewb_length_prefix_async() {
    let sink = RecordingSink::new();
    GapAdditionalBeaconSetData::try_new(&[0xAA, 0xBB])
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0xB2, 0xFC, 3, 2, 0xAA, 0xBB]);

    let too_large = [0; 255];
    assert!(GapAdditionalBeaconSetData::try_new(&too_large).is_err());
}

#[test]
fn semantic_event_masks_cover_all_cubewb_defined_bits() {
    pollster::block_on(semantic_event_masks_cover_all_cubewb_defined_bits_async());
}
async fn semantic_event_masks_cover_all_cubewb_defined_bits_async() {
    let gap_sink = RecordingSink::new();
    GapSetEventMask::new(
        EventFlags::PROCEDURE_COMPLETE
            | EventFlags::L2CAP_CONNECTION_UPDATE_REQUEST
            | EventFlags::L2CAP_CONNECTION_UPDATE_RESPONSE
            | EventFlags::L2CAP_PROCEDURE_TIMEOUT
            | EventFlags::ADDRESS_NOT_RESOLVED,
    )
    .exec(&gap_sink)
    .await
    .unwrap();
    assert_eq!(gap_sink.written_data(), [1, 0x91, 0xFC, 2, 0x80, 0x0F]);

    let gatt_sink = RecordingSink::new();
    GattSetEventMask::new(
        GattEventFlags::READ_EXT
            | GattEventFlags::INDICATION_EXT
            | GattEventFlags::NOTIFICATION_EXT,
    )
    .exec(&gatt_sink)
    .await
    .unwrap();
    assert_eq!(
        gatt_sink.written_data(),
        [1, 0x0A, 0xFD, 4, 0x00, 0x00, 0x70, 0x00]
    );
}

#[test]
fn extended_advertising_configuration_enforces_signed_power_and_sid() {
    pollster::block_on(extended_advertising_configuration_enforces_signed_power_and_sid_async());
}
async fn extended_advertising_configuration_enforces_signed_power_and_sid_async() {
    use core::time::Duration;
    use hci::types::extended_advertisement::{
        AdvertisingEvent, AdvertisingMode, AdvertisingPhy, ExtendedAdvertisingInterval,
    };

    let interval = ExtendedAdvertisingInterval::with_range(
        Duration::from_millis(20),
        Duration::from_millis(30),
    )
    .unwrap();
    let address = hci::types::BdAddrType::Public(hci::bt_hci::param::BdAddr([0; 6]));
    let make = |power, sid| {
        GapAdvSetConfig::try_new(
            AdvertisingMode::empty(),
            AdvertisingHandle::try_new(0).unwrap(),
            AdvertisingEvent::CONNECTABLE,
            &interval,
            AdvertisingChannelMap::CHANNEL_37,
            AddressType::Public,
            address,
            AdvertisingFilterPolicy::AllowConnectionAndScan,
            power,
            SecondaryAdvertisingMaximumSkip::new(0),
            AdvertisingPhy::Le1M,
            sid,
            false,
        )
    };

    let sid_zero = AdvertisingSid::try_new(0).unwrap();
    assert!(make(-128, sid_zero).is_err());
    assert!(make(21, sid_zero).is_err());
    assert!(make(127, sid_zero).is_ok());
    assert!(AdvertisingSid::try_new(16).is_err());

    let sink = RecordingSink::new();
    make(-127, AdvertisingSid::try_new(15).unwrap())
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();
    assert_eq!(sink.written_data()[26], 0x81);
}

#[test]
fn declarative_gap_adv_set_enable_derives_and_validates_the_set_count() {
    pollster::block_on(declarative_gap_adv_set_enable_derives_and_validates_the_set_count_async());
}
async fn declarative_gap_adv_set_enable_derives_and_validates_the_set_count_async() {
    use hci::types::extended_advertisement::AdvSet;

    let handle = AdvertisingHandle::try_new(0xEF).unwrap();
    assert_eq!(handle.value(), 0xEF);
    assert_eq!(u8::from(handle), 0xEF);
    let error = AdvertisingHandle::try_new(0xF0).unwrap_err();
    assert_eq!(error.actual(), 0xF0);
    assert_eq!(error.minimum(), 0x00);
    assert_eq!(error.maximum(), 0xEF);
    assert!(
        <AdvertisingHandle as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[0xEF])
            .is_ok()
    );
    assert!(
        <AdvertisingHandle as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[0xF0])
            .is_err()
    );

    let sink = RecordingSink::new();
    let sets = [AdvSet {
        handle,
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
        [1, 0xC1, 0xFC, 6, 1, 1, 0xEF, 0x34, 0x12, 5]
    );
}

#[test]
fn declarative_gap_set_oob_data_includes_type_and_length() {
    pollster::block_on(declarative_gap_set_oob_data_includes_type_and_length_async());
}
async fn declarative_gap_set_oob_data_includes_type_and_length_async() {
    let sink = RecordingSink::new();
    GapSetOobData::new(
        OobDeviceType::Remote,
        hci::types::BdAddrType::Public(hci::bt_hci::param::BdAddr([1, 2, 3, 4, 5, 6])),
        OobDataType::Random,
        OobDataLength::Present,
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

#[test]
fn declarative_gap_pairing_request_includes_force_rebond() {
    pollster::block_on(declarative_gap_pairing_request_includes_force_rebond_async());
}
async fn declarative_gap_pairing_request_includes_force_rebond_async() {
    let sink = RecordingSink::new();
    GapSendPairingRequest::new(hci::bt_hci::param::ConnHandle(0x1234), true)
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x9F, 0xFC, 3, 0x34, 0x12, 1]);
}

#[test]
fn declarative_gap_add_devices_to_list_counts_complete_records() {
    pollster::block_on(declarative_gap_add_devices_to_list_counts_complete_records_async());
}
async fn declarative_gap_add_devices_to_list_counts_complete_records_async() {
    let sink = RecordingSink::new();
    let entries = [hci::types::BdAddrType::Public(hci::bt_hci::param::BdAddr(
        [1, 2, 3, 4, 5, 6],
    ))];

    GapAddDevicesToList::try_new(&entries, AddDeviceToListMode::AppendBoth)
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(
        sink.written_data(),
        [1, 0xAB, 0xFC, 9, 1, 0, 1, 2, 3, 4, 5, 6, 4]
    );
}

#[test]
fn hal_set_peripheral_latency_uses_its_own_opcode() {
    pollster::block_on(hal_set_peripheral_latency_uses_its_own_opcode_async());
}
async fn hal_set_peripheral_latency_uses_its_own_opcode_async() {
    let sink = RecordingSink::new();

    HalSetPeripheralLatency::new(true)
        .exec(&sink)
        .await
        .unwrap();

    // OGF 0x3f / OCF 0x020, as used by aci_hal_set_*_latency in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x20, 0xFC, 1, 1]);
}

#[test]
fn hal_write_radio_reg_matches_cubewb() {
    pollster::block_on(hal_write_radio_reg_matches_cubewb_async());
}
async fn hal_write_radio_reg_matches_cubewb_async() {
    let sink = RecordingSink::new();

    HalWriteRadioReg::new(
        RadioRegisterAddress::new(0xAA),
        RadioRegisterValue::new(0x55),
    )
    .exec(&sink)
    .await
    .unwrap();

    // OGF 0x3f / OCF 0x031, as used by aci_hal_write_radio_reg in CubeWB.
    assert_eq!(sink.written_data(), [1, 0x31, 0xFC, 2, 0xAA, 0x55]);
}

#[test]
fn declarative_hal_read_radio_reg_matches_cubewb() {
    pollster::block_on(declarative_hal_read_radio_reg_matches_cubewb_async());
}
async fn declarative_hal_read_radio_reg_matches_cubewb_async() {
    let sink = RecordingSink::new();

    let value = HalReadRadioReg::new(RadioRegisterAddress::new(0xAA))
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(value.value, RadioRegisterValue::new(0));
    assert_eq!(sink.written_data(), [1, 0x30, 0xFC, 1, 0xAA]);
}

#[test]
fn declarative_hal_radio_reg_decodes_payload_without_status_byte() {
    use bt_hci::FromHciBytes;

    let value = HalRadioRegisterValue::from_hci_bytes_complete(&[0x55]).unwrap();

    assert_eq!(value.value, RadioRegisterValue::new(0x55));
}

#[cfg(before_fw_1_23_0)]
#[test]
fn hal_firmware_revision_decodes_without_status_byte() {
    use bt_hci::{FromHciBytes, FromHciBytesError};

    let revision = HalFirmwareRevision::from_hci_bytes_complete(&[0x34, 0x12]).unwrap();
    assert_eq!(revision.revision, 0x1234);

    assert!(matches!(
        HalFirmwareRevision::from_hci_bytes_complete(&[0x34]),
        Err(FromHciBytesError::InvalidSize)
    ));
}

#[cfg(before_fw_1_23_0)]
#[test]
fn hal_firmware_revision_has_no_wire_parameters() {
    pollster::block_on(hal_firmware_revision_has_no_wire_parameters_async());
}
#[cfg(before_fw_1_23_0)]
async fn hal_firmware_revision_has_no_wire_parameters_async() {
    let sink = RecordingSink::new();

    assert_eq!(
        HalGetFirmwareRevision::new()
            .exec(&sink)
            .await
            .unwrap()
            .revision,
        0
    );
    assert_eq!(sink.written_data(), [1, 0x00, 0xFC, 0]);
}

#[test]
fn declarative_hal_fixed_setters_match_cubewb() {
    pollster::block_on(declarative_hal_fixed_setters_match_cubewb_async());
}
async fn declarative_hal_fixed_setters_match_cubewb_async() {
    let sink = RecordingSink::new();

    HalSetTxPowerLevel::new(false, PowerLevel::Plus3dBm)
        .exec(&sink)
        .await
        .unwrap();
    HalStartTone::new(
        ToneChannel::try_new(0x27).unwrap(),
        ToneFrequencyOffset::new(0xAA),
    )
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
    HalRxStart::new(ToneChannel::try_new(0x27).unwrap())
        .exec(&sink)
        .await
        .unwrap();

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

#[test]
fn gap_io_capability_encodes_its_declared_value() {
    pollster::block_on(gap_io_capability_encodes_its_declared_value_async());
}
async fn gap_io_capability_encodes_its_declared_value_async() {
    let sink = RecordingSink::new();
    GapSetIoCapability::new(IoCapability::KeyboardDisplay)
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x85, 0xFC, 1, 0x04]);
}

#[test]
fn declarative_gap_init_matches_cubewb() {
    pollster::block_on(declarative_gap_init_matches_cubewb_async());
}
async fn declarative_gap_init_matches_cubewb_async() {
    fn assert_sync_contract<C>()
    where
        C: SyncCmd<Return = GapInit, ReturnBuf = [u8; 6]>,
    {
    }

    assert_sync_contract::<CmdGapInit>();

    let sink = RecordingSink::new();

    let _ = CmdGapInit::try_new(Role::PERIPHERAL | Role::CENTRAL, PrivacyMode::Enabled, 0x20)
        .unwrap()
        .exec(&sink)
        .await;

    assert_eq!(sink.written_data(), [1, 0x8A, 0xFC, 3, 0x05, 0x02, 0x20]);
}

#[test]
fn gap_init_rejects_empty_roles_and_boolean_style_privacy_encoding() {
    assert!(CmdGapInit::try_new(Role::empty(), PrivacyMode::Disabled, 8).is_err());
    assert!(
        <PrivacyMode as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[0x01]).is_err()
    );
}

#[test]
fn declarative_gap_command_status_matches_cubewb() {
    pollster::block_on(declarative_gap_command_status_matches_cubewb_async());
}
async fn declarative_gap_command_status_matches_cubewb_async() {
    fn assert_async_contract<C: AsyncCmd>() {}

    assert_async_contract::<GapPeripheralSecurityRequest>();

    let sink = RecordingSink::new();

    GapPeripheralSecurityRequest::new(hci::bt_hci::param::ConnHandle(0x0123))
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x8D, 0xFC, 2, 0x23, 0x01]);
}

#[test]
fn declarative_counted_bytes_write_only_the_used_payload() {
    pollster::block_on(declarative_counted_bytes_write_only_the_used_payload_async());
}
async fn declarative_counted_bytes_write_only_the_used_payload_async() {
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

    let value = GapInit::from_hci_bytes_complete(&[0x34, 0x12, 0x78, 0x56, 0xBC, 0x9A])
        .expect("valid GAP Init return payload");

    assert_eq!(value.service_handle, AttributeHandle(0x1234));
    assert_eq!(value.dev_name_handle, AttributeHandle(0x5678));
    assert_eq!(value.appearance_handle, AttributeHandle(0x9ABC));
}

#[test]
fn declarative_bounded_return_decodes_counted_bytes() {
    use bt_hci::FromHciBytes;

    assert_eq!(GattHandleValue::MAX_VALUE_LEN, 247);
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

    let mut maximum = [0; 251];
    maximum[0] = 247;
    maximum[2] = 247;
    assert!(GattHandleValue::from_hci_bytes_complete(&maximum).is_ok());

    let oversized = [0, 0, 248, 0];
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
        0,
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
        decode_declarative_counted_items::<Handles, AttributeHandle, u8, 1, 2, 0, 3>(&[4]),
        Err(FromHciBytesError::InvalidValue)
    ));
    assert!(matches!(
        decode_declarative_counted_items::<Handles, AttributeHandle, u8, 1, 2, 0, 3>(&[
            2, 0x34, 0x12,
        ]),
        Err(FromHciBytesError::InvalidSize)
    ));

    type Addresses = BoundedItems<hci::types::BdAddrType, 1>;
    assert!(matches!(
        decode_declarative_counted_items::<Addresses, hci::types::BdAddrType, u8, 1, 7, 0, 1>(&[
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

#[test]
fn gatt_read_handle_value_matches_cubewb() {
    pollster::block_on(gatt_read_handle_value_matches_cubewb_async());
}
async fn gatt_read_handle_value_matches_cubewb_async() {
    let sink = RecordingSink::new();

    let _ = GattReadHandleValue::new(
        AttributeHandle(0x0123),
        GattAttributeOffset::new(0x4567),
        GattRequestedValueLength::new(0x89AB),
    )
    .exec(&sink)
    .await;

    // OGF 0x3f / OCF 0x12a, as used by aci_gatt_read_handle_value in CubeWB.
    assert_eq!(
        sink.written_data(),
        [1, 0x2A, 0xFD, 6, 0x23, 0x01, 0x67, 0x45, 0xAB, 0x89]
    );
}

#[test]
fn gatt_read_multiple_variable_value_uses_command_status_envelope() {
    pollster::block_on(gatt_read_multiple_variable_value_uses_command_status_envelope_async());
}
async fn gatt_read_multiple_variable_value_uses_command_status_envelope_async() {
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

#[test]
fn declarative_tagged_uuid16_matches_cubewb() {
    pollster::block_on(declarative_tagged_uuid16_matches_cubewb_async());
}
async fn declarative_tagged_uuid16_matches_cubewb_async() {
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

#[test]
fn declarative_tagged_uuid128_matches_cubewb() {
    pollster::block_on(declarative_tagged_uuid128_matches_cubewb_async());
}
async fn declarative_tagged_uuid128_matches_cubewb_async() {
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

#[test]
fn inline_uuid_shape_drives_characteristic_procedures() {
    pollster::block_on(inline_uuid_shape_drives_characteristic_procedures_async());
}
async fn inline_uuid_shape_drives_characteristic_procedures_async() {
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

#[test]
fn inline_uuid_shape_drives_add_service() {
    pollster::block_on(inline_uuid_shape_drives_add_service_async());
}
async fn inline_uuid_shape_drives_add_service_async() {
    let sink = RecordingSink::new();
    let uuid = Uuid::Uuid16(0x1234);
    let _ = GattAddService::try_new(
        &uuid,
        ServiceType::Primary,
        GattAttributeRecordCapacity::new(0x12),
    )
    .unwrap()
    .exec(&sink)
    .await;

    assert_eq!(
        sink.written_data(),
        [1, 0x02, 0xFD, 5, 0x01, 0x34, 0x12, 0x01, 0x12]
    );
}

#[test]
fn inline_uuid_shape_drives_include_service() {
    pollster::block_on(inline_uuid_shape_drives_include_service_async());
}
async fn inline_uuid_shape_drives_include_service_async() {
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

#[test]
fn declarative_add_characteristic_includes_is_variable_byte() {
    pollster::block_on(declarative_add_characteristic_includes_is_variable_byte_async());
}
async fn declarative_add_characteristic_includes_is_variable_byte_async() {
    let sink = RecordingSink::new();
    let uuid = Uuid::Uuid16(0x4567);
    let _ = GattAddCharacteristic::try_new(
        AttributeHandle(0x0123),
        &uuid,
        GattAttributeValueLength::try_new(0x01AB).unwrap(),
        CharacteristicProperty::READ | CharacteristicProperty::WRITE,
        CharacteristicPermission::ENCRYPTED_READ,
        CharacteristicEvent::CONFIRM_READ,
        EncryptionKeySize::try_new(16).unwrap(),
        true,
    )
    .unwrap()
    .exec(&sink)
    .await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x04, 0xFD, 12, 0x23, 0x01, 0x01, 0x67, 0x45, 0xAB, 0x01, 0x0A, 0x04, 0x04, 0x10,
            0x01,
        ]
    );
}

#[test]
fn inline_uuid_shape_and_counted_value_drive_add_descriptor() {
    pollster::block_on(inline_uuid_shape_and_counted_value_drive_add_descriptor_async());
}
async fn inline_uuid_shape_and_counted_value_drive_add_descriptor_async() {
    let sink = RecordingSink::new();
    let uuid = Uuid::Uuid16(0x2902);
    let _ = GattAddCharacteristicDescriptor::try_new(
        AttributeHandle(0x0123),
        AttributeHandle(0x4567),
        &uuid,
        DescriptorValueMaxLength::try_new(3).unwrap(),
        &[0xAA, 0xBB],
        DescriptorPermission::ENCRYPTED,
        AccessPermission::READ_WRITE,
        CharacteristicEvent::ATTRIBUTE_WRITE,
        EncryptionKeySize::try_new(7).unwrap(),
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

#[test]
fn inline_uuid_shape_drives_read_by_type_commands() {
    pollster::block_on(inline_uuid_shape_drives_read_by_type_commands_async());
}
async fn inline_uuid_shape_drives_read_by_type_commands_async() {
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

#[test]
fn declarative_find_by_type_value_uses_raw_uuid16_and_counted_value() {
    pollster::block_on(declarative_find_by_type_value_uses_raw_uuid16_and_counted_value_async());
}
async fn declarative_find_by_type_value_uses_raw_uuid16_and_counted_value_async() {
    let sink = RecordingSink::new();
    GattFindByTypeValueRequest::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        AttributeHandle(0x4567),
        AttributeHandle(0x89AB),
        GattUuid16::new(0xCDEF),
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
        GattUuid16::new(0xCDEF),
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
        DescriptorPermission::empty(),
        AccessPermission::READ,
        CharacteristicEvent::empty(),
        EncryptionKeySize::try_new(7).unwrap(),
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
        DescriptorPermission::empty(),
        AccessPermission::READ,
        CharacteristicEvent::empty(),
        EncryptionKeySize::try_new(7).unwrap(),
        false,
    );
    assert!(result.is_err());

    assert!(DescriptorValueMaxLength::try_new(228).is_err());
    let minimum_key_size = EncryptionKeySize::try_new(7).unwrap();
    assert_eq!(minimum_key_size.value(), 7u8);
    assert_eq!(u8::from(minimum_key_size), 7);
    assert!(EncryptionKeySize::try_new(6).is_err());
    assert!(EncryptionKeySize::try_new(17).is_err());
    assert!(
        <EncryptionKeySize as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[6])
            .is_err()
    );
}

#[cfg(before_fw_1_23_0)]
#[test]
fn l2cap_coc_connect_confirm_uses_only_its_five_cubewb_inputs() {
    pollster::block_on(l2cap_coc_connect_confirm_uses_only_its_five_cubewb_inputs_async());
}
#[cfg(before_fw_1_23_0)]
async fn l2cap_coc_connect_confirm_uses_only_its_five_cubewb_inputs_async() {
    let sink = RecordingSink::new();
    let _ = L2CocConnectConfirm::new(
        hci::bt_hci::param::ConnHandle(0x0123),
        L2CocMtu::try_new(0x4567).unwrap(),
        L2CocMps::try_new(0x0089).unwrap(),
        L2CocInitialCredits::new(0xABCD),
        L2CocConnectionResult::try_new(0x0002).unwrap(),
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

#[cfg(since_fw_1_23_0)]
#[test]
fn l2cap_coc_connect_confirm_includes_maximum_channel_count() {
    pollster::block_on(l2cap_coc_connect_confirm_includes_maximum_channel_count_async());
}
#[cfg(since_fw_1_23_0)]
async fn l2cap_coc_connect_confirm_includes_maximum_channel_count_async() {
    let sink = RecordingSink::new();
    let _ = L2CocConnectConfirm::new(
        hci::bt_hci::param::ConnHandle(0x0123),
        L2CocMtu::try_new(0x4567).unwrap(),
        L2CocMps::try_new(0x0089).unwrap(),
        L2CocInitialCredits::new(0xABCD),
        L2CocConnectionResult::try_new(0x0002).unwrap(),
        L2CocMaximumChannelCount::try_new(5).unwrap(),
    )
    .exec(&sink)
    .await;

    assert_eq!(
        sink.written_data(),
        [
            1, 0x89, 0xFD, 11, 0x23, 0x01, 0x67, 0x45, 0x89, 0x00, 0xCD, 0xAB, 0x02, 0x00, 5,
        ]
    );
}

#[test]
fn declarative_l2cap_reconfig_writes_only_the_declared_channel_indices() {
    pollster::block_on(declarative_l2cap_reconfig_writes_only_the_declared_channel_indices_async());
}
async fn declarative_l2cap_reconfig_writes_only_the_declared_channel_indices_async() {
    let sink = RecordingSink::new();
    L2CocReconfig::try_new(
        hci::bt_hci::param::ConnHandle(0x0123),
        L2CocMtu::try_new(0x4567).unwrap(),
        L2CocMps::try_new(0x0089).unwrap(),
        &[L2CocChannelIndex::new(0xAA), L2CocChannelIndex::new(0xBB)],
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

#[test]
fn l2cap_credit_based_domains_reject_controller_invalid_values() {
    assert!(L2CocMtu::try_new(22).is_err());
    assert!(L2CocMps::try_new(22).is_err());
    assert!(L2CocMps::try_new(249).is_err());
    assert!(L2CocConnectionResult::try_new(0x10).is_err());
    assert!(L2CocMaximumChannelCount::try_new(0).is_err());
    assert!(L2CocMaximumChannelCount::try_new(6).is_err());
    assert!(L2CocSpsm::try_new(0).is_err());
    assert!(L2CocSpsm::try_new(0x0100).is_err());
    assert!(L2CocRequestedChannelCount::try_new(6).is_err());
    assert!(L2CocReconfigurationResult::try_new(5).is_err());
    assert!(L2CocCreditIncrement::try_new(0).is_err());
}

#[test]
fn l2cap_connect_confirm_decodes_semantic_channel_indices() {
    use bt_hci::FromHciBytes;

    let response = L2CapCocConnectConfirmWire::from_hci_bytes_complete(&[2, 0xAA, 0xBB]).unwrap();
    assert_eq!(
        response.channel_indices.as_slice(),
        [L2CocChannelIndex::new(0xAA), L2CocChannelIndex::new(0xBB)]
    );
}

#[test]
fn hal_config_offsets_own_their_documented_payload_lengths() {
    use hci::vendor::command::hal::{ConfigReadOffset, ConfigWriteOffset, HalWriteConfigData};

    assert!(HalWriteConfigData::try_new(ConfigWriteOffset::PublicAddress, &[0; 5]).is_err());
    assert!(HalWriteConfigData::try_new(ConfigWriteOffset::PublicAddress, &[0; 6]).is_ok());
    assert!(
        <ConfigReadOffset as hci::vendor::command::HciDecodeField<1>>::from_hci_field(&[0x01])
            .is_err()
    );
}

#[test]
fn gatt_long_updates_validate_intrinsic_and_cross_field_domains() {
    assert!(GattAttributeValueLength::try_new(513).is_err());
    assert!(GattNotificationTarget::try_new(0x0F00).is_err());
    assert!(GattNotificationTarget::try_new(0xEA40).is_err());
    assert!(GattNotificationTarget::try_new(0xEA3F).is_ok());
    assert!(GattNotificationTarget::for_enhanced_channel(L2CocChannelIndex::new(0x40)).is_err());

    let target = GattNotificationTarget::ALL_UNENHANCED;
    let total = GattAttributeValueLength::try_new(10).unwrap();
    assert!(
        GattUpdateLongCharacteristicValue::try_new(
            target,
            AttributeHandle(1),
            AttributeHandle(2),
            UpdateType::empty(),
            total,
            GattAttributeOffset::new(9),
            &[0xAA, 0xBB],
        )
        .is_err()
    );
    assert!(
        GattUpdateLongCharacteristicValue::try_new(
            target,
            AttributeHandle(1),
            AttributeHandle(2),
            UpdateType::empty(),
            total,
            GattAttributeOffset::new(8),
            &[0xAA, 0xBB],
        )
        .is_ok()
    );
}

#[cfg(before_fw_1_24_0)]
#[test]
fn legacy_gatt_deny_read_accepts_only_documented_att_errors() {
    use hci::vendor::command::gatt::GattDenyRead;

    let handle = hci::bt_hci::param::ConnHandle(1);
    assert!(GattDenyRead::try_new(handle, 0x07).is_err());
    assert!(GattDenyRead::try_new(handle, 0x08).is_ok());
    assert!(GattDenyRead::try_new(handle, 0x80).is_ok());
    assert!(GattDenyRead::try_new(handle, 0x9F).is_ok());
    assert!(GattDenyRead::try_new(handle, 0xA0).is_err());
}

#[cfg(since_fw_1_20_0)]
#[test]
fn ead_decryption_requires_randomizer_and_mic_overhead() {
    use hci::vendor::command::hal::{EadMode, HalEadEncryptDecrypt};

    let key = [0; 16];
    let iv = [0; 8];
    assert!(HalEadEncryptDecrypt::try_new(EadMode::Encrypt, &key, &iv, &[]).is_ok());
    assert!(HalEadEncryptDecrypt::try_new(EadMode::Decrypt, &key, &iv, &[0; 8]).is_err());
    assert!(HalEadEncryptDecrypt::try_new(EadMode::Decrypt, &key, &iv, &[0; 9]).is_ok());
}

#[cfg(since_fw_1_23_0)]
#[test]
fn system_reset_options_follow_the_selected_reset_mode() {
    use hci::vendor::command::sys::{
        ConfigWriteOffset, SysReset, SysResetMode, SysResetOptions, SysWriteConfigData,
    };

    assert!(SysReset::try_new(SysResetMode::NoOptionsChange, SysResetOptions::empty()).is_ok());
    assert!(
        SysReset::try_new(
            SysResetMode::NoOptionsChange,
            SysResetOptions::EXTENDED_ADVERTISING,
        )
        .is_err()
    );
    assert!(
        SysReset::try_new(
            SysResetMode::WithOptionsChange,
            SysResetOptions::EXTENDED_ADVERTISING | SysResetOptions::ENHANCED_ATT,
        )
        .is_ok()
    );

    assert!(SysWriteConfigData::try_new(ConfigWriteOffset::PublicAddress, &[0; 5]).is_err());
    assert!(SysWriteConfigData::try_new(ConfigWriteOffset::PublicAddress, &[0; 6]).is_ok());
    assert!(
        SysWriteConfigData::try_new(
            ConfigWriteOffset::LinkLayerMaximumDataLengthExtension,
            &[0; 7],
        )
        .is_err()
    );
    assert!(
        SysWriteConfigData::try_new(
            ConfigWriteOffset::LinkLayerMaximumDataLengthExtension,
            &[0; 8],
        )
        .is_ok()
    );
}

#[cfg(since_fw_1_24_0)]
#[test]
fn current_gatt_permissions_and_extra_data_ranges_are_validated() {
    pollster::block_on(current_gatt_permissions_and_extra_data_ranges_are_validated_async());
}
#[cfg(since_fw_1_24_0)]
async fn current_gatt_permissions_and_extra_data_ranges_are_validated_async() {
    use hci::vendor::command::gatt::{
        ExtraDataRangeError, ExtraDataReference, GattPermitRead, GattPermitWrite,
        GattWriteWithoutRespExt, ReadStatus,
    };

    let conn_handle = hci::bt_hci::param::ConnHandle(0x0123);
    let attribute_handle = AttributeHandle(0x4567);

    assert!(
        GattPermitWrite::try_new(
            conn_handle,
            attribute_handle,
            WriteStatus::Rejected,
            0x09,
            &[],
        )
        .is_err()
    );
    assert!(
        GattPermitWrite::try_new(
            conn_handle,
            attribute_handle,
            WriteStatus::Rejected,
            0x80,
            &[],
        )
        .is_ok()
    );
    assert!(
        GattPermitRead::try_new(conn_handle, ReadStatus::Allowed, 0, AttributeHandle(0)).is_ok()
    );
    assert!(
        GattPermitRead::try_new(conn_handle, ReadStatus::Allowed, 0x08, AttributeHandle(0))
            .is_err()
    );
    assert!(
        GattPermitRead::try_new(conn_handle, ReadStatus::Rejected, 0x7F, attribute_handle,)
            .is_err()
    );

    let inverted_start = 20;
    let inverted_end = 10;
    assert!(matches!(
        ExtraDataReference::try_new(inverted_start..inverted_end),
        Err(ExtraDataRangeError::Inverted { .. })
    ));
    assert!(matches!(
        ExtraDataReference::try_new(0..65_536),
        Err(ExtraDataRangeError::TooLong { .. })
    ));

    let data = ExtraDataReference::try_new(0x1020_3040..0x1020_3044).unwrap();
    assert_eq!((data.offset(), data.length()), (0x1020_3040, 4));

    let sink = RecordingSink::new();
    GattWriteWithoutRespExt::new(conn_handle, attribute_handle, false, data)
        .exec(&sink)
        .await
        .unwrap();
    assert_eq!(
        sink.written_data(),
        [
            1, 0x40, 0xFD, 11, 0x23, 0x01, 0x67, 0x45, 0, 4, 0, 0x40, 0x30, 0x20, 0x10,
        ]
    );
}

#[test]
fn declarative_l2cap_tx_data_writes_only_the_declared_data() {
    pollster::block_on(declarative_l2cap_tx_data_writes_only_the_declared_data_async());
}
async fn declarative_l2cap_tx_data_writes_only_the_declared_data_async() {
    let sink = RecordingSink::new();
    L2CocTxData::try_new(L2CocChannelIndex::new(3), &[0xAA, 0xBB])
        .unwrap()
        .exec(&sink)
        .await
        .unwrap();

    assert_eq!(sink.written_data(), [1, 0x8E, 0xFD, 5, 3, 2, 0, 0xAA, 0xBB]);
}

#[test]
fn declarative_l2cap_rejects_lengths_beyond_the_wire_bounds() {
    let reconfig = [L2CocChannelIndex::new(0); 6];
    let tx = [0; 253];

    assert!(
        L2CocReconfig::try_new(
            hci::bt_hci::param::ConnHandle(0),
            L2CocMtu::try_new(23).unwrap(),
            L2CocMps::try_new(23).unwrap(),
            &[],
        )
        .is_err()
    );
    assert!(
        L2CocReconfig::try_new(
            hci::bt_hci::param::ConnHandle(0),
            L2CocMtu::try_new(23).unwrap(),
            L2CocMps::try_new(23).unwrap(),
            &reconfig,
        )
        .is_err()
    );
    assert!(L2CocTxData::try_new(L2CocChannelIndex::new(0), &tx).is_err());
}
