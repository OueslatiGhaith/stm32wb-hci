//! Executing commands by calling the STM32WBA BLE stack functions directly.
//!
//! STM32CubeWBA ships its BLE stack as a library that declares one C function
//! per command, such as `aci_gap_init` or `hci_le_set_advertising_parameters`.
//! With the `direct` feature, every command the selected stack implements
//! also implements [`Direct`], whose [`call`](Direct::call) passes the
//! command's parameters to that function and decodes what it writes back,
//! instead of encoding an HCI packet for the stack to parse.
//!
//! The calls are generated from the catalog: each function takes the
//! command's parameters in order, followed by pointers to its return
//! parameters after the status, except those that echo a parameter.
//!
//! Besides the commands the catalog checks, bt-hci's `LeSetExtScanParams`,
//! `LeExtCreateConn`, and `LeExtCreateConnV2`, which write one parameter set
//! per PHY selected, implement [`Direct`] too: the stack reads one set per
//! PHY selected, in order.

use bt_hci::param::Status;

/// A command the STM32WBA BLE stack executes when its function is called.
pub trait Direct: bt_hci::cmd::Cmd {
    /// The return parameters for a command completing with Command Complete,
    /// `()` for one completing with Command Status.
    type Output;

    /// Call the stack function executing this command.
    ///
    /// # Safety
    ///
    /// The BLE stack must be initialized, and no other call into it may run
    /// until this one returns.
    unsafe fn call(&self) -> Result<Self::Output, Error>;
}

/// Why a direct call failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// The stack returned an error status.
    Hci(bt_hci::param::Error),
    /// The parameters do not have the layout the function takes, or the
    /// stack wrote back more than the return parameters hold. The function
    /// is not called in the first case.
    Invalid,
}

impl From<bt_hci::param::Error> for Error {
    fn from(error: bt_hci::param::Error) -> Self {
        Self::Hci(error)
    }
}

#[doc(hidden)]
pub mod __private {
    //! What the generated [`Direct`](super::Direct) implementations call.

    use bt_hci::{FromHciBytes, WriteHci};

    pub use super::Error;

    /// The status a stack function returned, as a result.
    #[inline]
    pub fn status(status: u8) -> Result<(), Error> {
        super::Status::from(status).to_result().map_err(Error::Hci)
    }

    /// Reads a command's parameters, as bt-hci encodes them, member by member.
    pub struct Reader<'a> {
        rest: &'a [u8],
    }

    impl<'a> Reader<'a> {
        /// Encode `params` into `buf`.
        #[inline]
        pub fn new<P: WriteHci + ?Sized>(buf: &'a mut [u8], params: &P) -> Result<Self, Error> {
            let len = params.size();
            let buf = buf.get_mut(..len).ok_or(Error::Invalid)?;
            params.write_hci(&mut *buf).map_err(|_| Error::Invalid)?;
            Ok(Self { rest: buf })
        }

        /// The next `len` bytes.
        #[inline]
        pub fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
            if len > self.rest.len() {
                return Err(Error::Invalid);
            }
            let (taken, rest) = self.rest.split_at(len);
            self.rest = rest;
            Ok(taken)
        }

        /// The next `N` bytes.
        #[inline]
        pub fn array<const N: usize>(&mut self) -> Result<&'a [u8; N], Error> {
            self.take(N)?.try_into().map_err(|_| Error::Invalid)
        }

        /// Succeeds if every byte was read.
        #[inline]
        pub fn finish(self) -> Result<(), Error> {
            if self.rest.is_empty() {
                Ok(())
            } else {
                Err(Error::Invalid)
            }
        }
    }

    /// Element `index` of a little-endian array of `N`-byte integers.
    #[inline]
    pub fn element<const N: usize>(bytes: &[u8], index: usize) -> [u8; N] {
        let mut value = [0; N];
        value.copy_from_slice(&bytes[index * N..][..N]);
        value
    }

    /// The number of elements `count` names, if at most `capacity`.
    #[inline]
    pub fn count(count: impl TryInto<usize>, capacity: usize) -> Result<usize, Error> {
        match count.try_into() {
            Ok(count) if count <= capacity => Ok(count),
            _ => Err(Error::Invalid),
        }
    }

    /// Collects return parameters as they are encoded.
    pub struct Writer<'a> {
        buf: &'a mut [u8],
        len: usize,
    }

    impl<'a> Writer<'a> {
        #[inline]
        pub fn new(buf: &'a mut [u8]) -> Self {
            Self { buf, len: 0 }
        }

        #[inline]
        pub fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
            let end = self.len + bytes.len();
            self.buf
                .get_mut(self.len..end)
                .ok_or(Error::Invalid)?
                .copy_from_slice(bytes);
            self.len = end;
            Ok(())
        }

        /// Decode what was written as `R`.
        #[inline]
        pub fn finish<R: for<'de> FromHciBytes<'de>>(self) -> Result<R, Error> {
            R::from_hci_bytes_complete(&self.buf[..self.len]).map_err(|_| Error::Invalid)
        }
    }
}

