use core::ops::Add;

use bt_hci::{FromHciBytes, ReadHci, WriteHci};

macro_rules! hci_impl_params {
    ($method:ident, $param_type:ident, $cmd:ident) => {
        async fn $method(&self, params: &$param_type) -> Result<(), Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            let mut bytes = [0; $param_type::LENGTH];
            params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..]).into())
                .exec(self)
                .await
                .map_err(|e| Error::from(e))
        }
    };
}

macro_rules! hci_impl_value_params {
    ($method:ident, $param_type:ident, $cmd:ident) => {
        async fn $method(&self, params: $param_type) -> Result<(), Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            let mut bytes = [0; $param_type::LENGTH];
            params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..]).into())
                .exec(self)
                .await
                .map_err(|e| Error::from(e))
        }
    };
}

macro_rules! hci_impl_validate_params {
    ($method:ident, $param_type:ident, $cmd:ident) => {
        async fn $method(&self, params: &$param_type) -> Result<(), Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            params.validate().map_err(|e| Error::from(e))?;

            let mut bytes = [0; $param_type::LENGTH];
            params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..]).into())
                .exec(self)
                .await
                .map_err(|e| Error::from(e))
        }
    };
}

macro_rules! hci_impl_variable_length_params {
    ($method:ident, $param_type:ident, $cmd:ident) => {
        async fn $method(&self, params: &$param_type) -> Result<(), Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            let mut bytes = [0; $param_type::MAX_LENGTH];
            params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..]).into())
                .exec(self)
                .await
                .map_err(|e| Error::from(e))
        }
    };
    ($method:ident, $param_type:ident, $cmd:ident, $ret:ident) => {
        async fn $method(&self, params: &$param_type) -> Result<$ret, Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            let mut bytes = [0; $param_type::MAX_LENGTH];
            params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..]).into())
                    .exec(self)
                    .await
                    .map_err(|e| Error::from(e))?
                    .buf()
                    .try_into()
                    .map_err(|e| Error::from(e))
        }
    };
    ($method:ident<$($genlife:lifetime),*>, $param_type:ident<$($lifetime:lifetime),*>, $cmd:ident) => {
        async fn $method<$($genlife),*>(
            &self,
            params: &$param_type<$($lifetime),*>
        ) -> Result<(), Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            let mut bytes = [0; $param_type::MAX_LENGTH];
            params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..]).into())
                .exec(self)
                .await
                .map_err(|e| Error::from(e))
        }
    };
}
macro_rules! hci_impl_validate_variable_length_params {
    ($method:ident, $param_type:ident, $cmd:ident) => {
        async fn $method(&self, params: &$param_type) -> Result<(), Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            params.validate().map_err(|e| Error::from(e))?;

            let mut bytes = [0; $param_type::MAX_LENGTH];
            let len = params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..len]).into())
                .exec(self)
                .await
                .map_err(|e| Error::from(e))
        }
    };
    ($method:ident<$($genlife:lifetime),*>, $param_type:ident<$($lifetime:lifetime),*>, $cmd:ident) => {
        async fn $method<$($genlife),*>(
            &self,
            params: &$param_type<$($lifetime),*>
        ) -> Result<(), Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            params.validate().map_err(|e| Error::from(e))?;

            let mut bytes = [0; $param_type::MAX_LENGTH];
            let len = params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..len]).into())
                .exec(self)
                .await
                .map_err(|e| Error::from(e))
        }
    };
    ($method:ident<$($genlife:lifetime),*>, $param_type:ident<$($lifetime:lifetime),*>, $cmd:ident, $ret:ident) => {
        async fn $method<$($genlife),*>(
            &self,
            params: &$param_type<$($lifetime),*>
        ) -> Result<$ret, Error> {
            #[allow(unused_imports)]
            use ::bt_hci::cmd::{AsyncCmd, SyncCmd};

            params.validate().map_err(|e| Error::from(e))?;

            let mut bytes = [0; $param_type::MAX_LENGTH];
            let len = params.copy_into_slice(&mut bytes);

            $cmd::new((&bytes[..len]).into())
                    .exec(self)
                    .await
                    .map_err(|e| Error::from(e))?
                    .buf()
                    .try_into()
                    .map_err(|e| Error::from(e))
        }
    };
}

