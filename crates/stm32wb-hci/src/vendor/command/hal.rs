//! Vendor-specific HCI commands and types needed for those commands.

use crate::vendor::command::BoundedBytes;

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    ranged
    /// Bluetooth RF channel accepted by [`HalStartTone`].
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct ToneChannel: u8 => 1 {
        minimum: 0,
        maximum: 39,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Signed frequency-offset byte passed through to the RF tone generator.
    ///
    /// CubeWB documents the complete byte domain and assigns the physical
    /// interpretation to the radio test procedure.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct ToneFrequencyOffset: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Controller radio-register address.
    ///
    /// The HAL API deliberately treats the register map as opaque and accepts
    /// the complete byte domain.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct RadioRegisterAddress: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    open_scalar
    /// Opaque byte stored in a controller radio register.
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct RadioRegisterValue: u8 => 1;
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Configuration-data offsets accepted by the STM32WB write-config commands.
    ///
    /// Variants are firmware-gated at the CubeWB version that first documents
    /// the corresponding STM32WB field. Convert a value to `usize` to obtain
    /// the field's required payload length.
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ConfigWriteOffset: u8 => 1 {
        /// Public Bluetooth device address; six bytes.
        PublicAddress = 0x00,
        /// Encryption root key; sixteen bytes.
        EncryptionRootKey = 0x08,
        /// Identity root key; sixteen bytes.
        IdentityRootKey = 0x18,
        /// Random Bluetooth device address; six bytes.
        RandomAddress = 0x2E,
        /// Additional GAP service record count; one byte.
        #[cfg(since_fw_0_17_0)]
        GapAdditionalRecordCount = 0x34,
        /// Secure Connections key type; one byte.
        #[cfg(since_fw_0_17_0)]
        SecureConnectionsKeyType = 0x35,
        /// Security Manager Protocol mode; one byte.
        SmpMode = 0xB0,
        /// Link Layer scan-channel map; one byte.
        LinkLayerScanChannelMap = 0xC0,
        /// Link Layer background-scan mode; one byte.
        #[cfg(since_fw_0_16_0)]
        LinkLayerBackgroundScanMode = 0xC1,
        /// Link Layer resolvable-private-address mode; one byte.
        #[cfg(since_fw_0_21_0)]
        LinkLayerResolvablePrivateAddressMode = 0xC3,
        /// Link Layer maximum data-length extension; eight bytes.
        #[cfg(since_fw_0_21_0)]
        LinkLayerMaximumDataLengthExtension = 0xD1,
    }
}

