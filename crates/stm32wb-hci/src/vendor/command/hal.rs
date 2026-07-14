//! Vendor-specific HCI commands and types needed for those commands.

use crate::vendor::command::BoundedBytes;

hci_ranged! {
    /// Bluetooth RF channel accepted by [`HalStartTone`].
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct ToneChannel: u8 => 1 {
        minimum: 0,
        maximum: 39,
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
            offset: u8 => 1,
            value: &'a [u8] => {
                kind: counted_bytes,
                count: u8 => 1,
                max_len: 46,
            },
        };
        Completion = CommandComplete;
        Return = ();
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalReadConfigData(cgid = 0x0, cid = 0x0D) {
        Params = {
            param: ConfigParameter => 1,
        };
        Completion = CommandComplete;
        Return = HalReadConfigDataReturn {
            value: BoundedBytes<16> => {
                kind: trailing_bytes,
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
            freq_offset: u8 => 1,
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
            address: u8 => 1,
        };
        Completion = CommandComplete;
        Return = HalRadioRegisterValue {
            value: u8 => 1,
        };
    }
}

stm32wb_hci_macros::vendor_cmd! {
    HalWriteRadioReg(cgid = 0x0, cid = 0x31) {
        Params = {
            address: u8 => 1,
            value: u8 => 1,
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
            rf_channel: u8 => 1,
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
hci_enum! {
    /// Operation selected by [`HalEadEncryptDecrypt`].
    #[derive(Copy, Clone)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum EadMode: u8 => 1 {
        /// Encrypt the supplied advertising data.
        Encrypt = 0x00,
        /// Decrypt the supplied advertising data.
        Decrypt = 0x01,
    }
}

/// Low-level configuration parameters for the controller.
pub struct ConfigData {
    /// Offset of the element in the configuration data structure which has to be written.
    ///
    /// Values:
    ///- 0x00: CONFIG_DATA_PUBADDR_OFFSET;
    ///  Bluetooth public address; 6 bytes
    ///- 0x08: CONFIG_DATA_ER_OFFSET;
    ///  Encryption root key used to derive LTK (legacy) and CSRK; 16 bytes
    ///- 0x18: CONFIG_DATA_IR_OFFSET;
    ///  Identity root key used to derive DHK (legacy) and IRK; 16 bytes
    ///- 0x2E: CONFIG_DATA_RANDOM_ADDRESS_OFFSET;
    ///  Static Random Address; 6 bytes
    ///- 0x34: CONFIG_DATA_GAP_ADD_REC_NBR_OFFSET;
    ///  GAP service additional record number; 1 byte
    ///- 0x35: CONFIG_DATA_SC_KEY_TYPE_OFFSET;
    ///  Secure Connection key type (0: "normal", 1: "debug"); 1 byte
    ///- 0xB0: CONFIG_DATA_SMP_MODE_OFFSET;
    ///  SMP mode (0: "normal", 1: "bypass", 2: "no blacklist"); 1 byte
    ///- 0xC0: CONFIG_DATA_LL_SCAN_CHAN_MAP_OFFSET (only for STM32WB);
    ///  LL scan channel map (same format as Primary_Adv_Channel_Map); 1
    ///  byte
    ///- 0xC1: CONFIG_DATA_LL_BG_SCAN_MODE_OFFSET (only for STM32WB);
    ///  LL background scan mode (0: "BG scan disabled", 1: "BG scan
    ///  enabled"); 1 byte
    offset: u8,
    /// Length of the value to be written
    length: u8,
    /// Data to be written
    value_buf: [u8; ConfigData::MAX_LENGTH],
}

impl ConfigData {
    /// Maximum length needed to serialize the data.
    pub const MAX_LENGTH: usize = 0x2E;

    /// Serializes the data into the given buffer.
    ///
    /// Returns the number of valid bytes in the buffer.
    ///
    /// # Panics
    ///
    /// The buffer must be large enough to support the serialized data (at least
    /// [`MAX_LENGTH`](ConfigData::MAX_LENGTH) bytes).
    pub fn copy_into_slice(&self, bytes: &mut [u8]) -> usize {
        bytes[0] = self.offset;
        bytes[1] = self.length;

        let len = self.length as usize;
        bytes[2..2 + len].copy_from_slice(&self.value_buf[..len]);

        2 + len
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn public_address(addr: bt_hci::param::BdAddr) -> ConfigDataDiversifierBuilder {
        let mut data = Self {
            offset: 0,
            length: 6,
            value_buf: [0; Self::MAX_LENGTH],
        };

        data.value_buf[0..6].copy_from_slice(&addr.0);

        ConfigDataDiversifierBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn random_address(addr: bt_hci::param::BdAddr) -> ConfigDataDiversifierBuilder {
        let mut data = Self {
            offset: 0x2E,
            length: 6,
            value_buf: [0; Self::MAX_LENGTH],
        };

        data.value_buf[0..6].copy_from_slice(&addr.0);

        ConfigDataDiversifierBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn diversifier(d: u16) -> ConfigDataEncryptionRootBuilder {
        let mut data = Self {
            offset: 6,
            length: 2,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0..2].copy_from_slice(&d.to_le_bytes());

        ConfigDataEncryptionRootBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn encryption_root(key: &crate::types::EncryptionKey) -> ConfigDataIdentityRootBuilder {
        let mut data = Self {
            offset: 8,
            length: 16,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0..16].copy_from_slice(&key.0);

        ConfigDataIdentityRootBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn identity_root(key: &crate::types::EncryptionKey) -> ConfigDataLinkLayerOnlyBuilder {
        let mut data = Self {
            offset: 24,
            length: 16,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0..16].copy_from_slice(&key.0);
        ConfigDataLinkLayerOnlyBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn link_layer_only(ll_only: bool) -> ConfigDataRoleBuilder {
        let mut data = Self {
            offset: 40,
            length: 1,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0] = ll_only as u8;
        ConfigDataRoleBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn role(role: Role) -> ConfigDataCompleteBuilder {
        let mut data = Self {
            offset: 41,
            length: 1,
            value_buf: [0; Self::MAX_LENGTH],
        };
        data.value_buf[0] = role as u8;
        ConfigDataCompleteBuilder { data }
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataDiversifierBuilder {
    data: ConfigData,
}

impl ConfigDataDiversifierBuilder {
    /// Specify the diversifier and continue building.
    pub fn diversifier(mut self, d: u16) -> ConfigDataEncryptionRootBuilder {
        let len = self.data.length as usize;
        self.data.value_buf[len..2 + len].copy_from_slice(&d.to_le_bytes());
        self.data.length += 2;

        ConfigDataEncryptionRootBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes only the public address.
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataEncryptionRootBuilder {
    data: ConfigData,
}

impl ConfigDataEncryptionRootBuilder {
    /// Specify the encryption root and continue building.
    pub fn encryption_root(
        mut self,
        key: &crate::types::EncryptionKey,
    ) -> ConfigDataIdentityRootBuilder {
        let len = self.data.length as usize;
        self.data.value_buf[len..16 + len].copy_from_slice(&key.0);
        self.data.length += 16;

        ConfigDataIdentityRootBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the diversifier, and may include fields before it,
    /// but does not include any fields after it (including the encryption root).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataIdentityRootBuilder {
    data: ConfigData,
}

impl ConfigDataIdentityRootBuilder {
    /// Specify the identity root and continue building.
    pub fn identity_root(
        mut self,
        key: &crate::types::EncryptionKey,
    ) -> ConfigDataLinkLayerOnlyBuilder {
        let len = self.data.length as usize;
        self.data.value_buf[len..16 + len].copy_from_slice(&key.0);
        self.data.length += 16;

        ConfigDataLinkLayerOnlyBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the encryption root, and may include fields before
    /// it, but does not include any fields after it (including the identity root).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataLinkLayerOnlyBuilder {
    data: ConfigData,
}

impl ConfigDataLinkLayerOnlyBuilder {
    /// Specify whether to use the link layer only and continue building.
    pub fn link_layer_only(mut self, ll_only: bool) -> ConfigDataRoleBuilder {
        self.data.value_buf[self.data.length as usize] = ll_only as u8;
        self.data.length += 1;
        ConfigDataRoleBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the identity root, and may include fields before
    /// it, but does not include any fields after it (including the link layer only flag).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataRoleBuilder {
    data: ConfigData,
}

impl ConfigDataRoleBuilder {
    /// Specify the device role and continue building.
    pub fn role(mut self, role: Role) -> ConfigDataCompleteBuilder {
        self.data.value_buf[self.data.length as usize] = role as u8;
        self.data.length += 1;
        ConfigDataCompleteBuilder { data: self.data }
    }

    /// Build the [ConfigData] as-is. It includes the link layer only flag, and may include fields
    /// before it, but does not include any fields after it (including the role).
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Builder for [`ConfigData`].
pub struct ConfigDataCompleteBuilder {
    data: ConfigData,
}

impl ConfigDataCompleteBuilder {
    /// Build the [ConfigData] as-is. It includes the role field, and may include fields before it.
    pub fn build(self) -> ConfigData {
        self.data
    }
}

/// Roles that the server can adopt.
#[repr(u8)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Role {
    /// Peripheral and primary device.
    /// - Only one connection.
    /// - 6 KB of RAM retention.
    Peripheral6Kb = 1,

    /// Peripheral and primary device.
    /// - Only one connection.
    /// - 12 KB of RAM retention.
    Peripheral12Kb = 2,

    /// Primary device and peripheral
    /// - Up to 8 connections
    /// - 12 KB of RAM retention
    Primary12Kb = 3,

    /// Primary device and peripheral.
    /// - Simultaneous advertising and scanning
    /// - Up to 4 connections
    /// - This mode is available starting from BlueNRG-MS FW stack version 7.1.b
    SimultaneousAdvertisingScanning = 4,
}

hci_enum! {
    /// Configuration parameters that are readable by [`HalReadConfigData`].
    #[derive(Copy, Clone)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum ConfigParameter: u8 => 1 {
    /// Bluetooth public address.
    PublicAddress = 0,

    /// Bluetooth random address.
    RandomAddress = 0x2E,

    /// Diversifier used to derive CSRK (connection signature resolving key).
    Diversifier = 6,

    /// Encryption root key used to derive the LTK (long-term key) and CSRK (connection signature
    /// resolving key).
    EncryptionRoot = 8,

    /// Identity root key used to derive the LTK (long-term key) and CSRK (connection signature
    /// resolving key).
    IdentityRoot = 24,

    /// Switch on/off Link Layer only mode.
    LinkLayerOnly = 40,

    /// BlueNRG-MS roles and mode configuration.
        Role = 41,
    }
}

hci_enum! {
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

hci_bitflags! {
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

hci_bitflags! {
    /// HAL vendor events enabled in the controller.
    pub struct HalEventFlags: u32 => 4 {
        /// [HAL Scan Request Report](crate::vendor::event::VendorEvent::HalScanReqReport) event
        const SCAN_REQ_REPORT = 0x00000001;
    }
}
