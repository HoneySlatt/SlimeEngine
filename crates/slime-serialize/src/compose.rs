//! Documents of several values, each a value of a registered type at its place in the document:
//! what a world's documents and saves are. [`write`](crate::write) and [`read`](crate::read) write
//! and read a document of one value through them.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use slime_reflect::{Path, Problem, ReflectError, Registry, Step, Value};

use crate::document::{Failure, Reader, Writer};
use crate::error::{ReadError, ReadProblem};
use crate::{MAX_DEPTH, MAX_SIZE, Mode, References, format, text};

/// The root's key of the version of the format.
const FORMAT: &str = "$format";
/// The root's key of the versions of the types past version 1.
const VERSIONS: &str = "$versions";
/// The root's key of a value that is not written as a table.
const VALUE: &str = "$value";

/// Writes a document of several values, each a value of a registered type at its place: the keys
/// of the tables that lead to it from the root, made where missing. One `$versions` holds the
/// versions of the types of all of them. Putting a value costs a lookup per key of its place,
/// whatever how many the document holds.
///
/// ```
/// use slime_reflect::{Registry, Value};
/// use slime_serialize::Composer;
///
/// let mut registry = Registry::new();
/// registry.register::<f32>();
/// let mut composer = Composer::new(&registry);
/// composer.put(&["lamps", "porch"], "f32", &Value::F32(30.0)).expect("a number");
/// composer.put(&["lamps", "gate"], "f32", &Value::F32(12.0)).expect("a number");
/// assert_eq!(composer.finish().expect("small"), "lamps = { porch = 30.0, gate = 12.0 }\n");
/// ```
pub struct Composer<'a> {
    writer: Writer<'a>,
    root: Vec<(String, Value)>,
    /// Each place that holds something, by its keys from the root: where it is in its table, and
    /// whether it is a table that the composer made to lead to a value, which a value can go into.
    places: HashMap<Vec<String>, (usize, bool)>,
}

impl<'a> Composer<'a> {
    /// A document that holds nothing yet, of values of the types of `registry`.
    pub fn new(registry: &'a Registry) -> Self {
        Self {
            writer: Writer {
                registry,
                references: None,
                transient: true,
                versions: BTreeMap::new(),
                partial: false,
            },
            root: Vec::new(),
            places: HashMap::new(),
        }
    }

    /// Names the entities that values refer to as `references` does, rather than refuse them.
    #[must_use]
    pub fn naming(mut self, references: &'a dyn References) -> Self {
        self.writer.references = Some(references);
        self
    }

    /// Leaves out the fields marked transient, as a save does: a reader gives them their defaults.
    #[must_use]
    pub fn without_transient(mut self) -> Self {
        self.writer.transient = false;
        self
    }

    /// Puts `value`, a whole value of the type named `ty`, at `place`, as [`write`](crate::write)
    /// writes it; at the root, for an empty `place`.
    ///
    /// # Errors
    ///
    /// As [`write`](crate::write), the path of the part at fault from the root; and when `place`
    /// goes through a value put, or holds one already.
    pub fn put(&mut self, place: &[&str], ty: &str, value: &Value) -> Result<(), ReflectError> {
        self.put_value(place, ty, value, false)
    }

    /// Puts `value`, a part of a value of the type named `ty`, at `place`, as
    /// [`write_partial`](crate::write_partial) writes it.
    ///
    /// # Errors
    ///
    /// As [`Composer::put`].
    pub fn put_part(
        &mut self,
        place: &[&str],
        ty: &str,
        value: &Value,
    ) -> Result<(), ReflectError> {
        self.put_value(place, ty, value, true)
    }

