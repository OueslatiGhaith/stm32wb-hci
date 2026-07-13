use bt_hci::WriteHci;

/// A value with an exact, canonical representation in an HCI request.
///
/// `N` is part of the trait so a declarative command field whose schema says
/// `field: Type => N` only compiles when `Type` explicitly supports that wire
/// width. Implementations must not rely on Rust structure layout or native
/// endianness.
pub trait HciEncodeField<const N: usize> {
    /// Write exactly `N` bytes to a synchronous HCI writer.
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error>;

    /// Write exactly `N` bytes to an asynchronous HCI writer.
    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error>;
}

/// A value decoded from an exact-width field in an HCI response.
///
/// Implementations receive exactly `N` bytes and must apply the protocol's
/// validity rules rather than interpreting arbitrary Rust memory.
pub trait HciDecodeField<const N: usize>: Sized {
    /// Decode one exact-width field.
    fn from_hci_field(bytes: &[u8; N]) -> Result<Self, bt_hci::FromHciBytesError>;
}

/// A reusable declarative payload that can be serialized into an HCI packet.
pub trait HciEncodePayload {
    /// Smallest encoded variant.
    const MIN_LEN: usize;
    /// Largest encoded variant.
    const MAX_LEN: usize;

    /// Exact encoded length of this value.
    fn hci_payload_size(&self) -> usize;

    /// Encode this value synchronously.
    fn write_hci_payload<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error>;

    /// Encode this value asynchronously.
    async fn write_hci_payload_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error>;
}

/// A reusable declarative payload that can be decoded from HCI bytes.
pub trait HciDecodePayload: Sized {
    /// Smallest encoded variant.
    const MIN_LEN: usize;
    /// Largest encoded variant.
    const MAX_LEN: usize;

    /// Decode one payload and return the unconsumed bytes.
    fn from_hci_payload(data: &[u8]) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError>;
}

impl<T> HciEncodePayload for &T
where
    T: HciEncodePayload,
{
    const MIN_LEN: usize = T::MIN_LEN;
    const MAX_LEN: usize = T::MAX_LEN;

    fn hci_payload_size(&self) -> usize {
        T::hci_payload_size(self)
    }

    fn write_hci_payload<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        T::write_hci_payload(self, writer)
    }

    async fn write_hci_payload_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        T::write_hci_payload_async(self, writer).await
    }
}

macro_rules! impl_hci_integer_field {
    ($ty:ty, $len:literal) => {
        impl HciEncodeField<$len> for $ty {
            #[inline]
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&self.to_le_bytes())
            }

            #[inline]
            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&self.to_le_bytes()).await
            }
        }

        impl HciDecodeField<$len> for $ty {
            #[inline]
            fn from_hci_field(bytes: &[u8; $len]) -> Result<Self, bt_hci::FromHciBytesError> {
                Ok(<$ty>::from_le_bytes(*bytes))
            }
        }
    };
}

impl_hci_integer_field!(u8, 1);
impl_hci_integer_field!(i8, 1);
impl_hci_integer_field!(u16, 2);
impl_hci_integer_field!(i16, 2);
impl_hci_integer_field!(u32, 4);
impl_hci_integer_field!(i32, 4);
impl_hci_integer_field!(u64, 8);
impl_hci_integer_field!(i64, 8);

impl HciEncodeField<1> for bool {
    #[inline]
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&[u8::from(*self)])
    }

    #[inline]
    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&[u8::from(*self)]).await
    }
}

impl HciDecodeField<1> for bool {
    #[inline]
    fn from_hci_field(bytes: &[u8; 1]) -> Result<Self, bt_hci::FromHciBytesError> {
        match bytes[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(bt_hci::FromHciBytesError::InvalidValue),
        }
    }
}

impl<const N: usize> HciEncodeField<N> for [u8; N] {
    #[inline]
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(self)
    }

    #[inline]
    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(self).await
    }
}

impl<const N: usize> HciDecodeField<N> for [u8; N] {
    #[inline]
    fn from_hci_field(bytes: &[u8; N]) -> Result<Self, bt_hci::FromHciBytesError> {
        Ok(*bytes)
    }
}

impl<T, const N: usize> HciEncodeField<N> for &T
where
    T: HciEncodeField<N>,
{
    fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        T::write_hci_field(self, writer)
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        T::write_hci_field_async(self, writer).await
    }
}

macro_rules! impl_hci_newtype_field {
    ($ty:path, $inner:ty, $len:literal) => {
        impl HciEncodeField<$len> for $ty {
            #[inline]
            fn write_hci_field<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
                <$inner as HciEncodeField<$len>>::write_hci_field(&self.0, writer)
            }

            #[inline]
            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <$inner as HciEncodeField<$len>>::write_hci_field_async(&self.0, writer).await
            }
        }

        impl HciDecodeField<$len> for $ty {
            #[inline]
            fn from_hci_field(bytes: &[u8; $len]) -> Result<Self, bt_hci::FromHciBytesError> {
                <$inner as HciDecodeField<$len>>::from_hci_field(bytes).map(Self)
            }
        }
    };
}

impl_hci_newtype_field!(crate::ConnectionHandle, u16, 2);
impl_hci_newtype_field!(crate::AdvertisingHandle, u8, 1);
impl_hci_newtype_field!(crate::BdAddr, [u8; 6], 6);
impl_hci_newtype_field!(crate::vendor::event::AttributeHandle, u16, 2);

impl HciEncodeField<7> for crate::BdAddrType {
    fn write_hci_field<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        match self {
            crate::BdAddrType::Public(address) => {
                writer.write_all(&[0])?;
                writer.write_all(&address.0)
            }
            crate::BdAddrType::Random(address) => {
                writer.write_all(&[1])?;
                writer.write_all(&address.0)
            }
        }
    }

    async fn write_hci_field_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        match self {
            crate::BdAddrType::Public(address) => {
                writer.write_all(&[0]).await?;
                writer.write_all(&address.0).await
            }
            crate::BdAddrType::Random(address) => {
                writer.write_all(&[1]).await?;
                writer.write_all(&address.0).await
            }
        }
    }
}

impl HciDecodeField<7> for crate::BdAddrType {
    fn from_hci_field(bytes: &[u8; 7]) -> Result<Self, bt_hci::FromHciBytesError> {
        let mut address = [0; 6];
        address.copy_from_slice(&bytes[1..]);
        crate::to_bd_addr_type(bytes[0], crate::BdAddr(address))
            .map_err(|_| bt_hci::FromHciBytesError::InvalidValue)
    }
}

#[doc(hidden)]
pub struct DeclarativeField<T, const N: usize>(pub T);

#[doc(hidden)]
pub struct DeclarativeParams<T>(pub T);

/// A variable-length value does not satisfy its declarative wire bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HciLengthError {
    actual: usize,
    minimum: usize,
    maximum: usize,
}

impl HciLengthError {
    pub(crate) const fn new(actual: usize, minimum: usize, maximum: usize) -> Self {
        Self {
            actual,
            minimum,
            maximum,
        }
    }

    /// Actual number of bytes/items, or the rejected bitmap value.
    pub const fn actual(self) -> usize {
        self.actual
    }

    /// Minimum number of bytes or items accepted by the schema.
    pub const fn minimum(self) -> usize {
        self.minimum
    }

    /// Maximum number of bytes/items, or the allowed bitmap mask.
    pub const fn maximum(self) -> usize {
        self.maximum
    }
}

#[doc(hidden)]
pub struct TaggedField<T, const MAX_LEN: usize> {
    bytes: [u8; MAX_LEN],
    len: usize,
    _value: core::marker::PhantomData<T>,
}

#[doc(hidden)]
pub struct PayloadField<T, const MIN_LEN: usize, const MAX_LEN: usize> {
    bytes: [u8; MAX_LEN],
    len: usize,
    _value: core::marker::PhantomData<T>,
}

