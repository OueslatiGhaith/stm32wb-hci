//! Parser for command payload writes in generated ST function bodies.
//!
//! Payload fields are inferred from assignments to `cpN->field` and generated
//! `Osal_MemCpy((void*) &cpN->field, ...)` calls. The function body is split
//! into C statements first, so generated writes may span multiple source lines.

use super::common::{decimal_digits, identifier, split_statements};
use super::docs::CommandDocs;
use crate::spec::{PayloadField, WireType, wire_type_for};
use anyhow::Result;
use chumsky::prelude::*;
use std::collections::HashMap;

/// Parses command payload fields from a generated C function body.
pub(super) fn parse_payload(
    body: &str,
    param_types: &HashMap<String, String>,
    doc: Option<&CommandDocs>,
) -> Result<Vec<PayloadField>> {
    let mut payload = Vec::new();

    for statement in split_statements(body) {
        let statement = statement.trim();
        if let Some(field) = parse_cp_assignment(statement) {
            let c_type = param_types.get(&field).cloned();
            payload.push(PayloadField {
                wire: wire_type_for(c_type.as_deref()),
                doc: doc.and_then(|d| d.params.get(&field)).cloned(),
                name: field,
                c_type,
                len: None,
                resolved: None,
            });
            continue;
        }

        if let Some((field, src, len)) = parse_memcpy(statement) {
            payload.push(PayloadField {
                name: field.clone(),
                c_type: param_types.get(&src).cloned(),
                wire: WireType::Bytes,
                len: Some(len),
                resolved: None,
                doc: doc
                    .and_then(|d| d.params.get(&field).or_else(|| d.params.get(&src)))
                    .cloned(),
            });
        }
    }

    Ok(payload)
}

/// Parses scalar payload writes such as `cp0->Conn_Handle = Conn_Handle;`.
fn parse_cp_assignment(input: &str) -> Option<String> {
    just("cp")
        .ignore_then(decimal_digits())
        .ignore_then(just("->"))
        .ignore_then(identifier())
        .then_ignore(just('=').padded())
        .then_ignore(any().repeated())
        .then_ignore(end())
        .parse(input)
        .ok()
}

/// Parses generated `Osal_MemCpy` payload writes.
fn parse_memcpy(input: &str) -> Option<(String, String, String)> {
    just("Osal_MemCpy")
        .padded()
        .ignore_then(just('(').padded())
        .ignore_then(just("(void*)").padded())
        .ignore_then(just('&').padded())
        .ignore_then(just("cp"))
        .ignore_then(decimal_digits())
        .ignore_then(just("->"))
        .ignore_then(identifier())
        .then_ignore(just(',').padded())
        .then_ignore(just("(const void*)").padded())
        .then(identifier())
        .then_ignore(just(',').padded())
        .then(any().repeated().collect::<String>().map(clean_memcpy_len))
        .then_ignore(end())
        .map(|((field, src), len)| (field, src, len))
        .parse(input)
        .ok()
}

/// Normalizes the captured memcpy length expression.
fn clean_memcpy_len(len: String) -> String {
    len.trim()
        .strip_suffix(';')
        .unwrap_or(len.trim())
        .trim()
        .strip_suffix(')')
        .unwrap_or_else(|| len.trim().strip_suffix(';').unwrap_or(len.trim()).trim())
        .trim()
        .to_owned()
}
