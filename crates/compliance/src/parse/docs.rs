//! Parser for generated Doxygen command and parameter documentation.
//!
//! The extracted docs provide formal ST command names, descriptions, enum-like
//! value lists, ranges, and unit scaling metadata for command parameters.

use super::common::{
    clean_doc_line, decimal_number, find_function_prototypes, hex_literal, identifier,
    parse_number_prefix, whitespace1,
};
use crate::spec::{
    CommandDoc, Constraints, DeviceFamily, FamilyAvailability, FamilyMention, ParamDoc, RangeDoc,
    UnitDoc, ValueDoc,
};
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
    let doc = DocText::from_lines(lines);
    let facts = DocFacts::parse(&doc);
    let availability = parse_explicit_family_availability(doc.text_lines());
    let unclassified_family_mentions =
        unclassified_family_mentions(doc.text_lines(), &availability);

    ParamDoc {
        description: doc.description(),
        availability,
        unclassified_family_mentions,
        values: facts.values,
        constraints: Constraints {
            ranges: facts.ranges,
            unit: facts.unit,
        },
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
        command: command_doc(&brief, &description),
        params,
    })
}

fn command_doc(brief: &Option<String>, description: &[String]) -> CommandDoc {
    let doc = DocText::from_lines(description);
    let availability = parse_explicit_family_availability(doc.text_lines());
    let unclassified_family_mentions =
        unclassified_family_mentions(doc.text_lines(), &availability);

    CommandDoc {
        brief: brief.clone(),
        description: doc.description(),
        availability,
        unclassified_family_mentions,
    }
}

#[derive(Debug)]
struct DocText {
    lines: Vec<String>,
    entries: Vec<DocEntry>,
}

impl DocText {
    fn from_lines(lines: &[String]) -> Self {
        let mut entries = Vec::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with('-') {
                entries.push(DocEntry::Bullet(DocBullet {
                    raw: trimmed.to_owned(),
                }));
            } else if is_standalone_doc_line(trimmed) {
                entries.push(DocEntry::Text(line.clone()));
            } else if let Some(DocEntry::Bullet(previous)) = entries.last_mut()
                && is_bullet_continuation(trimmed, &previous.raw)
            {
                previous.raw.push(' ');
                previous.raw.push_str(trimmed);
            } else {
                entries.push(DocEntry::Text(line.clone()));
            }
        }

        Self {
            lines: lines.to_vec(),
            entries,
        }
    }

    fn description(&self) -> String {
        self.lines.join(" ")
    }

    fn text_lines(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(|entry| match entry {
            DocEntry::Text(line) => Some(line.as_str()),
            DocEntry::Bullet(_) => None,
        })
    }

    fn bullet_lines(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(|entry| match entry {
            DocEntry::Text(_) => None,
            DocEntry::Bullet(bullet) => Some(bullet.raw.as_str()),
        })
    }
}

fn is_standalone_doc_line(line: &str) -> bool {
    parse_unit_line(line).is_some()
}

fn is_bullet_continuation(line: &str, previous_bullet: &str) -> bool {
    line.chars().next().is_some_and(char::is_lowercase) || has_unclosed_parenthesis(previous_bullet)
}

fn has_unclosed_parenthesis(input: &str) -> bool {
    let mut depth = 0usize;
    for ch in input.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth > 0
}

#[derive(Debug)]
enum DocEntry {
    Text(String),
    Bullet(DocBullet),
}

#[derive(Debug)]
struct DocBullet {
    raw: String,
}

#[derive(Default, Debug)]
struct DocFacts {
    values: Vec<ValueDoc>,
    ranges: Vec<RangeDoc>,
    unit: Option<UnitDoc>,
}

