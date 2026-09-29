//! The values each parameter and structure field may take, from the
//! `Values:` and `Flags:` lists of the generated headers' doc comments.
//!
//! Every list in the tagged headers has one of two shapes. In a function's
//! `@param` block, the header line is indented eight columns after the
//! comment's `*`, one `- ` item per value at the same indent, and label
//! continuations indented ten. In the comment of a structure field in
//! `ble_types.h`, the same lines are indented one and three. An item is a
//! value or a `first ... last` range, each value hexadecimal or decimal and
//! optionally followed by a parenthesized duration (`(20.000 ms)`) or note
//! (`(NaN)`), then an optional `: label`. Any other line inside a list, or a
//! list header anywhere else, is an error rather than a guess.

use std::collections::BTreeMap;

use clang::Entity;
use stm32wb_catalog::domain::parse_value;
use stm32wb_catalog::{Domain, DomainItem, DomainKind};

use crate::c::CRecord;

/// A documented member: its name, whether it is returned (`@param[out]`),
/// and its values.
pub type Documented = (String, bool, Domain);

/// A documented structure field: its structure, its name, and its values.
pub type FieldDocumented = (String, String, Domain);

const ITEM_INDENT: usize = 8;
const CONTINUATION_INDENT: usize = 10;
const FIELD_ITEM_INDENT: usize = 1;
const FIELD_CONTINUATION_INDENT: usize = 3;

/// The documented values of an entity's parameters.
pub fn domains(entity: Entity<'_>) -> Result<Vec<Documented>, String> {
    entity
        .get_comment()
        .map_or(Ok(Vec::new()), |comment| domains_in(&comment))
}

/// The documented values of every `@param` block of a doc comment.
pub fn domains_in(comment: &str) -> Result<Vec<Documented>, String> {
    let lines = comment_lines(comment);
    let mut documented = Vec::new();
    let mut param: Option<(String, bool)> = None;
    let mut list: Option<(DomainKind, Vec<Item>)> = None;
    let mut finish = |param: &Option<(String, bool)>,
                      list: &mut Option<(DomainKind, Vec<Item>)>|
     -> Result<(), String> {
        if let (Some((name, returned)), Some((kind, items))) = (param, list.take()) {
            let domain = domain(kind, items).map_err(|error| format!("{name}: {error}"))?;
            documented.push((name.clone(), *returned, domain));
        }
        Ok(())
    };
    for (indent, content) in lines {
        if let Some(rest) = content.strip_prefix('@') {
            finish(&param, &mut list)?;
            param = match rest.split_once(char::is_whitespace) {
                Some(("param", rest)) => Some((first_word(rest)?, false)),
                Some(("param[out]", rest)) => Some((first_word(rest)?, true)),
                _ => None,
            };
            continue;
        }
        let Some((name, _)) = &param else {
            if list_kind(content).is_some() {
                return Err(format!("a list outside any @param block: {content:?}"));
            }
            continue;
        };
        if let Some(kind) = list_kind(content) {
            if indent != ITEM_INDENT {
                return Err(format!(
                    "{name} starts a list indented {indent} rather than {ITEM_INDENT}"
                ));
            }
            if list.is_some() {
                return Err(format!("{name} documents two lists"));
            }
            list = Some((kind, Vec::new()));
            continue;
        }
        let Some((_, items)) = &mut list else {
            continue;
        };
        list_line(
            name,
            items,
            indent,
            content,
            ITEM_INDENT,
            CONTINUATION_INDENT,
        )?;
    }
    finish(&param, &mut list)?;
    Ok(documented)
}

/// The documented values of the fields of every structure in `records`
/// whose comments list them.
pub fn field_domains(records: &BTreeMap<String, CRecord>) -> Result<Vec<FieldDocumented>, String> {
    let mut documented = Vec::new();
    for (structure, record) in records {
        for field in &record.fields {
            let Some(comment) = &field.comment else {
                continue;
            };
            let name = format!("{structure}.{}", field.name);
            if let Some(domain) = field_domain(&name, comment)? {
                documented.push((structure.clone(), field.name.clone(), domain));
            }
        }
    }
    Ok(documented)
}

