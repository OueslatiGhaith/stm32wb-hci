//! Feature-aware extraction of the crate's vendor command and event surface.
//!
//! The checker deliberately works from the Rust syntax tree rather than source
//! text. Command, event, and module cfgs are evaluated structurally for the
//! selected Cube release and stack profile.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use stm32wb_hci_schema::{
    Completion as SchemaCompletion, FieldEncoding, Fields, SemanticWireType, VariableEncodingShape,
    VendorCommand, VendorEvents as SchemaVendorEvents, WireSize, WireTypeDeclaration,
};
use syn::{Expr, File, Item, ItemMacro, ItemMod, Lit, Meta, Path as SynPath, Type};

use crate::ComplianceTarget;
use crate::catalog::{
    Envelope, TaggedItemsVariantLayout, TaggedVariantLayout, VariableSemantic, WireLayout,
    WireSegment,
};
use crate::model::{CoverageEntry, CoverageOrigin, ProtocolCoverage};
use crate::rust_cfg::attrs_active;

pub(crate) struct RustCatalog {
    /// Active commands declared by the `vendor_cmd!` catalog, keyed by name.
    pub(crate) commands: BTreeMap<String, CommandDeclaration>,
    /// Active events declared by the `vendor_event!` catalog, keyed by code.
    pub(crate) events: BTreeMap<u16, EventDeclaration>,
}

