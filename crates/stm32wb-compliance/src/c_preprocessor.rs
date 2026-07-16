//! Include-aware preprocessing for generated CubeWB C sources.
//!
//! Cube sources are read from immutable Git tag objects, materialized in a
//! temporary directory, and parsed by libclang. Libclang evaluates the real C
//! preprocessor environment (including headers and function-like macros) and
//! reports the exact byte ranges skipped by conditional directives. Masking
//! those ranges keeps Tree-sitter offsets aligned with the original tagged
//! source without maintaining a second C expression evaluator.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use clang::diagnostic::Severity;
use clang::{Clang, Index};
use tempfile::TempDir;

const BLE_CORE_DIR: &str = "Middlewares/ST/STM32_WPAN/ble/core";

/// Materialized BLE core sources from one immutable CubeWB tag.
pub(crate) struct TaggedCPreprocessor {
    _temporary: TempDir,
    core_dir: PathBuf,
}

impl TaggedCPreprocessor {
    pub(crate) fn new(cube_dir: &Path, tag: &str) -> Result<Self, String> {
        let temporary = tempfile::tempdir()
            .map_err(|error| format!("could not create a temporary Cube source tree: {error}"))?;
        let core_dir = temporary.path().join("ble-core");
        materialize_tagged_core(cube_dir, tag, &core_dir)?;
        Ok(Self {
            _temporary: temporary,
            core_dir,
        })
    }

    /// Evaluate one tagged source with the headers from the same tag.
    pub(crate) fn preprocess(&self, relative_path: &str, source: &str) -> Result<String, String> {
        let source_path = checked_source_path(&self.core_dir, relative_path)?;
        let materialized = fs::read_to_string(&source_path).map_err(|error| {
            format!(
                "could not read materialized Cube source {}: {error}",
                source_path.display()
            )
        })?;
        if materialized != source {
            return Err(format!(
                "materialized Cube source {} does not match the tagged blob being inspected",
                source_path.display()
            ));
        }
        preprocess_path(
            &source_path,
            source,
            &[self.core_dir.clone(), self.core_dir.join("template")],
            DiagnosticPolicy::Strict,
        )
    }
}

/// Preprocess a standalone fixture. Production tagged sources use
/// [`TaggedCPreprocessor`] so their real include tree is available.
#[cfg(test)]
pub(crate) fn preprocess_c_source(source: &str, source_name: &str) -> Result<String, String> {
    let temporary = tempfile::tempdir()
        .map_err(|error| format!("could not create a temporary C fixture: {error}"))?;
    let source_path = temporary.path().join("fixture.c");
    fs::write(&source_path, source)
        .map_err(|error| format!("could not write temporary C fixture: {error}"))?;
    preprocess_path(
        &source_path,
        source,
        &[temporary.path().to_path_buf()],
        DiagnosticPolicy::PreprocessorOnly,
    )
    .map_err(|message| format!("{source_name}: {message}"))
}

fn checked_source_path(core_dir: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative_path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "invalid relative Cube source path {relative_path:?}"
        ));
    }
    Ok(core_dir.join(path))
}