/// The values the comment of the structure field `name` lists, if any.
pub fn field_domain(name: &str, comment: &str) -> Result<Option<Domain>, String> {
    let mut list: Option<(DomainKind, Vec<Item>)> = None;
    for (indent, content) in comment_lines(comment) {
        if let Some(kind) = list_kind(content) {
            if indent != FIELD_ITEM_INDENT {
                return Err(format!(
                    "{name} starts a list indented {indent} rather than {FIELD_ITEM_INDENT}"
                ));
            }
            if list.is_some() {
                return Err(format!("{name} documents two lists"));
            }
            list = Some((kind, Vec::new()));
            continue;
        }
        if let Some((_, items)) = &mut list {
            list_line(
                name,
                items,
                indent,
                content,
                FIELD_ITEM_INDENT,
                FIELD_CONTINUATION_INDENT,
            )?;
        }
    }
    list.map(|(kind, items)| domain(kind, items).map_err(|error| format!("{name}: {error}")))
        .transpose()
}

/// Each line of a doc comment after its `*`, with the spaces indenting it.
fn comment_lines(comment: &str) -> impl Iterator<Item = (usize, &str)> {
    comment.lines().filter_map(|line| {
        let rest = line.trim_start().strip_prefix('*')?;
        if rest.starts_with('/') {
            return None;
        }
        let content = rest.trim_start_matches(' ');
        Some((rest.len() - content.len(), content.trim_end()))
    })
}

/// The kind of list a line starts, if it is a list header.
fn list_kind(content: &str) -> Option<DomainKind> {
    match content {
        "Values:" => Some(DomainKind::Values),
        "Flags:" => Some(DomainKind::Flags),
        _ => None,
    }
}

/// Add one line inside the list of `name` to its items: an item, a label
/// continuation, or a blank line.
fn list_line(
    name: &str,
    items: &mut Vec<Item>,
    indent: usize,
    content: &str,
    item_indent: usize,
    continuation_indent: usize,
) -> Result<(), String> {
    match content.strip_prefix("- ") {
        None if content.is_empty() => {}
        Some(item) if indent == item_indent => {
            items.push(parse_item(item).map_err(|error| format!("{name}: {error}"))?)
        }
        None if indent == continuation_indent => match items.last_mut() {
            Some(item) => item.continue_label(content),
            None => return Err(format!("{name} continues a label before any item")),
        },
        _ => {
            return Err(format!(
                "{name} has a line inside its list that is not an item: {content:?}"
            ));
        }
    }
    Ok(())
}

fn first_word(text: &str) -> Result<String, String> {
    text.split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| "a @param without a name".to_owned())
}

/// One list item as written, before its durations become a unit.
struct Item {
    first: Value,
    last: Option<Value>,
    label: String,
}

/// A value as written, with its parenthesized duration or note.
struct Value {
    value: i64,
    hex_digits: Option<u8>,
    note: Option<String>,
}

impl Item {
    fn continue_label(&mut self, text: &str) {
        if !self.label.is_empty() {
            self.label.push(' ');
        }
        self.label.push_str(text);
    }

    fn values(&self) -> impl Iterator<Item = &Value> {
        std::iter::once(&self.first).chain(&self.last)
    }
}

/// `0x0020 (20.000 ms)  ... 0x4000 (10240.000 ms) : for Low Duty Cycle`
fn parse_item(text: &str) -> Result<Item, String> {
    let (first, rest) = parse_value_and_note(text)?;
    let (last, rest) = match rest.trim_start().strip_prefix("...") {
        Some(rest) => {
            let (last, rest) = parse_value_and_note(rest.trim_start())?;
            (Some(last), rest)
        }
        None => (None, rest),
    };
    let rest = rest.trim_start();
    let label = match rest.strip_prefix(':') {
        Some(label) => label.trim().to_owned(),
        None if rest.is_empty() => String::new(),
        None => return Err(format!("unexpected {rest:?} in item {text:?}")),
    };
    Ok(Item { first, last, label })
}

