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
//! variable-length data in return parameters that must be `Copy`, a borrowed
//! list of structures in events, and the vendor event code identifying each
//! ST event.

use core::fmt;
use core::marker::PhantomData;
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

    /// For [`AttBearer`], the last enhanced ATT bearer it accepts; 0 for any
    /// other type. Declarations check it against the catalog's bearer
    /// members.
    #[doc(hidden)]
    const LAST_ENHANCED_BEARER: u16 = 0;

    /// Every value the type encodes, for a type standing for some values of
    /// an integer member, such as those [`wire_values!`] declares; `None`
    /// for any other type. Declarations check each value is one the catalog
    /// documents for the member.
    #[doc(hidden)]
    const VALUES: Option<&'static [i64]> = None;

    /// Every bit the type may set, for a type standing for some flags of an
    /// integer member, such as those [`wire_flags!`] declares; `None` for
    /// any other type. Declarations check each bit is one the catalog
    /// documents for the member.
    #[doc(hidden)]
    const BITS: Option<u64> = None;

    /// The inclusive ranges of values the type encodes, for a type standing
    /// for a range of values of an integer member, such as those
    /// [`wire_duration!`] declares; `None` for any other type. Declarations
    /// check each value is one the catalog documents for the member.
    #[doc(hidden)]
    const RANGES: Option<&'static [(i64, i64)]> = None;

    /// The duration of one unit in microseconds, for a type counting time,
    /// such as those [`wire_duration!`] declares; `None` for any other type.
    /// Declarations check it is the unit the catalog documents for the
    /// member.
    #[doc(hidden)]
    const UNIT_US: Option<u32> = None;
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
    BdAddr => 6,
    ConnHandle => 2,
}

/// A `bool` parameter is sent as 0 or 1, so the member must document both.
impl HciWireType for bool {
    const WIDTH: usize = 1;
    const VALUES: Option<&'static [i64]> = Some(&[0, 1]);
}

/// Whether `first..=last` lies within the union of the `documented` ranges.
const fn covered(first: i64, last: i64, documented: &[(i64, i64)]) -> bool {
    let mut next = first;
    loop {
        let mut end = None;
        let mut item = 0;
        while item < documented.len() {
            let (from, to) = documented[item];
            if from <= next && next <= to {
                end = match end {
                    Some(end) if end >= to => Some(end),
                    _ => Some(to),
                };
            }
            item += 1;
        }
        match end {
            None => return false,
            Some(end) if end >= last => return true,
            Some(end) => next = end + 1,
        }
    }
}

/// Whether every value `T` encodes is one the catalog documents for a
/// member listing values, each documented item being an inclusive range. A
/// flags type, whose combinations the list does not name, never is; a type
/// standing for no particular values always is.
#[doc(hidden)]
pub const fn values_documented<T: HciWireType>(documented: &[(i64, i64)]) -> bool {
    if T::BITS.is_some() {
        return false;
    }
    if let Some(values) = T::VALUES {
        let mut index = 0;
        while index < values.len() {
            if !covered(values[index], values[index], documented) {
                return false;
            }
            index += 1;
        }
    }
    if let Some(ranges) = T::RANGES {
        let mut index = 0;
        while index < ranges.len() {
            if !covered(ranges[index].0, ranges[index].1, documented) {
                return false;
            }
            index += 1;
        }
    }
    true
}

/// Whether `T` counts in the unit the catalog documents for a member listing
/// values, in microseconds: a type counting time only for a time member,
/// and only in its unit.
#[doc(hidden)]
pub const fn unit_documented<T: HciWireType>(documented: Option<u32>) -> bool {
    match (T::UNIT_US, documented) {
        (Some(unit), Some(documented)) => unit == documented,
        (None, None) => true,
        _ => false,
    }
}

