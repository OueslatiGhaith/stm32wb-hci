//! Shared allocation-free decoders for declarative command returns and events.
//!
//! The catalog-specific layers supply their semantic field decoder and map the
//! structured errors into their public error type. Length envelopes and owned
//! collection construction live here so command returns and events cannot
//! drift into subtly different wire behavior.

use core::mem::MaybeUninit;

/// Owned, bounded bytes decoded from a variable-length HCI field.
#[derive(Clone, Copy)]
pub struct BoundedBytes<const MAX_LEN: usize> {
    bytes: [u8; MAX_LEN],
    len: usize,
}

impl<const MAX_LEN: usize> BoundedBytes<MAX_LEN> {
    /// Returns only the bytes present on the wire.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl<const MAX_LEN: usize> AsRef<[u8]> for BoundedBytes<MAX_LEN> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<const MAX_LEN: usize> core::fmt::Debug for BoundedBytes<MAX_LEN> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_list().entries(self.as_slice()).finish()
    }
}

/// Owned, allocation-free items decoded from a counted HCI field.
#[derive(Clone, Copy)]
pub struct BoundedItems<T: Copy, const MAX_ITEMS: usize> {
    items: [MaybeUninit<T>; MAX_ITEMS],
    len: usize,
}

impl<T: Copy, const MAX_ITEMS: usize> BoundedItems<T, MAX_ITEMS> {
    /// Number of initialized items decoded from the wire.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the wire collection was empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns only the initialized items present on the wire.
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: constructors initialize every element in `0..len`, `len`
        // never exceeds `MAX_ITEMS`, and `T: Copy` cannot require drop glue.
        unsafe { core::slice::from_raw_parts(self.items.as_ptr().cast::<T>(), self.len) }
    }
}

impl<T: Copy, const MAX_ITEMS: usize> AsRef<[T]> for BoundedItems<T, MAX_ITEMS> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Copy + core::fmt::Debug, const MAX_ITEMS: usize> core::fmt::Debug
    for BoundedItems<T, MAX_ITEMS>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_list().entries(self.as_slice()).finish()
    }
}

/// Construction contract for a count-prefixed byte-field target.
#[doc(hidden)]
pub trait HciDecodeCountedBytes<C, const COUNT_LEN: usize, const MAX_LEN: usize>: Sized {
    fn from_counted_bytes(bytes: &[u8]) -> Self;
}

/// Construction contract for a field that consumes all remaining bytes.
#[doc(hidden)]
pub trait HciDecodeTrailingBytes<const MIN_LEN: usize, const MAX_LEN: usize>: Sized {
    fn from_trailing_bytes(bytes: &[u8]) -> Self;
}

/// Construction contract for a count-prefixed exact-width item target.
#[doc(hidden)]
pub trait HciDecodeCountedItems<
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>: Sized
{
    /// Construct a target from the initialized prefix of `items`.
    ///
    /// # Safety
    ///
    /// Every element in `items[..len]` must be initialized and `len` must not
    /// exceed `MAX_ITEMS`.
    unsafe fn from_counted_items(items: [MaybeUninit<Item>; MAX_ITEMS], len: usize) -> Self;
}

impl<C, const COUNT_LEN: usize, const MAX_LEN: usize> HciDecodeCountedBytes<C, COUNT_LEN, MAX_LEN>
    for BoundedBytes<MAX_LEN>
{
    fn from_counted_bytes(value: &[u8]) -> Self {
        let len = value.len();
        let mut bytes = [0; MAX_LEN];
        bytes[..len].copy_from_slice(value);
        Self { bytes, len }
    }
}

impl<const MIN_LEN: usize, const MAX_LEN: usize> HciDecodeTrailingBytes<MIN_LEN, MAX_LEN>
    for BoundedBytes<MAX_LEN>
{
    fn from_trailing_bytes(data: &[u8]) -> Self {
        let len = data.len();
        let mut bytes = [0; MAX_LEN];
        bytes[..len].copy_from_slice(data);
        Self { bytes, len }
    }
}

impl<Item, C, const COUNT_LEN: usize, const ITEM_LEN: usize, const MAX_ITEMS: usize>
    HciDecodeCountedItems<Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS> for BoundedItems<Item, MAX_ITEMS>
where
    Item: Copy,
{
    unsafe fn from_counted_items(items: [MaybeUninit<Item>; MAX_ITEMS], len: usize) -> Self {
        Self { items, len }
    }
}

#[cfg(feature = "defmt")]
impl<const MAX_LEN: usize> defmt::Format for BoundedBytes<MAX_LEN> {
    fn format(&self, formatter: defmt::Formatter) {
        defmt::write!(formatter, "{=[u8]}", self.as_slice());
    }
}

