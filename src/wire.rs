//! Wire widths and owned buffers for the values HCI commands and events carry.
//!
//! Encoding and decoding go through [`bt_hci::WriteHci`] and
//! [`bt_hci::FromHciBytes`], whose integer impls are little-endian (bt-hci
//! rejects big-endian targets at compile time). This module adds what those
//! traits leave
//! implicit: the exact width of a fixed-size value, which declarations are
//! checked against the catalog with at compile time, the alternatives of a
//! value whose width a selector member decides, the identity of the Rust
//! types standing for the catalog's C structures, an owned buffer for
//! variable-length data in return parameters that must be `Copy`, and the
//! vendor event code identifying each ST event.

use core::fmt;
use core::ops::Deref;

use bt_hci::param::{BdAddr, ConnHandle};
use bt_hci::uuid::BluetoothUuid;
use bt_hci::{FromHciBytes, FromHciBytesError, WriteHci};

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

/// A value encoded as one of several alternatives, which an earlier member
/// (the selector) identifies.
///
/// The selector is written from [`selector`](Self::selector) rather than
/// declared, so it cannot disagree with the value.
pub trait HciWireUnion: WriteHci {
    /// The selector value and encoded width of every alternative.
    const VARIANTS: &'static [(u64, usize)];

    /// The selector value of this alternative, one of [`Self::VARIANTS`].
    fn selector(&self) -> u64;
}

/// Whether every alternative of `T` is one the catalog encodes with the same
/// width, every alternative the catalog names is one of `T`'s, and every
/// selector value fits the selector member's `selector_width` bytes.
///
/// `explicit` lists the catalog's `(selector, width)` alternatives and
/// `default` the width of every selector value not listed.
#[doc(hidden)]
pub const fn union_matches<T: HciWireUnion>(
    explicit: &[(u64, usize)],
    default: Option<usize>,
    selector_width: usize,
) -> bool {
    let mut index = 0;
    while index < T::VARIANTS.len() {
        let (selector, width) = T::VARIANTS[index];
        if selector_width < 8 && selector >> (8 * selector_width) != 0 {
            return false;
        }
        let mut catalog = default;
        let mut explicit_index = 0;
        while explicit_index < explicit.len() {
            if explicit[explicit_index].0 == selector {
                catalog = Some(explicit[explicit_index].1);
            }
            explicit_index += 1;
        }
        match catalog {
            Some(catalog) if catalog == width => {}
            _ => return false,
        }
        index += 1;
    }
    let mut explicit_index = 0;
    while explicit_index < explicit.len() {
        let mut found = false;
        let mut index = 0;
        while index < T::VARIANTS.len() {
            found |= T::VARIANTS[index].0 == explicit[explicit_index].0;
            index += 1;
        }
        if !found {
            return false;
        }
        explicit_index += 1;
    }
    true
}

/// A 16-bit or 128-bit UUID, as the vendor commands select it with a UUID
/// type member (1 for 16 bits, 2 for 128 bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uuid {
    /// A 16-bit UUID assigned by the Bluetooth SIG.
    Uuid16(u16),
    /// A full 128-bit UUID, in little-endian byte order.
    Uuid128([u8; 16]),
}

impl HciWireUnion for Uuid {
    const VARIANTS: &'static [(u64, usize)] = &[(1, 2), (2, 16)];

    fn selector(&self) -> u64 {
        match self {
            Uuid::Uuid16(_) => 1,
            Uuid::Uuid128(_) => 2,
        }
    }
}

impl WriteHci for Uuid {
    fn size(&self) -> usize {
        match self {
            Uuid::Uuid16(_) => 2,
            Uuid::Uuid128(_) => 16,
        }
    }

    fn write_hci<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        match self {
            Uuid::Uuid16(uuid) => writer.write_all(&uuid.to_le_bytes()),
            Uuid::Uuid128(uuid) => writer.write_all(uuid),
        }
    }

    async fn write_hci_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        match self {
            Uuid::Uuid16(uuid) => writer.write_all(&uuid.to_le_bytes()).await,
            Uuid::Uuid128(uuid) => writer.write_all(uuid).await,
        }
    }
}