fn materialize_tagged_core(cube_dir: &Path, tag: &str, destination: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cube_dir)
        .args(["ls-tree", "-r", "--name-only", tag, "--", BLE_CORE_DIR])
        .output()
        .map_err(|error| format!("could not list {BLE_CORE_DIR} at {tag}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-tree for {tag}:{BLE_CORE_DIR} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let listing = String::from_utf8(output.stdout)
        .map_err(|error| format!("git ls-tree returned non-UTF-8 paths: {error}"))?;
    let prefix = format!("{BLE_CORE_DIR}/");
    let mut count = 0_usize;
    for tagged_path in listing.lines() {
        let relative_path = tagged_path.strip_prefix(&prefix).ok_or_else(|| {
            format!("git listed {tagged_path:?} outside requested directory {BLE_CORE_DIR}")
        })?;
        let destination_path = checked_source_path(destination, relative_path)?;
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "could not create temporary Cube directory {}: {error}",
                    parent.display()
                )
            })?;
        }

        let spec = format!("{tag}:{tagged_path}");
        let blob = Command::new("git")
            .arg("-C")
            .arg(cube_dir)
            .arg("show")
            .arg(&spec)
            .output()
            .map_err(|error| format!("could not run git show {spec}: {error}"))?;
        if !blob.status.success() {
            return Err(format!(
                "git show {spec} failed: {}",
                String::from_utf8_lossy(&blob.stderr).trim()
            ));
        }
        fs::write(&destination_path, blob.stdout).map_err(|error| {
            format!(
                "could not materialize tagged Cube source {}: {error}",
                destination_path.display()
            )
        })?;
        count += 1;
    }
    if count == 0 {
        return Err(format!("no Cube BLE core sources were found at {tag}"));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum DiagnosticPolicy {
    /// Tagged Cube input is expected to be a complete translation unit.
    Strict,
    /// Standalone unit-test snippets may intentionally omit unrelated C types.
    #[cfg(test)]
    PreprocessorOnly,
}

fn preprocess_path(
    source_path: &Path,
    source: &str,
    include_dirs: &[PathBuf],
    policy: DiagnosticPolicy,
) -> Result<String, String> {
    let _guard = clang_mutex()
        .lock()
        .map_err(|_| "libclang preprocessing lock is poisoned".to_owned())?;
    let clang = Clang::new().map_err(|error| format!("could not load libclang: {error}"))?;
    let index = Index::new(&clang, false, false);
    let mut arguments = vec!["-std=c11".to_owned()];
    arguments.extend(
        include_dirs
            .iter()
            .map(|directory| format!("-I{}", directory.display())),
    );
    for directory in c_system_search_paths()? {
        arguments.push("-isystem".to_owned());
        arguments.push(directory.display().to_string());
    }
    let mut parser = index.parser(source_path);
    parser
        .arguments(&arguments)
        .detailed_preprocessing_record(true)
        .keep_going(true);
    let translation_unit = parser.parse().map_err(|error| {
        format!(
            "libclang could not parse {}: {error:?}",
            source_path.display()
        )
    })?;

    let directive_ranges = directive_ranges(source)?;
    reject_diagnostics(&translation_unit, source_path, &directive_ranges, policy)?;

    let file = translation_unit.get_file(source_path).ok_or_else(|| {
        format!(
            "libclang did not retain the main source file {}",
            source_path.display()
        )
    })?;
    let mut output = source.as_bytes().to_vec();
    for (start, end) in &directive_ranges {
        mask_range(&mut output, *start, *end)?;
    }
    for range in file.get_skipped_ranges() {
        let start = usize::try_from(range.get_start().get_file_location().offset)
            .map_err(|_| "libclang skipped-range start does not fit usize".to_owned())?;
        let end = usize::try_from(range.get_end().get_file_location().offset)
            .map_err(|_| "libclang skipped-range end does not fit usize".to_owned())?;
        mask_range(&mut output, start, end)?;
    }
    String::from_utf8(output)
        .map_err(|error| format!("masked C source is no longer valid UTF-8: {error}"))
}

fn clang_mutex() -> &'static Mutex<()> {
    static CLANG: OnceLock<Mutex<()>> = OnceLock::new();
    CLANG.get_or_init(|| Mutex::new(()))
}

fn c_system_search_paths() -> Result<&'static [PathBuf], String> {
    static PATHS: OnceLock<Result<Vec<PathBuf>, String>> = OnceLock::new();
    match PATHS.get_or_init(|| {
        let driver =
            clang_sys::support::Clang::find(None, &["-std=c11".to_owned()]).ok_or_else(|| {
                "could not find a clang driver for its system header paths".to_owned()
            })?;
        driver.c_search_paths.ok_or_else(|| {
            format!(
                "could not discover C system header paths from {}",
                driver.path.display()
            )
        })
    }) {
        Ok(paths) => Ok(paths),
        Err(error) => Err(error.clone()),
    }
}

fn reject_diagnostics(
    translation_unit: &clang::TranslationUnit<'_>,
    source_path: &Path,
    directive_ranges: &[(usize, usize)],
    policy: DiagnosticPolicy,
) -> Result<(), String> {
    let mut errors = Vec::new();
    for diagnostic in translation_unit.get_diagnostics() {
        if diagnostic.get_severity() < Severity::Error {
            continue;
        }
        let location = diagnostic.get_location().get_file_location();
        let in_main_file = location
            .file
            .is_some_and(|file| file.get_path() == source_path);
        let in_directive = usize::try_from(location.offset).ok().is_some_and(|offset| {
            directive_ranges
                .iter()
                .any(|(start, end)| (*start..*end).contains(&offset))
        });
        let reject = matches!(policy, DiagnosticPolicy::Strict)
            || diagnostic.get_severity() == Severity::Fatal
            || (in_main_file && in_directive);
        if reject {
            let file = location
                .file
                .map(|file| file.get_path().display().to_string())
                .unwrap_or_else(|| source_path.display().to_string());
            errors.push(format!(
                "{file}:{}:{}: {}",
                location.line,
                location.column,
                diagnostic.get_text()
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "libclang rejected the C preprocessor input:\n{}",
            errors.join("\n")
        ))
    }
}

fn directive_ranges(source: &str) -> Result<Vec<(usize, usize)>, String> {
    let lines = line_ranges(source);
    let starts = directive_starts(source, &lines);
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if !starts[index] {
            index += 1;
            continue;
        }
        let start = lines[index].0;
        loop {
            let (line_start, line_end) = lines[index];
            let line = source[line_start..line_end].trim_end_matches(['\r', '\n']);
            let continued = line.ends_with('\\');
            index += 1;
            if !continued {
                ranges.push((start, line_end));
                break;
            }
            if index == lines.len() {
                return Err("preprocessor directive ends with a line continuation".to_owned());
            }
        }
    }
    Ok(ranges)
}

