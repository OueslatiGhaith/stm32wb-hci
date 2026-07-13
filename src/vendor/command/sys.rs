//! System commands (ACI_RESET, ACI_GET_INFORMATION, ACI_WRITE_CONFIG_DATA, ACI_READ_CONFIG_DATA).

#[cfg(after_fw_0_17_1)]
use crate::vendor::command::BoundedBytes;
#[cfg_attr(after_fw_0_17_1, doc = "Mode for [sys_reset](SysReset).")]
#[cfg_attr(not(after_fw_0_17_1), doc = "Mode for `sys_reset`.")]
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SysResetMode {
    /// Reset without BLE stack options change.
    NoOptionsChange = 0x00,
    /// Reset with BLE stack option changes.
    WithOptionsChange = 0x01,
}

impl crate::vendor::command::HciEncodeField<1> for SysResetMode {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8])
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[*self as u8]).await
    }
}

#[cfg(after_fw_0_17_1)]
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

#[cfg(after_fw_0_17_1)]
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

#[cfg(after_fw_0_17_1)]
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

#[cfg(after_fw_0_17_1)]
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