    fn put_value(
        &mut self,
        place: &[&str],
        ty: &str,
        value: &Value,
        partial: bool,
    ) -> Result<(), ReflectError> {
        let within = |error: ReflectError| {
            place.iter().rev().fold(error, |error, key| {
                error.within(Step::Field((*key).to_owned()))
            })
        };
        self.writer.partial = partial;
        let document = self
            .writer
            .value(ty, value, &Path::default(), None)
            .map_err(within)?;
        let occupied = || {
            within(ReflectError::new(Problem::Invalid(
                "the place holds a value already, or goes through one".to_owned(),
            )))
        };
        let Some((last, parents)) = place.split_last() else {
            if !self.root.is_empty() {
                return Err(occupied());
            }
            match document {
                Value::Struct(fields) => self.root = fields,
                other => self.root.push((VALUE.to_owned(), other)),
            }
            for (index, (key, _)) in self.root.iter().enumerate() {
                self.places.insert(vec![key.clone()], (index, false));
            }
            return Ok(());
        };
        let places = &mut self.places;
        let mut table = &mut self.root;
        let mut keys = Vec::with_capacity(place.len());
        for key in parents {
            keys.push((*key).to_owned());
            let index = match places.get(&keys) {
                Some(&(index, true)) => index,
                Some(&(_, false)) => return Err(occupied()),
                None => {
                    table.push(((*key).to_owned(), Value::Struct(Vec::new())));
                    places.insert(keys.clone(), (table.len() - 1, true));
                    table.len() - 1
                }
            };
            table = match &mut table[index].1 {
                Value::Struct(inner) => inner,
                _ => return Err(occupied()),
            };
        }
        keys.push((*last).to_owned());
        if places.contains_key(&keys) {
            return Err(occupied());
        }
        table.push(((*last).to_owned(), document));
        places.insert(keys, (table.len() - 1, false));
        Ok(())
    }

    /// The document's text: `$versions` first, if a type past version 1 is in it, then the values in
    /// the order they were put.
    ///
    /// # Errors
    ///
    /// When the text would be larger than [`MAX_SIZE`], or nest deeper than [`MAX_DEPTH`], which a
    /// reader refuses: what is written can be read.
    pub fn finish(self) -> Result<String, ReflectError> {
        let mut root = Vec::new();
        if !self.writer.versions.is_empty() {
            let versions = self
                .writer
                .versions
                .into_iter()
                .map(|(name, version)| (name, Value::Int(i64::from(version))))
                .collect();
            root.push((VERSIONS.to_owned(), Value::Struct(versions)));
        }
        root.extend(self.root);
        let depth = 1 + root
            .iter()
            .map(|(_, value)| depth(value))
            .max()
            .unwrap_or(0);
        if depth > MAX_DEPTH {
            return Err(ReflectError::new(Problem::Invalid(format!(
                "the document would nest {depth} levels deep, past the {MAX_DEPTH} that a reader reads"
            ))));
        }
        let text = text::print(&root);
        if text.len() > MAX_SIZE {
            return Err(ReflectError::new(Problem::Invalid(format!(
                "the document would be {} bytes, past the {MAX_SIZE} that a reader reads",
                text.len()
            ))));
        }
        Ok(text)
    }
}

/// How deep `value` nests, as a reader counts it: 1 for a value that holds no other, one more for
/// each table or array that holds it.
fn depth(value: &Value) -> usize {
    match value {
        Value::Struct(fields) => {
            1 + fields
                .iter()
                .map(|(_, field)| depth(field))
                .max()
                .unwrap_or(0)
        }
        Value::List(elements) => 1 + elements.iter().map(depth).max().unwrap_or(0),
        _ => 1,
    }
}

/// Reads a document of several values, one place at a time, each as the value of a registered type
/// that the reader names, at its type's version now: what [`Composer`] writes.
pub struct Document<'a> {
    registry: &'a Registry,
    text: &'a str,
    parsed: toml_edit::Document<&'a str>,
    root: Vec<(String, Value)>,
    /// The versions of the types that the document says, by the names it gives them.
    versions: HashMap<String, u32>,
    mode: Mode,
    warnings: Vec<ReadError>,
    /// The place of each key of each table that a lookup went through, by the keys of the table.
    index: RefCell<HashMap<Vec<String>, HashMap<String, usize>>>,
}

