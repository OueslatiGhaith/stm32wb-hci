//! libclang parsing of tagged Cube sources and the typed view of their
//! packed structures.
//!
//! Sources are compiled for the Cortex-M target the wrappers are written for
//! (`thumbv7em-none-eabi`, freestanding). The few hosted headers the BLE
//! templates include (`string.h`, …) contribute nothing to the protocol, so a
//! shim directory supplies empty stand-ins; `stm32_wpan_common.h` is replaced
//! by its GCC packing macros and `stm32wbxx.h` by the few CMSIS symbols
//! `shci.c` uses, so the SHCI sources can be parsed without CMSIS.

use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use clang::diagnostic::Severity;
use clang::{Entity, EntityKind, EvaluationResult, Index, TranslationUnit, Type, TypeKind};
use stm32wb_catalog::{Bearer, Element, Field, FieldType, Scalar, Structs};
use tempfile::TempDir;

const SHIM_HEADERS: [(&str, &str); 6] = [
    ("string.h", "/* freestanding shim: no protocol content */\n"),
    ("stdio.h", "/* freestanding shim: no protocol content */\n"),
    ("stdlib.h", "/* freestanding shim: no protocol content */\n"),
    (
        "stm32_wpan_common.h",
        "/* shim: the GCC branch of STM32_WPAN's packing macros */\n\
         #include <stdint.h>\n\
         #define PACKED__ __attribute__((packed))\n\
         #define PACKED_STRUCT struct PACKED__\n\
         #define ALIGN(n) __attribute__((aligned(n)))\n",
    ),
    (
        CMSIS_PACKING,
        "/* shim: the GCC branch of CMSIS's packing macros, which STM32WBA's BLE\n\
         headers use without including */\n\
         #define __PACKED_STRUCT struct __attribute__((packed))\n\
         #define __PACKED_UNION union __attribute__((packed))\n",
    ),
    (
        "stm32wbxx.h",
        "/* shim: the CMSIS symbols shci.c uses, and the C library it reaches through them */\n\
         #include <stddef.h>\n\
         #include <stdint.h>\n\
         void *memcpy(void *destination, const void *source, size_t length);\n\
         typedef struct { volatile uint32_t IPCCBR; } FLASH_TypeDef;\n\
         #define FLASH ((FLASH_TypeDef *)0x58004000UL)\n\
         #define FLASH_IPCCBR_IPCCDBA 0x3FFFUL\n\
         #define SRAM2A_BASE 0x20030000UL\n\
         #define READ_BIT(REG, BIT) ((REG) & (BIT))\n",
    ),
];

/// The shim header defining CMSIS's packing macros, force-included where
/// headers use them without including CMSIS.
pub const CMSIS_PACKING: &str = "cmsis_packing.h";

/// Headers standing in for the hosted C library and CMSIS.
pub struct Shim {
    directory: TempDir,
}

impl Shim {
    pub fn new() -> Result<Self> {
        let directory = tempfile::tempdir().context("could not create the shim")?;
        for (name, contents) in SHIM_HEADERS {
            fs::write(directory.path().join(name), contents)
                .with_context(|| format!("could not write shim header {name}"))?;
        }
        Ok(Self { directory })
    }

    pub fn path(&self) -> &Path {
        self.directory.path()
    }
}

/// Parse one source, failing on any error diagnostic: a translation unit
/// clang could only partly understand would silently lose declarations.
pub fn parse<'i>(
    index: &'i Index<'_>,
    source: &Path,
    shim: &Shim,
    include_dirs: &[PathBuf],
) -> Result<TranslationUnit<'i>> {
    parse_with(index, source, shim, include_dirs, &[])
}