/// Whether every bit `T` may set is one the catalog documents for a member
/// listing flags, `documented` being their union. A value type is checked
/// as the bits of each of its values; a range of values never is; a type
/// standing for no particular values or flags always is.
#[doc(hidden)]
pub const fn flags_documented<T: HciWireType>(documented: u64) -> bool {
    if T::RANGES.is_some() {
        return false;
    }
    if let Some(bits) = T::BITS {
        return bits & !documented == 0;
    }
    let Some(values) = T::VALUES else {
        return true;
    };
    let mut index = 0;
    while index < values.len() {
        if values[index] < 0 || values[index] as u64 & !documented != 0 {
            return false;
        }
        index += 1;
    }
    true
}

/// Whether `T` stands for no particular values or flags, as a member the
/// catalog documents no checkable values for requires.
#[doc(hidden)]
pub const fn is_opaque<T: HciWireType>() -> bool {
    T::VALUES.is_none() && T::BITS.is_none() && T::RANGES.is_none() && T::UNIT_US.is_none()
}

/// Declare an enum standing for some documented values of an integer member,
/// encoded as its `repr` integer. Every declaration using it for a member
/// checks at compile time that each of its values is one the catalog
/// documents for that member, on every target.
///
/// ```ignore
/// wire_values! {
///     /// Whether the scan requests advertising reports.
///     pub enum ScanType: u8 {
///         /// Listen only.
///         Passive = 0x00,
///         /// Send scan requests.
///         Active = 0x01,
///     }
/// }
/// ```
#[macro_export]
macro_rules! wire_values {
    (
        $(#[$attr:meta])*
        $vis:vis enum $name:ident: $repr:ident {
            $(
                $(#[$variant_attr:meta])*
                $variant:ident = $value:expr
            ),+ $(,)?
        }
    ) => {
        $(#[$attr])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        #[repr($repr)]
        $vis enum $name {
            $(
                $(#[$variant_attr])*
                $variant = $value,
            )+
        }

        impl $crate::wire::HciWireType for $name {
            const WIDTH: usize = ::core::mem::size_of::<$repr>();
            const VALUES: ::core::option::Option<&'static [i64]> =
                ::core::option::Option::Some(&[$($name::$variant as $repr as i64),+]);
        }

        impl ::core::convert::From<$name> for $repr {
            fn from(value: $name) -> $repr {
                value as $repr
            }
        }

        impl ::core::convert::TryFrom<$repr> for $name {
            type Error = ::bt_hci::FromHciBytesError;

            fn try_from(value: $repr) -> ::core::result::Result<Self, Self::Error> {
                $(
                    if value == $name::$variant as $repr {
                        return ::core::result::Result::Ok($name::$variant);
                    }
                )+
                ::core::result::Result::Err(::bt_hci::FromHciBytesError::InvalidValue)
            }
        }

        impl ::bt_hci::WriteHci for $name {
            fn size(&self) -> usize {
                ::core::mem::size_of::<$repr>()
            }

            fn write_hci<W: ::embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
                writer.write_all(&(*self as $repr).to_le_bytes())
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&(*self as $repr).to_le_bytes()).await
            }
        }

        impl<'de> ::bt_hci::FromHciBytes<'de> for $name {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> ::core::result::Result<(Self, &'de [u8]), ::bt_hci::FromHciBytesError> {
                let (raw, rest) = <$repr as ::bt_hci::FromHciBytes<'de>>::from_hci_bytes(data)?;
                ::core::result::Result::Ok((<$name as ::core::convert::TryFrom<$repr>>::try_from(raw)?, rest))
            }
        }
    };
}

/// Declare a set of flags standing for some documented bits of an integer
/// member, encoded as its `repr` integer. Every declaration using it for a
/// member checks at compile time that each of its flags is a bit the catalog
/// documents for that member, on every target.
///
/// ```ignore
/// wire_flags! {
///     /// The advertising channels to use.
///     pub struct AdvChannelMap: u8 {
///         /// Channel 37.
///         const CHANNEL_37 = 0x01;
///         /// Channel 38.
///         const CHANNEL_38 = 0x02;
///     }
/// }
/// ```
#[macro_export]
macro_rules! wire_flags {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident: $repr:ident {
            $(
                $(#[$flag_attr:meta])*
                const $flag:ident = $value:expr;
            )+
        }
    ) => {
        $(#[$attr])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        $vis struct $name($repr);

        impl $name {
            $(
                $(#[$flag_attr])*
                pub const $flag: Self = Self($value);
            )+

            /// No flags.
            pub const fn empty() -> Self {
                Self(0)
            }

            /// Every flag.
            pub const fn all() -> Self {
                Self(0 $(| $value)+)
            }

            /// The encoded bits.
            pub const fn bits(self) -> $repr {
                self.0
            }

            /// The flags `bits` encodes, or `None` if it sets any other bit.
            pub const fn from_bits(bits: $repr) -> ::core::option::Option<Self> {
                if bits & !Self::all().0 == 0 {
                    ::core::option::Option::Some(Self(bits))
                } else {
                    ::core::option::Option::None
                }
            }

            /// Whether no flag is set.
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }

            /// Whether every flag of `other` is set.
            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            /// The flags set in either.
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            /// The flags set in both.
            pub const fn intersection(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }
        }

        impl ::core::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, other: Self) -> Self {
                self.union(other)
            }
        }

        impl ::core::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, other: Self) {
                *self = self.union(other);
            }
        }

        impl ::core::ops::BitAnd for $name {
            type Output = Self;

            fn bitand(self, other: Self) -> Self {
                self.intersection(other)
            }
        }

        impl ::core::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, other: Self) {
                *self = self.intersection(other);
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(::core::stringify!($name))?;
                f.write_str("(")?;
                let mut first = true;
                $(
                    if self.contains(Self::$flag) {
                        if !first {
                            f.write_str(" | ")?;
                        }
                        f.write_str(::core::stringify!($flag))?;
                        first = false;
                    }
                )+
                if first {
                    f.write_str("empty")?;
                }
                f.write_str(")")
            }
        }

        impl $crate::wire::HciWireType for $name {
            const WIDTH: usize = ::core::mem::size_of::<$repr>();
            const BITS: ::core::option::Option<u64> =
                ::core::option::Option::Some($name::all().0 as u64);
        }

        impl ::core::convert::From<$name> for $repr {
            fn from(value: $name) -> $repr {
                value.0
            }
        }

        impl ::core::convert::TryFrom<$repr> for $name {
            type Error = ::bt_hci::FromHciBytesError;

            fn try_from(bits: $repr) -> ::core::result::Result<Self, Self::Error> {
                $name::from_bits(bits).ok_or(::bt_hci::FromHciBytesError::InvalidValue)
            }
        }

        impl ::bt_hci::WriteHci for $name {
            fn size(&self) -> usize {
                ::core::mem::size_of::<$repr>()
            }

            fn write_hci<W: ::embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
                writer.write_all(&self.0.to_le_bytes())
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&self.0.to_le_bytes()).await
            }
        }

        impl<'de> ::bt_hci::FromHciBytes<'de> for $name {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> ::core::result::Result<(Self, &'de [u8]), ::bt_hci::FromHciBytesError> {
                let (raw, rest) = <$repr as ::bt_hci::FromHciBytes<'de>>::from_hci_bytes(data)?;
                ::core::result::Result::Ok((<$name as ::core::convert::TryFrom<$repr>>::try_from(raw)?, rest))
            }
        }
    };
}

