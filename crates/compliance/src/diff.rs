use crate::spec::{
    CommandSpec, EventSpec, FirmwareSpec, PackedStructSpec, PayloadField, StructFieldSpec,
};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Serialize)]
pub struct FirmwareDiff {
    pub from: String,
    pub to: String,
    pub commands: CommandDiff,
    pub events: EventDiff,
    pub packed_structs: StructDiff,
}

#[derive(Default, Debug, Serialize)]
pub struct CommandDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<ChangedItem>,
}

#[derive(Default, Debug, Serialize)]
pub struct StructDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<ChangedItem>,
}

#[derive(Default, Debug, Serialize)]
pub struct EventDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ChangedItem {
    pub name: String,
    pub changes: Vec<Change>,
}

#[derive(Debug, Serialize)]
pub struct Change {
    pub path: String,
    pub from: serde_json::Value,
    pub to: serde_json::Value,
}

pub fn diff_firmware(from: &FirmwareSpec, to: &FirmwareSpec) -> FirmwareDiff {
    FirmwareDiff {
        from: from.firmware.clone(),
        to: to.firmware.clone(),
        commands: diff_commands(&from.commands, &to.commands),
        events: diff_events(&from.events, &to.events),
        packed_structs: diff_structs(&from.packed_structs, &to.packed_structs),
    }
}

fn diff_commands(from: &[CommandSpec], to: &[CommandSpec]) -> CommandDiff {
    let from_by_name = from
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect::<HashMap<_, _>>();
    let to_by_name = to
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect::<HashMap<_, _>>();
    let names = sorted_keys(&from_by_name, &to_by_name);
    let mut diff = CommandDiff::default();

    for name in names {
        match (from_by_name.get(name), to_by_name.get(name)) {
            (None, Some(_)) => diff.added.push(name.to_owned()),
            (Some(_), None) => diff.removed.push(name.to_owned()),
            (Some(from), Some(to)) => {
                let changes = command_changes(from, to);
                if !changes.is_empty() {
                    diff.changed.push(ChangedItem {
                        name: name.to_owned(),
                        changes,
                    });
                }
            }
            (None, None) => {}
        }
    }

    diff
}

fn command_changes(from: &CommandSpec, to: &CommandSpec) -> Vec<Change> {
    let mut changes = Vec::new();
    push_change(&mut changes, "group", &from.group, &to.group);
    push_change(&mut changes, "ogf", &from.ogf, &to.ogf);
    push_change(&mut changes, "ocf", &from.ocf, &to.ocf);
    push_change(&mut changes, "opcode", &from.opcode, &to.opcode);
    push_change(&mut changes, "event", &from.event, &to.event);
    push_change(&mut changes, "return_len", &from.return_len, &to.return_len);
    push_change(
        &mut changes,
        "payload",
        &payload_signature(&from.payload),
        &payload_signature(&to.payload),
    );
    changes
}

fn diff_events(from: &[EventSpec], to: &[EventSpec]) -> EventDiff {
    let from_by_name = from
        .iter()
        .map(|event| (event.name.as_str(), event))
        .collect::<HashMap<_, _>>();
    let to_by_name = to
        .iter()
        .map(|event| (event.name.as_str(), event))
        .collect::<HashMap<_, _>>();
    let names = sorted_keys(&from_by_name, &to_by_name);
    let mut diff = EventDiff::default();

    for name in names {
        match (from_by_name.get(name), to_by_name.get(name)) {
            (None, Some(_)) => diff.added.push(name.to_owned()),
            (Some(_), None) => diff.removed.push(name.to_owned()),
            (Some(_), Some(_)) | (None, None) => {}
        }
    }

    diff
}

fn diff_structs(from: &[PackedStructSpec], to: &[PackedStructSpec]) -> StructDiff {
    let from_by_name = from
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect::<HashMap<_, _>>();
    let to_by_name = to
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect::<HashMap<_, _>>();
    let names = sorted_keys(&from_by_name, &to_by_name);
    let mut diff = StructDiff::default();

    for name in names {
        match (from_by_name.get(name), to_by_name.get(name)) {
            (None, Some(_)) => diff.added.push(name.to_owned()),
            (Some(_), None) => diff.removed.push(name.to_owned()),
            (Some(from), Some(to)) => {
                let changes = struct_changes(from, to);
                if !changes.is_empty() {
                    diff.changed.push(ChangedItem {
                        name: name.to_owned(),
                        changes,
                    });
                }
            }
            (None, None) => {}
        }
    }

    diff
}

fn struct_changes(from: &PackedStructSpec, to: &PackedStructSpec) -> Vec<Change> {
    let mut changes = Vec::new();
    push_change(&mut changes, "byte_size", &from.byte_size, &to.byte_size);
    push_change(
        &mut changes,
        "fields",
        &field_signature(&from.fields),
        &field_signature(&to.fields),
    );
    changes
}

fn payload_signature(payload: &[PayloadField]) -> Vec<serde_json::Value> {
    payload
        .iter()
        .map(|field| {
            serde_json::json!({
                "name": field.name,
                "c_type": field.c_type,
                "wire": field.wire,
                "len": field.len,
                "resolved": field.resolved,
            })
        })
        .collect()
}

fn field_signature(fields: &[StructFieldSpec]) -> Vec<serde_json::Value> {
    fields
        .iter()
        .map(|field| {
            serde_json::json!({
                "name": field.name,
                "c_type": field.c_type,
                "wire": field.wire,
                "array_len": field.array_len,
                "byte_offset": field.byte_offset,
                "byte_size": field.byte_size,
            })
        })
        .collect()
}

fn push_change<T>(changes: &mut Vec<Change>, path: &str, from: &T, to: &T)
where
    T: Serialize,
{
    let from = serde_json::to_value(from).expect("diff value should serialize");
    let to = serde_json::to_value(to).expect("diff value should serialize");
    if from != to {
        changes.push(Change {
            path: path.to_owned(),
            from,
            to,
        });
    }
}

fn sorted_keys<'a, T>(
    from: &'a HashMap<&'a str, T>,
    to: &'a HashMap<&'a str, T>,
) -> BTreeSet<&'a str> {
    from.keys().chain(to.keys()).copied().collect()
}
