//! Parser for generated ST C command functions.
//!
//! This module extracts request header assignments such as `rq.ogf`, `rq.ocf`,
//! `rq.event`, and `rq.rlen`. Function boundary discovery is delegated to the
//! shared comment-aware C cursor.

use super::common::{
    CFunction, decimal_digits, find_function_definitions, hex_literal, identifier,
};
use anyhow::Result;
use chumsky::prelude::*;

/// Generated C command function with its name, raw signature, and body.
pub(super) type Function = CFunction;

/// Splits a generated C source file into `tBleStatus` function definitions.
pub(super) fn split_functions(source: &str) -> Result<Vec<Function>> {
    find_function_definitions(source, "tBleStatus")
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

#[derive(Clone, Copy)]
enum IntegerBase {
    Hex,
    Decimal,
}

/// Finds the first `rq.<field> = <integer>` assignment in a function body.
fn parse_rq_assignment(body: &str, field: &str, base: IntegerBase) -> Option<Result<u64>> {
    super::common::split_statements(body)
        .iter()
        .map(|statement| statement.trim())
        .find_map(|line| {
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
