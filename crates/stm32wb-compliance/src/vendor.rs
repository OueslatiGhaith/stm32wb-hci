//! Reading the generated STM32CubeWB protocol catalog without modifying its checkout.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use tree_sitter::{Node, Parser, Tree};

use crate::c_preprocessor::TaggedCPreprocessor;
#[cfg(test)]
use crate::c_preprocessor::preprocess_c_source;
use crate::catalog::{
    CatalogCommand, CatalogCommandKind, CatalogCompletion, CatalogEvent, CatalogEventKind,
    CatalogFamily, CatalogSchema, CommandScope, Envelope, EventScope, Evidence, FixedFieldRole,
    VariableSemantic, WireLayout, WireLayoutEvidence, WireSegment,
};
#[cfg(test)]
use crate::model::{CoverageEntry, CoverageOrigin};

pub const AUTO_SOURCE_DIR: &str = "Middlewares/ST/STM32_WPAN/ble/core/auto";
const SHCI_SOURCE: &str = "Middlewares/ST/STM32_WPAN/interface/patterns/ble_thread/shci/shci.h";
const EVENT_SOURCE: &str = "ble_events.c";
const STANDARD_HCI_SOURCE: &str = "ble_hci_le.c";
const TYPES_SOURCE: &str = "ble_types.h";

/// Load vendor ACI, system SHCI, standard HCI, and transport-envelope metadata
/// from a CubeWB tag without changing the checkout.
pub(crate) fn load_vendor_catalog(cube_dir: &Path, tag: &str) -> Result<CatalogSchema, String> {
    verify_tag(cube_dir, tag)?;
    let preprocessor = TaggedCPreprocessor::new(cube_dir, tag)?;

    let types_path = format!("{AUTO_SOURCE_DIR}/{TYPES_SOURCE}");
    let types_source = git_show(cube_dir, tag, &types_path)?;
    let preprocessed_types =
        preprocessor.preprocess(&format!("auto/{TYPES_SOURCE}"), &types_source)?;
    let packed_layouts =
        parse_packed_struct_envelopes_from_preprocessed(&types_source, &preprocessed_types);

    let mut catalog = CatalogSchema::new(CatalogFamily::Stm32Wb, tag);
    for file in command_source_files(cube_dir, tag)? {
        let path = format!("{AUTO_SOURCE_DIR}/{file}");
        let source = git_show(cube_dir, tag, &path)?;
        let preprocessed = preprocessor.preprocess(&format!("auto/{file}"), &source)?;
        let commands = extract_command_metadata_from_preprocessed(
            &source,
            &preprocessed,
            &file,
            CommandScope::VendorAci,
            &packed_layouts,
        )?;
        catalog.commands.extend(commands);
    }

    let path = format!("{AUTO_SOURCE_DIR}/{EVENT_SOURCE}");
    let source = git_show(cube_dir, tag, &path)?;
    let preprocessed_events = preprocessor.preprocess(&format!("auto/{EVENT_SOURCE}"), &source)?;
    let vendor_events = extract_event_table_from_preprocessed(
        &source,
        &preprocessed_events,
        "hci_vs_event_table",
        EventScope::VendorAci,
        &packed_layouts,
    )?;
    catalog.events.extend(vendor_events);

    let standard_path = format!("{AUTO_SOURCE_DIR}/{STANDARD_HCI_SOURCE}");
    let standard_source = git_show(cube_dir, tag, &standard_path)?;
    let preprocessed_standard =
        preprocessor.preprocess(&format!("auto/{STANDARD_HCI_SOURCE}"), &standard_source)?;
    let standard_commands = extract_command_metadata_from_preprocessed(
        &standard_source,
        &preprocessed_standard,
        STANDARD_HCI_SOURCE,
        CommandScope::StandardHci,
        &packed_layouts,
    )?;
    catalog.commands.extend(standard_commands);

    let standard_events = extract_event_table_from_preprocessed(
        &source,
        &preprocessed_events,
        "hci_event_table",
        EventScope::StandardHci,
        &packed_layouts,
    )?;
    catalog.events.extend(standard_events);

    let le_events = extract_event_table_from_preprocessed(
        &source,
        &preprocessed_events,
        "hci_le_event_table",
        EventScope::LeMeta,
        &packed_layouts,
    )?;
    catalog.events.extend(le_events);

    let shci_source = git_show_lossy_utf8(cube_dir, tag, SHCI_SOURCE)?;
    catalog.events.extend(extract_shci_events(&shci_source)?);

    catalog.normalize()?;
    Ok(catalog)
}

/// Discover generated vendor ACI sources at the selected tag instead of
/// maintaining a list that must be updated for every CubeWB release. Standard
/// HCI LE commands live in `ble_hci_le.c` and are deliberately outside this
/// crate's vendor-command surface (they are primarily supplied by `bt-hci`).
fn command_source_files(cube_dir: &Path, tag: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cube_dir)
        .args(["ls-tree", "-r", "--name-only", tag, "--", AUTO_SOURCE_DIR])
        .output()
        .map_err(|error| format!("could not list CubeWB auto sources: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-tree for {tag} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let prefix = format!("{AUTO_SOURCE_DIR}/");
    let mut files = String::from_utf8(output.stdout)
        .map_err(|error| format!("git ls-tree did not return UTF-8 file names: {error}"))?
        .lines()
        .filter_map(|path| path.strip_prefix(&prefix))
        .filter(|file| file.starts_with("ble_") && file.ends_with("_aci.c"))
        .filter(|file| *file != "ble_hci_le.c")
        .map(str::to_owned)
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "no generated vendor ACI C sources were found at {tag}"
        ));
    }
    Ok(files)
}

fn verify_tag(cube_dir: &Path, tag: &str) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cube_dir)
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("refs/tags/{tag}^{{commit}}"))
        .output()
        .map_err(|error| format!("could not run git for {}: {error}", cube_dir.display()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "STM32CubeWB tag {tag:?} was not found in {}",
            cube_dir.display()
        ))
    }
}

fn git_show(cube_dir: &Path, tag: &str, path: &str) -> Result<String, String> {
    let output = git_show_bytes(cube_dir, tag, path)?;
    String::from_utf8(output)
        .map_err(|error| format!("git show {tag}:{path} did not return UTF-8 source: {error}"))
}

/// Some older CubeWB SHCI headers contain legacy single-byte characters in
/// comments. Replacing those bytes is safe for syntax extraction while
/// keeping the generated BLE-source adapter strict about its input encoding.
fn git_show_lossy_utf8(cube_dir: &Path, tag: &str, path: &str) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git_show_bytes(cube_dir, tag, path)?).into_owned())
}

fn git_show_bytes(cube_dir: &Path, tag: &str, path: &str) -> Result<Vec<u8>, String> {
    let spec = format!("{tag}:{path}");
    let output = Command::new("git")
        .arg("-C")
        .arg(cube_dir)
        .arg("show")
        .arg(&spec)
        .output()
        .map_err(|error| format!("could not run git show {spec}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git show {spec} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

/// Extract `rq.ocf` assignments from an auto-generated ACI C source file.
#[cfg(test)]
pub fn extract_vendor_commands(
    source: &str,
    source_name: &str,
) -> Result<Vec<CoverageEntry>, String> {
    Ok(
        extract_command_metadata(source, source_name, CommandScope::VendorAci)?
            .into_iter()
            .map(|command| {
                CoverageEntry::new(
                    command.ocf(),
                    command.name,
                    CoverageOrigin::VendorAutoSource,
                )
            })
            .collect(),
    )
}

/// Extract the event code/function pairs from `hci_vs_event_table`.
#[cfg(test)]
pub fn extract_vendor_events(source: &str) -> Result<Vec<CoverageEntry>, String> {
    Ok(
        extract_event_table(source, "hci_vs_event_table", EventScope::VendorAci)?
            .into_iter()
            .map(|event| {
                CoverageEntry::new(event.code, event.name, CoverageOrigin::VendorAutoSource)
            })
            .collect(),
    )
}

/// Parse one generated C source file with the grammar used for all C
/// extraction. Tree-sitter intentionally remains tolerant of unrelated
/// preprocessor extensions: callers validate the specific declarations they
/// need, rather than rejecting a whole generated file because an unrelated
/// extension has an error node.
#[cfg(test)]
fn parse_c_tree(source: &str, source_name: &str) -> Result<Tree, String> {
    let preprocessed = preprocess_c_source(source, source_name)?;
    parse_preprocessed_c_tree(&preprocessed, source_name)
}

fn parse_preprocessed_c_tree(preprocessed: &str, source_name: &str) -> Result<Tree, String> {
    let mut parser = Parser::new();
    let language = tree_sitter_c::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|error| format!("{source_name}: could not load C grammar: {error}"))?;
    parser
        .parse(preprocessed, None)
        .ok_or_else(|| format!("{source_name}: C parser did not return a syntax tree"))
}

fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes())
        .expect("tree-sitter nodes always refer to the parsed UTF-8 source")
}

fn collect_nodes<'tree>(node: Node<'tree>, kind: &str, nodes: &mut Vec<Node<'tree>>) {
    if node.kind() == kind {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes(child, kind, nodes);
    }
}

/// Return the identifier at the core of a C declarator. Array, function,
/// pointer, and attributed declarators all wrap their actual identifier in a
/// `declarator` field, so this avoids reconstructing C declarator syntax by
/// hand.
fn declarator_identifier(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" => Some(node),
        _ => node
            .child_by_field_name("declarator")
            .and_then(declarator_identifier),
    }
}

fn declarator_has_pointer(node: Node<'_>) -> bool {
    node.kind() == "pointer_declarator"
        || node
            .child_by_field_name("declarator")
            .is_some_and(declarator_has_pointer)
}

fn declarator_is_scalar(node: Node<'_>) -> bool {
    match node.kind() {
        "identifier" | "field_identifier" => true,
        "pointer_declarator" | "array_declarator" | "function_declarator" => false,
        _ => node
            .child_by_field_name("declarator")
            .is_some_and(declarator_is_scalar),
    }
}

fn field_expression_is(node: Node<'_>, source: &str, receiver: &str, field: &str) -> bool {
    node.kind() == "field_expression"
        && node
            .child_by_field_name("argument")
            .is_some_and(|argument| node_text(argument, source).trim() == receiver)
        && node
            .child_by_field_name("field")
            .is_some_and(|member| node_text(member, source).trim() == field)
}

/// Find simple `receiver.field = value` assignments in source order. The AST
/// avoids treating comments, strings, comparisons, or a similarly named local
/// variable as transport metadata.
fn assignment_values<'tree>(
    body: Node<'tree>,
    source: &str,
    receiver: &str,
    field: &str,
) -> Vec<(Node<'tree>, Node<'tree>)> {
    let mut assignments = Vec::new();
    collect_nodes(body, "assignment_expression", &mut assignments);
    assignments
        .into_iter()
        .filter_map(|assignment| {
            let operator = assignment.child_by_field_name("operator")?;
            (node_text(operator, source) == "=")
                .then_some(())
                .filter(|_| {
                    assignment
                        .child_by_field_name("left")
                        .is_some_and(|left| field_expression_is(left, source, receiver, field))
                })
                .and_then(|_| assignment.child_by_field_name("right"))
                .map(|value| (assignment, value))
        })
        .collect()
}

fn assignment_value<'tree>(
    body: Node<'tree>,
    source: &str,
    receiver: &str,
    field: &str,
) -> Option<Node<'tree>> {
    assignment_values(body, source, receiver, field)
        .into_iter()
        .next()
        .map(|(_, value)| value)
}

fn assignment_integer(body: Node<'_>, source: &str, member: &str) -> Option<u16> {
    let value = assignment_value(body, source, "rq", member)?;
    parse_complete_c_integer(node_text(value, source))
}

fn generated_command_prefix(scope: CommandScope) -> &'static str {
    match scope {
        CommandScope::VendorAci => "aci_",
        CommandScope::StandardHci => "hci_",
    }
}

