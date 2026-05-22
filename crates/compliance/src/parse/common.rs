//! Shared parser primitives for the ST source parsers.
//!
//! These helpers wrap the small `chumsky` parsers used across multiple modules
//! and keep generated-source assumptions in one place. They also provide a
//! narrow C source cursor for finding generated functions, prototypes, structs,
//! and statements while ignoring comments and string literals.

use anyhow::{Context, Result};
use chumsky::prelude::*;

/// Function definition extracted from generated C source.
pub(super) struct CFunction {
    /// C function name.
    pub(super) name: String,
    /// Raw parameter list between the function parentheses.
    pub(super) signature: String,
    /// Raw function body between the outer braces.
    pub(super) body: String,
}

/// Function prototype extracted from generated C headers.
pub(super) struct CPrototype {
    /// C function name.
    pub(super) name: String,
    /// Byte offset where the prototype return type starts.
    pub(super) start: usize,
}

/// Packed struct typedef extracted from generated C headers.
pub(super) struct CPackedStruct {
    /// Typedef name following the packed struct body.
    pub(super) name: String,
    /// Raw body between the outer braces.
    pub(super) body: String,
}

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

/// Finds generated function definitions with the given return type.
pub(super) fn find_function_definitions(source: &str, return_type: &str) -> Result<Vec<CFunction>> {
    let masked = mask_comments_and_strings(source);
    let mut functions = Vec::new();
    let mut cursor = 0;

    while let Some(start) = find_keyword(&masked, return_type, cursor) {
        let Some((name, open_paren, close_paren)) =
            parse_function_head(&masked, start, return_type.len())
        else {
            cursor = start + return_type.len();
            continue;
        };

        let after_paren = skip_whitespace(&masked, close_paren + 1);
        if !masked[after_paren..].starts_with('{') {
            cursor = close_paren + 1;
            continue;
        }

        let close_brace = find_matching_delimiter(&masked, after_paren, b'{', b'}')
            .with_context(|| format!("failed to find end of {name}"))?;
        functions.push(CFunction {
            name,
            signature: source[open_paren + 1..close_paren].to_owned(),
            body: source[after_paren + 1..close_brace].to_owned(),
        });
        cursor = close_brace + 1;
    }

    Ok(functions)
}

/// Finds generated function prototypes with the given return type.
pub(super) fn find_function_prototypes(source: &str, return_type: &str) -> Vec<CPrototype> {
    let masked = mask_comments_and_strings(source);
    let mut prototypes = Vec::new();
    let mut cursor = 0;

    while let Some(start) = find_keyword(&masked, return_type, cursor) {
        let Some((name, _, close_paren)) = parse_function_head(&masked, start, return_type.len())
        else {
            cursor = start + return_type.len();
            continue;
        };

        let after_paren = skip_whitespace(&masked, close_paren + 1);
        if masked[after_paren..].starts_with(';') {
            prototypes.push(CPrototype { name, start });
        }
        cursor = close_paren + 1;
    }

    prototypes
}

/// Finds `typedef __PACKED_STRUCT { ... } Name;` declarations.
pub(super) fn find_packed_structs(source: &str) -> Result<Vec<CPackedStruct>> {
    const TYPEDEF: &str = "typedef";
    const PACKED_STRUCT: &str = "__PACKED_STRUCT";

    let masked = mask_comments_and_strings(source);
    let mut structs = Vec::new();
    let mut cursor = 0;

    while let Some(start) = find_keyword(&masked, TYPEDEF, cursor) {
        let after_typedef = skip_whitespace(&masked, start + TYPEDEF.len());
        if !masked[after_typedef..].starts_with(PACKED_STRUCT) {
            cursor = after_typedef;
            continue;
        }

        let after_macro = skip_whitespace(&masked, after_typedef + PACKED_STRUCT.len());
        if !masked[after_macro..].starts_with('{') {
            cursor = after_macro;
            continue;
        }

        let close_brace = find_matching_delimiter(&masked, after_macro, b'{', b'}')
            .with_context(|| format!("failed to find packed struct end at byte {start}"))?;
        let after_brace = skip_whitespace(&masked, close_brace + 1);
        let Some((name, after_name)) = parse_identifier_at(&masked, after_brace) else {
            cursor = close_brace + 1;
            continue;
        };
        let after_name = skip_whitespace(&masked, after_name);
        if !masked[after_name..].starts_with(';') {
            cursor = after_name;
            continue;
        }

        structs.push(CPackedStruct {
            name,
            body: source[after_macro + 1..close_brace].to_owned(),
        });
        cursor = after_name + 1;
    }

    Ok(structs)
}

