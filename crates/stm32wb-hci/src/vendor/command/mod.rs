//! Declarative STM32WB vendor commands and their wire-format support.
//!
//! Each `vendor_cmd!` declaration generates a public command type and a
//! command-specific `*Params` type in its protocol module. Construct either
//! type with `new` (unconstrained fixed-size parameters) or `try_new`
//! (variable-size parameters or declarative constraints). Parameter types
//! retain their semantic fields behind same-named read-only accessors, then
//! encode those fields directly for the HCI wire. Execute the command through
//! [`bt_hci::cmd::SyncCmd::exec`] or [`bt_hci::cmd::AsyncCmd::exec`] according
//! to the command's declared completion mechanism.

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
pub(crate) const fn assert_hci_payload_length(length: usize) {
    ::core::assert!(length <= u8::MAX as usize);
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
                /// Bytes carried by the fixture.
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
        DirectVariableEncodingFixture(cgid = 0x1, cid = 0x11) {
            Params<'a> = {
                bitmap: u8 => 1,
                selected: &'a [u16] => {
                    kind: bitmap_items,
                    bitmap: bitmap,
                    mask: 0x03,
                    item: u16 => 2,
                    max_items: 2,
                },
                counted: &'a [u16] => {
                    kind: counted_items,
                    count: u8 => 1,
                    item: u16 => 2,
                    min_items: 1,
                    max_items: 2,
                },
                trailing: &'a [u8] => {
                    kind: trailing_bytes,
                    min_len: 1,
                    max_len: 3,
                },
            };
            Completion = CommandStatus;
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
    fn semantic_params_expose_fields_and_encode_variable_shapes_directly() {
        use bt_hci::WriteHci;

        let selected = [0x1122, 0x3344];
        let counted = [0x5566];
        let trailing = [0xAA, 0xBB];
        let command =
            DirectVariableEncodingFixture::try_new(0x03, &selected, &counted, &trailing).unwrap();
        let params = command.params();

        assert_eq!(*params.bitmap(), 0x03);
        assert_eq!(params.selected(), selected);
        assert_eq!(params.counted(), counted);
        assert_eq!(params.trailing(), trailing);
        assert_eq!(params.encoded_len(), 10);

        let mut encoded = [0; 10];
        let mut remaining = &mut encoded[..];
        params.write_hci(&mut remaining).unwrap();
        assert!(remaining.is_empty());
        assert_eq!(
            encoded,
            [0x03, 0x22, 0x11, 0x44, 0x33, 1, 0x66, 0x55, 0xAA, 0xBB],
        );
    }

    #[test]
    fn direct_variable_encodings_preserve_validation_diagnostics() {
        let selected = [0x1122];
        let counted = [0x3344];
        let trailing = [0xAA];

        let error = DirectVariableEncodingFixture::try_new(0x03, &selected, &counted, &trailing)
            .err()
            .unwrap();
        assert_eq!(error.actual(), 1);
        assert_eq!(error.minimum(), 2);
        assert_eq!(error.maximum(), 2);

        let error = DirectVariableEncodingFixture::try_new(0x04, &[], &counted, &trailing)
            .err()
            .unwrap();
        assert_eq!(error.actual(), 0x04);
        assert_eq!(error.maximum(), 0x03);

        let error = DirectVariableEncodingFixture::try_new(0, &[], &[], &trailing)
            .err()
            .unwrap();
        assert_eq!(error.actual(), 0);
        assert_eq!(error.minimum(), 1);

        let error = DirectVariableEncodingFixture::try_new(0, &[], &counted, &[])
            .err()
            .unwrap();
        assert_eq!(error.actual(), 0);
        assert_eq!(error.minimum(), 1);
        assert_eq!(error.maximum(), 3);
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
