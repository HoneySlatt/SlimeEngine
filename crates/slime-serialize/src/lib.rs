#![doc = include_str!("../README.md")]

mod binary;
mod compose;
mod document;
mod error;
pub mod format;
mod text;

use std::io::Read as _;

use slime_reflect::{Path, Reflect, ReflectError, Registry, Value};

pub use crate::compose::{Composer, Document};
use crate::compose::{line_breaks, parsed, position, too_large};
pub use crate::error::{ReadError, ReadProblem};

/// The size of the largest document read, in bytes, as text or in the binary encoding: 16 MiB. A
/// document holds at most [`MAX_VALUES`] values too, which bounds the memory that reading it takes.
pub const MAX_SIZE: usize = 16 << 20;

/// The most values that a document read holds, its tables, arrays and the tables that dotted keys
/// make included: 2²⁰, a little over a million. A text's are counted before it is parsed, by an
/// upper bound linear in its size: one for each `=`, `,`, `[`, `{` and `.` that it holds, a float's
/// point or a quoted key's included; the binary encoding's as they are read.
///
/// The text's parser takes up to about 600 bytes of memory for each value counted, besides its keys
/// and texts: the documents of that many values that took the most to read, a million keys each
/// with its value, or a million tables, took 570 MB and up to 2 s in a release build.
pub const MAX_VALUES: usize = 1 << 20;

/// The most warnings that a lenient reading keeps ([`Read::warnings`], [`Document::warnings`]):
/// 100. Past them, one more counts those left out ([`ReadProblem::MoreLeftOut`]), so that a
/// document that holds many costs memory and time in proportion with its size.
pub const MAX_WARNINGS: usize = 100;

/// The deepest that a document's values nest, tables and arrays within others, as read: 64 levels,
/// in a text as in the binary encoding. The text's parser refuses deeper ones before they are counted,
/// past 80.
pub const MAX_DEPTH: usize = 64;

/// How a document names the entities and the assets that values refer to: entities by the
/// persistent identities of a world, which its documents give (`slime-world`); assets by their IDs,
/// written as UUIDs, unless the document names them otherwise, as a scene file names them by the
/// short names of its table of assets. An entity's handle holds within one run only: where a
/// document names no entity, a reference to an entity is neither written nor read.
pub trait References {
    /// The name that the document gives the entity whose handle has the bits `entity`, if it has
    /// one.
    fn name(&self, entity: u64) -> Option<String>;

    /// The bits of the handle of the entity that the document names `name`, if one is.
    fn entity(&self, name: &str) -> Option<u64>;

    /// The name that the document gives the asset whose ID has the bits `id`, if it has one: by
    /// default, its ID written as a UUID, as a document that names no entity writes it.
    fn asset_name(&self, id: u128) -> Option<String> {
        Some(document::uuid(id))
    }

    /// The bits of the ID of the asset that the document names `name`, if one is: by default, the
    /// ID that `name` writes as a UUID, in either case.
    fn asset(&self, name: &str) -> Option<u128> {
        document::parse_uuid(name)
    }
}

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
    /// What was left out, in [`Mode::Lenient`]: fields and types unknown, the first
    /// [`MAX_WARNINGS`], then one that counts the rest; none in [`Mode::Strict`].
    pub warnings: Vec<ReadError>,
}

/// Writes `value`, a whole value of the type named `ty` in `registry`, as a document
/// ([`format`](mod@format)): the same value gives the same bytes on every platform. It checks the
/// value as [`read`] does, so that what it writes can be read back.
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
/// When `value` is not a whole value of `ty`, with the path of the part at fault: a field missing, a
/// value of another kind, an integer wider than its type, an array of another length, a number
/// out of its field's range, NaN or an infinity in a field that has one; and when it holds a
/// reference to an entity, which is not written; and when the document would be larger than
/// [`MAX_SIZE`], hold more than [`MAX_VALUES`] values or nest deeper than [`MAX_DEPTH`], which
/// [`read`] refuses.
pub fn write(registry: &Registry, ty: &str, value: &Value) -> Result<String, ReflectError> {
    write_document(registry, ty, value, false)
}

/// Writes `value`, a part of a value of the type named `ty`, as [`write`](fn@write) does, but for
/// the fields that it leaves out, which the document leaves out too: a save's differences, which
/// [`read_partial`] reads back. Tuples, lists and options are written whole.
///
/// # Errors
///
/// As [`write`](fn@write), but for missing fields of structs and variants.
pub fn write_partial(registry: &Registry, ty: &str, value: &Value) -> Result<String, ReflectError> {
    write_document(registry, ty, value, true)
}