/// If the command requires no params and returns a command status:
///
/// ```rust,ignore
///     vendor_cmd! {
///         GapAdvClearSets(GAP_ADV_CLEAR_SETS) {
///             Params = ();
///         }
///     }
/// ```
///
/// If the command requires no params and returns a command complete with just a status:
/// ```rust,ignore
///     vendor_cmd! {
///         GapAdvClearSets(GAP_ADV_CLEAR_SETS) {
///             Params = ();
///             Return = ();
///         }
///     }
/// ```
///
/// If the command requires params and returns a command complete with more than a status:
/// ```rust,ignore
///     vendor_cmd! {
///         GapAdvClearSets(GAP_ADV_CLEAR_SETS) {
///             Params<'a> = ParamBuffer<'a>;
///             Return = ReturnBuffer<25>;
///         }
///     }
/// ```
///
/// Note that the `ReturnBuffer` `MAX_LEN` should be two or more, to accomodate the command status.
macro_rules! vendor_cmd {
    (
        $cmd:ident($opcode:ident) $params:tt
    ) => {
        ::bt_hci::cmd::cmd! {
            $cmd(VENDOR_SPECIFIC, crate::vendor::opcode::$opcode.ocf()) $params
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Hash, Eq)]
pub struct ParamBuffer<'a>(&'a [u8]);

impl<'a> From<&'a [u8]> for ParamBuffer<'a> {
    #[inline]
    fn from(buf: &'a [u8]) -> Self {
        Self(buf)
    }
}

impl<'a> WriteHci for ParamBuffer<'a> {
    #[inline]
    fn size(&self) -> usize {
        self.0.len()
    }

    #[inline]
    fn write_hci<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(self.0)
    }

    #[inline]
    async fn write_hci_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(self.0).await
    }
}

#[derive(Clone, Copy)]
pub struct ReturnBuffer<const MAX_LEN: usize>([u8; MAX_LEN], usize);

impl<const MAX_LEN: usize> ReturnBuffer<MAX_LEN> {
    #[inline]
    pub fn buf(&self) -> &[u8] {
        &self.0[..self.1]
    }
}

impl<'de, const MAX_LEN: usize> ReadHci<'de> for ReturnBuffer<MAX_LEN> {
    const MAX_LEN: usize = MAX_LEN - 1;

    #[inline]
    fn read_hci<R: embedded_io::Read>(
        mut reader: R,
        buf: &'de mut [u8],
    ) -> Result<Self, bt_hci::ReadHciError<R::Error>> {
        reader.read_exact(buf)?;

        Self::from_hci_bytes_complete(buf).map_err(|_| bt_hci::ReadHciError::InvalidValue)
    }

    #[inline]
    async fn read_hci_async<R: embedded_io_async::Read>(
        mut reader: R,
        buf: &'de mut [u8],
    ) -> Result<Self, bt_hci::ReadHciError<R::Error>> {
        reader.read_exact(buf).await?;

        Self::from_hci_bytes_complete(buf).map_err(|_| bt_hci::ReadHciError::InvalidValue)
    }
}

impl<'de, const MAX_LEN: usize> FromHciBytes<'de> for ReturnBuffer<MAX_LEN> {
    #[inline]
    fn from_hci_bytes(data: &'de [u8]) -> Result<(Self, &'de [u8]), bt_hci::FromHciBytesError> {
        if data.len() < MAX_LEN {
            let mut buf = [0u8; MAX_LEN];

            buf[1..][..data.len()].copy_from_slice(data);

            Ok((Self(buf, data.len().add(1)), &[]))
        } else {
            Err(bt_hci::FromHciBytesError::InvalidSize)
        }
    }
}

pub mod gap;
pub mod gatt;
pub mod hal;
pub mod l2cap;