impl<'a> Document<'a> {
    /// Parses `text`, a document of values of the types of `registry`, read in `mode`: its format's
    /// version and its types' versions are checked here, its values as they are read.
    ///
    /// # Errors
    ///
    /// As [`read`](crate::read): a text too large, too deep or not TOML, a later version of the
    /// format or of a type, a type that the registry does not hold under `$versions` in
    /// [`Mode::Strict`].
    pub fn parse(registry: &'a Registry, text: &'a str, mode: Mode) -> Result<Self, ReadError> {
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
        let text::Parsed { document, root } = text::parse(text).map_err(|(span, problem)| {
            ReadError::new(
                span.map(|span| position(text, span.start)),
                Path::default(),
                problem,
            )
        })?;
        let mut read = Self {
            registry,
            text,
            parsed: document,
            root,
            versions: HashMap::new(),
            mode,
            warnings: Vec::new(),
            index: RefCell::default(),
        };
        read.format()?;
        read.versions()?;
        Ok(read)
    }

    /// Checks `$format`, and takes it out of the root.
    fn format(&mut self) -> Result<(), ReadError> {
        let Some(found) = take(&mut self.root, FORMAT) else {
            return Ok(());
        };
        let problem = match found {
            Value::Int(version) if (1..=i64::from(format::VERSION)).contains(&version) => {
                return Ok(());
            }
            Value::Int(version) if version > i64::from(format::VERSION) => {
                ReadProblem::NewerFormat {
                    found: version,
                    known: format::VERSION,
                }
            }
            _ => ReadProblem::Value(Problem::Invalid(
                "the format's version is an integer from 1".to_owned(),
            )),
        };
        Err(self.locate(Failure {
            path: Path::default(),
            at: Path::default().field(FORMAT),
            problem,
        }))
    }

