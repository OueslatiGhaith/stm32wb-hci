//! `Direct` calls against stand-ins for the STM32WBA BLE stack functions.

#![cfg(all(feature = "direct", feature = "wba_1_10_0", feature = "stack-wba-full"))]
#![allow(non_snake_case, clippy::missing_safety_doc)]

use std::sync::Mutex;

use bt_hci::FromHciBytes;
use bt_hci::cmd::controller_baseband::Reset;
use bt_hci::cmd::le::{LeReadBufferSize, LeSetAdvParams, LeSetAdvParamsParams, LeSetExtScanParams};
use bt_hci::param::{AddrKind, PhyParams, ScanningFilterPolicy, ScanningPhy};
use stm32wb_hci::aci::gap::GapGetOobData;
use stm32wb_hci::aci::gatt::GattAddService;
use stm32wb_hci::aci::general::AciReadConfigData;
use stm32wb_hci::aci::hal::HalReadConfigData;
use stm32wb_hci::aci::values::{OobDataType, ReadableConfigDataOffset, ServiceType};
use stm32wb_hci::direct::{Direct, Error};
use stm32wb_hci::wire::Uuid;

/// The arguments the last stand-in received.
static ARGS: Mutex<Vec<u8>> = Mutex::new(Vec::new());
/// Serializes the tests, which share `ARGS`.
static TEST: Mutex<()> = Mutex::new(());

fn record(bytes: &[u8]) {
    ARGS.lock().unwrap().extend_from_slice(bytes);
}