impl<T, const MIN_LEN: usize, const MAX_LEN: usize> PayloadField<T, MIN_LEN, MAX_LEN>
where
    T: HciEncodePayload,
{
    pub fn try_new(value: T) -> Result<Self, HciLengthError> {
        let len = value.hci_payload_size();
        if T::MIN_LEN != MIN_LEN || T::MAX_LEN != MAX_LEN || !(MIN_LEN..=MAX_LEN).contains(&len) {
            return Err(HciLengthError::new(len, T::MIN_LEN, T::MAX_LEN));
        }

        let mut bytes = [0; MAX_LEN];
        let mut remaining = &mut bytes[..len];
        if value.write_hci_payload(&mut remaining).is_err() {
            return Err(HciLengthError::new(len.saturating_add(1), MIN_LEN, MAX_LEN));
        }
        if !remaining.is_empty() {
            return Err(HciLengthError::new(len - remaining.len(), len, len));
        }

        Ok(Self {
            bytes,
            len,
            _value: core::marker::PhantomData,
        })
    }
}

impl<T, const MAX_LEN: usize> TaggedField<T, MAX_LEN> {
    pub fn try_new<const MIN_LEN: usize, Fields>(fields: Fields) -> Result<Self, HciLengthError>
    where
        Fields: DeclarativeFieldList,
    {
        let len = fields.size();
        if !(MIN_LEN..=MAX_LEN).contains(&len) {
            return Err(HciLengthError::new(len, MIN_LEN, MAX_LEN));
        }

        let mut bytes = [0; MAX_LEN];
        let mut remaining = &mut bytes[..len];
        if fields.write(&mut remaining).is_err() {
            return Err(HciLengthError::new(len.saturating_add(1), MIN_LEN, MAX_LEN));
        }
        if !remaining.is_empty() {
            return Err(HciLengthError::new(len - remaining.len(), len, len));
        }

        Ok(Self {
            bytes,
            len,
            _value: core::marker::PhantomData,
        })
    }
}

#[doc(hidden)]
pub trait HciBitmap: Copy {
    fn to_usize(self) -> usize;
}

impl HciBitmap for u8 {
    fn to_usize(self) -> usize {
        usize::from(self)
    }
}

impl HciBitmap for u16 {
    fn to_usize(self) -> usize {
        usize::from(self)
    }
}

impl HciBitmap for u32 {
    fn to_usize(self) -> usize {
        self as usize
    }
}

#[doc(hidden)]
pub struct BitmapItems<T, Item, const ITEM_LEN: usize, const MAX_ITEMS: usize> {
    value: T,
    _item: core::marker::PhantomData<Item>,
}

impl<T, Item, const ITEM_LEN: usize, const MAX_ITEMS: usize>
    BitmapItems<T, Item, ITEM_LEN, MAX_ITEMS>
where
    T: AsRef<[Item]>,
{
    pub fn try_new<B: HciBitmap>(
        value: T,
        bitmap: B,
        allowed_mask: usize,
    ) -> Result<Self, HciLengthError> {
        let bitmap = bitmap.to_usize();
        if bitmap & !allowed_mask != 0 {
            return Err(HciLengthError::new(bitmap, 0, allowed_mask));
        }

        let expected = (bitmap & allowed_mask).count_ones() as usize;
        let actual = value.as_ref().len();
        if actual != expected || actual > MAX_ITEMS {
            return Err(HciLengthError::new(actual, expected, expected));
        }
        Ok(Self {
            value,
            _item: core::marker::PhantomData,
        })
    }
}

#[doc(hidden)]
pub trait HciCount<const N: usize>: HciEncodeField<N> + Copy {
    const MAX: usize;

    fn from_usize(value: usize) -> Option<Self>;

    fn to_usize(self) -> usize;
}

impl HciCount<1> for u8 {
    const MAX: usize = u8::MAX as usize;

    fn from_usize(value: usize) -> Option<Self> {
        value.try_into().ok()
    }

    fn to_usize(self) -> usize {
        usize::from(self)
    }
}

impl HciCount<2> for u16 {
    const MAX: usize = u16::MAX as usize;

    fn from_usize(value: usize) -> Option<Self> {
        value.try_into().ok()
    }

    fn to_usize(self) -> usize {
        usize::from(self)
    }
}

#[doc(hidden)]
pub struct CountedBytes<T, C, const COUNT_LEN: usize, const MAX_LEN: usize> {
    value: T,
    count: C,
}

impl<T, C, const COUNT_LEN: usize, const MAX_LEN: usize> CountedBytes<T, C, COUNT_LEN, MAX_LEN>
where
    T: AsRef<[u8]>,
    C: HciCount<COUNT_LEN>,
{
    pub fn try_new(value: T) -> Result<Self, HciLengthError> {
        let actual = value.as_ref().len();
        let maximum = core::cmp::min(MAX_LEN, C::MAX);
        let count = C::from_usize(actual).ok_or(HciLengthError::new(actual, 0, maximum))?;
        if actual > MAX_LEN {
            return Err(HciLengthError::new(actual, 0, maximum));
        }
        Ok(Self { value, count })
    }
}

#[doc(hidden)]
pub struct TrailingBytes<T, const MIN_LEN: usize, const MAX_LEN: usize> {
    value: T,
}

impl<T, const MIN_LEN: usize, const MAX_LEN: usize> TrailingBytes<T, MIN_LEN, MAX_LEN>
where
    T: AsRef<[u8]>,
{
    pub fn try_new(value: T) -> Result<Self, HciLengthError> {
        let actual = value.as_ref().len();
        if !(MIN_LEN..=MAX_LEN).contains(&actual) {
            return Err(HciLengthError::new(actual, MIN_LEN, MAX_LEN));
        }
        Ok(Self { value })
    }
}

#[doc(hidden)]
pub struct CountedItems<
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
> {
    value: T,
    count: C,
    _item: core::marker::PhantomData<Item>,
}

impl<T, Item, C, const COUNT_LEN: usize, const ITEM_LEN: usize, const MAX_ITEMS: usize>
    CountedItems<T, Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>
where
    T: AsRef<[Item]>,
    C: HciCount<COUNT_LEN>,
{
    pub fn try_new(value: T) -> Result<Self, HciLengthError> {
        let actual = value.as_ref().len();
        let maximum = core::cmp::min(MAX_ITEMS, C::MAX);
        let count = C::from_usize(actual).ok_or(HciLengthError::new(actual, 0, maximum))?;
        if actual > MAX_ITEMS {
            return Err(HciLengthError::new(actual, 0, maximum));
        }
        Ok(Self {
            value,
            count,
            _item: core::marker::PhantomData,
        })
    }
}

/// Owned, bounded bytes decoded from a variable-length HCI response field.
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

/// Owned, allocation-free items decoded from a counted HCI response field.
#[derive(Clone, Copy)]
pub struct BoundedItems<T: Copy, const MAX_ITEMS: usize> {
    items: [core::mem::MaybeUninit<T>; MAX_ITEMS],
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

