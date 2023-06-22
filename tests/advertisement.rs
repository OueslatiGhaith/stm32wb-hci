#![feature(async_fn_in_trait)]

use hci::types::Advertisement;

extern crate stm32wb_hci as hci;

#[test]
fn complete_name() {
    // Example advertising data - complete local name
    let expected = [
        0x0a, 0x09, 0x50, 0x65, 0x64, 0x6f, 0x6d, 0x65, 0x74, 0x65, 0x72,
    ];
    let adv = Advertisement::CompleteLocalName("Pedometer");

    let mut o = [0; 31];
    let l = adv.copy_into_slice(&mut o);
    assert_eq!(expected.len(), l);
    assert_eq!(expected, o[..l]);
}

#[test]
fn ibeacon() {
    let expected = [
        0x1a, 0xff, 0x4c, 0x0, 0x2, 0x15, 0xfb, 0xb, 0x57, 0xa2, 0x82, 0x28, 0x44, 0xcd, 0x91,
        0x3a, 0x94, 0xa1, 0x22, 0xba, 0x12, 0x6, 0x0, 0x1, 0x0, 0x2, 0xd1,
    ];
    let payload = [
        0x2, 0x15, 0xfb, 0xb, 0x57, 0xa2, 0x82, 0x28, 0x44, 0xcd, 0x91, 0x3a, 0x94, 0xa1, 0x22,
        0xba, 0x12, 0x6, 0x0, 0x1, 0x0, 0x2, 0xd1,
    ];
    let adv = Advertisement::ManufacturerSpecificData(0x4c, &payload);

    let mut o = [0; 31];
    let l = adv.copy_into_slice(&mut o);
    assert_eq!(expected.len(), l);
    assert_eq!(expected, o[..l]);
}

#[test]
fn eddystone() {
    let expected = [
        0x10, 0x16, 0xaa, 0xfe, 0x10, 0x0, 0x01, 0x72, 0x75, 0x73, 0x74, 0x2d, 0x6c, 0x61, 0x6e,
        0x67, 0x01,
    ];
    // URL(0x10), 0, https://www. (0x01), "rust-lang", .org/ (0x01)
    let payload = b"\x10\x00\x01rust-lang\x01";
    let adv = Advertisement::ServiceData16BitUuid(0xfeaa, payload);
    let mut o = [0; 31];
    let l = adv.copy_into_slice(&mut o);
    assert_eq!(expected.len(), l);
    assert_eq!(expected, o[..l]);
}