/// [`parse`], force-including the shim headers `prelude` first.
pub fn parse_with<'i>(
    index: &'i Index<'_>,
    source: &Path,
    shim: &Shim,
    include_dirs: &[PathBuf],
    prelude: &[&str],
) -> Result<TranslationUnit<'i>> {
    let mut arguments = vec![
        "--target=thumbv7em-none-eabi".to_owned(),
        "-ffreestanding".to_owned(),
        "-std=c11".to_owned(),
        format!("-I{}", shim.path().display()),
    ];
    arguments.extend(
        include_dirs
            .iter()
            .map(|directory| format!("-I{}", directory.display())),
    );
    for header in prelude {
        arguments.push("-include".to_owned());
        arguments.push(shim.path().join(header).display().to_string());
    }
    let unit = index
        .parser(source)
        .arguments(&arguments)
        .skip_function_bodies(false)
        .parse()
        .with_context(|| format!("libclang could not parse {}", source.display()))?;
    let errors = unit
        .get_diagnostics()
        .into_iter()
        .filter(|diagnostic| diagnostic.get_severity() >= Severity::Error)
        .map(|diagnostic| diagnostic.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(unit)
    } else {
        Err(anyhow!(
            "libclang rejected {}:\n  {}",
            source.display(),
            errors.join("\n  ")
        ))
    }
}

/// Entities declared at the top level of the main file.
pub fn main_file_entities<'tu>(unit: &'tu TranslationUnit<'_>) -> Vec<Entity<'tu>> {
    unit.get_entity()
        .get_children()
        .into_iter()
        .filter(|entity| entity.is_in_main_file())
        .collect()
}

/// The parameters an entity's documentation says address an ATT bearer,
/// with the range of enhanced bearers it gives.
pub fn bearers(entity: Entity<'_>) -> Result<Vec<Bearer>> {
    entity
        .get_comment()
        .map_or(Ok(Vec::new()), |comment| bearers_in(&comment))
}

/// The `@param` blocks of a doc comment that list an enhanced ATT bearer
/// range, written `0xEA00 ... 0xEAnn`. Any other spelling of a value starting
/// at 0xEA is an error rather than a guess.
pub fn bearers_in(comment: &str) -> Result<Vec<Bearer>> {
    let mut bearers = Vec::new();
    for block in comment.split("@param").skip(1) {
        let block = block.split("@return").next().unwrap_or_default();
        let words = block
            .split_whitespace()
            .filter(|word| !word.starts_with('*'))
            .collect::<Vec<_>>();
        let Some((name, _)) = words.split_first() else {
            continue;
        };
        let hex = |word: &str| {
            word.trim_end_matches(':')
                .strip_prefix("0x")
                .and_then(|digits| u16::from_str_radix(digits, 16).ok())
        };
        // Values at 0xEA that do not end a range each start one.
        let mut ranges = (0..words.len())
            .filter(|&index| {
                words[index].starts_with("0xEA") && (index == 0 || words[index - 1] != "...")
            })
            .map(|index| {
                let window = &words[index..words.len().min(index + 3)];
                match window {
                    [first, "...", last] => hex(first).zip(hex(last)).with_context(|| {
                        format!("{name} documents a malformed range {}", window.join(" "))
                    }),
                    _ => Err(anyhow!(
                        "{name} documents a value at 0xEA that is not a range: {}",
                        window.join(" ")
                    )),
                }
            });
        let Some(range) = ranges.next() else {
            continue;
        };
        let (first, last) = range?;
        if ranges.next().is_some() {
            bail!("{name} documents several enhanced ATT bearer ranges");
        }
        bearers.push(Bearer {
            member: (*name).to_owned(),
            first,
            last,
        });
    }
    Ok(bearers)
}

/// The `@param` blocks of an entity's documentation noting that the size
/// their member declares is only a maximum.
pub fn maximum_sized(entity: Entity<'_>) -> Vec<String> {
    entity
        .get_comment()
        .map_or_else(Vec::new, |comment| maximum_sized_in(&comment))
}

