#![doc = include_str!("../README.md")]

mod document;
mod error;
pub mod format;
mod text;

use std::collections::{BTreeMap, HashMap};
use std::ops::Range as Span;

use slime_reflect::{Path, Problem, Reflect, ReflectError, Registry, Step, Value};

use crate::document::{Failure, Reader, Writer};
pub use crate::error::{ReadError, ReadProblem};

/// The size of the largest document read, in bytes: 64 MiB.
pub const MAX_SIZE: usize = 64 << 20;

/// The deepest that a document's values nest, tables and arrays within others, as read: 64 levels.
/// The parser refuses deeper ones before they are counted, past 80.
pub const MAX_DEPTH: usize = 64;

/// How strictly a document is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A source, which people write: a field that its type does not have is an error.
    Strict,
    /// A save, which an earlier version of a program may have written: a field that its type does
    /// not have, and a version of a type that the program does not know, are left out, each with a
    /// warning ([`Read::warnings`]).
    Lenient,
}

/// A value read, and what a lenient reading left out.
#[derive(Debug)]
pub struct Read {
    /// The value, of the type read, at that type's version.
    pub value: Value,
    /// What was left out, in [`Mode::Lenient`]: fields and types unknown; none in
    /// [`Mode::Strict`].
    pub warnings: Vec<ReadError>,
}

/// Writes `value`, a value of the type named `ty` in `registry`, as a document
/// ([`format`](mod@format)): the same value gives the same bytes on every platform. It writes the fields that
/// the value holds, all of a whole value and some of a partial one.
///
/// ```
/// use slime_reflect::{Reflect, Registry};
/// use slime_serialize::{Mode, read, write};
///
/// /// A lantern on a post.
/// #[derive(Debug, PartialEq, Reflect)]
/// #[reflect(name = "Lantern")]
/// struct Lantern {
///     intensity: f32,
///     lit: bool,
///     range: Option<f32>,
/// }
///
/// let mut registry = Registry::new();
/// registry.register::<Lantern>();
/// let lantern = Lantern { intensity: 140.0, lit: true, range: None };
/// let text = write(&registry, "Lantern", &lantern.to_value()).expect("a lantern");
/// assert_eq!(text, "intensity = 140.0\nlit = true\n");
/// let read = read(&registry, "Lantern", &text, Mode::Strict).expect("written as one");
/// assert_eq!(Lantern::from_value(&read.value), Ok(lantern));
/// ```
///
/// # Errors
///
/// When `value` is not a value of `ty`, with the path of the part at fault, as
/// [`Registry::check`] says, and when it holds a reference to an entity, which is not written.
pub fn write(registry: &Registry, ty: &str, value: &Value) -> Result<String, ReflectError> {
    let mut writer = Writer {
        registry,
        versions: BTreeMap::new(),
    };
    let document = writer.value(ty, value, &Path::default())?;
    let mut root = Vec::new();
    if !writer.versions.is_empty() {
        let versions = writer
            .versions
            .into_iter()
            .map(|(name, version)| (name, Value::Int(i64::from(version))))
            .collect();
        root.push((VERSIONS.to_owned(), Value::Struct(versions)));
    }
    match document {
        Value::Struct(fields) => root.extend(fields),
        other => root.push((VALUE.to_owned(), other)),
    }
    Ok(text::print(&root))
}

/// Reads the value of the type named `ty` in `registry` that `text`, a document, holds
/// ([`format`](mod@format)), at the type's version now. A field that the document leaves out takes its
/// default; one without a default is an error.
///
/// # Errors
///
/// When `text` is larger than [`MAX_SIZE`], nests deeper than [`MAX_DEPTH`], is not TOML, is of a
/// later version of the format, or does not hold a value of `ty`: a field missing, or one that the
/// type does not have in [`Mode::Strict`], a value of another kind, a number out of its field's
/// range or of its type's width, a type of a later version, or of an earlier one whose upgrade
/// refuses it ([`ReadProblem`]). The error says where, by line and column and by the path in the
/// value. It never panics.
pub fn read(registry: &Registry, ty: &str, text: &str, mode: Mode) -> Result<Read, ReadError> {
    read_document(registry, ty, text, mode, false)
}

