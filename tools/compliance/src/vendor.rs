//! Reading the generated STM32CubeWB protocol catalog without modifying its checkout.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use tree_sitter::{Node, Parser, Tree};

use crate::c_preprocessor::preprocess_c_source;
use crate::catalog::{
    CatalogCommand, CatalogEvent, CatalogFamily, CatalogSchema, CommandScope,
    CompletionExpectation, EventPayloadLayout, EventScope, RequestLayout, ResponseLayout,
};
#[cfg(test)]
use crate::model::{CoverageEntry, CoverageOrigin};

pub const AUTO_SOURCE_DIR: &str = "Middlewares/ST/STM32_WPAN/ble/core/auto";
const EVENT_SOURCE: &str = "ble_events.c";
const STANDARD_HCI_SOURCE: &str = "ble_hci_le.c";
const TYPES_SOURCE: &str = "ble_types.h";

/// Load vendor ACI, standard HCI, and transport-envelope metadata from a
/// CubeWB tag without changing the checkout.
pub(crate) fn load_vendor_catalog(cube_dir: &Path, tag: &str) -> Result<CatalogSchema, String> {
    verify_tag(cube_dir, tag)?;

    let types_path = format!("{AUTO_SOURCE_DIR}/{TYPES_SOURCE}");
    let types_source = git_show(cube_dir, tag, &types_path)?;
    let packed_layouts = parse_packed_struct_envelopes(&types_source);

    let mut catalog = CatalogSchema::new(CatalogFamily::Stm32Wb, tag);
    for file in command_source_files(cube_dir, tag)? {
        let path = format!("{AUTO_SOURCE_DIR}/{file}");
        let source = git_show(cube_dir, tag, &path)?;
        let commands = extract_command_metadata_with_evidence(
            &source,
            &file,
            CommandScope::VendorAci,
            &packed_layouts,
        )?;
        catalog.commands.extend(commands);
    }

    let path = format!("{AUTO_SOURCE_DIR}/{EVENT_SOURCE}");
    let source = git_show(cube_dir, tag, &path)?;
    let vendor_events = extract_event_table_with_evidence(
        &source,
        "hci_vs_event_table",
        EventScope::VendorAci,
        &packed_layouts,
    )?;
    catalog.events.extend(vendor_events);

    let standard_path = format!("{AUTO_SOURCE_DIR}/{STANDARD_HCI_SOURCE}");
    let standard_source = git_show(cube_dir, tag, &standard_path)?;
    let standard_commands = extract_command_metadata_with_evidence(
        &standard_source,
        STANDARD_HCI_SOURCE,
        CommandScope::StandardHci,
        &packed_layouts,
    )?;
    catalog.commands.extend(standard_commands);

    let standard_events = extract_event_table_with_evidence(
        &source,
        "hci_event_table",
        EventScope::StandardHci,
        &packed_layouts,
    )?;
    catalog.events.extend(standard_events);

    let le_events = extract_event_table_with_evidence(
        &source,
        "hci_le_event_table",
        EventScope::LeMeta,
        &packed_layouts,
    )?;
    catalog.events.extend(le_events);

    catalog.normalize();
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
    String::from_utf8(output.stdout)
        .map_err(|error| format!("git show {spec} did not return UTF-8 source: {error}"))
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
                CoverageEntry::new(command.ocf, command.name, CoverageOrigin::VendorAutoSource)
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
fn parse_c_tree(source: &str, source_name: &str) -> Result<Tree, String> {
    let preprocessed = preprocess_c_source(source, source_name)?;
    let mut parser = Parser::new();
    let language = tree_sitter_c::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|error| format!("{source_name}: could not load C grammar: {error}"))?;
    parser
        .parse(&preprocessed, None)
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

/// Find the first simple `receiver.field = value` assignment in source order.
/// The AST avoids treating comments, strings, comparisons, or a similarly
/// named local variable as transport metadata.
fn assignment_value<'tree>(
    body: Node<'tree>,
    source: &str,
    receiver: &str,
    field: &str,
) -> Option<Node<'tree>> {
    let mut assignments = Vec::new();
    collect_nodes(body, "assignment_expression", &mut assignments);
    assignments.into_iter().find_map(|assignment| {
        let operator = assignment.child_by_field_name("operator")?;
        (node_text(operator, source) == "=")
            .then_some(())
            .filter(|_| {
                assignment
                    .child_by_field_name("left")
                    .is_some_and(|left| field_expression_is(left, source, receiver, field))
            })
            .and_then(|_| assignment.child_by_field_name("right"))
    })
}