fn validate_command_metadata_assignments(
    body: Node<'_>,
    source: &str,
    source_name: &str,
    command: &str,
) -> Result<(), String> {
    for member in ["ogf", "ocf", "event", "clen", "rlen"] {
        let assignments = assignment_values(body, source, "rq", member);
        if assignments.len() > 1 {
            return Err(format!(
                "{source_name}: generated command `{command}` assigns rq.{member} {} times",
                assignments.len()
            ));
        }
        if assignments
            .first()
            .is_some_and(|(assignment, _)| nested_in_dynamic_control_flow(*assignment, body))
        {
            return Err(format!(
                "{source_name}: generated command `{command}` assigns rq.{member} inside dynamic control flow"
            ));
        }
    }
    Ok(())
}

/// Extract generated command functions from `function_definition` AST nodes.
/// This is deliberately structural: a local `tBleStatus status`, a comment,
/// or a string literal can no longer be mistaken for a command declaration.
#[cfg(test)]
pub(crate) fn extract_command_metadata(
    source: &str,
    source_name: &str,
    scope: CommandScope,
) -> Result<Vec<CatalogCommand>, String> {
    extract_command_metadata_with_evidence(source, source_name, scope, &PackedLayouts::new())
}

#[cfg(test)]
fn extract_command_metadata_with_evidence(
    source: &str,
    source_name: &str,
    scope: CommandScope,
    packed_layouts: &PackedLayouts,
) -> Result<Vec<CatalogCommand>, String> {
    let tree = parse_c_tree(source, source_name)?;
    extract_command_metadata_from_tree(source, source_name, scope, packed_layouts, &tree)
}

fn extract_command_metadata_from_preprocessed(
    source: &str,
    preprocessed: &str,
    source_name: &str,
    scope: CommandScope,
    packed_layouts: &PackedLayouts,
) -> Result<Vec<CatalogCommand>, String> {
    let tree = parse_preprocessed_c_tree(preprocessed, source_name)?;
    extract_command_metadata_from_tree(source, source_name, scope, packed_layouts, &tree)
}

fn extract_command_metadata_from_tree(
    source: &str,
    source_name: &str,
    scope: CommandScope,
    packed_layouts: &PackedLayouts,
    tree: &Tree,
) -> Result<Vec<CatalogCommand>, String> {
    let mut functions = Vec::new();
    collect_nodes(tree.root_node(), "function_definition", &mut functions);

    let mut commands = Vec::new();
    for function in functions {
        let Some(return_type) = function.child_by_field_name("type") else {
            continue;
        };
        if node_text(return_type, source).trim() != "tBleStatus" {
            continue;
        }
        let Some(declarator) = function.child_by_field_name("declarator") else {
            continue;
        };
        let Some(name_node) = declarator_identifier(declarator) else {
            continue;
        };
        let name = node_text(name_node, source).to_owned();
        if !name.starts_with(generated_command_prefix(scope)) {
            continue;
        }
        let Some(body) = function.child_by_field_name("body") else {
            return Err(format!(
                "{source_name}: generated command `{name}` has no function body"
            ));
        };
        if function.has_error() {
            return Err(format!(
                "{source_name}: generated command `{name}` contains C syntax errors"
            ));
        }
        validate_command_metadata_assignments(body, source, source_name, &name)?;
        let Some(ocf) = assignment_integer(body, source, "ocf") else {
            let expression = assignment_value(body, source, "rq", "ocf")
                .map(|value| node_text(value, source).trim())
                .unwrap_or("<missing>");
            return Err(format!(
                "{source_name}: generated command `{name}` has unsupported rq.ocf `{expression}`"
            ));
        };
        let kind = match scope {
            CommandScope::VendorAci => CatalogCommandKind::VendorAci { ocf },
            CommandScope::StandardHci => {
                let ogf = assignment_integer(body, source, "ogf").ok_or_else(|| {
                    format!(
                        "{source_name}: standard command `{name}` has an OCF but no literal rq.ogf"
                    )
                })?;
                if ogf > 0x3f {
                    return Err(format!(
                        "{source_name}: standard command `{name}` OGF 0x{ogf:X} exceeds six bits"
                    ));
                }
                if ocf > 0x03ff {
                    return Err(format!(
                        "{source_name}: standard command `{name}` OCF 0x{ocf:X} exceeds ten bits"
                    ));
                }
                CatalogCommandKind::StandardHci {
                    opcode: (ogf << 10) | ocf,
                }
            }
        };
        let source_offset = u32::try_from(function.start_byte()).map_err(|_| {
            format!("{source_name}: command `{name}` source offset exceeds schema range")
        })?;
        commands.push(CatalogCommand {
            kind,
            name,
            source_name: source_name.to_owned(),
            source_offset,
            completion: command_completion(body, source, packed_layouts),
            request: request_layout(declarator, body, source, packed_layouts),
        });
    }

    if commands.is_empty() {
        return Err(format!(
            "{source_name}: no generated command functions were found"
        ));
    }
    Ok(commands)
}

/// Locate one exact generated table declaration and return its initializer.
/// Matching the declarator identifier makes a forward declaration or a name
/// embedded in a comment irrelevant.
fn find_event_table_initializer<'tree>(
    root: Node<'tree>,
    source: &str,
    table_name: &str,
) -> Option<Node<'tree>> {
    let mut declarators = Vec::new();
    collect_nodes(root, "init_declarator", &mut declarators);
    declarators.into_iter().find_map(|declarator| {
        let name = declarator_identifier(declarator.child_by_field_name("declarator")?)?;
        (node_text(name, source) == table_name)
            .then_some(())
            .and_then(|_| declarator.child_by_field_name("value"))
            .filter(|value| value.kind() == "initializer_list")
    })
}

#[cfg(test)]
pub(crate) fn extract_event_table(
    source: &str,
    table_name: &str,
    scope: EventScope,
) -> Result<Vec<CatalogEvent>, String> {
    extract_event_table_with_evidence(source, table_name, scope, &PackedLayouts::new())
}

#[cfg(test)]
fn extract_event_table_with_evidence(
    source: &str,
    table_name: &str,
    scope: EventScope,
    packed_layouts: &PackedLayouts,
) -> Result<Vec<CatalogEvent>, String> {
    let tree = parse_c_tree(source, EVENT_SOURCE)?;
    extract_event_table_from_tree(source, table_name, scope, packed_layouts, &tree)
}

fn extract_event_table_from_preprocessed(
    source: &str,
    preprocessed: &str,
    table_name: &str,
    scope: EventScope,
    packed_layouts: &PackedLayouts,
) -> Result<Vec<CatalogEvent>, String> {
    let tree = parse_preprocessed_c_tree(preprocessed, EVENT_SOURCE)?;
    extract_event_table_from_tree(source, table_name, scope, packed_layouts, &tree)
}

fn extract_event_table_from_tree(
    source: &str,
    table_name: &str,
    scope: EventScope,
    packed_layouts: &PackedLayouts,
    tree: &Tree,
) -> Result<Vec<CatalogEvent>, String> {
    let table = find_event_table_initializer(tree.root_node(), source, table_name)
        .ok_or_else(|| format!("ble_events.c: {table_name} was not found or has no initializer"))?;
    if table.has_error() {
        return Err(format!(
            "ble_events.c: {table_name} initializer contains C syntax errors"
        ));
    }
    let process_layouts = if scope == EventScope::VendorAci {
        event_process_layouts(tree.root_node(), source, packed_layouts)
    } else {
        BTreeMap::new()
    };

    let mut entries = Vec::new();
    let mut cursor = table.walk();
    for entry in table.named_children(&mut cursor) {
        if entry.kind() != "initializer_list" {
            continue;
        }
        let Some(code_node) = entry.named_child(0) else {
            return Err(format!("ble_events.c: malformed {table_name} entry"));
        };
        let Some(code) = parse_complete_c_integer(node_text(code_node, source)) else {
            return Err(format!("ble_events.c: malformed {table_name} entry code"));
        };
        let Some(handler) = entry.named_child(1) else {
            return Err(format!(
                "ble_events.c: {table_name} entry has no handler name"
            ));
        };
        if handler.kind() != "identifier" {
            return Err(format!(
                "ble_events.c: {table_name} entry has no handler name"
            ));
        }
        let handler_name = node_text(handler, source);
        let kind = match scope {
            EventScope::VendorAci => {
                let payload = process_layouts
                    .get(handler_name)
                    .cloned()
                    .unwrap_or_else(|| {
                        event_payload_layout(
                            format!(
                                "{}_rp0",
                                handler_name
                                    .strip_suffix("_process")
                                    .unwrap_or(handler_name)
                            ),
                            packed_layouts,
                        )
                    });
                CatalogEventKind::VendorAci { payload }
            }
            EventScope::StandardHci => CatalogEventKind::StandardHci,
            EventScope::LeMeta => CatalogEventKind::LeMeta,
            EventScope::SystemShci => {
                return Err("shci.h events must be extracted from SHCI_SUB_EVT_CODE_t".to_owned());
            }
        };
        entries.push(CatalogEvent {
            kind,
            code,
            name: handler_name.to_owned(),
            source_name: EVENT_SOURCE.to_owned(),
            source_offset: u32::try_from(handler.start_byte()).map_err(|_| {
                format!("ble_events.c: {table_name} entry source offset exceeds schema range")
            })?,
        });
    }

    if entries.is_empty() {
        return Err(format!("ble_events.c: {table_name} contains no entries"));
    }
    Ok(entries)
}

/// Extract the system-channel event inventory from CubeWB's tagged SHCI
/// header. Unlike BLE ACI events, SHCI events are declared as a C enum whose
/// later values rely on implicit incrementing.
fn extract_shci_events(source: &str) -> Result<Vec<CatalogEvent>, String> {
    const ENUM_NAME: &str = "SHCI_SUB_EVT_CODE_t";
    const BASE_NAME: &str = "SHCI_SUB_EVT_CODE_BASE";

    let tree = parse_preprocessed_c_tree(source, "shci.h")?;
    let base = preprocessor_integer(tree.root_node(), source, BASE_NAME)
        .ok_or_else(|| format!("shci.h: {BASE_NAME} is missing or is not an integer literal"))?;
    let enumeration = find_enum_body(tree.root_node(), source, ENUM_NAME)
        .ok_or_else(|| format!("shci.h: {ENUM_NAME} was not found"))?;
    if enumeration.has_error() {
        return Err(format!("shci.h: {ENUM_NAME} contains C syntax errors"));
    }

    let layouts = parse_packed_struct_envelopes_with_marker(
        source,
        source,
        "shci.h",
        "PACKED_STRUCT",
        "struct       ",
    );
    let mut events = Vec::new();
    let mut previous = None;
    let mut cursor = enumeration.walk();
    for enumerator in enumeration.named_children(&mut cursor) {
        if enumerator.kind() != "enumerator" {
            continue;
        }
        let name_node = enumerator
            .child_by_field_name("name")
            .ok_or_else(|| format!("shci.h: malformed {ENUM_NAME} enumerator"))?;
        let name = node_text(name_node, source);
        let code = match enumerator.child_by_field_name("value") {
            Some(value) => {
                let expression = node_text(value, source).trim();
                if expression == BASE_NAME {
                    base
                } else {
                    parse_complete_c_integer(expression).ok_or_else(|| {
                        format!(
                            "shci.h: {ENUM_NAME} value for {name} uses unsupported expression `{expression}`"
                        )
                    })?
                }
            }
            None => previous
                .and_then(|value: u16| value.checked_add(1))
                .ok_or_else(|| format!("shci.h: {name} has no preceding enum value"))?,
        };
        previous = Some(code);
        events.push(CatalogEvent {
            kind: CatalogEventKind::SystemShci {
                payload: shci_event_payload(name, &layouts),
            },
            code,
            name: name.to_owned(),
            source_name: SHCI_SOURCE.to_owned(),
            source_offset: u32::try_from(name_node.start_byte())
                .map_err(|_| format!("shci.h: {name} source offset exceeds schema range"))?,
        });
    }

    if events.is_empty() {
        return Err(format!("shci.h: {ENUM_NAME} contains no entries"));
    }
    Ok(events)
}

