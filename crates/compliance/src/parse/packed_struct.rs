//! Parser for packed structs in ST generated type headers.
//!
//! The generated `ble_types.h` file uses `typedef __PACKED_STRUCT { ... } Name;`
//! for command/event payload types. This module extracts fields and computes
//! byte offsets when all field sizes are statically known.

use super::common::{clean_doc_line, find_matching_brace, identifier};
use super::docs::parse_param_doc;
use crate::spec::{PackedStructSpec, ParamDoc, StructFieldSpec, WireType, wire_type_for};
use anyhow::{Context, Result};
use chumsky::prelude::*;
use std::collections::HashMap;

/// Parses all packed structs from a generated ST header.
pub(super) fn parse_packed_structs(source: &str) -> Result<Vec<PackedStructSpec>> {
    let mut structs = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = source[cursor..].find("typedef __PACKED_STRUCT") {
        let start = cursor + relative_start;
        let Some(open_brace) = source[start..].find('{').map(|idx| start + idx) else {
            break;
        };
        let close_brace = find_matching_brace(source, open_brace)
            .with_context(|| format!("failed to find packed struct end at byte {start}"))?;
        let after_brace = &source[close_brace + 1..];
        let Some((name, consumed)) = parse_struct_name(after_brace) else {
            cursor = close_brace + 1;
            continue;
        };

        structs.push(PackedStructSpec {
            name,
            byte_size: None,
            fields: parse_struct_fields(&source[open_brace + 1..close_brace])?,
        });

        cursor = close_brace + 1 + consumed;
    }

    resolve_struct_layouts(&mut structs);

    Ok(structs)
}

/// Parses the typedef name after a packed struct body.
fn parse_struct_name(input: &str) -> Option<(String, usize)> {
    let trimmed = input.trim_start();
    let skipped = input.len() - trimmed.len();
    let name = identifier().parse(trimmed).ok()?;
    let suffix = &trimmed[name.len()..];
    let rest = suffix.trim_start();
    let after_semicolon = rest.strip_prefix(';')?;
    let consumed = skipped + name.len() + (suffix.len() - after_semicolon.len());
    Some((name, consumed))
}

/// Parses fields and field docs from the body of one packed struct.
fn parse_struct_fields(body: &str) -> Result<Vec<StructFieldSpec>> {
    let mut fields = Vec::new();
    let mut pending_doc = None;
    let mut lines = body.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with("/**") {
            let mut doc_lines = Vec::new();
            if trimmed.ends_with("*/") {
                doc_lines.push(trimmed.to_owned());
            } else {
                doc_lines.push(trimmed.to_owned());
                for doc_line in lines.by_ref() {
                    let doc_trimmed = doc_line.trim();
                    doc_lines.push(doc_trimmed.to_owned());
                    if doc_trimmed.ends_with("*/") {
                        break;
                    }
                }
            }
            pending_doc = Some(parse_field_doc(&doc_lines));
            continue;
        }

        if let Some((c_type, name, array_len)) = parse_struct_field_line(trimmed) {
            fields.push(StructFieldSpec {
                wire: wire_type_for(Some(&c_type)),
                c_type,
                name,
                array_len,
                byte_offset: None,
                byte_size: None,
                doc: pending_doc.take(),
            });
        }
    }

    Ok(fields)
}

/// Resolves static byte offsets and sizes for structs whose fields are known.
fn resolve_struct_layouts(structs: &mut [PackedStructSpec]) {
    let mut sizes = HashMap::new();

    loop {
        let mut changed = false;

        for spec in structs.iter_mut() {
            if spec.byte_size.is_some() {
                continue;
            }

            let mut offset = 0usize;
            let mut resolved = true;

            for field in &mut spec.fields {
                let Some(byte_size) = resolve_field_size(field, &sizes) else {
                    field.byte_offset = None;
                    field.byte_size = None;
                    resolved = false;
                    break;
                };

                field.byte_offset = Some(offset);
                field.byte_size = Some(byte_size);
                offset += byte_size;
            }

            if resolved {
                spec.byte_size = Some(offset);
                sizes.insert(spec.name.clone(), offset);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }
}

/// Resolves one struct field size from its wire type and optional array length.
fn resolve_field_size(
    field: &StructFieldSpec,
    struct_sizes: &HashMap<String, usize>,
) -> Option<usize> {
    let element_size = match &field.wire {
        WireType::U8 => 1,
        WireType::U16Le => 2,
        WireType::U32Le => 4,
        WireType::Struct { name } => *struct_sizes.get(name)?,
        WireType::Bytes | WireType::Unknown { .. } => return None,
    };

    let array_len = match field.array_len.as_deref() {
        Some(len) => parse_static_array_len(len)?,
        None => 1,
    };

    Some(element_size * array_len)
}

/// Parses a literal array length such as `6`.
fn parse_static_array_len(len: &str) -> Option<usize> {
    len.trim().parse().ok()
}

/// Parses a field-level Doxygen block.
fn parse_field_doc(lines: &[String]) -> ParamDoc {
    let cleaned = lines
        .iter()
        .map(|line| {
            line.trim()
                .trim_start_matches("/**")
                .trim_end_matches("*/")
                .to_owned()
        })
        .map(|line| clean_doc_line(&line))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    parse_param_doc(&cleaned)
}

/// Parses one struct field declaration.
fn parse_struct_field_line(input: &str) -> Option<(String, String, Option<String>)> {
    just("const")
        .padded()
        .or_not()
        .then(identifier().padded())
        .then(just('*').padded().or_not())
        .then(identifier().padded())
        .then(
            none_of(']')
                .repeated()
                .collect::<String>()
                .map(|s| s.trim().to_owned())
                .delimited_by(just('['), just(']'))
                .or_not(),
        )
        .then_ignore(just(';').padded())
        .then_ignore(end())
        .map(|((((is_const, base), pointer), name), array_len)| {
            let mut c_type = String::new();
            if is_const.is_some() {
                c_type.push_str("const ");
            }
            c_type.push_str(&base);
            if pointer.is_some() {
                c_type.push('*');
            }
            (c_type, name, array_len)
        })
        .parse(input)
        .ok()
}