fn write_document(
    registry: &Registry,
    ty: &str,
    value: &Value,
    partial: bool,
) -> Result<String, ReflectError> {
    let mut composer = Composer::new(registry);
    if partial {
        composer.put_part(&[], ty, value)?;
    } else {
        composer.put(&[], ty, value)?;
    }
    composer.finish()
}

/// Reads the value of the type named `ty` in `registry` that `text`, a document, holds
/// ([`format`](mod@format)), at the type's version now. A field that the document leaves out takes its
/// default; one without a default is an error.
///
/// # Errors
///
/// When `text` is larger than [`MAX_SIZE`], holds more than [`MAX_VALUES`] values, nests deeper
/// than [`MAX_DEPTH`], is not TOML, is of a later version of the format, or does not hold a value
/// of `ty`: a field missing, or one that the
/// type does not have in [`Mode::Strict`], a value of another kind, a number out of its field's
/// range or of its type's width, a type of a later version, or of an earlier one whose upgrade
/// refuses it ([`ReadProblem`]). The error says where, by line and column and by the path in the
/// value. It never panics.
pub fn read(registry: &Registry, ty: &str, text: &str, mode: Mode) -> Result<Read, ReadError> {
    read_document(registry, ty, text, mode, false)
}

/// Reads a part of a value of the type named `ty`, which `text` holds, as [`read`] does, but for
/// the fields of structs and variants that the document leaves out, at every depth, which the value
/// leaves out too, tuples, lists and options read whole: a save's differences, which upgrades bring
/// from earlier versions, told that the value is partial ([`slime_reflect::Upgrade`]).
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
/// As [`read`], and when the file cannot be read ([`ReadProblem::Io`]) or is not UTF-8 text
/// ([`ReadProblem::NotText`]), at the first byte that is not.
pub fn read_file(
    registry: &Registry,
    ty: &str,
    file: &std::path::Path,
    mode: Mode,
) -> Result<Read, ReadError> {
    // At most one byte past the limit, whatever the file says its size is: a pipe or a device
    // says none.
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .and_then(|opened| opened.take(MAX_SIZE as u64 + 1).read_to_end(&mut bytes))
        .map_err(|error| ReadError::io(file, error))?;
    let size = bytes.len() as u64;
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
    let text = String::from_utf8(bytes).map_err(|error| {
        let valid = &error.as_bytes()[..error.utf8_error().valid_up_to()];
        let before = String::from_utf8_lossy(valid);
        ReadError::new(
            Some(position(&before, &line_breaks(&before), before.len())),
            Path::default(),
            ReadProblem::NotText,
        )
        .in_file(file)
    })?;
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

/// The document that `text` holds in the engine's binary encoding, read without its types, so that a
/// program that knows none, such as a cooker, encodes a source
/// (`docs/adr/0094-the-binary-encoding-of-documents.md`):
/// the bytes read through [`Document::parse_binary`] give the values that the text gives through
/// [`Document::parse`]. The same text gives the same bytes on every platform.
///
/// ```
/// use slime_reflect::{Registry, Value};
/// use slime_serialize::{Document, Mode, encode};
///
/// let bytes = encode("[lamps]\nporch = 30.0\n").expect("TOML");
/// let mut registry = Registry::new();
/// registry.register::<f32>();
/// let mut document = Document::parse_binary(&registry, &bytes, Mode::Strict).expect("a document");
/// assert_eq!(document.read(&["lamps", "porch"], "f32", None).expect("a number"), Some(Value::F32(30.0)));
/// ```
///
/// # Errors
///
/// When `text` is larger than [`MAX_SIZE`], holds more than [`MAX_VALUES`] values, nests deeper
/// than [`MAX_DEPTH`], is not TOML or holds a date or a time, which the engine's values do not
/// hold, as reading it refuses, at its line and column; when the encoding would be larger than
/// [`MAX_SIZE`], which [`Document::parse_binary`] refuses. A document that a [`Composer`] writes is
/// encoded so, from its text.
pub fn encode(text: &str) -> Result<Vec<u8>, ReadError> {
    let bytes = binary::encode(&parsed(text)?.root);
    if bytes.len() > MAX_SIZE {
        return Err(too_large(bytes.len()));
    }
    Ok(bytes)
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

fn read_document(
    registry: &Registry,
    ty: &str,
    text: &str,
    mode: Mode,
    partial: bool,
) -> Result<Read, ReadError> {
    let mut document = Document::parse(registry, text, mode)?;
    let value = document.root(ty, partial)?;
    Ok(Read {
        value,
        warnings: document.warnings(),
    })
}