fn find_enum_body<'tree>(root: Node<'tree>, source: &str, name: &str) -> Option<Node<'tree>> {
    let mut definitions = Vec::new();
    collect_nodes(root, "type_definition", &mut definitions);
    definitions.into_iter().find_map(|definition| {
        let mut cursor = definition.walk();
        let matches = definition
            .children_by_field_name("declarator", &mut cursor)
            .filter_map(declarator_identifier)
            .any(|identifier| node_text(identifier, source) == name);
        if !matches {
            return None;
        }
        definition
            .child_by_field_name("type")
            .filter(|node| node.kind() == "enum_specifier")?
            .child_by_field_name("body")
    })
}

fn preprocessor_integer(root: Node<'_>, source: &str, name: &str) -> Option<u16> {
    let mut definitions = Vec::new();
    collect_nodes(root, "preproc_def", &mut definitions);
    definitions.into_iter().find_map(|definition| {
        let identifier = definition.child_by_field_name("name")?;
        (node_text(identifier, source) == name).then_some(())?;
        let value = node_text(definition.child_by_field_name("value")?, source).trim();
        parse_parenthesized_c_integer(value)
    })
}

fn parse_parenthesized_c_integer(mut expression: &str) -> Option<u16> {
    expression = expression.trim();
    while expression.starts_with('(') && expression.ends_with(')') {
        expression = expression[1..expression.len() - 1].trim();
    }
    parse_complete_c_integer(expression)
}

fn shci_event_payload(name: &str, layouts: &PackedLayouts) -> WireLayoutEvidence {
    let fixed_struct = |type_name: &str| {
        fixed_packed_size(layouts, type_name).map_or_else(
            || {
                WireLayoutEvidence::Unresolved(format!(
                    "packed type `{type_name}` was not resolved"
                ))
            },
            |size| WireLayoutEvidence::fixed(size as u32),
        )
    };
    match name {
        // CubeWB transports these enum-valued SHCI payloads as one byte. The
        // C enum's host ABI size is not its wire size.
        "SHCI_SUB_EVT_CODE_READY" | "SHCI_SUB_EVT_ERROR_NOTIF" => WireLayoutEvidence::fixed(1),
        "SHCI_SUB_EVT_BLE_NVM_RAM_UPDATE" => fixed_struct("SHCI_C2_BleNvmRamUpdate_Evt_t"),
        "SHCI_SUB_EVT_THREAD_NVM_RAM_UPDATE" => fixed_struct("SHCI_C2_ThreadNvmRamUpdate_Evt_t"),
        "SHCI_SUB_EVT_NVM_START_WRITE" => fixed_struct("SHCI_C2_NvmStartWrite_Evt_t"),
        "SHCI_SUB_EVT_NVM_END_WRITE" => WireLayoutEvidence::fixed(0),
        "SHCI_SUB_EVT_NVM_START_ERASE" => fixed_struct("SHCI_C2_NvmStartErase_Evt_t"),
        "SHCI_SUB_EVT_NVM_END_ERASE" => WireLayoutEvidence::fixed(0),
        _ => WireLayoutEvidence::Unresolved(format!(
            "shci.h does not declare a payload structure for `{name}`"
        )),
    }
}

fn event_process_layouts(
    root: Node<'_>,
    source: &str,
    packed_layouts: &PackedLayouts,
) -> BTreeMap<String, WireLayoutEvidence> {
    let mut functions = Vec::new();
    collect_nodes(root, "function_definition", &mut functions);
    functions
        .into_iter()
        .filter_map(|function| {
            let declarator = function.child_by_field_name("declarator")?;
            let name = node_text(declarator_identifier(declarator)?, source).to_owned();
            if !name.ends_with("_event_process") {
                return None;
            }
            let body = function.child_by_field_name("body")?;
            let type_name = c_pointer_variable_type(body, source, "rp0");
            let callback_name = name.strip_suffix("_process").unwrap_or(&name);
            let layout = type_name.map_or_else(
                || {
                    let derived_type = format!("{callback_name}_rp0");
                    if packed_layouts.contains_key(&derived_type) {
                        event_payload_layout(derived_type, packed_layouts)
                    } else if body_calls_without_arguments(body, source, callback_name) {
                        WireLayoutEvidence::fixed(0)
                    } else {
                        event_payload_layout(derived_type, packed_layouts)
                    }
                },
                |type_name| event_payload_layout(type_name, packed_layouts),
            );
            Some((name, layout))
        })
        .collect()
}

fn body_calls_without_arguments(body: Node<'_>, source: &str, function_name: &str) -> bool {
    let mut calls = Vec::new();
    collect_nodes(body, "call_expression", &mut calls);
    let matching = calls
        .into_iter()
        .filter_map(|call| {
            let function = call.child_by_field_name("function")?;
            (function.kind() == "identifier" && node_text(function, source) == function_name)
                .then(|| call.child_by_field_name("arguments"))
                .flatten()
        })
        .collect::<Vec<_>>();
    matches!(matching.as_slice(), [arguments] if arguments.named_child_count() == 0)
}

fn c_pointer_variable_type(body: Node<'_>, source: &str, variable: &str) -> Option<String> {
    let mut declarations = Vec::new();
    collect_nodes(body, "declaration", &mut declarations);
    declarations.into_iter().find_map(|declaration| {
        let type_node = declaration.child_by_field_name("type")?;
        let mut cursor = declaration.walk();
        declaration
            .children_by_field_name("declarator", &mut cursor)
            .find_map(|declarator| {
                let name = declarator_identifier(declarator)?;
                (node_text(name, source) == variable && declarator_has_pointer(declarator))
                    .then(|| node_text(type_node, source).trim().to_owned())
            })
    })
}

fn request_structure_types(body: Node<'_>, source: &str) -> Result<Vec<String>, String> {
    let mut declarations = Vec::new();
    collect_nodes(body, "declaration", &mut declarations);
    let mut structures = BTreeMap::<usize, (usize, String, String)>::new();
    for declaration in declarations {
        let Some(type_node) = declaration.child_by_field_name("type") else {
            continue;
        };
        let mut cursor = declaration.walk();
        for declarator in declaration.children_by_field_name("declarator", &mut cursor) {
            let Some(identifier) = declarator_identifier(declarator) else {
                continue;
            };
            let name = node_text(identifier, source);
            let Some(suffix) = name.strip_prefix("cp") else {
                continue;
            };
            if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
                continue;
            }
            let index = suffix
                .parse::<usize>()
                .map_err(|_| format!("request structure pointer `{name}` has an invalid index"))?;
            if name != format!("cp{index}") {
                return Err(format!(
                    "request structure pointer `{name}` does not use a canonical cpN index"
                ));
            }
            if !declarator_has_pointer(declarator) {
                return Err(format!("request structure `{name}` is not a pointer"));
            }
            let type_name = node_text(type_node, source).trim().to_owned();
            if structures
                .insert(
                    index,
                    (declaration.start_byte(), name.to_owned(), type_name),
                )
                .is_some()
            {
                return Err(format!(
                    "request structure pointer index cp{index} is declared more than once"
                ));
            }
        }
    }

    let mut previous_offset = None;
    let mut types = Vec::with_capacity(structures.len());
    for (expected, (index, (offset, name, type_name))) in structures.into_iter().enumerate() {
        if index != expected {
            return Err(format!(
                "request structure pointers are not contiguous: expected cp{expected}, found `{name}`"
            ));
        }
        if previous_offset.is_some_and(|previous| offset < previous) {
            return Err(format!(
                "request structure pointer `{name}` is declared out of wire order"
            ));
        }
        previous_offset = Some(offset);
        types.push(type_name);
    }
    Ok(types)
}

enum ParsedCompletion {
    CommandComplete,
    CommandStatus,
    Event(u8),
    Unresolved(String),
}

fn parsed_completion(body: Node<'_>, source: &str) -> ParsedCompletion {
    match assignment_value(body, source, "rq", "event") {
        None => ParsedCompletion::CommandComplete,
        Some(value) => match parse_complete_c_integer(node_text(value, source)) {
            Some(0x0e) => ParsedCompletion::CommandComplete,
            Some(0x0f) => ParsedCompletion::CommandStatus,
            Some(value) if value <= u16::from(u8::MAX) => ParsedCompletion::Event(value as u8),
            Some(value) => ParsedCompletion::Unresolved(format!("0x{value:04X}")),
            None => ParsedCompletion::Unresolved(node_text(value, source).trim().to_owned()),
        },
    }
}

fn command_completion(
    body: Node<'_>,
    source: &str,
    packed_layouts: &PackedLayouts,
) -> CatalogCompletion {
    match parsed_completion(body, source) {
        ParsedCompletion::CommandComplete => CatalogCompletion::CommandComplete {
            returns: return_layout(body, source, packed_layouts),
        },
        ParsedCompletion::CommandStatus => CatalogCompletion::CommandStatus {},
        ParsedCompletion::Event(code) => CatalogCompletion::Event { code },
        ParsedCompletion::Unresolved(expression) => CatalogCompletion::Unresolved { expression },
    }
}

fn index_input_terms<'tree>(
    body: Node<'tree>,
    source: &str,
    before: usize,
) -> Vec<(Node<'tree>, Node<'tree>)> {
    let mut assignments = Vec::new();
    collect_nodes(body, "assignment_expression", &mut assignments);
    assignments
        .into_iter()
        .filter_map(|assignment| {
            if assignment.end_byte() > before {
                return None;
            }
            let operator = assignment.child_by_field_name("operator")?;
            let left = assignment.child_by_field_name("left")?;
            (node_text(operator, source) == "+="
                && left.kind() == "identifier"
                && node_text(left, source) == "index_input")
                .then(|| {
                    assignment
                        .child_by_field_name("right")
                        .map(|right| (assignment, right))
                })
                .flatten()
        })
        .collect()
}

fn nested_in_dynamic_control_flow(mut node: Node<'_>, body: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent == body {
            return false;
        }
        if matches!(
            parent.kind(),
            "if_statement"
                | "switch_statement"
                | "case_statement"
                | "for_statement"
                | "while_statement"
                | "do_statement"
        ) {
            return true;
        }
        node = parent;
    }
    true
}

fn nested_in_case_statement(mut node: Node<'_>, body: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent == body {
            return false;
        }
        if parent.kind() == "case_statement" {
            return true;
        }
        node = parent;
    }
    false
}

/// Attainable non-negative values within the one-octet HCI command payload.
/// Retaining the set preserves strides such as `18 + 7 * count`, whose
/// greatest valid value is 249 rather than 255.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RequestValues(BTreeSet<usize>);

impl RequestValues {
    fn exact(value: usize) -> Option<Self> {
        (value <= usize::from(u8::MAX)).then(|| Self(BTreeSet::from([value])))
    }

    fn unsigned() -> Self {
        Self((0..=usize::from(u8::MAX)).collect())
    }

    fn combine(self, other: Self, operation: impl Fn(usize, usize) -> Option<usize>) -> Self {
        let mut values = BTreeSet::new();
        for left in self.0 {
            for &right in &other.0 {
                if let Some(value) = operation(left, right)
                    && value <= usize::from(u8::MAX)
                {
                    values.insert(value);
                }
            }
        }
        Self(values)
    }

    fn add(self, other: Self) -> Self {
        self.combine(other, usize::checked_add)
    }

    fn union(mut self, other: Self) -> Self {
        self.0.extend(other.0);
        self
    }

    fn envelope(&self) -> Option<(usize, usize)> {
        Some((*self.0.first()?, *self.0.last()?))
    }
}

struct RequestExpressionContext<'a, 'tree> {
    function_declarator: Node<'tree>,
    body: Node<'tree>,
    source: &'a str,
    before: usize,
    packed_layouts: &'a PackedLayouts,
}