#[cfg(feature = "defmt")]
impl<T: Copy + defmt::Format, const MAX_ITEMS: usize> defmt::Format for BoundedItems<T, MAX_ITEMS> {
    fn format(&self, formatter: defmt::Formatter) {
        defmt::write!(formatter, "{=[?]}", self.as_slice());
    }
}

/// A catalog-independent failure while decoding one declarative field.
pub(crate) enum DecodeError<E> {
    /// The semantic field decoder rejected the field's bytes.
    Field(E),
    /// The input ended before `required` bytes of the current field were read.
    Truncated { required: usize },
    /// A count exceeded the maximum declared by the schema.
    CountTooLarge { actual: usize, maximum: usize },
    /// A remaining-byte field was outside its inclusive length range.
    LengthOutOfRange {
        actual: usize,
        minimum: usize,
        maximum: usize,
    },
    /// Multiplying a count by its item width overflowed `usize`.
    SizeOverflow { actual: usize, maximum: usize },
}

/// Decode one exact-width field and return the unconsumed input.
pub(crate) fn decode_fixed_field<T, E, const N: usize>(
    data: &[u8],
    decode: impl FnOnce(&[u8; N]) -> Result<T, E>,
) -> Result<(T, &[u8]), DecodeError<E>> {
    if data.len() < N {
        return Err(DecodeError::Truncated { required: N });
    }

    let (field, rest) = data.split_at(N);
    let field = field
        .try_into()
        .expect("split_at returned the declared field width");
    decode(field)
        .map(|value| (value, rest))
        .map_err(DecodeError::Field)
}

/// Decode a count-prefixed byte field and construct its owned representation.
pub(crate) fn decode_counted_bytes<T, E, const COUNT_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
    decode_count: impl FnOnce(&[u8; COUNT_LEN]) -> Result<usize, E>,
    build: impl FnOnce(&[u8]) -> T,
) -> Result<(T, &[u8]), DecodeError<E>> {
    let (len, after_count) = decode_fixed_field(data, decode_count)?;
    if len > MAX_LEN {
        return Err(DecodeError::CountTooLarge {
            actual: len,
            maximum: MAX_LEN,
        });
    }
    if after_count.len() < len {
        return Err(DecodeError::Truncated {
            required: COUNT_LEN + len,
        });
    }

    let (value, rest) = after_count.split_at(len);
    Ok((build(value), rest))
}

/// Decode all remaining bytes when their length is within the declared range.
pub(crate) fn decode_trailing_bytes<T, E, const MIN_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
    build: impl FnOnce(&[u8]) -> T,
) -> Result<(T, &[u8]), DecodeError<E>> {
    let len = data.len();
    if !(MIN_LEN..=MAX_LEN).contains(&len) {
        return Err(DecodeError::LengthOutOfRange {
            actual: len,
            minimum: MIN_LEN,
            maximum: MAX_LEN,
        });
    }

    Ok((build(data), &[]))
}

/// Decode a count-prefixed sequence of exact-width items.
pub(crate) fn decode_counted_items<
    T,
    Item: Copy,
    C,
    E,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>(
    data: &[u8],
    decode_count: impl FnOnce(&[u8; COUNT_LEN]) -> Result<usize, E>,
    mut decode_item: impl FnMut(&[u8; ITEM_LEN]) -> Result<Item, E>,
) -> Result<(T, &[u8]), DecodeError<E>>
where
    T: HciDecodeCountedItems<Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>,
{
    let (len, mut remaining) = decode_fixed_field(data, decode_count)?;
    if len > MAX_ITEMS {
        return Err(DecodeError::CountTooLarge {
            actual: len,
            maximum: MAX_ITEMS,
        });
    }

    let required_items = ITEM_LEN.checked_mul(len).ok_or(DecodeError::SizeOverflow {
        actual: len,
        maximum: MAX_ITEMS,
    })?;
    if remaining.len() < required_items {
        return Err(DecodeError::Truncated {
            required: COUNT_LEN + required_items,
        });
    }

    let mut items = [MaybeUninit::uninit(); MAX_ITEMS];
    for slot in items.iter_mut().take(len) {
        let (item, rest) = decode_fixed_field(remaining, &mut decode_item)?;
        slot.write(item);
        remaining = rest;
    }

    // SAFETY: the loop initialized exactly the first `len` entries after the
    // schema bound established `len <= MAX_ITEMS`.
    let value = unsafe { T::from_counted_items(items, len) };
    Ok((value, remaining))
}
