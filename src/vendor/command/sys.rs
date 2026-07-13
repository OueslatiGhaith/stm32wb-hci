//! System commands (ACI_RESET, ACI_GET_INFORMATION, ACI_WRITE_CONFIG_DATA, ACI_READ_CONFIG_DATA).

use crate::vendor::command::BoundedBytes;
#[cfg(after_fw_0_17_1)]
use crate::{BadStatusError, Status};
#[cfg(after_fw_0_17_1)]
use bt_hci::{cmd::SyncCmd, controller::ControllerCmdSync};

/// System-level commands.
pub trait SysCommands {
    /// These ACI general commands first appear in STM32CubeWB v1.24.0.  The
    /// descriptor types remain compiled below so blanket controller bounds do
    /// not change shape, but the public API must not be exposed for the older
    /// wireless firmware releases supported by this crate.
    #[cfg(after_fw_0_17_1)]
    /// Reset the BLE stack.
    async fn sys_reset(&self, mode: SysResetMode, options: u32) -> Result<(), Error>;

    /// Read local ACI information.
    #[cfg(after_fw_0_17_1)]
    async fn get_information(&self) -> Result<SysInformation, Error>;

    /// Write a value to the configure data structure.
    #[cfg(after_fw_0_17_1)]
    async fn sys_write_config_data(&self, offset: u8, data: &[u8]) -> Result<(), Error>;

    /// Read a value from the configure data structure.
    #[cfg(after_fw_0_17_1)]
    async fn sys_read_config_data(&self, offset: u8) -> Result<SysConfigData, Error>;
}

/// Mode for [sys_reset](SysCommands::sys_reset).
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

/// Return value for [get_information](SysCommands::get_information).
#[cfg(after_fw_0_17_1)]
pub struct SysInformation {
    /// BLE stack version (8 bytes).
    pub version: [u8; 8],
    /// BLE stack options bitmask.
    pub options: u32,
    /// BLE stack debug information (12 bytes).
    pub debug_info: [u8; 12],
}

/// Return value for [sys_read_config_data](SysCommands::sys_read_config_data).
#[cfg(after_fw_0_17_1)]
pub struct SysConfigData {
    /// Raw config data bytes.
    pub data: [u8; 32],
    pub len: usize,
}

/// Error type for system commands.
#[cfg(after_fw_0_17_1)]
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    HciError(Status),
    UnknownHciError(u8),
    InvalidParameterLength(crate::vendor::command::HciLengthError),
    IoError,
}

#[cfg(after_fw_0_17_1)]
impl From<crate::vendor::command::HciLengthError> for Error {
    fn from(error: crate::vendor::command::HciLengthError) -> Self {
        Self::InvalidParameterLength(error)
    }
}

#[cfg(after_fw_0_17_1)]
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

vendor_cmd! {
    SysReset(SYS_RESET) {
        Params = {
            mode: SysResetMode => 1,
            options: u32 => 4,
        };
        Completion = CommandComplete;
        Return = ();
    }
}

vendor_cmd! {
    SysGetInformation(SYS_GET_INFORMATION) {
        Params = ();
        Completion = CommandComplete;
        Return = SysGetInformationReturn {
            version: [u8; 8] => 8,
            options: u32 => 4,
            debug_info: [u8; 12] => 12,
        };
    }
}

vendor_cmd! {
    SysWriteConfigData(SYS_WRITE_CONFIG_DATA) {
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

vendor_cmd! {
    SysReadConfigData(SYS_READ_CONFIG_DATA) {
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

#[cfg(after_fw_0_17_1)]
impl<T> SysCommands for T
where
    T: ControllerCmdSync<SysReset>
        + ControllerCmdSync<SysGetInformation>
        + for<'t> ControllerCmdSync<SysWriteConfigData<'t>>
        + ControllerCmdSync<SysReadConfigData>,
{
    async fn sys_reset(&self, mode: SysResetMode, options: u32) -> Result<(), Error> {
        SysReset::new(mode, options)
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn get_information(&self) -> Result<SysInformation, Error> {
        let info = SysGetInformation::new()
            .exec(self)
            .await
            .map_err(Error::from)?;
        Ok(SysInformation {
            version: info.version,
            options: info.options,
            debug_info: info.debug_info,
        })
    }

    async fn sys_write_config_data(&self, offset: u8, data: &[u8]) -> Result<(), Error> {
        SysWriteConfigData::try_new(offset, data)?
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn sys_read_config_data(&self, offset: u8) -> Result<SysConfigData, Error> {
        let value = SysReadConfigData::new(offset)
            .exec(self)
            .await
            .map_err(Error::from)?
            .data;
        let len = value.as_slice().len();
        let mut data = [0; 32];
        data[..len].copy_from_slice(value.as_slice());
        Ok(SysConfigData { data, len })
    }
}