fn assignment_integer(body: Node<'_>, source: &str, member: &str) -> Option<u16> {
    let value = assignment_value(body, source, "rq", member)?;
    parse_c_integer(node_text(value, source), 0).map(|(value, _)| value)
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

fn extract_command_metadata_with_evidence(
    source: &str,
    source_name: &str,
    scope: CommandScope,
    packed_layouts: &PackedLayouts,
) -> Result<Vec<CatalogCommand>, String> {
    let tree = parse_c_tree(source, source_name)?;
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
        let Some(body) = function.child_by_field_name("body") else {
            continue;
        };
        let Some(ocf) = assignment_integer(body, source, "ocf") else {
            continue;
        };
        if function.has_error() {
            return Err(format!(
                "{source_name}: generated command `{name}` contains C syntax errors"
            ));
        }
        let ogf = assignment_integer(body, source, "ogf").map(|value| value as u8);
        let opcode = match scope {
            CommandScope::VendorAci => None,
            CommandScope::StandardHci => ogf.map(|ogf| (u16::from(ogf) << 10) | ocf),
        };
        if matches!(scope, CommandScope::StandardHci) && ogf.is_none() {
            return Err(format!(
                "{source_name}: standard command `{name}` has an OCF but no literal rq.ogf"
            ));
        }
        let source_offset = u32::try_from(function.start_byte()).map_err(|_| {
            format!("{source_name}: command `{name}` source offset exceeds schema range")
        })?;
        commands.push(CatalogCommand {
            scope,
            name,
            source_name: source_name.to_owned(),
            source_offset,
            ogf,
            ocf,
            opcode,
            completion: completion_expectation(body, source),
            request: request_layout(declarator, body, source, packed_layouts),
            response: response_layout(body, source, packed_layouts),
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

fn extract_event_table_with_evidence(
    source: &str,
    table_name: &str,
    scope: EventScope,
    packed_layouts: &PackedLayouts,
) -> Result<Vec<CatalogEvent>, String> {
    let tree = parse_c_tree(source, EVENT_SOURCE)?;
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
        let Some((code, _)) = parse_c_integer(node_text(code_node, source), 0) else {
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
        let payload = (scope == EventScope::VendorAci).then(|| {
            process_layouts
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
                })
        });
        entries.push(CatalogEvent {
            scope,
            code,
            name: handler_name.to_owned(),
            source_name: EVENT_SOURCE.to_owned(),
            source_offset: u32::try_from(handler.start_byte()).map_err(|_| {
                format!("ble_events.c: {table_name} entry source offset exceeds schema range")
            })?,
            payload,
        });
    }

    if entries.is_empty() {
        return Err(format!("ble_events.c: {table_name} contains no entries"));
    }
    Ok(entries)
}

fn event_process_layouts(
    root: Node<'_>,
    source: &str,
    packed_layouts: &PackedLayouts,
) -> BTreeMap<String, EventPayloadLayout> {
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
            let layout = type_name.map_or_else(
                || {
                    if body_uses_identifier(body, source, "in") {
                        event_payload_layout(
                            format!("{}_rp0", name.strip_suffix("_process").unwrap_or(&name)),
                            packed_layouts,
                        )
                    } else {
                        EventPayloadLayout::Fixed(0)
                    }
                },
                |type_name| event_payload_layout(type_name, packed_layouts),
            );
            Some((name, layout))
        })
        .collect()
}