/// Declare a duration counted in units of an integer member, standing for
/// a documented range of its values and some special ones, encoded as its
/// `repr` integer. Every declaration using it for a member checks at compile
/// time that the catalog documents the member in the same unit, and each
/// value of the range and each special value, on every target.
///
/// ```ignore
/// wire_duration! {
///     /// How long to scan.
///     pub struct ScanDuration: u16 {
///         unit_us = 10_000;
///         units = 0x0001..=0xFFFF;
///         /// Scan until scanning is disabled.
///         const CONTINUOUS = 0x0000;
///     }
/// }
/// ```
#[macro_export]
macro_rules! wire_duration {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident: $repr:ident {
            unit_us = $unit:expr;
            units = $min:literal..=$max:literal;
            $(
                $(#[$special_attr:meta])*
                const $special:ident = $value:expr;
            )*
        }
    ) => {
        $(#[$attr])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(feature = "defmt", derive(defmt::Format))]
        $vis struct $name($repr);

        #[allow(unused_comparisons, clippy::absurd_extreme_comparisons)]
        const _: () = {
            ::core::assert!($min <= $max, "the range is empty");
            $(
                ::core::assert!(
                    !($min <= $value && $value <= $max),
                    "a special value is in the range"
                );
            )*
        };

        impl $name {
            /// The duration of one unit, in microseconds.
            pub const UNIT_US: u32 = $unit;
            /// The shortest duration.
            pub const MIN: Self = Self($min);
            /// The longest duration.
            pub const MAX: Self = Self($max);
            $(
                $(#[$special_attr])*
                pub const $special: Self = Self($value);
            )*

            /// `units` units, or `None` outside [`MIN`](Self::MIN)..=[`MAX`](Self::MAX).
            #[allow(unused_comparisons, clippy::absurd_extreme_comparisons)]
            pub const fn from_units(units: $repr) -> ::core::option::Option<Self> {
                if $min <= units && units <= $max {
                    ::core::option::Option::Some(Self(units))
                } else {
                    ::core::option::Option::None
                }
            }

            /// `micros` microseconds, or `None` unless it is a whole number of
            /// units within [`MIN`](Self::MIN)..=[`MAX`](Self::MAX).
            pub const fn from_micros(micros: u64) -> ::core::option::Option<Self> {
                let unit = Self::UNIT_US as u64;
                if micros % unit != 0 || micros / unit > $max as u64 {
                    return ::core::option::Option::None;
                }
                Self::from_units((micros / unit) as $repr)
            }

            /// `millis` milliseconds, or `None` unless it is a whole number of
            /// units within [`MIN`](Self::MIN)..=[`MAX`](Self::MAX).
            pub const fn from_millis(millis: u64) -> ::core::option::Option<Self> {
                match millis.checked_mul(1000) {
                    ::core::option::Option::Some(micros) => Self::from_micros(micros),
                    ::core::option::Option::None => ::core::option::Option::None,
                }
            }

            /// The encoded number of units.
            pub const fn units(self) -> $repr {
                self.0
            }

            /// The duration in microseconds, or `None` for a special value.
            pub const fn as_micros(self) -> ::core::option::Option<u64> {
                match Self::from_units(self.0) {
                    ::core::option::Option::Some(_) => {
                        ::core::option::Option::Some(self.0 as u64 * Self::UNIT_US as u64)
                    }
                    ::core::option::Option::None => ::core::option::Option::None,
                }
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                $(
                    if *self == Self::$special {
                        return f.write_str(::core::concat!(
                            ::core::stringify!($name),
                            "(",
                            ::core::stringify!($special),
                            ")"
                        ));
                    }
                )*
                match self.as_micros() {
                    ::core::option::Option::Some(micros) => {
                        ::core::write!(f, "{}({} µs)", ::core::stringify!($name), micros)
                    }
                    ::core::option::Option::None => {
                        ::core::write!(f, "{}({:#x})", ::core::stringify!($name), self.0)
                    }
                }
            }
        }

        impl $crate::wire::HciWireType for $name {
            const WIDTH: usize = ::core::mem::size_of::<$repr>();
            const RANGES: ::core::option::Option<&'static [(i64, i64)]> =
                ::core::option::Option::Some(&[
                    ($min as i64, $max as i64),
                    $(($name::$special.0 as i64, $name::$special.0 as i64),)*
                ]);
            const UNIT_US: ::core::option::Option<u32> = ::core::option::Option::Some($unit);
        }

        impl ::core::convert::From<$name> for $repr {
            fn from(value: $name) -> $repr {
                value.0
            }
        }

        impl ::core::convert::TryFrom<$repr> for $name {
            type Error = ::bt_hci::FromHciBytesError;

            fn try_from(units: $repr) -> ::core::result::Result<Self, Self::Error> {
                $(
                    if units == $name::$special.0 {
                        return ::core::result::Result::Ok($name::$special);
                    }
                )*
                $name::from_units(units).ok_or(::bt_hci::FromHciBytesError::InvalidValue)
            }
        }

        impl ::bt_hci::WriteHci for $name {
            fn size(&self) -> usize {
                ::core::mem::size_of::<$repr>()
            }

            fn write_hci<W: ::embedded_io::Write>(&self, mut writer: W) -> Result<(), W::Error> {
                writer.write_all(&self.0.to_le_bytes())
            }

            async fn write_hci_async<W: ::embedded_io_async::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), W::Error> {
                writer.write_all(&self.0.to_le_bytes()).await
            }
        }

        impl<'de> ::bt_hci::FromHciBytes<'de> for $name {
            fn from_hci_bytes(
                data: &'de [u8],
            ) -> ::core::result::Result<(Self, &'de [u8]), ::bt_hci::FromHciBytesError> {
                let (raw, rest) = <$repr as ::bt_hci::FromHciBytes<'de>>::from_hci_bytes(data)?;
                ::core::result::Result::Ok((<$name as ::core::convert::TryFrom<$repr>>::try_from(raw)?, rest))
            }
        }
    };
}

impl<T: HciWireType, const N: usize> HciWireType for [T; N] {
    const WIDTH: usize = T::WIDTH * N;
}

impl<T: HciWireType + ?Sized> HciWireType for &T {
    const WIDTH: usize = T::WIDTH;
    const LAST_ENHANCED_BEARER: u16 = T::LAST_ENHANCED_BEARER;
    const VALUES: Option<&'static [i64]> = T::VALUES;
    const BITS: Option<u64> = T::BITS;
    const RANGES: Option<&'static [(i64, i64)]> = T::RANGES;
    const UNIT_US: Option<u32> = T::UNIT_US;
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

/// The ATT bearer a GATT or ATT procedure runs on: the unenhanced bearer of a
/// connection, named by its handle, or an enhanced ATT bearer, an L2CAP
/// connection-oriented channel named by its index.
///
/// It is encoded in 16 bits, as the connection handle (0x0000..=0x0EFF) or
/// as 0xEA00 plus the channel index, up to [`LAST_ENHANCED`](Self::LAST_ENHANCED).
/// The catalog records which parameters take a bearer rather than a
/// connection handle, and declarations check that exactly those are
/// `AttBearer`s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AttBearer(u16);

impl AttBearer {
    stm32wb_hci_macros::att_bearer_range!();

    /// The first enhanced ATT bearer, channel index 0.
    const FIRST_ENHANCED: u16 = 0xEA00;

    /// The last connection handle.
    const LAST_CONNECTION: u16 = 0x0EFF;

    /// The unenhanced bearer of the connection `handle`.
    pub fn unenhanced(handle: ConnHandle) -> Self {
        Self(handle.raw())
    }

    /// The enhanced ATT bearer on the L2CAP channel `channel_index`, if the
    /// selected release has that channel.
    pub const fn enhanced(channel_index: u8) -> Option<Self> {
        Self::from_raw(Self::FIRST_ENHANCED + channel_index as u16)
    }

    /// The bearer encoded as `raw`, if it names one.
    pub const fn from_raw(raw: u16) -> Option<Self> {
        if raw <= Self::LAST_CONNECTION
            || (Self::FIRST_ENHANCED <= raw && raw <= Self::LAST_ENHANCED)
        {
            Some(Self(raw))
        } else {
            None
        }
    }

    /// The encoded bearer.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// The connection whose unenhanced bearer this is.
    pub fn connection(self) -> Option<ConnHandle> {
        (self.0 <= Self::LAST_CONNECTION).then(|| ConnHandle::new(self.0))
    }

    /// The L2CAP channel index of an enhanced bearer.
    pub const fn channel_index(self) -> Option<u8> {
        if self.0 >= Self::FIRST_ENHANCED {
            Some((self.0 - Self::FIRST_ENHANCED) as u8)
        } else {
            None
        }
    }
}

impl From<ConnHandle> for AttBearer {
    fn from(handle: ConnHandle) -> Self {
        Self::unenhanced(handle)
    }
}

impl HciWireType for AttBearer {
    const WIDTH: usize = 2;
    const LAST_ENHANCED_BEARER: u16 = Self::LAST_ENHANCED;
}

impl WriteHci for AttBearer {
    fn size(&self) -> usize {
        2
    }

    fn write_hci<W: embedded_io::Write>(&self, writer: W) -> Result<(), W::Error> {
        self.0.write_hci(writer)
    }

    async fn write_hci_async<W: embedded_io_async::Write>(
        &self,
        writer: W,
    ) -> Result<(), W::Error> {
        self.0.write_hci_async(writer).await
    }
}

/// A value naming neither a connection nor an enhanced bearer of the
/// selected release is invalid.
impl<'de> FromHciBytes<'de> for AttBearer {
    fn from_hci_bytes(data: &'de [u8]) -> Result<(Self, &'de [u8]), FromHciBytesError> {
        let (raw, rest) = u16::from_hci_bytes(data)?;
        let bearer = Self::from_raw(raw).ok_or(FromHciBytesError::InvalidValue)?;
        Ok((bearer, rest))
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

/// An optional parameter supplied after an omitted one, which the command
/// cannot send: its parameters stop at the first one omitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct OmittedBefore {
    /// The Rust name of the supplied field.
    pub field: &'static str,
    /// The Rust name of the omitted field before it.
    pub omitted: &'static str,
}

impl fmt::Display for OmittedBefore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} cannot be sent without {}, which comes before it",
            self.field, self.omitted
        )
    }
}

