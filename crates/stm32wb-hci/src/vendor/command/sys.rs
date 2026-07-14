//! System-level commands introduced by STM32CubeWB 1.23.

use crate::vendor::command::BoundedBytes;

hci_enum! {
    /// Reset behavior selected by [`SysReset`].
    #[derive(Copy, Clone)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SysResetMode: u8 => 1 {
        /// Reset without changing BLE stack options.
        NoOptionsChange = 0x00,
        /// Reset and apply the supplied BLE stack options.
        WithOptionsChange = 0x01,
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysReset(cgid = 0x6, cid = 0x00) {
        Params = {
            mode: SysResetMode => 1,
            options: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysGetInformation(cgid = 0x6, cid = 0x01) {
        Params = ();
        Completion = CommandComplete;
        Return = SysGetInformationReturn {
            version: [u8; 8] => 8,
            options: u32 => 4,
            debug_info: [u8; 12] => 12,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysWriteConfigData(cgid = 0x6, cid = 0x02) {
        Params<'a> = {
            offset: u8 => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 253,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysReadConfigData(cgid = 0x6, cid = 0x03) {
        Params = {
            offset: u8 => 1,
        };
        Completion = CommandComplete;
        Return = SysReadConfigDataReturn {
            data: BoundedBytes<250> => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 250,
            },
        };
    }
}