fn take_args() -> Vec<u8> {
    std::mem::take(&mut *ARGS.lock().unwrap())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn HCI_LE_SET_ADVERTISING_PARAMETERS(
    min: u16,
    max: u16,
    kind: u8,
    own: u8,
    peer_kind: u8,
    peer: *const u8,
    map: u8,
    filter: u8,
) -> u8 {
    record(&min.to_le_bytes());
    record(&max.to_le_bytes());
    record(&[kind, own, peer_kind]);
    record(unsafe { core::slice::from_raw_parts(peer, 6) });
    record(&[map, filter]);
    0
}

#[test]
fn core_parameters_are_passed_in_order() {
    let _test = TEST.lock().unwrap();
    let wire = [0x20, 0, 0x40, 0, 0, 0, 1, 1, 2, 3, 4, 5, 6, 7, 0];
    let command =
        LeSetAdvParams::from(LeSetAdvParamsParams::from_hci_bytes_complete(&wire).unwrap());
    unsafe { command.call() }.unwrap();
    assert_eq!(take_args(), wire);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn HCI_LE_READ_BUFFER_SIZE(len: *mut u16, packets: *mut u8) -> u8 {
    unsafe {
        *len = 251;
        *packets = 6;
    }
    0
}

#[test]
fn core_returns_are_decoded() {
    let _test = TEST.lock().unwrap();
    let returned = unsafe { LeReadBufferSize::new().call() }.unwrap();
    assert_eq!({ returned.le_acl_data_packet_length }, 251);
    assert_eq!(returned.total_num_le_acl_data_packets, 6);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn HCI_RESET() -> u8 {
    0x0C
}

#[test]
fn an_error_status_is_returned() {
    let _test = TEST.lock().unwrap();
    match unsafe { Reset::new().call() } {
        Err(Error::Hci(error)) => assert_eq!(error.to_status().into_inner(), 0x0C),
        other => panic!("{other:?}"),
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ACI_HAL_READ_CONFIG_DATA(offset: u8, len: *mut u8, data: *mut u8) -> u8 {
    record(&[offset]);
    unsafe {
        *len = 6;
        core::ptr::copy_nonoverlapping([1, 2, 3, 4, 5, 6].as_ptr(), data, 6);
    }
    0
}

#[test]
fn counted_returns_keep_the_counted_elements() {
    let _test = TEST.lock().unwrap();
    let command = HalReadConfigData::new(ReadableConfigDataOffset::StaticRandomAddress);
    let returned = unsafe { command.call() }.unwrap();
    assert_eq!(take_args(), [0x2E]);
    assert_eq!(returned.data.as_slice(), [1, 2, 3, 4, 5, 6]);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ACI_READ_CONFIG_DATA(_offset: u8, len: *mut u8, _data: *mut u8) -> u8 {
    unsafe { *len = 251 };
    0
}

#[test]
fn a_count_beyond_the_capacity_is_invalid() {
    let _test = TEST.lock().unwrap();
    let command = AciReadConfigData::new(ReadableConfigDataOffset::PublicAddress);
    assert_eq!(unsafe { command.call() }.unwrap_err(), Error::Invalid);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ACI_GATT_ADD_SERVICE(
    uuid_type: u8,
    uuid: *const u8,
    service_type: u8,
    max_records: u8,
    handle: *mut u16,
) -> u8 {
    let width = if uuid_type == 1 { 2 } else { 16 };
    record(&[uuid_type]);
    record(unsafe { core::slice::from_raw_parts(uuid, width) });
    record(&[service_type, max_records]);
    unsafe { *handle = 0x0042 };
    0
}

#[test]
fn unions_pass_the_selected_width() {
    let _test = TEST.lock().unwrap();
    let returned =
        unsafe { GattAddService::new(Uuid::Uuid16(0x180D), ServiceType::Primary, 4).call() }
            .unwrap();
    assert_eq!(take_args(), [1, 0x0D, 0x18, 1, 4]);
    assert_eq!(returned.service_handle, 0x0042);

    let uuid = [7; 16];
    unsafe { GattAddService::new(Uuid::Uuid128(uuid), ServiceType::Secondary, 9).call() }.unwrap();
    let mut expected = vec![2];
    expected.extend_from_slice(&uuid);
    expected.extend_from_slice(&[2, 9]);
    assert_eq!(take_args(), expected);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn ACI_GAP_GET_OOB_DATA(
    oob_type: u8,
    address_type: *mut u8,
    address: *mut u8,
    len: *mut u8,
    data: *mut u8,
) -> u8 {
    record(&[oob_type]);
    unsafe {
        *address_type = 1;
        core::ptr::copy_nonoverlapping([9, 8, 7, 6, 5, 4].as_ptr(), address, 6);
        *len = 16;
        core::ptr::write_bytes(data, 0xAB, 16);
    }
    0
}

#[test]
fn echoed_parameters_are_returned_in_place() {
    let _test = TEST.lock().unwrap();
    let returned = unsafe { GapGetOobData::new(OobDataType::Random).call() }.unwrap();
    assert_eq!(take_args(), [1]);
    assert_eq!(returned.address.raw(), [9, 8, 7, 6, 5, 4]);
    assert_eq!(returned.oob_data_type, 1);
    assert_eq!(returned.oob_data_len, 16);
    assert_eq!(returned.oob_data, [0xAB; 16]);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn HCI_LE_SET_EXTENDED_SCAN_PARAMETERS(
    own: u8,
    filter: u8,
    phys: u8,
    sets: *const u8,
) -> u8 {
    record(&[own, filter, phys]);
    record(unsafe { core::slice::from_raw_parts(sets, phys.count_ones() as usize * 5) });
    0
}

#[test]
fn bt_hci_extended_scans_pass_one_set_per_phy() {
    let _test = TEST.lock().unwrap();
    let one_m = ScanningPhy::from_hci_bytes_complete(&[1, 0x10, 0, 0x08, 0]).unwrap();
    let coded = ScanningPhy::from_hci_bytes_complete(&[0, 0x20, 0, 0x18, 0]).unwrap();
    let command = LeSetExtScanParams::new(
        AddrKind::RANDOM,
        ScanningFilterPolicy::BasicUnfiltered,
        PhyParams {
            le_1m_phy: Some(one_m),
            le_2m_phy: None,
            le_coded_phy: Some(coded),
        },
    );
    unsafe { command.call() }.unwrap();
    assert_eq!(
        take_args(),
        [1, 0, 0b101, 1, 0x10, 0, 0x08, 0, 0, 0x20, 0, 0x18, 0]
    );
}