fn request_layout(
    function_declarator: Node<'_>,
    body: Node<'_>,
    source: &str,
    packed_layouts: &PackedLayouts,
) -> WireLayoutEvidence {
    let envelope = request_envelope(function_declarator, body, source, packed_layouts);
    let type_names = match request_structure_types(body, source) {
        Ok(type_names) => type_names,
        Err(reason) => return WireLayoutEvidence::Unresolved(reason),
    };
    if type_names.is_empty() {
        return envelope;
    }
    let packed_segments = type_names
        .iter()
        .map(|type_name| normalized_packed_layout(type_name, packed_layouts))
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .and_then(|layouts| {
            layouts
                .into_iter()
                .try_fold(Vec::new(), |mut segments, layout| {
                    segments.extend(layout.into_segments()?);
                    Some(segments)
                })
        });
    let Some(packed_segments) = packed_segments else {
        return envelope;
    };
    let packed = WireLayout::from_segments(packed_segments)
        .expect("resolved packed C fields form a valid wire layout");
    let type_description = type_names.join(" + ");
    match envelope {
        Evidence::Known(layout) => {
            let declared = layout.envelope();
            let storage = packed.envelope();
            if storage.minimum() == declared.minimum() {
                let safe_envelope = Envelope::bounded(
                    declared.minimum(),
                    declared.maximum().min(storage.maximum()),
                );
                return WireLayout::with_envelope(
                    safe_envelope,
                    packed
                        .into_segments()
                        .expect("packed layouts retain their field schema"),
                )
                .map(Evidence::Known)
                .expect("the packed capacity covers the normalized request envelope");
            }
            let minimum_delta = declared.minimum().checked_sub(storage.minimum());
            let maximum_delta = declared.maximum().checked_sub(storage.maximum());
            if let (Some(minimum_delta), Some(maximum_delta), Some(mut segments)) =
                (minimum_delta, maximum_delta, packed.into_segments())
                && minimum_delta == maximum_delta
            {
                if minimum_delta != 0 {
                    segments.push(WireSegment::fixed(minimum_delta));
                }
                return WireLayout::with_envelope(declared, segments)
                    .map(Evidence::Known)
                    .unwrap_or_else(|| {
                        Evidence::Unresolved(format!(
                            "packed C structures `{type_description}` cannot represent rq.clen {declared}"
                        ))
                    });
            }
            Evidence::Unresolved(format!(
                "packed C structures `{type_description}` are {storage}, but rq.clen evaluates to {declared}",
            ))
        }
        Evidence::Unresolved(reason) => Evidence::Unresolved(reason),
    }
}

fn request_envelope(
    function_declarator: Node<'_>,
    body: Node<'_>,
    source: &str,
    packed_layouts: &PackedLayouts,
) -> WireLayoutEvidence {
    let Some(value) = assignment_value(body, source, "rq", "clen") else {
        return WireLayoutEvidence::fixed(0);
    };
    let value_text = node_text(value, source).trim();
    if let Some((size, end)) = parse_c_integer(value_text, 0)
        && value_text[end..].trim().is_empty()
    {
        return WireLayoutEvidence::fixed(u32::from(size));
    }
    if expression_identifier(value)
        .is_some_and(|identifier| node_text(identifier, source).trim() == "index_input")
    {
        let terms = index_input_terms(body, source, value.start_byte());
        let formula = terms
            .iter()
            .map(|(_, term)| node_text(*term, source).trim())
            .collect::<Vec<_>>()
            .join(" + ");
        // Summing terms from both sides of a conditional would manufacture a
        // length that the generated wrapper never emits. Keep such control
        // flow explicit until it has a dedicated evaluator.
        if terms.is_empty()
            || terms
                .iter()
                .any(|(assignment, _)| nested_in_dynamic_control_flow(*assignment, body))
        {
            return WireLayoutEvidence::Unresolved(formula);
        }
        let context = RequestExpressionContext {
            function_declarator,
            body,
            source,
            before: value.start_byte(),
            packed_layouts,
        };
        let mut resolving = BTreeSet::new();
        let total = terms.iter().try_fold(
            RequestValues::exact(0).expect("zero fits in an HCI payload"),
            |total, (_, term)| {
                Some(total.add(request_expression_values(*term, &context, &mut resolving)?))
            },
        );
        let Some(total) = total else {
            return WireLayoutEvidence::Unresolved(formula);
        };
        let Some((minimum, maximum)) = total.envelope() else {
            return WireLayoutEvidence::Unresolved(formula);
        };
        let (Ok(minimum), Ok(maximum)) = (u32::try_from(minimum), u32::try_from(maximum)) else {
            return WireLayoutEvidence::Unresolved(formula);
        };
        return if minimum == maximum {
            WireLayoutEvidence::fixed(maximum)
        } else {
            WireLayoutEvidence::known(minimum, maximum)
        };
    }
    WireLayoutEvidence::Unresolved(value_text.to_owned())
}

fn request_expression_values(
    node: Node<'_>,
    context: &RequestExpressionContext<'_, '_>,
    resolving: &mut BTreeSet<String>,
) -> Option<RequestValues> {
    match node.kind() {
        "number_literal" => {
            let text = node_text(node, context.source).trim();
            let (value, end) = parse_c_integer(text, 0)?;
            text[end..]
                .trim()
                .is_empty()
                .then(|| RequestValues::exact(usize::from(value)))
                .flatten()
        }
        "identifier" => {
            request_identifier_interval(node_text(node, context.source), context, resolving)
        }
        "parenthesized_expression" | "cast_expression" => node
            .child_by_field_name("value")
            .or_else(|| node.named_child(0))
            .and_then(|inner| request_expression_values(inner, context, resolving)),
        "sizeof_expression" => {
            // Tree-sitter parses a typedef name as a value identifier when
            // this generated `.c` file is read without its included headers.
            // Accept either grammar shape, then resolve only names whose size
            // is independently proven by the packed type catalog.
            let subject = node
                .child_by_field_name("type")
                .or_else(|| node.child_by_field_name("value"))?;
            let type_name = node_text(subject, context.source)
                .trim()
                .trim_start_matches('(')
                .trim_end_matches(')')
                .trim();
            primitive_c_size(type_name)
                .or_else(|| fixed_packed_size(context.packed_layouts, type_name))
                .and_then(RequestValues::exact)
        }
        "binary_expression" => {
            let left =
                request_expression_values(node.child_by_field_name("left")?, context, resolving)?;
            let right =
                request_expression_values(node.child_by_field_name("right")?, context, resolving)?;
            match node_text(node.child_by_field_name("operator")?, context.source) {
                "+" => Some(left.combine(right, usize::checked_add)),
                "*" => Some(left.combine(right, usize::checked_mul)),
                // Truncating a u16 input to the HCI-sized domain is sound for
                // monotone addition and multiplication. Subtraction and
                // division could make an input above 255 relevant again, so
                // those expressions deliberately remain unresolved.
                _ => None,
            }
        }
        "conditional_expression" => {
            let consequence = request_expression_values(
                node.child_by_field_name("consequence")?,
                context,
                resolving,
            )?;
            let alternative = request_expression_values(
                node.child_by_field_name("alternative")?,
                context,
                resolving,
            )?;
            Some(consequence.union(alternative))
        }
        _ => None,
    }
}

