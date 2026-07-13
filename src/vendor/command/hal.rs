//! Vendor-specific HCI commands and types needed for those commands.

extern crate byteorder;

use byteorder::{ByteOrder, LittleEndian};

use crate::{
    BadStatusError, Status,
    vendor::{
        command::BoundedBytes,
        event::command::{HalConfigData, HalConfigParameter},
    },
};

impl TryFrom<BoundedBytes<16>> for HalConfigData {
    type Error = Error;

    fn try_from(value: BoundedBytes<16>) -> Result<Self, Self::Error> {
        let bytes = value.as_slice();
        let value = match bytes.len() {
            1 => HalConfigParameter::Byte(bytes[0]),
            2 => HalConfigParameter::Diversifier(LittleEndian::read_u16(bytes)),
            6 => {
                let mut address = [0; 6];
                address.copy_from_slice(bytes);
                HalConfigParameter::PublicAddress(crate::BdAddr(address))
            }
            16 => {
                let mut key = [0; 16];
                key.copy_from_slice(bytes);
                HalConfigParameter::EncryptionKey(crate::host::EncryptionKey(key))
            }
            other => {
                return Err(crate::event::Error::Vendor(
                    crate::vendor::event::VendorError::BadConfigParameterLength(other),
                )
                .into());
            }
        };
        Ok(Self { value })
    }
}

impl crate::vendor::command::HciDecodeField<16> for [u16; 8] {
    fn from_hci_field(bytes: &[u8; 16]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(core::array::from_fn(|index| {
            LittleEndian::read_u16(&bytes[index * 2..index * 2 + 2])
        }))
    }
}

impl crate::vendor::command::HciDecodeField<44> for [u16; 22] {
    fn from_hci_field(bytes: &[u8; 44]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(core::array::from_fn(|index| {
            LittleEndian::read_u16(&bytes[index * 2..index * 2 + 2])
        }))
    }
}

vendor_cmd! {
    HalGetFirmwareRevision(cgid = 0x0, cid = 0x00) {
        Params = ();
        Completion = CommandComplete;
        Return = HalFirmwareRevision {
            revision: u16 => 2,
        };
    }
}