pub fn maximum_sized_in(comment: &str) -> Vec<String> {
    comment
        .split("@param")
        .skip(1)
        .filter_map(|block| {
            let block = block.split("@return").next().unwrap_or_default();
            let words = block
                .split_whitespace()
                .filter(|word| !word.starts_with('*'))
                .collect::<Vec<_>>();
            let (name, _) = words.split_first()?;
            words
                .join(" ")
                .contains("the indicated size is the maximum size")
                .then(|| (*name).to_owned())
        })
        .collect()
}

/// Descend through implicit conversions, parentheses, and casts.
pub fn strip(mut entity: Entity<'_>) -> Entity<'_> {
    while matches!(
        entity.get_kind(),
        EntityKind::UnexposedExpr | EntityKind::ParenExpr | EntityKind::CStyleCastExpr
    ) {
        match entity.get_children().last() {
            Some(child) => entity = *child,
            None => break,
        }
    }
    entity
}

pub fn int_value(entity: Entity<'_>) -> Option<i64> {
    match entity.evaluate()? {
        EvaluationResult::SignedInteger(value) => Some(value),
        EvaluationResult::UnsignedInteger(value) => i64::try_from(value).ok(),
        _ => None,
    }
}

/// The operator spelling of a binary, compound-assignment, or unary operator.
pub fn operator(entity: Entity<'_>) -> Option<String> {
    match entity.get_kind() {
        EntityKind::BinaryOperator | EntityKind::CompoundAssignOperator => {
            // libclang spells binary operators as their display name; older
            // releases do not, so fall back to the token after the left operand.
            if let Some(name) = entity.get_display_name().filter(|name| !name.is_empty()) {
                return Some(name);
            }
            let left = entity.get_children().first()?.get_range()?.tokenize().len();
            entity
                .get_range()?
                .tokenize()
                .get(left)
                .map(|token| token.get_spelling())
        }
        EntityKind::UnaryOperator => entity
            .get_range()?
            .tokenize()
            .first()
            .map(|token| token.get_spelling()),
        _ => None,
    }
}

/// Every entity below `root`, in source (pre-)order.
pub fn descendants(root: Entity<'_>) -> Vec<Entity<'_>> {
    let mut nodes = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        nodes.push(node);
        let mut children = node.get_children();
        children.reverse();
        stack.extend(children);
    }
    nodes
}

/// A function parameter referenced by `entity`, if that is all it is.
pub fn parameter(entity: Entity<'_>) -> Option<String> {
    let entity = strip(entity);
    (entity.get_kind() == EntityKind::DeclRefExpr)
        .then(|| entity.get_reference())
        .flatten()
        .filter(|reference| reference.get_kind() == EntityKind::ParmDecl)
        .and_then(|reference| reference.get_name())
}

/// A local variable referenced by `entity`, if that is all it is.
pub fn local(entity: Entity<'_>) -> Option<String> {
    let entity = strip(entity);
    (entity.get_kind() == EntityKind::DeclRefExpr)
        .then(|| entity.get_reference())
        .flatten()
        .filter(|reference| reference.get_kind() == EntityKind::VarDecl)
        .and_then(|reference| reference.get_name())
}

/// `base->field` or `base.field`: the base variable and the field name.
pub fn member(entity: Entity<'_>) -> Option<(String, String)> {
    let entity = strip(entity);
    if entity.get_kind() != EntityKind::MemberRefExpr {
        return None;
    }
    let field = entity
        .get_reference()
        .filter(|reference| reference.get_kind() == EntityKind::FieldDecl)?
        .get_name()?;
    let base = local(*entity.get_children().first()?)?;
    Some((base, field))
}

/// `&base->field`: the base variable and the field name.
pub fn address_of_member(entity: Entity<'_>) -> Option<(String, String)> {
    let entity = strip(entity);
    if entity.get_kind() != EntityKind::UnaryOperator || operator(entity).as_deref() != Some("&") {
        return None;
    }
    member(*entity.get_children().first()?)
}

/// A C integer type as a wire scalar.
pub fn scalar(ty: Type<'_>) -> Option<Scalar> {
    let canonical = ty.get_canonical_type();
    let signed = match canonical.get_kind() {
        TypeKind::CharS
        | TypeKind::SChar
        | TypeKind::Short
        | TypeKind::Int
        | TypeKind::Long
        | TypeKind::LongLong => true,
        TypeKind::CharU
        | TypeKind::UChar
        | TypeKind::UShort
        | TypeKind::UInt
        | TypeKind::ULong
        | TypeKind::ULongLong
        | TypeKind::Bool => false,
        _ => return None,
    };
    Some(match (canonical.get_sizeof().ok()?, signed) {
        (1, false) => Scalar::U8,
        (2, false) => Scalar::U16,
        (4, false) => Scalar::U32,
        (8, false) => Scalar::U64,
        (1, true) => Scalar::I8,
        (2, true) => Scalar::I16,
        (4, true) => Scalar::I32,
        (8, true) => Scalar::I64,
        _ => return None,
    })
}

/// The C type of a structure member, classified for wire conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CType {
    Scalar(Scalar),
    /// A typedef'd packed structure or union.
    Record(String),
    Array {
        element: Box<CType>,
        len: usize,
        /// The declared length is derived from `BLE_CMD_MAX_PARAM_LEN` or
        /// `BLE_EVT_MAX_PARAM_LEN`: the array is a variable-length buffer.
        capacity: bool,
    },
    Enum(String),
    Pointer,
    Other(String),
}

