//! Internal declarations for semantic values with canonical HCI wire forms.

mod decode;

pub use decode::{
    BoundedBytes, BoundedItems, HciDecodeCountedBytes, HciDecodeCountedItems,
    HciDecodeTrailingBytes,
};
pub(crate) use decode::{
    DecodeError, decode_counted_bytes, decode_counted_items, decode_fixed_field,
    decode_fixed_items, decode_prefixed_bytes, decode_trailing_bytes,
};

/// A value with an exact, canonical representation in an HCI request.
///
/// `N` is part of the trait so a declarative field whose schema says
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

/// A value decoded from an exact-width HCI field.
///
/// Implementations receive exactly `N` bytes and must apply the protocol's
/// validity rules rather than interpreting arbitrary Rust memory.
pub trait HciDecodeField<const N: usize>: Sized {
    /// Decode one exact-width field.
    fn from_hci_field(bytes: &[u8; N]) -> Result<Self, bt_hci::FromHciBytesError>;
}

/// An integer type that can prefix a counted declarative field.
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

/// Declare a fieldless HCI enum and its exact-width wire encoding.
///
/// The generated decoder rejects values that are not one of the declared
/// discriminants, so the enum remains a closed protocol value in both
/// directions.
macro_rules! hci_enum {
    (
        $(#[$enum_attr:meta])*
        $vis:vis enum $name:ident : $repr:ty => $len:literal {
            $(
                $(#[$variant_attr:meta])*
                $variant:ident = $value:expr,
            )+
        }
    ) => {
        $(#[$enum_attr])*
        #[repr($repr)]
        $vis enum $name {
            $(
                $(#[$variant_attr])*
                $variant = $value,
            )+
        }

        impl crate::vendor::command::HciEncodeField<$len> for $name {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                let value: $repr = match self {
                    $(Self::$variant => $value,)+
                };
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field(
                    &value,
                    writer,
                )
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                let value: $repr = match self {
                    $(Self::$variant => $value,)+
                };
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field_async(
                    &value,
                    writer,
                )
                .await
            }
        }

        impl crate::vendor::command::HciDecodeField<$len> for $name {
            fn from_hci_field(
                bytes: &[u8; $len],
            ) -> Result<Self, bt_hci::FromHciBytesError> {
                let value =
                    <$repr as crate::vendor::command::HciDecodeField<$len>>::from_hci_field(bytes)?;
                $(
                    if value == $value {
                        return Ok(Self::$variant);
                    }
                )+
                Err(bt_hci::FromHciBytesError::InvalidValue)
            }
        }
    };
}

macro_rules! hci_ranged_error {
    ($actual:expr, $minimum:expr, $maximum:expr) => {
        crate::vendor::command::HciValueError::new(
            $actual as u64,
            $minimum as u64,
            $maximum as u64,
            None,
        )
    };
    ($actual:expr, $minimum:expr, $maximum:expr, $sentinel:expr) => {
        crate::vendor::command::HciValueError::new(
            $actual as u64,
            $minimum as u64,
            $maximum as u64,
            Some($sentinel as u64),
        )
    };
}

/// Declare a bounded unsigned scalar and its exact-width HCI wire encoding.
///
/// The scalar can only be constructed or decoded when its value is within the
/// declared inclusive range or matches its optional named sentinel. This keeps
/// intrinsic protocol domains in their semantic types while command-specific
/// relationships remain in `vendor_cmd!`.
macro_rules! hci_ranged {
    (
        $(#[$struct_attr:meta])*
        $vis:vis struct $name:ident : $repr:ty => $len:literal {
            minimum: $minimum:expr,
            maximum: $maximum:expr,
            $(sentinel: $sentinel_name:ident = $sentinel:expr,)?
        }
    ) => {
        $(#[$struct_attr])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        $vis struct $name($repr);

        impl $name {
            /// Smallest accepted value.
            pub const MINIMUM: $repr = $minimum;

            /// Largest accepted value.
            pub const MAXIMUM: $repr = $maximum;

            $(
                /// Additional accepted value outside the inclusive range.
                pub const $sentinel_name: Self = Self($sentinel);
            )?

            /// Construct a value within the declared domain.
            pub const fn try_new(
                value: $repr,
            ) -> Result<Self, crate::vendor::command::HciValueError> {
                if (value >= Self::MINIMUM && value <= Self::MAXIMUM)
                    $(|| value == $sentinel)?
                {
                    Ok(Self(value))
                } else {
                    Err(hci_ranged_error!(
                        value,
                        Self::MINIMUM,
                        Self::MAXIMUM
                        $(, $sentinel)?
                    ))
                }
            }

            $(
                /// Whether this value is the declared out-of-range sentinel.
                pub const fn is_sentinel(self) -> bool {
                    self.0 == $sentinel
                }
            )?

            /// Return the underlying wire value.
            pub const fn value(self) -> $repr {
                self.0
            }
        }

        impl TryFrom<$repr> for $name {
            type Error = crate::vendor::command::HciValueError;

            fn try_from(value: $repr) -> Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for $repr {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl crate::vendor::command::HciEncodeField<$len> for $name {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field(
                    &self.0,
                    writer,
                )
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field_async(
                    &self.0,
                    writer,
                )
                .await
            }
        }

        impl crate::vendor::command::HciDecodeField<$len> for $name {
            fn from_hci_field(
                bytes: &[u8; $len],
            ) -> Result<Self, bt_hci::FromHciBytesError> {
                let value =
                    <$repr as crate::vendor::command::HciDecodeField<$len>>::from_hci_field(bytes)?;
                Self::try_new(value).map_err(|_| bt_hci::FromHciBytesError::InvalidValue)
            }
        }
    };
}

/// Declare HCI bitflags once while retaining `defmt`'s compact formatter.
///
/// The ordinary backend receives explicit standard derives because bitflags 2
/// no longer adds them automatically. The `defmt` backend wraps bitflags 1,
/// which already supplies those implementations.
macro_rules! hci_bitflags {
    (
        $(#[$($struct_attr:tt)*])*
        $vis:vis struct $name:ident : $repr:ty => $len:literal {
            $(
                $(#[$($flag_attr:tt)*])*
                const $flag:ident = $value:expr;
            )+
        }
    ) => {
        #[cfg(not(feature = "defmt"))]
        bitflags::bitflags! {
            $(#[$($struct_attr)*])*
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            $vis struct $name: $repr {
                $(
                    $(#[$($flag_attr)*])*
                    const $flag = $value;
                )+
            }
        }

        #[cfg(feature = "defmt")]
        defmt::bitflags! {
            $(#[$($struct_attr)*])*
            $vis struct $name: $repr {
                $(
                    $(#[$($flag_attr)*])*
                    const $flag = $value;
                )+
            }
        }

        impl crate::vendor::command::HciEncodeField<$len> for $name {
            fn write_hci_field<W: embedded_io::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field(
                    &self.bits(),
                    writer,
                )
            }

            async fn write_hci_field_async<W: embedded_io_async::Write>(
                &self,
                writer: W,
            ) -> Result<(), W::Error> {
                <$repr as crate::vendor::command::HciEncodeField<$len>>::write_hci_field_async(
                    &self.bits(),
                    writer,
                )
                .await
            }
        }

        impl crate::vendor::command::HciDecodeField<$len> for $name {
            fn from_hci_field(
                bytes: &[u8; $len],
            ) -> Result<Self, bt_hci::FromHciBytesError> {
                let bits =
                    <$repr as crate::vendor::command::HciDecodeField<$len>>::from_hci_field(bytes)?;
                Self::from_bits(bits).ok_or(bt_hci::FromHciBytesError::InvalidValue)
            }
        }

        impl crate::vendor::command::HciBitmap for $name {
            fn to_usize(self) -> usize {
                self.bits() as usize
            }
        }
    };
}