fn request_identifier_interval(
    identifier: &str,
    context: &RequestExpressionContext<'_, '_>,
    resolving: &mut BTreeSet<String>,
) -> Option<RequestValues> {
    match identifier.trim() {
        "BLE_CMD_MAX_PARAM_LEN" | "BLE_EVT_MAX_PARAM_LEN" => {
            return RequestValues::exact(usize::from(u8::MAX));
        }
        _ => {}
    }
    let identifier = identifier.trim().to_owned();
    if !resolving.insert(identifier.clone()) {
        return None;
    }

    let mut initializers = Vec::new();
    collect_nodes(context.body, "init_declarator", &mut initializers);
    let initializer_values = initializers
        .into_iter()
        .filter_map(|declaration| {
            (declaration.end_byte() <= context.before
                && declarator_identifier(declaration.child_by_field_name("declarator")?)
                    .is_some_and(|name| node_text(name, context.source) == identifier))
            .then(|| declaration.child_by_field_name("value"))
            .flatten()
        })
        .collect::<Vec<_>>();
    let initialized = match initializer_values.as_slice() {
        [] => Some(None),
        [value] => request_expression_values(*value, context, resolving).map(Some),
        _ => None,
    };

    let mut assignments = Vec::new();
    collect_nodes(context.body, "assignment_expression", &mut assignments);
    let assignment_values = assignments
        .into_iter()
        .filter(|assignment| assignment.end_byte() <= context.before)
        .filter_map(|assignment| {
            let operator = assignment.child_by_field_name("operator")?;
            let left = assignment.child_by_field_name("left")?;
            (node_text(operator, context.source) == "="
                && left.kind() == "identifier"
                && node_text(left, context.source) == identifier)
                .then(|| {
                    assignment
                        .child_by_field_name("right")
                        .map(|right| (assignment, right))
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    let assignments_have_supported_flow = assignment_values.iter().all(|(assignment, _)| {
        !nested_in_dynamic_control_flow(*assignment, context.body)
            || nested_in_case_statement(*assignment, context.body)
    }) && (assignment_values.len() <= 1
        || assignment_values
            .iter()
            .all(|(assignment, _)| nested_in_case_statement(*assignment, context.body)));
    let assigned = if assignments_have_supported_flow {
        assignment_values
            .into_iter()
            .try_fold(None::<RequestValues>, |values, (_, expression)| {
                let value = request_expression_values(expression, context, resolving)?;
                Some(Some(match values {
                    Some(values) => values.union(value),
                    None => value,
                }))
            })
    } else {
        None
    };
    // Once an initialized local is subsequently reassigned, determining its
    // final value requires control-flow analysis. Do not silently prefer the
    // initializer and understate the generated request envelope.
    let result = match (initialized, assigned) {
        (Some(Some(value)), Some(None)) | (Some(None), Some(Some(value))) => Some(value),
        (Some(None), Some(None)) => function_parameter_interval(&identifier, context),
        (Some(Some(_)), Some(Some(_))) | (None, _) | (_, None) => None,
    };

    resolving.remove(&identifier);
    result
}

fn function_parameter_interval(
    identifier: &str,
    context: &RequestExpressionContext<'_, '_>,
) -> Option<RequestValues> {
    let mut parameters = Vec::new();
    collect_nodes(
        context.function_declarator,
        "parameter_declaration",
        &mut parameters,
    );
    parameters.into_iter().find_map(|parameter| {
        let declarator = parameter.child_by_field_name("declarator")?;
        let name = declarator_identifier(declarator)?;
        if node_text(name, context.source) != identifier {
            return None;
        }
        if !declarator_is_scalar(declarator) {
            return None;
        }
        let type_name = node_text(parameter.child_by_field_name("type")?, context.source).trim();
        match type_name {
            // Values above 255 cannot contribute to a valid non-negative HCI
            // command-length formula, so both widths have the same bounded
            // attainable set here.
            "uint8_t" | "unsigned char" | "uint16_t" | "unsigned short" => {
                Some(RequestValues::unsigned())
            }
            _ => None,
        }
    })
}

fn expression_identifier(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "identifier" => Some(node),
        "parenthesized_expression" => node.named_child(0).and_then(expression_identifier),
        _ => None,
    }
}

fn sizeof_variable(node: Node<'_>) -> Option<Node<'_>> {
    (node.kind() == "sizeof_expression")
        .then(|| node.child_by_field_name("value"))
        .flatten()
        .and_then(expression_identifier)
}

fn c_variable_type(body: Node<'_>, source: &str, variable: &str) -> Option<String> {
    let mut declarations = Vec::new();
    collect_nodes(body, "declaration", &mut declarations);
    declarations.into_iter().find_map(|declaration| {
        let type_node = declaration.child_by_field_name("type")?;
        let mut cursor = declaration.walk();
        declaration
            .children_by_field_name("declarator", &mut cursor)
            .find_map(|declarator| {
                let name = declarator_identifier(declarator)?;
                (node_text(name, source) == variable && !declarator_has_pointer(declarator))
                    .then(|| node_text(type_node, source).trim().to_owned())
            })
    })
}

fn return_layout(
    body: Node<'_>,
    source: &str,
    packed_layouts: &PackedLayouts,
) -> WireLayoutEvidence {
    let Some(value) = assignment_value(body, source, "rq", "rlen") else {
        return WireLayoutEvidence::Unresolved(
            "CubeWB does not state a Command Complete response length".to_owned(),
        );
    };
    let value_text = node_text(value, source).trim();
    if let Some((size, end)) = parse_c_integer(value_text, 0)
        && value_text[end..].trim().is_empty()
    {
        return normalized_return_layout(u32::from(size), u32::from(size), false);
    }
    if let Some(variable) = sizeof_variable(value)
        && let Some(type_name) = c_variable_type(body, source, node_text(variable, source))
    {
        return return_layout_for_struct(type_name, packed_layouts);
    }
    WireLayoutEvidence::Unresolved(value_text.to_owned())
}

fn return_layout_for_struct(type_name: String, layouts: &PackedLayouts) -> WireLayoutEvidence {
    match normalized_packed_layout(&type_name, layouts) {
        Ok(layout) => normalized_struct_return_layout(layout),
        Err(reason) => WireLayoutEvidence::Unresolved(reason),
    }
}

fn normalized_struct_return_layout(layout: WireLayout) -> WireLayoutEvidence {
    let envelope = layout.envelope();
    let Some(minimum) = envelope.minimum().checked_sub(1) else {
        return WireLayoutEvidence::Unresolved(
            "CubeWB Command Complete response cannot contain its status byte".to_owned(),
        );
    };
    let Some(maximum) = envelope.maximum().checked_sub(1) else {
        return WireLayoutEvidence::Unresolved(
            "CubeWB Command Complete response cannot contain its status byte".to_owned(),
        );
    };
    let Some(mut segments) = layout.into_segments() else {
        return WireLayoutEvidence::Unresolved(
            "CubeWB Command Complete response structure has no field schema".to_owned(),
        );
    };
    let Some(WireSegment::Fixed {
        length: 1,
        role: FixedFieldRole::Status,
    }) = segments.first()
    else {
        return WireLayoutEvidence::Unresolved(
            "CubeWB Command Complete response does not start with a one-byte `Status` field"
                .to_owned(),
        );
    };
    segments.remove(0);
    WireLayout::with_envelope(Envelope::bounded(minimum, maximum), segments)
        .map(Evidence::Known)
        .unwrap_or_else(|| {
            Evidence::Unresolved(
                "CubeWB Command Complete response schema is inconsistent after removing status"
                    .to_owned(),
            )
        })
}

fn normalized_return_layout(minimum: u32, maximum: u32, variable: bool) -> WireLayoutEvidence {
    let Some(minimum) = minimum.checked_sub(1) else {
        return WireLayoutEvidence::Unresolved(
            "CubeWB Command Complete response cannot contain its status byte".to_owned(),
        );
    };
    let Some(maximum) = maximum.checked_sub(1) else {
        return WireLayoutEvidence::Unresolved(
            "CubeWB Command Complete response cannot contain its status byte".to_owned(),
        );
    };
    if variable && minimum != maximum {
        WireLayoutEvidence::known(minimum, maximum)
    } else {
        WireLayoutEvidence::fixed(maximum)
    }
}

fn event_payload_layout(type_name: String, layouts: &PackedLayouts) -> WireLayoutEvidence {
    match normalized_packed_layout(&type_name, layouts) {
        Ok(layout) => Evidence::Known(layout),
        Err(reason) => WireLayoutEvidence::Unresolved(reason),
    }
}

fn normalized_packed_layout(
    type_name: &str,
    layouts: &PackedLayouts,
) -> Result<WireLayout, String> {
    let layout = layouts
        .get(type_name)
        .cloned()
        .flatten()
        .ok_or_else(|| format!("packed C structure `{type_name}` could not be resolved"))?;
    let minimum = u32::try_from(layout.minimum)
        .map_err(|_| format!("packed C structure `{type_name}` minimum exceeds schema range"))?;
    let maximum = u32::try_from(layout.maximum)
        .map_err(|_| format!("packed C structure `{type_name}` maximum exceeds schema range"))?;
    WireLayout::with_envelope(Envelope::bounded(minimum, maximum), layout.segments)
        .ok_or_else(|| format!("packed C structure `{type_name}` has an inconsistent schema"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackedEnvelope {
    minimum: usize,
    maximum: usize,
    variable: bool,
    segments: Vec<WireSegment>,
}

type PackedLayouts = BTreeMap<String, Option<PackedEnvelope>>;

fn fixed_packed_size(layouts: &PackedLayouts, type_name: &str) -> Option<usize> {
    let layout = layouts.get(type_name).cloned().flatten()?;
    (!layout.variable && layout.minimum == layout.maximum).then_some(layout.maximum)
}

#[derive(Clone, Debug)]
struct PackedMultiplicity {
    dimensions: Vec<String>,
}

#[derive(Clone, Debug)]
struct PackedEnvelopeField {
    name: String,
    type_name: String,
    multiplicity: PackedMultiplicity,
}

#[derive(Clone, Debug)]
struct PackedEnvelopeDefinition {
    name: String,
    fields: Option<Vec<PackedEnvelopeField>>,
}

#[cfg(test)]
fn parse_packed_struct_envelopes(source: &str) -> PackedLayouts {
    let Ok(preprocessed) = preprocess_c_source(source, TYPES_SOURCE) else {
        return BTreeMap::new();
    };
    parse_packed_struct_envelopes_from_preprocessed(source, &preprocessed)
}

fn parse_packed_struct_envelopes_from_preprocessed(
    source: &str,
    preprocessed: &str,
) -> PackedLayouts {
    parse_packed_struct_envelopes_with_marker(
        source,
        preprocessed,
        TYPES_SOURCE,
        "__PACKED_STRUCT",
        "struct         ",
    )
}

fn parse_packed_struct_envelopes_with_marker(
    source: &str,
    preprocessed: &str,
    source_name: &str,
    packed_marker: &str,
    parsable_marker: &str,
) -> PackedLayouts {
    debug_assert_eq!(packed_marker.len(), parsable_marker.len());

    let normalized = source.replace(packed_marker, parsable_marker);
    let normalized_preprocessed = preprocessed.replace(packed_marker, parsable_marker);
    let Ok(tree) = parse_preprocessed_c_tree(&normalized_preprocessed, source_name) else {
        return BTreeMap::new();
    };
    let mut type_definitions = Vec::new();
    collect_nodes(tree.root_node(), "type_definition", &mut type_definitions);
    let definitions = type_definitions
        .into_iter()
        .filter_map(|definition| {
            node_text(definition, source)
                .contains(packed_marker)
                .then_some(())?;
            if definition.has_error() {
                return None;
            }
            let type_node = definition.child_by_field_name("type")?;
            if type_node.kind() != "struct_specifier" {
                return None;
            }
            let body = type_node.child_by_field_name("body")?;
            let mut cursor = definition.walk();
            let name = definition
                .children_by_field_name("declarator", &mut cursor)
                .find_map(declarator_identifier)
                .map(|name| node_text(name, &normalized).to_owned())?;
            Some(PackedEnvelopeDefinition {
                name,
                fields: packed_envelope_fields(body, &normalized),
            })
        })
        .collect::<Vec<_>>();

    let mut layouts = definitions
        .iter()
        .map(|definition| (definition.name.clone(), None))
        .collect::<BTreeMap<_, Option<PackedEnvelope>>>();
    let mut changed = true;
    while changed {
        changed = false;
        for definition in &definitions {
            let Some(fields) = &definition.fields else {
                continue;
            };
            let Some(layout) = packed_struct_envelope(fields, &layouts) else {
                continue;
            };
            if layouts.get(&definition.name) != Some(&Some(layout.clone())) {
                layouts.insert(definition.name.clone(), Some(layout));
                changed = true;
            }
        }
    }
    layouts
}

fn packed_envelope_fields(body: Node<'_>, source: &str) -> Option<Vec<PackedEnvelopeField>> {
    let mut fields = Vec::new();
    let mut cursor = body.walk();
    for declaration in body.named_children(&mut cursor) {
        if declaration.kind() == "comment" {
            continue;
        }
        if declaration.kind() != "field_declaration" {
            return None;
        }
        let type_node = declaration.child_by_field_name("type")?;
        let type_name = node_text(type_node, source).trim().to_owned();
        let mut declarator_cursor = declaration.walk();
        let declarators = declaration
            .children_by_field_name("declarator", &mut declarator_cursor)
            .collect::<Vec<_>>();
        if declarators.is_empty() {
            return None;
        }
        for declarator in declarators {
            let name = declarator_identifier(declarator)
                .map(|identifier| node_text(identifier, source).to_owned())?;
            fields.push(PackedEnvelopeField {
                name,
                type_name: type_name.clone(),
                multiplicity: packed_field_multiplicity(declarator, source)?,
            });
        }
    }
    Some(fields)
}

fn packed_field_multiplicity(declarator: Node<'_>, source: &str) -> Option<PackedMultiplicity> {
    match declarator.kind() {
        "identifier" | "field_identifier" => Some(PackedMultiplicity {
            dimensions: Vec::new(),
        }),
        "pointer_declarator" | "abstract_pointer_declarator" => None,
        "array_declarator" => {
            let size = declarator.child_by_field_name("size")?;
            let expression = node_text(size, source).trim();
            let inner = declarator.child_by_field_name("declarator")?;
            let mut multiplicity = packed_field_multiplicity(inner, source)?;
            multiplicity.dimensions.push(expression.to_owned());
            Some(multiplicity)
        }
        _ => declarator
            .child_by_field_name("declarator")
            .and_then(|inner| packed_field_multiplicity(inner, source)),
    }
}

fn expression_uses_transport_capacity(expression: &str) -> bool {
    expression
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|identifier| {
            matches!(
                identifier,
                "BLE_EVT_MAX_PARAM_LEN" | "BLE_CMD_MAX_PARAM_LEN"
            )
        })
}

fn packed_fixed_field_width(
    field: &PackedEnvelopeField,
    known: &BTreeMap<String, Option<PackedEnvelope>>,
) -> Option<u32> {
    let element_width = primitive_c_size(&field.type_name)
        .or_else(|| fixed_packed_size(known, &field.type_name))?;
    let count = field
        .multiplicity
        .dimensions
        .iter()
        .try_fold(1usize, |count, expression| {
            (!expression_uses_transport_capacity(expression))
                .then(|| evaluate_capacity_expression(expression, known))
                .flatten()
                .and_then(|dimension| count.checked_mul(dimension))
        })?;
    u32::try_from(element_width.checked_mul(count)?).ok()
}

fn normalized_c_field_name(name: &str) -> String {
    name.trim_matches('_').to_ascii_lowercase()
}

fn field_name_is_length_or_count(name: &str) -> bool {
    let name = normalized_c_field_name(name);
    name.contains("length")
        || name.ends_with("_len")
        || name.starts_with("num_")
        || name.contains("count")
}

fn inferred_capacity_semantic(
    fields: &[PackedEnvelopeField],
    index: usize,
    known: &BTreeMap<String, Option<PackedEnvelope>>,
) -> Option<VariableSemantic> {
    let length = fields.get(index.checked_sub(1)?)?;
    if !field_name_is_length_or_count(&length.name) {
        return None;
    }
    let length_width = packed_fixed_field_width(length, known)?;
    let selector = index
        .checked_sub(2)
        .and_then(|selector| fields.get(selector));
    if let Some(selector) = selector {
        let selector_name = normalized_c_field_name(&selector.name);
        let length_name = normalized_c_field_name(&length.name);
        let selector_width = packed_fixed_field_width(selector, known)?;
        if selector_name.contains("format") && length_name.contains("length") {
            return Some(VariableSemantic::TaggedItems {
                tag_width: selector_width,
                length_width,
                variants: Vec::new(),
            });
        }
        if length_name == "data_length"
            && selector_name.contains("length")
            && !selector_name.starts_with("event_data")
        {
            return Some(VariableSemantic::LengthPrefixedRecords {
                record_len_width: selector_width,
                length_width,
                minimum_record_len: None,
            });
        }
    }
    Some(VariableSemantic::Counted {
        prefix_width: length_width,
    })
}

fn packed_struct_envelope(
    fields: &[PackedEnvelopeField],
    known: &BTreeMap<String, Option<PackedEnvelope>>,
) -> Option<PackedEnvelope> {
    let mut layout = PackedEnvelope {
        minimum: 0,
        maximum: 0,
        variable: false,
        segments: Vec::new(),
    };
    for (field_index, field) in fields.iter().enumerate() {
        let primitive_size = primitive_c_size(&field.type_name);
        let element = primitive_size.map_or_else(
            || known.get(&field.type_name).cloned().flatten(),
            |size| {
                Some(PackedEnvelope {
                    minimum: size,
                    maximum: size,
                    variable: false,
                    segments: vec![WireSegment::fixed(u32::try_from(size).ok()?)],
                })
            },
        )?;
        let mut fixed_count = 1usize;
        let mut capacity_count = None;
        for expression in &field.multiplicity.dimensions {
            let count = evaluate_capacity_expression(expression, known)?;
            if expression_uses_transport_capacity(expression) {
                if capacity_count.replace(count).is_some() {
                    return None;
                }
            } else {
                fixed_count = fixed_count.checked_mul(count)?;
            }
        }
        match capacity_count {
            None => {
                layout.minimum = layout
                    .minimum
                    .checked_add(element.minimum.checked_mul(fixed_count)?)?;
                layout.maximum = layout
                    .maximum
                    .checked_add(element.maximum.checked_mul(fixed_count)?)?;
                layout.variable |= element.variable;
                if element.variable {
                    for _ in 0..fixed_count {
                        layout.segments.extend(element.segments.iter().cloned());
                    }
                } else {
                    let length = element.maximum.checked_mul(fixed_count)?;
                    if length != 0 && primitive_size.is_some() {
                        let segment = if field.name.eq_ignore_ascii_case("status") && length == 1 {
                            WireSegment::status()
                        } else {
                            WireSegment::fixed(u32::try_from(length).ok()?)
                        };
                        layout.segments.push(segment);
                    } else if length != 0 {
                        for _ in 0..fixed_count {
                            layout.segments.extend(element.segments.iter().cloned());
                        }
                    }
                }
            }
            Some(capacity_count) => {
                if element.variable || element.minimum != element.maximum {
                    return None;
                }
                let element_width = element.maximum.checked_mul(fixed_count)?;
                layout.maximum = layout
                    .maximum
                    .checked_add(element_width.checked_mul(capacity_count)?)?;
                layout.variable = true;
                let element_width = u32::try_from(element_width).ok()?;
                let capacity_count = u32::try_from(capacity_count).ok()?;
                let segment = inferred_capacity_semantic(fields, field_index, known).map_or_else(
                    || WireSegment::variable(element_width, 0, capacity_count),
                    |semantic| {
                        WireSegment::variable_with_semantic(
                            element_width,
                            0,
                            capacity_count,
                            semantic,
                        )
                    },
                );
                layout.segments.push(segment);
            }
        }
    }
    layout.segments = WireLayout::from_segments(layout.segments)?.into_segments()?;
    Some(layout)
}

fn evaluate_capacity_expression(
    expression: &str,
    known: &BTreeMap<String, Option<PackedEnvelope>>,
) -> Option<usize> {
    let mut parser = CapacityExpressionParser {
        input: expression.as_bytes(),
        index: 0,
        known,
    };
    let value = parser.expression()?;
    parser.skip_whitespace();
    (parser.index == parser.input.len()).then_some(value)
}

struct CapacityExpressionParser<'a> {
    input: &'a [u8],
    index: usize,
    known: &'a BTreeMap<String, Option<PackedEnvelope>>,
}

impl CapacityExpressionParser<'_> {
    fn expression(&mut self) -> Option<usize> {
        let mut value = self.term()?;
        loop {
            self.skip_whitespace();
            match self.input.get(self.index) {
                Some(b'+') => {
                    self.index += 1;
                    value = value.checked_add(self.term()?)?;
                }
                Some(b'-') => {
                    self.index += 1;
                    value = value.checked_sub(self.term()?)?;
                }
                _ => return Some(value),
            }
        }
    }

    fn term(&mut self) -> Option<usize> {
        let mut value = self.factor()?;
        loop {
            self.skip_whitespace();
            match self.input.get(self.index) {
                Some(b'*') => {
                    self.index += 1;
                    value = value.checked_mul(self.factor()?)?;
                }
                Some(b'/') => {
                    self.index += 1;
                    value = value.checked_div(self.factor()?)?;
                }
                _ => return Some(value),
            }
        }
    }

    fn factor(&mut self) -> Option<usize> {
        self.skip_whitespace();
        if self.input.get(self.index) == Some(&b'(') {
            self.index += 1;
            let value = self.expression()?;
            self.skip_whitespace();
            (self.input.get(self.index) == Some(&b')')).then_some(())?;
            self.index += 1;
            return Some(value);
        }
        if self.input.get(self.index).is_some_and(u8::is_ascii_digit) {
            let start = self.index;
            while self.input.get(self.index).is_some_and(u8::is_ascii_digit) {
                self.index += 1;
            }
            return core::str::from_utf8(&self.input[start..self.index])
                .ok()?
                .parse()
                .ok();
        }
        let identifier = self.identifier()?;
        if matches!(
            identifier,
            "BLE_EVT_MAX_PARAM_LEN" | "BLE_CMD_MAX_PARAM_LEN"
        ) {
            return Some(255);
        }
        if identifier != "sizeof" {
            return None;
        }
        self.skip_whitespace();
        (self.input.get(self.index) == Some(&b'(')).then_some(())?;
        self.index += 1;
        let type_name = self.identifier()?.to_owned();
        self.skip_whitespace();
        (self.input.get(self.index) == Some(&b')')).then_some(())?;
        self.index += 1;
        primitive_c_size(&type_name).or_else(|| {
            let layout = self.known.get(&type_name).cloned().flatten()?;
            (!layout.variable && layout.minimum == layout.maximum).then_some(layout.maximum)
        })
    }

    fn identifier(&mut self) -> Option<&str> {
        self.skip_whitespace();
        let start = self.index;
        while self
            .input
            .get(self.index)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            self.index += 1;
        }
        (self.index > start).then(|| core::str::from_utf8(&self.input[start..self.index]).ok())?
    }

    fn skip_whitespace(&mut self) {
        while self
            .input
            .get(self.index)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.index += 1;
        }
    }
}

fn primitive_c_size(base: &str) -> Option<usize> {
    match base.trim() {
        "uint8_t" | "int8_t" | "char" | "unsigned char" | "signed char" => Some(1),
        "uint16_t" | "int16_t" | "short" | "unsigned short" | "signed short" => Some(2),
        "uint32_t" | "int32_t" | "unsigned int" | "signed int" | "unsigned" | "int" => Some(4),
        "uint64_t" | "int64_t" | "unsigned long long" | "long long" => Some(8),
        _ => None,
    }
}

/// Parse a C integer literal, including common unsigned/long suffixes.
fn parse_c_integer(source: &str, start: usize) -> Option<(u16, usize)> {
    let bytes = source.as_bytes();
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    let literal_start = index;
    if bytes.get(index) == Some(&b'0') && matches!(bytes.get(index + 1), Some(b'x' | b'X')) {
        index += 2;
        let digits_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_hexdigit) {
            index += 1;
        }
        if digits_start == index {
            return None;
        }
    } else {
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if literal_start == index {
            return None;
        }
    }
    let number_end = index;
    while bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'u' | b'U' | b'l' | b'L'))
    {
        index += 1;
    }
    let literal = &source[literal_start..number_end];
    let value = if let Some(hex) = literal
        .strip_prefix("0x")
        .or_else(|| literal.strip_prefix("0X"))
    {
        u16::from_str_radix(hex, 16).ok()?
    } else {
        literal.parse().ok()?
    };
    Some((value, index))
}