/// bt-hci's extended scan and connect commands, which write one parameter
/// set per PHY selected where the catalog's layout always has two or three.
/// The stack functions read one set per PHY selected, in order.
///
/// The extended create connection commands exist wherever extended scanning
/// does, version 2 in fewer releases.
#[stm32wb_hci_macros::direct_available(hci_le_set_extended_scan_parameters)]
mod per_phy {
    use bt_hci::cmd::Cmd;
    use bt_hci::cmd::le::{LeExtCreateConn, LeSetExtScanParams};
    use stm32wb_hci_macros::direct_available;

    use super::__private as d;
    use super::{Direct, Error};

    /// The LE 1M and LE Coded PHYs, which scanning may select.
    const SCANNING_PHYS: u8 = 0b101;
    /// The LE 1M, LE 2M, and LE Coded PHYs, which initiating may select.
    const INITIATING_PHYS: u8 = 0b111;
    const SCAN_SET_WIDTH: usize = 5;
    const INITIATING_SET_WIDTH: usize = 16;

    /// The parameter sets for `phys`, if it selects only PHYs in `allowed`.
    fn sets<'a>(
        r: &mut d::Reader<'a>,
        phys: u8,
        allowed: u8,
        width: usize,
    ) -> Result<&'a [u8], Error> {
        if phys & !allowed != 0 {
            return Err(Error::Invalid);
        }
        r.take(phys.count_ones() as usize * width)
    }

    #[direct_available(hci_le_set_extended_scan_parameters)]
    impl Direct for LeSetExtScanParams {
        type Output = ();

        unsafe fn call(&self) -> Result<(), Error> {
            unsafe extern "C" {
                #[link_name = "HCI_LE_SET_EXTENDED_SCAN_PARAMETERS"]
                fn hci_le_set_extended_scan_parameters(
                    own_address_type: u8,
                    scanning_filter_policy: u8,
                    scanning_phys: u8,
                    scan_param_phy: *const u8,
                ) -> u8;
            }
            let mut buf = [0u8; 3 + 3 * SCAN_SET_WIDTH];
            let mut r = d::Reader::new(&mut buf, self.params())?;
            let &[own_address_type, scanning_filter_policy, scanning_phys] = r.array::<3>()?;
            let sets = sets(&mut r, scanning_phys, SCANNING_PHYS, SCAN_SET_WIDTH)?;
            r.finish()?;
            d::status(unsafe {
                hci_le_set_extended_scan_parameters(
                    own_address_type,
                    scanning_filter_policy,
                    scanning_phys,
                    sets.as_ptr(),
                )
            })
        }
    }

    #[direct_available(hci_le_extended_create_connection)]
    impl Direct for LeExtCreateConn {
        type Output = ();

        unsafe fn call(&self) -> Result<(), Error> {
            unsafe extern "C" {
                #[link_name = "HCI_LE_EXTENDED_CREATE_CONNECTION"]
                fn hci_le_extended_create_connection(
                    initiator_filter_policy: u8,
                    own_address_type: u8,
                    peer_address_type: u8,
                    peer_address: *const u8,
                    initiating_phys: u8,
                    init_param_phy: *const u8,
                ) -> u8;
            }
            let mut buf = [0u8; 10 + 3 * INITIATING_SET_WIDTH];
            let mut r = d::Reader::new(&mut buf, self.params())?;
            let &[initiator_filter_policy, own_address_type, peer_address_type] = r.array::<3>()?;
            let peer_address = r.take(6)?;
            let &[initiating_phys] = r.array::<1>()?;
            let sets = sets(
                &mut r,
                initiating_phys,
                INITIATING_PHYS,
                INITIATING_SET_WIDTH,
            )?;
            r.finish()?;
            d::status(unsafe {
                hci_le_extended_create_connection(
                    initiator_filter_policy,
                    own_address_type,
                    peer_address_type,
                    peer_address.as_ptr(),
                    initiating_phys,
                    sets.as_ptr(),
                )
            })
        }
    }

    #[direct_available(hci_le_extended_create_connection_v2)]
    impl Direct for bt_hci::cmd::le::LeExtCreateConnV2 {
        type Output = ();

        unsafe fn call(&self) -> Result<(), Error> {
            unsafe extern "C" {
                #[link_name = "HCI_LE_EXTENDED_CREATE_CONNECTION_V2"]
                fn hci_le_extended_create_connection_v2(
                    advertising_handle: u8,
                    subevent: u8,
                    initiator_filter_policy: u8,
                    own_address_type: u8,
                    peer_address_type: u8,
                    peer_address: *const u8,
                    initiating_phys: u8,
                    init_param_phy: *const u8,
                ) -> u8;
            }
            let mut buf = [0u8; 12 + 3 * INITIATING_SET_WIDTH];
            let mut r = d::Reader::new(&mut buf, self.params())?;
            let &[
                advertising_handle,
                subevent,
                initiator_filter_policy,
                own_address_type,
                peer_address_type,
            ] = r.array::<5>()?;
            let peer_address = r.take(6)?;
            let &[initiating_phys] = r.array::<1>()?;
            let sets = sets(
                &mut r,
                initiating_phys,
                INITIATING_PHYS,
                INITIATING_SET_WIDTH,
            )?;
            r.finish()?;
            d::status(unsafe {
                hci_le_extended_create_connection_v2(
                    advertising_handle,
                    subevent,
                    initiator_filter_policy,
                    own_address_type,
                    peer_address_type,
                    peer_address.as_ptr(),
                    initiating_phys,
                    sets.as_ptr(),
                )
            })
        }
    }
}
