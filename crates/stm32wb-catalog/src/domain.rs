//! The values a member may take, as the generated headers document them.
//!
//! Each `@param` block of a generated header may list its values after
//! `Values:`, or its bits after `Flags:`:
//!
//! ```text
//! @param Advertising_Interval_Min Minimum advertising interval.
//!        Time = N * 0.625 ms.
//!        Values:
//!        - 0x0020 (20.000 ms)  ... 0x4000 (10240.000 ms)
//! ```
//!
//! The catalog records each list with the releases documenting it, one item
//! per line of the list, written the way the header writes the value:
//!
//! ```text
//! 0x00: ADV_IND (Connectable undirected advertising)
//! 0x0020..=0x4000
//! -127..=20: Tx power
//! ```
//!
//! A time member's unit is derived from the durations the list writes next to
//! its values, such as `(20.000 ms)` for 0x0020, and must agree with every one
//! of them.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Element, Error, Field, FieldType, Layout, ReleaseRange, Scalar};

/// One documented value, or an inclusive range of them, with its label.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DomainItem {
    pub first: i64,
    pub last: i64,
    /// Hexadecimal digits the header writes the value with, or `None` for a
    /// decimal value.
    pub hex_digits: Option<u8>,
    pub label: Option<String>,
}

impl DomainItem {
    fn write_value(&self, f: &mut fmt::Formatter<'_>, value: i64) -> fmt::Result {
        match self.hex_digits {
            Some(digits) => write!(f, "0x{value:0width$X}", width = usize::from(digits)),
            None => write!(f, "{value}"),
        }
    }
}

impl fmt::Display for DomainItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.write_value(f, self.first)?;
        if self.last != self.first {
            f.write_str("..=")?;
            self.write_value(f, self.last)?;
        }
        if let Some(label) = &self.label {
            write!(f, ": {label}")?;
        }
        Ok(())
    }
}

/// A value written in hexadecimal (`0x0020`) or decimal (`-127`), with the
/// hexadecimal digits it was written with.
pub fn parse_value(text: &str) -> Option<(i64, Option<u8>)> {
    match text.strip_prefix("0x") {
        Some(digits) if !digits.is_empty() && digits.len() <= 16 => {
            let value = u64::from_str_radix(digits, 16).ok()?;
            let upper = digits
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase());
            let value = i64::try_from(value).ok()?;
            upper.then_some((value, Some(digits.len() as u8)))
        }
        Some(_) => None,
        None => text.parse().ok().map(|value| (value, None)),
    }
}

impl FromStr for DomainItem {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Error> {
        let error = || Error::parse(format!("invalid domain item {source:?}"));
        let (values, label) = match source.split_once(": ") {
            Some((values, label)) => (values, Some(label.to_owned())),
            None => (source, None),
        };
        let (first, last) = match values.split_once("..=") {
            Some((first, last)) => (first, Some(last)),
            None => (values, None),
        };
        let (first, digits) = parse_value(first).ok_or_else(error)?;
        let last = match last {
            Some(last) => match parse_value(last).ok_or_else(error)? {
                (value, last_digits) if last_digits.is_some() == digits.is_some() => value,
                _ => return Err(error()),
            },
            None => first,
        };
        let item = Self {
            first,
            last,
            hex_digits: digits,
            label,
        };
        if item.first > item.last || item.to_string() != source {
            return Err(error());
        }
        Ok(item)
    }
}

impl Serialize for DomainItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for DomainItem {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Whether a list gives the values of a member or the bits of a bitmap.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DomainKind {
    Values,
    Flags,
}

/// The documented values of one member.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Domain {
    pub kind: DomainKind,
    pub items: Vec<DomainItem>,
    /// The duration of one unit in microseconds, for a time member.
    pub unit_us: Option<u32>,
}

/// The documented values of one member over a range of releases.
///
/// Written as a table with `values` or `flags`, `unit_us` for a time member,
/// and `returned = true` for a return parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberDomain {
    pub releases: ReleaseRange,
    pub member: String,
    /// Whether the member is a return parameter rather than a parameter.
    pub returned: bool,
    pub domain: Domain,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberDomainRepr {
    releases: ReleaseRange,
    member: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    returned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    values: Option<Vec<DomainItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    flags: Option<Vec<DomainItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit_us: Option<u32>,
}

impl Serialize for MemberDomain {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let items = Some(self.domain.items.clone());
        let (values, flags) = match self.domain.kind {
            DomainKind::Values => (items, None),
            DomainKind::Flags => (None, items),
        };
        MemberDomainRepr {
            releases: self.releases,
            member: self.member.clone(),
            returned: self.returned,
            values,
            flags,
            unit_us: self.domain.unit_us,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MemberDomain {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let repr = MemberDomainRepr::deserialize(deserializer)?;
        let (kind, items) = match (repr.values, repr.flags) {
            (Some(items), None) => (DomainKind::Values, items),
            (None, Some(items)) => (DomainKind::Flags, items),
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "domain of {} must set exactly one of `values` or `flags`",
                    repr.member
                )));
            }
        };
        Ok(Self {
            releases: repr.releases,
            member: repr.member,
            returned: repr.returned,
            domain: Domain {
                kind,
                items,
                unit_us: repr.unit_us,
            },
        })
    }
}

