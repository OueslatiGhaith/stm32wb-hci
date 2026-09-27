//! Hardware abstraction layer commands.

use stm32wb_hci_macros::vendor_command;

vendor_command! {
    /// Stop a tone started with [`HalToneStart`].
    aci_hal_tone_stop => HalToneStop {}
}

vendor_command! {
    /// Start transmitting a continuous tone on an RF channel.
    aci_hal_tone_start => HalToneStart {
        rf_channel: u8,
        freq_offset: u8,
    }
}

vendor_command! {
    /// Select the radio activities reported by the radio activity event.
    aci_hal_set_radio_activity_mask => HalSetRadioActivityMask {
        radio_activity_mask: u16,
    }
}

vendor_command! {
    /// Enable or disable the HAL events.
    aci_hal_set_event_mask => HalSetEventMask {
        event_mask: u32,
    }
}

vendor_command! {
    /// Reset the BLE stack.
    aci_hal_stack_reset => HalStackReset {}
}

#[cfg(test)]
mod tests {
    use bt_hci::WriteHci;
    use bt_hci::cmd::Cmd;

    use super::*;

    fn encode(command: &impl WriteHci) -> ([u8; 16], usize) {
        let mut buffer = [0; 16];
        let mut writer = &mut buffer[..];
        command.write_hci(&mut writer).unwrap();
        let len = 16 - writer.len();
        (buffer, len)
    }

    #[test]
    fn opcodes_come_from_the_catalog() {
        assert_eq!(HalToneStop::OPCODE.to_raw(), 0xFC16);
        assert_eq!(HalToneStart::OPCODE.to_raw(), 0xFC15);
        assert_eq!(HalSetRadioActivityMask::OPCODE.to_raw(), 0xFC18);
    }

    #[test]
    fn parameters_encode_in_catalog_order() {
        let (bytes, len) = encode(&HalToneStart::new(39, 2));
        assert_eq!(bytes[..len], [0x15, 0xFC, 2, 39, 2]);
        let (bytes, len) = encode(&HalSetRadioActivityMask::new(0x0102));
        assert_eq!(bytes[..len], [0x18, 0xFC, 2, 0x02, 0x01]);
        let (bytes, len) = encode(&HalToneStop::new());
        assert_eq!(bytes[..len], [0x16, 0xFC, 0]);
    }

    #[cfg(feature = "stack-full-extended")]
    #[test]
    fn profile_specific_commands_follow_the_catalog() {
        let (bytes, len) = encode(&HalSetEventMask::new(0x0403_0201));
        assert_eq!(bytes[..len], [0x1A, 0xFC, 4, 1, 2, 3, 4]);
    }
}