/// Reads a part of a value of the type named `ty`, which `text` holds, as [`read`] does, but for
/// the fields that the document leaves out, which the value leaves out too, at every depth: a save's
/// differences, which an upgrade brings from an earlier version as it brings a whole value.
///
/// # Errors
///
/// As [`read`], but for missing fields.
pub fn read_partial(
    registry: &Registry,
    ty: &str,
    text: &str,
    mode: Mode,
) -> Result<Read, ReadError> {
    read_document(registry, ty, text, mode, true)
}

/// Reads the value of the type named `ty` that the file `file` holds, as [`read`] does; its errors
/// and warnings name the file.
///
/// # Errors
///
/// As [`read`], and when the file cannot be read, or is not UTF-8 ([`ReadProblem::Io`]).
pub fn read_file(
    registry: &Registry,
    ty: &str,
    file: &std::path::Path,
    mode: Mode,
) -> Result<Read, ReadError> {
    let size = std::fs::metadata(file)
        .map_err(|error| ReadError::io(file, error))?
        .len();
    if size > MAX_SIZE as u64 {
        return Err(ReadError::new(
            None,
            Path::default(),
            ReadProblem::TooLarge {
                size,
                limit: MAX_SIZE,
            },
        )
        .in_file(file));
    }
    let text = std::fs::read_to_string(file).map_err(|error| ReadError::io(file, error))?;
    let read = read(registry, ty, &text, mode).map_err(|error| error.in_file(file))?;
    Ok(Read {
        value: read.value,
        warnings: read
            .warnings
            .into_iter()
            .map(|warning| warning.in_file(file))
            .collect(),
    })
}

/// Writes `value` as [`write`](fn@write) does, its type registered in a registry of its own.
///
/// # Errors
///
/// As [`write`](fn@write): when `value` holds a reference to an entity.
pub fn write_as<T: Reflect>(value: &T) -> Result<String, ReflectError> {
    let mut registry = Registry::new();
    registry.register::<T>();
    write(&registry, &T::type_name(), &value.to_value())
}

/// Reads the value of `T` that `text` holds, as [`read`] does, `T` registered in a registry of its
/// own, and turns it into a `T` ([`Reflect::from_value`]), which its type's check accepts or
/// refuses; with what a lenient reading left out.
///
/// # Errors
///
/// As [`read`], and when `T`'s check refuses the value, without a line and a column then.
pub fn read_as<T: Reflect>(text: &str, mode: Mode) -> Result<(T, Vec<ReadError>), ReadError> {
    let mut registry = Registry::new();
    registry.register::<T>();
    let read = read(&registry, &T::type_name(), text, mode)?;
    let value = T::from_value(&read.value).map_err(|error| {
        ReadError::new(
            None,
            error.path().clone(),
            ReadProblem::Value(error.problem().clone()),
        )
    })?;
    Ok((value, read.warnings))
}

/// The root's key of the version of the format.
const FORMAT: &str = "$format";
/// The root's key of the versions of the types past version 1.
const VERSIONS: &str = "$versions";
/// The root's key of a value that is not written as a table.
const VALUE: &str = "$value";

