//! ST system (SHCI) commands, sent on the CPU2 system channel, in opcode
//! order, and the system events it delivers, in code order.
//!
//! Each command implements [`SystemCommand`](crate::wire::SystemCommand)
//! rather than bt-hci's `Cmd`: the system channel has its own command buffer,
//! and several system opcodes are also ACI opcodes. Addresses the wireless
//! CPU reads, such as [`BleInit`]'s buffers, are `u32`s.
//!
//! Each event implements [`SystemEvent`](crate::wire::SystemEvent), and
//! [`ShciEvent`] decodes any of them from the payload of a system event
//! packet, which starts with the sub-event code.
//!
//! The catalog rules out the commands and events for Thread, Zigbee,
//! 802.15.4, and the LLD test binaries, and the firmware upgrade command,
//! which omits its addresses when they are zero.

use stm32wb_hci_macros::{system_command, system_event, system_events};

system_command! {
    /// Read the state of the firmware upgrade service (FUS), which also
    /// starts it if the wireless stack is running.
    SHCI_C2_FUS_GetState => FusGetState {} -> FusState {
        error_code: u8,
    }
}

system_command! {
    /// Delete the wireless stack.
    SHCI_C2_FUS_FwDelete => FusFwDelete {}
}

system_command! {
    /// Replace the key authenticating wireless stack images.
    SHCI_C2_FUS_UpdateAuthKey => FusUpdateAuthKey {
        key_size: u8,
        key_data: [u8; 64],
    }
}

system_command! {
    /// Lock the authentication key, which can then no longer be replaced.
    SHCI_C2_FUS_LockAuthKey => FusLockAuthKey {}
}

system_command! {
    /// Store a user key of up to 32 bytes, returning the index the service
    /// allocates to it. The key type, and an encrypted key's IV, follow the
    /// key.
    SHCI_C2_FUS_StoreUsrKey => FusStoreUsrKey {
        key_data: &'a [u8],
        #[wire(name = "IV")]
        key_type: UserKeyType,
    } -> FusUsrKeyIndex {
        key_index: u8,
    }
}

/// The type of a user key [`FusStoreUsrKey`] stores, written as its
/// `KeyType`, with the IV an encrypted key is sent with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum UserKeyType {
    /// A plain key (`KEYTYPE_SIMPLE`).
    Simple,
    /// A master key (`KEYTYPE_MASTER`), which decrypts encrypted keys.
    Master,
    /// A key encrypted with the master key (`KEYTYPE_ENCRYPTED`), with its
    /// 12-byte IV.
    Encrypted { iv: [u8; 12] },
}

impl crate::wire::HciWireUnion for UserKeyType {
    const VARIANTS: &'static [(u64, usize)] = &[(1, 0), (2, 0), (3, 12)];

    fn selector(&self) -> u64 {
        match self {
            UserKeyType::Simple => 1,
            UserKeyType::Master => 2,
            UserKeyType::Encrypted { .. } => 3,
        }
    }
}

impl bt_hci::WriteHci for UserKeyType {
    fn size(&self) -> usize {
        match self {
            UserKeyType::Simple | UserKeyType::Master => 0,
            UserKeyType::Encrypted { iv } => iv.len(),
        }
    }

    fn write_hci<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        match self {
            UserKeyType::Simple | UserKeyType::Master => Ok(()),
            UserKeyType::Encrypted { iv } => writer.write_all(iv),
        }
    }

    async fn write_hci_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        match self {
            UserKeyType::Simple | UserKeyType::Master => Ok(()),
            UserKeyType::Encrypted { iv } => writer.write_all(iv).await,
        }
    }
}

system_command! {
    /// Load the stored user key `key_index` into the AES engine.
    SHCI_C2_FUS_LoadUsrKey => FusLoadUsrKey {
        key_index: u8,
    }
}

system_command! {
    /// Start the wireless stack from the firmware upgrade service.
    SHCI_C2_FUS_StartWs => FusStartWs {}
}

system_command! {
    /// Delete the wireless stack and every image the service stores.
    SHCI_C2_FUS_FwPurge => FusFwPurge {}
}

system_command! {
    /// Lock the stored user key `key_index` in the AES engine until reset.
    SHCI_C2_FUS_LockUsrKey => FusLockUsrKey {
        key_index: u8,
    }
}

