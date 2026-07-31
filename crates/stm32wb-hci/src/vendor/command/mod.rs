//! Declarative STM32WB vendor commands and their wire-format support.
//!
//! Each `vendor_cmd!` declaration generates a public command type and a
//! command-specific `*Params` type in its protocol module. Construct either
//! type with `new` (unconstrained fixed-size parameters) or `try_new`
//! (variable-size parameters or declarative constraints), then execute the
//! command through
//! [`bt_hci::cmd::SyncCmd::exec`] or [`bt_hci::cmd::AsyncCmd::exec`] according
//! to the command's declared completion mechanism.

use bt_hci::WriteHci;

pub use crate::wire::{BoundedBytes, BoundedItems};
#[doc(hidden)]
pub use crate::wire::{
    HciCount, HciDecodeCountedBytes, HciDecodeCountedItems, HciDecodeTrailingBytes,
};
pub use crate::wire::{HciDecodeField, HciEncodeField};

/// Build the ten-bit vendor OCF from STM32's three-bit command-group ID and
/// seven-bit command ID.
const fn vendor_ocf(cgid: u16, cid: u16) -> u16 {
    ::core::assert!(cgid <= 0b111, "vendor command-group ID exceeds three bits");
    ::core::assert!(cid <= 0b111_1111, "vendor command ID exceeds seven bits");
    (cgid << 7) | cid
}

impl HciDecodeField<7> for crate::types::BdAddrType {
    fn from_hci_field(bytes: &[u8; 7]) -> Result<Self, bt_hci::FromHciBytesError> {
        let mut address = [0; 6];
        address.copy_from_slice(&bytes[1..]);
        crate::types::to_bd_addr_type(bytes[0], bt_hci::param::BdAddr(address))
            .map_err(|_| bt_hci::FromHciBytesError::InvalidValue)
    }
}

#[doc(hidden)]
pub(crate) struct DeclarativeField<T, const N: usize>(pub T);

#[doc(hidden)]
pub(crate) struct DeclarativeParams<T>(pub T);

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

/// A scalar value is outside the range, and optional sentinel, accepted by its
/// semantic HCI type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HciValueError {
    actual: u64,
    minimum: u64,
    maximum: u64,
    allowed_sentinel: Option<u64>,
}

impl HciValueError {
    pub(crate) const fn new(
        actual: u64,
        minimum: u64,
        maximum: u64,
        allowed_sentinel: Option<u64>,
    ) -> Self {
        Self {
            actual,
            minimum,
            maximum,
            allowed_sentinel,
        }
    }

    /// Rejected value.
    pub const fn actual(self) -> u64 {
        self.actual
    }

    /// Smallest accepted value.
    pub const fn minimum(self) -> u64 {
        self.minimum
    }

    /// Largest accepted value.
    pub const fn maximum(self) -> u64 {
        self.maximum
    }

    /// Additional accepted value outside the inclusive range, if any.
    pub const fn allowed_sentinel(self) -> Option<u64> {
        self.allowed_sentinel
    }
}

/// A relationship between command parameters does not satisfy its declarative
/// constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct HciConstraintError {
    command: &'static str,
    constraint: &'static str,
}

impl HciConstraintError {
    const fn new(command: &'static str, constraint: &'static str) -> Self {
        Self {
            command,
            constraint,
        }
    }

    /// Generated command type whose parameters were rejected.
    pub const fn command(self) -> &'static str {
        self.command
    }

    /// Declarative constraint that was not satisfied.
    pub const fn constraint(self) -> &'static str {
        self.constraint
    }
}

/// Parameter validation failure for a command that has both variable wire
/// bounds and relationships between fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HciValidationError {
    /// A variable field or the complete HCI request exceeded its wire bounds.
    Length(HciLengthError),
    /// A declared relationship between command fields was not satisfied.
    Constraint(HciConstraintError),
}

impl From<HciLengthError> for HciValidationError {
    fn from(error: HciLengthError) -> Self {
        Self::Length(error)
    }
}

impl From<HciConstraintError> for HciValidationError {
    fn from(error: HciConstraintError) -> Self {
        Self::Constraint(error)
    }
}