impl core::error::Error for OmittedBefore {}

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
    same_name(T::C_NAME, c_name)
}

/// Whether `T` is the type declared for the catalog's vendor event `c_name`.
#[doc(hidden)]
pub const fn declares_event<T: VendorEvent<'static>>(c_name: &str) -> bool {
    same_name(T::C_NAME, c_name)
}

/// Whether `T` is the type declared for the catalog's system event `c_name`.
#[doc(hidden)]
pub const fn declares_system_event<T: SystemEvent<'static>>(c_name: &str) -> bool {
    same_name(T::C_NAME, c_name)
}

const fn same_name(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
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

/// A variable-length list of structures borrowed from an event, decoding
/// each element as it is read. Every element is checked when the event is
/// decoded, so reading one cannot fail.
pub struct Elements<'a, T> {
    bytes: &'a [u8],
    element: PhantomData<fn() -> T>,
}

impl<'a, T: HciWireType + FromHciBytes<'a>> Elements<'a, T> {
    /// Split `count` elements off the front of `data`, checking that each
    /// decodes.
    #[doc(hidden)]
    pub fn decode(data: &'a [u8], count: usize) -> Result<(Self, &'a [u8]), FromHciBytesError> {
        let (bytes, rest) = count
            .checked_mul(T::WIDTH)
            .and_then(|len| data.split_at_checked(len))
            .ok_or(FromHciBytesError::InvalidSize)?;
        for element in bytes.chunks_exact(T::WIDTH) {
            T::from_hci_bytes_complete(element)?;
        }
        Ok((
            Self {
                bytes,
                element: PhantomData,
            },
            rest,
        ))
    }

    /// The number of elements.
    pub fn len(&self) -> usize {
        self.bytes.len() / T::WIDTH
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The element at `index`, if the list is that long.
    pub fn get(&self, index: usize) -> Option<T> {
        let start = index.checked_mul(T::WIDTH)?;
        let element = self.bytes.get(start..start.checked_add(T::WIDTH)?)?;
        Some(Self::element(element))
    }

    /// The elements in order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = T> + use<'a, T> {
        self.bytes.chunks_exact(T::WIDTH).map(Self::element)
    }

    /// The encoded elements, as the event carries them.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }

    fn element(bytes: &'a [u8]) -> T {
        match T::from_hci_bytes_complete(bytes) {
            Ok(element) => element,
            Err(_) => unreachable!("every element was decoded with the event"),
        }
    }
}

