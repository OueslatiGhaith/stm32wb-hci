//! Commands from the generated `ble_*_aci.c` and `ble_hci_le.c` wrappers.
//!
//! Each wrapper fills one or more packed `<name>_cpN` structures in a command
//! buffer, adds each member's encoded size to `index_input`, and sends the
//! buffer with `hci_send_req`. The analysis reads that code through libclang
//! references rather than text:
//!
//! - the opcode is `rq.ogf << 10 | rq.ocf`, and `rq.event = 0x0F` marks a
//!   Command Status command;
//! - member order and widths come from the `cpN` structures, and every member
//!   must be written exactly once, in order, followed by an `index_input`
//!   increment equal to its encoded size;
//! - a variable buffer's element count is the parameter passed as the
//!   `Osal_MemCpy` length, traced back to the member that parameter was
//!   assigned to;
//! - a union's width is the `size` chosen by a `switch` or `?:` on a
//!   parameter, traced back to the member holding that parameter;
//! - return parameters come from the `<name>_rp0` structure, with variable
//!   buffers counted by the `resp` member the copy length refers to.
//!
//! Anything outside these shapes leaves the layout unresolved with a reason.

use std::collections::BTreeMap;

use clang::{Entity, EntityKind, TranslationUnit};
use stm32wb_catalog::{
    CommandScope, Completion, Element, Field, FieldType, Layout, Scalar, Structs, UnionVariant,
};

use crate::c::{
    self, CRecord, CType, address_of_member, descendants, int_value, local, member, operator,
    parameter, strip,
};

#[derive(Debug)]
pub struct ExtractedCommand {
    pub scope: CommandScope,
    pub opcode: u16,
    pub name: String,
    pub completion: Completion,
    pub params: Layout,
    pub returns: Option<Layout>,
    pub structs: Structs,
    /// Count relationships the code proves: (count member, buffer member,
    /// whether the count immediately precedes the buffer).
    pub proven_counts: Vec<(String, String, bool)>,
}

pub fn extract(
    unit: &TranslationUnit<'_>,
    records: &BTreeMap<String, CRecord>,
    scope: CommandScope,
) -> Result<Vec<ExtractedCommand>, String> {
    let mut commands = Vec::new();
    for function in c::main_file_entities(unit) {
        if function.get_kind() != EntityKind::FunctionDecl || !function.is_definition() {
            continue;
        }
        let name = function.get_name().unwrap_or_default();
        let command =
            analyze(function, &name, records, scope).map_err(|error| format!("{name}: {error}"))?;
        commands.push(command);
    }
    Ok(commands)
}

