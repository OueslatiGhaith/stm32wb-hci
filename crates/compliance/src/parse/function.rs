//! Parser for generated ST C command functions.
//!
//! This module extracts function boundaries and request header assignments such
//! as `rq.ogf`, `rq.ocf`, `rq.event`, and `rq.rlen`.

use super::common::{decimal_digits, find_matching_brace, hex_literal, identifier};
use anyhow::{Context, Result};
use chumsky::prelude::*;

/// Generated C command function with its name, raw signature, and body.
pub(super) struct Function {
    pub(super) name: String,
    pub(super) signature: String,
    pub(super) body: String,
}

/// Splits a generated C source file into `tBleStatus` function definitions.
pub(super) fn split_functions(source: &str) -> Result<Vec<Function>> {
    let mut functions = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = source[cursor..].find("tBleStatus") {
        let start = cursor + relative_start;
        let Some(open_brace) = source[start..].find('{').map(|idx| start + idx) else {
            break;
        };
        let Some((name, signature)) = parse_function_header(&source[start..open_brace]) else {
            cursor = open_brace + 1;
            continue;
        };

        let body_start = open_brace + 1;
        let body_end = find_matching_brace(source, body_start - 1)
            .with_context(|| format!("failed to find end of {name}"))?;

        functions.push(Function {
            name,
            signature,
            body: source[body_start..body_end].to_owned(),
        });

        cursor = body_end + 1;
    }

    Ok(functions)
}

/// Parses a hexadecimal request assignment from a function body.
pub(super) fn parse_hex_assignment(body: &str, field: &str) -> Result<Option<u16>> {
    parse_rq_assignment(body, field, IntegerBase::Hex)
        .map(|value| Ok(value? as u16))
        .transpose()
}

/// Parses a decimal request assignment from a function body.
pub(super) fn parse_decimal_assignment(body: &str, field: &str) -> Result<Option<usize>> {
    parse_rq_assignment(body, field, IntegerBase::Decimal)
        .map(|value| Ok(value? as usize))
        .transpose()
}

/// Parses a `tBleStatus name(args)` function header.
fn parse_function_header(input: &str) -> Option<(String, String)> {
    just("tBleStatus")
        .padded()
        .ignore_then(identifier().padded())
        .then(
            none_of(')')
                .repeated()
                .collect::<String>()
                .delimited_by(just('('), just(')'))
                .padded(),
        )
        .then_ignore(end())
        .parse(input)
        .ok()
}

#[derive(Clone, Copy)]
enum IntegerBase {
    Hex,
    Decimal,
}

/// Finds the first `rq.<field> = <integer>` assignment in a function body.
fn parse_rq_assignment(body: &str, field: &str, base: IntegerBase) -> Option<Result<u64>> {
    body.lines().map(str::trim).find_map(|line| {
        let parsed = match base {
            IntegerBase::Hex => parse_rq_hex_line(line),
            IntegerBase::Decimal => parse_rq_decimal_line(line),
        }?;

        (parsed.0 == field).then_some(Ok(parsed.1))
    })
}

/// Parses a hexadecimal `rq.<field> = ...` line.
fn parse_rq_hex_line(input: &str) -> Option<(String, u64)> {
    rq_assignment_prefix()
        .then(hex_literal())
        .then_ignore(any().repeated())
        .then_ignore(end())
        .parse(input)
        .ok()
}

/// Parses a decimal `rq.<field> = ...` line.
fn parse_rq_decimal_line(input: &str) -> Option<(String, u64)> {
    rq_assignment_prefix()
        .then(decimal_digits().try_map(|digits, span| {
            digits
                .parse::<u64>()
                .map_err(|err| Simple::custom(span, err.to_string()))
        }))
        .then_ignore(any().repeated())
        .then_ignore(end())
        .parse(input)
        .ok()
}

/// Parses the `rq.<field> =` prefix and returns the field name.
fn rq_assignment_prefix() -> impl Parser<char, String, Error = Simple<char>> {
    just("rq.")
        .ignore_then(identifier())
        .then_ignore(just('=').padded())
}