    pub(crate) fn from_array_prefix(
        values: [T; MAX_ITEMS],
        len: usize,
    ) -> Result<Self, HciLengthError> {
        if len > MAX_ITEMS {
            return Err(HciLengthError::new(len, 0, MAX_ITEMS));
        }
        let mut items = [core::mem::MaybeUninit::uninit(); MAX_ITEMS];
        for (slot, value) in items.iter_mut().zip(values) {
            slot.write(value);
        }
        Ok(Self { items, len })
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

#[doc(hidden)]
pub fn decode_declarative_fixed_field<T, const N: usize>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeField<N>,
{
    if data.len() < N {
        return Err(bt_hci::FromHciBytesError::InvalidSize);
    }
    let (field, rest) = data.split_at(N);
    let field: &[u8; N] = field
        .try_into()
        .map_err(|_| bt_hci::FromHciBytesError::InvalidSize)?;
    T::from_hci_field(field).map(|value| (value, rest))
}

#[doc(hidden)]
pub trait HciDecodeCountedBytes<C, const COUNT_LEN: usize, const MAX_LEN: usize>: Sized {
    fn decode_counted_bytes(data: &[u8]) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError>;
}

#[doc(hidden)]
pub trait HciDecodeTrailingBytes<const MIN_LEN: usize, const MAX_LEN: usize>: Sized {
    fn decode_trailing_bytes(data: &[u8]) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError>;
}

#[doc(hidden)]
pub trait HciDecodeCountedItems<
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>: Sized
{
    fn decode_counted_items(data: &[u8]) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError>;
}

#[doc(hidden)]
pub fn decode_declarative_counted_bytes<T, C, const COUNT_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeCountedBytes<C, COUNT_LEN, MAX_LEN>,
{
    T::decode_counted_bytes(data)
}

#[doc(hidden)]
pub fn decode_declarative_trailing_bytes<T, const MIN_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeTrailingBytes<MIN_LEN, MAX_LEN>,
{
    T::decode_trailing_bytes(data)
}

#[doc(hidden)]
pub fn decode_declarative_counted_items<
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MAX_ITEMS: usize,
>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeCountedItems<Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>,
{
    T::decode_counted_items(data)
}

#[doc(hidden)]
pub fn decode_declarative_payload<T, const MIN_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodePayload,
{
    if T::MIN_LEN != MIN_LEN || T::MAX_LEN != MAX_LEN {
        return Err(bt_hci::FromHciBytesError::InvalidValue);
    }
    let initial_len = data.len();
    let (value, rest) = T::from_hci_payload(data)?;
    let consumed = initial_len - rest.len();
    if !(MIN_LEN..=MAX_LEN).contains(&consumed) {
        return Err(bt_hci::FromHciBytesError::InvalidSize);
    }
    Ok((value, rest))
}

impl<C, const COUNT_LEN: usize, const MAX_LEN: usize> HciDecodeCountedBytes<C, COUNT_LEN, MAX_LEN>
    for BoundedBytes<MAX_LEN>
where
    C: HciCount<COUNT_LEN> + HciDecodeField<COUNT_LEN>,
{
    fn decode_counted_bytes(data: &[u8]) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError> {
        let (count, data) = decode_declarative_fixed_field::<C, COUNT_LEN>(data)?;
        let len = count.to_usize();
        if len > MAX_LEN {
            return Err(bt_hci::FromHciBytesError::InvalidValue);
        }
        if data.len() < len {
            return Err(bt_hci::FromHciBytesError::InvalidSize);
        }
        let (value, rest) = data.split_at(len);
        let mut bytes = [0; MAX_LEN];
        bytes[..len].copy_from_slice(value);
        Ok((Self { bytes, len }, rest))
    }
}

impl<const MIN_LEN: usize, const MAX_LEN: usize> HciDecodeTrailingBytes<MIN_LEN, MAX_LEN>
    for BoundedBytes<MAX_LEN>
{
    fn decode_trailing_bytes(data: &[u8]) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError> {
        let len = data.len();
        if !(MIN_LEN..=MAX_LEN).contains(&len) {
            return Err(bt_hci::FromHciBytesError::InvalidSize);
        }
        let mut bytes = [0; MAX_LEN];
        bytes[..len].copy_from_slice(data);
        Ok((Self { bytes, len }, &[]))
    }
}

impl<Item, C, const COUNT_LEN: usize, const ITEM_LEN: usize, const MAX_ITEMS: usize>
    HciDecodeCountedItems<Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS> for BoundedItems<Item, MAX_ITEMS>
where
    Item: Copy + HciDecodeField<ITEM_LEN>,
    C: HciCount<COUNT_LEN> + HciDecodeField<COUNT_LEN>,
{
    fn decode_counted_items(data: &[u8]) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError> {
        let (count, mut data) = decode_declarative_fixed_field::<C, COUNT_LEN>(data)?;
        let len = count.to_usize();
        if len > MAX_ITEMS {
            return Err(bt_hci::FromHciBytesError::InvalidValue);
        }
        let required = ITEM_LEN
            .checked_mul(len)
            .ok_or(bt_hci::FromHciBytesError::InvalidSize)?;
        if data.len() < required {
            return Err(bt_hci::FromHciBytesError::InvalidSize);
        }

        let mut items = [core::mem::MaybeUninit::uninit(); MAX_ITEMS];
        for slot in items.iter_mut().take(len) {
            let (item, rest) = decode_declarative_fixed_field::<Item, ITEM_LEN>(data)?;
            slot.write(item);
            data = rest;
        }
        Ok((Self { items, len }, data))
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

#[doc(hidden)]
pub const fn assert_hci_field_list_length(length: usize) {
    ::core::assert!(length <= u8::MAX as usize);
}

#[doc(hidden)]
pub trait DeclarativeFieldList {
    fn size(&self) -> usize;

    fn write<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error>;

    async fn write_async<W: embedded_io_async::Write>(&self, writer: W) -> Result<(), W::Error>;
}

#[doc(hidden)]
pub trait DeclarativeEncodedField {
    fn size(&self) -> usize;

    fn write<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error>;

    async fn write_async<W: embedded_io_async::Write>(&self, writer: W) -> Result<(), W::Error>;
}

impl<T, const N: usize> DeclarativeEncodedField for DeclarativeField<T, N>
where
    T: HciEncodeField<N>,
{
    fn size(&self) -> usize {
        N
    }

    fn write<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.0.write_hci_field(writer)
    }

    async fn write_async<W: embedded_io_async::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.0.write_hci_field_async(writer).await
    }
}

impl<T, const MAX_LEN: usize> DeclarativeEncodedField for TaggedField<T, MAX_LEN> {
    fn size(&self) -> usize {
        self.len
    }

    fn write<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&self.bytes[..self.len])
    }

    async fn write_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&self.bytes[..self.len]).await
    }
}

impl<T, const MIN_LEN: usize, const MAX_LEN: usize> DeclarativeEncodedField
    for PayloadField<T, MIN_LEN, MAX_LEN>
{
    fn size(&self) -> usize {
        self.len
    }

    fn write<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(&self.bytes[..self.len])
    }

    async fn write_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(&self.bytes[..self.len]).await
    }
}

impl<T, C, const COUNT_LEN: usize, const MAX_LEN: usize> DeclarativeEncodedField
    for CountedBytes<T, C, COUNT_LEN, MAX_LEN>
where
    T: AsRef<[u8]>,
    C: HciCount<COUNT_LEN>,
{
    fn size(&self) -> usize {
        COUNT_LEN + self.value.as_ref().len()
    }

    fn write<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        self.count.write_hci_field(&mut writer)?;
        writer.write_all(self.value.as_ref())
    }

    async fn write_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        self.count.write_hci_field_async(&mut writer).await?;
        writer.write_all(self.value.as_ref()).await
    }
}

impl<T, const MIN_LEN: usize, const MAX_LEN: usize> DeclarativeEncodedField
    for TrailingBytes<T, MIN_LEN, MAX_LEN>
where
    T: AsRef<[u8]>,
{
    fn size(&self) -> usize {
        self.value.as_ref().len()
    }

    fn write<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        writer.write_all(self.value.as_ref())
    }

    async fn write_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        writer.write_all(self.value.as_ref()).await
    }
}

impl<T, Item, C, const COUNT_LEN: usize, const ITEM_LEN: usize, const MAX_ITEMS: usize>
    DeclarativeEncodedField for CountedItems<T, Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>
where
    T: AsRef<[Item]>,
    Item: HciEncodeField<ITEM_LEN>,
    C: HciCount<COUNT_LEN>,
{
    fn size(&self) -> usize {
        COUNT_LEN + ITEM_LEN * self.value.as_ref().len()
    }

    fn write<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        self.count.write_hci_field(&mut writer)?;
        for item in self.value.as_ref() {
            item.write_hci_field(&mut writer)?;
        }
        Ok(())
    }

    async fn write_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        self.count.write_hci_field_async(&mut writer).await?;
        for item in self.value.as_ref() {
            item.write_hci_field_async(&mut writer).await?;
        }
        Ok(())
    }
}

impl<T, Item, const ITEM_LEN: usize, const MAX_ITEMS: usize> DeclarativeEncodedField
    for BitmapItems<T, Item, ITEM_LEN, MAX_ITEMS>