#[doc(hidden)]
pub(crate) struct TaggedField<T, const MAX_LEN: usize> {
    bytes: [u8; MAX_LEN],
    len: usize,
    _value: core::marker::PhantomData<T>,
}

impl<T, const MAX_LEN: usize> TaggedField<T, MAX_LEN> {
    pub(crate) fn try_new<const MIN_LEN: usize, Fields>(
        fields: Fields,
    ) -> Result<Self, HciLengthError>
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
#[allow(dead_code)]
pub(crate) trait HciBitmap: Copy {
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
#[allow(dead_code)]
pub(crate) struct BitmapItems<T, Item, const ITEM_LEN: usize, const MAX_ITEMS: usize> {
    value: T,
    _item: core::marker::PhantomData<Item>,
}

impl<T, Item, const ITEM_LEN: usize, const MAX_ITEMS: usize>
    BitmapItems<T, Item, ITEM_LEN, MAX_ITEMS>
where
    T: AsRef<[Item]>,
{
    #[allow(dead_code)]
    pub(crate) fn try_new<B: HciBitmap>(
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
pub(crate) struct CountedBytes<
    T,
    C,
    const COUNT_LEN: usize,
    const MIN_LEN: usize,
    const MAX_LEN: usize,
> {
    value: T,
    count: C,
}

impl<T, C, const COUNT_LEN: usize, const MIN_LEN: usize, const MAX_LEN: usize>
    CountedBytes<T, C, COUNT_LEN, MIN_LEN, MAX_LEN>
where
    T: AsRef<[u8]>,
    C: HciCount<COUNT_LEN>,
{
    pub(crate) fn try_new(value: T) -> Result<Self, HciLengthError> {
        let actual = value.as_ref().len();
        let maximum = core::cmp::min(MAX_LEN, C::MAX);
        let count = C::from_usize(actual).ok_or(HciLengthError::new(actual, MIN_LEN, maximum))?;
        if !(MIN_LEN..=maximum).contains(&actual) {
            return Err(HciLengthError::new(actual, MIN_LEN, maximum));
        }
        Ok(Self { value, count })
    }
}

#[doc(hidden)]
#[allow(dead_code)]
pub(crate) struct TrailingBytes<T, const MIN_LEN: usize, const MAX_LEN: usize> {
    value: T,
}

impl<T, const MIN_LEN: usize, const MAX_LEN: usize> TrailingBytes<T, MIN_LEN, MAX_LEN>
where
    T: AsRef<[u8]>,
{
    #[allow(dead_code)]
    pub(crate) fn try_new(value: T) -> Result<Self, HciLengthError> {
        let actual = value.as_ref().len();
        if !(MIN_LEN..=MAX_LEN).contains(&actual) {
            return Err(HciLengthError::new(actual, MIN_LEN, MAX_LEN));
        }
        Ok(Self { value })
    }
}

#[doc(hidden)]
pub(crate) struct CountedItems<
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MIN_ITEMS: usize,
    const MAX_ITEMS: usize,
> {
    value: T,
    count: C,
    _item: core::marker::PhantomData<Item>,
}

impl<
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MIN_ITEMS: usize,
    const MAX_ITEMS: usize,
> CountedItems<T, Item, C, COUNT_LEN, ITEM_LEN, MIN_ITEMS, MAX_ITEMS>
where
    T: AsRef<[Item]>,
    C: HciCount<COUNT_LEN>,
{
    pub(crate) fn try_new(value: T) -> Result<Self, HciLengthError> {
        let actual = value.as_ref().len();
        let maximum = core::cmp::min(MAX_ITEMS, C::MAX);
        let count = C::from_usize(actual).ok_or(HciLengthError::new(actual, MIN_ITEMS, maximum))?;
        if !(MIN_ITEMS..=maximum).contains(&actual) {
            return Err(HciLengthError::new(actual, MIN_ITEMS, maximum));
        }
        Ok(Self {
            value,
            count,
            _item: core::marker::PhantomData,
        })
    }
}

#[doc(hidden)]
pub fn decode_declarative_fixed_field<T, const N: usize>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeField<N>,
{
    crate::wire::decode_fixed_field(data, T::from_hci_field).map_err(map_declarative_decode_error)
}

fn map_declarative_decode_error(
    error: crate::wire::DecodeError<bt_hci::FromHciBytesError>,
) -> bt_hci::FromHciBytesError {
    match error {
        crate::wire::DecodeError::Field(error) => error,
        crate::wire::DecodeError::CountTooLarge { .. }
        | crate::wire::DecodeError::CountTooSmall { .. } => bt_hci::FromHciBytesError::InvalidValue,
        crate::wire::DecodeError::Truncated { .. }
        | crate::wire::DecodeError::LengthOutOfRange { .. }
        | crate::wire::DecodeError::SizeOverflow { .. } => bt_hci::FromHciBytesError::InvalidSize,
    }
}

#[doc(hidden)]
pub fn decode_declarative_counted_bytes<
    T,
    C,
    const COUNT_LEN: usize,
    const MIN_LEN: usize,
    const MAX_LEN: usize,
>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeCountedBytes<C, COUNT_LEN, MAX_LEN>,
    C: HciCount<COUNT_LEN> + HciDecodeField<COUNT_LEN>,
{
    crate::wire::decode_counted_bytes::<T, _, COUNT_LEN, MIN_LEN, MAX_LEN>(
        data,
        |bytes| C::from_hci_field(bytes).map(HciCount::to_usize),
        <T as HciDecodeCountedBytes<C, COUNT_LEN, MAX_LEN>>::from_counted_bytes,
    )
    .map_err(map_declarative_decode_error)
}

#[doc(hidden)]
pub fn decode_declarative_trailing_bytes<T, const MIN_LEN: usize, const MAX_LEN: usize>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeTrailingBytes<MIN_LEN, MAX_LEN>,
{
    crate::wire::decode_trailing_bytes::<T, bt_hci::FromHciBytesError, MIN_LEN, MAX_LEN>(
        data,
        <T as HciDecodeTrailingBytes<MIN_LEN, MAX_LEN>>::from_trailing_bytes,
    )
    .map_err(map_declarative_decode_error)
}

#[doc(hidden)]
pub fn decode_declarative_counted_items<
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MIN_ITEMS: usize,
    const MAX_ITEMS: usize,
>(
    data: &[u8],
) -> Result<(T, &[u8]), bt_hci::FromHciBytesError>
where
    T: HciDecodeCountedItems<Item, C, COUNT_LEN, ITEM_LEN, MAX_ITEMS>,
    Item: Copy + HciDecodeField<ITEM_LEN>,
    C: HciCount<COUNT_LEN> + HciDecodeField<COUNT_LEN>,
{
    crate::wire::decode_counted_items::<T, Item, C, _, COUNT_LEN, ITEM_LEN, MIN_ITEMS, MAX_ITEMS>(
        data,
        |bytes| C::from_hci_field(bytes).map(HciCount::to_usize),
        Item::from_hci_field,
    )
    .map_err(map_declarative_decode_error)
}

#[doc(hidden)]
pub(crate) const fn assert_hci_field_list_length(length: usize) {
    ::core::assert!(length <= u8::MAX as usize);
}

#[doc(hidden)]
pub(crate) trait DeclarativeFieldList {
    fn size(&self) -> usize;

    fn write<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error>;

    async fn write_async<W: embedded_io_async::Write>(&self, writer: W) -> Result<(), W::Error>;
}

#[doc(hidden)]
pub(crate) trait DeclarativeEncodedField {
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

impl<T, C, const COUNT_LEN: usize, const MIN_LEN: usize, const MAX_LEN: usize>
    DeclarativeEncodedField for CountedBytes<T, C, COUNT_LEN, MIN_LEN, MAX_LEN>
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

impl<
    T,
    Item,
    C,
    const COUNT_LEN: usize,
    const ITEM_LEN: usize,
    const MIN_ITEMS: usize,
    const MAX_ITEMS: usize,
> DeclarativeEncodedField for CountedItems<T, Item, C, COUNT_LEN, ITEM_LEN, MIN_ITEMS, MAX_ITEMS>
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

#[cfg(test)]
mod tests {
    use super::HciDecodeField;

    stm32wb_hci_macros::wire_type! {
        adapters: [command];
        closed
        #[derive(Debug, Eq, PartialEq)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        enum SemanticEnumFixture: u8 => 1 {
            First = 0x01,
            Third = 0x03,
        }
    }

    stm32wb_hci_macros::wire_type! {
        adapters: [command];
        bitflags
        struct SemanticFlagsFixture: u8 => 1 {
            const FIRST = 0x01;
            const THIRD = 0x04;
        }
    }

    stm32wb_hci_macros::vendor_cmd! {
        AggregateLengthFixture(cgid = 0x1, cid = 0x0E) {
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

    stm32wb_hci_macros::vendor_cmd! {
        MinimumLengthFixture(cgid = 0x1, cid = 0x0F) {
            Params<'a> = {
                data: &'a [u8] => {
                    kind: counted_bytes,
                    count: u8 => 1,
                    min_len: 3,
                    max_len: 4,
                },
            };
            Completion = CommandComplete;
            Return = ();
        }
    }

    stm32wb_hci_macros::vendor_cmd! {
        FixedConstraintFixture(cgid = 0x1, cid = 0x10) {
            Params = {
                mode: u8 => 1,
                minimum: u8 => 1,
                maximum: u8 => 1,
            };
            Constraints = {
                range(mode, 1, 2);
                ordered(minimum, maximum);
            };
            Completion = CommandComplete;
            Return = ();
        }
    }

    #[test]
    fn semantic_wire_declarations_reject_unknown_values_and_bits() {
        assert_eq!(
            SemanticEnumFixture::from_hci_field(&[0x03]),
            Ok(SemanticEnumFixture::Third)
        );
        assert!(SemanticEnumFixture::from_hci_field(&[0x02]).is_err());

        assert_eq!(
            SemanticFlagsFixture::from_hci_field(&[0x05]),
            Ok(SemanticFlagsFixture::FIRST | SemanticFlagsFixture::THIRD)
        );
        assert!(SemanticFlagsFixture::from_hci_field(&[0x02]).is_err());
        assert!(SemanticFlagsFixture::from_bits(0x02).is_none());
        assert_eq!(
            SemanticFlagsFixture::from_bits_truncate(0x07),
            SemanticFlagsFixture::FIRST | SemanticFlagsFixture::THIRD
        );
        assert_eq!(!(SemanticFlagsFixture::FIRST), SemanticFlagsFixture::THIRD);
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

    #[test]
    fn counted_bytes_enforce_the_declared_minimum_length() {
        let error = match MinimumLengthFixture::try_new(&[0; 2]) {
            Ok(_) => panic!("short counted byte field was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.actual(), 2);
        assert_eq!(error.minimum(), 3);
        assert_eq!(error.maximum(), 4);
        assert!(MinimumLengthFixture::try_new(&[0; 3]).is_ok());
    }

    #[test]
    fn fixed_constraints_preserve_diagnostics_and_order() {
        assert_eq!(
            FixedConstraintFixture::try_new(0, 2, 1)
                .err()
                .unwrap()
                .constraint(),
            "1 <= mode <= 2",
        );
        assert_eq!(
            FixedConstraintFixture::try_new(1, 2, 1)
                .err()
                .unwrap()
                .constraint(),
            "minimum <= maximum",
        );
        assert!(FixedConstraintFixture::try_new(1, 1, 2).is_ok());
    }

    #[test]
    fn command_ids_are_derived_from_the_declaration() {
        use bt_hci::cmd::Cmd;

        assert_eq!(AggregateLengthFixture::CGID, 0x1);
        assert_eq!(AggregateLengthFixture::CID, 0x0E);
        assert_eq!(AggregateLengthFixture::OCF, 0x008E);
        assert_eq!(<AggregateLengthFixture<'_> as Cmd>::OPCODE.to_raw(), 0xFC8E);
    }
}

pub mod gap;
pub mod gatt;
pub mod hal;
pub mod l2cap;
#[cfg(since_fw_1_23_0)]
pub mod sys;