impl<T> Clone for Elements<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Elements<'_, T> {}

impl<T> PartialEq for Elements<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl<T> Eq for Elements<'_, T> {}

impl<'a, T: HciWireType + FromHciBytes<'a> + fmt::Debug> fmt::Debug for Elements<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(feature = "defmt")]
impl<'a, T: HciWireType + FromHciBytes<'a> + defmt::Format> defmt::Format for Elements<'a, T> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "[");
        for (index, element) in self.iter().enumerate() {
            if index > 0 {
                defmt::write!(f, ", ");
            }
            defmt::write!(f, "{}", element);
        }
        defmt::write!(f, "]");
    }
}

/// A bt-hci command that completes with Command Complete, as the catalog
/// lists a standard command to.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "the catalog completes `{Self}` with Command Complete, but it is not a bt-hci SyncCmd"
)]
pub trait CompletesWithCommandComplete: bt_hci::cmd::SyncCmd {}

impl<T: bt_hci::cmd::SyncCmd> CompletesWithCommandComplete for T {}

/// A bt-hci command that completes with Command Status, as the catalog lists
/// a standard command to.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "the catalog completes `{Self}` with Command Status, but it is not a bt-hci AsyncCmd"
)]
pub trait CompletesWithCommandStatus: bt_hci::cmd::AsyncCmd {}