system_command! {
    /// Unload the stored user key `key_index` from the AES engine.
    SHCI_C2_FUS_UnloadUsrKey => FusUnloadUsrKey {
        key_index: u8,
    }
}

system_command! {
    /// Refuse wireless stack images older than the installed one from now on.
    SHCI_C2_FUS_ActivateAntiRollback => FusActivateAntiRollback {}
}

system_command! {
    /// Start the BLE stack with its memory and radio configuration. Fields
    /// the selected release does not have are not declared.
    SHCI_C2_BLE_Init => BleInit {
        p_ble_buffer_address: u32,
        ble_buffer_size: u32,
        num_attr_record: u16,
        num_attr_serv: u16,
        attr_value_arr_size: u16,
        num_of_links: u8,
        extended_packet_length_enable: u8,
        pr_write_list_size: u8,
        mblock_count: u8,
        att_mtu: u16,
        peripheral_sca: u16,
        central_sca: u8,
        ls_source: u8,
        max_conn_event_length: u32,
        hs_startup_time: u16,
        viterbi_enable: u8,
        options: u8,
        hw_version: u8,
        max_coc_initiator_nbr: u8,
        min_tx_power: i8,
        max_tx_power: i8,
        rx_model_config: u8,
        max_adv_set_nbr: u8,
        max_adv_data_len: u16,
        tx_path_compens: i16,
        rx_path_compens: i16,
        ble_core_version: u8,
        #[wire(since = "1.16.0")]
        options_extension: u8,
        #[wire(since = "1.23.0")]
        max_add_eatt_bearers: u8,
        #[wire(since = "1.24.0")]
        extra_data_buffer: u32,
        #[wire(since = "1.24.0")]
        extra_data_buffer_size: u32,
    }
}

system_command! {
    /// Configure the wireless CPU's debug GPIOs and traces, given the
    /// addresses and sizes of the configuration tables.
    SHCI_C2_DEBUG_Init => DebugInit {
        p_gpio_config: u32,
        p_traces_config: u32,
        p_general_config: u32,
        gpio_config_size: u8,
        traces_config_size: u8,
        general_config_size: u8,
    }
}

system_command! {
    /// Signal that flash erasing starts (`erase_activity` 1) or ends (0), so
    /// the radio can avoid it.
    SHCI_C2_FLASH_EraseActivity => FlashEraseActivity {
        erase_activity: u8,
    }
}

system_command! {
    /// Write the NVM data of the stack `ip` to flash.
    SHCI_C2_FLASH_StoreData => FlashStoreData {
        ip: u8,
    }
}

system_command! {
    /// Erase the NVM data of the stack `ip` from flash.
    SHCI_C2_FLASH_EraseData => FlashEraseData {
        ip: u8,
    }
}

system_command! {
    /// Allow or forbid the radio of the stack `ip` to enter low power mode.
    SHCI_C2_RADIO_AllowLowPower => RadioAllowLowPower {
        ip: u8,
        flag_radio_low_power_on: u8,
    }
}

system_command! {
    /// Restart the wireless stack without resetting the device.
    SHCI_C2_Reinit => Reinit {}
}

system_command! {
    /// Configure the GPIO controlling an external power amplifier.
    SHCI_C2_ExtpaConfig => ExtpaConfig {
        gpio_port: u32,
        gpio_pin_number: u16,
        gpio_polarity: u8,
        gpio_status: u8,
    }
}

system_command! {
    /// Select whether radio activity or the wireless CPU's semaphore
    /// arbitrates flash access.
    SHCI_C2_SetFlashActivityControl => SetFlashActivityControl {
        source: u8,
    }
}

system_command! {
    /// Configure the wireless CPU: its options, the system events it reports,
    /// and the addresses of the stacks' NVM data in RAM.
    SHCI_C2_Config => Config {
        payload_cmd_size: u8,
        config1: u8,
        evt_mask1: u8,
        spare1: u8,
        ble_nvm_ram_address: u32,
        thread_nvm_ram_address: u32,
        revision_id: u16,
        device_id: u16,
    }
}

system_command! {
    /// Switch the system clock source of the wireless CPU.
    SHCI_C2_SetSystemClock => SetSystemClock {
        clock_sel: u8,
    }
}