impl From<ConfigWriteOffset> for usize {
    fn from(offset: ConfigWriteOffset) -> Self {
        match offset {
            ConfigWriteOffset::PublicAddress | ConfigWriteOffset::RandomAddress => 6,
            ConfigWriteOffset::EncryptionRootKey | ConfigWriteOffset::IdentityRootKey => 16,
            #[cfg(since_fw_0_17_0)]
            ConfigWriteOffset::GapAdditionalRecordCount
            | ConfigWriteOffset::SecureConnectionsKeyType => 1,
            ConfigWriteOffset::SmpMode | ConfigWriteOffset::LinkLayerScanChannelMap => 1,
            #[cfg(since_fw_0_16_0)]
            ConfigWriteOffset::LinkLayerBackgroundScanMode => 1,
            #[cfg(since_fw_0_21_0)]
            ConfigWriteOffset::LinkLayerResolvablePrivateAddressMode => 1,
            #[cfg(since_fw_0_21_0)]
            ConfigWriteOffset::LinkLayerMaximumDataLengthExtension => 8,
        }
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Configuration-data offsets accepted by the STM32WB read-config commands.
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ConfigReadOffset: u8 => 1 {
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

impl crate::vendor::command::HciDecodeField<16> for [u16; 8] {
    fn from_hci_field(bytes: &[u8; 16]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(core::array::from_fn(|index| {
            let offset = index * 2;
            u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
        }))
    }
}

#[cfg(before_fw_0_23_0)]
stm32wb_hci_macros::vendor_cmd! {
    HalGetFirmwareRevision(cgid = 0x0, cid = 0x00) {
        Params = ();
        Completion = CommandComplete;
        Return = HalFirmwareRevision {
            revision: u16 => 2,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalWriteConfigData(cgid = 0x0, cid = 0x0C) {
        Params<'a> = {
            offset: ConfigWriteOffset => 1,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 46,
            },
        };
        Constraints = {
            len_eq(value, offset);
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalReadConfigData(cgid = 0x0, cid = 0x0D) {
        Params = {
            offset: ConfigReadOffset => 1,
        };
        Completion = CommandComplete;
        Return = HalReadConfigDataReturn {
            value: BoundedBytes<16> => {
                kind: counted_bytes,
                count: u8 => 1,
                min_len: 1,
                max_len: 16,
            },
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalSetTxPowerLevel(cgid = 0x0, cid = 0x0F) {
        Params = {
            high_power_mode: bool => 1,
            power_level: PowerLevel => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalGetTxTestPacketCount(cgid = 0x0, cid = 0x14) {
        Params = ();
        Completion = CommandComplete;
        Return = HalTxTestPacketCount {
            packet_count: u32 => 4,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalStartTone(cgid = 0x0, cid = 0x15) {
        Params = {
            channel: ToneChannel => 1,
            freq_offset: ToneFrequencyOffset => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalStopTone(cgid = 0x0, cid = 0x16) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalGetLinkStatus(cgid = 0x0, cid = 0x17) {
        Params = ();
        Completion = CommandComplete;
        Return = HalLinkStatusRaw {
            link_status: [u8; 8] => 8,
            link_connection_handles: [u16; 8] => 16,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalSetRadioActivityMask(cgid = 0x0, cid = 0x18) {
        Params = {
            mask: RadioActivityFlags => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalGetAnchorPeriod(cgid = 0x0, cid = 0x19) {
        Params = ();
        Completion = CommandComplete;
        Return = HalAnchorPeriodRaw {
            anchor_interval: u32 => 4,
            max_slot: u32 => 4,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalSetEventMask(cgid = 0x0, cid = 0x1A) {
        Params = {
            mask: HalEventFlags => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(before_fw_0_23_0)]
stm32wb_hci_macros::vendor_cmd! {
    HalGetPmDebugInfo(cgid = 0x0, cid = 0x1C) {
        Params = ();
        Completion = CommandComplete;
        Return = HalPmDebugInfo {
            tx: u8 => 1,
            rx: u8 => 1,
            mblocks: u8 => 1,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalSetPeripheralLatency(cgid = 0x0, cid = 0x20) {
        Params = {
            enabled: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalReadRssi(cgid = 0x0, cid = 0x22) {
        Params = ();
        Completion = CommandComplete;
        Return = HalRssi {
            value: u8 => 1,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalReadRadioReg(cgid = 0x0, cid = 0x30) {
        Params = {
            address: RadioRegisterAddress => 1,
        };
        Completion = CommandComplete;
        Return = HalRadioRegisterValue {
            value: RadioRegisterValue => 1,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalWriteRadioReg(cgid = 0x0, cid = 0x31) {
        Params = {
            address: RadioRegisterAddress => 1,
            value: RadioRegisterValue => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalReadRawRssi(cgid = 0x0, cid = 0x32) {
        Params = ();
        Completion = CommandComplete;
        Return = HalRawRssi {
            value: [u8; 3] => 3,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalRxStart(cgid = 0x0, cid = 0x33) {
        Params = {
            rf_channel: ToneChannel => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalRxStop(cgid = 0x0, cid = 0x34) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(before_fw_0_23_0)]
stm32wb_hci_macros::vendor_cmd! {
    HalStackReset(cgid = 0x0, cid = 0x3B) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(since_fw_0_20_0)]
stm32wb_hci_macros::vendor_cmd! {
    HalEadEncryptDecrypt(cgid = 0x0, cid = 0x2F) {
        Params<'a> = {
            mode: EadMode => 1,
            key: &'a [u8; 16] => 16,
            iv: &'a [u8; 8] => 8,
            data: &'a [u8] => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 228,
            },
        };
        Constraints = {
            implies_len_at_least(mode, EadMode::Decrypt, data, 9);
        };
        Completion = CommandComplete;
        Return = HalEadEncryptDecryptReturn {
            data: BoundedBytes<249> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 249,
            },
        };
    }
}

#[cfg(since_fw_0_20_0)]
stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Operation selected by [`HalEadEncryptDecrypt`].
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum EadMode: u8 => 1 {
        /// Encrypt the supplied advertising data.
        Encrypt = 0x00,
        /// Decrypt the supplied advertising data.
        Decrypt = 0x01,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    closed
    /// Transmitter power levels available for the system.
    ///
    /// STM32WB5x uses single byte parameter for PA level.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum PowerLevel: u8 => 1 {
    /// -40 dBm.
    Minus40dBm = 0x00,

    /// -20.85 dBm.
    Minus20_85dBm = 0x01,

    /// -19.75 dBm.
    Minus19_75dBm = 0x02,

    /// -18.85 dBm.
    Minus18_85dBm = 0x03,

    /// 17.6 dBm.
    Minus17_6dBm = 0x04,

    /// -16.5 dBm.
    Minus16_5dBm = 0x05,

    /// -15.25 dBm.
    Minus15_25dBm = 0x06,

    /// -14.1 dBm.
    Minus14_1dBm = 0x07,

    /// -13.15 dBm.
    Minus13_15dBm = 0x08,

    /// -12.05 dBm.
    Minus12_05dBm = 0x09,

    /// -10.9 dBm.
    Minus10_9dBm = 0x0A,

    /// -9.9 dBm.
    Minus9_9dBm = 0x0B,

    /// -8.85 dBm.
    Minus8_85dBm = 0x0C,

    /// -7.8 dBm.
    Minus7_8dBm = 0x0D,

    /// -6.9 dBm.
    Minus6_9dBm = 0x0E,

    /// -5.9 dBm.
    Minus5_9dBm = 0x0F,

    /// -4.95 dBm.
    Minus4_95dBm = 0x10,

    /// -4 dBm.
    Minus4dBm = 0x11,

    /// -3.15 dBm.
    Minus3_15dBm = 0x12,

    /// -2.45 dBm.
    Minus2_45dBm = 0x13,

    /// -1.8 dBm.
    Minus1_8dBm = 0x14,

    /// -1.3 dBm.
    Minus1_3dBm = 0x15,

    /// -0.85 dBm.
    Minus0_85dBm = 0x16,

    /// -0.5 dBm.
    Minus0_5dBm = 0x17,

    /// -0.15 dBm.
    Minus0_15dBm = 0x18,

    /// 0 dBm.
    ZerodBm = 0x19,

    /// 1 dBm.
    Plus1dBm = 0x1A,

    /// 2 dBm.
    Plus2dBm = 0x1B,

    /// 3 dBm.
    Plus3dBm = 0x1C,

    /// 4 dBm.
    Plus4dBm = 0x1D,

    /// 5 dBm.
    Plus5dBm = 0x1E,

    /// 6 dBm.
        Plus6dBm = 0x1F,
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// Radio activities reported through the HAL activity mask.
    pub struct RadioActivityFlags: u16 => 2 {
        /// Idle
        const IDLE = 0x0001;
        /// Advertising
        const ADVERTISING = 0x0002;
        /// Peripheral connection
        const PERIPHERAL_CONN = 0x0004;
        /// Scanning
        const SCANNING = 0x0008;
        /// Central connection
        const CENTRAL_CONN = 0x0020;
        /// Tx test mode
        const TX_TEST = 0x0040;
        /// Rx test mode
        const RX_TEST = 0x0080;
    }
}

stm32wb_hci_macros::wire_type! {
    adapters: [command];
    bitflags
    /// HAL vendor events enabled in the controller.
    pub struct HalEventFlags: u32 => 4 {
        /// [HAL Scan Request Report](crate::vendor::event::VendorEvent::HalScanReqReport) event
        const SCAN_REQ_REPORT = 0x00000001;
    }
}