/// Facts that identify a command. Failing to derive any of them is fatal:
/// the catalog must never guess an opcode or completion kind.
fn analyze(
    function: Entity<'_>,
    name: &str,
    records: &BTreeMap<String, CRecord>,
    scope: CommandScope,
) -> Result<ExtractedCommand, String> {
    let body = function
        .get_children()
        .into_iter()
        .find(|child| child.get_kind() == EntityKind::CompoundStmt)
        .ok_or("the wrapper has no body")?;
    let nodes = descendants(body);

    let mut request = BTreeMap::new();
    for node in &nodes {
        if node.get_kind() != EntityKind::BinaryOperator || operator(*node).as_deref() != Some("=")
        {
            continue;
        }
        let [left, right] = node.get_children()[..] else {
            continue;
        };
        if let Some((base, field)) = member(left)
            && base == "rq"
            && request.insert(field.clone(), right).is_some()
        {
            return Err(format!("rq.{field} is assigned more than once"));
        }
    }
    let integer = |field: &str| {
        request
            .get(field)
            .and_then(|value| int_value(*value))
            .ok_or_else(|| format!("rq.{field} is not an integer constant"))
    };
    let ogf = integer("ogf")?;
    let ocf = integer("ocf")?;
    if !(0..0x40).contains(&ogf) || !(0..0x400).contains(&ocf) {
        return Err(format!("OGF 0x{ogf:X} / OCF 0x{ocf:X} is out of range"));
    }
    let opcode = u16::try_from(ogf << 10 | ocf).expect("range-checked");
    match (scope, ogf == 0x3F) {
        (CommandScope::Vendor, false) | (CommandScope::Standard, true) => {
            return Err(format!(
                "OGF 0x{ogf:02X} does not belong to {scope:?} commands"
            ));
        }
        _ => {}
    }
    let completion = match request.get("event").map(|value| int_value(*value)) {
        None => Completion::CommandComplete,
        Some(Some(0x0F)) => Completion::CommandStatus,
        Some(other) => return Err(format!("unexpected rq.event {other:?}")),
    };
    let return_variable = request
        .get("rparam")
        .map(|value| strip(*value))
        .filter(|value| value.get_kind() == EntityKind::UnaryOperator)
        .and_then(|value| value.get_children().first().copied())
        .and_then(local)
        .ok_or("rq.rparam does not point to a local variable")?;

    let mut structs = Structs::new();
    let mut proven_counts = Vec::new();
    let params = params_layout(name, &nodes, records, &mut structs, &mut proven_counts)
        .unwrap_or_else(Layout::Unresolved);
    let returns = match (completion, return_variable.as_str()) {
        (Completion::CommandStatus, "status") => None,
        (Completion::CommandStatus, other) => {
            return Err(format!("a Command Status wrapper returns through {other}"));
        }
        (Completion::CommandComplete, "status") => Some(Layout::Fields(vec![Field::new(
            "Status",
            FieldType::Scalar(Scalar::U8),
        )])),
        (Completion::CommandComplete, "resp") => Some(
            returns_layout(name, &nodes, records, &mut structs).unwrap_or_else(Layout::Unresolved),
        ),
        (_, other) => return Err(format!("unexpected return variable {other}")),
    };

    // A layout left unresolved must not leave orphaned structure definitions.
    let mut referenced = Structs::new();
    for layout in std::iter::once(&params).chain(returns.as_ref()) {
        if let Layout::Fields(fields) = layout {
            collect_structs(fields, &structs, &mut referenced);
        }
    }

    Ok(ExtractedCommand {
        scope,
        opcode,
        name: name.to_owned(),
        completion,
        params,
        returns,
        structs: referenced,
        proven_counts,
    })
}

fn collect_structs(fields: &[Field], all: &Structs, referenced: &mut Structs) {
    for field in fields {
        let name = match &field.ty {
            FieldType::Struct(name)
            | FieldType::Array {
                element: Element::Struct(name),
                ..
            }
            | FieldType::Counted {
                element: Element::Struct(name),
                ..
            } => name,
            _ => continue,
        };
        if let Some(inner) = all.get(name)
            && referenced.insert(name.clone(), inner.clone()).is_none()
        {
            collect_structs(inner, all, referenced);
        }
    }
}

/// What a length expression counts: `count * unit` bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Length {
    count: Count,
    unit: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Count {
    Constant(i64),
    Parameter(String),
    Local(String),
    DereferencedParameter(String),
    Member(String, String),
}

fn length(entity: Entity<'_>) -> Option<Length> {
    let entity = strip(entity);
    if let Some(value) = int_value(entity) {
        return Some(Length {
            count: Count::Constant(value),
            unit: 1,
        });
    }
    if entity.get_kind() == EntityKind::BinaryOperator && operator(entity).as_deref() == Some("*") {
        let [left, right] = entity.get_children()[..] else {
            return None;
        };
        for (count, unit) in [(left, right), (right, left)] {
            let unit = strip(unit);
            if unit.get_kind() == EntityKind::UnaryExpr
                && let (Some(unit), Some(count)) = (int_value(unit), count_of(count))
            {
                return Some(Length { count, unit });
            }
        }
        return None;
    }
    count_of(entity).map(|count| Length { count, unit: 1 })
}

fn count_of(entity: Entity<'_>) -> Option<Count> {
    let entity = strip(entity);
    if let Some(name) = parameter(entity) {
        return Some(Count::Parameter(name));
    }
    if let Some(name) = local(entity) {
        return Some(Count::Local(name));
    }
    if let Some((base, field)) = member(entity) {
        return Some(Count::Member(base, field));
    }
    if entity.get_kind() == EntityKind::UnaryOperator && operator(entity).as_deref() == Some("*") {
        return parameter(*entity.get_children().first()?).map(Count::DereferencedParameter);
    }
    None
}