impl From<u16> for Uuid {
    fn from(uuid: u16) -> Self {
        Uuid::Uuid16(uuid)
    }
}

impl From<u128> for Uuid {
    fn from(uuid: u128) -> Self {
        Uuid::Uuid128(uuid.to_le_bytes())
    }
}

/// 32-bit UUIDs, which the vendor commands cannot select, become their
/// 128-bit form.
impl From<BluetoothUuid> for Uuid {
    fn from(uuid: BluetoothUuid) -> Self {
        match uuid {
            BluetoothUuid::Uuid16(uuid) => Uuid::Uuid16(uuid.to_u16()),
            uuid => Uuid::from(uuid.to_u128()),
        }
    }
}

/// A variable-length field longer than the capacity the catalog declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TooLong {
    /// The Rust name of the field, or `"parameters"` when the fields fit their
    /// capacities but not the 255 bytes of the parameters together.
    pub field: &'static str,
    /// The number of elements supplied, or of parameter bytes.
    pub len: usize,
    /// The largest number of elements the command accepts, or 255 bytes.
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

/// Up to `MAX_LEN` elements of a variable-length field, owned so the value
/// containing it can be `Copy`.
#[derive(Clone, Copy)]
pub struct BoundedArray<T, const MAX_LEN: usize> {
    elements: [T; MAX_LEN],
    len: usize,
}

/// Up to `MAX_LEN` bytes of a variable-length field.
pub type BoundedBytes<const MAX_LEN: usize> = BoundedArray<u8, MAX_LEN>;

impl<T: Copy + Default, const MAX_LEN: usize> BoundedArray<T, MAX_LEN> {
    /// Copy `elements`, which must not exceed the field's capacity.
    pub fn new(elements: &[T]) -> Result<Self, FromHciBytesError> {
        let mut array = Self::default();
        array
            .elements
            .get_mut(..elements.len())
            .ok_or(FromHciBytesError::InvalidSize)?
            .copy_from_slice(elements);
        array.len = elements.len();
        Ok(array)
    }

    /// Append `element`, which must fit the field's capacity.
    pub fn push(&mut self, element: T) -> Result<(), FromHciBytesError> {
        *self
            .elements
            .get_mut(self.len)
            .ok_or(FromHciBytesError::InvalidSize)? = element;
        self.len += 1;
        Ok(())
    }
}

impl<T, const MAX_LEN: usize> BoundedArray<T, MAX_LEN> {
    /// The elements present on the wire.
    pub fn as_slice(&self) -> &[T] {
        &self.elements[..self.len]
    }
}

impl<T: Copy + Default, const MAX_LEN: usize> Default for BoundedArray<T, MAX_LEN> {
    fn default() -> Self {
        Self {
            elements: [T::default(); MAX_LEN],
            len: 0,
        }
    }
}

impl<T, const MAX_LEN: usize> Deref for BoundedArray<T, MAX_LEN> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const MAX_LEN: usize> AsRef<[T]> for BoundedArray<T, MAX_LEN> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: PartialEq, const MAX_LEN: usize> PartialEq for BoundedArray<T, MAX_LEN> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq, const MAX_LEN: usize> Eq for BoundedArray<T, MAX_LEN> {}

impl<T: fmt::Debug, const MAX_LEN: usize> fmt::Debug for BoundedArray<T, MAX_LEN> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

#[cfg(feature = "defmt")]
impl<T: defmt::Format, const MAX_LEN: usize> defmt::Format for BoundedArray<T, MAX_LEN> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=[?]}", self.as_slice());
    }
}

/// A Rust type standing for one C structure of the catalog, which
/// declarations check their structure members against.
pub trait CatalogStruct: HciWireType {
    /// The C name of the structure.
    const C_NAME: &'static str;
}