system_event! {
    /// The wireless CPU has started, running the firmware `sysevt_ready_rsp`
    /// names: 0x00 the wireless stack, 0x01 the firmware upgrade service,
    /// 0x10 or 0x11 an NVM backup or restore.
    SHCI_SUB_EVT_CODE_READY => ReadyEvent {
        sysevt_ready_rsp: u8,
    }
}

system_event! {
    /// The wireless CPU reports an error it cannot recover from.
    SHCI_SUB_EVT_ERROR_NOTIF => ErrorNotifEvent {
        error_code: u8,
    }
}

system_event! {
    /// The BLE stack updated the NVM data it keeps in SRAM, from
    /// `start_address` over `size` bytes.
    SHCI_SUB_EVT_BLE_NVM_RAM_UPDATE => BleNvmRamUpdateEvent {
        start_address: u32,
        size: u32,
    }
}

system_event! {
    /// The wireless CPU is about to write `number_of_words` words to flash.
    SHCI_SUB_EVT_NVM_START_WRITE => NvmStartWriteEvent {
        number_of_words: u32,
    }
}

system_event! {
    /// The wireless CPU finished writing to flash.
    SHCI_SUB_EVT_NVM_END_WRITE => NvmEndWriteEvent {}
}

system_event! {
    /// The wireless CPU is about to erase `number_of_sectors` flash sectors.
    SHCI_SUB_EVT_NVM_START_ERASE => NvmStartEraseEvent {
        number_of_sectors: u32,
    }
}

system_event! {
    /// The wireless CPU finished erasing flash.
    SHCI_SUB_EVT_NVM_END_ERASE => NvmEndEraseEvent {}
}

system_events! {
    /// Every ST system event the selected target emits. Decode one from the
    /// payload of a system event packet with
    /// [`ShciEvent::from_system_params`].
    pub enum ShciEvent<'a> {
        SHCI_SUB_EVT_CODE_READY => Ready(ReadyEvent),
        SHCI_SUB_EVT_ERROR_NOTIF => ErrorNotif(ErrorNotifEvent),
        SHCI_SUB_EVT_BLE_NVM_RAM_UPDATE => BleNvmRamUpdate(BleNvmRamUpdateEvent),
        SHCI_SUB_EVT_NVM_START_WRITE => NvmStartWrite(NvmStartWriteEvent),
        SHCI_SUB_EVT_NVM_END_WRITE => NvmEndWrite(NvmEndWriteEvent),
        SHCI_SUB_EVT_NVM_START_ERASE => NvmStartErase(NvmStartEraseEvent),
        SHCI_SUB_EVT_NVM_END_ERASE => NvmEndErase(NvmEndEraseEvent),
    }
}

#[cfg(test)]
mod tests {
    use bt_hci::{FromHciBytes, WriteHci};

    use super::*;
    use crate::wire::{SystemCommand, SystemEvent};

    fn encode<C: SystemCommand>(command: &C) -> ([u8; 64], usize) {
        let mut buffer = [0; 64];
        let mut writer = &mut buffer[..];
        command.params().write_hci(&mut writer).unwrap();
        let len = 64 - writer.len();
        (buffer, len)
    }

    #[test]
    fn system_commands_carry_their_opcode_and_returns() {
        assert_eq!(FusGetState::OPCODE, 0xFC52);
        assert_eq!(encode(&FusGetState::new()).1, 0);
        let state =
            <FusGetState as SystemCommand>::Return::from_hci_bytes_complete(&[0x03]).unwrap();
        assert_eq!(state.error_code, 3);

        let (bytes, len) = encode(&ExtpaConfig::new(0x4800_0000, 0x0010, 1, 0));
        assert_eq!(ExtpaConfig::OPCODE, 0xFC72);
        assert_eq!(bytes[..len], [0, 0, 0, 0x48, 0x10, 0, 1, 0]);

        let (bytes, len) = encode(&RadioAllowLowPower::new(1, 0));
        assert_eq!(bytes[..len], [1, 0]);
    }