impl RustCatalog {
    /// Derive coverage from the same command and event values used for wire
    /// validation. No second inventory can drift from the declarative catalog.
    pub(crate) fn coverage(&self) -> ProtocolCoverage {
        let mut coverage = ProtocolCoverage {
            commands: self
                .commands
                .values()
                .map(CommandDeclaration::coverage_entry)
                .collect(),
            events: self
                .events
                .values()
                .map(EventDeclaration::coverage_entry)
                .collect(),
        };
        coverage
            .commands
            .sort_by_key(|entry| (entry.code, entry.name.clone()));
        coverage
            .events
            .sort_by_key(|entry| (entry.code, entry.name.clone()));
        coverage
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandDeclaration {
    pub(crate) name: String,
    pub(crate) code: u16,
    pub(crate) completion: CommandCompletion,
    /// Command parameter bytes, excluding the HCI command header.
    pub(crate) request: WireLayout,
    pub(crate) location: PathBuf,
}

impl CommandDeclaration {
    fn coverage_entry(&self) -> CoverageEntry {
        CoverageEntry::new(self.code, &self.name, CoverageOrigin::VendorCommandCatalog)
            .at(self.location.clone())
    }
}

/// Completion shape declared by one Rust `vendor_cmd!` invocation.
///
/// Command Complete owns its command return envelope; Command Status cannot
/// carry one, so invalid completion/return combinations are unrepresentable
/// after parsing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommandCompletion {
    CommandComplete { returns: WireLayout },
    CommandStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EventDeclaration {
    pub(crate) name: String,
    pub(crate) code: u16,
    /// Vendor event payload bytes, excluding the two-byte vendor event code.
    pub(crate) payload: WireLayout,
    pub(crate) location: PathBuf,
}

impl EventDeclaration {
    fn coverage_entry(&self) -> CoverageEntry {
        CoverageEntry::new(self.code, &self.name, CoverageOrigin::VendorEventCatalog)
            .at(self.location.clone())
    }
}

#[derive(Clone)]
struct SourceUnit {
    path: PathBuf,
    file: File,
}

#[derive(Clone)]
struct WireTypeComponent {
    type_name: Option<String>,
    ty: Type,
}

#[derive(Clone)]
struct WireTypeShape {
    width: u32,
    components: Vec<WireTypeComponent>,
}

type WireTypeShapes = BTreeMap<String, WireTypeShape>;

/// Load the declarative vendor command and event catalogs for one selected
/// release/profile target.
pub(crate) fn load_rust_catalog(
    crate_dir: &Path,
    target: ComplianceTarget,
) -> Result<RustCatalog, String> {
    let wire_type_shapes = load_wire_type_shapes(&crate_dir.join("src"), target)?;
    let command_root = crate_dir.join("src/vendor/command/mod.rs");
    let command_root_file = read_rust_file(&command_root)?;
    let mut command_sources = Vec::new();
    let mut visited = BTreeSet::new();
    collect_command_sources(
        command_root,
        command_root_file,
        target,
        &mut visited,
        &mut command_sources,
    )?;

    let commands = collect_commands(&command_sources, target, &wire_type_shapes)?;

    let event_path = crate_dir.join("src/vendor/event/mod.rs");
    let event_file = read_rust_file(&event_path)?;
    let events =
        parse_vendor_event_declarations(&event_file, target, &event_path, &wire_type_shapes)?;

    Ok(RustCatalog { commands, events })
}

fn load_wire_type_shapes(
    source_dir: &Path,
    target: ComplianceTarget,
) -> Result<WireTypeShapes, String> {
    let mut paths = Vec::new();
    collect_rust_paths(source_dir, &mut paths)?;
    paths.sort();

    let mut shapes = WireTypeShapes::new();
    for path in paths {
        let file = read_rust_file(&path)?;
        collect_wire_type_shapes_from_items(&file.items, target, &path, &mut shapes)?;
    }
    for (name, shape) in &shapes {
        if shape.width == 0 {
            return Err(format!(
                "wire type `{name}` declares a zero canonical width"
            ));
        }
        if !shape.components.is_empty() {
            let component_width = shape.components.iter().try_fold(0u32, |total, component| {
                total
                    .checked_add(resolve_type_width(&component.ty, &shapes)?)
                    .ok_or_else(|| format!("wire type `{name}` component widths overflow u32"))
            })?;
            if component_width != shape.width {
                return Err(format!(
                    "wire type `{name}` declares canonical width {}, but its semantic components require {component_width}",
                    shape.width,
                ));
            }
        }
    }
    Ok(shapes)
}

fn collect_rust_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "could not read an entry in {}: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_paths(&path, paths)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
    Ok(())
}

fn collect_wire_type_shapes_from_items(
    items: &[Item],
    target: ComplianceTarget,
    path: &Path,
    shapes: &mut WireTypeShapes,
) -> Result<(), String> {
    for item in items {
        if !item_is_active(item, target, path)? {
            continue;
        }
        match item {
            Item::Macro(item) if is_macro_named(&item.mac.path, "wire_type") => {
                let declaration = syn::parse2::<SemanticWireType>(item.mac.tokens.clone())
                    .map_err(|error| {
                        format!(
                            "{}: unsupported wire_type! declaration: {error}",
                            path.display()
                        )
                    })?;
                let (name, width, components) = match declaration.declaration {
                    WireTypeDeclaration::ClosedEnum(value) => {
                        let Some(width) = value.width else { continue };
                        let repr = value.repr;
                        (
                            value.name.to_string(),
                            width,
                            vec![WireTypeComponent {
                                type_name: simple_type_name(&repr),
                                ty: repr,
                            }],
                        )
                    }
                    WireTypeDeclaration::OpenEnum(value) => {
                        let repr = value.repr;
                        (
                            value.name.to_string(),
                            value.width,
                            vec![WireTypeComponent {
                                type_name: simple_type_name(&repr),
                                ty: repr,
                            }],
                        )
                    }
                    WireTypeDeclaration::OpenScalar(value) => {
                        let repr = value.repr;
                        (
                            value.name.to_string(),
                            value.width,
                            vec![WireTypeComponent {
                                type_name: simple_type_name(&repr),
                                ty: repr,
                            }],
                        )
                    }
                    WireTypeDeclaration::Ranged(value) => {
                        let repr = value.repr;
                        (
                            value.name.to_string(),
                            value.width,
                            vec![WireTypeComponent {
                                type_name: simple_type_name(&repr),
                                ty: repr,
                            }],
                        )
                    }
                    WireTypeDeclaration::Bitflags(value) => {
                        let repr = value.repr;
                        (
                            value.name.to_string(),
                            value.width,
                            vec![WireTypeComponent {
                                type_name: simple_type_name(&repr),
                                ty: repr,
                            }],
                        )
                    }
                    WireTypeDeclaration::Composite(value) => {
                        let name = simple_type_name(&value.ty).ok_or_else(|| {
                            format!(
                                "{}: composite wire type must use a path type",
                                path.display()
                            )
                        })?;
                        let components = value
                            .fields
                            .into_iter()
                            .map(|field| WireTypeComponent {
                                type_name: simple_type_name(&field.ty),
                                ty: field.ty,
                            })
                            .collect();
                        (name, value.width, components)
                    }
                    WireTypeDeclaration::Primitive(value) => {
                        let name = simple_type_name(&value.ty).ok_or_else(|| {
                            format!(
                                "{}: primitive wire type must use a path type",
                                path.display()
                            )
                        })?;
                        (name, value.width, Vec::new())
                    }
                    WireTypeDeclaration::Transparent(value) => {
                        let name = simple_type_name(&value.ty).ok_or_else(|| {
                            format!(
                                "{}: transparent wire type must use a path type",
                                path.display()
                            )
                        })?;
                        let inner = value.inner;
                        (
                            name,
                            value.width,
                            vec![WireTypeComponent {
                                type_name: simple_type_name(&inner),
                                ty: inner,
                            }],
                        )
                    }
                };
                let width = width.base10_parse::<u32>().map_err(|error| {
                    format!(
                        "{}: wire type `{name}` has an invalid width: {error}",
                        path.display()
                    )
                })?;
                let builtin_width = match name.as_str() {
                    "u8" | "i8" => Some(1),
                    "u16" | "i16" => Some(2),
                    "u32" | "i32" => Some(4),
                    "u64" | "i64" => Some(8),
                    _ => None,
                };
                if builtin_width.is_some_and(|builtin| builtin != width) {
                    return Err(format!(
                        "{}: primitive wire type `{name}` declares width {width}, but its canonical Rust width is {}",
                        path.display(),
                        builtin_width.unwrap(),
                    ));
                }
                let shape = WireTypeShape { width, components };
                if let Some(previous) = shapes.get(&name) {
                    if previous.width != shape.width
                        || previous.components.len() != shape.components.len()
                        || previous
                            .components
                            .iter()
                            .zip(&shape.components)
                            .any(|(left, right)| left.type_name != right.type_name)
                    {
                        return Err(format!(
                            "{}: wire type `{name}` has conflicting active declarations",
                            path.display()
                        ));
                    }
                } else {
                    shapes.insert(name, shape);
                }
            }
            Item::Mod(module) if module.content.is_some() => {
                let (_, nested) = module.content.as_ref().expect("checked above");
                collect_wire_type_shapes_from_items(nested, target, path, shapes)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn simple_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .qself
            .is_none()
            .then(|| {
                path.path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
            })
            .flatten(),
        Type::Reference(reference) => simple_type_name(&reference.elem),
        _ => None,
    }
}

fn expanded_wire_type_shape(
    name: &str,
    width: u32,
    shapes: &WireTypeShapes,
    resolving: &mut BTreeSet<String>,
) -> Option<Vec<u32>> {
    let shape = shapes.get(name)?;
    if shape.width != width || shape.components.is_empty() || !resolving.insert(name.to_owned()) {
        return None;
    }
    let mut widths = Vec::new();
    for component in &shape.components {
        let component_width = resolve_type_width(&component.ty, shapes).ok()?;
        let nested = component.type_name.as_deref().and_then(|type_name| {
            expanded_wire_type_shape(type_name, component_width, shapes, resolving)
        });
        if let Some(nested) = nested {
            widths.extend(nested);
        } else {
            widths.push(component_width);
        }
    }
    resolving.remove(name);
    (widths.iter().sum::<u32>() == width).then_some(widths)
}

fn resolve_type_width(ty: &Type, shapes: &WireTypeShapes) -> Result<u32, String> {
    match ty {
        Type::Reference(reference) => resolve_type_width(&reference.elem, shapes),
        Type::Array(array) => {
            let element = resolve_type_width(&array.elem, shapes)?;
            let Expr::Lit(length) = &array.len else {
                return Err("wire array length must be an integer literal".to_owned());
            };
            let Lit::Int(length) = &length.lit else {
                return Err("wire array length must be an integer literal".to_owned());
            };
            let length = length
                .base10_parse::<u32>()
                .map_err(|error| format!("invalid wire array length: {error}"))?;
            element
                .checked_mul(length)
                .ok_or_else(|| "wire array width overflows u32".to_owned())
        }
        Type::Path(_) => {
            let name =
                simple_type_name(ty).ok_or_else(|| "wire type must be a simple path".to_owned())?;
            if let Some(shape) = shapes.get(&name) {
                return Ok(shape.width);
            }
            match name.as_str() {
                "u8" | "i8" | "bool" => Ok(1),
                "u16" | "i16" => Ok(2),
                "u32" | "i32" => Ok(4),
                "u64" | "i64" => Ok(8),
                _ => Err(format!(
                    "semantic wire type `{name}` has no canonical wire_type! declaration"
                )),
            }
        }
        _ => Err("canonical wire widths require a path, reference, or array type".to_owned()),
    }
}

fn resolve_wire_size(size: &WireSize, shapes: &WireTypeShapes) -> Result<usize, String> {
    size.terms()
        .iter()
        .try_fold(size.constant_part(), |total, term| {
            let width = usize::try_from(resolve_type_width(term.ty(), shapes)?)
                .expect("u32 wire widths fit usize");
            width
                .checked_mul(term.multiplier())
                .and_then(|value| total.checked_add(value))
                .ok_or_else(|| "declarative wire size overflows usize".to_owned())
        })
}

fn read_rust_file(path: &Path) -> Result<File, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    syn::parse_file(&source)
        .map_err(|error| format!("could not parse Rust source {}: {error}", path.display()))
}

/// Discover command modules from `src/vendor/command/mod.rs` rather than
/// maintaining a hand-written group list. This keeps a future, cfg-gated module
/// out of old firmware inventories and automatically brings it in when its cfg
/// becomes active.
fn collect_command_sources(
    path: PathBuf,
    file: File,
    target: ComplianceTarget,
    visited: &mut BTreeSet<PathBuf>,
    sources: &mut Vec<SourceUnit>,
) -> Result<(), String> {
    if !attrs_active(&file.attrs, target, &path)? {
        return Ok(());
    }

    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    if !visited.insert(canonical) {
        return Ok(());
    }

    // Descend before moving `file` into the source collection. Inline modules
    // are represented as their own source unit so direct-item scans never need
    // to recursively re-scan a parent module.
    for item in &file.items {
        let Item::Mod(module) = item else {
            continue;
        };
        if !attrs_active(&module.attrs, target, &path)? {
            continue;
        }

        if let Some((_, items)) = &module.content {
            let inline_path = path.join(format!("<{}>", module.ident));
            collect_command_sources(
                inline_path,
                File {
                    shebang: None,
                    attrs: Vec::new(),
                    items: items.clone(),
                },
                target,
                visited,
                sources,
            )?;
        } else {
            let module_path = external_module_path(&path, module)?;
            let module_file = read_rust_file(&module_path)?;
            collect_command_sources(module_path, module_file, target, visited, sources)?;
        }
    }

    sources.push(SourceUnit { path, file });
    Ok(())
}

fn external_module_path(parent_path: &Path, module: &ItemMod) -> Result<PathBuf, String> {
    if let Some(path) = module_path_override(module)? {
        return Ok(parent_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path));
    }

    let parent_dir = parent_path.parent().unwrap_or_else(|| Path::new("."));
    let child_dir = if parent_path.file_name().is_some_and(|name| name == "mod.rs") {
        parent_dir.to_path_buf()
    } else {
        parent_dir.join(
            parent_path
                .file_stem()
                .ok_or_else(|| format!("{} has no file stem", parent_path.display()))?,
        )
    };
    let flat = child_dir.join(format!("{}.rs", module.ident));
    let nested = child_dir.join(module.ident.to_string()).join("mod.rs");

    match (flat.is_file(), nested.is_file()) {
        (true, false) => Ok(flat),
        (false, true) => Ok(nested),
        (false, false) => Err(format!(
            "{}: active module `{}` has no source file (looked for {} or {})",
            parent_path.display(),
            module.ident,
            flat.display(),
            nested.display(),
        )),
        (true, true) => Err(format!(
            "{}: active module `{}` is ambiguous: both {} and {} exist",
            parent_path.display(),
            module.ident,
            flat.display(),
            nested.display(),
        )),
    }
}

fn module_path_override(module: &ItemMod) -> Result<Option<String>, String> {
    let Some(attribute) = module
        .attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("path"))
    else {
        return Ok(None);
    };
    let Meta::NameValue(value) = &attribute.meta else {
        return Err(format!(
            "module `{}`: unsupported #[path] attribute shape",
            module.ident
        ));
    };
    let Expr::Lit(literal) = &value.value else {
        return Err(format!(
            "module `{}`: #[path] must contain a string literal",
            module.ident
        ));
    };
    let Lit::Str(path) = &literal.lit else {
        return Err(format!(
            "module `{}`: #[path] must contain a string literal",
            module.ident
        ));
    };
    Ok(Some(path.value()))
}