impl CType {
    fn of(ty: Type<'_>, capacity: bool) -> Self {
        if let Some(scalar) = scalar(ty) {
            return Self::Scalar(scalar);
        }
        let canonical = ty.get_canonical_type();
        match canonical.get_kind() {
            TypeKind::ConstantArray => Self::Array {
                element: Box::new(Self::of(
                    ty.get_element_type().expect("arrays have element types"),
                    false,
                )),
                len: canonical.get_size().unwrap_or(0),
                capacity,
            },
            TypeKind::Record => {
                typedef_name(ty).map_or_else(|| Self::Other(ty.get_display_name()), Self::Record)
            }
            TypeKind::Enum => Self::Enum(ty.get_display_name()),
            TypeKind::Pointer => Self::Pointer,
            _ => Self::Other(ty.get_display_name()),
        }
    }
}

/// The typedef naming a record type, e.g. `Adv_Set_t` for
/// `typedef __PACKED_STRUCT { ... } Adv_Set_t;`.
pub fn typedef_name(ty: Type<'_>) -> Option<String> {
    let mut ty = ty;
    loop {
        match ty.get_kind() {
            TypeKind::Typedef => return ty.get_typedef_name(),
            TypeKind::Elaborated => ty = ty.get_elaborated_type()?,
            _ => {
                let declaration = ty.get_declaration()?;
                return declaration.get_name();
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CField {
    pub name: String,
    pub ty: CType,
    /// The documentation comment of the member, which may list its values.
    pub comment: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CRecord {
    pub union: bool,
    pub size: usize,
    /// Every member sits at the byte offset the preceding members imply.
    pub packed: bool,
    pub fields: Vec<CField>,
}

/// Every typedef'd structure and union visible in a translation unit.
pub fn records(unit: &TranslationUnit<'_>) -> BTreeMap<String, CRecord> {
    let mut records = BTreeMap::new();
    for entity in unit.get_entity().get_children() {
        if entity.get_kind() != EntityKind::TypedefDecl {
            continue;
        }
        let (Some(name), Some(underlying)) =
            (entity.get_name(), entity.get_typedef_underlying_type())
        else {
            continue;
        };
        let canonical = underlying.get_canonical_type();
        if canonical.get_kind() != TypeKind::Record {
            continue;
        }
        let Some(declaration) = canonical.get_declaration() else {
            continue;
        };
        let union = declaration.get_kind() == EntityKind::UnionDecl;
        let Some(members) = canonical.get_fields() else {
            continue;
        };
        let mut fields = Vec::new();
        let mut packed = true;
        let mut offset_bits = 0usize;
        for member in members {
            let (Some(field_name), Some(ty)) = (member.get_name(), member.get_type()) else {
                continue;
            };
            let capacity = member.get_range().is_some_and(|range| {
                range.tokenize().iter().any(|token| {
                    matches!(
                        token.get_spelling().as_str(),
                        "BLE_CMD_MAX_PARAM_LEN" | "BLE_EVT_MAX_PARAM_LEN"
                    )
                })
            });
            let expected = if union { 0 } else { offset_bits };
            if member.get_offset_of_field().ok() != Some(expected) {
                packed = false;
            }
            offset_bits += ty.get_sizeof().unwrap_or(0) * 8;
            fields.push(CField {
                name: field_name,
                ty: CType::of(ty, capacity),
                comment: member.get_comment(),
            });
        }
        let size = canonical.get_sizeof().unwrap_or(0);
        if !union && size * 8 != offset_bits {
            packed = false;
        }
        records.insert(
            name,
            CRecord {
                union,
                size,
                packed,
                fields,
            },
        );
    }
    records
}

/// Convert a fixed-width C member to a catalog field type, collecting the
/// structures it references. Variable parts need code evidence and are
/// handled by the command and event analyses instead.
pub fn fixed_field(
    field: &CField,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<FieldType, String> {
    match &field.ty {
        CType::Scalar(scalar) => Ok(FieldType::Scalar(*scalar)),
        CType::Record(name) => {
            define_struct(name, records, structs)?;
            Ok(FieldType::Struct(name.clone()))
        }
        CType::Array {
            element,
            len,
            capacity: false,
        } => Ok(FieldType::Array {
            element: element_type(element, records, structs)?,
            len: u16::try_from(*len).map_err(|_| format!("{} is too long", field.name))?,
        }),
        CType::Array { capacity: true, .. } => {
            Err(format!("{} is a variable-length buffer", field.name))
        }
        CType::Enum(name) => Err(format!(
            "{} has enum type {name}, whose width depends on the CPU2 ABI",
            field.name
        )),
        CType::Pointer => Err(format!(
            "{} is a pointer; the payload is decoded procedurally",
            field.name
        )),
        CType::Other(name) => Err(format!("{} has unsupported type {name}", field.name)),
    }
}

pub fn element_type(
    element: &CType,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Element, String> {
    match element {
        CType::Scalar(scalar) => Ok(Element::Scalar(*scalar)),
        CType::Record(name) => {
            define_struct(name, records, structs)?;
            Ok(Element::Struct(name.clone()))
        }
        other => Err(format!("unsupported array element {other:?}")),
    }
}

/// Byte width of an array element.
pub fn element_width(element: &CType, records: &BTreeMap<String, CRecord>) -> Option<usize> {
    match element {
        CType::Scalar(scalar) => Some(usize::from(scalar.width())),
        CType::Record(name) => records.get(name).map(|record| record.size),
        _ => None,
    }
}

fn define_struct(
    name: &str,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<(), String> {
    if structs.contains_key(name) {
        return Ok(());
    }
    let record = records
        .get(name)
        .ok_or_else(|| format!("structure {name} is not declared"))?;
    if record.union {
        return Err(format!("{name} is a union without a selector"));
    }
    if !record.packed {
        return Err(format!("{name} is not packed"));
    }
    let mut fields = Vec::new();
    for field in &record.fields {
        let ty = fixed_field(field, records, structs).map_err(|error| format!("{name}.{error}"))?;
        fields.push(Field::new(field.name.clone(), ty));
    }
    structs.insert(name.to_owned(), fields);
    Ok(())
}
