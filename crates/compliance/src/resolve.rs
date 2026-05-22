use crate::spec::{CommandSpec, PackedStructSpec, ResolvedPayload, ReturnPayloadSpec, WireType};
use std::collections::HashMap;

pub fn resolve_command_payloads(commands: &mut [CommandSpec], structs: &[PackedStructSpec]) {
    let struct_sizes = structs
        .iter()
        .filter_map(|s| s.byte_size.map(|size| (s.name.as_str(), size)))
        .collect::<HashMap<_, _>>();

    for command in commands {
        for field in &mut command.payload {
            field.resolved = match (&field.wire, field.len.as_deref()) {
                (WireType::U8, None) => Some(ResolvedPayload::Scalar { byte_size: 1 }),
                (WireType::U16Le, None) => Some(ResolvedPayload::Scalar { byte_size: 2 }),
                (WireType::U32Le, None) => Some(ResolvedPayload::Scalar { byte_size: 4 }),
                (WireType::Struct { name }, None) => {
                    struct_sizes.get(name.as_str()).copied().map(|byte_size| {
                        ResolvedPayload::Struct {
                            name: name.clone(),
                            byte_size,
                        }
                    })
                }
                (WireType::Bytes, Some(len)) => resolve_bytes_len(len, &struct_sizes),
                _ => None,
            };
        }
    }
}

pub fn resolve_command_return_payloads(commands: &mut [CommandSpec], structs: &[PackedStructSpec]) {
    let structs_by_name = structs
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect::<HashMap<_, _>>();

    for command in commands {
        let struct_name = format!("{}_rp0", command.name);
        command.return_payload = structs_by_name
            .get(struct_name.as_str())
            .map(|return_struct| ReturnPayloadSpec {
                struct_name,
                byte_size: return_struct.byte_size,
                fields: return_struct.fields.clone(),
            });
    }
}

fn resolve_bytes_len(len: &str, struct_sizes: &HashMap<&str, usize>) -> Option<ResolvedPayload> {
    if let Some((count_expr, struct_name)) = parse_sizeof_product(len) {
        let element_size = struct_sizes.get(struct_name.as_str()).copied()?;
        return Some(ResolvedPayload::StructArray {
            name: struct_name,
            element_size,
            byte_len_expr: format!("{count_expr} * {element_size}"),
            count_expr,
        });
    }

    Some(ResolvedPayload::Bytes {
        count_expr: len.trim().to_owned(),
        element_size: 1,
    })
}

fn parse_sizeof_product(expr: &str) -> Option<(String, String)> {
    let (left, right) = expr.split_once('*')?;
    let count_expr = left.trim();
    let right = right.trim();
    let inner = right
        .strip_prefix("(sizeof(")
        .and_then(|s| s.strip_suffix("))"))
        .or_else(|| {
            right
                .strip_prefix("sizeof(")
                .and_then(|s| s.strip_suffix(')'))
        })?;

    if count_expr.is_empty() || inner.is_empty() {
        return None;
    }

    Some((count_expr.to_owned(), inner.to_owned()))
}