fn collect_commands(
    sources: &[SourceUnit],
    target: ComplianceTarget,
    wire_type_shapes: &WireTypeShapes,
) -> Result<BTreeMap<String, CommandDeclaration>, String> {
    let mut commands = BTreeMap::<String, CommandDeclaration>::new();
    let mut codes = BTreeMap::<u16, CommandDeclaration>::new();

    for source in sources {
        for item in &source.file.items {
            let Item::Macro(item) = item else {
                continue;
            };
            if !is_macro_named(&item.mac.path, "vendor_cmd")
                || !attrs_active(&item.attrs, target, &source.path)?
            {
                continue;
            }

            let command = parse_vendor_command(item, &source.path, wire_type_shapes)?;

            if let Some(previous) = commands.get(&command.name) {
                return Err(format!(
                    "{}: declaration `{}` is active more than once (also declared in {})",
                    source.path.display(),
                    command.name,
                    previous.location.display()
                ));
            }
            if let Some(previous) = codes.insert(command.code, command.clone()) {
                return Err(format!(
                    "{}: declarations `{}` and `{}` both declare active vendor OCF 0x{:03X}",
                    source.path.display(),
                    previous.name,
                    command.name,
                    command.code,
                ));
            }
            commands.insert(command.name.clone(), command);
        }
    }

    if commands.is_empty() {
        return Err("no active vendor_cmd! command declarations were found".to_owned());
    }
    Ok(commands)
}

fn parse_vendor_command(
    item: &ItemMacro,
    path: &Path,
    wire_type_shapes: &WireTypeShapes,
) -> Result<CommandDeclaration, String> {
    let command = syn::parse2::<VendorCommand>(item.mac.tokens.clone()).map_err(|error| {
        format!(
            "{}: unsupported vendor_cmd! declaration: {error}",
            path.display()
        )
    })?;

    let request = wire_layout(
        command.params.fields(),
        &command.params.min_size(),
        &command.params.max_size(),
        Some(usize::from(u8::MAX)),
        wire_type_shapes,
    )?;
    let completion = match command.completion {
        SchemaCompletion::CommandComplete => {
            let returns = command
                .returns
                .as_ref()
                .expect("the shared parser requires Return for CommandComplete");
            let return_maximum = resolve_wire_size(&returns.max_size(), wire_type_shapes)?;
            if return_maximum > usize::from(u8::MAX) {
                return Err(format!(
                    "command return envelope is {return_maximum} bytes, exceeding the HCI 255-byte limit"
                ));
            }
            CommandCompletion::CommandComplete {
                returns: wire_layout(
                    returns.fields(),
                    &returns.min_size(),
                    &returns.max_size(),
                    None,
                    wire_type_shapes,
                )?,
            }
        }
        SchemaCompletion::CommandStatus => CommandCompletion::CommandStatus,
    };

    Ok(CommandDeclaration {
        name: command.name.to_string(),
        code: command.ocf(),
        completion,
        request,
        location: path.to_path_buf(),
    })
}

fn wire_layout(
    fields: Option<&Fields>,
    minimum: &WireSize,
    maximum: &WireSize,
    maximum_cap: Option<usize>,
    wire_type_shapes: &WireTypeShapes,
) -> Result<WireLayout, String> {
    let segments = fields.map_or_else(
        || Ok(Vec::new()),
        |fields| {
            fields
                .fields()
                .iter()
                .map(|field| field_segments(field, wire_type_shapes))
                .collect::<Result<Vec<_>, _>>()
                .map(|segments| segments.into_iter().flatten().collect::<Vec<_>>())
        },
    )?;
    let minimum = resolve_wire_size(minimum, wire_type_shapes)?;
    let maximum = resolve_wire_size(maximum, wire_type_shapes)?;
    let maximum = maximum_cap.map_or(maximum, |cap| maximum.min(cap));
    let minimum = u32::try_from(minimum).expect("HCI envelopes fit in u32");
    let maximum = u32::try_from(maximum).expect("HCI envelopes fit in u32");
    WireLayout::with_envelope(Envelope::bounded(minimum, maximum), segments)
        .ok_or_else(|| "declarative wire layout is inconsistent with its envelope".to_owned())
}

