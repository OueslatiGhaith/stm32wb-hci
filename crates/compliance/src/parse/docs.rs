//! Parser for generated Doxygen command and parameter documentation.
//!
//! The extracted docs provide formal ST command names, descriptions, enum-like
//! value lists, ranges, and unit scaling metadata for command parameters.

use super::common::{
    clean_doc_line, decimal_number, find_function_prototypes, hex_literal, identifier,
    parse_number_prefix, whitespace1,
};
use crate::spec::{CommandDoc, Constraints, ParamDoc, RangeDoc, UnitDoc, ValueDoc};
use anyhow::Result;
use chumsky::prelude::*;
use std::collections::HashMap;

/// Documentation for one generated command prototype.
pub(super) struct CommandDocs {
    pub(super) command: CommandDoc,
    pub(super) params: HashMap<String, ParamDoc>,
}

/// Parses command docs from a generated ST header file.
pub(super) fn parse_command_docs(header: &str) -> Result<HashMap<String, CommandDocs>> {
    parse_function_docs(header, "tBleStatus")
}

/// Parses docs attached to generated C function prototypes with the given return type.
pub(super) fn parse_function_docs(
    header: &str,
    return_type: &str,
) -> Result<HashMap<String, CommandDocs>> {
    let mut docs = HashMap::new();

    for prototype in find_function_prototypes(header, return_type) {
        let Some((doc_start, doc_end)) = previous_doc_block(header, prototype.start) else {
            continue;
        };

        docs.insert(
            prototype.name,
            parse_doc_block(&header[doc_start + 3..doc_end])?,
        );
    }

    Ok(docs)
}

/// Parses cleaned documentation lines for a parameter or struct field.
pub(super) fn parse_param_doc(lines: &[String]) -> ParamDoc {
    ParamDoc {
        description: lines.join(" "),
        values: parse_values(lines),
        constraints: parse_constraints(lines),
    }
}

/// Finds the Doxygen block immediately preceding a prototype.
fn previous_doc_block(source: &str, before: usize) -> Option<(usize, usize)> {
    let prefix = &source[..before];
    let doc_end = prefix.rfind("*/")?;
    if !source[doc_end + 2..before].trim().is_empty() {
        return None;
    }

    let doc_start = source[..doc_end].rfind("/**")?;
    Some((doc_start, doc_end))
}

/// Parses one raw Doxygen block into command and parameter docs.
fn parse_doc_block(raw: &str) -> Result<CommandDocs> {
    let lines = raw
        .lines()
        .map(clean_doc_line)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let brief = lines
        .iter()
        .find_map(|line| line.strip_prefix("@brief ").map(str::to_owned));

    let mut description = Vec::new();
    let mut params = HashMap::new();
    let mut current_param: Option<(String, Vec<String>)> = None;

    for line in lines {
        if let Some((name, text)) = parse_param_doc_start(&line) {
            if let Some((name, body)) = current_param.take() {
                params.insert(name, parse_param_doc(&body));
            }
            current_param = Some((name, vec![text]));
        } else if line.starts_with("@return") {
            if let Some((name, body)) = current_param.take() {
                params.insert(name, parse_param_doc(&body));
            }
        } else if let Some((_, body)) = current_param.as_mut() {
            body.push(line);
        } else if !line.starts_with("@brief ") {
            description.push(line);
        }
    }

    if let Some((name, body)) = current_param.take() {
        params.insert(name, parse_param_doc(&body));
    }

    Ok(CommandDocs {
        command: CommandDoc {
            brief,
            description: description.join(" "),
        },
        params,
    })
}

/// Parses `@param [in] Name ...` and returns the parameter name and first line.
fn parse_param_doc_start(input: &str) -> Option<(String, String)> {
    let bracket = none_of(']').repeated().delimited_by(just('['), just(']'));

    just("@param")
        .ignore_then(bracket.or_not())
        .ignore_then(whitespace1())
        .ignore_then(identifier())
        .then(
            any()
                .repeated()
                .collect::<String>()
                .map(|s| s.trim().to_owned()),
        )
        .then_ignore(end())
        .parse(input)
        .ok()
}

/// Parses range and unit constraints from documentation lines.
fn parse_constraints(lines: &[String]) -> Constraints {
    Constraints {
        ranges: parse_ranges(lines),
        unit: lines.iter().find_map(|line| parse_unit_line(line.trim())),
    }
}

/// Parses bullet values such as `- 0x01: Enabled`.
fn parse_values(lines: &[String]) -> Vec<ValueDoc> {
    lines
        .iter()
        .filter_map(|line| {
            let (value, text) = parse_value_line(line.trim())?;
            let raw = line.trim().to_owned();
            let (label, description) = split_label_description(text.as_deref());
            Some(ValueDoc {
                value,
                raw,
                label,
                description,
            })
        })
        .collect()
}

/// Parses one bullet value line.
fn parse_value_line(input: &str) -> Option<(u64, Option<String>)> {
    just('-')
        .padded()
        .ignore_then(hex_literal())
        .then(
            just(':')
                .padded()
                .ignore_then(any().repeated().collect::<String>())
                .or_not(),
        )
        .then_ignore(end())
        .map(|(value, text)| {
            (
                value,
                text.map(|text| text.trim().to_owned())
                    .filter(|text| !text.is_empty()),
            )
        })
        .parse(input)
        .ok()
}

/// Parses range constraints from documentation lines.
fn parse_ranges(lines: &[String]) -> Vec<RangeDoc> {
    lines
        .iter()
        .filter_map(|line| parse_range_line(line.trim()))
        .collect()
}

/// Parses one `- min ... max[: description]` range line.
fn parse_range_line(input: &str) -> Option<RangeDoc> {
    let range = input.strip_prefix('-')?.trim();
    let (left, right) = range.split_once("...")?;
    let (right, description) = match right.split_once(':') {
        Some((right, description)) => (right, Some(description.trim().to_owned())),
        None => (right, None),
    };

    Some(RangeDoc {
        min: parse_number_prefix(left.trim())?,
        max: parse_number_prefix(right.trim())?,
        raw: input.to_owned(),
        description: description.filter(|description| !description.is_empty()),
    })
}

/// Parses unit scaling lines such as `Time = N * 0.625 ms.`.
fn parse_unit_line(input: &str) -> Option<UnitDoc> {
    just("Time")
        .padded()
        .ignore_then(just('=').padded())
        .ignore_then(identifier())
        .then_ignore(just('*').padded())
        .then(decimal_number())
        .then_ignore(whitespace1())
        .then(identifier())
        .then_ignore(any().repeated())
        .then_ignore(end())
        .map(|((variable, scale), unit)| UnitDoc {
            variable,
            scale,
            unit,
            raw: input.to_owned(),
        })
        .parse(input)
        .ok()
}

/// Splits `Label (description)` text when ST docs use that convention.
fn split_label_description(text: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(text) = text else {
        return (None, None);
    };

    if let Some((label, rest)) = text.split_once('(') {
        return (
            Some(label.trim().to_owned()),
            Some(rest.trim_end_matches(')').trim().to_owned()),
        );
    }

    (Some(text.to_owned()), None)
}