/// The inclusive range of values a scalar holds.
fn scalar_range(scalar: Scalar) -> (i64, i64) {
    let bits = 8 * u32::from(scalar.width());
    if scalar.is_signed() {
        (-(1 << (bits - 1)), (1 << (bits - 1)) - 1)
    } else if bits == 64 {
        (0, i64::MAX)
    } else {
        (0, (1 << bits) - 1)
    }
}

/// A domain belongs to an integer member of the layout, lists distinct items
/// in ascending order, holds only values the member can encode, and, for
/// flags, lists single bits or a named zero.
pub(crate) fn validate_domain(layout: &Layout, domain: &MemberDomain) -> Result<(), Error> {
    let member = &domain.member;
    let Layout::Fields(fields) = layout else {
        return Err(Error::invalid(format!(
            "{member} has a domain, but its layout is unresolved"
        )));
    };
    // A byte array of up to 8 bytes, such as an event mask, is a
    // little-endian integer on the wire.
    let (min, max, type_name) = match fields.iter().find(|field| field.name == *member) {
        Some(Field {
            ty: FieldType::Scalar(scalar) | FieldType::Optional(scalar),
            ..
        }) => {
            let (min, max) = scalar_range(*scalar);
            (min, max, scalar.name().to_owned())
        }
        Some(Field {
            ty:
                FieldType::Array {
                    element: Element::Scalar(Scalar::U8),
                    len: len @ 1..=8,
                },
            ..
        }) => {
            let (min, max) = scalar_range(match len {
                1 => Scalar::U8,
                2 => Scalar::U16,
                3 | 4 => Scalar::U32,
                _ => Scalar::U64,
            });
            let max = if *len == 3 { (1 << 24) - 1 } else { max };
            (min, max, format!("[u8; {len}]"))
        }
        Some(_) => {
            return Err(Error::invalid(format!(
                "{member} has a domain, but is not an integer"
            )));
        }
        None => {
            return Err(Error::invalid(format!(
                "{member} has a domain, but is not a member"
            )));
        }
    };
    // Headers document some unsigned members, such as a transmit power in
    // dBm, with negative values: the byte's two's-complement reading.
    let min = if min == 0 && max < i64::MAX {
        -(max + 1) / 2
    } else {
        min
    };
    let items = &domain.domain.items;
    if items.is_empty() {
        return Err(Error::invalid(format!("{member} has an empty domain")));
    }
    for item in items {
        if item.first < min || item.last > max {
            return Err(Error::invalid(format!(
                "{member} documents {item}, which a {type_name} cannot hold"
            )));
        }
        if domain.domain.kind == DomainKind::Flags
            && (item.first != item.last || (item.first != 0 && item.first & (item.first - 1) != 0))
        {
            return Err(Error::invalid(format!(
                "{member} documents the flag {item}, which is not a single bit"
            )));
        }
    }
    // Items may overlap: some lists depend on the stack profile, such as
    // `0x00 ... 0x25: for BO variant` and `0x00 ... 0xFF: otherwise`.
    if items.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::invalid(format!(
            "{member} documents values out of order or twice"
        )));
    }
    if domain.domain.unit_us == Some(0) {
        return Err(Error::invalid(format!("{member} has a zero unit")));
    }
    Ok(())
}

/// How an item's label restricts it to one MCU of the documented family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mcu {
    Stm32wb,
    Stm32wba,
}

/// The MCU a phrase such as `only for STM32WB` names, if it ends right after
/// the name, as in `(only for STM32WB)` or `with STM32WBA`.
fn mcu_after(label: &str, phrase: &str) -> Option<Mcu> {
    let rest = &label[label.find(phrase)? + phrase.len()..];
    let rest = rest.strip_prefix(" STM32WB")?;
    let (mcu, rest) = match rest.strip_prefix('A') {
        Some(rest) => (Mcu::Stm32wba, rest),
        None => (Mcu::Stm32wb, rest),
    };
    rest.chars()
        .next()
        .is_none_or(|next| !next.is_ascii_alphanumeric() && next != ' ')
        .then_some(mcu)
}

impl Domain {
    /// The inclusive ranges of values an STM32WB binary accepts: every item,
    /// except those whose label says they are not supported on STM32WB or
    /// only on STM32WBA. A label naming any other condition, such as a stack
    /// variant, is an error rather than a guess, and so are flags, which
    /// document bits rather than values.
    pub fn stm32wb_ranges(&self) -> Result<Vec<(i64, i64)>, String> {
        if self.kind == DomainKind::Flags {
            return Err("its documentation lists bits rather than values".to_owned());
        }
        let mut ranges = Vec::new();
        for item in &self.items {
            let label = item.label.as_deref().unwrap_or_default();
            let mcu = mcu_after(label, "not supported on")
                .map(|mcu| (mcu, false))
                .or_else(|| mcu_after(label, "only for").map(|mcu| (mcu, true)))
                .or_else(|| mcu_after(label, "with").map(|mcu| (mcu, true)));
            let applies = match mcu {
                Some((mcu, only)) => (mcu == Mcu::Stm32wb) == only,
                None if label.contains("STM32WB") || label.contains("variant") => {
                    return Err(format!(
                        "{item} names a condition the catalog cannot interpret"
                    ));
                }
                None => true,
            };
            if applies {
                ranges.push((item.first, item.last));
            }
        }
        Ok(ranges)
    }
}