fn field_segments(
    field: &stm32wb_hci_schema::Field,
    wire_type_shapes: &WireTypeShapes,
) -> Result<Vec<WireSegment>, String> {
    match &field.encoding {
        FieldEncoding::Fixed(_) => {
            let width = resolve_type_width(&field.ty, wire_type_shapes)?;
            Ok(simple_type_name(&field.ty)
                .and_then(|name| {
                    expanded_wire_type_shape(&name, width, wire_type_shapes, &mut BTreeSet::new())
                })
                .map_or_else(
                    || vec![WireSegment::fixed(width)],
                    |widths| widths.into_iter().map(WireSegment::fixed).collect(),
                ))
        }
        FieldEncoding::Variable(encoding) => variable_segments(encoding, wire_type_shapes),
    }
}

fn variable_segments(
    encoding: &stm32wb_hci_schema::VariableEncoding,
    wire_type_shapes: &WireTypeShapes,
) -> Result<Vec<WireSegment>, String> {
    let storage_min_len = resolve_wire_size(&encoding.storage_min_size(), wire_type_shapes)?;
    let storage_max_len = resolve_wire_size(&encoding.storage_max_size(), wire_type_shapes)?;
    let semantic_min_len = resolve_wire_size(&encoding.min_size(), wire_type_shapes)?;
    let semantic_max_len = resolve_wire_size(&encoding.max_size(), wire_type_shapes)?;
    if storage_min_len > semantic_min_len {
        return Err(format!(
            "variable field storage minimum {storage_min_len} is larger than semantic minimum {semantic_min_len}"
        ));
    }
    if storage_max_len < semantic_max_len {
        return Err(format!(
            "variable field storage maximum {storage_max_len} is smaller than semantic maximum {semantic_max_len}"
        ));
    }
    let segments = match &encoding.shape {
        VariableEncodingShape::CountedBytes {
            count,
            min_len: _,
            max_len,
        } => {
            let count_width = usize::try_from(resolve_type_width(&count.ty, wire_type_shapes)?)
                .expect("u32 wire widths fit usize");
            validate_integer_capacity("counted byte count", count_width, max_len.value)?;
            let storage_minimum = storage_elements(storage_min_len, count_width, 1, "minimum")?;
            let storage_maximum = storage_elements(storage_max_len, count_width, 1, "maximum")?;
            vec![
                WireSegment::fixed(wire_width(count_width)),
                WireSegment::variable_with_semantic(
                    1,
                    wire_width(storage_minimum),
                    wire_width(storage_maximum),
                    VariableSemantic::Counted {
                        prefix_width: wire_width(count_width),
                    },
                ),
            ]
        }
        VariableEncodingShape::CountedItems {
            count,
            item,
            min_items: _,
            max_items,
        } => {
            let count_width = usize::try_from(resolve_type_width(&count.ty, wire_type_shapes)?)
                .expect("u32 wire widths fit usize");
            let item_width = usize::try_from(resolve_type_width(&item.ty, wire_type_shapes)?)
                .expect("u32 wire widths fit usize");
            validate_integer_capacity("counted item count", count_width, max_items.value)?;
            let storage_minimum =
                storage_elements(storage_min_len, count_width, item_width, "minimum")?;
            let storage_maximum =
                storage_elements(storage_max_len, count_width, item_width, "maximum")?;
            vec![
                WireSegment::fixed(wire_width(count_width)),
                WireSegment::variable_with_semantic(
                    wire_width(item_width),
                    wire_width(storage_minimum),
                    wire_width(storage_maximum),
                    VariableSemantic::Counted {
                        prefix_width: wire_width(count_width),
                    },
                ),
            ]
        }
        VariableEncodingShape::Tagged(tagged) => {
            let tag_width = usize::try_from(resolve_type_width(&tagged.tag.ty, wire_type_shapes)?)
                .expect("u32 wire widths fit usize");
            for variant in &tagged.variants {
                validate_integer_capacity("tagged value", tag_width, variant.tag.value)?;
            }
            let variants = tagged
                .variants
                .iter()
                .map(|variant| {
                    let payload_widths = variant
                        .fields
                        .fields()
                        .iter()
                        .map(|field| field_segments(field, wire_type_shapes))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .map(|segment| match segment {
                            WireSegment::Fixed { length, .. } => length,
                            WireSegment::Variable { .. } => {
                                unreachable!("tagged variants contain only fixed fields")
                            }
                        })
                        .collect();
                    Ok(TaggedVariantLayout {
                        tag: u64::try_from(variant.tag.value).expect("HCI tags fit in u64"),
                        payload_widths,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let required_min = variants
                .iter()
                .map(|variant| {
                    usize::try_from(variant.payload_widths.iter().sum::<u32>())
                        .expect("u32 wire widths fit usize")
                        + tag_width
                })
                .min()
                .expect("tagged declarations require a variant");
            let required_max = variants
                .iter()
                .map(|variant| {
                    usize::try_from(variant.payload_widths.iter().sum::<u32>())
                        .expect("u32 wire widths fit usize")
                        + tag_width
                })
                .max()
                .expect("tagged declarations require a variant");
            if (tagged.min_len.value, tagged.max_len.value) != (required_min, required_max) {
                return Err(format!(
                    "tagged field declares lengths {}..={}, but its variants require {required_min}..={required_max}",
                    tagged.min_len.value, tagged.max_len.value,
                ));
            }
            let storage_minimum = storage_elements(storage_min_len, tag_width, 1, "minimum")?;
            let storage_maximum = storage_elements(storage_max_len, tag_width, 1, "maximum")?;
            vec![
                WireSegment::fixed(wire_width(tag_width)),
                WireSegment::variable_with_semantic(
                    1,
                    wire_width(storage_minimum),
                    wire_width(storage_maximum),
                    VariableSemantic::Tagged {
                        tag_width: wire_width(tag_width),
                        variants,
                    },
                ),
            ]
        }
        VariableEncodingShape::LengthPrefixedRecords {
            record_len,
            length,
            min_record_len,
            max_len,
        } => {
            let record_len_width =
                usize::try_from(resolve_type_width(&record_len.ty, wire_type_shapes)?)
                    .expect("u32 wire widths fit usize");
            let length_width = usize::try_from(resolve_type_width(&length.ty, wire_type_shapes)?)
                .expect("u32 wire widths fit usize");
            validate_integer_capacity("record byte length", length_width, max_len.value)?;
            let prefix_width = record_len_width + length_width;
            let storage_minimum = storage_elements(storage_min_len, prefix_width, 1, "minimum")?;
            let storage_maximum = storage_elements(storage_max_len, prefix_width, 1, "maximum")?;
            vec![
                WireSegment::fixed(wire_width(record_len_width)),
                WireSegment::fixed(wire_width(length_width)),
                WireSegment::variable_with_semantic(
                    1,
                    wire_width(storage_minimum),
                    wire_width(storage_maximum),
                    VariableSemantic::LengthPrefixedRecords {
                        record_len_width: wire_width(record_len_width),
                        length_width: wire_width(length_width),
                        minimum_record_len: Some(wire_width(min_record_len.value)),
                    },
                ),
            ]
        }
        VariableEncodingShape::TaggedItems(tagged) => {
            let tag_width = usize::try_from(resolve_type_width(&tagged.tag.ty, wire_type_shapes)?)
                .expect("u32 wire widths fit usize");
            let length_width =
                usize::try_from(resolve_type_width(&tagged.length.ty, wire_type_shapes)?)
                    .expect("u32 wire widths fit usize");
            validate_integer_capacity(
                "tagged item byte length",
                length_width,
                tagged.max_len.value,
            )?;
            let variants = tagged
                .variants
                .iter()
                .map(|variant| {
                    validate_integer_capacity("tagged item value", tag_width, variant.tag.value)?;
                    let item_width = resolve_type_width(&variant.item.ty, wire_type_shapes)?;
                    let expected = u32::try_from(tagged.max_len.value)
                        .expect("HCI lengths fit u32")
                        / item_width;
                    if u32::try_from(variant.max_items.value).expect("HCI counts fit u32")
                        != expected
                    {
                        return Err(format!(
                            "tagged_items variant {:#x} declares {} items, but max_len {} and canonical item width {item_width} allow {expected}",
                            variant.tag.value, variant.max_items.value, tagged.max_len.value,
                        ));
                    }
                    Ok(TaggedItemsVariantLayout {
                        tag: u64::try_from(variant.tag.value).expect("HCI tags fit in u64"),
                        item_width,
                        maximum_items: wire_width(variant.max_items.value),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let prefix_width = tag_width + length_width;
            let storage_minimum = storage_elements(storage_min_len, prefix_width, 1, "minimum")?;
            let storage_maximum = storage_elements(storage_max_len, prefix_width, 1, "maximum")?;
            vec![
                WireSegment::fixed(wire_width(tag_width)),
                WireSegment::fixed(wire_width(length_width)),
                WireSegment::variable_with_semantic(
                    1,
                    wire_width(storage_minimum),
                    wire_width(storage_maximum),
                    VariableSemantic::TaggedItems {
                        tag_width: wire_width(tag_width),
                        length_width: wire_width(length_width),
                        variants,
                    },
                ),
            ]
        }
        VariableEncodingShape::TrailingBytes {
            min_len: _,
            max_len: _,
        } => vec![WireSegment::variable_with_semantic(
            1,
            wire_width(storage_min_len),
            wire_width(storage_max_len),
            VariableSemantic::TrailingBytes,
        )],
        VariableEncodingShape::BitmapItems {
            bitmap, mask, item, ..
        } => {
            let item_width = usize::try_from(resolve_type_width(&item.ty, wire_type_shapes)?)
                .expect("u32 wire widths fit usize");
            let storage_minimum = storage_elements(storage_min_len, 0, item_width, "minimum")?;
            let storage_maximum = storage_elements(storage_max_len, 0, item_width, "maximum")?;
            vec![WireSegment::variable_with_semantic(
                wire_width(item_width),
                wire_width(storage_minimum),
                wire_width(storage_maximum),
                VariableSemantic::BitmapItems {
                    bitmap_field: bitmap.to_string(),
                    mask: u64::try_from(mask.value).expect("HCI bitmaps fit in u64"),
                },
            )]
        }
    };
    Ok(segments)
}

fn wire_width(value: usize) -> u32 {
    u32::try_from(value).expect("HCI field widths fit in u32")
}

fn validate_integer_capacity(label: &str, width: usize, maximum: usize) -> Result<(), String> {
    if width == 0 {
        return Err(format!("{label} canonical wire width must be nonzero"));
    }
    if width < core::mem::size_of::<usize>() {
        let capacity = (1usize << (width * u8::BITS as usize)) - 1;
        if maximum > capacity {
            return Err(format!(
                "{label} maximum {maximum} does not fit in canonical {width}-byte width"
            ));
        }
    }
    Ok(())
}

fn storage_elements(
    storage_len: usize,
    prefix_width: usize,
    element_width: usize,
    bound: &str,
) -> Result<usize, String> {
    if element_width == 0 {
        return Err("variable item canonical wire width must be nonzero".to_owned());
    }
    let payload = storage_len.checked_sub(prefix_width).ok_or_else(|| {
        format!("variable field storage {bound} is smaller than its canonical prefix")
    })?;
    if payload % element_width != 0 {
        return Err(format!(
            "variable field storage {bound} leaves {payload} bytes, which is not divisible by its canonical {element_width}-byte item width"
        ));
    }
    Ok(payload / element_width)
}

fn parse_vendor_event_declarations(
    file: &File,
    target: ComplianceTarget,
    path: &Path,
    wire_type_shapes: &WireTypeShapes,
) -> Result<BTreeMap<u16, EventDeclaration>, String> {
    if !attrs_active(&file.attrs, target, path)? {
        return Err(format!(
            "{}: VendorEvent source is disabled for selected target {target}",
            path.display()
        ));
    }

    let mut macros = Vec::new();
    collect_vendor_event_macros(&file.items, target, path, &mut macros)?;
    let [item] = macros.as_slice() else {
        return Err(format!(
            "{}: found {} active `vendor_event!` catalogs; expected exactly one",
            path.display(),
            macros.len()
        ));
    };
    let invocation =
        syn::parse2::<SchemaVendorEvents>(item.mac.tokens.clone()).map_err(|error| {
            format!(
                "{}: unsupported vendor_event! declaration: {error}",
                path.display()
            )
        })?;

    let mut events = BTreeMap::new();
    for definition in invocation.events {
        if !attrs_active(&definition.attrs, target, path)? {
            continue;
        }
        let payload_maximum = resolve_wire_size(&definition.payload.max_size(), wire_type_shapes)?;
        if payload_maximum > 253 {
            return Err(format!(
                "vendor event `{}` payload is at most 253 bytes, but its canonical types allow {payload_maximum}",
                definition.name,
            ));
        }
        let event = EventDeclaration {
            name: definition.name.to_string(),
            code: definition.code,
            payload: wire_layout(
                definition.payload.fields(),
                &definition.payload.min_size(),
                &definition.payload.max_size(),
                None,
                wire_type_shapes,
            )?,
            location: path.to_path_buf(),
        };
        if let Some(previous) = events.insert(event.code, event.clone()) {
            return Err(format!(
                "{}: events `{}` and `{}` both declare active vendor code 0x{:04X}",
                path.display(),
                previous.name,
                event.name,
                event.code,
            ));
        }
    }
    if events.is_empty() {
        return Err(format!(
            "{}: vendor_event! has no active declarations for {target}",
            path.display()
        ));
    }
    Ok(events)
}

fn collect_vendor_event_macros<'ast>(
    items: &'ast [Item],
    target: ComplianceTarget,
    path: &Path,
    macros: &mut Vec<&'ast ItemMacro>,
) -> Result<(), String> {
    for item in items {
        if !item_is_active(item, target, path)? {
            continue;
        }
        match item {
            Item::Macro(item) if is_macro_named(&item.mac.path, "vendor_event") => {
                macros.push(item);
            }
            Item::Mod(module) if module.content.is_some() => {
                let (_, nested) = module.content.as_ref().expect("checked above");
                collect_vendor_event_macros(nested, target, path, macros)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn item_is_active(item: &Item, target: ComplianceTarget, path: &Path) -> Result<bool, String> {
    let attributes = match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        Item::Verbatim(_) => return Ok(true),
        _ => return Ok(true),
    };
    attrs_active(attributes, target, path)
}

fn is_macro_named(path: &SynPath, name: &str) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn version(major: u16, minor: u16, patch: u16) -> crate::FirmwareVersion {
        crate::FirmwareVersion::new(major, minor, patch)
    }

    fn target(firmware: crate::FirmwareVersion) -> ComplianceTarget {
        ComplianceTarget::new(
            firmware,
            crate::McuFamily::Wb5x,
            crate::StackProfile::FullExtended,
        )
    }

    fn fixture_commands(
        source: &str,
        firmware: crate::FirmwareVersion,
    ) -> BTreeMap<String, CommandDeclaration> {
        let path = PathBuf::from("fixture.rs");
        let unit = SourceUnit {
            path: path.clone(),
            file: syn::parse_file(source).unwrap(),
        };
        collect_commands(
            std::slice::from_ref(&unit),
            target(firmware),
            &fixture_wire_types(),
        )
        .unwrap()
    }

    fn fixture_wire_types() -> WireTypeShapes {
        [
            ("Role", 1),
            ("ConnHandle", 2),
            ("AttributeHandle", 2),
            ("IoCapability", 1),
            ("Phy", 5),
        ]
        .into_iter()
        .map(|(name, width)| {
            (
                name.to_owned(),
                WireTypeShape {
                    width,
                    components: Vec::new(),
                },
            )
        })
        .collect()
    }

    fn assert_complete_envelope(completion: &CommandCompletion, expected: Envelope) {
        let CommandCompletion::CommandComplete { returns } = completion else {
            panic!("expected Command Complete");
        };
        assert_eq!(returns.envelope(), expected);
    }

    #[test]
    fn keeps_declaration_return_metadata() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = ();
                    Completion = CommandComplete;
                    Return = Result { value: [u8; 8], };
                }
            }
            impl<T> Commands for T {
                hci_impl_params!(command, Params, Current);
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 0));
        let declaration = declarations.get("Current").unwrap();
        assert_eq!(declaration.code, 0x0003);
        assert_complete_envelope(&declaration.completion, Envelope::fixed(8));
        assert_eq!(declaration.request, Envelope::fixed(0));
    }

    #[test]
    fn parses_declarative_fixed_command_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = {
                        role: Role,
                        enabled: bool,
                        name_len: u8,
                    };
                    Completion = CommandComplete;
                    Return = Result {
                        first_handle: AttributeHandle,
                        second_handle: AttributeHandle,
                        third_handle: AttributeHandle,
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::try_new(); }
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 0));
        let declaration = declarations.get("Current").unwrap();
        assert_complete_envelope(&declaration.completion, Envelope::fixed(6));
        assert_eq!(declaration.request, Envelope::fixed(3));
    }

    #[test]
    fn parses_the_qualified_proc_macro_with_the_same_declaration_contract() {
        let source = r#"
            stm32wb_hci_macros::vendor_cmd! {
                GapSetIoCapability(cgid = 0x1, cid = 0x05) {
                    Params = { io_capability: IoCapability, };
                    Completion = CommandComplete;
                    Return = ();
                }
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 1));
        let declaration = declarations.get("GapSetIoCapability").unwrap();
        assert_eq!(declaration.code, 0x085);
        assert_eq!(declaration.request, Envelope::fixed(1));
        assert_complete_envelope(&declaration.completion, Envelope::fixed(0));
    }

    #[test]
    fn parses_explicit_command_status_shape() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { procedure: u8, };
                    Completion = CommandStatus;
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 0));
        let declaration = declarations.get("Current").unwrap();
        assert_eq!(declaration.completion, CommandCompletion::CommandStatus);
        assert_eq!(declaration.request, Envelope::fixed(1));
    }

    #[test]
    fn parses_counted_request_and_bounded_return_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        conn_handle: ConnHandle,
                        handles: &'a [AttributeHandle] => {
                            kind: counted_items,
                            count: u8,
                            item: AttributeHandle,
                            max_items: 126,
                        },
                    };
                    Completion = CommandComplete;
                    Return = Result {
                        total_length: u16,
                        value: BoundedBytes<249> => {
                            kind: counted_bytes,
                            count: u16,
                            max_len: 249,
                        },
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 0));
        let declaration = declarations.get("Current").unwrap();
        assert_complete_envelope(&declaration.completion, Envelope::bounded(4, 253));
        assert_eq!(declaration.request, Envelope::bounded(3, 255));
        assert!(matches!(
            declaration.request.segments(),
            Some([
                WireSegment::Fixed { length: 2, .. },
                WireSegment::Fixed { length: 1, .. },
                WireSegment::Variable {
                    semantic: Some(VariableSemantic::Counted { prefix_width: 1 }),
                    ..
                },
            ])
        ));
        let CommandCompletion::CommandComplete { returns } = &declaration.completion else {
            panic!("expected Command Complete");
        };
        assert!(matches!(
            returns.segments(),
            Some([
                WireSegment::Fixed { length: 2, .. },
                WireSegment::Fixed { length: 2, .. },
                WireSegment::Variable {
                    semantic: Some(VariableSemantic::Counted { prefix_width: 2 }),
                    ..
                },
            ])
        ));
    }

    #[test]
    fn parses_inline_trailing_byte_returns() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { offset: u8, };
                    Completion = CommandComplete;
                    Return = Result {
                        value: BoundedBytes<16> => {
                            kind: trailing_bytes,
                            min_len: 1,
                            max_len: 16,
                        },
                    };
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::new(); }
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 0));
        let declaration = declarations.get("Current").unwrap();
        assert_eq!(declaration.request, Envelope::fixed(1));
        assert_complete_envelope(&declaration.completion, Envelope::bounded(1, 16));
        let CommandCompletion::CommandComplete { returns } = &declaration.completion else {
            panic!("expected Command Complete");
        };
        assert!(matches!(
            returns.segments(),
            Some([WireSegment::Variable {
                semantic: Some(VariableSemantic::TrailingBytes),
                ..
            }])
        ));
    }

    #[test]
    fn rejects_fields_after_trailing_bytes() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        value: &'a [u8] => {
                            kind: trailing_bytes,
                            min_len: 0,
                            max_len: 16,
                        },
                        suffix: u8,
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("trailing_bytes must be the final declarative field"));
    }

    #[test]
    fn rejects_invalid_or_legacy_command_ids() {
        for (source, expected) in [
            (
                "vendor_cmd! { Current(cgid = 0x8, cid = 0x01) {} }",
                "command group ID must fit in three bits",
            ),
            (
                "vendor_cmd! { Current(cgid = 0x1, cid = 0x80) {} }",
                "command ID must fit in seven bits",
            ),
            ("vendor_cmd! { Current(CURRENT) {} }", "expected `cgid`"),
        ] {
            let file = syn::parse_file(source).unwrap();
            let Item::Macro(item) = &file.items[0] else {
                panic!("expected vendor_cmd! macro item");
            };
            let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
                .unwrap_err();
            assert!(error.contains(expected), "{error}");
        }

        let unit = SourceUnit {
            path: PathBuf::from("fixture.rs"),
            file: syn::parse_file(
                r#"
                    vendor_cmd! { First(cgid = 0x1, cid = 0x02) { Params = (); Completion = CommandStatus; } }
                    vendor_cmd! { Second(cgid = 0x1, cid = 0x02) { Params = (); Completion = CommandStatus; } }
                "#,
            )
            .unwrap(),
        };
        let error = collect_commands(
            std::slice::from_ref(&unit),
            target(version(0, 17, 0)),
            &WireTypeShapes::new(),
        )
        .unwrap_err();
        assert!(error.contains("both declare active vendor OCF 0x082"));
    }

    #[test]
    fn parses_tagged_and_bitmap_selected_request_shapes() {
        let source = r#"
            pub trait Commands { async fn command(&self); }
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        scanning_phys: u8,
                        phy_params: &'a [Phy] => {
                            kind: bitmap_items,
                            bitmap: scanning_phys,
                            mask: 0x05,
                            item: Phy,
                            max_items: 2,
                        },
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8,
                            variants: {
                                Uuid::Uuid16(value) => {
                                    tag: 0x01,
                                    fields: { value: u16, },
                                },
                                Uuid::Uuid128(value) => {
                                    tag: 0x02,
                                    fields: { value: [u8; 16], },
                                },
                            },
                            min_len: 3,
                            max_len: 17,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
            impl<T> Commands for T {
                async fn command(&self) { Current::try_new(); }
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 0));
        let declaration = declarations.get("Current").unwrap();
        assert_eq!(declaration.completion, CommandCompletion::CommandStatus);
        assert_eq!(declaration.request, Envelope::bounded(4, 28));
        assert!(matches!(
            declaration.request.segments(),
            Some([
                WireSegment::Fixed { length: 1, .. },
                WireSegment::Variable {
                    semantic: Some(VariableSemantic::BitmapItems {
                        bitmap_field,
                        mask: 0x05,
                    }),
                    ..
                },
                WireSegment::Fixed { length: 1, .. },
                WireSegment::Variable {
                    semantic: Some(VariableSemantic::Tagged { variants, .. }),
                    ..
                },
            ]) if bitmap_field == "scanning_phys" && variants.len() == 2
        ));
    }

    #[test]
    fn parses_constraints_and_rejects_unknown_parameter_references() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        minimum: u16,
                        maximum: u16,
                        mode: u8,
                        data: &'a [u8] => {
                            kind: counted_bytes,
                            count: u8,
                            max_len: 16,
                        },
                    };
                    Constraints = {
                        self.minimum <= self.maximum;
                        self.minimum in 0x20..=0x4000;
                        self.mode in [0x00, 0x02];
                        self.maximum in [0] || self.maximum in 0x20..=0x4000;
                        (self.minimum == 0) iff (self.maximum == 0);
                        (self.minimum in 0x20..=0x4000
                            && self.maximum in 0x20..=0x4000)
                            implies self.minimum <= self.maximum;
                        self.mode == 0x00 implies self.maximum == 0;
                        self.mode == 0x02 implies self.maximum in 0x20..=0x4000;
                        self.data.len() <= self.mode;
                        self.mode == 0x02 implies self.data.len() >= 1;
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let declarations = fixture_commands(source, version(0, 17, 0));
        assert_eq!(
            declarations.get("Current").unwrap().request,
            Envelope::bounded(6, 22)
        );

        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = { value: u8, };
                    Constraints = { self.value == self.missing; };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("unknown parameter(s): missing"), "{error}");
    }

    #[test]
    fn rejects_removed_payload_kind() {
        for source in [
            r#"
                    vendor_cmd! {
                        Current(cgid = 0x0, cid = 0x03) {
                            Params<'a> = {
                                uuid: &'a Uuid => {
                                    kind: payload,
                                    min_len: 3,
                                    max_len: 17,
                                },
                            };
                            Completion = CommandStatus;
                        }
                    }
                "#,
            r#"
                    vendor_cmd! {
                        Current(cgid = 0x0, cid = 0x03) {
                            Params = ();
                            Completion = CommandComplete;
                            Return = Result {
                                uuid: Uuid => {
                                    kind: payload,
                                    min_len: 3,
                                    max_len: 17,
                                },
                            };
                        }
                    }
                "#,
        ] {
            let file = syn::parse_file(source).unwrap();
            let Item::Macro(item) = &file.items[0] else {
                panic!("expected vendor_cmd! macro item");
            };
            let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
                .unwrap_err();
            assert!(
                error.contains("unknown declarative variable kind `payload`"),
                "{error}"
            );
        }
    }

    #[test]
    fn rejects_incorrect_tagged_range_and_bitmap_cardinality() {
        let bad_tagged = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8,
                            variants: {
                                Uuid::Uuid16(value) => {
                                    tag: 0x01,
                                    fields: { value: u16, },
                                },
                                Uuid::Uuid128(value) => {
                                    tag: 0x02,
                                    fields: { value: [u8; 16], },
                                },
                            },
                            min_len: 2,
                            max_len: 17,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(bad_tagged).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("variants require 3..=17"));

        let bad_bitmap = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        scanning_phys: u8,
                        phy_params: &'a [Phy] => {
                            kind: bitmap_items,
                            bitmap: scanning_phys,
                            mask: 0x05,
                            item: Phy,
                            max_items: 3,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(bad_bitmap).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("mask selects 2 bits but max_items is 3"));
    }

    #[test]
    fn rejects_tagged_payload_field_not_bound_by_pattern() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params<'a> = {
                        uuid: &'a Uuid => {
                            kind: tagged,
                            tag: u8,
                            variants: {
                                Uuid::Uuid16(actual) => {
                                    tag: 0x01,
                                    fields: { typo: u16, },
                                },
                            },
                            min_len: 3,
                            max_len: 3,
                        },
                    };
                    Completion = CommandStatus;
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("payload field `typo` is not bound"));
    }

    #[test]
    fn rejects_return_on_explicit_command_status() {
        let source = r#"
            vendor_cmd! {
                Current(cgid = 0x0, cid = 0x03) {
                    Params = ();
                    Completion = CommandStatus;
                    Return = ();
                }
            }
        "#;
        let file = syn::parse_file(source).unwrap();
        let Item::Macro(item) = &file.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("CommandStatus and must not declare Return"));
    }

    #[test]
    fn rejects_legacy_completion_inference_and_return_buffers() {
        let missing_completion = syn::parse_file(
            r#"vendor_cmd! { Current(cgid = 0x0, cid = 0x03) { Params = (); Return = (); } }"#,
        )
        .unwrap();
        let Item::Macro(item) = &missing_completion.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("missing a `Completion = ...` declaration"));

        let return_buffer = syn::parse_file(
            r#"
                vendor_cmd! {
                    Current(cgid = 0x0, cid = 0x03) {
                        Params = ();
                        Completion = CommandComplete;
                        Return = ReturnBuffer<9>;
                    }
                }
            "#,
        )
        .unwrap();
        let Item::Macro(item) = &return_buffer.items[0] else {
            panic!("expected vendor_cmd! macro item");
        };
        let error = parse_vendor_command(item, Path::new("fixture.rs"), &WireTypeShapes::new())
            .unwrap_err();
        assert!(error.contains("expected `()` or an inline named field body"));
    }

    #[test]
    fn loads_declarative_variable_shapes_from_the_real_crate() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci");
        let coverage = load_rust_catalog(&crate_dir, target(version(1, 17, 1))).unwrap();

        let update = coverage.commands.get("GapUpdateAdvertisingData").unwrap();
        assert_eq!(update.request, Envelope::bounded(1, 32));
        assert_complete_envelope(&update.completion, Envelope::fixed(0));

        let discoverable = coverage.commands.get("GapSetLimitedDiscoverable").unwrap();
        // Independent field capacities exceed one HCI command, but the
        // generated constructor rejects aggregate payloads above 255 bytes.
        assert_eq!(discoverable.request, Envelope::bounded(13, 255));

        let read = coverage.commands.get("GattReadHandleValue").unwrap();
        assert_eq!(read.request, Envelope::fixed(6));
        assert_complete_envelope(&read.completion, Envelope::bounded(4, 251));

        assert!(
            coverage
                .commands
                .contains_key("GattReadMultipleVarCharValue")
        );

        let tagged = coverage
            .commands
            .get("GattDiscoverPrimaryServicesByUUID")
            .unwrap();
        assert_eq!(tagged.request, Envelope::bounded(5, 19));
        assert_eq!(tagged.completion, CommandCompletion::CommandStatus);

        let bonded = coverage.commands.get("GapGetBondedDevices").unwrap();
        // Count plus at most 35 seven-byte address records; status is framing.
        assert_complete_envelope(&bonded.completion, Envelope::bounded(1, 246));

        let config = coverage.commands.get("HalReadConfigData").unwrap();
        assert_complete_envelope(&config.completion, Envelope::bounded(2, 17));

        let channels = coverage.commands.get("L2CocConnectConfirm").unwrap();
        assert_complete_envelope(&channels.completion, Envelope::bounded(1, 6));

        assert_eq!(coverage.events.len(), 61);
        let gap_procedure = coverage.events.get(&0x0407).unwrap();
        assert_eq!(gap_procedure.name, "GapProcedureComplete");
        assert_eq!(gap_procedure.payload, Envelope::bounded(3, 253));

        let bond_lost = coverage.events.get(&0x0405).unwrap();
        assert_eq!(bond_lost.payload, Envelope::fixed(0));

        let read_multiple = coverage.events.get(&0x0C15).unwrap();
        assert_eq!(read_multiple.payload, Envelope::bounded(3, 253));
    }

    #[test]
    fn production_command_catalog_uses_the_proc_macro_entry_point() {
        fn inspect(items: &[Item], path: &Path, command_count: &mut usize) {
            for item in items {
                match item {
                    Item::Macro(item) if is_macro_named(&item.mac.path, "vendor_cmd") => {
                        *command_count += 1;
                        let macro_path = item
                            .mac
                            .path
                            .segments
                            .iter()
                            .map(|segment| segment.ident.to_string())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            macro_path,
                            ["stm32wb_hci_macros", "vendor_cmd"],
                            "{} contains a vendor command that bypasses the shared parser",
                            path.display()
                        );
                    }
                    Item::Mod(module) if module.content.is_some() => {
                        let (_, nested) = module.content.as_ref().expect("checked above");
                        inspect(nested, path, command_count);
                    }
                    _ => {}
                }
            }
        }

        let command_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci/src/vendor/command");
        let mut command_count = 0;
        for entry in fs::read_dir(&command_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "rs")
                && path.file_name().is_some_and(|name| name != "mod.rs")
            {
                let file = read_rust_file(&path).unwrap();
                inspect(&file.items, &path, &mut command_count);
            }
        }
        assert_eq!(command_count, 143);
    }

    #[test]
    fn production_event_catalog_uses_the_proc_macro_and_shared_schema() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci/src/vendor/event/mod.rs");
        let file = read_rust_file(&path).unwrap();
        let macros = file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Macro(item) if is_macro_named(&item.mac.path, "vendor_event") => Some(item),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [catalog] = macros.as_slice() else {
            panic!("expected exactly one production vendor-event catalog");
        };
        let macro_path = catalog
            .mac
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        assert_eq!(macro_path, ["stm32wb_hci_macros", "vendor_event"]);

        let parsed = syn::parse2::<SchemaVendorEvents>(catalog.mac.tokens.clone()).unwrap();
        assert_eq!(parsed.events.len(), 66);
    }

    #[test]
    fn loads_unique_command_ids_for_every_declared_firmware() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stm32wb-hci");
        for firmware in crate::FirmwareVersion::declared_in_manifest(&crate_dir).unwrap() {
            load_rust_catalog(&crate_dir, target(firmware)).unwrap();
        }
    }

    #[test]
    fn selects_commands_from_declaration_cfg() {
        let source = r#"
            vendor_cmd! { 
                Current(cgid = 0x0, cid = 0x03) { 
                    Params = (); 
                    Completion = CommandStatus; 
                } 
            }

            #[cfg(since_fw_1_17_1)]
            vendor_cmd! {
                Retained(cgid = 0x0, cid = 0x01) {
                    Params = ();
                    Completion = CommandStatus;
                }
            }
        "#;

        let path = PathBuf::from("fixture.rs");
        let unit = SourceUnit {
            path,
            file: syn::parse_file(source).unwrap(),
        };

        let firmware = version(1, 17, 0);
        let declarations = collect_commands(
            std::slice::from_ref(&unit),
            target(firmware),
            &WireTypeShapes::new(),
        )
        .unwrap();

        assert_eq!(declarations.len(), 1);
        assert!(declarations.contains_key("Current"));
        assert!(!declarations.contains_key("Retained"));

        let future = collect_commands(
            std::slice::from_ref(&unit),
            target(version(1, 18, 0)),
            &WireTypeShapes::new(),
        )
        .unwrap();

        assert!(future.contains_key("Current"));
        assert!(future.contains_key("Retained"));
    }

    #[test]
    fn discovers_only_active_command_modules() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "stm32wb-compliance-command-modules-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let root = directory.join("mod.rs");
        fs::write(
            &root,
            r#"
                pub mod current;
                #[cfg(since_fw_1_17_1)]
                pub mod future;
            "#,
        )
        .unwrap();
        fs::write(directory.join("current.rs"), "").unwrap();
        fs::write(directory.join("future.rs"), "").unwrap();

        let mut sources = Vec::new();
        let mut visited = BTreeSet::new();
        collect_command_sources(
            root.clone(),
            read_rust_file(&root).unwrap(),
            target(version(0, 17, 0)),
            &mut visited,
            &mut sources,
        )
        .unwrap();
        let names = sources
            .iter()
            .filter_map(|source| source.path.file_name().and_then(|name| name.to_str()))
            .collect::<BTreeSet<_>>();
        assert!(names.contains("current.rs"));
        assert!(!names.contains("future.rs"));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn rejects_canonical_wire_declarations_that_drift_from_their_semantic_type() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "stm32wb-compliance-wire-types-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("types.rs"),
            r#"
                wire_type! {
                    adapters: [command];
                    ranged pub struct Drifted: u16 => 1 {
                        minimum: 0,
                        maximum: 1,
                    }
                }
            "#,
        )
        .unwrap();

        let error = load_wire_type_shapes(&directory, target(version(1, 24, 0)))
            .err()
            .expect("drifted canonical declaration must be rejected");
        assert!(
            error.contains("declares canonical width 1")
                && error.contains("semantic components require 2"),
            "{error}"
        );

        fs::remove_dir_all(directory).unwrap();
    }
}
