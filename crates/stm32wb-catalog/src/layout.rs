//! Wire layouts of command parameters, command returns, and event payloads.
//!
//! Each field is stored as one line of a small notation so that a release
//! diff of the checked-in catalog reads like a protocol changelog:
//!
//! ```text
//! Advertising_Type: u8
//! Direct_Address: [u8; 6]
//! Local_Name: [u8; Local_Name_Length] (capacity 242)
//! Adv_Set: [Adv_Set_t; Number_of_Sets] (capacity 63)
//! Service_UUID: union(Service_UUID_Type) { 1 => 2, 2 => 16 }
//! fw_src_add: u32 (optional)
//! ```
//!
//! A counted field's element count is the value of the named earlier field.
//! A union's width is selected by the value of the named earlier field; `_`
//! marks the width used for every other selector value. Optional members end
//! the layout: each is sent only if every earlier one is, so the parameters
//! stop at the first one omitted.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::Error;

/// A fixed-width little-endian integer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Scalar {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
}

impl Scalar {
    pub const fn width(self) -> u16 {
        match self {
            Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 => 4,
            Self::U64 | Self::I64 => 8,
        }
    }

    pub const fn is_signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            _ => return None,
        })
    }
}

/// The element of an array: an integer or a named packed structure.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Element {
    Scalar(Scalar),
    Struct(String),
}

impl fmt::Display for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(scalar) => f.write_str(scalar.name()),
            Self::Struct(name) => f.write_str(name),
        }
    }
}

/// One alternative of a selector-dependent union.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnionVariant {
    /// Selector value, or `None` for every value not listed explicitly.
    pub tag: Option<u64>,
    /// Encoded width of this alternative in bytes.
    pub width: u16,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FieldType {
    Scalar(Scalar),
    /// A single embedded packed structure.
    Struct(String),
    /// A fixed number of elements.
    Array {
        element: Element,
        len: u16,
    },
    /// A variable number of elements; `count` names the earlier field
    /// holding the element count. `capacity` is the element capacity the
    /// generated C buffer declares.
    Counted {
        element: Element,
        count: String,
        capacity: u16,
    },
    /// Bytes whose width depends on the value of the earlier `selector` field.
    Union {
        selector: String,
        variants: Vec<UnionVariant>,
    },
    /// An integer the sender may omit, together with every later member.
    Optional(Scalar),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Field {
    pub name: String,
    pub ty: FieldType,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: FieldType) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.name)?;
        match &self.ty {
            FieldType::Scalar(scalar) => f.write_str(scalar.name()),
            FieldType::Struct(name) => f.write_str(name),
            FieldType::Array { element, len } => write!(f, "[{element}; {len}]"),
            FieldType::Counted {
                element,
                count,
                capacity,
            } => write!(f, "[{element}; {count}] (capacity {capacity})"),
            FieldType::Union { selector, variants } => {
                write!(f, "union({selector}) {{ ")?;
                for (index, variant) in variants.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    match variant.tag {
                        Some(tag) => write!(f, "{tag} => {}", variant.width)?,
                        None => write!(f, "_ => {}", variant.width)?,
                    }
                }
                f.write_str(" }")
            }
            FieldType::Optional(scalar) => write!(f, "{} (optional)", scalar.name()),
        }
    }
}

impl FromStr for Field {
    type Err = Error;

    fn from_str(source: &str) -> Result<Self, Error> {
        let mut parser = Parser::new(source);
        let field = parser.field()?;
        parser.end()?;
        Ok(field)
    }
}

impl Serialize for Field {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Field {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Packed structures referenced by the layouts of one definition.
pub type Structs = BTreeMap<String, Vec<Field>>;

/// A layout extracted from the generated source, or the reason it could not
/// be derived without guessing.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Layout {
    Fields(Vec<Field>),
    Unresolved(String),
}

impl Layout {
    pub fn fields(&self) -> Option<&[Field]> {
        match self {
            Self::Fields(fields) => Some(fields),
            Self::Unresolved(_) => None,
        }
    }

