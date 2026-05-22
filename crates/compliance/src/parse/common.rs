//! Shared parser primitives for the ST source parsers.
//!
//! These helpers wrap the small `chumsky` parsers used across multiple modules
//! and keep generated-source assumptions in one place.

use chumsky::prelude::*;

/// Removes Doxygen decorations from a single documentation line.
pub(super) fn clean_doc_line(line: &str) -> String {
    line.trim()
        .trim_start_matches('*')
        .trim()
        .trim_end_matches("<br>")
        .trim()
        .to_owned()
}

/// Parses the C identifier subset used by generated ST files.
pub(super) fn identifier() -> impl Parser<char, String, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
        .then(filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_').repeated())
        .map(|(first, rest)| {
            let mut ident = String::with_capacity(rest.len() + 1);
            ident.push(first);
            ident.extend(rest);
            ident
        })
}

/// Parses one or more decimal digits as a string.
pub(super) fn decimal_digits() -> impl Parser<char, String, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .at_least(1)
        .collect()
}

/// Parses a `0x...` integer literal.
pub(super) fn hex_literal() -> impl Parser<char, u64, Error = Simple<char>> {
    just("0x")
        .ignore_then(hex_digits())
        .try_map(|digits, span| {
            u64::from_str_radix(&digits, 16).map_err(|err| Simple::custom(span, err.to_string()))
        })
}

/// Parses a decimal or hexadecimal integer literal.
pub(super) fn number_literal() -> impl Parser<char, u64, Error = Simple<char>> {
    hex_literal().or(decimal_digits().try_map(|digits, span| {
        digits
            .parse::<u64>()
            .map_err(|err| Simple::custom(span, err.to_string()))
    }))
}

/// Parses the first integer literal in a string and ignores trailing text.
pub(super) fn parse_number_prefix(input: &str) -> Option<u64> {
    number_literal()
        .then_ignore(any().repeated())
        .then_ignore(end())
        .parse(input)
        .ok()
}

/// Parses a decimal number, preserving fractional text when present.
pub(super) fn decimal_number() -> impl Parser<char, String, Error = Simple<char>> {
    decimal_digits()
        .then(just('.').ignore_then(decimal_digits()).or_not())
        .map(|(whole, fraction)| match fraction {
            Some(fraction) => format!("{whole}.{fraction}"),
            None => whole,
        })
}

/// Parses at least one whitespace character.
pub(super) fn whitespace1() -> impl Parser<char, (), Error = Simple<char>> {
    filter(|c: &char| c.is_whitespace())
        .repeated()
        .at_least(1)
        .ignored()
}

/// Formats parser errors for `anyhow` messages.
pub(super) fn format_errors(errors: Vec<Simple<char>>) -> String {
    errors
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Finds the closing brace for a braced item using simple byte-level counting.
pub(super) fn find_matching_brace(source: &str, open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, byte) in source.bytes().enumerate().skip(open_brace) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parses one or more hexadecimal digits as a string.
fn hex_digits() -> impl Parser<char, String, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_hexdigit())
        .repeated()
        .at_least(1)
        .collect()
}
