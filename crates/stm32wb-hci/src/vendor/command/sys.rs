//! System-level commands introduced by STM32CubeWB 1.23.

use crate::vendor::command::BoundedBytes;

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

hci_enum! {
    /// Configuration-data offsets accepted by [`SysWriteConfigData`].
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SysWritableConfigOffset: u8 => 1 {
        /// Public Bluetooth device address.
        PublicAddress = 0x00,
        /// Encryption root key.
        EncryptionRootKey = 0x08,
        /// Identity root key.
        IdentityRootKey = 0x18,
        /// Random Bluetooth device address.
        RandomAddress = 0x2E,
        /// Additional GAP service record count.
        GapAdditionalRecordCount = 0x34,
        /// Secure Connections key type.
        SecureConnectionsKeyType = 0x35,
        /// Security Manager Protocol mode.
        SmpMode = 0xB0,
        /// Link Layer scan-channel map.
        LinkLayerScanChannelMap = 0xC0,
        /// Link Layer background-scan mode.
        LinkLayerBackgroundScanMode = 0xC1,
        /// Link Layer resolvable-private-address mode.
        LinkLayerResolvablePrivateAddressMode = 0xC3,
        /// Link Layer maximum data-length extension.
        LinkLayerMaximumDataLengthExtension = 0xD1,
    }
}

hci_enum! {
    /// Configuration-data offsets accepted by [`SysReadConfigData`].
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum SysReadableConfigOffset: u8 => 1 {
        /// Public Bluetooth device address.
        PublicAddress = 0x00,
        /// Encryption root key.
        EncryptionRootKey = 0x08,
        /// Identity root key.
        IdentityRootKey = 0x18,
        /// Random Bluetooth device address.
        RandomAddress = 0x2E,
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
            offset: SysWritableConfigOffset => 1,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 253,
            },
        };
        Constraints = {
            implies_len_eq(offset, SysWritableConfigOffset::PublicAddress, data, 6);
            implies_len_eq(offset, SysWritableConfigOffset::EncryptionRootKey, data, 16);
            implies_len_eq(offset, SysWritableConfigOffset::IdentityRootKey, data, 16);
            implies_len_eq(offset, SysWritableConfigOffset::RandomAddress, data, 6);
            implies_len_eq(offset, SysWritableConfigOffset::GapAdditionalRecordCount, data, 1);
            implies_len_eq(offset, SysWritableConfigOffset::SecureConnectionsKeyType, data, 1);
            implies_len_eq(offset, SysWritableConfigOffset::SmpMode, data, 1);
            implies_len_eq(offset, SysWritableConfigOffset::LinkLayerScanChannelMap, data, 1);
            implies_len_eq(offset, SysWritableConfigOffset::LinkLayerBackgroundScanMode, data, 1);
            implies_len_eq(
                offset,
                SysWritableConfigOffset::LinkLayerResolvablePrivateAddressMode,
                data,
                1
            );
            implies_len_eq(
                offset,
                SysWritableConfigOffset::LinkLayerMaximumDataLengthExtension,
                data,
                8
            );
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    SysReadConfigData(cgid = 0x6, cid = 0x03) {
        Params = {
            offset: SysReadableConfigOffset => 1,
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