/// Splits a C body into semicolon-terminated statements outside parentheses.
///
/// Block braces reset statement starts so assignments inside `if` bodies are
/// parsed as their own statements rather than being prefixed by the block head.
pub(super) fn split_statements(source: &str) -> Vec<String> {
    let masked = mask_comments_and_strings(source);
    let mut statements = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;

    for (idx, byte) in masked.bytes().enumerate() {
        match byte {
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            b'{' | b'}' if paren_depth == 0 && bracket_depth == 0 => {
                start = idx + 1;
            }
            b';' if paren_depth == 0 && bracket_depth == 0 => {
                let statement = source[start..=idx].trim();
                if !statement.is_empty() {
                    statements.push(statement.to_owned());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }

    statements
}

/// Replaces comments and string/character literals with spaces, preserving byte
/// length and newlines so offsets remain valid in the original source.
fn mask_comments_and_strings(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut masked = Vec::with_capacity(bytes.len());
    let mut idx = 0;

    while idx < bytes.len() {
        match bytes[idx] {
            b'/' if bytes.get(idx + 1) == Some(&b'/') => {
                masked.push(b' ');
                masked.push(b' ');
                idx += 2;
                while idx < bytes.len() && bytes[idx] != b'\n' {
                    masked.push(b' ');
                    idx += 1;
                }
            }
            b'/' if bytes.get(idx + 1) == Some(&b'*') => {
                masked.push(b' ');
                masked.push(b' ');
                idx += 2;
                while idx < bytes.len() {
                    if bytes[idx] == b'*' && bytes.get(idx + 1) == Some(&b'/') {
                        masked.push(b' ');
                        masked.push(b' ');
                        idx += 2;
                        break;
                    }
                    masked.push(if bytes[idx] == b'\n' { b'\n' } else { b' ' });
                    idx += 1;
                }
            }
            b'"' | b'\'' => {
                let quote = bytes[idx];
                masked.push(b' ');
                idx += 1;
                while idx < bytes.len() {
                    let byte = bytes[idx];
                    masked.push(if byte == b'\n' { b'\n' } else { b' ' });
                    idx += 1;
                    if byte == b'\\' && idx < bytes.len() {
                        masked.push(if bytes[idx] == b'\n' { b'\n' } else { b' ' });
                        idx += 1;
                        continue;
                    }
                    if byte == quote {
                        break;
                    }
                }
            }
            byte => {
                masked.push(byte);
                idx += 1;
            }
        }
    }

    String::from_utf8(masked).expect("source masking preserves UTF-8 for ASCII C sources")
}

/// Finds a keyword occurrence with C identifier boundaries.
fn find_keyword(source: &str, keyword: &str, start: usize) -> Option<usize> {
    let mut cursor = start;
    while let Some(relative) = source[cursor..].find(keyword) {
        let idx = cursor + relative;
        let before = idx
            .checked_sub(1)
            .and_then(|idx| source.as_bytes().get(idx))
            .copied();
        let after = source.as_bytes().get(idx + keyword.len()).copied();
        if before.is_none_or(|byte| !is_ident_byte(byte))
            && after.is_none_or(|byte| !is_ident_byte(byte))
        {
            return Some(idx);
        }
        cursor = idx + keyword.len();
    }

    None
}

/// Parses `<return_type> name(...)` after a known return-type offset.
fn parse_function_head(
    source: &str,
    start: usize,
    return_type_len: usize,
) -> Option<(String, usize, usize)> {
    let name_start = skip_whitespace(source, start + return_type_len);
    let (name, after_name) = parse_identifier_at(source, name_start)?;
    let open_paren = skip_whitespace(source, after_name);
    if !source[open_paren..].starts_with('(') {
        return None;
    }
    let close_paren = find_matching_delimiter(source, open_paren, b'(', b')')?;
    Some((name, open_paren, close_paren))
}

/// Parses a C identifier at a byte offset.
fn parse_identifier_at(source: &str, start: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    let first = *bytes.get(start)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }

    let mut end = start + 1;
    while bytes.get(end).is_some_and(|byte| is_ident_byte(*byte)) {
        end += 1;
    }

    Some((source[start..end].to_owned(), end))
}

/// Skips ASCII whitespace from a byte offset.
fn skip_whitespace(source: &str, start: usize) -> usize {
    let mut idx = start;
    while source
        .as_bytes()
        .get(idx)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        idx += 1;
    }
    idx
}

/// Finds a matching delimiter in already-masked source text.
fn find_matching_delimiter(
    source: &str,
    open_delimiter: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, byte) in source.bytes().enumerate().skip(open_delimiter) {
        match byte {
            byte if byte == open => depth += 1,
            byte if byte == close => {
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

/// Returns whether a byte can continue a C identifier.
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Parses one or more hexadecimal digits as a string.
fn hex_digits() -> impl Parser<char, String, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_hexdigit())
        .repeated()
        .at_least(1)
        .collect()
}
