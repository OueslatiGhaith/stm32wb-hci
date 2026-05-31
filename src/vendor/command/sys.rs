//! System commands (ACI_RESET, ACI_GET_INFORMATION, ACI_WRITE_CONFIG_DATA, ACI_READ_CONFIG_DATA).

use bt_hci::{cmd::SyncCmd, controller::ControllerCmdSync};
use byteorder::{ByteOrder, LittleEndian};

use crate::vendor::command::{ParamBuffer, ReturnBuffer};
use crate::{BadStatusError, Status};

/// System-level commands.
pub trait SysCommands {
    /// Reset the BLE stack.
    async fn sys_reset(&self, mode: SysResetMode, options: u32) -> Result<(), Error>;

    /// Read local ACI information.
    async fn get_information(&self) -> Result<SysInformation, Error>;

    /// Write a value to the configure data structure.
    async fn sys_write_config_data(&self, offset: u8, data: &[u8]) -> Result<(), Error>;

    /// Read a value from the configure data structure.
    async fn sys_read_config_data(&self, offset: u8) -> Result<SysConfigData, Error>;
}

/// Mode for [sys_reset](SysCommands::sys_reset).
#[repr(u8)]
pub enum SysResetMode {
    /// Reset without BLE stack options change.
    NoOptionsChange = 0x00,
    /// Reset with BLE stack option changes.
    WithOptionsChange = 0x01,
}

/// Return value for [get_information](SysCommands::get_information).
pub struct SysInformation {
    /// BLE stack version (8 bytes).
    pub version: [u8; 8],
    /// BLE stack options bitmask.
    pub options: u32,
    /// BLE stack debug information (12 bytes).
    pub debug_info: [u8; 12],
}

/// Return value for [sys_read_config_data](SysCommands::sys_read_config_data).
pub struct SysConfigData {
    /// Raw config data bytes.
    pub data: [u8; 32],
    pub len: usize,
}

/// Error type for system commands.
#[derive(Copy, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    HciError(Status),
    UnknownHciError(u8),
    IoError,
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

vendor_cmd! {
    SysReset(SYS_RESET) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    SysGetInformation(SYS_GET_INFORMATION) {
        Params = ();
        Return = ReturnBuffer<25>;
    }
}

vendor_cmd! {
    SysWriteConfigData(SYS_WRITE_CONFIG_DATA) {
        Params<'a> = ParamBuffer<'a>;
        Return = ();
    }
}

vendor_cmd! {
    SysReadConfigData(SYS_READ_CONFIG_DATA) {
        Params<'a> = ParamBuffer<'a>;
        Return = ReturnBuffer<33>;
    }
}

impl<T> SysCommands for T
where
    T: for<'t> ControllerCmdSync<SysReset<'t>>
        + ControllerCmdSync<SysGetInformation>
        + for<'t> ControllerCmdSync<SysWriteConfigData<'t>>
        + for<'t> ControllerCmdSync<SysReadConfigData<'t>>,
{
    async fn sys_reset(&self, mode: SysResetMode, options: u32) -> Result<(), Error> {
        let mut bytes = [0u8; 5];
        bytes[0] = mode as u8;
        LittleEndian::write_u32(&mut bytes[1..5], options);
        SysReset::new((&bytes[..]).into())
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn get_information(&self) -> Result<SysInformation, Error> {
        let buf = SysGetInformation::new()
            .exec(self)
            .await
            .map_err(Error::from)?;
        let b = buf.buf();
        let mut info = SysInformation {
            version: [0u8; 8],
            options: 0,
            debug_info: [0u8; 12],
        };
        info.version.copy_from_slice(&b[0..8]);
        info.options = LittleEndian::read_u32(&b[8..12]);
        info.debug_info.copy_from_slice(&b[12..24]);
        Ok(info)
    }

    async fn sys_write_config_data(&self, offset: u8, data: &[u8]) -> Result<(), Error> {
        let mut bytes = [0u8; 34];
        bytes[0] = offset;
        bytes[1] = data.len() as u8;
        bytes[2..2 + data.len()].copy_from_slice(data);
        SysWriteConfigData::new((&bytes[..2 + data.len()]).into())
            .exec(self)
            .await
            .map_err(|e| e.into())
    }

    async fn sys_read_config_data(&self, offset: u8) -> Result<SysConfigData, Error> {
        let buf = SysReadConfigData::new((&[offset][..]).into())
            .exec(self)
            .await
            .map_err(Error::from)?;
        let b = buf.buf();
        let len = b.len().min(32);
        let mut result = SysConfigData {
            data: [0u8; 32],
            len,
        };
        result.data[..len].copy_from_slice(&b[..len]);
        Ok(result)
    }
}