impl<T: bt_hci::cmd::AsyncCmd> CompletesWithCommandStatus for T {}

/// A bt-hci value whose encoding is its packed layout, so its size is its
/// wire width.
#[doc(hidden)]
#[diagnostic::on_unimplemented(
    message = "the catalog encodes `{Self}` in a fixed width, but it is not a bt-hci FixedSizeValue"
)]
pub trait FixedWidth: bt_hci::FixedSizeValue {}

impl<T: bt_hci::FixedSizeValue> FixedWidth for T {}

#[doc(hidden)]
pub const fn completes_with_command_complete<T: CompletesWithCommandComplete>() {}

#[doc(hidden)]
pub const fn completes_with_command_status<T: CompletesWithCommandStatus>() {}

/// The wire width of a bt-hci fixed-size value.
#[doc(hidden)]
pub const fn fixed_width<T: FixedWidth>() -> usize {
    core::mem::size_of::<T>()
}

/// Assert that bt-hci decodes the parameters of an event of `width` bytes,
/// and neither one byte fewer nor one more: the catalog's width.
///
/// Values are all zeros or all ones, one of which each field accepts.
#[doc(hidden)]
pub fn assert_event_width<T: for<'a> FromHciBytes<'a>>(width: usize) {
    let decodes = |fill: u8, width: usize| {
        let buffer = [fill; 256];
        T::from_hci_bytes_complete(&buffer[..width]).is_ok()
    };
    let name = core::any::type_name::<T>();
    assert!(
        decodes(0, width) || decodes(1, width),
        "{} does not decode {} bytes",
        name,
        width
    );
    for fill in [0, 1] {
        assert!(
            !decodes(fill, width + 1) && (width == 0 || !decodes(fill, width - 1)),
            "{} decodes a width other than {}",
            name,
            width
        );
    }
}

