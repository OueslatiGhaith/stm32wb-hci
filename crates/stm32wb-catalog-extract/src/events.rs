//! Events from the generated `ble_events.c` dispatch tables.
//!
//! `hci_event_table`, `hci_le_event_table`, and `hci_vs_event_table` map each
//! event code to a process function. A process function that only casts the
//! packet to `<event>_rp0` and forwards its members to the callback defines
//! the payload as that packed structure. Anything more (loops, pointer
//! arithmetic) decodes the payload procedurally and is left unresolved.
//! Parameters addressing an ATT bearer come from the doc comment of the
//! callback in `ble_events.h`, as for commands.
//!
//! Event payloads carry no code relating a variable buffer to its count, so
//! their structure is read by the rule of `declared.rs`: a buffer is counted,
//! in elements, by the unsigned integer member immediately before it.

use std::collections::BTreeMap;

use clang::{Entity, EntityKind, TranslationUnit};
use stm32wb_catalog::{Bearer, EventScope, Layout, Structs};

use crate::c::{self, CRecord, descendants, int_value, member, strip};

#[derive(Debug)]
pub struct ExtractedEvent {
    pub scope: EventScope,
    pub code: u16,
    pub name: String,
    pub payload: Layout,
    pub structs: Structs,
    /// Parameters documented to accept an enhanced ATT bearer.
    pub bearers: Vec<Bearer>,
    /// Documented values of parameters.
    pub domains: Vec<crate::domains::Documented>,
}

const TABLES: [(&str, EventScope); 3] = [
    ("hci_event_table", EventScope::Standard),
    ("hci_le_event_table", EventScope::LeMeta),
    ("hci_vs_event_table", EventScope::Vendor),
];

pub fn extract(
    unit: &TranslationUnit<'_>,
    records: &BTreeMap<String, CRecord>,
) -> Result<Vec<ExtractedEvent>, String> {
    let entities = c::main_file_entities(unit);
    // The callbacks the process functions forward to, which `ble_events.h`
    // declares and documents.
    let callbacks = unit
        .get_entity()
        .get_children()
        .into_iter()
        .filter(|entity| entity.get_kind() == EntityKind::FunctionDecl)
        .filter_map(|entity| Some((entity.get_name()?, entity)))
        .collect::<BTreeMap<_, _>>();
    let functions = entities
        .iter()
        .filter(|entity| entity.get_kind() == EntityKind::FunctionDecl && entity.is_definition())
        .filter_map(|entity| Some((entity.get_name()?, *entity)))
        .collect::<BTreeMap<_, _>>();

    let mut events = Vec::new();
    for (table, scope) in TABLES {
        let declaration = entities
            .iter()
            .find(|entity| {
                entity.get_kind() == EntityKind::VarDecl
                    && entity.get_name().as_deref() == Some(table)
            })
            .ok_or_else(|| format!("{table} is not defined"))?;
        let initializer = declaration
            .get_children()
            .into_iter()
            .find(|child| child.get_kind() == EntityKind::InitListExpr)
            .ok_or_else(|| format!("{table} has no initializer"))?;
        for entry in initializer.get_children() {
            let [code, handler] = entry.get_children()[..] else {
                return Err(format!("{table} has a malformed entry"));
            };
            let code = int_value(code)
                .and_then(|code| u16::try_from(code).ok())
                .ok_or_else(|| format!("{table} has a non-constant code"))?;
            let handler = strip(handler)
                .get_reference()
                .and_then(|reference| reference.get_name())
                .ok_or_else(|| format!("{table} entry 0x{code:04X} has no handler"))?;
            let name = handler
                .strip_suffix("_process")
                .ok_or_else(|| format!("handler {handler} is not a process function"))?
                .to_owned();
            let function = functions
                .get(&handler)
                .ok_or_else(|| format!("{handler} is not defined"))?;
            let mut structs = Structs::new();
            let payload =
                payload(*function, &name, records, &mut structs).unwrap_or_else(|reason| {
                    structs.clear();
                    Layout::Unresolved(reason)
                });
            let callback = callbacks
                .get(&name)
                .ok_or_else(|| format!("the callback {name} is not declared"))?;
            let bearers = c::bearers(*callback).map_err(|error| format!("{name}: {error}"))?;
            let domains =
                crate::domains::domains(*callback).map_err(|error| format!("{name}: {error}"))?;
            events.push(ExtractedEvent {
                scope,
                code,
                name,
                payload,
                structs,
                bearers,
                domains,
            });
        }
    }
    Ok(events)
}

fn payload(
    function: Entity<'_>,
    name: &str,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Layout, String> {
    let body = function
        .get_children()
        .into_iter()
        .find(|child| child.get_kind() == EntityKind::CompoundStmt)
        .ok_or("the process function has no body")?;
    let statements = body.get_children();
    let record_name = format!("{name}_rp0");

    // `callback()` alone: an event without parameters.
    if let [call] = statements[..]
        && strip(call).get_kind() == EntityKind::CallExpr
        && call
            .get_arguments()
            .is_some_and(|arguments| arguments.is_empty())
    {
        return Ok(Layout::Fields(Vec::new()));
    }

    // `<name>_rp0 *rp0 = (void*)in; callback(rp0->A, rp0->B, ...);`
    let [declaration, call] = statements[..] else {
        return Err("the process function decodes its payload procedurally".to_owned());
    };
    let declared = descendants(declaration)
        .into_iter()
        .find(|node| node.get_kind() == EntityKind::VarDecl)
        .and_then(|variable| variable.get_type())
        .and_then(|ty| ty.get_pointee_type())
        .and_then(c::typedef_name);
    if declared.as_deref() != Some(record_name.as_str()) {
        return Err(format!("the payload is not read through {record_name}"));
    }
    let record = records
        .get(&record_name)
        .ok_or_else(|| format!("{record_name} is not declared"))?;
    let call = strip(call);
    let forwarded = call
        .get_arguments()
        .unwrap_or_default()
        .into_iter()
        .map(|argument| {
            member(argument)
                .filter(|(base, _)| base == "rp0")
                .map(|(_, field)| field)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or("the callback receives something other than rp0 members")?;
    let members = record
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    if call.get_kind() != EntityKind::CallExpr || forwarded != members {
        return Err(format!(
            "the callback does not receive every {record_name} member in order"
        ));
    }

    let mut fields = Vec::new();
    crate::declared::record_fields(&record_name, record, records, structs, &mut fields)?;
    Ok(Layout::Fields(fields))
}