    /// Reads `$versions`, and takes it out of the root.
    fn versions(&mut self) -> Result<(), ReadError> {
        let Some(table) = take(&mut self.root, VERSIONS) else {
            return Ok(());
        };
        let at = Path::default().field(VERSIONS);
        let Value::Struct(entries) = table else {
            return Err(self.locate(Failure {
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
                return Err(self.locate(fail(Problem::Invalid(format!(
                    "the version of `{name}` is not an integer"
                )))));
            };
            let Some(registration) = self.registry.get(&name) else {
                let unknown = self.locate(fail(Problem::NotRegistered(name)));
                match self.mode {
                    Mode::Strict => return Err(unknown),
                    Mode::Lenient => {
                        self.warnings.push(unknown);
                        continue;
                    }
                }
            };
            let known = registration.info().version;
            let version = match u32::try_from(version) {
                Ok(version) if version >= 1 => version,
                Err(_) if version > 0 => {
                    return Err(self.locate(fail(Problem::Invalid(format!(
                        "the version of `{name}` is {version}, past any version a type has"
                    )))));
                }
                _ => {
                    return Err(self.locate(fail(Problem::Invalid(format!(
                        "the version of `{name}` is {version}: versions start at 1"
                    )))));
                }
            };
            if version > known {
                return Err(self.locate(fail(Problem::NewerVersion {
                    found: version,
                    known,
                })));
            }
            self.versions.insert(name, version);
        }
        Ok(())
    }

    /// The keys of the table at `place`, in the document's order; none where it holds no table.
    pub fn keys(&self, place: &[&str]) -> Vec<String> {
        match self.at(place) {
            Some(Value::Struct(fields)) => fields.iter().map(|(name, _)| name.clone()).collect(),
            _ => Vec::new(),
        }
    }

    /// Reads the whole value of the type named `ty` at `place`, as [`read`](crate::read) reads it,
    /// the entities that it refers to named as `references` names them; `None` where the document
    /// holds nothing.
    ///
    /// # Errors
    ///
    /// As [`read`](crate::read), the path of the part at fault from the root.
    pub fn read(
        &mut self,
        place: &[&str],
        ty: &str,
        references: Option<&dyn References>,
    ) -> Result<Option<Value>, ReadError> {
        self.read_value(place, ty, references, false)
    }

    /// Reads a part of a value of the type named `ty` at `place`, as
    /// [`read_partial`](crate::read_partial) reads it; `None` where the document holds nothing.
    ///
    /// # Errors
    ///
    /// As [`Document::read`], but for missing fields.
    pub fn read_part(
        &mut self,
        place: &[&str],
        ty: &str,
        references: Option<&dyn References>,
    ) -> Result<Option<Value>, ReadError> {
        self.read_value(place, ty, references, true)
    }

    fn read_value(
        &mut self,
        place: &[&str],
        ty: &str,
        references: Option<&dyn References>,
        partial: bool,
    ) -> Result<Option<Value>, ReadError> {
        let Some(document) = self.at(place).cloned() else {
            return Ok(None);
        };
        let at = place.iter().fold(Path::default(), |at, key| at.field(*key));
        self.value_at(document, at.clone(), at, ty, references, partial)
            .map(Some)
    }

    /// Reads the value of the type named `ty` that the whole document holds, as [`read`](crate::read)
    /// does: its root table, or its `$value`.
    pub(crate) fn root(&mut self, ty: &str, partial: bool) -> Result<Value, ReadError> {
        let mut root = std::mem::take(&mut self.root);
        self.index.get_mut().clear();
        let (document, at) = match take(&mut root, VALUE) {
            Some(value) => {
                if let Some((key, _)) = root.first() {
                    return Err(self.locate(Failure {
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
        self.value_at(document, at, Path::default(), ty, None, partial)
    }

    /// The value of `ty` that `document`, at `at` in the document, holds, at `path` in what is read.
    fn value_at(
        &mut self,
        document: Value,
        at: Path,
        path: Path,
        ty: &str,
        references: Option<&dyn References>,
        partial: bool,
    ) -> Result<Value, ReadError> {
        let mut reader = Reader {
            registry: self.registry,
            references,
            versions: &self.versions,
            mode: self.mode,
            partial,
            warnings: Vec::new(),
        };
        let read = reader.value(ty, document, &path, &at, None);
        let warnings = reader.warnings;
        for warning in warnings {
            let warning = self.locate(warning);
            self.warnings.push(warning);
        }
        read.map_err(|failure| self.locate(failure))
    }

    /// What a lenient reading left out so far, taken out of the document.
    pub fn warnings(&mut self) -> Vec<ReadError> {
        std::mem::take(&mut self.warnings)
    }

    /// The document's value at `place`, if it holds one: a lookup per key, each table's keys found
    /// once, the first time a place goes through it.
    fn at(&self, place: &[&str]) -> Option<&Value> {
        let mut index = self.index.borrow_mut();
        let mut fields = &self.root;
        let mut table = Vec::with_capacity(place.len());
        let (last, parents) = place.split_last()?;
        let mut find = |table: &[String], fields: &[(String, Value)], key: &str| {
            let keys = index.entry(table.to_vec()).or_insert_with(|| {
                fields
                    .iter()
                    .enumerate()
                    .map(|(position, (name, _))| (name.clone(), position))
                    .collect()
            });
            keys.get(key).copied()
        };
        for key in parents {
            match find(&table, fields, key).map(|position| &fields[position].1) {
                Some(Value::Struct(inner)) => fields = inner,
                _ => return None,
            }
            table.push((*key).to_owned());
        }
        find(&table, fields, last).map(|position| &fields[position].1)
    }

    /// The error that `failure` makes, placed in the text: at the key of a field unknown or given
    /// twice, at the value otherwise.
    fn locate(&self, failure: Failure) -> ReadError {
        let at_key = matches!(
            failure.problem,
            ReadProblem::Value(Problem::UnknownField { .. } | Problem::DuplicateField(_))
        );
        let span = text::locate(&self.parsed, &failure.at, at_key);
        ReadError::new(
            span.map(|span| position(self.text, span.start)),
            failure.path,
            failure.problem,
        )
    }
}

/// Takes the root's entry under `key` out of it.
fn take(root: &mut Vec<(String, Value)>, key: &str) -> Option<Value> {
    let index = root.iter().position(|(name, _)| name == key)?;
    Some(root.remove(index).1)
}

/// The line and the column, from 1, of the byte `offset` in `text`, the column in characters.
pub(crate) fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = text.get(..offset).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let start = before.rfind('\n').map_or(0, |newline| newline + 1);
    (line, before[start..].chars().count() + 1)
}
