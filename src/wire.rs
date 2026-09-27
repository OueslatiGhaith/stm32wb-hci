//! Wire widths and owned buffers for the values HCI commands and events carry.
//!
//! Encoding and decoding go through [`bt_hci::WriteHci`] and
//! [`bt_hci::FromHciBytes`], whose integer impls are little-endian (bt-hci
//! rejects big-endian targets at compile time). This module adds what those
//! traits leave
//! implicit: the exact width of a fixed-size value, which declarations are
//! checked against the catalog with at compile time, and an owned buffer for
//! variable-length data in return parameters that must be `Copy`.

use core::fmt;
use core::ops::Deref;

use bt_hci::FromHciBytesError;
use bt_hci::param::{BdAddr, ConnHandle};

/// A value with one exact, canonical HCI wire width.
///
/// The width is that of the value's HCI encoding, never of its Rust layout.
pub trait HciWireType {
    /// Number of bytes in the HCI encoding.
    const WIDTH: usize;
}

macro_rules! wire_width {
    ($($ty:ty => $width:expr),* $(,)?) => {
        $(
            impl HciWireType for $ty {
                const WIDTH: usize = $width;
            }
        )*
    };
}

wire_width! {
    u8 => 1,
    i8 => 1,
    u16 => 2,
    i16 => 2,
    u32 => 4,
    u64 => 8,
    bool => 1,
    BdAddr => 6,
    ConnHandle => 2,
}

impl<T: HciWireType, const N: usize> HciWireType for [T; N] {
    const WIDTH: usize = T::WIDTH * N;
}

impl<T: HciWireType + ?Sized> HciWireType for &T {
    const WIDTH: usize = T::WIDTH;
}

/// A variable-length field longer than the capacity the catalog declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TooLong {
    /// The Rust name of the field.
    pub field: &'static str,
    /// The number of elements supplied.
    pub len: usize,
    /// The largest number of elements the command accepts.
    pub capacity: usize,
}

impl fmt::Display for TooLong {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} has {} elements but holds at most {}",
            self.field, self.len, self.capacity
        )
    }
}

impl core::error::Error for TooLong {}

/// Up to `MAX_LEN` bytes of a variable-length field, owned so the value
/// containing it can be `Copy`.
#[derive(Clone, Copy)]
pub struct BoundedBytes<const MAX_LEN: usize> {
    bytes: [u8; MAX_LEN],
    len: usize,
}

impl<const MAX_LEN: usize> BoundedBytes<MAX_LEN> {
    /// Copy `bytes`, which must not exceed the field's capacity.
    pub fn new(bytes: &[u8]) -> Result<Self, FromHciBytesError> {
        let mut buffer = [0; MAX_LEN];
        buffer
            .get_mut(..bytes.len())
            .ok_or(FromHciBytesError::InvalidSize)?
            .copy_from_slice(bytes);
        Ok(Self {
            bytes: buffer,
            len: bytes.len(),
        })
    }

    /// The bytes present on the wire.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl<const MAX_LEN: usize> Default for BoundedBytes<MAX_LEN> {
    fn default() -> Self {
        Self {
            bytes: [0; MAX_LEN],
            len: 0,
        }
    }
}

impl<const MAX_LEN: usize> Deref for BoundedBytes<MAX_LEN> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<const MAX_LEN: usize> AsRef<[u8]> for BoundedBytes<MAX_LEN> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<const MAX_LEN: usize> PartialEq for BoundedBytes<MAX_LEN> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<const MAX_LEN: usize> Eq for BoundedBytes<MAX_LEN> {}

impl<const MAX_LEN: usize> fmt::Debug for BoundedBytes<MAX_LEN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

#[cfg(feature = "defmt")]
impl<const MAX_LEN: usize> defmt::Format for BoundedBytes<MAX_LEN> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=[u8]}", self.as_slice());
    }
}

#[cfg(test)]
mod tests {
    use bt_hci::WriteHci;

    use super::*;

    /// A fixed-size value's declared width must be what bt-hci writes.
    fn encoded_width<T: HciWireType + WriteHci>(value: T) {
        let mut buffer = [0u8; 16];
        let mut writer = &mut buffer[..];
        value.write_hci(&mut writer).unwrap();
        let written = 16 - writer.len();
        assert_eq!(value.size(), T::WIDTH, "{}", core::any::type_name::<T>());
        assert_eq!(written, T::WIDTH, "{}", core::any::type_name::<T>());
    }

    #[test]
    fn widths_match_the_encoding() {
        encoded_width(0u8);
        encoded_width(0i8);
        encoded_width(0u16);
        encoded_width(0i16);
        encoded_width(0u32);
        encoded_width(0u64);
        encoded_width(true);
        encoded_width(BdAddr::new([1, 2, 3, 4, 5, 6]));
        encoded_width(ConnHandle::new(0x0EFF));
        encoded_width([0u16; 3]);
        assert_eq!(<&[BdAddr; 2]>::WIDTH, 12);
    }

    #[test]
    fn bounded_bytes_keep_only_the_wire_bytes() {
        let bytes = BoundedBytes::<4>::new(&[1, 2, 3]).unwrap();
        assert_eq!(bytes.as_slice(), [1, 2, 3]);
        assert_eq!(bytes.len(), 3);
        assert_eq!(bytes, BoundedBytes::<4>::new(&[1, 2, 3]).unwrap());
        assert_ne!(bytes, BoundedBytes::<4>::new(&[1, 2]).unwrap());
        assert!(BoundedBytes::<4>::default().is_empty());
        assert_eq!(
            BoundedBytes::<2>::new(&[1, 2, 3]),
            Err(FromHciBytesError::InvalidSize)
        );
    }
}