    /// Validate field references and compute the encoded size envelope.
    pub fn envelope(&self, structs: &Structs) -> Result<Option<Envelope>, Error> {
        match self {
            Self::Fields(fields) => envelope(fields, structs).map(Some),
            Self::Unresolved(_) => Ok(None),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum LayoutRepr {
    Fields(Vec<Field>),
    Unresolved { unresolved: String },
}

impl Serialize for Layout {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Fields(fields) => LayoutRepr::Fields(fields.clone()),
            Self::Unresolved(reason) => LayoutRepr::Unresolved {
                unresolved: reason.clone(),
            },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Layout {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match LayoutRepr::deserialize(deserializer)? {
            LayoutRepr::Fields(fields) => Self::Fields(fields),
            LayoutRepr::Unresolved { unresolved } => Self::Unresolved(unresolved),
        })
    }
}

/// Inclusive encoded size bounds of a layout in bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Envelope {
    pub min: usize,
    pub max: usize,
}

/// Fixed width of a structure, validating that it contains no variable parts.
pub fn struct_width(name: &str, structs: &Structs) -> Result<u16, Error> {
    let fields = structs
        .get(name)
        .ok_or_else(|| Error::invalid(format!("structure {name} is not defined")))?;
    let mut width = 0u16;
    for field in fields {
        let field_width = match &field.ty {
            FieldType::Scalar(scalar) => scalar.width(),
            FieldType::Struct(inner) if inner != name => struct_width(inner, structs)?,
            FieldType::Array { element, len } => element_width(element, structs)?
                .checked_mul(*len)
                .ok_or_else(|| Error::invalid(format!("{name}.{} overflows", field.name)))?,
            _ => {
                return Err(Error::invalid(format!(
                    "structure {name} field {} is not fixed-width",
                    field.name
                )));
            }
        };
        width = width
            .checked_add(field_width)
            .ok_or_else(|| Error::invalid(format!("structure {name} overflows")))?;
    }
    Ok(width)
}

pub fn element_width(element: &Element, structs: &Structs) -> Result<u16, Error> {
    match element {
        Element::Scalar(scalar) => Ok(scalar.width()),
        Element::Struct(name) => struct_width(name, structs),
    }
}

fn envelope(fields: &[Field], structs: &Structs) -> Result<Envelope, Error> {
    let mut min = 0usize;
    let mut max = 0usize;
    let mut earlier: BTreeMap<&str, &FieldType> = BTreeMap::new();
    let mut optional: Option<&str> = None;
    for field in fields {
        if let Some(optional) = optional
            && !matches!(field.ty, FieldType::Optional(_))
        {
            return Err(Error::invalid(format!(
                "{} follows the optional member {optional}, so it must be optional",
                field.name
            )));
        }
        let (field_min, field_max) = match &field.ty {
            FieldType::Scalar(scalar) => (scalar.width().into(), scalar.width().into()),
            FieldType::Struct(name) => {
                let width = struct_width(name, structs)?.into();
                (width, width)
            }
            FieldType::Array { element, len } => {
                let width = usize::from(element_width(element, structs)?) * usize::from(*len);
                (width, width)
            }
            FieldType::Counted {
                element,
                count,
                capacity,
            } => {
                require_scalar_reference(&earlier, &field.name, count)?;
                let width = usize::from(element_width(element, structs)?);
                (0, width * usize::from(*capacity))
            }
            FieldType::Union { selector, variants } => {
                require_scalar_reference(&earlier, &field.name, selector)?;
                if variants.is_empty() {
                    return Err(Error::invalid(format!("union {} is empty", field.name)));
                }
                let widths = variants.iter().map(|variant| usize::from(variant.width));
                (widths.clone().min().unwrap_or(0), widths.max().unwrap_or(0))
            }
            FieldType::Optional(scalar) => {
                optional = Some(&field.name);
                (0, scalar.width().into())
            }
        };
        if earlier.insert(&field.name, &field.ty).is_some() {
            return Err(Error::invalid(format!("duplicate field {}", field.name)));
        }
        min += field_min;
        max += field_max;
    }
    Ok(Envelope { min, max })
}

fn require_scalar_reference(
    earlier: &BTreeMap<&str, &FieldType>,
    field: &str,
    reference: &str,
) -> Result<(), Error> {
    match earlier.get(reference) {
        Some(FieldType::Scalar(scalar)) if !scalar.is_signed() => Ok(()),
        Some(_) => Err(Error::invalid(format!(
            "{field} refers to {reference}, which is not an unsigned integer"
        ))),
        None => Err(Error::invalid(format!(
            "{field} refers to {reference}, which is not an earlier field"
        ))),
    }
}

struct Parser<'a> {
    source: &'a str,
    rest: &'a str,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            rest: source,
        }
    }

    fn error(&self, expected: &str) -> Error {
        Error::parse(format!(
            "invalid field {:?}: expected {expected} at {:?}",
            self.source, self.rest
        ))
    }

    fn skip_space(&mut self) {
        self.rest = self.rest.trim_start();
    }

    fn eat(&mut self, token: &str) -> bool {
        self.skip_space();
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn expect(&mut self, token: &str) -> Result<(), Error> {
        if self.eat(token) {
            Ok(())
        } else {
            Err(self.error(&format!("{token:?}")))
        }
    }

    fn word(&mut self) -> Option<&'a str> {
        self.skip_space();
        let end = self
            .rest
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(self.rest.len());
        (end > 0).then(|| {
            let (word, rest) = self.rest.split_at(end);
            self.rest = rest;
            word
        })
    }

    fn identifier(&mut self) -> Result<&'a str, Error> {
        match self.word() {
            Some(word) if !word.starts_with(|c: char| c.is_ascii_digit()) => Ok(word),
            _ => Err(self.error("an identifier")),
        }
    }

    fn integer<T: FromStr>(&mut self) -> Result<T, Error> {
        self.word()
            .and_then(|word| word.parse().ok())
            .ok_or_else(|| self.error("an integer"))
    }

    fn element(&mut self) -> Result<Element, Error> {
        let name = self.identifier()?;
        Ok(Scalar::from_name(name)
            .map_or_else(|| Element::Struct(name.to_owned()), Element::Scalar))
    }

    fn field(&mut self) -> Result<Field, Error> {
        let name = self.identifier()?.to_owned();
        self.expect(":")?;
        let ty = if self.eat("[") {
            let element = self.element()?;
            self.expect(";")?;
            let count = self.word().ok_or_else(|| self.error("a count"))?;
            self.expect("]")?;
            if let Ok(len) = count.parse() {
                FieldType::Array { element, len }
            } else {
                self.expect("(")?;
                self.expect("capacity")?;
                let capacity = self.integer()?;
                self.expect(")")?;
                FieldType::Counted {
                    element,
                    count: count.to_owned(),
                    capacity,
                }
            }
        } else if self.eat("union") {
            self.expect("(")?;
            let selector = self.identifier()?.to_owned();
            self.expect(")")?;
            self.expect("{")?;
            let mut variants = Vec::new();
            loop {
                let tag = if self.eat("_") {
                    None
                } else {
                    Some(self.integer()?)
                };
                self.expect("=>")?;
                variants.push(UnionVariant {
                    tag,
                    width: self.integer()?,
                });
                if !self.eat(",") {
                    break;
                }
            }
            self.expect("}")?;
            FieldType::Union { selector, variants }
        } else {
            match self.element()? {
                Element::Scalar(scalar) if self.eat("(") => {
                    self.expect("optional")?;
                    self.expect(")")?;
                    FieldType::Optional(scalar)
                }
                Element::Scalar(scalar) => FieldType::Scalar(scalar),
                Element::Struct(name) => FieldType::Struct(name),
            }
        };
        Ok(Field { name, ty })
    }

    fn end(&mut self) -> Result<(), Error> {
        self.skip_space();
        if self.rest.is_empty() {
            Ok(())
        } else {
            Err(self.error("end of field"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(source: &str) -> Field {
        let field: Field = source.parse().unwrap();
        assert_eq!(field.to_string(), source, "notation must round-trip");
        field
    }

    #[test]
    fn notation_round_trips_every_field_type() {
        assert_eq!(field("Status: u8").ty, FieldType::Scalar(Scalar::U8));
        assert_eq!(field("RSSI: i8").ty, FieldType::Scalar(Scalar::I8));
        field("Direct_Address: [u8; 6]");
        field("Entry: Adv_Set_t");
        field("Adv_Set: [Adv_Set_t; Number_of_Sets] (capacity 63)");
        field("Service_UUID: union(Service_UUID_Type) { 1 => 2, 2 => 16 }");
        field("Include_UUID: union(Include_UUID_Type) { 2 => 16, _ => 2 }");
        assert_eq!(
            field("fw_src_add: u32 (optional)").ty,
            FieldType::Optional(Scalar::U32)
        );
    }

    #[test]
    fn notation_rejects_malformed_fields() {
        for source in [
            "Status u8",
            "Status: u8 trailing",
            "Data: [u8; Len]",
            "Data: [u8; Len] (capacity x)",
            "1st: u8",
            "Uuid: union(Type) { }",
            "Entry: Adv_Set_t (optional)",
            "Address: u32 (capacity 4)",
        ] {
            assert!(source.parse::<Field>().is_err(), "{source}");
        }
    }

    #[test]
    fn envelopes_follow_references() {
        let structs = Structs::from([(
            "Adv_Set_t".to_owned(),
            vec![field("Handle: u8"), field("Duration: u16")],
        )]);
        let layout = Layout::Fields(vec![
            field("Enable: u8"),
            field("Number_of_Sets: u8"),
            field("Adv_Set: [Adv_Set_t; Number_of_Sets] (capacity 84)"),
        ]);
        assert_eq!(
            layout.envelope(&structs).unwrap(),
            Some(Envelope {
                min: 2,
                max: 2 + 3 * 84
            })
        );

        let dangling = Layout::Fields(vec![field("Data: [u8; Len] (capacity 4)")]);
        assert!(dangling.envelope(&structs).is_err());
        let undefined = Layout::Fields(vec![field("Entry: Missing_t")]);
        assert!(undefined.envelope(&structs).is_err());
        let optional = Layout::Fields(vec![
            field("Mode: u8"),
            field("Source: u32 (optional)"),
            field("Destination: u32 (optional)"),
        ]);
        assert_eq!(
            optional.envelope(&structs).unwrap(),
            Some(Envelope { min: 1, max: 9 })
        );
        let required_after_optional =
            Layout::Fields(vec![field("Source: u32 (optional)"), field("Mode: u8")]);
        assert!(required_after_optional.envelope(&structs).is_err());
        let counted_by_optional = Layout::Fields(vec![
            field("Len: u8 (optional)"),
            field("Data: [u8; Len] (capacity 4)"),
        ]);
        assert!(counted_by_optional.envelope(&structs).is_err());

        assert_eq!(
            Layout::Unresolved("pointer field".into())
                .envelope(&structs)
                .unwrap(),
            None
        );
    }
}