where
    T: AsRef<[Item]>,
    Item: HciEncodeField<ITEM_LEN>,
{
    fn size(&self) -> usize {
        ITEM_LEN * self.value.as_ref().len()
    }

    fn write<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        for item in self.value.as_ref() {
            item.write_hci_field(&mut writer)?;
        }
        Ok(())
    }

    async fn write_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        for item in self.value.as_ref() {
            item.write_hci_field_async(&mut writer).await?;
        }
        Ok(())
    }
}

impl DeclarativeFieldList for () {
    #[inline]
    fn size(&self) -> usize {
        0
    }

    #[inline]
    fn write<W: embedded_io::Write>(&self, _writer: W) -> Result<(), W::Error> {
        Ok(())
    }

    #[inline]
    async fn write_async<W: embedded_io_async::Write>(&self, _writer: W) -> Result<(), W::Error> {
        Ok(())
    }
}

impl<Head, Tail> DeclarativeFieldList for (Head, Tail)
where
    Head: DeclarativeEncodedField,
    Tail: DeclarativeFieldList,
{
    #[inline]
    fn size(&self) -> usize {
        self.0.size() + self.1.size()
    }

    #[inline]
    fn write<W: embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
        self.0.write(&mut writer)?;
        self.1.write(writer)
    }

    #[inline]
    async fn write_async<W: embedded_io_async::Write>(
        &self,
        mut writer: W,
    ) -> Result<(), W::Error> {
        self.0.write_async(&mut writer).await?;
        self.1.write_async(writer).await
    }
}

impl<T: DeclarativeFieldList> WriteHci for DeclarativeParams<T> {
    #[inline]
    fn size(&self) -> usize {
        self.0.size()
    }

    #[inline]
    fn write_hci<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.0.write(writer)
    }

    #[inline]
    async fn write_hci_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        self.0.write_async(writer).await
    }
}

