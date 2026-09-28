//! Commands for the wireless binary as a whole rather than one layer, in
//! opcode order. They exist from 1.23.0.

#[allow(
    unused_imports,
    reason = "releases before 1.23.0 have no general commands"
)]
use stm32wb_hci_macros::vendor_command;

#[allow(
    unused_imports,
    reason = "releases before 1.23.0 have no general commands"
)]
use crate::aci::flags::ResetOptions;

#[allow(
    unused_imports,
    reason = "releases before 1.23.0 have no general commands"
)]
use crate::wire::BoundedBytes;

vendor_command! {
    /// Reset the wireless binary in the given `mode`, with mode-specific
    /// `options`.
    aci_reset => AciReset {
        mode: u8,
        options: ResetOptions,
    }
}

vendor_command! {
    /// Read the version, options, and debug information of the wireless
    /// binary.
    aci_get_information => AciGetInformation {} -> AciInformation {
        version: [u32; 2],
        options: u32,
        debug_info: [u32; 3],
    }
}

vendor_command! {
    /// Write a value to the configuration data at `offset`.
    aci_write_config_data => AciWriteConfigData {
        offset: u8,
        value: &'a [u8],
    }
}

vendor_command! {
    /// Read the configuration data at `offset`.
    aci_read_config_data => AciReadConfigData { offset: u8 } -> AciConfigData {
        data: BoundedBytes<250>,
    }
}

#[cfg(all(test, any(feature = "fw_1_23_0", feature = "fw_1_24_0")))]
mod tests {
    use bt_hci::cmd::{Cmd, SyncCmd};
    use bt_hci::{FromHciBytes, WriteHci};

    use super::*;

    #[test]
    fn general_commands_use_the_top_of_the_vendor_range() {
        assert_eq!(AciReset::OPCODE.to_raw(), 0xFF00);
        let mut buffer = [0; 16];
        let mut writer = &mut buffer[..];
        AciReset::new(1, ResetOptions::LL_ONLY | ResetOptions::ENHANCED_ATT)
            .write_hci(&mut writer)
            .unwrap();
        let len = 16 - writer.len();
        assert_eq!(buffer[..len], [0x00, 0xFF, 5, 1, 0x01, 0x02, 0, 0]);

        let mut bytes = [0; 24];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let information =
            <AciGetInformation as SyncCmd>::Return::from_hci_bytes_complete(&bytes).unwrap();
        assert_eq!(information.version, [0x0302_0100, 0x0706_0504]);
        assert_eq!(information.options, 0x0B0A_0908);
        assert_eq!(information.debug_info[2], 0x1716_1514);
    }
}