fn parse_value_and_note(text: &str) -> Result<(Value, &str), String> {
    let end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .unwrap_or(text.len());
    let (word, rest) = text.split_at(end);
    // Headers spell hexadecimal digits in either case; the catalog uppercases.
    let normalized = match word.strip_prefix("0x") {
        Some(digits) => format!("0x{}", digits.to_ascii_uppercase()),
        None => word.to_owned(),
    };
    let (value, hex_digits) =
        parse_value(&normalized).ok_or_else(|| format!("{word:?} is not a value"))?;
    let trimmed = rest.trim_start();
    let (note, rest) = match trimmed.strip_prefix('(') {
        Some(inner) => {
            let (note, rest) = inner
                .split_once(')')
                .ok_or_else(|| format!("unclosed note after {word}"))?;
            (Some(note.trim().to_owned()), rest)
        }
        None => (None, rest),
    };
    Ok((
        Value {
            value,
            hex_digits,
            note,
        },
        rest,
    ))
}

/// The catalog's view of a list: items in ascending order, labelled with the
/// text or else the note, and the unit every written duration agrees with.
fn domain(kind: DomainKind, items: Vec<Item>) -> Result<Domain, String> {
    let unit_us = unit_us(&items)?;
    let mut items = items
        .into_iter()
        .map(|item| {
            let note = item
                .values()
                .filter_map(|value| value.note.as_deref())
                .find(|note| duration_ms(note).is_none())
                .map(str::to_owned);
            let label = Some(item.label).filter(|label| !label.is_empty()).or(note);
            DomainItem {
                first: item.first.value,
                last: item
                    .last
                    .as_ref()
                    .map_or(item.first.value, |last| last.value),
                hex_digits: item.first.hex_digits,
                label,
            }
        })
        .collect::<Vec<_>>();
    items.sort();
    Ok(Domain {
        kind,
        items,
        unit_us,
    })
}

/// The milliseconds a note like `20.000 ms` writes, with its decimals.
fn duration_ms(note: &str) -> Option<(&str, usize)> {
    let number = note.strip_suffix(" ms")?;
    let decimals = number
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    number
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.')
        .then_some((number, decimals))
}

/// The one unit, in whole microseconds, that every written duration rounds
/// to: taken from the largest value, whose duration is the most precise, and
/// checked against every other.
fn unit_us(items: &[Item]) -> Result<Option<u32>, String> {
    let durations = items
        .iter()
        .flat_map(Item::values)
        .filter_map(|value| {
            let (number, decimals) = duration_ms(value.note.as_deref()?)?;
            Some((value.value, number, decimals))
        })
        .collect::<Vec<_>>();
    let Some(&(largest, number, _)) = durations
        .iter()
        .filter(|(value, ..)| *value > 0)
        .max_by_key(|(value, ..)| *value)
    else {
        return Ok(None);
    };
    let unit = number
        .parse::<f64>()
        .map(|ms| (ms * 1000.0 / largest as f64).round())
        .map_err(|error| format!("duration {number} ms: {error}"))?;
    if !(1.0..=f64::from(u32::MAX)).contains(&unit) {
        return Err(format!("{number} ms for {largest} gives no whole unit"));
    }
    for &(value, number, decimals) in &durations {
        let expected = format!("{:.decimals$}", value as f64 * unit / 1000.0);
        if expected != number {
            return Err(format!(
                "{value} is written as {number} ms, but a {unit} us unit gives {expected} ms"
            ));
        }
    }
    Ok(Some(unit as u32))
}