#[derive(Debug)]
enum Write<'tu> {
    /// `cpN->member = parameter-or-expression`
    Assign { parameter: Option<String> },
    /// `Osal_MemCpy(&cpN->member, source, length)`
    Copy { length: Entity<'tu> },
}

/// The ordered `cpN` members and how the wrapper writes each of them.
fn params_layout(
    name: &str,
    nodes: &[Entity<'_>],
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
    proven_counts: &mut Vec<(String, String, bool)>,
) -> Result<Layout, String> {
    // The command buffer views, keyed by variable name, in `cpN` order.
    let mut views = Vec::new();
    for node in nodes {
        if node.get_kind() != EntityKind::VarDecl {
            continue;
        }
        let (Some(variable), Some(ty)) = (node.get_name(), node.get_type()) else {
            continue;
        };
        let Some(record) = ty.get_pointee_type().and_then(c::typedef_name) else {
            continue;
        };
        if let Some(index) = record
            .strip_prefix(name)
            .and_then(|suffix| suffix.strip_prefix("_cp"))
            .and_then(|index| index.parse::<usize>().ok())
        {
            views.push((index, variable, record));
        }
    }
    views.sort();
    if views
        .iter()
        .enumerate()
        .any(|(position, (index, ..))| position != *index)
    {
        return Err(format!(
            "{name}: command buffer views are not numbered cp0, cp1, …"
        ));
    }

    // Every write to a view member, each followed by its size increment.
    let mut writes: BTreeMap<(String, String), (Write<'_>, Entity<'_>)> = BTreeMap::new();
    let mut order = Vec::new();
    let mut pending: Option<((String, String), Write<'_>)> = None;
    for node in nodes {
        let write = match node.get_kind() {
            EntityKind::BinaryOperator if operator(*node).as_deref() == Some("=") => {
                let [left, right] = node.get_children()[..] else {
                    continue;
                };
                member(left).map(|target| {
                    (
                        target,
                        Write::Assign {
                            parameter: parameter(right),
                        },
                    )
                })
            }
            EntityKind::CallExpr if node.get_name().as_deref() == Some("Osal_MemCpy") => {
                let arguments = node.get_arguments().unwrap_or_default();
                match arguments[..] {
                    [destination, _, length] => address_of_member(destination)
                        .map(|target| (target, Write::Copy { length })),
                    _ => None,
                }
            }
            EntityKind::CompoundAssignOperator if operator(*node).as_deref() == Some("+=") => {
                let [left, right] = node.get_children()[..] else {
                    continue;
                };
                if local(left).as_deref() != Some("index_input") {
                    continue;
                }
                let (target, write) = pending
                    .take()
                    .ok_or("an index_input increment follows no member write")?;
                if writes.insert(target.clone(), (write, right)).is_some() {
                    return Err(format!(
                        "{}->{} is written more than once",
                        target.0, target.1
                    ));
                }
                order.push(target);
                continue;
            }
            _ => None,
        };
        let Some((target, write)) = write else {
            continue;
        };
        if !views.iter().any(|(_, variable, _)| *variable == target.0) {
            continue;
        }
        if let Some((previous, _)) = pending.replace((target, write)) {
            return Err(format!(
                "{}->{} is written without an index_input increment",
                previous.0, previous.1
            ));
        }
    }
    if let Some((target, _)) = pending {
        return Err(format!(
            "{}->{} has no index_input increment",
            target.0, target.1
        ));
    }

    let mut fields = Vec::new();
    let mut expected_order = Vec::new();
    // Parameter name -> the member it was stored in.
    let mut stored: BTreeMap<String, String> = BTreeMap::new();
    for (_, variable, record_name) in &views {
        let record = records
            .get(record_name)
            .ok_or_else(|| format!("{record_name} is not declared"))?;
        if record.union || !record.packed {
            return Err(format!("{record_name} is not a packed structure"));
        }
        for field in &record.fields {
            let target = (variable.clone(), field.name.clone());
            expected_order.push(target.clone());
            let (write, increment) = writes
                .get(&target)
                .ok_or_else(|| format!("{variable}->{} is never written", field.name))?;
            let increment = length(*increment).ok_or_else(|| {
                format!(
                    "{variable}->{} has an unrecognized size increment",
                    field.name
                )
            })?;
            let ty = member_type(
                field,
                write,
                &increment,
                records,
                structs,
                &stored,
                nodes,
                fields.last().map(|field: &Field| field.name.as_str()),
                proven_counts,
            )
            .map_err(|error| format!("{variable}->{}: {error}", field.name))?;
            if let Write::Assign {
                parameter: Some(parameter),
            } = write
            {
                stored.insert(parameter.clone(), field.name.clone());
            }
            fields.push(Field::new(field.name.clone(), ty));
        }
    }
    if order != expected_order {
        return Err("members are not written in structure order".to_owned());
    }
    Ok(Layout::Fields(fields))
}

#[allow(clippy::too_many_arguments)]
fn member_type(
    field: &c::CField,
    write: &Write<'_>,
    increment: &Length,
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
    stored: &BTreeMap<String, String>,
    nodes: &[Entity<'_>],
    previous: Option<&str>,
    proven_counts: &mut Vec<(String, String, bool)>,
) -> Result<FieldType, String> {
    let constant = |bytes: usize| Length {
        count: Count::Constant(i64::try_from(bytes).unwrap_or(i64::MAX)),
        unit: 1,
    };
    match (&field.ty, write) {
        (CType::Scalar(scalar), Write::Assign { .. }) => {
            if *increment != constant(scalar.width().into()) {
                return Err(format!(
                    "size increment {increment:?} does not match its width"
                ));
            }
            Ok(FieldType::Scalar(*scalar))
        }
        (CType::Record(record_name), Write::Copy { length: copied }) => {
            let record = records
                .get(record_name)
                .ok_or_else(|| format!("{record_name} is not declared"))?;
            let copied = length(*copied).ok_or("unrecognized copy length")?;
            if copied != *increment {
                return Err("the copy length and size increment differ".to_owned());
            }
            if !record.union {
                if copied != constant(record.size) {
                    return Err("a fixed structure is copied with a different size".to_owned());
                }
                return c::fixed_field(field, records, structs);
            }
            let Count::Local(size) = &copied.count else {
                return Err("a union is copied without a selected size".to_owned());
            };
            let (selector, variants) = union_variants(nodes, size)?;
            let selector = stored
                .get(&selector)
                .ok_or_else(|| {
                    format!("selector parameter {selector} is not stored in the command")
                })?
                .clone();
            if variants
                .iter()
                .any(|variant| usize::from(variant.width) > record.size)
            {
                return Err(format!("a {record_name} alternative exceeds the union"));
            }
            Ok(FieldType::Union { selector, variants })
        }
        (
            CType::Array {
                element,
                len,
                capacity,
            },
            Write::Copy { length: copied },
        ) => {
            let width = c::element_width(element, records).ok_or("unsupported array element")?;
            let copied = length(*copied).ok_or("unrecognized copy length")?;
            if copied != *increment {
                return Err("the copy length and size increment differ".to_owned());
            }
            if !capacity {
                if copied != constant(width * len) {
                    return Err("a fixed array is copied with a different size".to_owned());
                }
                return c::fixed_field(field, records, structs);
            }
            let Count::Parameter(parameter) = &copied.count else {
                return Err(format!(
                    "a variable buffer is copied with length {copied:?}"
                ));
            };
            let count = stored
                .get(parameter)
                .ok_or_else(|| format!("count parameter {parameter} is not stored in the command"))?
                .clone();
            let unit = usize::try_from(copied.unit).map_err(|_| "negative element size")?;
            let (element, capacity) = if unit == width {
                (c::element_type(element, records, structs)?, *len)
            } else if unit == 1 {
                // The count is in bytes even though C declares larger elements.
                (Element::Scalar(Scalar::U8), width * len)
            } else {
                return Err(format!("element size {unit} does not match {width}"));
            };
            proven_counts.push((
                count.clone(),
                field.name.clone(),
                previous == Some(count.as_str()),
            ));
            Ok(FieldType::Counted {
                element,
                count,
                capacity: u16::try_from(capacity).map_err(|_| "capacity overflows u16")?,
            })
        }
        (ty, write) => Err(format!("unsupported member {ty:?} written by {write:?}")),
    }
}

/// Resolve the widths a local `size` variable takes, and the parameter that
/// selects among them, from its `?:` initializer or the `switch` assigning it.
fn union_variants(
    nodes: &[Entity<'_>],
    variable: &str,
) -> Result<(String, Vec<UnionVariant>), String> {
    let declaration = nodes
        .iter()
        .find(|node| {
            node.get_kind() == EntityKind::VarDecl && node.get_name().as_deref() == Some(variable)
        })
        .ok_or_else(|| format!("{variable} is not declared locally"))?;
    if let Some(initializer) = declaration
        .get_children()
        .into_iter()
        .find(|child| child.get_kind() != EntityKind::TypeRef)
    {
        let mut variants = Vec::new();
        let selector = conditional_variants(initializer, &mut variants)?;
        return Ok((selector, variants));
    }

    let switch = nodes
        .iter()
        .find(|node| {
            node.get_kind() == EntityKind::SwitchStmt
                && descendants(**node)
                    .iter()
                    .any(|inner| assigns(*inner, variable).is_some())
        })
        .ok_or_else(|| format!("{variable} is neither initialized nor assigned by a switch"))?;
    let children = switch.get_children();
    let selector = children
        .first()
        .and_then(|condition| parameter(*condition))
        .ok_or("the switch does not select on a parameter")?;
    let mut variants = Vec::new();
    for node in descendants(*switch) {
        match node.get_kind() {
            EntityKind::CaseStmt => {
                let case = node.get_children();
                let tag = case
                    .first()
                    .and_then(|value| int_value(*value))
                    .and_then(|value| u64::try_from(value).ok())
                    .ok_or("a case label is not a non-negative constant")?;
                let width = case
                    .iter()
                    .skip(1)
                    .find_map(|statement| assigns(*statement, variable))
                    .ok_or("a case does not assign the size")?;
                variants.push(UnionVariant {
                    tag: Some(tag),
                    width,
                });
            }
            EntityKind::DefaultStmt => {
                let body = node.get_children();
                match body.first().map(|statement| statement.get_kind()) {
                    Some(EntityKind::ReturnStmt) => {}
                    _ => {
                        let width = body
                            .iter()
                            .find_map(|statement| assigns(*statement, variable))
                            .ok_or("the default case neither returns nor assigns the size")?;
                        variants.push(UnionVariant { tag: None, width });
                    }
                }
            }
            _ => {}
        }
    }
    if variants.is_empty() {
        return Err("the switch has no cases".to_owned());
    }
    Ok((selector, variants))
}

/// `variable = constant`: the constant.
fn assigns(statement: Entity<'_>, variable: &str) -> Option<u16> {
    if statement.get_kind() != EntityKind::BinaryOperator
        || operator(statement).as_deref() != Some("=")
    {
        return None;
    }
    let [left, right] = statement.get_children()[..] else {
        return None;
    };
    (local(left).as_deref() == Some(variable))
        .then(|| int_value(right))
        .flatten()
        .and_then(|value| u16::try_from(value).ok())
}

/// `(p == a) ? x : ((p == b) ? y : z)`: the variants, returning `p`.
fn conditional_variants(
    expression: Entity<'_>,
    variants: &mut Vec<UnionVariant>,
) -> Result<String, String> {
    let expression = strip(expression);
    if let Some(width) = int_value(expression) {
        variants.push(UnionVariant {
            tag: None,
            width: u16::try_from(width).map_err(|_| "negative union width")?,
        });
        return Ok(String::new());
    }
    if expression.get_kind() != EntityKind::ConditionalOperator {
        return Err("a union size is neither constant nor conditional".to_owned());
    }
    let [condition, then, otherwise] = expression.get_children()[..] else {
        return Err("malformed conditional".to_owned());
    };
    let condition = strip(condition);
    if operator(condition).as_deref() != Some("==") {
        return Err("a union size condition is not an equality".to_owned());
    }
    let [left, right] = condition.get_children()[..] else {
        return Err("malformed equality".to_owned());
    };
    let (selector, tag) = match (
        parameter(left),
        int_value(right),
        parameter(right),
        int_value(left),
    ) {
        (Some(selector), Some(tag), ..) | (.., Some(selector), Some(tag)) => (selector, tag),
        _ => return Err("a union size condition does not compare a parameter".to_owned()),
    };
    let width = int_value(then).ok_or("a union alternative is not constant")?;
    variants.push(UnionVariant {
        tag: Some(u64::try_from(tag).map_err(|_| "negative union tag")?),
        width: u16::try_from(width).map_err(|_| "negative union width")?,
    });
    let rest = conditional_variants(otherwise, variants)?;
    if !rest.is_empty() && rest != selector {
        return Err("a union size depends on several parameters".to_owned());
    }
    Ok(selector)
}

/// Command Complete return parameters from `resp`, the `<name>_rp0` structure.
fn returns_layout(
    name: &str,
    nodes: &[Entity<'_>],
    records: &BTreeMap<String, CRecord>,
    structs: &mut Structs,
) -> Result<Layout, String> {
    let record_name = format!("{name}_rp0");
    let record = records
        .get(&record_name)
        .ok_or_else(|| format!("{record_name} is not declared"))?;
    if record.union || !record.packed {
        return Err(format!("{record_name} is not a packed structure"));
    }
    // `*Param = resp.Member` assignments, so `*Param` lengths can be traced.
    let mut dereferenced = BTreeMap::new();
    let mut copies = BTreeMap::new();
    for node in nodes {
        match node.get_kind() {
            EntityKind::BinaryOperator if operator(*node).as_deref() == Some("=") => {
                let [left, right] = node.get_children()[..] else {
                    continue;
                };
                let left = strip(left);
                if left.get_kind() == EntityKind::UnaryOperator
                    && operator(left).as_deref() == Some("*")
                    && let (Some(parameter), Some(("resp", field))) = (
                        left.get_children()
                            .first()
                            .and_then(|inner| parameter(*inner)),
                        member(right)
                            .as_ref()
                            .map(|(base, field)| (base.as_str(), field.clone())),
                    )
                {
                    dereferenced.insert(parameter, field);
                }
            }
            EntityKind::CallExpr if node.get_name().as_deref() == Some("Osal_MemCpy") => {
                let arguments = node.get_arguments().unwrap_or_default();
                if let [_, source, length_argument] = arguments[..]
                    && let Some((base, field)) = member(source)
                    && base == "resp"
                {
                    copies.insert(field, length_argument);
                }
            }
            _ => {}
        }
    }

    let mut fields: Vec<Field> = Vec::new();
    for field in &record.fields {
        let ty = match &field.ty {
            CType::Array {
                element,
                len,
                capacity: true,
            } => {
                let width =
                    c::element_width(element, records).ok_or("unsupported array element")?;
                let copied = copies
                    .get(&field.name)
                    .and_then(|length_argument| length(*length_argument))
                    .ok_or_else(|| {
                        format!("resp.{} is not copied with a traceable length", field.name)
                    })?;
                let count = match &copied.count {
                    Count::Member(base, count) if base == "resp" => count.clone(),
                    Count::DereferencedParameter(parameter) => dereferenced
                        .get(parameter)
                        .cloned()
                        .ok_or_else(|| format!("*{parameter} is not read from resp"))?,
                    other => {
                        return Err(format!(
                            "resp.{} is copied with length {other:?}",
                            field.name
                        ));
                    }
                };
                let unit = usize::try_from(copied.unit).map_err(|_| "negative element size")?;
                let (element, capacity) = if unit == width {
                    (c::element_type(element, records, structs)?, *len)
                } else if unit == 1 {
                    (Element::Scalar(Scalar::U8), width * len)
                } else {
                    return Err(format!("element size {unit} does not match {width}"));
                };
                FieldType::Counted {
                    element,
                    count,
                    capacity: u16::try_from(capacity).map_err(|_| "capacity overflows u16")?,
                }
            }
            _ => {
                c::fixed_field(field, records, structs).map_err(|error| format!("resp.{error}"))?
            }
        };
        fields.push(Field::new(field.name.clone(), ty));
    }
    if fields.first().map(|field| (&field.name, &field.ty))
        != Some((&"Status".to_owned(), &FieldType::Scalar(Scalar::U8)))
    {
        return Err(format!(
            "{record_name} does not begin with a one-byte Status"
        ));
    }
    Ok(Layout::Fields(fields))
}
