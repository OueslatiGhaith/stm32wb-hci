//! System (SHCI) events from the tagged `shci.h`, and system commands from
//! the `shci.c` wrappers sending them.
//!
//! Event codes are the values of `SHCI_SUB_EVT_CODE_t`, evaluated by clang.
//! `shci.h` documents each payload structure with a comment, directly above
//! it, whose first line is the event's enumerator name; that attachment links
//! a code to its packed structure. An event whose
//! enumerator documents no structure is left unresolved rather than assumed
//! to be empty.

use std::collections::BTreeMap;

use clang::{Entity, EntityKind, TranslationUnit, TypeKind};
use stm32wb_catalog::{
    CommandScope, Completion, EventScope, Field, FieldType, Layout, Scalar, Structs,
};

use crate::c::{self, CRecord, CType, descendants, int_value, local, operator, parameter, strip};
use crate::commands::ExtractedCommand;
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
            bearers: Vec::new(),
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

/// The function every system command wrapper sends its command with:
/// `shci_send(opcode, length, parameters, response)`.
const SEND: &str = "shci_send";

/// System commands from the `shci.c` wrappers.
///
/// Each wrapper calls `shci_send` once. The parameters it sends take one of
/// three shapes the analysis derives:
///
/// - none: a length and buffer of 0;
/// - a packed structure: `sizeof` the structure, sent from a pointer to it;
/// - a local byte buffer of constant length, filled either byte by byte from
///   the wrapper's parameters (each byte one parameter, truncated to a byte)
///   or through a cast to a packed structure.
///
/// A length computed at run time, or any other shape, leaves the parameters
/// unresolved. Return parameters are the bytes of the Command Complete
/// payload the wrapper reads: `payload[0]` is the status, and later bytes
/// are stored through a pointer parameter or copied into one's member.
///
/// A pointer member of a parameter structure is an address the wireless CPU
/// reads, 4 bytes on the Cortex-M4 the wrappers are compiled for (AAPCS32
/// section 5.1.1), and is recorded as a `u32`.
pub fn commands(
    unit: &TranslationUnit<'_>,
    records: &BTreeMap<String, CRecord>,
) -> Result<Vec<ExtractedCommand>, String> {
    let mut commands = Vec::new();
    for function in c::main_file_entities(unit) {
        if function.get_kind() != EntityKind::FunctionDecl || !function.is_definition() {
            continue;
        }
        let name = function.get_name().unwrap_or_default();
        let sends = descendants(function)
            .into_iter()
            .filter(|node| {
                node.get_kind() == EntityKind::CallExpr && node.get_name().as_deref() == Some(SEND)
            })
            .collect::<Vec<_>>();
        let send = match sends[..] {
            // Local commands, e.g. reading the firmware information from
            // shared memory, send nothing.
            [] => continue,
            [send] => send,
            _ => return Err(format!("{name} calls {SEND} more than once")),
        };
        let arguments = send.get_arguments().unwrap_or_default();
        let [opcode, length, buffer, _response] = arguments[..] else {
            return Err(format!(
                "{name} calls {SEND} with {} arguments",
                arguments.len()
            ));
        };
        let opcode = int_value(opcode)
            .and_then(|opcode| u16::try_from(opcode).ok())
            .filter(|opcode| opcode >> 10 == 0x3F)
            .ok_or_else(|| format!("{name} sends no constant OGF 0x3F opcode"))?;

        let mut structs = Structs::new();
        let params = params_layout(function, length, buffer, records, &mut structs).unwrap_or_else(
            |reason| {
                structs.clear();
                Layout::Unresolved(reason)
            },
        );
        let returns = returns_layout(function).unwrap_or_else(Layout::Unresolved);
        commands.push(ExtractedCommand {
            scope: CommandScope::System,
            opcode,
            name,
            completion: Completion::CommandComplete,
            params,
            returns: Some(returns),
            structs,
            bearers: Vec::new(),
            proven_counts: Vec::new(),
        });
    }
    if commands.is_empty() {
        return Err(format!("no function calls {SEND}"));
    }
    Ok(commands)
}

