//! System commands (ACI_RESET, ACI_GET_INFORMATION, ACI_WRITE_CONFIG_DATA, ACI_READ_CONFIG_DATA).

#[cfg(since_fw_0_17_1)]
use crate::vendor::command::BoundedBytes;
hci_enum! {
    #[derive(Copy, Clone)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SysResetMode: u8 => 1 {
        /// Reset without BLE stack options change.
        NoOptionsChange = 0x00,
        /// Reset with BLE stack option changes.
        WithOptionsChange = 0x01,
    }
}

#[cfg(since_fw_0_17_1)]
vendor_cmd! {
    SysReset(cgid = 0x6, cid = 0x00) {
        Params = {
            mode: SysResetMode => 1,
            options: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_0_17_1)]
vendor_cmd! {
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

#[cfg(since_fw_0_17_1)]
vendor_cmd! {
    SysWriteConfigData(cgid = 0x6, cid = 0x02) {
        Params<'a> = {
            offset: u8 => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 32,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_0_17_1)]
vendor_cmd! {
    SysReadConfigData(cgid = 0x6, cid = 0x03) {
        Params = {
            offset: u8 => 1,
        };
        Completion = CommandComplete;
        Return = SysReadConfigDataReturn {
            data: BoundedBytes<32> => {
                kind: trailing_bytes,
                min_len: 0,
                max_len: 32,
            },
        };
    }
}