/// Whether `T` stands for the catalog's C structure `c_name`.
#[doc(hidden)]
pub const fn stands_for<T: CatalogStruct>(c_name: &str) -> bool {
    let (left, right) = (T::C_NAME.as_bytes(), c_name.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// An ST vendor event, which bt-hci delivers as [`bt_hci::event::Vendor`]:
/// a 16-bit vendor event code followed by the event's parameters.
pub trait VendorEvent<'a>: FromHciBytes<'a> {
    /// The vendor event code.
    const CODE: u16;

    /// Decode the parameters of a vendor event, code included, if the code is
    /// this event's. The parameters must be exactly those the catalog lists.
    fn from_vendor_params(params: &'a [u8]) -> Option<Result<Self, FromHciBytesError>> {
        let (code, payload) = params.split_first_chunk::<2>()?;
        (u16::from_le_bytes(*code) == Self::CODE).then(|| Self::from_hci_bytes_complete(payload))
    }

    /// Decode a vendor event if its code is this event's.
    fn from_vendor(
        event: &'a bt_hci::event::Vendor<'_>,
    ) -> Option<Result<Self, FromHciBytesError>> {
        Self::from_vendor_params(&event.params)
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

    struct Wide;

    impl WriteHci for Wide {
        fn size(&self) -> usize {
            0
        }

        fn write_hci<W: embedded_io::Write>(&self, _: W) -> Result<(), W::Error> {
            Ok(())
        }

        async fn write_hci_async<W: embedded_io_async::Write>(&self, _: W) -> Result<(), W::Error> {
            Ok(())
        }
    }

    impl HciWireUnion for Wide {
        const VARIANTS: &'static [(u64, usize)] = &[(0x100, 2)];

        fn selector(&self) -> u64 {
            0x100
        }
    }

    #[test]
    fn unions_match_the_catalog_alternatives() {
        assert!(union_matches::<Uuid>(&[(1, 2), (2, 16)], None, 1));
        assert!(union_matches::<Uuid>(&[(2, 16)], Some(2), 1));
        assert!(
            !union_matches::<Uuid>(&[(1, 2)], None, 1),
            "no 128-bit alternative"
        );
        assert!(
            !union_matches::<Uuid>(&[(1, 2), (2, 4)], None, 1),
            "wrong width"
        );
        assert!(
            !union_matches::<Uuid>(&[(1, 2), (2, 16), (3, 4)], None, 1),
            "missing alternative"
        );
        assert!(
            !union_matches::<Wide>(&[], Some(2), 1),
            "selector wider than its member"
        );
        assert!(union_matches::<Wide>(&[], Some(2), 2));
    }

    #[test]
    fn uuids_encode_their_alternative() {
        for (uuid, selector, bytes) in [
            (Uuid::from(0x180Du16), 1, &[0x0D, 0x18][..]),
            (
                Uuid::from(0x0F0E_0D0C_0B0A_0908_0706_0504_0302_0100u128),
                2,
                &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15][..],
            ),
        ] {
            let mut buffer = [0; 16];
            let mut writer = &mut buffer[..];
            uuid.write_hci(&mut writer).unwrap();
            let written = 16 - writer.len();
            assert_eq!(&buffer[..written], bytes);
            assert_eq!(uuid.size(), written);
            assert_eq!(uuid.selector(), selector);
            assert!(Uuid::VARIANTS.contains(&(selector, written)));
        }
        assert_eq!(
            Uuid::from(BluetoothUuid::from_u16(0x2A00)),
            Uuid::Uuid16(0x2A00)
        );
        assert_eq!(
            Uuid::from(BluetoothUuid::from_u32(0x1234_5678)),
            Uuid::from(BluetoothUuid::from_u32(0x1234_5678).to_u128())
        );
    }

    #[test]
    fn bounded_arrays_grow_to_their_capacity() {
        let mut array = BoundedArray::<u16, 2>::default();
        array.push(1).unwrap();
        array.push(2).unwrap();
        assert_eq!(array.push(3), Err(FromHciBytesError::InvalidSize));
        assert_eq!(array.as_slice(), [1, 2]);
        assert_eq!(array, BoundedArray::new(&[1, 2]).unwrap());
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