/// The fields of a packed parameter structure, with pointers as addresses.
fn record_fields(
    record_name: &str,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<(Vec<Field>, usize), String> {
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
            let ty = match field.ty {
                CType::Pointer => Ok(FieldType::Scalar(Scalar::U32)),
                _ => c::fixed_field(field, records, structs),
            };
            ty.map(|ty| Field::new(field.name.clone(), ty))
                .map_err(|error| format!("{record_name}.{error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((fields, record.size))
}

/// The packed structure a pointer-typed expression points to.
fn pointee_record(expression: Entity<'_>) -> Option<String> {
    let ty = expression.get_type()?;
    if ty.get_canonical_type().get_kind() != TypeKind::Pointer {
        return None;
    }
    c::typedef_name(ty.get_pointee_type()?)
}

fn params_layout(
    function: Entity<'_>,
    length: Entity<'_>,
    buffer: Entity<'_>,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Layout, String> {
    let length = int_value(length)
        .and_then(|length| usize::try_from(length).ok())
        .ok_or("the parameter length is computed at run time")?;
    let buffer = strip(buffer);
    if length == 0 {
        return if int_value(buffer) == Some(0) {
            Ok(Layout::Fields(Vec::new()))
        } else {
            Err("a buffer is sent with no length".to_owned())
        };
    }

    // A local byte buffer, filled in the wrapper.
    if let Some(variable) = local(buffer) {
        return local_buffer(function, &variable, length, records, structs);
    }

    // A pointer to the parameter structure.
    let record = pointee_record(buffer)
        .ok_or("the parameters are neither a local buffer nor a structure pointer")?;
    let (fields, size) = record_fields(&record, records, structs)?;
    if size != length {
        return Err(format!(
            "{length} bytes are sent from the {size}-byte {record}"
        ));
    }
    Ok(Layout::Fields(fields))
}

fn local_buffer(
    function: Entity<'_>,
    variable: &str,
    length: usize,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Layout, String> {
    let mut bytes: BTreeMap<usize, String> = BTreeMap::new();
    let mut record = None;
    for node in descendants(function) {
        if node.get_kind() != EntityKind::BinaryOperator || operator(node).as_deref() != Some("=") {
            continue;
        }
        let [left, right] = node.get_children()[..] else {
            continue;
        };
        let left = strip(left);
        match left.get_kind() {
            // variable[index] = parameter;
            EntityKind::ArraySubscriptExpr => {
                let [base, index] = left.get_children()[..] else {
                    continue;
                };
                if local(base).as_deref() != Some(variable) {
                    continue;
                }
                let index = int_value(index)
                    .and_then(|index| usize::try_from(index).ok())
                    .ok_or_else(|| format!("{variable} is written at a computed index"))?;
                let name = parameter(right).ok_or_else(|| {
                    format!("{variable}[{index}] is not written from a parameter")
                })?;
                if bytes.insert(index, name).is_some() {
                    return Err(format!("{variable}[{index}] is written more than once"));
                }
            }
            // ((Structure *)variable)->member = parameter;
            EntityKind::MemberRefExpr => {
                let Some(base) = left.get_children().first().copied() else {
                    continue;
                };
                if local(base).as_deref() != Some(variable) {
                    continue;
                }
                let name = pointee_record(strip_parens(base))
                    .ok_or_else(|| format!("{variable} is cast to no structure"))?;
                if record
                    .replace(name.clone())
                    .is_some_and(|other| other != name)
                {
                    return Err(format!("{variable} is cast to several structures"));
                }
            }
            _ => {}
        }
    }
    match (record, bytes.is_empty()) {
        (Some(record), true) => {
            let (fields, size) = record_fields(&record, records, structs)?;
            if size != length {
                return Err(format!(
                    "{length} bytes are sent from the {size}-byte {record}"
                ));
            }
            Ok(Layout::Fields(fields))
        }
        (None, false) => {
            if !bytes.keys().copied().eq(0..length) {
                return Err(format!(
                    "{length} bytes are sent but {variable} is written at {:?}",
                    bytes.keys().collect::<Vec<_>>()
                ));
            }
            Ok(Layout::Fields(
                bytes
                    .into_values()
                    .map(|name| Field::new(name, FieldType::Scalar(Scalar::U8)))
                    .collect(),
            ))
        }
        (Some(_), false) => Err(format!("{variable} is written both ways")),
        (None, true) => Err(format!("{variable} is sent without being written")),
    }
}

/// Descend through parentheses only, keeping casts, whose type is the
/// structure a buffer is viewed as.
fn strip_parens(mut entity: Entity<'_>) -> Entity<'_> {
    while entity.get_kind() == EntityKind::ParenExpr {
        match entity.get_children().first() {
            Some(child) => entity = *child,
            None => break,
        }
    }
    entity
}

/// `...->payload[index]`: the byte of the Command Complete payload read.
fn payload_index(entity: Entity<'_>) -> Option<usize> {
    // The subscript itself only, not the conversions wrapping it, so each
    // read is found once.
    if entity.get_kind() != EntityKind::ArraySubscriptExpr {
        return None;
    }
    let [base, index] = entity.get_children()[..] else {
        return None;
    };
    let base = strip(base);
    let field = (base.get_kind() == EntityKind::MemberRefExpr)
        .then(|| base.get_reference())
        .flatten()?
        .get_name()?;
    (field == "payload")
        .then(|| int_value(index))
        .flatten()
        .and_then(|index| usize::try_from(index).ok())
}

fn returns_layout(function: Entity<'_>) -> Result<Layout, String> {
    let nodes = descendants(function);
    let reads = nodes
        .iter()
        .filter_map(|node| payload_index(*node))
        .collect::<Vec<_>>();
    if !reads.contains(&0) {
        return Err("the wrapper does not read the status".to_owned());
    }

    // Each later byte, with the field it is stored in and its width.
    let mut stored: BTreeMap<usize, (String, Scalar)> = BTreeMap::new();
    for node in &nodes {
        let reads_in = |entity: Entity<'_>| {
            descendants(entity)
                .into_iter()
                .filter_map(payload_index)
                .collect::<Vec<_>>()
        };
        match node.get_kind() {
            // *p_name = payload[index];
            EntityKind::BinaryOperator if operator(*node).as_deref() == Some("=") => {
                let [left, right] = node.get_children()[..] else {
                    continue;
                };
                let [index] = reads_in(right)[..] else {
                    continue;
                };
                let left = strip(left);
                let pointer = (left.get_kind() == EntityKind::UnaryOperator
                    && operator(left).as_deref() == Some("*"))
                .then(|| left.get_children().first().copied())
                .flatten()
                .and_then(parameter)
                .ok_or_else(|| format!("payload[{index}] is stored in no pointer parameter"))?;
                let name = pointer.strip_prefix("p_").unwrap_or(&pointer).to_owned();
                stored.insert(index, (name, Scalar::U8));
            }
            // memcpy(&pParam->member, &payload[index], sizeof(pParam->member));
            EntityKind::CallExpr if node.get_name().as_deref() == Some("memcpy") => {
                let arguments = node.get_arguments().unwrap_or_default();
                let [destination, source, length] = arguments[..] else {
                    continue;
                };
                let [index] = reads_in(source)[..] else {
                    continue;
                };
                let destination = strip(destination);
                let member = (destination.get_kind() == EntityKind::UnaryOperator
                    && operator(destination).as_deref() == Some("&"))
                .then(|| destination.get_children().first().copied())
                .flatten()
                .map(strip)
                .filter(|member| member.get_kind() == EntityKind::MemberRefExpr)
                .and_then(|member| member.get_reference())
                .ok_or_else(|| format!("payload[{index}] is copied into no member"))?;
                let scalar = member
                    .get_type()
                    .and_then(c::scalar)
                    .ok_or_else(|| format!("payload[{index}] is copied into a non-integer"))?;
                if int_value(length) != Some(i64::from(scalar.width())) {
                    return Err(format!("payload[{index}] is copied with another length"));
                }
                stored.insert(index, (member.get_name().unwrap_or_default(), scalar));
            }
            _ => {}
        }
    }

    let mut fields = vec![Field::new("Status", FieldType::Scalar(Scalar::U8))];
    let mut next = 1;
    for (index, (name, scalar)) in stored {
        if index != next {
            return Err(format!(
                "payload[{index}] does not follow the previous byte"
            ));
        }
        next += usize::from(scalar.width());
        fields.push(Field::new(name, FieldType::Scalar(scalar)));
    }
    if let Some(unexplained) = reads.iter().find(|&&index| index != 0 && index >= next) {
        return Err(format!("payload[{unexplained}] is read but not stored"));
    }
    Ok(Layout::Fields(fields))
}