    #[test]
    fn user_keys_carry_their_type_size_and_iv() {
        assert_eq!(FusStoreUsrKey::OPCODE, 0xFC58);
        let (bytes, len) = encode(&FusStoreUsrKey::try_new(&[7; 16], UserKeyType::Simple).unwrap());
        assert_eq!(bytes[..2], [1, 16]);
        assert_eq!(bytes[2..len], [7; 16]);

        let iv = [9; 12];
        let command = FusStoreUsrKey::try_new(&[7; 32], UserKeyType::Encrypted { iv }).unwrap();
        let (bytes, len) = encode(&command);
        assert_eq!(len, 2 + 32 + 12);
        assert_eq!(bytes[..2], [3, 32]);
        assert_eq!(bytes[2..34], [7; 32]);
        assert_eq!(bytes[34..len], iv);

        assert!(FusStoreUsrKey::try_new(&[7; 33], UserKeyType::Master).is_err());
        let index =
            <FusStoreUsrKey as SystemCommand>::Return::from_hci_bytes_complete(&[2]).unwrap();
        assert_eq!(index.key_index, 2);
    }

    #[test]
    fn ble_init_is_a_system_command() {
        assert_eq!(BleInit::OPCODE, 0xFC66);
    }

    #[cfg(feature = "fw_1_24_0")]
    #[test]
    fn ble_init_encodes_every_field_in_order() {
        let command = BleInit::from(BleInitParams {
            p_ble_buffer_address: 0,
            ble_buffer_size: 0,
            num_attr_record: 68,
            num_attr_serv: 4,
            attr_value_arr_size: 1344,
            num_of_links: 2,
            extended_packet_length_enable: 1,
            pr_write_list_size: 0x3A,
            mblock_count: 0x79,
            att_mtu: 156,
            peripheral_sca: 500,
            central_sca: 0,
            ls_source: 1,
            max_conn_event_length: 0xFFFF_FFFF,
            hs_startup_time: 0x148,
            viterbi_enable: 1,
            options: 0,
            hw_version: 0,
            max_coc_initiator_nbr: 32,
            min_tx_power: -40,
            max_tx_power: 6,
            rx_model_config: 0,
            max_adv_set_nbr: 2,
            max_adv_data_len: 1650,
            tx_path_compens: 0,
            rx_path_compens: 0,
            ble_core_version: 11,
            options_extension: 0,
            max_add_eatt_bearers: 4,
            extra_data_buffer: 0x2003_0000,
            extra_data_buffer_size: 0x100,
        });
        let (bytes, len) = encode(&command);
        assert_eq!(len, 55);
        assert_eq!(bytes[8..10], 68u16.to_le_bytes());
        assert_eq!(bytes[20..22], 500u16.to_le_bytes());
        assert_eq!(bytes[34], (-40i8) as u8);
        assert_eq!(bytes[46], 4);
        assert_eq!(bytes[47..51], 0x2003_0000u32.to_le_bytes());
        assert_eq!(bytes[51..55], 0x100u32.to_le_bytes());
    }

    #[test]
    fn system_events_decode_after_their_sub_event_code() {
        // The payload of a ready event packet reporting the upgrade service.
        let params = [0x00, 0x92, 0x01];
        assert_eq!(
            ShciEvent::from_system_params(&params),
            Some(Ok(ShciEvent::Ready(ReadyEvent {
                sysevt_ready_rsp: 1
            })))
        );
        assert_eq!(
            ReadyEvent::from_system_params(&params),
            Some(Ok(ReadyEvent {
                sysevt_ready_rsp: 1
            }))
        );

        let params = [0x02, 0x92, 0x00, 0x00, 0x03, 0x20, 0x00, 0x10, 0x00, 0x00];
        let Some(Ok(ShciEvent::BleNvmRamUpdate(update))) = ShciEvent::from_system_params(&params)
        else {
            panic!("0x9202 is the BLE NVM RAM update");
        };
        assert_eq!((update.start_address, update.size), (0x2003_0000, 0x1000));

        assert_eq!(
            ShciEvent::from_system_params(&[0x05, 0x92]),
            Some(Ok(ShciEvent::NvmEndWrite(NvmEndWriteEvent {})))
        );
        // Thread's NVM update is outside this crate.
        assert!(ShciEvent::from_system_params(&[0x03, 0x92, 0, 0, 0, 0, 0, 0, 0, 0]).is_none());
        assert!(
            ShciEvent::from_system_params(&[0x00, 0x92])
                .unwrap()
                .is_err()
        );
    }
}