vendor_cmd! {
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

vendor_cmd! {
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

vendor_cmd! {
    HalSetTxPowerLevel(cgid = 0x0, cid = 0x0F) {
        Params = {
            high_power_mode: bool => 1,
            power_level: PowerLevel => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalGetTxTestPacketCount(cgid = 0x0, cid = 0x14) {
        Params = ();
        Completion = CommandComplete;
        Return = HalTxTestPacketCount {
            packet_count: u32 => 4,
        };
    }
}

vendor_cmd! {
    HalStartTone(cgid = 0x0, cid = 0x15) {
        Params = {
            channel: u8 => 1,
            freq_offset: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalStopTone(cgid = 0x0, cid = 0x16) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalGetLinkStatus(cgid = 0x0, cid = 0x17) {
        Params = ();
        Completion = CommandComplete;
        Return = HalLinkStatusRaw {
            link_status: [u8; 8] => 8,
            link_connection_handles: [u16; 8] => 16,
        };
    }
}

vendor_cmd! {
    HalSetRadioActivityMask(cgid = 0x0, cid = 0x18) {
        Params = {
            mask: RadioActivityFlags => 2,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalGetAnchorPeriod(cgid = 0x0, cid = 0x19) {
        Params = ();
        Completion = CommandComplete;
        Return = HalAnchorPeriodRaw {
            anchor_interval: u32 => 4,
            max_slot: u32 => 4,
        };
    }
}

vendor_cmd! {
    HalSetEventMask(cgid = 0x0, cid = 0x1A) {
        Params = {
            mask: HalEventFlags => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
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

vendor_cmd! {
    HalSetPeripheralLatency(cgid = 0x0, cid = 0x20) {
        Params = {
            enabled: bool => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalReadRssi(cgid = 0x0, cid = 0x22) {
        Params = ();
        Completion = CommandComplete;
        Return = HalRssi {
            value: u8 => 1,
        };
    }
}

vendor_cmd! {
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

vendor_cmd! {
    HalWriteRadioReg(cgid = 0x0, cid = 0x31) {
        Params = {
            address: u8 => 1,
            value: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalReadRawRssi(cgid = 0x0, cid = 0x32) {
        Params = ();
        Completion = CommandComplete;
        Return = HalRawRssi {
            value: [u8; 3] => 3,
        };
    }
}

vendor_cmd! {
    HalRxStart(cgid = 0x0, cid = 0x33) {
        Params = {
            rf_channel: u8 => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalRxStop(cgid = 0x0, cid = 0x34) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    HalStackReset(cgid = 0x0, cid = 0x3B) {
        Params = ();
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    HalGetLinkStatusV2(cgid = 0x0, cid = 0x1B) {
        Params = ();
        Completion = CommandComplete;
        Return = HalLinkStatusV2Raw {
            link_status: [u8; 22] => 22,
            link_connection_handles: [u16; 22] => 44,
        };
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    HalSetSyncEventConfig(cgid = 0x0, cid = 0x21) {
        Params = {
            group_id: u8 => 1,
            enable_sync: bool => 1,
            enable_cb_trigger: bool => 1,
            trigger_source: SyncTriggerSource => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
    HalContinuousTxStart(cgid = 0x0, cid = 0x2E) {
        Params = {
            rf_channel: u8 => 1,
            phy: ContinuousTxPhy => 1,
            pattern: ContinuousTxPattern => 1,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

#[cfg(after_fw_0_17_1)]
vendor_cmd! {
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
            data: BoundedBytes<237> => {
                kind: counted_bytes,
                count: u16 => 2,
                max_len: 237,
            },
        };
    }
}

/// Potential errors from parameter validation.
///
/// Before some commands are sent to the controller, the parameters are validated. This type
/// enumerates the potential validation errors. Must be specialized on the types of communication
/// errors.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// For the [Start Tone](HalStartTone) command, the channel was greater than the maximum
    /// allowed channel (39). The invalid channel is returned.
    InvalidChannel(u8),

    /// Event Parsing Error
    ParseError(crate::event::Error),

    /// A variable-length parameter exceeds the command's wire bounds.
    InvalidParameterLength(crate::vendor::command::HciLengthError),

    /// An error occurred during execution of the command
    HciError(Status),

    /// An error occurred during execution of the command
    UnknownHciError(u8),

    /// An internal error occurred during execution of the controller. This is a bug.
    IoError,
}

impl From<crate::vendor::command::HciLengthError> for Error {
    fn from(error: crate::vendor::command::HciLengthError) -> Self {
        Self::InvalidParameterLength(error)
    }
}

impl<T> From<bt_hci::cmd::Error<T>> for Error {
    fn from(err: bt_hci::cmd::Error<T>) -> Self {
        match err {
            bt_hci::cmd::Error::Io(_) => Self::IoError,
            bt_hci::cmd::Error::Hci(err) => match Status::try_from(err.to_status().into_inner()) {
                Ok(status) => Self::HciError(status),
                Err(BadStatusError::BadValue(status)) => Self::UnknownHciError(status),
            },
        }
    }
}

impl From<crate::event::Error> for Error {
    fn from(e: crate::event::Error) -> Self {
        Self::ParseError(e)
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
    pub fn public_address(addr: crate::BdAddr) -> ConfigDataDiversifierBuilder {
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
    pub fn random_address(addr: crate::BdAddr) -> ConfigDataDiversifierBuilder {
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
        LittleEndian::write_u16(&mut data.value_buf[0..2], d);

        ConfigDataEncryptionRootBuilder { data }
    }

    /// Builder for [ConfigData].
    ///
    /// The controller allows us to write any _contiguous_ portion of the [ConfigData] structure in
    /// [`write_config_data`](HalWriteConfigData).  The builder associated functions allow
    /// us to start with any field, and the returned builder allows only either chaining the next
    /// field or building the structure to write.
    pub fn encryption_root(key: &crate::host::EncryptionKey) -> ConfigDataIdentityRootBuilder {
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
    pub fn identity_root(key: &crate::host::EncryptionKey) -> ConfigDataLinkLayerOnlyBuilder {
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
        LittleEndian::write_u16(&mut self.data.value_buf[len..2 + len], d);
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
        key: &crate::host::EncryptionKey,
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
        key: &crate::host::EncryptionKey,
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

/// Configuration parameters that are readable by the
/// [`read_config_data`](HalReadConfigData) command.
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigParameter {
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

impl crate::vendor::command::HciEncodeField<1> for ConfigParameter {
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

/// Transmitter power levels available for the system.
///
/// STM32WB5x uses single byte parameter for PA level.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerLevel {
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

impl crate::vendor::command::HciEncodeField<1> for PowerLevel {
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

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct RadioActivityFlags: u16 {
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

#[cfg(feature = "defmt")]
defmt::bitflags! {
    pub struct RadioActivityFlags: u16 {
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

impl crate::vendor::command::HciEncodeField<2> for RadioActivityFlags {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.bits().write_hci_field(writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        self.bits().write_hci_field_async(writer).await
    }
}

#[cfg(not(feature = "defmt"))]
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct HalEventFlags: u32 {
        /// [HAL Scan Request Report](crate::vendor::event::VendorEvent::HalScanReqReport) event
        const SCAN_REQ_REPORT = 0x00000001;
    }
}

#[cfg(feature = "defmt")]
defmt::bitflags! {
    pub struct HalEventFlags: u32 {
        /// [HAL Scan Request Report](crate::vendor::event::VendorEvent::HalScanReqReport) event
        const SCAN_REQ_REPORT = 0x00000001;
    }
}

impl crate::vendor::command::HciEncodeField<4> for HalEventFlags {
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.bits().write_hci_field(writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        self.bits().write_hci_field_async(writer).await
    }
}

#[cfg(after_fw_0_17_1)]
/// Return value for [get_link_status_v2](HalGetLinkStatusV2).
pub struct HalLinkStatusV2 {
    /// Link statuses for up to 20 links + 2 ISO streams.
    pub link_status: [u8; 22],
    /// Connection handles for each link (0 if not connected).
    pub link_connection_handles: [u16; 22],
}

#[cfg_attr(
    after_fw_0_17_1,
    doc = "Trigger source for [set_sync_event_config](HalSetSyncEventConfig)."
)]
#[cfg_attr(
    not(after_fw_0_17_1),
    doc = "Trigger source for `set_sync_event_config`."
)]
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SyncTriggerSource {
    Cig = 0x00,
    Big = 0x01,
}

#[cfg_attr(
    after_fw_0_17_1,
    doc = "PHY for [continuous_tx_start](HalContinuousTxStart)."
)]
#[cfg_attr(not(after_fw_0_17_1), doc = "PHY for `continuous_tx_start`.")]
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContinuousTxPhy {
    Le1M = 0x01,
    Le2M = 0x02,
}

#[cfg_attr(
    after_fw_0_17_1,
    doc = "Data pattern for [continuous_tx_start](HalContinuousTxStart)."
)]
#[cfg_attr(not(after_fw_0_17_1), doc = "Data pattern for `continuous_tx_start`.")]
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContinuousTxPattern {
    Prbs9 = 0x00,
    Alternating11110000 = 0x01,
    Alternating10101010 = 0x02,
    Prbs15 = 0x03,
    AllOnes = 0x04,
    AllZeros = 0x05,
    Alternating00001111 = 0x06,
    Alternating0101 = 0x07,
}

#[cfg_attr(
    after_fw_0_17_1,
    doc = "Mode for [ead_encrypt_decrypt](HalEadEncryptDecrypt)."
)]
#[cfg_attr(not(after_fw_0_17_1), doc = "Mode for `ead_encrypt_decrypt`.")]
#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum EadMode {
    Encrypt = 0x00,
    Decrypt = 0x01,
}

macro_rules! impl_u8_hci_field {
    ($type:ty) => {
        impl crate::vendor::command::HciEncodeField<1> for $type {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&[*self as u8])
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&[*self as u8]).await
            }
        }
    };
}

impl_u8_hci_field!(SyncTriggerSource);
impl_u8_hci_field!(ContinuousTxPhy);
impl_u8_hci_field!(ContinuousTxPattern);
impl_u8_hci_field!(EadMode);

#[cfg(after_fw_0_17_1)]
/// Parameters for [ead_encrypt_decrypt](HalEadEncryptDecrypt).
pub struct EadParams {
    /// EAD operation mode.
    pub mode: EadMode,
    /// Session key (16 bytes, little-endian).
    pub key: [u8; 16],
    /// Initialization vector (8 bytes, little-endian).
    pub iv: [u8; 8],
    /// Input data (up to 248 bytes).
    pub data: [u8; 248],
    /// Length of valid data in `data`.
    pub data_len: usize,
}

#[cfg(after_fw_0_17_1)]
/// Return value for [ead_encrypt_decrypt](HalEadEncryptDecrypt).
pub struct HalEadResult {
    /// Result data.
    pub data: [u8; 248],
    /// Length of valid data in `data`.
    pub data_len: usize,
}