fn read_document(
    registry: &Registry,
    ty: &str,
    text: &str,
    mode: Mode,
    partial: bool,
) -> Result<Read, ReadError> {
    if text.len() > MAX_SIZE {
        return Err(ReadError::new(
            None,
            Path::default(),
            ReadProblem::TooLarge {
                size: text.len() as u64,
                limit: MAX_SIZE,
            },
        ));
    }
    let parsed = text::parse(text).map_err(|(span, problem)| {
        ReadError::new(
            span.map(|span| position(text, span.start)),
            Path::default(),
            problem,
        )
    })?;
    let text::Parsed {
        mut root,
        spans,
        keys,
    } = parsed;
    let locate = |failure: Failure| {
        // A field unknown or given twice is at its key; any other problem at the value.
        let at_key = matches!(
            failure.problem,
            ReadProblem::Value(Problem::UnknownField { .. } | Problem::DuplicateField(_))
        );
        let span = match keys.get(&failure.at) {
            Some(key) if at_key => Some(key.clone()),
            _ => span_of(&spans, &failure.at),
        };
        ReadError::new(
            span.map(|span| position(text, span.start)),
            failure.path,
            failure.problem,
        )
    };
    let mut warnings = Vec::new();
    if let Some(found) = take(&mut root, FORMAT) {
        let at = Path::default().field(FORMAT);
        let problem = match found {
            Value::Int(version) if (1..=i64::from(format::VERSION)).contains(&version) => None,
            Value::Int(version) if version > i64::from(format::VERSION) => {
                Some(ReadProblem::NewerFormat {
                    found: version,
                    known: format::VERSION,
                })
            }
            _ => Some(ReadProblem::Value(Problem::Invalid(
                "the format's version is an integer from 1".to_owned(),
            ))),
        };
        if let Some(problem) = problem {
            return Err(locate(Failure {
                path: Path::default(),
                at,
                problem,
            }));
        }
    }
    let mut versions = HashMap::new();
    if let Some(table) = take(&mut root, VERSIONS) {
        let at = Path::default().field(VERSIONS);
        let Value::Struct(entries) = table else {
            return Err(locate(Failure {
                path: Path::default(),
                at,
                problem: ReadProblem::Value(Problem::Invalid(
                    "the types' versions are a table".to_owned(),
                )),
            }));
        };
        for (name, version) in entries {
            let at = at.clone().field(name.clone());
            let fail = |problem| Failure {
                path: Path::default(),
                at: at.clone(),
                problem: ReadProblem::Value(problem),
            };
            let Value::Int(version) = version else {
                return Err(locate(fail(Problem::Invalid(format!(
                    "the version of `{name}` is not an integer"
                )))));
            };
            let Some(registration) = registry.get(&name) else {
                let unknown = fail(Problem::NotRegistered(name));
                match mode {
                    Mode::Strict => return Err(locate(unknown)),
                    Mode::Lenient => {
                        warnings.push(locate(unknown));
                        continue;
                    }
                }
            };
            let known = registration.info().version;
            let version = match u32::try_from(version) {
                Ok(version) if version >= 1 => version,
                _ => {
                    return Err(locate(fail(Problem::Invalid(format!(
                        "the version of `{name}` is {version}: versions start at 1"
                    )))));
                }
            };
            if version > known {
                return Err(locate(fail(Problem::NewerVersion {
                    found: version,
                    known,
                })));
            }
            versions.insert(name, version);
        }
    }
    let (document, at) = match take(&mut root, VALUE) {
        Some(value) => {
            if let Some((key, _)) = root.first() {
                return Err(locate(Failure {
                    path: Path::default(),
                    at: Path::default().field(key.clone()),
                    problem: ReadProblem::Value(Problem::Invalid(format!(
                        "`{key}` beside `{VALUE}`, which is the whole value"
                    ))),
                }));
            }
            (value, Path::default().field(VALUE))
        }
        None => (Value::Struct(root), Path::default()),
    };
    let mut reader = Reader {
        registry,
        versions,
        mode,
        partial,
        warnings: Vec::new(),
    };
    let value = reader
        .value(ty, document, &Path::default(), &at, None)
        .map_err(locate)?;
    warnings.extend(reader.warnings.into_iter().map(locate));
    Ok(Read { value, warnings })
}

/// Takes the root's entry under `key` out of it.
fn take(root: &mut Vec<(String, Value)>, key: &str) -> Option<Value> {
    let index = root.iter().position(|(name, _)| name == key)?;
    Some(root.remove(index).1)
}

/// Where the part at `at` in the document lies in its text, or the nearest part that holds it, if
/// the parser said: a part that an upgrade made has no place of its own.
fn span_of(spans: &HashMap<Path, Span<usize>>, at: &Path) -> Option<Span<usize>> {
    let steps: &[Step] = at.steps();
    (0..=steps.len()).rev().find_map(|len| {
        let prefix = steps[..len]
            .iter()
            .cloned()
            .fold(Path::default(), Path::then);
        spans.get(&prefix).cloned()
    })
}

/// The line and the column, from 1, of the byte `offset` in `text`, the column in characters.
fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map_or(0, |newline| newline + 1);
    (line, before[start..].chars().count() + 1)
}