/// An ST system (SHCI) command, sent on the CPU2 system channel.
///
/// System opcodes share OGF 0x3F with the ACI commands, and some share their
/// full opcode with an ACI command too, so a system command is not a bt-hci
/// [`Cmd`](bt_hci::cmd::Cmd) and cannot be sent to the BLE controller by
/// mistake. The system channel's transport writes [`OPCODE`](Self::OPCODE)
/// and the encoded [`params`](Self::params) into its command buffer, and
/// answers with a Command Complete whose first return parameter is the
/// status, followed by [`Return`](Self::Return).
pub trait SystemCommand {
    /// The command opcode.
    const OPCODE: u16;
    /// The parameters, encoded in catalog order.
    type Params: WriteHci;
    /// The return parameters after the status.
    type Return: for<'de> FromHciBytes<'de> + Copy;

    /// The command's parameters.
    fn params(&self) -> &Self::Params;
}

/// An ST vendor event, which bt-hci delivers as [`bt_hci::event::Vendor`]:
/// a 16-bit vendor event code followed by the event's parameters.
pub trait VendorEvent<'a>: FromHciBytes<'a> {
    /// The vendor event code.
    const CODE: u16;

    /// The latest C name of the event in the catalog.
    const C_NAME: &'static str;

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

/// An ST system (SHCI) event, which the wireless CPU sends on the system
/// channel as a vendor-specific HCI event: a 16-bit sub-event code followed
/// by the event's parameters.
pub trait SystemEvent<'a>: FromHciBytes<'a> {
    /// The sub-event code.
    const CODE: u16;

