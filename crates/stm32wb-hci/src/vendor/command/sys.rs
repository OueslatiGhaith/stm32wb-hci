//! System-level commands introduced by STM32CubeWB 1.23.

use crate::vendor::command::BoundedBytes;
pub use crate::vendor::command::hal::{ConfigReadOffset, ConfigWriteOffset};

hci_enum! {
    /// Reset behavior selected by [`SysReset`].
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SysResetMode: u8 => 1 {
        /// Reset without changing BLE stack options.
        NoOptionsChange = 0x00,
        /// Reset and apply the supplied BLE stack options.
        WithOptionsChange = 0x01,
    }
}

hci_bitflags! {
    /// Optional BLE stack features selected by [`SysReset`].
    pub struct SysResetOptions: u32 => 4 {
        /// Run the Link Layer without the host stack.
        const LL_ONLY = 0x0000_0001;
        /// Disable the Service Changed characteristic declaration.
        const NO_SERVICE_CHANGE_DESCRIPTION = 0x0000_0002;
        /// Make the Device Name characteristic read-only.
        const DEVICE_NAME_READ_ONLY = 0x0000_0004;
        /// Enable extended advertising support.
        const EXTENDED_ADVERTISING = 0x0000_0008;
        /// Enable Channel Selection Algorithm #2.
        const CHANNEL_SELECTION_ALGORITHM_2 = 0x0000_0010;
        /// Use the reduced GATT database representation in nonvolatile memory.
        const REDUCED_GATT_DATABASE_IN_NVM = 0x0000_0020;
        /// Enable GATT caching support.
        const GATT_CACHING = 0x0000_0040;
        /// Enable LE Power Class 1 support.
        const LE_POWER_CLASS_1 = 0x0000_0080;
        /// Make the Appearance characteristic writable.
        const APPEARANCE_WRITABLE = 0x0000_0100;
        /// Enable Enhanced ATT support.
        const ENHANCED_ATT = 0x0000_0200;
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysReset(cgid = 0x6, cid = 0x00) {
        Params = {
            mode: SysResetMode => 1,
            options: SysResetOptions => 4,
        };
        Constraints = {
            implies_eq(
                mode,
                SysResetMode::NoOptionsChange,
                options,
                SysResetOptions::empty()
            );
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
            options: SysResetOptions => 4,
            debug_info: [u8; 12] => 12,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysWriteConfigData(cgid = 0x6, cid = 0x02) {
        Params<'a> = {
            offset: ConfigWriteOffset => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 253,
            },
        };
        Constraints = {
            len_eq(data, offset);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysReadConfigData(cgid = 0x6, cid = 0x03) {
        Params = {
            offset: ConfigReadOffset => 1,
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

impl SysReadConfigDataReturn {
    /// Configuration bytes returned by the controller.
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }
}
