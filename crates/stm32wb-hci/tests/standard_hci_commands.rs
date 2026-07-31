use stm32wb_hci as hci;

mod vendor;

use hci::bt_hci::cmd::{AsyncCmd, SyncCmd};
#[cfg(since_fw_1_17_0)]
use hci::standard::LeGenerateDhkeyV2;
use hci::standard::{
    LeGenerateDhkey, LeReadLocalP256PublicKey, LeReadPeerResolvableAddress, LeReceiverTest,
    LeReceiverTestV2, LeTransmitterTest, LeTransmitterTestV2,
};
#[cfg(since_fw_1_23_0)]
use hci::standard::{LeSetResolvablePrivateAddressTimeoutV2, ResolvablePrivateAddressTimeoutRange};
use vendor::RecordingSink;

#[test]
fn p256_public_key_command_has_the_stm32wb_hci_envelope() {
    pollster::block_on(p256_public_key_command_has_the_stm32wb_hci_envelope_async());
}
async fn p256_public_key_command_has_the_stm32wb_hci_envelope_async() {
    let sink = RecordingSink::new();

    let _ = LeReadLocalP256PublicKey::new().exec(&sink).await;

    // HCI command indicator, LE OGF / OCF 0x025, no parameters.
    assert_eq!(sink.written_data(), [1, 0x25, 0x20, 0]);
}

#[test]
fn generate_dhkey_preserves_the_full_64_byte_key() {
    pollster::block_on(generate_dhkey_preserves_the_full_64_byte_key_async());
}
async fn generate_dhkey_preserves_the_full_64_byte_key_async() {
    let sink = RecordingSink::new();
    let key = [0xA5; 64];

    let _ = LeGenerateDhkey::new(key).exec(&sink).await;

    let bytes = sink.written_data();
    assert_eq!(&bytes[..4], [1, 0x26, 0x20, 64]);
    assert_eq!(&bytes[4..], key);
}

#[test]
fn resolvable_address_command_has_exact_input_and_opcode() {
    pollster::block_on(resolvable_address_command_has_exact_input_and_opcode_async());
}
async fn resolvable_address_command_has_exact_input_and_opcode_async() {
    let sink = RecordingSink::new();

    let _ = LeReadPeerResolvableAddress::new([1, 2, 3, 4, 5, 6, 7])
        .exec(&sink)
        .await;

    assert_eq!(sink.written_data(), [1, 0x2B, 0x20, 7, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn v2_test_commands_use_their_v2_ocfs() {
    pollster::block_on(v2_test_commands_use_their_v2_ocfs_async());
}
async fn v2_test_commands_use_their_v2_ocfs_async() {
    let receiver = RecordingSink::new();
    let _ = LeReceiverTestV2::new([1, 2, 3]).exec(&receiver).await;
    assert_eq!(receiver.written_data(), [1, 0x33, 0x20, 3, 1, 2, 3]);

    let transmitter = RecordingSink::new();
    let _ = LeTransmitterTestV2::new([1, 2, 3, 4])
        .exec(&transmitter)
        .await;
    assert_eq!(transmitter.written_data(), [1, 0x34, 0x20, 4, 1, 2, 3, 4]);
}

#[test]
fn v1_test_commands_use_their_v1_ocfs() {
    pollster::block_on(v1_test_commands_use_their_v1_ocfs_async());
}
async fn v1_test_commands_use_their_v1_ocfs_async() {
    let receiver = RecordingSink::new();
    let _ = LeReceiverTest::new([1]).exec(&receiver).await;
    assert_eq!(receiver.written_data(), [1, 0x1D, 0x20, 1, 1]);

    let transmitter = RecordingSink::new();
    let _ = LeTransmitterTest::new([1, 2, 3]).exec(&transmitter).await;
    assert_eq!(transmitter.written_data(), [1, 0x1E, 0x20, 3, 1, 2, 3]);
}

#[cfg(since_fw_1_17_0)]
#[test]
fn dhkey_v2_is_exposed_only_on_firmware_that_has_it() {
    pollster::block_on(dhkey_v2_is_exposed_only_on_firmware_that_has_it_async());
}
#[cfg(since_fw_1_17_0)]
async fn dhkey_v2_is_exposed_only_on_firmware_that_has_it_async() {
    let sink = RecordingSink::new();
    let key = [0x5A; 65];

    let _ = LeGenerateDhkeyV2::new(key).exec(&sink).await;

    let bytes = sink.written_data();
    assert_eq!(&bytes[..4], [1, 0x5E, 0x20, 65]);
    assert_eq!(&bytes[4..], key);
}

#[cfg(since_fw_1_23_0)]
#[test]
fn resolvable_private_address_timeout_v2_uses_its_full_opcode() {
    pollster::block_on(resolvable_private_address_timeout_v2_uses_its_full_opcode_async());
}
#[cfg(since_fw_1_23_0)]
async fn resolvable_private_address_timeout_v2_uses_its_full_opcode_async() {
    let sink = RecordingSink::new();

    LeSetResolvablePrivateAddressTimeoutV2::new(
        ResolvablePrivateAddressTimeoutRange::try_new(1, 0x0403).unwrap(),
    )
    .exec(&sink)
    .await
    .unwrap();

    assert_eq!(sink.written_data(), [1, 0x9E, 0x20, 4, 1, 0, 3, 4]);

    assert!(ResolvablePrivateAddressTimeoutRange::try_new(0, 10).is_err());
    assert!(ResolvablePrivateAddressTimeoutRange::try_new(10, 9).is_err());
    assert!(ResolvablePrivateAddressTimeoutRange::try_new(1, 0x0E11).is_err());
}
