//! System (SHCI) events from the tagged `shci.h`.
//!
//! Event codes are the values of `SHCI_SUB_EVT_CODE_t`, evaluated by clang.
//! `shci.h` documents each payload structure with a comment, directly above
//! it, whose first line is the event's enumerator name; that attachment links
//! a code to its packed structure. An event whose
//! enumerator documents no structure is left unresolved rather than assumed
//! to be empty.

use std::collections::BTreeMap;

use clang::{EntityKind, TranslationUnit};
use stm32wb_catalog::{EventScope, Field, Layout, Structs};

use crate::c::{self, CRecord};
use crate::events::ExtractedEvent;

const CODES: &str = "SHCI_SUB_EVT_CODE_t";

pub fn extract(
    unit: &TranslationUnit<'_>,
    records: &BTreeMap<String, CRecord>,
) -> Result<Vec<ExtractedEvent>, String> {
    let entities = unit.get_entity().get_children();
    let codes = entities
        .iter()
        .find(|entity| {
            entity.get_kind() == EntityKind::TypedefDecl
                && entity.get_name().as_deref() == Some(CODES)
        })
        .and_then(|typedef| typedef.get_typedef_underlying_type())
        .and_then(|ty| ty.get_canonical_type().get_declaration())
        .filter(|declaration| declaration.get_kind() == EntityKind::EnumDecl)
        .ok_or_else(|| format!("{CODES} is not declared as an enum"))?;

    // Typedef name -> the enumerator its documentation comment names.
    let mut documented: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entity in &entities {
        if entity.get_kind() != EntityKind::TypedefDecl {
            continue;
        }
        let (Some(name), Some(comment)) = (entity.get_name(), entity.get_comment()) else {
            continue;
        };
        // clang attaches the nearest preceding documentation comment even
        // across intervening comments; only a comment ending on the line
        // before the declaration documents it.
        let comment_end = entity
            .get_comment_range()
            .map(|range| range.get_end().get_spelling_location().line);
        let declaration_start = entity
            .get_range()
            .map(|range| range.get_start().get_spelling_location().line);
        if comment_end
            .zip(declaration_start)
            .is_none_or(|(end, start)| end + 1 != start)
        {
            continue;
        }
        if let Some(first) = comment
            .lines()
            .map(|line| {
                line.trim()
                    .trim_start_matches("/**")
                    .trim_start_matches('*')
                    .trim()
            })
            .find(|line| !line.is_empty())
        {
            documented.entry(first.to_owned()).or_default().push(name);
        }
    }

    let mut events = Vec::new();
    for constant in codes.get_children() {
        if constant.get_kind() != EntityKind::EnumConstantDecl {
            continue;
        }
        let name = constant
            .get_name()
            .ok_or("an SHCI event code has no name")?;
        let code = constant
            .get_enum_constant_value()
            .and_then(|(value, _)| u16::try_from(value).ok())
            .ok_or_else(|| format!("{name} is not a 16-bit code"))?;
        let mut structs = Structs::new();
        let payload = match documented.get(&name).map(Vec::as_slice) {
            None | Some([]) => {
                Layout::Unresolved(format!("shci.h documents no payload structure for {name}"))
            }
            Some([record_name]) => {
                payload(record_name, records, &mut structs).unwrap_or_else(|reason| {
                    structs.clear();
                    Layout::Unresolved(reason)
                })
            }
            Some(several) => {
                return Err(format!("{name} documents several structures: {several:?}"));
            }
        };
        events.push(ExtractedEvent {
            scope: EventScope::System,
            code,
            name,
            payload,
            structs,
        });
    }
    if events.is_empty() {
        return Err(format!("{CODES} has no enumerators"));
    }
    Ok(events)
}

fn payload(
    record_name: &str,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Layout, String> {
    let record = records
        .get(record_name)
        .ok_or_else(|| format!("{record_name} is not a structure"))?;
    if record.union || !record.packed {
        return Err(format!("{record_name} is not a packed structure"));
    }
    let fields = record
        .fields
        .iter()
        .map(|field| {
            c::fixed_field(field, records, structs)
                .map(|ty| Field::new(field.name.clone(), ty))
                .map_err(|error| format!("{record_name}.{error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Layout::Fields(fields))
}