impl DocFacts {
    fn parse(doc: &DocText) -> Self {
        Self {
            values: parse_values(doc),
            ranges: parse_ranges(doc),
            unit: doc
                .text_lines()
                .find_map(|line| parse_unit_line(line.trim())),
        }
    }
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

/// Parses bullet values such as `- 0x01: Enabled`.
fn parse_values(doc: &DocText) -> Vec<ValueDoc> {
    doc.bullet_lines()
        .filter_map(|line| {
            let (value, text) = parse_value_line(line.trim())?;
            let raw = line.trim().to_owned();
            let (label, description) = split_label_description(text.as_deref());
            let availability = parse_explicit_family_availability([line]);
            let unclassified_family_mentions = unclassified_family_mentions([line], &availability);
            Some(ValueDoc {
                value,
                raw,
                label,
                description,
                availability,
                unclassified_family_mentions,
            })
        })
        .collect()
}

/// Parses range constraints from bullet entries.
fn parse_ranges(doc: &DocText) -> Vec<RangeDoc> {
    doc.bullet_lines()
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

    let availability = parse_range_availability(input, description.as_deref());
    let unclassified_family_mentions = unclassified_family_mentions([input], &availability);

    Some(RangeDoc {
        min: parse_number_prefix(left.trim())?,
        max: parse_number_prefix(right.trim())?,
        raw: input.to_owned(),
        description: description
            .clone()
            .filter(|description| !description.is_empty()),
        availability,
        unclassified_family_mentions,
    })
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

/// Extracts explicit WB/WBA-only qualifiers from ST prose.
fn parse_explicit_family_availability<I, S>(lines: I) -> FamilyAvailability
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut only = Vec::new();
    let mut evidence = Vec::new();

    for line in lines {
        let line = line.as_ref();
        let tokens = family_tokens(line);
        if tokens_indicate_only_for(&tokens, "STM32WB") && !only.contains(&DeviceFamily::Stm32Wb) {
            only.push(DeviceFamily::Stm32Wb);
            evidence.push(FamilyMention {
                family: DeviceFamily::Stm32Wb,
                text: line.to_owned(),
            });
        }
        if tokens_indicate_only_for(&tokens, "STM32WBA") && !only.contains(&DeviceFamily::Stm32Wba)
        {
            only.push(DeviceFamily::Stm32Wba);
            evidence.push(FamilyMention {
                family: DeviceFamily::Stm32Wba,
                text: line.to_owned(),
            });
        }
    }

    only.sort();
    evidence.sort_by_key(|mention| mention.family);
    FamilyAvailability { only, evidence }
}

fn parse_range_availability(input: &str, description: Option<&str>) -> FamilyAvailability {
    let explicit = parse_explicit_family_availability([input]);
    if !explicit.is_empty() {
        return explicit;
    }

    parse_range_family_qualifier(description)
}

fn parse_range_family_qualifier(description: Option<&str>) -> FamilyAvailability {
    let Some(description) = description else {
        return FamilyAvailability::default();
    };

    let tokens = family_tokens(description);
    let families = family_mentions(&tokens);
    if families.len() != 1 {
        return FamilyAvailability::default();
    }

    if tokens.len() == 1 || tokens_end_with_family_with(&tokens, families[0]) {
        return FamilyAvailability {
            only: vec![families[0]],
            evidence: vec![FamilyMention {
                family: families[0],
                text: description.to_owned(),
            }],
        };
    }

    FamilyAvailability::default()
}

fn tokens_end_with_family_with(tokens: &[String], family: DeviceFamily) -> bool {
    let [.., with, token] = tokens else {
        return false;
    };

    with == "WITH" && token == family.token()
}

fn family_mentions(tokens: &[String]) -> Vec<DeviceFamily> {
    let mut families = Vec::new();

    for token in tokens {
        if family_token_matches(token, "STM32WB") && !families.contains(&DeviceFamily::Stm32Wb) {
            families.push(DeviceFamily::Stm32Wb);
        }
        if family_token_matches(token, "STM32WBA") && !families.contains(&DeviceFamily::Stm32Wba) {
            families.push(DeviceFamily::Stm32Wba);
        }
    }

    families.sort();
    families
}

fn unclassified_family_mentions<I, S>(
    lines: I,
    availability: &FamilyAvailability,
) -> Vec<FamilyMention>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut mentions = Vec::new();

    for line in lines {
        let line = line.as_ref();
        for family in family_mentions(&family_tokens(line)) {
            if availability.only.contains(&family)
                || mentions
                    .iter()
                    .any(|mention: &FamilyMention| mention.family == family && mention.text == line)
            {
                continue;
            }
            mentions.push(FamilyMention {
                family,
                text: line.to_owned(),
            });
        }
    }

    mentions
}

impl DeviceFamily {
    fn token(self) -> &'static str {
        match self {
            DeviceFamily::Stm32Wb => "STM32WB",
            DeviceFamily::Stm32Wba => "STM32WBA",
        }
    }
}

fn family_tokens(input: &str) -> Vec<String> {
    input
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_uppercase)
        .collect()
}

fn tokens_indicate_only_for(tokens: &[String], family: &str) -> bool {
    tokens.windows(3).any(|window| {
        window[0] == "ONLY" && window[1] == "FOR" && family_token_matches(&window[2], family)
    }) || tokens
        .windows(2)
        .any(|window| family_token_matches(&window[0], family) && window[1] == "ONLY")
}

fn family_token_matches(token: &str, family: &str) -> bool {
    token == family
}