fn parse_complete_c_integer(source: &str) -> Option<u16> {
    let (value, end) = parse_c_integer(source, 0)?;
    source[end..].trim().is_empty().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ocfs_and_function_names() {
        let source = r#"
            tBleStatus aci_gap_a(void) { rq.ocf = 0x081; }
            tBleStatus aci_gap_b(void) { rq.ocf = 130U; }
        "#;
        let commands = extract_vendor_commands(source, "gap.c").unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].code, 0x81);
        assert_eq!(commands[0].name, "aci_gap_a");
        assert_eq!(commands[1].code, 130);
    }

    #[test]
    fn integer_metadata_requires_a_complete_literal() {
        let command_source = r#"
            tBleStatus aci_fixture(void)
            {
                rq.ocf = 0x081 + 1;
            }
        "#;
        let error = extract_command_metadata(command_source, "fixture.c", CommandScope::VendorAci)
            .unwrap_err();
        assert!(error.contains("unsupported rq.ocf `0x081 + 1`"));

        let event_source = r#"
            const hci_event_table_type hci_vs_event_table[] = {
                { 0x0400 + 1, aci_fixture_event_process },
            };
        "#;
        let error = extract_event_table(event_source, "hci_vs_event_table", EventScope::VendorAci)
            .unwrap_err();
        assert!(error.contains("malformed hci_vs_event_table entry code"));

        let completion_source = r#"
            tBleStatus aci_fixture(void)
            {
                rq.ocf = 0x081;
                rq.event = 0x0E + 1;
            }
        "#;
        let commands =
            extract_command_metadata(completion_source, "fixture.c", CommandScope::VendorAci)
                .unwrap();
        assert_eq!(
            commands[0].completion,
            CatalogCompletion::Unresolved {
                expression: "0x0E + 1".to_owned(),
            }
        );
    }

    #[test]
    fn command_metadata_does_not_confuse_local_status_with_the_function_name() {
        let source = r#"
            tBleStatus aci_fixture(void)
            {
                tBleStatus status = 0;
                struct hci_request rq;
                rq.ocf = 0x081;
                rq.rparam = &status;
                rq.rlen = 1;
                return status;
            }
        "#;
        let commands =
            extract_command_metadata(source, "fixture.c", CommandScope::VendorAci).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "aci_fixture");
        assert_eq!(commands[0].ocf(), 0x81);
    }

    #[test]
    fn command_metadata_fails_closed_for_missing_or_ambiguous_assignments() {
        let missing = r#"
            tBleStatus aci_fixture(void)
            {
                struct hci_request rq;
            }
        "#;
        let error =
            extract_command_metadata(missing, "fixture.c", CommandScope::VendorAci).unwrap_err();
        assert!(error.contains("unsupported rq.ocf `<missing>`"));

        let duplicate = r#"
            tBleStatus aci_fixture(void)
            {
                struct hci_request rq;
                rq.ocf = 0x081;
                rq.ocf = 0x082;
            }
        "#;
        let error =
            extract_command_metadata(duplicate, "fixture.c", CommandScope::VendorAci).unwrap_err();
        assert!(error.contains("assigns rq.ocf 2 times"));

        let conditional = r#"
            tBleStatus aci_fixture(uint8_t select)
            {
                struct hci_request rq;
                if (select) {
                    rq.ocf = 0x081;
                }
            }
        "#;
        let error = extract_command_metadata(conditional, "fixture.c", CommandScope::VendorAci)
            .unwrap_err();
        assert!(error.contains("assigns rq.ocf inside dynamic control flow"));
    }

    #[test]
    fn standard_command_identity_is_derived_from_one_opcode() {
        let source = r#"
            tBleStatus hci_fixture(void)
            {
                struct hci_request rq;
                rq.ogf = 0x08;
                rq.ocf = 0x003;
            }
        "#;

        let commands =
            extract_command_metadata(source, "fixture.c", CommandScope::StandardHci).unwrap();
        assert_eq!(
            commands[0].kind,
            CatalogCommandKind::StandardHci { opcode: 0x2003 }
        );
        assert_eq!(commands[0].ogf(), Some(0x08));
        assert_eq!(commands[0].ocf(), 0x003);
    }

    #[test]
    fn rejects_standard_command_identity_outside_opcode_widths() {
        let oversized_ogf = r#"
            tBleStatus hci_fixture(void)
            {
                struct hci_request rq;
                rq.ogf = 0x40;
                rq.ocf = 0x001;
            }
        "#;
        let error = extract_command_metadata(oversized_ogf, "fixture.c", CommandScope::StandardHci)
            .unwrap_err();
        assert!(error.contains("OGF 0x40 exceeds six bits"));

        let oversized_ocf = r#"
            tBleStatus hci_fixture(void)
            {
                struct hci_request rq;
                rq.ogf = 0x08;
                rq.ocf = 0x400;
            }
        "#;
        let error = extract_command_metadata(oversized_ocf, "fixture.c", CommandScope::StandardHci)
            .unwrap_err();
        assert!(error.contains("OCF 0x400 exceeds ten bits"));
    }

    #[test]
    fn resolves_counted_byte_request_envelope_from_ast_and_packed_capacity() {
        let types = r#"
            typedef __PACKED_STRUCT
            {
                uint8_t Offset;
                uint8_t Length;
                uint8_t Value[BLE_CMD_MAX_PARAM_LEN - 2];
            } fixture_cp0;
        "#;
        let source = r#"
            tBleStatus aci_fixture(uint8_t Offset, uint8_t Length, const uint8_t *Value)
            {
                struct hci_request rq;
                uint8_t cmd_buffer[BLE_CMD_MAX_PARAM_LEN];
                fixture_cp0 *cp0 = (fixture_cp0 *)(cmd_buffer);
                int index_input = 0;
                cp0->Offset = Offset;
                index_input += 1;
                cp0->Length = Length;
                index_input += 1;
                Osal_MemCpy((void *)&cp0->Value, (const void *)Value, Length);
                index_input += Length;
                rq.ocf = 0x081;
                rq.clen = index_input;
            }
        "#;

        let commands = extract_command_metadata_with_evidence(
            source,
            "fixture.c",
            CommandScope::VendorAci,
            &parse_packed_struct_envelopes(types),
        )
        .unwrap();
        assert_eq!(commands[0].request.bounds(), Some((2, 255)));
        let Evidence::Known(layout) = &commands[0].request else {
            panic!("expected resolved request layout");
        };
        assert_eq!(
            layout.segments(),
            Some(
                [
                    WireSegment::fixed(1),
                    WireSegment::fixed(1),
                    WireSegment::variable_with_semantic(
                        1,
                        0,
                        253,
                        VariableSemantic::Counted { prefix_width: 1 },
                    ),
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn request_structure_discovery_rejects_gaps_and_has_no_fixed_limit() {
        let types = r#"
            typedef __PACKED_STRUCT
            {
                uint8_t Value;
            } fixture_cp;
        "#;
        let gap = r#"
            typedef uint8_t fixture_cp;

            tBleStatus aci_fixture(void)
            {
                struct hci_request rq;
                fixture_cp *cp0 = 0;
                fixture_cp *cp2 = 0;
                rq.ocf = 0x081;
                rq.clen = 2;
            }
        "#;
        let commands = extract_command_metadata_with_evidence(
            gap,
            "fixture.c",
            CommandScope::VendorAci,
            &parse_packed_struct_envelopes(types),
        )
        .unwrap();
        let Evidence::Unresolved(reason) = &commands[0].request else {
            panic!("a cpN gap must leave request structure unresolved");
        };
        assert!(reason.contains("expected cp1, found `cp2`"));

        let declarations = (0..17)
            .map(|index| format!("fixture_cp *cp{index} = 0;"))
            .collect::<Vec<_>>()
            .join("\n");
        let source = format!(
            r#"
                typedef uint8_t fixture_cp;

                tBleStatus aci_fixture(void)
                {{
                    struct hci_request rq;
                    {declarations}
                    rq.ocf = 0x081;
                    rq.clen = 17;
                }}
            "#
        );
        let commands = extract_command_metadata_with_evidence(
            &source,
            "fixture.c",
            CommandScope::VendorAci,
            &parse_packed_struct_envelopes(types),
        )
        .unwrap();
        assert_eq!(commands[0].request.bounds(), Some((17, 17)));
        let Evidence::Known(layout) = &commands[0].request else {
            panic!("all cpN structures should resolve");
        };
        let segments = layout.segments().expect("cpN fields retain boundaries");
        assert_eq!(segments.len(), 17);
        assert!(
            segments
                .iter()
                .all(|segment| segment == &WireSegment::fixed(1))
        );
    }

    #[test]
    fn resolves_counted_item_capacity_with_sizeof_without_rounding_up() {
        let types = r#"
            typedef __PACKED_STRUCT
            {
                uint8_t Address_Type;
                uint8_t Address[6];
            } Item_t;

            typedef __PACKED_STRUCT
            {
                uint8_t Count;
                Item_t Items[(BLE_CMD_MAX_PARAM_LEN - 2) / sizeof(Item_t)];
            } fixture_cp0;
        "#;
        let source = r#"
            tBleStatus aci_fixture(uint8_t Count, const Item_t *Items, uint8_t Mode)
            {
                struct hci_request rq;
                uint8_t cmd_buffer[BLE_CMD_MAX_PARAM_LEN];
                fixture_cp0 *cp0 = (fixture_cp0 *)(cmd_buffer);
                int index_input = 0;
                cp0->Count = Count;
                index_input += 1;
                Osal_MemCpy((void *)&cp0->Items, (const void *)Items,
                            Count * (sizeof(Item_t)));
                index_input += Count * (sizeof(Item_t));
                index_input += 1;
                rq.ocf = 0x081;
                rq.clen = index_input;
            }
        "#;

        let commands = extract_command_metadata_with_evidence(
            source,
            "fixture.c",
            CommandScope::VendorAci,
            &parse_packed_struct_envelopes(types),
        )
        .unwrap();
        assert_eq!(commands[0].request.bounds(), Some((2, 254)));
        let Evidence::Known(layout) = &commands[0].request else {
            panic!("expected resolved request layout");
        };
        assert_eq!(
            layout.segments(),
            Some(
                [
                    WireSegment::fixed(1),
                    WireSegment::variable_with_semantic(
                        7,
                        0,
                        36,
                        VariableSemantic::Counted { prefix_width: 1 },
                    ),
                    WireSegment::fixed(1),
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn resolves_ternary_and_switch_selected_request_widths() {
        let source = r#"
            tBleStatus aci_ternary(uint8_t UUID_Type)
            {
                struct hci_request rq;
                int index_input = 0;
                int uuid_size = (UUID_Type == 2) ? 16 : 2;
                index_input += 1;
                index_input += uuid_size;
                index_input += 1;
                rq.ocf = 0x081;
                rq.clen = index_input;
            }

            tBleStatus aci_switch(uint8_t UUID_Type)
            {
                struct hci_request rq;
                int index_input = 0;
                uint8_t size;
                switch (UUID_Type) {
                    case 1: size = 2; break;
                    case 2: size = 16; break;
                    default: return 1;
                }
                index_input += 2;
                index_input += size;
                rq.ocf = 0x082;
                rq.clen = index_input;
            }
        "#;

        let commands =
            extract_command_metadata(source, "fixture.c", CommandScope::VendorAci).unwrap();
        assert_eq!(commands[0].request, WireLayoutEvidence::known(4, 18));
        assert_eq!(commands[1].request, WireLayoutEvidence::known(4, 18));
    }

    #[test]
    fn caps_multiple_variable_request_fields_at_the_hci_envelope() {
        let source = r#"
            tBleStatus aci_fixture(uint8_t First_Length, uint8_t Second_Length)
            {
                struct hci_request rq;
                int index_input = 0;
                index_input += 13;
                index_input += First_Length;
                index_input += Second_Length;
                rq.ocf = 0x081;
                rq.clen = index_input;
                index_input += 99;
            }
        "#;

        let commands =
            extract_command_metadata(source, "fixture.c", CommandScope::VendorAci).unwrap();
        assert_eq!(commands[0].request, WireLayoutEvidence::known(13, 255));
    }

    #[test]
    fn preserves_an_unsupported_request_term_as_unresolved() {
        let source = r#"
            tBleStatus aci_fixture(const uint8_t *Value)
            {
                struct hci_request rq;
                int index_input = 0;
                index_input += encoded_size(Value);
                rq.ocf = 0x081;
                rq.clen = index_input;
            }
        "#;

        let commands =
            extract_command_metadata(source, "fixture.c", CommandScope::VendorAci).unwrap();
        assert_eq!(
            commands[0].request,
            WireLayoutEvidence::Unresolved("encoded_size(Value)".to_owned())
        );
    }

    #[test]
    fn request_resolution_fails_closed_for_ambiguous_inputs() {
        let source = r#"
            tBleStatus aci_pointer(const uint8_t *Length)
            {
                struct hci_request rq;
                int index_input = 0;
                index_input += Length;
                rq.ocf = 0x081;
                rq.clen = index_input;
            }

            tBleStatus aci_subtract(uint16_t Length)
            {
                struct hci_request rq;
                int index_input = 0;
                index_input += Length - 1;
                rq.ocf = 0x082;
                rq.clen = index_input;
            }

            tBleStatus aci_branched(uint8_t Extended)
            {
                struct hci_request rq;
                int index_input = 0;
                if (Extended) {
                    index_input += 2;
                } else {
                    index_input += 1;
                }
                rq.ocf = 0x083;
                rq.clen = index_input;
            }

            tBleStatus aci_reassigned(uint8_t Extended)
            {
                struct hci_request rq;
                int index_input = 0;
                int size = 2;
                if (Extended) {
                    size = 16;
                }
                index_input += size;
                rq.ocf = 0x084;
                rq.clen = index_input;
            }

            tBleStatus aci_sequential(void)
            {
                struct hci_request rq;
                int index_input = 0;
                int size;
                size = 2;
                size = 16;
                index_input += size;
                rq.ocf = 0x085;
                rq.clen = index_input;
            }

            tBleStatus aci_partial_switch(uint8_t Type)
            {
                struct hci_request rq;
                int index_input = 0;
                int size;
                switch (Type) {
                    case 1: size = 2; break;
                    case 2: size = encoded_size(Type); break;
                    default: return 1;
                }
                index_input += size;
                rq.ocf = 0x086;
                rq.clen = index_input;
            }
        "#;

        let commands =
            extract_command_metadata(source, "fixture.c", CommandScope::VendorAci).unwrap();

        assert_eq!(commands.len(), 6);
        assert!(
            commands
                .iter()
                .all(|command| matches!(command.request, WireLayoutEvidence::Unresolved(_)))
        );
    }

    #[test]
    fn command_extraction_ignores_comment_and_string_decoys() {
        let source = r#"
            /* tBleStatus aci_comment(void) { rq.ocf = 0x999; } */
            const char *example = "tBleStatus aci_string(void) { rq.ocf = 0x998; }";

            tBleStatus aci_real(void)
            {
                struct hci_request rq;
                rq.ocf = 0x081;
            }
        "#;

        let commands =
            extract_command_metadata(source, "fixture.c", CommandScope::VendorAci).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "aci_real");
        assert_eq!(commands[0].ocf(), 0x81);
    }

    #[test]
    fn command_extraction_honors_active_c_preprocessor_branches() {
        let source = r#"
            #define API_LEVEL 2
            #if API_LEVEL >= 2
            tBleStatus aci_current(void) { rq.ocf = 0x081; }
            #else
            tBleStatus aci_old(void) { rq.ocf = 0x080; }
            #endif
            #if 0
            tBleStatus aci_disabled(void) { rq.ocf = 0x082; }
            #endif
        "#;

        let commands = extract_vendor_commands(source, "fixture.c").unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "aci_current");
        assert_eq!(commands[0].code, 0x81);
    }

    #[test]
    fn event_extraction_honors_active_c_preprocessor_branches() {
        let source = r#"
            #if 0
            const hci_event_table_t hci_vs_event_table[] = { { 0x0400U, old } };
            #else
            const hci_event_table_t hci_vs_event_table[] = { { 0x0401U, current } };
            #endif
        "#;

        let events = extract_vendor_events(source).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "current");
        assert_eq!(events[0].code, 0x401);
    }

    #[test]
    fn ignored_event_handlers_need_structural_zero_payload_evidence() {
        let source = r#"
            static void aci_zero_event_process(const uint8_t* in) {
                aci_zero_event();
            }
            static void aci_ambiguous_event_process(const uint8_t* in) {
            }
            static void aci_ignored_event_process(const uint8_t* in) {
                aci_ignored_event();
            }
        "#;
        let types = r#"
            typedef __PACKED_STRUCT {
                uint16_t Value;
            } aci_ignored_event_rp0;
        "#;
        let tree = parse_c_tree(source, EVENT_SOURCE).unwrap();
        let packed_layouts = parse_packed_struct_envelopes(types);
        let layouts = event_process_layouts(tree.root_node(), source, &packed_layouts);

        assert_eq!(
            layouts.get("aci_zero_event_process"),
            Some(&WireLayoutEvidence::fixed(0))
        );
        assert!(matches!(
            layouts.get("aci_ambiguous_event_process"),
            Some(WireLayoutEvidence::Unresolved(reason))
                if reason.contains("aci_ambiguous_event_rp0")
        ));
        assert_eq!(
            layouts
                .get("aci_ignored_event_process")
                .and_then(WireLayoutEvidence::bounds),
            Some((2, 2))
        );
    }

    #[test]
    fn rejects_a_syntax_error_inside_a_selected_command() {
        let source = r#"
            tBleStatus aci_broken(void)
            {
                struct hci_request rq;
                rq.ocf = 0x081;
                @
            }
        "#;

        let error =
            extract_command_metadata(source, "fixture.c", CommandScope::VendorAci).unwrap_err();
        assert!(error.contains("aci_broken"));
        assert!(error.contains("syntax errors"));
    }

    #[test]
    fn extracts_only_vendor_event_table() {
        let source = r#"
            const hci_event_table_t hci_event_table[] = { { 0x0001U, ignored } };
            const hci_event_table_t hci_vs_event_table[] = {
                { 0x0400U, aci_gap_limited_discoverable_event_process },
                { 0x0C01U, aci_gatt_attribute_modified_event_process },
            };
        "#;
        let events = extract_vendor_events(source).unwrap();
        assert_eq!(
            events.iter().map(|entry| entry.code).collect::<Vec<_>>(),
            vec![0x400, 0xc01]
        );
    }

    #[test]
    fn keeps_standard_event_namespaces_separate() {
        let source = r#"
            const hci_event_table_t hci_event_table[] = { { 0x0005U, ordinary } };
            const hci_event_table_t hci_le_event_table[] = { { 0x0001U, le_meta } };
            const hci_event_table_t hci_vs_event_table[] = { { 0x0400U, vendor } };
        "#;
        let ordinary =
            extract_event_table(source, "hci_event_table", EventScope::StandardHci).unwrap();
        let le = extract_event_table(source, "hci_le_event_table", EventScope::LeMeta).unwrap();
        let vendor =
            extract_event_table(source, "hci_vs_event_table", EventScope::VendorAci).unwrap();
        assert_eq!(ordinary[0].code, 0x05);
        assert_eq!(le[0].code, 0x01);
        assert_eq!(vendor[0].code, 0x0400);
        assert_eq!(ordinary[0].kind, CatalogEventKind::StandardHci);
        assert_eq!(le[0].kind, CatalogEventKind::LeMeta);
        assert!(matches!(
            &vendor[0].kind,
            CatalogEventKind::VendorAci {
                payload: WireLayoutEvidence::Unresolved(reason)
            } if reason.contains("vendor_rp0")
        ));
    }

    #[test]
    fn extracts_complete_shci_enum_with_payload_evidence() {
        let source = r#"
            #define SHCI_SUB_EVT_CODE_BASE ( 0x9200 )
            typedef enum {
                SHCI_SUB_EVT_CODE_READY = SHCI_SUB_EVT_CODE_BASE,
                SHCI_SUB_EVT_ERROR_NOTIF,
                SHCI_SUB_EVT_BLE_NVM_RAM_UPDATE,
                SHCI_SUB_EVT_THREAD_NVM_RAM_UPDATE,
                SHCI_SUB_EVT_NVM_START_WRITE,
                SHCI_SUB_EVT_NVM_END_WRITE,
                SHCI_SUB_EVT_NVM_START_ERASE,
                SHCI_SUB_EVT_NVM_END_ERASE,
                SHCI_SUB_EVT_CODE_CONCURRENT_802154_EVT,
            } SHCI_SUB_EVT_CODE_t;

            typedef PACKED_STRUCT {
                uint32_t StartAddress;
                uint32_t Size;
            } SHCI_C2_BleNvmRamUpdate_Evt_t;
            typedef PACKED_STRUCT {
                uint32_t StartAddress;
                uint32_t Size;
            } SHCI_C2_ThreadNvmRamUpdate_Evt_t;
            typedef PACKED_STRUCT {
                uint32_t NumberOfWords;
            } SHCI_C2_NvmStartWrite_Evt_t;
            typedef PACKED_STRUCT {
                uint32_t NumberOfSectors;
            } SHCI_C2_NvmStartErase_Evt_t;
        "#;

        let events = extract_shci_events(source).unwrap();
        assert_eq!(events.len(), 9);
        assert_eq!(
            events.iter().map(|event| event.code).collect::<Vec<_>>(),
            (0x9200..=0x9208).collect::<Vec<_>>()
        );
        assert_eq!(
            events[2].proprietary_payload().and_then(Evidence::bounds),
            Some((8, 8))
        );
        assert_eq!(
            events[4].proprietary_payload().and_then(Evidence::bounds),
            Some((4, 4))
        );
        assert_eq!(
            events[5].proprietary_payload().and_then(Evidence::bounds),
            Some((0, 0))
        );
        assert!(matches!(
            events[8].proprietary_payload(),
            Some(WireLayoutEvidence::Unresolved(reason))
                if reason.contains("does not declare a payload structure")
        ));
    }

    #[test]
    fn resolves_fixed_and_capacity_shaped_packed_responses() {
        let types = r#"
            typedef __PACKED_STRUCT
            {
                uint16_t Value;
            } nested_rp0;

            typedef __PACKED_STRUCT
            {
                uint8_t Status;
                nested_rp0 Nested;
                uint8_t Bytes[4];
            } fixed_rp0;

            typedef __PACKED_STRUCT
            {
                uint8_t Status;
                uint8_t Bytes[BLE_EVT_MAX_PARAM_LEN - 1];
            } capacity_rp0;

            typedef __PACKED_STRUCT
            {
                uint8_t Result;
                uint8_t Status;
            } misplaced_status_rp0;

            typedef __PACKED_STRUCT
            {
                uint8_t Address_Type;
                uint8_t Address[6];
            } Bonded_Device_Entry_t;

            typedef __PACKED_STRUCT
            {
                uint8_t Status;
                uint8_t Data_Length;
                uint8_t Data[(BLE_EVT_MAX_PARAM_LEN - 3) - 2];
            } hal_rp0;

            typedef __PACKED_STRUCT
            {
                uint8_t Status;
                uint8_t Num_of_Addresses;
                Bonded_Device_Entry_t Entries[
                    ((BLE_EVT_MAX_PARAM_LEN - 3) - 2)
                    / sizeof(Bonded_Device_Entry_t)
                ];
            } gap_rp0;

            typedef __PACKED_STRUCT
            {
                uint8_t Status;
                uint16_t Length;
                uint16_t Value_Length;
                uint8_t Value[(BLE_EVT_MAX_PARAM_LEN - 3) - 5];
            } gatt_rp0;

            typedef __PACKED_STRUCT
            {
                uint8_t Status;
                uint8_t Channel_Number;
                uint8_t Channel_Index_List[(BLE_EVT_MAX_PARAM_LEN - 3) - 2];
            } l2cap_rp0;
        "#;
        let layouts = parse_packed_struct_envelopes(types);
        assert_eq!(fixed_packed_size(&layouts, "nested_rp0"), Some(2));
        assert_eq!(fixed_packed_size(&layouts, "fixed_rp0"), Some(7));
        assert_eq!(fixed_packed_size(&layouts, "capacity_rp0"), None);

        let returns = ["fixed_rp0", "hal_rp0", "gap_rp0", "gatt_rp0", "l2cap_rp0"]
            .map(|type_name| return_layout_for_struct(type_name.to_owned(), &layouts));
        assert_eq!(returns[0].bounds(), Some((6, 6)));
        assert_eq!(returns[1].bounds(), Some((1, 251)));
        assert_eq!(returns[2].bounds(), Some((1, 246)));
        assert_eq!(returns[3].bounds(), Some((4, 251)));
        assert_eq!(returns[4].bounds(), Some((1, 251)));
        assert!(matches!(
            return_layout_for_struct("misplaced_status_rp0".to_owned(), &layouts),
            WireLayoutEvidence::Unresolved(reason) if reason.contains("one-byte `Status` field")
        ));
        assert!(matches!(
            return_layout_for_struct("missing_rp0".to_owned(), &layouts),
            WireLayoutEvidence::Unresolved(reason) if reason.contains("missing_rp0")
        ));
    }

    #[test]
    fn packed_layouts_ignore_inactive_c_preprocessor_fields() {
        let types = r#"
            typedef __PACKED_STRUCT
            {
                uint8_t Status;
                #if 0
                uint8_t Excluded[4];
                #else
                uint8_t Included[2];
                #endif
            } active_rp0;
        "#;

        let layouts = parse_packed_struct_envelopes(types);
        assert_eq!(fixed_packed_size(&layouts, "active_rp0"), Some(3));
    }

    #[test]
    fn packed_arrays_preserve_nested_fixed_dimensions_and_explicit_capacities() {
        let types = r#"
            #define FIXED_WIDTH 4

            typedef __PACKED_STRUCT
            {
                uint8_t Matrix[2][3];
                uint8_t Expression_Fixed[2 + 2];
            } fixed_arrays_t;

            typedef __PACKED_STRUCT
            {
                uint8_t Matrix[2][BLE_EVT_MAX_PARAM_LEN / 2];
            } capacity_array_t;

            typedef __PACKED_STRUCT
            {
                uint8_t Unknown[FIXED_WIDTH];
            } preprocessor_fixed_t;
        "#;

        let layouts = parse_packed_struct_envelopes(types);
        assert_eq!(fixed_packed_size(&layouts, "fixed_arrays_t"), Some(10));
        assert_eq!(fixed_packed_size(&layouts, "capacity_array_t"), None);
        assert_eq!(
            layouts.get("capacity_array_t"),
            Some(&Some(PackedEnvelope {
                minimum: 0,
                maximum: 254,
                variable: true,
                segments: vec![WireSegment::variable(2, 0, 127)],
            }))
        );
        assert_eq!(layouts.get("preprocessor_fixed_t"), Some(&None));
    }

    #[test]
    fn resolves_capacity_shaped_event_struct_envelopes() {
        let types = r#"
            typedef __PACKED_STRUCT
            {
                uint16_t Found_Attribute_Handle;
                uint16_t Group_End_Handle;
            } Attribute_Group_Handle_Pair_t;

            typedef __PACKED_STRUCT
            {
                uint16_t Connection_Handle;
                uint8_t Num_of_Handle_Pair;
                Attribute_Group_Handle_Pair_t Pairs[
                    ((BLE_EVT_MAX_PARAM_LEN - 2) - 3)
                    / sizeof(Attribute_Group_Handle_Pair_t)
                ];
            } event_rp0;
        "#;
        let layouts = parse_packed_struct_envelopes(types);
        assert_eq!(
            layouts.get("event_rp0"),
            Some(&Some(PackedEnvelope {
                minimum: 3,
                maximum: 251,
                variable: true,
                segments: vec![
                    WireSegment::fixed(2),
                    WireSegment::fixed(1),
                    WireSegment::variable_with_semantic(
                        4,
                        0,
                        62,
                        VariableSemantic::Counted { prefix_width: 1 },
                    ),
                ],
            }))
        );
    }

    #[test]
    fn discovers_vendor_aci_files_without_standard_hci() {
        let names = [
            "Middlewares/ST/STM32_WPAN/ble/core/auto/ble_gap_aci.c",
            "Middlewares/ST/STM32_WPAN/ble/core/auto/ble_hci_le.c",
            "Middlewares/ST/STM32_WPAN/ble/core/auto/ble_gen_aci.c",
            "Middlewares/ST/STM32_WPAN/ble/core/auto/ble_events.c",
        ];
        let prefix = format!("{AUTO_SOURCE_DIR}/");
        let files = names
            .iter()
            .filter_map(|path| path.strip_prefix(&prefix))
            .filter(|file| file.starts_with("ble_") && file.ends_with("_aci.c"))
            .filter(|file| *file != "ble_hci_le.c")
            .collect::<Vec<_>>();

        assert_eq!(files, vec!["ble_gap_aci.c", "ble_gen_aci.c"]);
    }
}