    /// The latest C name of the event in the catalog.
    const C_NAME: &'static str;

    /// Decode the parameters of a system event, sub-event code included, if
    /// the code is this event's. The parameters must be exactly those the
    /// catalog lists.
    fn from_system_params(params: &'a [u8]) -> Option<Result<Self, FromHciBytesError>> {
        let (code, payload) = params.split_first_chunk::<2>()?;
        (u16::from_le_bytes(*code) == Self::CODE).then(|| Self::from_hci_bytes_complete(payload))
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

    #[test]
    fn borrowed_elements_are_checked_when_decoded() {
        let (flags, rest) = Elements::<bool>::decode(&[1, 0, 1, 9], 3).unwrap();
        assert_eq!(rest, [9]);
        assert_eq!(flags.len(), 3);
        assert_eq!(flags.get(2), Some(true));
        assert_eq!(flags.get(3), None);
        assert!(flags.iter().eq([true, false, true]));
        assert!(Elements::<bool>::decode(&[1, 2], 2).is_err());
        assert!(Elements::<bool>::decode(&[1], 2).is_err());
        assert!(Elements::<u16>::decode(&[], usize::MAX).is_err());
    }

    #[test]
    fn att_bearers_name_a_connection_or_an_enhanced_channel() {
        encoded_width(AttBearer::enhanced(0).unwrap());
        let bearer = AttBearer::from(ConnHandle::new(0x0801));
        assert_eq!(bearer.raw(), 0x0801);
        assert_eq!(bearer.connection(), Some(ConnHandle::new(0x0801)));
        assert_eq!(bearer.channel_index(), None);

        let bearer = AttBearer::enhanced(5).unwrap();
        assert_eq!(bearer.raw(), 0xEA05);
        assert_eq!(bearer.connection(), None);
        assert_eq!(bearer.channel_index(), Some(5));
        assert_eq!(
            AttBearer::from_hci_bytes_complete(&[0x05, 0xEA]),
            Ok(bearer)
        );

        let last_channel = (AttBearer::LAST_ENHANCED - 0xEA00) as u8;
        assert!(AttBearer::enhanced(last_channel).is_some());
        assert_eq!(AttBearer::enhanced(last_channel + 1), None);
        for invalid in [0x0F00, 0xE9FF, AttBearer::LAST_ENHANCED + 1] {
            assert_eq!(AttBearer::from_raw(invalid), None, "{}", invalid);
            assert_eq!(
                AttBearer::from_hci_bytes_complete(&invalid.to_le_bytes()),
                Err(FromHciBytesError::InvalidValue)
            );
        }
    }

    /// Releases before 1.17.0 document 32 enhanced ATT bearers, later ones 64.
    #[test]
    fn enhanced_bearers_follow_the_selected_release() {
        let last = if cfg!(any(feature = "fw_1_15_0", feature = "fw_1_16_0")) {
            0xEA1F
        } else {
            0xEA3F
        };
        assert_eq!(AttBearer::LAST_ENHANCED, last);
        assert_eq!(
            <AttBearer as HciWireType>::LAST_ENHANCED_BEARER,
            AttBearer::LAST_ENHANCED
        );
        assert_eq!(<ConnHandle as HciWireType>::LAST_ENHANCED_BEARER, 0);
    }
}