#[derive(Clone, Copy, Debug, Default)]
struct LexicalState {
    block_comment: bool,
    quote: Option<u8>,
}

fn directive_starts(source: &str, lines: &[(usize, usize)]) -> Vec<bool> {
    let mut state = LexicalState::default();
    lines
        .iter()
        .map(|&(start, end)| line_starts_directive(&source[start..end], &mut state))
        .collect()
}

fn line_starts_directive(line: &str, state: &mut LexicalState) -> bool {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut directive_position = state.quote.is_none();
    let mut directive = false;

    while index < bytes.len() {
        if state.block_comment {
            if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                state.block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote) = state.quote {
            if bytes[index] == b'\\' {
                index += 2;
            } else {
                if bytes[index] == quote {
                    state.quote = None;
                }
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            state.block_comment = true;
            index += 2;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            break;
        }
        if matches!(bytes[index], b'\'' | b'"') {
            state.quote = Some(bytes[index]);
            directive_position = false;
            index += 1;
            continue;
        }
        if directive_position && bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if directive_position && bytes[index] == b'#' {
            directive = true;
        }
        directive_position = false;
        index += 1;
    }
    directive
}

fn line_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            ranges.push((start, index + 1));
            start = index + 1;
        }
    }
    if start < source.len() {
        ranges.push((start, source.len()));
    }
    ranges
}

fn mask_range(output: &mut [u8], start: usize, end: usize) -> Result<(), String> {
    let output_len = output.len();
    let range = output.get_mut(start..end).ok_or_else(|| {
        format!(
            "libclang returned invalid source range {start}..{end} for {} bytes",
            output_len
        )
    })?;
    for byte in range {
        if !matches!(*byte, b'\r' | b'\n') {
            *byte = b' ';
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preprocess_fixture(files: &[(&str, &str)], main: &str) -> Result<String, String> {
        let temporary = tempfile::tempdir().unwrap();
        for (path, source) in files {
            let path = checked_source_path(temporary.path(), path).unwrap();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, source).unwrap();
        }
        let source_path = checked_source_path(temporary.path(), main).unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        preprocess_path(
            &source_path,
            &source,
            &[temporary.path().to_path_buf()],
            DiagnosticPolicy::Strict,
        )
    }

    #[test]
    fn follows_includes_and_uses_header_macros() {
        let source = preprocess_fixture(
            &[
                (
                    "configuration.h",
                    "#define API_LEVEL 2\n#define ENABLED(value) ((value) >= API_LEVEL)\n",
                ),
                (
                    "fixture.c",
                    "#include \"configuration.h\"\n#if ENABLED(2)\nint current;\n#else\nint old;\n#endif\n",
                ),
            ],
            "fixture.c",
        )
        .unwrap();
        assert!(source.contains("int current;"));
        assert!(!source.contains("int old;"));
    }

    #[test]
    fn logical_operators_use_c_short_circuit_semantics() {
        let source = preprocess_c_source(
            "#if 1 || (1 / 0)\nint selected;\n#endif\n#if 0 && (1 / 0)\nint wrong;\n#endif\n",
            "fixture.c",
        )
        .unwrap();
        assert!(source.contains("int selected;"));
        assert!(!source.contains("int wrong;"));
    }

    #[test]
    fn rejects_missing_includes_and_evaluated_preprocessor_errors() {
        let missing =
            preprocess_c_source("#include \"missing.h\"\nint value;\n", "fixture.c").unwrap_err();
        assert!(missing.contains("file not found"));

        let division =
            preprocess_c_source("#if 1 / 0\nint value;\n#endif\n", "fixture.c").unwrap_err();
        assert!(division.contains("division by zero"));
    }

    #[test]
    fn preserves_offsets_and_ignores_directive_decoys() {
        let source = r##"
/* #if 0 */
const char *example = "#else";
#if 0
int inactive;
#else
int active;
#endif
"##;
        let filtered = preprocess_c_source(source, "fixture.c").unwrap();
        assert_eq!(filtered.len(), source.len());
        assert!(filtered.contains("const char *example"));
        assert!(filtered.contains("int active;"));
        assert!(!filtered.contains("int inactive;"));
    }
}
