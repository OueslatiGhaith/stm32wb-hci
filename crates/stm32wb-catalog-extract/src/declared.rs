//! Layouts from ST's generated packed structures alone, without the code that
//! fills or reads them.
//!
//! ST's generator declares each command's parameters as `<name>_cp0`,
//! `<name>_cp1`, …, split after every variable buffer, and its return
//! parameters as `<name>_rp0`. Where no code relates a buffer to its count,
//! one rule of the generator is applied: a buffer sized by
//! `BLE_CMD_MAX_PARAM_LEN` or `BLE_EVT_MAX_PARAM_LEN` is counted, in
//! elements, by the unsigned integer member immediately before it.
//!
//! Event payloads rely on this rule on every platform. The STM32WBA sources
//! ship no command or event code, so all their layouts do; `snapshot.rs`
//! checks the rule against every layout the STM32WB code proves.
//!
//! A union has no selector without code, so a structure holding one is left
//! unresolved.

use std::collections::BTreeMap;

use stm32wb_catalog::{Completion, Field, FieldType, Layout, Structs};

use crate::c::{self, CRecord, CType};

/// The parameter and return layouts of a command, from its `_cpN` and `_rp0`
/// structures, with the structures they reference.
pub fn command(
    name: &str,
    completion: Completion,
    records: &BTreeMap<String, CRecord>,
) -> (Layout, Option<Layout>, Structs) {
    let mut structs = Structs::new();
    let params = params(name, records, &mut structs).unwrap_or_else(Layout::Unresolved);
    let returns = match completion {
        Completion::CommandStatus => None,
        Completion::CommandComplete => {
            Some(returns(name, records, &mut structs).unwrap_or_else(Layout::Unresolved))
        }
    };
    let structs = referenced([Some(&params), returns.as_ref()], &structs);
    (params, returns, structs)
}

/// The payload of an event from its `_rp0` structure, or no parameters where
/// the generator declares none, with the structures it references.
pub fn event(name: &str, records: &BTreeMap<String, CRecord>) -> (Layout, Structs) {
    let record_name = format!("{name}_rp0");
    let mut structs = Structs::new();
    let mut fields = Vec::new();
    let payload = match records.get(&record_name) {
        None => Layout::Fields(fields),
        Some(record) => {
            match record_fields(&record_name, record, records, &mut structs, &mut fields) {
                Ok(()) => Layout::Fields(fields),
                Err(reason) => Layout::Unresolved(reason),
            }
        }
    };
    let structs = referenced([Some(&payload)], &structs);
    (payload, structs)
}

/// The structures resolved layouts reference, without those a layout left
/// unresolved defined before failing.
fn referenced<'a>(
    layouts: impl IntoIterator<Item = Option<&'a Layout>>,
    structs: &Structs,
) -> Structs {
    let mut referenced = Structs::new();
    for layout in layouts.into_iter().flatten() {
        if let Layout::Fields(fields) = layout {
            crate::commands::collect_structs(fields, structs, &mut referenced);
        }
    }
    referenced
}

fn params(
    name: &str,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Layout, String> {
    let mut fields = Vec::new();
    for index in 0.. {
        let record_name = format!("{name}_cp{index}");
        let Some(record) = records.get(&record_name) else {
            break;
        };
        record_fields(&record_name, record, records, structs, &mut fields)?;
    }
    Ok(Layout::Fields(fields))
}

/// `<name>_rp0`, or a lone Status where the generator declares none.
fn returns(
    name: &str,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Layout, String> {
    let record_name = format!("{name}_rp0");
    let Some(record) = records.get(&record_name) else {
        return Ok(Layout::Fields(vec![Field::new(
            "Status",
            FieldType::Scalar(stm32wb_catalog::Scalar::U8),
        )]));
    };
    let mut fields = Vec::new();
    record_fields(&record_name, record, records, structs, &mut fields)?;
    Ok(Layout::Fields(fields))
}

/// Append the members of one packed structure to `fields`, counting each
/// variable buffer by the member before it.
pub fn record_fields(
    record_name: &str,
    record: &CRecord,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
    fields: &mut Vec<Field>,
) -> Result<(), String> {
    if record.union || !record.packed {
        return Err(format!("{record_name} is not a packed structure"));
    }
    for field in &record.fields {
        let ty = match &field.ty {
            CType::Array {
                element,
                len,
                capacity: true,
            } => {
                let count = fields
                    .last()
                    .filter(|previous| {
                        matches!(previous.ty, FieldType::Scalar(scalar) if !scalar.is_signed())
                    })
                    .ok_or_else(|| format!("{} is not preceded by an unsigned count", field.name))?
                    .name
                    .clone();
                FieldType::Counted {
                    element: c::element_type(element, records, structs)?,
                    count,
                    capacity: u16::try_from(*len).map_err(|_| "capacity overflows u16")?,
                }
            }
            _ => c::fixed_field(field, records, structs)?,
        };
        fields.push(Field::new(field.name.clone(), ty));
    }
    Ok(())
}