/// Defines a reusable tagged enum or fixed struct wire payload and generates
/// its Rust type, encoder, decoder, and length metadata from one declaration.
macro_rules! vendor_payload {
    (
        $(#[$enum_attr:meta])*
        $vis:vis enum $payload:ident {
            Tag = $tag_ty:ty => $tag_len:literal;
            $($(#[$variant_attr:meta])*
            $variant:ident(
                $($field:ident: $field_ty:ty => $field_len:literal),+ $(,)?
            ) = $tag:literal;)+
        }
    ) => {
        $(#[$enum_attr])*
        #[derive(Copy, Clone, Debug, PartialEq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        $vis enum $payload {
            $($(#[$variant_attr])*
            $variant($($field_ty),+),)+
        }

        impl crate::vendor::command::HciEncodePayload for $payload {
            const MIN_LEN: usize = {
                let minimum = usize::MAX;
                $(let candidate = $tag_len $(+ $field_len)+;
                let minimum = if candidate < minimum {
                    candidate
                } else {
                    minimum
                };)+
                minimum
            };

            const MAX_LEN: usize = {
                let maximum = 0usize;
                $(let candidate = $tag_len $(+ $field_len)+;
                let maximum = if candidate > maximum {
                    candidate
                } else {
                    maximum
                };)+
                maximum
            };

            fn hci_payload_size(&self) -> usize {
                match self {
                    $(Self::$variant(..) => $tag_len $(+ $field_len)+,)+
                }
            }

            fn write_hci_payload<W: embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                match self {
                    $(Self::$variant($($field),+) => {
                        let tag: $tag_ty = $tag;
                        <$tag_ty as crate::vendor::command::HciEncodeField<$tag_len>>::write_hci_field(
                            &tag,
                            &mut writer,
                        )?;
                        $(<$field_ty as crate::vendor::command::HciEncodeField<$field_len>>::write_hci_field(
                            $field,
                            &mut writer,
                        )?;)+
                        Ok(())
                    },)+
                }
            }

            async fn write_hci_payload_async<W: embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                match self {
                    $(Self::$variant($($field),+) => {
                        let tag: $tag_ty = $tag;
                        <$tag_ty as crate::vendor::command::HciEncodeField<$tag_len>>::write_hci_field_async(
                            &tag,
                            &mut writer,
                        ).await?;
                        $(<$field_ty as crate::vendor::command::HciEncodeField<$field_len>>::write_hci_field_async(
                            $field,
                            &mut writer,
                        ).await?;)+
                        Ok(())
                    },)+
                }
            }
        }

        impl crate::vendor::command::HciDecodePayload for $payload {
            const MIN_LEN: usize =
                <Self as crate::vendor::command::HciEncodePayload>::MIN_LEN;
            const MAX_LEN: usize =
                <Self as crate::vendor::command::HciEncodePayload>::MAX_LEN;

            fn from_hci_payload(
                data: &[u8],
            ) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError> {
                let (tag, data) = crate::vendor::command::decode_declarative_fixed_field::<
                    $tag_ty,
                    $tag_len,
                >(data)?;
                match tag {
                    $($tag => {
                        $(let ($field, data) =
                            crate::vendor::command::decode_declarative_fixed_field::<
                                $field_ty,
                                $field_len,
                            >(data)?;)+
                        Ok((Self::$variant($($field),+), data))
                    },)+
                    _ => Err(bt_hci::FromHciBytesError::InvalidValue),
                }
            }
        }

        impl<'de> bt_hci::FromHciBytes<'de> for $payload {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> Result<(Self, &'de [u8]), bt_hci::FromHciBytesError> {
                <Self as crate::vendor::command::HciDecodePayload>::from_hci_payload(data)
            }
        }

        const _: () = ::core::assert!(
            <$payload as crate::vendor::command::HciEncodePayload>::MAX_LEN
                <= u8::MAX as usize
        );
    };
    (
        $(#[$struct_attr:meta])*
        $vis:vis struct $payload:ident {
            $($(#[$field_attr:meta])*
            $field:ident: $field_ty:ty => $field_len:literal,)*
        }
    ) => {
        $(#[$struct_attr])*
        #[derive(Copy, Clone, Debug)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        $vis struct $payload {
            $($(#[$field_attr])*
            pub $field: $field_ty,)*
        }

        impl crate::vendor::command::HciEncodePayload for $payload {
            const MIN_LEN: usize = 0 $(+ $field_len)*;
            const MAX_LEN: usize = 0 $(+ $field_len)*;

            fn hci_payload_size(&self) -> usize {
                <Self as crate::vendor::command::HciEncodePayload>::MAX_LEN
            }

            fn write_hci_payload<W: embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                $(<$field_ty as crate::vendor::command::HciEncodeField<$field_len>>::write_hci_field(
                    &self.$field,
                    &mut writer,
                )?;)*
                Ok(())
            }

            async fn write_hci_payload_async<W: embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                $(<$field_ty as crate::vendor::command::HciEncodeField<$field_len>>::write_hci_field_async(
                    &self.$field,
                    &mut writer,
                ).await?;)*
                Ok(())
            }
        }

        impl crate::vendor::command::HciDecodePayload for $payload {
            const MIN_LEN: usize =
                <Self as crate::vendor::command::HciEncodePayload>::MIN_LEN;
            const MAX_LEN: usize =
                <Self as crate::vendor::command::HciEncodePayload>::MAX_LEN;

            fn from_hci_payload(
                data: &[u8],
            ) -> Result<(Self, &[u8]), bt_hci::FromHciBytesError> {
                $(let ($field, data) =
                    crate::vendor::command::decode_declarative_fixed_field::<
                        $field_ty,
                        $field_len,
                    >(data)?;)*
                Ok((Self { $($field,)* }, data))
            }
        }

        impl<'de> bt_hci::FromHciBytes<'de> for $payload {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> Result<(Self, &'de [u8]), bt_hci::FromHciBytesError> {
                <Self as crate::vendor::command::HciDecodePayload>::from_hci_payload(data)
            }
        }

        const _: () = ::core::assert!(
            <$payload as crate::vendor::command::HciEncodePayload>::MAX_LEN
                <= u8::MAX as usize
        );
    };
}

macro_rules! declarative_field_list_type {
    () => {
        ()
    };
    ($ty:ty => $len:literal, $($rest:tt)*) => {
        (
            crate::vendor::command::DeclarativeField<$ty, $len>,
            declarative_field_list_type!($($rest)*)
        )
    };
}

macro_rules! declarative_field_list_value {
    () => {
        ()
    };
    ($name:ident => $len:literal, $($rest:tt)*) => {
        (
            crate::vendor::command::DeclarativeField::<_, $len>($name),
            declarative_field_list_value!($($rest)*)
        )
    };
}

macro_rules! declarative_schema_field_type {
    ($ty:ty, $len:literal) => {
        crate::vendor::command::DeclarativeField<$ty, $len>
    };
    (
        $ty:ty,
        {
            kind: counted_bytes,
            count: $count_ty:ty => $count_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        crate::vendor::command::CountedBytes<$ty, $count_ty, $count_len, $max_len>
    };
    (
        $ty:ty,
        {
            kind: trailing_bytes,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        crate::vendor::command::TrailingBytes<$ty, $min_len, $max_len>
    };
    (
        $ty:ty,
        {
            kind: counted_items,
            count: $count_ty:ty => $count_len:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        crate::vendor::command::CountedItems<
            $ty,
            $item_ty,
            $count_ty,
            $count_len,
            $item_len,
            $max_items,
        >
    };
    (
        $ty:ty,
        {
            kind: tagged,
            tag: $tag_ty:ty => $tag_len:literal,
            variants: {
                $($variant_pattern:pat => {
                    tag: $tag:literal,
                    fields: {
                        $($variant_field:ident: $variant_ty:ty => $variant_len:literal,)*
                    },
                },)*
            },
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        crate::vendor::command::TaggedField<$ty, $max_len>
    };
    (
        $ty:ty,
        {
            kind: payload,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        crate::vendor::command::PayloadField<$ty, $min_len, $max_len>
    };
    (
        $ty:ty,
        {
            kind: bitmap_items,
            bitmap: $bitmap:ident,
            mask: $mask:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        crate::vendor::command::BitmapItems<$ty, $item_ty, $item_len, $max_items>
    };
}

#[allow(unused_macros)]
macro_rules! declarative_tagged_payload_value {
    () => {
        ()
    };
    ($value:ident: $ty:ty => $len:literal, $($rest:tt)*) => {
        (
            crate::vendor::command::DeclarativeField::<&$ty, $len>($value),
            declarative_tagged_payload_value!($($rest)*)
        )
    };
}

macro_rules! declarative_schema_field_value {
    ($value:ident: $ty:ty, $len:literal) => {
        crate::vendor::command::DeclarativeField::<_, $len>($value)
    };
    (
        $value:ident: $ty:ty,
        {
            kind: counted_bytes,
            count: $count_ty:ty => $count_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        crate::vendor::command::CountedBytes::<_, $count_ty, $count_len, $max_len>::try_new($value)?
    };
    (
        $value:ident: $ty:ty,
        {
            kind: trailing_bytes,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        crate::vendor::command::TrailingBytes::<_, $min_len, $max_len>::try_new($value)?
    };
    (
        $value:ident: $ty:ty,
        {
            kind: counted_items,
            count: $count_ty:ty => $count_len:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        crate::vendor::command::CountedItems::<
            _,
            $item_ty,
            $count_ty,
            $count_len,
            $item_len,
            $max_items,
        >::try_new($value)?
    };
    (
        $value:ident: $ty:ty,
        {
            kind: tagged,
            tag: $tag_ty:ty => $tag_len:literal,
            variants: {
                $($variant_pattern:pat => {
                    tag: $tag:literal,
                    fields: {
                        $($variant_field:ident: $variant_ty:ty => $variant_len:literal,)*
                    },
                },)*
            },
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        match &$value {
            $($variant_pattern => {
                let tag: $tag_ty = $tag;
                crate::vendor::command::TaggedField::<$ty, $max_len>::try_new::<$min_len, _>((
                    crate::vendor::command::DeclarativeField::<$tag_ty, $tag_len>(tag),
                    declarative_tagged_payload_value!(
                        $($variant_field: $variant_ty => $variant_len,)*
                    ),
                ))?
            },)*
        }
    };
    (
        $value:ident: $ty:ty,
        {
            kind: payload,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        crate::vendor::command::PayloadField::<$ty, $min_len, $max_len>::try_new($value)?
    };
    (
        $value:ident: $ty:ty,
        {
            kind: bitmap_items,
            bitmap: $bitmap:ident,
            mask: $mask:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        crate::vendor::command::BitmapItems::<_, $item_ty, $item_len, $max_items>::try_new(
            $value, $bitmap, $mask,
        )?
    };
}

macro_rules! declarative_schema_max_len {
    ($len:literal) => {
        $len
    };
    (
        {
            kind: counted_bytes,
            count: $count_ty:ty => $count_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        $count_len + $max_len
    };
    (
        {
            kind: trailing_bytes,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        $max_len
    };
    (
        {
            kind: counted_items,
            count: $count_ty:ty => $count_len:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        $count_len + $item_len * $max_items
    };
    (
        {
            kind: tagged,
            tag: $tag_ty:ty => $tag_len:literal,
            variants: {
                $($variant_pattern:pat => {
                    tag: $tag:literal,
                    fields: {
                        $($variant_field:ident: $variant_ty:ty => $variant_len:literal,)*
                    },
                },)*
            },
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        $max_len
    };
    (
        {
            kind: payload,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        $max_len
    };
    (
        {
            kind: bitmap_items,
            bitmap: $bitmap:ident,
            mask: $mask:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        $item_len * $max_items
    };
}

macro_rules! declarative_schema_validate {
    ($len:literal) => {
        const _: () = ();
    };
    (
        {
            kind: counted_bytes,
            count: $count_ty:ty => $count_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        const _: () = ::core::assert!(
            $max_len
                <= <$count_ty as crate::vendor::command::HciCount<$count_len>>::MAX
        );
    };
    (
        {
            kind: trailing_bytes,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        const _: () = ::core::assert!($min_len <= $max_len);
    };
    (
        {
            kind: counted_items,
            count: $count_ty:ty => $count_len:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        const _: () = ::core::assert!(
            $max_items
                <= <$count_ty as crate::vendor::command::HciCount<$count_len>>::MAX
        );
    };
    (
        {
            kind: tagged,
            tag: $tag_ty:ty => $tag_len:literal,
            variants: {
                $($variant_pattern:pat => {
                    tag: $tag:literal,
                    fields: {
                        $($variant_field:ident: $variant_ty:ty => $variant_len:literal,)*
                    },
                },)*
            },
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        #[allow(clippy::int_plus_one)]
        const _: () = {
            ::core::assert!($min_len <= $max_len);
            $(::core::assert!(
                $tag_len $(+ $variant_len)* >= $min_len
                    && $tag_len $(+ $variant_len)* <= $max_len
            );)*
            ::core::assert!(false $(|| $tag_len $(+ $variant_len)* == $min_len)*);
            ::core::assert!(false $(|| $tag_len $(+ $variant_len)* == $max_len)*);
        };
    };
    (
        {
            kind: payload,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        }
    ) => {
        const _: () = ::core::assert!($min_len <= $max_len);
    };
    (
        {
            kind: bitmap_items,
            bitmap: $bitmap:ident,
            mask: $mask:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        }
    ) => {
        const _: () = ::core::assert!(($mask as usize).count_ones() as usize == $max_items);
    };
}

macro_rules! declarative_schema_list_type {
    () => {
        ()
    };
    ($ty:ty => $shape:tt, $($rest:tt)*) => {
        (
            declarative_schema_field_type!($ty, $shape),
            declarative_schema_list_type!($($rest)*)
        )
    };
}

macro_rules! declarative_schema_list_value {
    () => {
        ()
    };
    ($value:ident: $ty:ty => $shape:tt, $($rest:tt)*) => {
        (
            declarative_schema_field_value!($value: $ty, $shape),
            declarative_schema_list_value!($($rest)*)
        )
    };
}

macro_rules! decode_declarative_schema_field {
    ($ty:ty, $len:literal, $data:ident) => {
        crate::vendor::command::decode_declarative_fixed_field::<$ty, $len>($data)
    };
    (
        $ty:ty,
        {
            kind: counted_bytes,
            count: $count_ty:ty => $count_len:literal,
            max_len: $max_len:literal,
        },
        $data:ident
    ) => {
        crate::vendor::command::decode_declarative_counted_bytes::<
            $ty,
            $count_ty,
            $count_len,
            $max_len,
        >($data)
    };
    (
        $ty:ty,
        {
            kind: trailing_bytes,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        },
        $data:ident
    ) => {
        crate::vendor::command::decode_declarative_trailing_bytes::<$ty, $min_len, $max_len>($data)
    };
    (
        $ty:ty,
        {
            kind: counted_items,
            count: $count_ty:ty => $count_len:literal,
            item: $item_ty:ty => $item_len:literal,
            max_items: $max_items:literal,
        },
        $data:ident
    ) => {
        crate::vendor::command::decode_declarative_counted_items::<
            $ty,
            $item_ty,
            $count_ty,
            $count_len,
            $item_len,
            $max_items,
        >($data)
    };
    (
        $ty:ty,
        {
            kind: payload,
            min_len: $min_len:literal,
            max_len: $max_len:literal,
        },
        $data:ident
    ) => {
        crate::vendor::command::decode_declarative_payload::<$ty, $min_len, $max_len>($data)
    };
}

macro_rules! declarative_return {
    (
        $ret:ident {
            $($field:ident: $ty:ty => $shape:tt,)*
        }
    ) => {
        #[derive(Copy, Clone, Debug)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        #[allow(missing_docs)]
        pub struct $ret {
            $(pub $field: $ty,)*
        }

        impl<'de> ::bt_hci::FromHciBytes<'de> for $ret {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> Result<(Self, &'de [u8]), ::bt_hci::FromHciBytesError> {
                $(
                    let ($field, data) = decode_declarative_schema_field!($ty, $shape, data)?;
                )*

                Ok((Self { $($field,)* }, data))
            }
        }
    };
}

macro_rules! declarative_command {
    (
        $cmd:ident($opcode:ident) {
            $($field:ident: $ty:ty => $len:literal,)*
        }
        Return = $ret:ty;
        ReturnLen = $ret_len:expr;
    ) => {
        const _: () = crate::vendor::command::assert_hci_field_list_length(0 $(+ $len)*);
        const _: () = crate::vendor::command::assert_hci_field_list_length($ret_len);

        #[allow(missing_docs)]
        pub struct $cmd(
            crate::vendor::command::DeclarativeParams<
                declarative_field_list_type!($($ty => $len,)*)
            >
        );

        impl $cmd {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn new($($field: $ty),*) -> Self {
                Self(crate::vendor::command::DeclarativeParams(
                    declarative_field_list_value!($($field => $len,)*)
                ))
            }
        }

        impl ::bt_hci::cmd::Cmd for $cmd {
            const OPCODE: ::bt_hci::cmd::Opcode = ::bt_hci::cmd::Opcode::new(
                ::bt_hci::cmd::OpcodeGroup::VENDOR_SPECIFIC,
                crate::vendor::opcode::$opcode.ocf(),
            );
            type Params = crate::vendor::command::DeclarativeParams<
                declarative_field_list_type!($($ty => $len,)*)
            >;

            fn params(&self) -> &Self::Params {
                &self.0
            }
        }

        impl ::bt_hci::WriteHci for $cmd {
            #[inline]
            fn size(&self) -> usize {
                <Self as ::bt_hci::cmd::Cmd>::params(self).size() + 3
            }

            fn write_hci<W: ::embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))?;
                <Self as ::bt_hci::cmd::Cmd>::params(self).write_hci(writer)
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer
                    .write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))
                    .await?;
                <Self as ::bt_hci::cmd::Cmd>::params(self)
                    .write_hci_async(writer)
                    .await
            }
        }

        impl ::bt_hci::cmd::SyncCmd for $cmd {
            type Return = $ret;
            type Handle = ();
            type ReturnBuf = [u8; $ret_len];

            fn param_handle(&self) {}

            fn return_handle(
                _data: &[u8],
            ) -> Result<Self::Handle, ::bt_hci::FromHciBytesError> {
                Ok(())
            }
        }
    };
    (
        $cmd:ident($opcode:ident) {
            $($field:ident: $ty:ty => $len:literal,)*
        }
        CommandStatus;
    ) => {
        const _: () = crate::vendor::command::assert_hci_field_list_length(0 $(+ $len)*);

        #[allow(missing_docs)]
        pub struct $cmd(
            crate::vendor::command::DeclarativeParams<
                declarative_field_list_type!($($ty => $len,)*)
            >
        );

        impl $cmd {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn new($($field: $ty),*) -> Self {
                Self(crate::vendor::command::DeclarativeParams(
                    declarative_field_list_value!($($field => $len,)*)
                ))
            }
        }

        impl ::bt_hci::cmd::Cmd for $cmd {
            const OPCODE: ::bt_hci::cmd::Opcode = ::bt_hci::cmd::Opcode::new(
                ::bt_hci::cmd::OpcodeGroup::VENDOR_SPECIFIC,
                crate::vendor::opcode::$opcode.ocf(),
            );
            type Params = crate::vendor::command::DeclarativeParams<
                declarative_field_list_type!($($ty => $len,)*)
            >;

            fn params(&self) -> &Self::Params {
                &self.0
            }
        }

        impl ::bt_hci::WriteHci for $cmd {
            #[inline]
            fn size(&self) -> usize {
                <Self as ::bt_hci::cmd::Cmd>::params(self).size() + 3
            }

            fn write_hci<W: ::embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))?;
                <Self as ::bt_hci::cmd::Cmd>::params(self).write_hci(writer)
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer
                    .write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))
                    .await?;
                <Self as ::bt_hci::cmd::Cmd>::params(self)
                    .write_hci_async(writer)
                    .await
            }
        }

        impl ::bt_hci::cmd::AsyncCmd for $cmd {}
    };
}

macro_rules! declarative_variable_command {
    (
        $cmd:ident<$life:lifetime>($opcode:ident) {
            $($field:ident: $ty:ty => $shape:tt,)*
        }
        Return = $ret:ty;
        ReturnLen = $ret_len:expr;
    ) => {
        $(declarative_schema_validate!($shape);)*
        const _: () = crate::vendor::command::assert_hci_field_list_length($ret_len);

        #[allow(missing_docs)]
        pub struct $cmd<$life>(
            crate::vendor::command::DeclarativeParams<
                declarative_schema_list_type!($($ty => $shape,)*)
            >
        );

        impl<$life> $cmd<$life> {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn try_new(
                $($field: $ty),*
            ) -> Result<Self, crate::vendor::command::HciLengthError> {
                let params = crate::vendor::command::DeclarativeParams(
                    declarative_schema_list_value!($($field: $ty => $shape,)*)
                );
                let actual = crate::vendor::command::DeclarativeFieldList::size(&params.0);
                if actual > u8::MAX as usize {
                    return Err(crate::vendor::command::HciLengthError::new(
                        actual,
                        0,
                        u8::MAX as usize,
                    ));
                }
                Ok(Self(params))
            }
        }

        impl<$life> ::bt_hci::cmd::Cmd for $cmd<$life> {
            const OPCODE: ::bt_hci::cmd::Opcode = ::bt_hci::cmd::Opcode::new(
                ::bt_hci::cmd::OpcodeGroup::VENDOR_SPECIFIC,
                crate::vendor::opcode::$opcode.ocf(),
            );
            type Params = crate::vendor::command::DeclarativeParams<
                declarative_schema_list_type!($($ty => $shape,)*)
            >;

            fn params(&self) -> &Self::Params {
                &self.0
            }
        }

        impl<$life> ::bt_hci::WriteHci for $cmd<$life> {
            #[inline]
            fn size(&self) -> usize {
                <Self as ::bt_hci::cmd::Cmd>::params(self).size() + 3
            }

            fn write_hci<W: ::embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))?;
                <Self as ::bt_hci::cmd::Cmd>::params(self).write_hci(writer)
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer
                    .write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))
                    .await?;
                <Self as ::bt_hci::cmd::Cmd>::params(self)
                    .write_hci_async(writer)
                    .await
            }
        }

        impl<$life> ::bt_hci::cmd::SyncCmd for $cmd<$life> {
            type Return = $ret;
            type Handle = ();
            type ReturnBuf = [u8; $ret_len];

            fn param_handle(&self) {}

            fn return_handle(
                _data: &[u8],
            ) -> Result<Self::Handle, ::bt_hci::FromHciBytesError> {
                Ok(())
            }
        }
    };
    (
        $cmd:ident<$life:lifetime>($opcode:ident) {
            $($field:ident: $ty:ty => $shape:tt,)*
        }
        CommandStatus;
    ) => {
        $(declarative_schema_validate!($shape);)*

        #[allow(missing_docs)]
        pub struct $cmd<$life>(
            crate::vendor::command::DeclarativeParams<
                declarative_schema_list_type!($($ty => $shape,)*)
            >
        );

        impl<$life> $cmd<$life> {
            #[allow(clippy::too_many_arguments)]
            #[allow(missing_docs)]
            pub fn try_new(
                $($field: $ty),*
            ) -> Result<Self, crate::vendor::command::HciLengthError> {
                let params = crate::vendor::command::DeclarativeParams(
                    declarative_schema_list_value!($($field: $ty => $shape,)*)
                );
                let actual = crate::vendor::command::DeclarativeFieldList::size(&params.0);
                if actual > u8::MAX as usize {
                    return Err(crate::vendor::command::HciLengthError::new(
                        actual,
                        0,
                        u8::MAX as usize,
                    ));
                }
                Ok(Self(params))
            }
        }

        impl<$life> ::bt_hci::cmd::Cmd for $cmd<$life> {
            const OPCODE: ::bt_hci::cmd::Opcode = ::bt_hci::cmd::Opcode::new(
                ::bt_hci::cmd::OpcodeGroup::VENDOR_SPECIFIC,
                crate::vendor::opcode::$opcode.ocf(),
            );
            type Params = crate::vendor::command::DeclarativeParams<
                declarative_schema_list_type!($($ty => $shape,)*)
            >;

            fn params(&self) -> &Self::Params {
                &self.0
            }
        }

        impl<$life> ::bt_hci::WriteHci for $cmd<$life> {
            #[inline]
            fn size(&self) -> usize {
                <Self as ::bt_hci::cmd::Cmd>::params(self).size() + 3
            }

            fn write_hci<W: ::embedded_io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))?;
                <Self as ::bt_hci::cmd::Cmd>::params(self).write_hci(writer)
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer
                    .write_all(&<Self as ::bt_hci::cmd::Cmd>::header(self))
                    .await?;
                <Self as ::bt_hci::cmd::Cmd>::params(self)
                    .write_hci_async(writer)
                    .await
            }
        }

        impl<$life> ::bt_hci::cmd::AsyncCmd for $cmd<$life> {}
    };
}

/// Declares a vendor command's request fields, completion mechanism, and
/// command-complete payload.
///
/// Fixed-width fields use `field: Type => N`. The expansion requires
/// [`HciEncodeField<N>`](HciEncodeField) for request fields and
/// [`HciDecodeField<N>`](HciDecodeField) for return fields. The declared widths
/// are also available to the source-based compliance checker.
///
/// A command which completes through Command Status has no `Return`:
///
/// ```rust,ignore
/// vendor_cmd! {
///     GapPeripheralSecurityRequest(GAP_PERIPHERAL_SECURITY_REQUEST) {
///         Params = {
///             conn_handle: ConnectionHandle => 2,
///         };
///         Completion = CommandStatus;
///     }
/// }
/// ```
///
/// A Command Complete status is transport metadata and is not part of
/// `Return`. `Return = ()` therefore means no payload bytes after status:
///
/// ```rust,ignore
/// vendor_cmd! {
///     GapSetIoCapability(GAP_SET_IO_CAPABILITY) {
///         Params = {
///             io_capability: IoCapability => 1,
///         };
///         Completion = CommandComplete;
///         Return = ();
///     }
/// }
/// ```
///
/// A named return body generates the owned return structure and its exact
/// payload decoder:
///
/// ```rust,ignore
/// vendor_cmd! {
///     CmdGapInit(GAP_INIT) {
///         Params = {
///             role: Role => 1,
///             privacy_enabled: bool => 1,
///             dev_name_characteristic_len: u8 => 1,
///         };
///         Completion = CommandComplete;
///         Return = GapInit {
///             service_handle: AttributeHandle => 2,
///             dev_name_handle: AttributeHandle => 2,
///             appearance_handle: AttributeHandle => 2,
///         };
///     }
/// }
/// ```
///
/// Counted fields use a braced shape. Their count is derived from the supplied
/// slice and `try_new` rejects values beyond the declared maximum:
///
/// ```rust,ignore
/// vendor_cmd! {
///     GattReadMultiple(GATT_READ_MULTIPLE_VAR_CHAR_VALUE) {
///         Params<'a> = {
///             conn_handle: ConnectionHandle => 2,
///             handles: &'a [AttributeHandle] => {
///                 kind: counted_items,
///                 count: u8 => 1,
///                 item: AttributeHandle => 2,
///                 max_items: 126,
///             },
///         };
///         Completion = CommandStatus;
///     }
/// }
/// ```
///
/// Multiple variable fields are allowed. Each field checks its own declared
/// bound, and the generated `try_new` also rejects combinations whose actual
/// encoded request exceeds the HCI 255-byte parameter limit.
///
/// An uncounted byte tail uses `trailing_bytes` and must be the final field.
/// This is useful for controller responses whose event length supplies the
/// only length information:
///
/// ```rust,ignore
/// Return = ConfigData {
///     value: BoundedBytes<16> => {
///         kind: trailing_bytes,
///         min_len: 1,
///         max_len: 16,
///     },
/// };
/// ```
///
/// Tagged fields declare match patterns, discriminants, and payloads together.
/// The macro generates the match and encoder directly; the compliance parser
/// verifies the declared range and that payload names are bound by the
/// corresponding pattern. `try_new` also checks the resulting encoded size:
///
/// ```rust,ignore
/// uuid: &'a Uuid => {
///     kind: tagged,
///     tag: u8 => 1,
///     variants: {
///         Uuid::Uuid16(value) => {
///             tag: 0x01,
///             fields: { value: u16 => 2, },
///         },
///         Uuid::Uuid128(value) => {
///             tag: 0x02,
///             fields: { value: [u8; 16] => 16, },
///         },
///     },
///     min_len: 3,
///     max_len: 17,
/// },
/// ```
///
/// When the same tagged shape appears in more than one command or event,
/// `vendor_payload!` defines it once and generates both encoding and decoding:
///
/// ```rust,ignore
/// vendor_payload! {
///     pub enum Uuid {
///         Tag = u8 => 1;
///         Uuid16(value: u16 => 2) = 0x01;
///         Uuid128(value: [u8; 16] => 16) = 0x02;
///     }
/// }
///
/// // Inside Params or Return:
/// uuid: &'a Uuid => {
///     kind: payload,
///     min_len: 3,
///     max_len: 17,
/// },
/// ```
///
/// The compliance checker resolves the payload type and rejects bounds that
/// differ from the generated payload metadata. Command return structures stay
/// inline in each `vendor_cmd!` declaration so the complete command wire shape
/// is visible at its definition site.
///
/// A bitmap-selected list emits no separate count. Its number of records must
/// equal the number of selected bits, and bits outside `mask` are rejected:
///
/// ```rust,ignore
/// phy_params: &'a [ExtScanPhyParams] => {
///     kind: bitmap_items,
///     bitmap: scanning_phys,
///     mask: 0x05,
///     item: ExtScanPhyParams => 5,
///     max_items: 2,
/// },
/// ```
///
/// Counted returns use [`BoundedBytes`] or [`BoundedItems`] so decoding remains
/// allocation-free while the return value owns the initialized wire data.
macro_rules! vendor_cmd {
    (
        $cmd:ident($opcode:ident) {
            Params = ();
            Completion = CommandComplete;
            Return = ();
        }
    ) => {
        declarative_command! {
            $cmd($opcode) {}
            Return = ();
            ReturnLen = 0;
        }
        impl Default for $cmd {
            fn default() -> Self {
                Self::new()
            }
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params = ();
            Completion = CommandStatus;
        }
    ) => {
        declarative_command! {
            $cmd($opcode) {}
            CommandStatus;
        }
        impl Default for $cmd {
            fn default() -> Self {
                Self::new()
            }
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params = ();
            Completion = CommandComplete;
            Return = $ret:ident {
                $($ret_field:ident: $ret_ty:ty => $ret_shape:tt,)*
            };
        }
    ) => {
        declarative_return! {
            $ret {
                $($ret_field: $ret_ty => $ret_shape,)*
            }
        }
        declarative_command! {
            $cmd($opcode) {}
            Return = $ret;
            ReturnLen = 0 $(+ declarative_schema_max_len!($ret_shape))*;
        }
        impl Default for $cmd {
            fn default() -> Self {
                Self::new()
            }
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params = {
                $($field:ident: $ty:ty => $len:literal,)*
            };
            Completion = CommandComplete;
            Return = ();
        }
    ) => {
        declarative_command! {
            $cmd($opcode) {
                $($field: $ty => $len,)*
            }
            Return = ();
            ReturnLen = 0;
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params = {
                $($field:ident: $ty:ty => $len:literal,)*
            };
            Completion = CommandStatus;
        }
    ) => {
        declarative_command! {
            $cmd($opcode) {
                $($field: $ty => $len,)*
            }
            CommandStatus;
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params = {
                $($field:ident: $ty:ty => $len:literal,)*
            };
            Completion = CommandComplete;
            Return = $ret:ident {
                $($ret_field:ident: $ret_ty:ty => $ret_shape:tt,)*
            };
        }
    ) => {
        declarative_return! {
            $ret {
                $($ret_field: $ret_ty => $ret_shape,)*
            }
        }
        declarative_command! {
            $cmd($opcode) {
                $($field: $ty => $len,)*
            }
            Return = $ret;
            ReturnLen = 0 $(+ declarative_schema_max_len!($ret_shape))*;
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params<$life:lifetime> = {
                $($field:ident: $ty:ty => $shape:tt,)*
            };
            Completion = CommandComplete;
            Return = ();
        }
    ) => {
        declarative_variable_command! {
            $cmd<$life>($opcode) {
                $($field: $ty => $shape,)*
            }
            Return = ();
            ReturnLen = 0;
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params<$life:lifetime> = {
                $($field:ident: $ty:ty => $shape:tt,)*
            };
            Completion = CommandStatus;
        }
    ) => {
        declarative_variable_command! {
            $cmd<$life>($opcode) {
                $($field: $ty => $shape,)*
            }
            CommandStatus;
        }
    };
    (
        $cmd:ident($opcode:ident) {
            Params<$life:lifetime> = {
                $($field:ident: $ty:ty => $shape:tt,)*
            };
            Completion = CommandComplete;
            Return = $ret:ident {
                $($ret_field:ident: $ret_ty:ty => $ret_shape:tt,)*
            };
        }
    ) => {
        declarative_return! {
            $ret {
                $($ret_field: $ret_ty => $ret_shape,)*
            }
        }
        declarative_variable_command! {
            $cmd<$life>($opcode) {
                $($field: $ty => $shape,)*
            }
            Return = $ret;
            ReturnLen = 0 $(+ declarative_schema_max_len!($ret_shape))*;
        }
    };
}

/// Adds controller bounds only when a firmware cfg is active.
///
/// This keeps the legacy controller extension traits compatible with command
/// descriptors whose generated types are themselves cfg-gated.
macro_rules! cfg_command_bounds {
    (
        $trait:ident,
        $cfg:meta,
        $($bounds:tt)+
    ) => {
        #[cfg($cfg)]
        trait $trait: $($bounds)+ {}

        #[cfg($cfg)]
        impl<T> $trait for T
        where
            T: $($bounds)+,
        {}

        #[cfg(not($cfg))]
        trait $trait {}

        #[cfg(not($cfg))]
        impl<T> $trait for T {}
    };
}

#[cfg(test)]
mod tests {
    use super::{HciLengthError, TaggedField};

    vendor_payload! {
        pub(crate) enum ReusablePayloadFixture {
            Tag = u8 => 1;
            Short(value: u8 => 1) = 0x01;
            Long(value: u16 => 2) = 0x02;
        }
    }

    declarative_return! {
        ReusablePayloadReturn {
            payload: ReusablePayloadFixture => {
                kind: payload,
                min_len: 2,
                max_len: 3,
            },
        }
    }

    vendor_cmd! {
        AggregateLengthFixture(GAP_UPDATE_ADVERTISING_DATA) {
            Params<'a> = {
                prefix: u8 => 1,
                first: &'a [u8] => {
                    kind: counted_bytes,
                    count: u8 => 1,
                    max_len: 254,
                },
                second: &'a [u8] => {
                    kind: counted_bytes,
                    count: u8 => 1,
                    max_len: 254,
                },
            };
            Completion = CommandStatus;
        }
    }

    enum TaggedFixture {
        Empty,
        Pair { left: u8, right: u16 },
    }

    fn encode_fixture<'a>(
        value: &'a TaggedFixture,
    ) -> Result<TaggedField<&'a TaggedFixture, 4>, HciLengthError> {
        Ok(declarative_schema_field_value!(
            value: &'a TaggedFixture,
            {
                kind: tagged,
                tag: u8 => 1,
                variants: {
                    TaggedFixture::Empty => {
                        tag: 0x01,
                        fields: {},
                    },
                    TaggedFixture::Pair { left, right } => {
                        tag: 0x02,
                        fields: {
                            left: u8 => 1,
                            right: u16 => 2,
                        },
                    },
                },
                min_len: 1,
                max_len: 4,
            }
        ))
    }

    #[test]
    fn generated_tagged_codec_handles_unit_and_struct_variants() {
        let empty = encode_fixture(&TaggedFixture::Empty).unwrap();
        assert_eq!(&empty.bytes[..empty.len], [0x01]);

        let pair = encode_fixture(&TaggedFixture::Pair {
            left: 0xAA,
            right: 0x1234,
        })
        .unwrap();
        assert_eq!(&pair.bytes[..pair.len], [0x02, 0xAA, 0x34, 0x12]);
    }

    #[test]
    fn reusable_payload_decodes_inside_declarative_return() {
        use bt_hci::FromHciBytes;

        let short = ReusablePayloadReturn::from_hci_bytes_complete(&[0x01, 0xAA]).unwrap();
        assert_eq!(short.payload, ReusablePayloadFixture::Short(0xAA));

        let long = ReusablePayloadReturn::from_hci_bytes_complete(&[0x02, 0x34, 0x12]).unwrap();
        assert_eq!(long.payload, ReusablePayloadFixture::Long(0x1234));
    }

    #[test]
    fn variable_command_checks_the_actual_aggregate_hci_length() {
        let first = [0; 126];
        let valid = [0; 126];
        AggregateLengthFixture::try_new(0, &first, &valid).unwrap();

        let too_long = [0; 127];
        let error = match AggregateLengthFixture::try_new(0, &first, &too_long) {
            Ok(_) => panic!("aggregate HCI parameter overflow was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.actual(), 256);
        assert_eq!(error.maximum(), 255);
    }
}

pub mod gap;
pub mod gatt;
pub mod hal;
pub mod l2cap;
pub mod sys;