fn body_uses_identifier(body: Node<'_>, source: &str, identifier: &str) -> bool {
    let mut identifiers = Vec::new();
    collect_nodes(body, "identifier", &mut identifiers);
    identifiers
        .into_iter()
        .any(|node| node_text(node, source) == identifier)
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

fn completion_expectation(body: Node<'_>, source: &str) -> CompletionExpectation {
    match assignment_value(body, source, "rq", "event") {
        None => CompletionExpectation::CommandComplete,
        Some(value) => match parse_c_integer(node_text(value, source), 0).map(|(value, _)| value) {
            Some(0x0e) => CompletionExpectation::CommandComplete,
            Some(0x0f) => CompletionExpectation::CommandStatus,
            Some(value) if value <= u16::from(u8::MAX) => CompletionExpectation::Event(value as u8),
            Some(value) => CompletionExpectation::Unresolved(format!("0x{value:04X}")),
            None => CompletionExpectation::Unresolved(node_text(value, source).trim().to_owned()),
        },
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
) -> RequestLayout {
    let Some(value) = assignment_value(body, source, "rq", "clen") else {
        return RequestLayout::Empty;
    };
    let value_text = node_text(value, source).trim();
    if let Some((size, end)) = parse_c_integer(value_text, 0)
        && value_text[end..].trim().is_empty()
    {
        return RequestLayout::Fixed(u32::from(size));
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
            return RequestLayout::Unresolved(formula);
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
            return RequestLayout::Unresolved(formula);
        };
        let Some((minimum, maximum)) = total.envelope() else {
            return RequestLayout::Unresolved(formula);
        };
        let (Ok(minimum), Ok(maximum)) = (u32::try_from(minimum), u32::try_from(maximum)) else {
            return RequestLayout::Unresolved(formula);
        };
        return if minimum == maximum {
            RequestLayout::Fixed(maximum)
        } else {
            RequestLayout::Variable { minimum, maximum }
        };
    }
    RequestLayout::Unresolved(value_text.to_owned())
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

fn response_layout(body: Node<'_>, source: &str, packed_layouts: &PackedLayouts) -> ResponseLayout {
    let Some(value) = assignment_value(body, source, "rq", "rlen") else {
        return ResponseLayout::None;
    };
    let value_text = node_text(value, source).trim();
    if let Some((size, end)) = parse_c_integer(value_text, 0)
        && value_text[end..].trim().is_empty()
    {
        return if size == 1 {
            ResponseLayout::Status
        } else {
            ResponseLayout::Fixed(u32::from(size))
        };
    }
    if let Some(variable) = sizeof_variable(value)
        && let Some(type_name) = c_variable_type(body, source, node_text(variable, source))
    {
        return response_layout_for_struct(type_name, packed_layouts);
    }
    ResponseLayout::Unresolved(value_text.to_owned())
}

fn response_layout_for_struct(type_name: String, layouts: &PackedLayouts) -> ResponseLayout {
    match normalized_packed_layout(&type_name, layouts) {
        Ok((minimum, maximum, true)) => ResponseLayout::Variable { minimum, maximum },
        Ok((_, maximum, false)) => ResponseLayout::Fixed(maximum),
        Err(reason) => ResponseLayout::Unresolved(reason),
    }
}

fn event_payload_layout(type_name: String, layouts: &PackedLayouts) -> EventPayloadLayout {
    match normalized_packed_layout(&type_name, layouts) {
        Ok((minimum, maximum, true)) => EventPayloadLayout::Variable { minimum, maximum },
        Ok((_, maximum, false)) => EventPayloadLayout::Fixed(maximum),
        Err(reason) => EventPayloadLayout::Unresolved(reason),
    }
}

fn normalized_packed_layout(
    type_name: &str,
    layouts: &PackedLayouts,
) -> Result<(u32, u32, bool), String> {
    let layout = layouts
        .get(type_name)
        .copied()
        .flatten()
        .ok_or_else(|| format!("packed C structure `{type_name}` could not be resolved"))?;
    let minimum = u32::try_from(layout.minimum)
        .map_err(|_| format!("packed C structure `{type_name}` minimum exceeds schema range"))?;
    let maximum = u32::try_from(layout.maximum)
        .map_err(|_| format!("packed C structure `{type_name}` maximum exceeds schema range"))?;
    Ok((minimum, maximum, layout.variable))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PackedEnvelope {
    minimum: usize,
    maximum: usize,
    variable: bool,
}

type PackedLayouts = BTreeMap<String, Option<PackedEnvelope>>;

fn fixed_packed_size(layouts: &PackedLayouts, type_name: &str) -> Option<usize> {
    let layout = layouts.get(type_name).copied().flatten()?;
    (!layout.variable && layout.minimum == layout.maximum).then_some(layout.maximum)
}

#[derive(Clone, Debug)]
enum PackedMultiplicity {
    Fixed(usize),
    Capacity(String),
}

#[derive(Clone, Debug)]
struct PackedEnvelopeField {
    type_name: String,
    multiplicity: PackedMultiplicity,
}

#[derive(Clone, Debug)]
struct PackedEnvelopeDefinition {
    name: String,
    fields: Option<Vec<PackedEnvelopeField>>,
}

fn parse_packed_struct_envelopes(source: &str) -> PackedLayouts {
    const PACKED_MARKER: &str = "__PACKED_STRUCT";
    const PARSABLE_MARKER: &str = "struct         ";
    debug_assert_eq!(PACKED_MARKER.len(), PARSABLE_MARKER.len());

    let normalized = source.replace(PACKED_MARKER, PARSABLE_MARKER);
    let Ok(tree) = parse_c_tree(&normalized, TYPES_SOURCE) else {
        return BTreeMap::new();
    };
    let mut type_definitions = Vec::new();
    collect_nodes(tree.root_node(), "type_definition", &mut type_definitions);
    let definitions = type_definitions
        .into_iter()
        .filter_map(|definition| {
            node_text(definition, source)
                .contains(PACKED_MARKER)
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
            if layouts.get(&definition.name) != Some(&Some(layout)) {
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
            fields.push(PackedEnvelopeField {
                type_name: type_name.clone(),
                multiplicity: packed_field_multiplicity(declarator, source)?,
            });
        }
    }
    Some(fields)
}

fn packed_field_multiplicity(declarator: Node<'_>, source: &str) -> Option<PackedMultiplicity> {
    match declarator.kind() {
        "identifier" | "field_identifier" => Some(PackedMultiplicity::Fixed(1)),
        "pointer_declarator" | "abstract_pointer_declarator" => None,
        "array_declarator" => {
            let size = declarator.child_by_field_name("size")?;
            let expression = node_text(size, source).trim();
            if let Some((count, end)) = parse_c_integer(expression, 0)
                && expression[end..].trim().is_empty()
            {
                Some(PackedMultiplicity::Fixed(usize::from(count)))
            } else {
                Some(PackedMultiplicity::Capacity(expression.to_owned()))
            }
        }
        _ => declarator
            .child_by_field_name("declarator")
            .and_then(|inner| packed_field_multiplicity(inner, source)),
    }
}

fn packed_struct_envelope(
    fields: &[PackedEnvelopeField],
    known: &BTreeMap<String, Option<PackedEnvelope>>,
) -> Option<PackedEnvelope> {
    let mut layout = PackedEnvelope {
        minimum: 0,
        maximum: 0,
        variable: false,
    };
    for field in fields {
        let element = primitive_c_size(&field.type_name).map_or_else(
            || known.get(&field.type_name).copied().flatten(),
            |size| {
                Some(PackedEnvelope {
                    minimum: size,
                    maximum: size,
                    variable: false,
                })
            },
        )?;
        match &field.multiplicity {
            PackedMultiplicity::Fixed(count) => {
                layout.minimum = layout
                    .minimum
                    .checked_add(element.minimum.checked_mul(*count)?)?;
                layout.maximum = layout
                    .maximum
                    .checked_add(element.maximum.checked_mul(*count)?)?;
                layout.variable |= element.variable;
            }
            PackedMultiplicity::Capacity(expression) => {
                if element.variable || element.minimum != element.maximum {
                    return None;
                }
                let count = evaluate_capacity_expression(expression, known)?;
                layout.maximum = layout
                    .maximum
                    .checked_add(element.maximum.checked_mul(count)?)?;
                layout.variable = true;
            }
        }
    }
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
            let layout = self.known.get(&type_name).copied().flatten()?;
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
        assert_eq!(commands[0].ocf, 0x81);
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
        assert_eq!(
            commands[0].request,
            RequestLayout::Variable {
                minimum: 2,
                maximum: 255,
            }
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
        assert_eq!(
            commands[0].request,
            RequestLayout::Variable {
                minimum: 2,
                maximum: 254,
            }
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
        assert_eq!(
            commands[0].request,
            RequestLayout::Variable {
                minimum: 4,
                maximum: 18,
            }
        );
        assert_eq!(
            commands[1].request,
            RequestLayout::Variable {
                minimum: 4,
                maximum: 18,
            }
        );
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
        assert_eq!(
            commands[0].request,
            RequestLayout::Variable {
                minimum: 13,
                maximum: 255,
            }
        );
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
            RequestLayout::Unresolved("encoded_size(Value)".to_owned())
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
                .all(|command| matches!(command.request, RequestLayout::Unresolved(_)))
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
        assert_eq!(commands[0].ocf, 0x81);
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
        assert!(ordinary[0].payload.is_none());
        assert!(le[0].payload.is_none());
        assert!(matches!(
            &vendor[0].payload,
            Some(EventPayloadLayout::Unresolved(reason)) if reason.contains("vendor_rp0")
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

        let responses = ["fixed_rp0", "hal_rp0", "gap_rp0", "gatt_rp0", "l2cap_rp0"]
            .map(|type_name| response_layout_for_struct(type_name.to_owned(), &layouts));
        assert_eq!(responses[0], ResponseLayout::Fixed(7));
        assert_eq!(
            responses[1],
            ResponseLayout::Variable {
                minimum: 2,
                maximum: 252,
            }
        );
        assert_eq!(
            responses[2],
            ResponseLayout::Variable {
                minimum: 2,
                maximum: 247,
            }
        );
        assert_eq!(
            responses[3],
            ResponseLayout::Variable {
                minimum: 5,
                maximum: 252,
            }
        );
        assert_eq!(
            responses[4],
            ResponseLayout::Variable {
                minimum: 2,
                maximum: 252,
            }
        );
        assert!(matches!(
            response_layout_for_struct("missing_rp0".to_owned(), &layouts),
            ResponseLayout::Unresolved(reason) if reason.contains("missing_rp0")
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
