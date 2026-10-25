//! Values of reflected types turned into values as documents hold them, and back, as
//! [`crate::format`] says: the type's description, from the registry, leads both ways.

use std::collections::{BTreeMap, HashMap};

use slime_reflect::{
    FieldInfo, Kind, Number, Path, Problem, Range, ReflectError, Registry, TypeInfo, Value,
    is_tuple,
};

use crate::error::ReadProblem;
use crate::{Mode, References};

/// Turns values into values as documents hold them, and notes the versions of the types past
/// version 1 that they hold.
pub(crate) struct Writer<'a> {
    pub registry: &'a Registry,
    /// The names of the entities that values refer to, if the document names them.
    pub references: Option<&'a dyn References>,
    /// Whether fields marked transient are written: a save leaves them out.
    pub transient: bool,
    /// The versions of the types past version 1 met, by name, in the order of their names.
    pub versions: BTreeMap<String, u32>,
    /// Whether the value may leave fields out, a part of a value: the document leaves them out too.
    pub partial: bool,
    /// Whether a whole value's fields that hold their defaults are written: a scene file leaves
    /// them out, and a reader gives them their defaults again.
    pub defaults: bool,
}

impl Writer<'_> {
    /// `value`, a value of the type named `ty` at `path` in the value written, a number within
    /// `range`, as a document holds it: checked as a reader checks it, so that what is written can
    /// be read.
    pub(crate) fn value(
        &mut self,
        ty: &str,
        value: &Value,
        path: &Path,
        range: Option<Range>,
    ) -> Result<Value, ReflectError> {
        let info =
            info(self.registry, ty).map_err(|problem| ReflectError::at(path.clone(), problem))?;
        if info.version > 1 {
            self.versions.insert(info.name.clone(), info.version);
        }
        let error = |problem| ReflectError::at(path.clone(), problem);
        let within = |number: Number| match range {
            Some(range) if !range.holds(number) => Err(error(Problem::OutOfRange {
                value: number,
                range,
            })),
            _ => Ok(()),
        };
        // An integer of the type's width, as the registry checks it.
        let fits = || {
            self.registry
                .check(&info.name, value)
                .map_err(|refused| error(refused.problem().clone()))
        };
        Ok(match (&info.kind, value) {
            (Kind::Bool, Value::Bool(boolean)) => Value::Bool(*boolean),
            (Kind::Int { .. }, &Value::Int(number)) => {
                fits()?;
                within(Number::Int(number))?;
                Value::Int(number)
            }
            // Integers past TOML's, which holds 64 signed bits, are written as text.
            (Kind::UInt { .. }, &Value::UInt(number)) => {
                fits()?;
                within(Number::UInt(number))?;
                match i64::try_from(number) {
                    Ok(number) => Value::Int(number),
                    Err(_) => Value::Text(number.to_string()),
                }
            }
            (Kind::F32, &Value::F32(number)) => {
                within(Number::F32(number))?;
                Value::F32(number)
            }
            (Kind::F64, &Value::F64(number)) => {
                within(Number::F64(number))?;
                Value::F64(number)
            }
            (Kind::Text, Value::Text(text)) => Value::Text(text.clone()),
            (Kind::Asset, &Value::Asset(id)) => Value::Text(match self.references {
                Some(references) => references.asset_name(id).ok_or_else(|| {
                    error(Problem::Invalid(
                        "a reference to an asset that has no name in the document".to_owned(),
                    ))
                })?,
                None => uuid(id),
            }),
            (Kind::Entity, &Value::Entity(entity)) => {
                let references = self
                    .references
                    .ok_or_else(|| error(Problem::Invalid(ENTITY.to_owned())))?;
                Value::Text(references.name(entity).ok_or_else(|| {
                    error(Problem::Invalid(
                        "a reference to an entity that has no name in the document".to_owned(),
                    ))
                })?)
            }
            // An option that is not a field's: none or one element.
            (Kind::Option(inner), Value::Option(held)) => Value::List(match held {
                Some(held) => vec![self.value(inner, held, path, range)?],
                None => Vec::new(),
            }),
            (Kind::List { element, len }, Value::List(elements)) => {
                if let Some(len) = *len
                    && elements.len() != len
                {
                    return Err(error(Problem::WrongLength {
                        expected: len,
                        found: elements.len(),
                    }));
                }
                Value::List(
                    elements
                        .iter()
                        .enumerate()
                        .map(|(index, element_value)| {
                            let at = path.clone().index(index);
                            self.value(element, element_value, &at, range)
                        })
                        .collect::<Result<_, _>>()?,
                )
            }
            (Kind::Struct(fields), Value::Struct(values)) => {
                self.fields(info, fields, values, path)?
            }
            (Kind::Enum(variants), Value::Enum(name, values)) => {
                let variant = variants
                    .iter()
                    .find(|variant| &variant.name == name)
                    .ok_or_else(|| {
                        error(Problem::UnknownVariant {
                            variant: name.clone(),
                            known: variants
                                .iter()
                                .map(|variant| variant.name.clone())
                                .collect(),
                        })
                    })?;
                if variant.fields.is_empty() && values.is_empty() {
                    Value::Text(name.clone())
                } else {
                    let fields = self.fields(info, &variant.fields, values, path)?;
                    Value::Struct(vec![(name.clone(), fields)])
                }
            }
            _ => {
                return Err(error(Problem::WrongKind {
                    expected: info.name.clone(),
                    found: value.kind(),
                }));
            }
        })
    }

    /// The fields `values` of a struct or a variant whose fields `fields` describes: an array of
    /// them for a tuple, which a value holds whole, the value of the one there is for a tuple of one,
    /// and otherwise a table of them, an option that holds none left out. A whole value holds every
    /// field; a partial one those it gives.
    fn fields(
        &mut self,
        owner: &TypeInfo,
        fields: &[FieldInfo],
        values: &[(String, Value)],
        path: &Path,
    ) -> Result<Value, ReflectError> {
        if let Some((unknown, _)) = values
            .iter()
            .find(|(name, _)| !fields.iter().any(|field| &field.name == name))
        {
            return Err(ReflectError::at(
                path.clone(),
                Problem::UnknownField {
                    field: unknown.clone(),
                    known: fields.iter().map(|field| field.name.clone()).collect(),
                },
            ));
        }
        let given = |field: &FieldInfo| {
            values
                .iter()
                .find(|(name, _)| name == &field.name)
                .map(|(_, value)| value)
        };
        if is_tuple(fields) {
            let mut elements = Vec::new();
            for field in fields {
                let at = path.clone().field(field.name.clone());
                let value = given(field).ok_or_else(|| {
                    ReflectError::at(path.clone(), Problem::MissingField(field.name.clone()))
                })?;
                elements.push(self.value(&field.ty, value, &at, field.range)?);
            }
            return Ok(match <[Value; 1]>::try_from(elements) {
                Ok([only]) => only,
                Err(elements) => Value::List(elements),
            });
        }
        let mut table = Vec::new();
        for field in fields {
            if field.transient && !self.transient {
                continue;
            }
            let Some(value) = given(field) else {
                if self.partial {
                    continue;
                }
                return Err(ReflectError::at(
                    path.clone(),
                    Problem::MissingField(field.name.clone()),
                ));
            };
            if !self.defaults
                && !self.partial
                && self
                    .registry
                    .field_default(owner, field)
                    .is_some_and(|default| default.same(value))
            {
                continue;
            }
            let at = path.clone().field(field.name.clone());
            let kind = info(self.registry, &field.ty)
                .map_err(|problem| ReflectError::at(at.clone(), problem))?
                .kind
                .clone();
            // A whole value's option field is left out when it holds none, and is its value otherwise; a
            // part's is written as options elsewhere are, so that it can say that one was emptied.
            match (kind, value) {
                (Kind::Option(_), Value::Option(None)) if !self.partial => {}
                (Kind::Option(inner), Value::Option(Some(held))) if !self.partial => {
                    let held = self.value(&inner, held, &at, field.range)?;
                    table.push((field.name.clone(), held));
                }
                _ => {
                    let written = self.value(&field.ty, value, &at, field.range)?;
                    table.push((field.name.clone(), written));
                }
            }
        }
        Ok(Value::Struct(table))
    }
}

/// Why a reference to an entity is neither written nor read where the document names no entity.
const ENTITY: &str = "a reference to an entity, whose handle holds within one run only, is written \
                      and read only where a document names entities";

/// A problem found while reading, at `path` in the value and at `at` in the document.
pub(crate) struct Failure {
    pub path: Path,
    pub at: Path,
    pub problem: ReadProblem,
}

/// Turns values as documents hold them into values of reflected types, bringing those of earlier
/// versions of their types to the types' versions now.
pub(crate) struct Reader<'a> {
    pub registry: &'a Registry,
    /// The entities that the document names, if it names them.
    pub references: Option<&'a dyn References>,
    /// The versions of the types that the document says, by the names it gives them.
    pub versions: &'a HashMap<String, u32>,
    pub mode: Mode,
    /// Whether the value may leave fields out: a part of a value, as a save's differences are.
    pub partial: bool,
    /// What a lenient reading left out.
    pub warnings: Vec<Failure>,
}

impl Reader<'_> {
    /// The value of the type named `ty` that `document`, at `at` in the document, holds: at `path`
    /// in the value read, a number within `range`.
    pub(crate) fn value(
        &mut self,
        ty: &str,
        document: Value,
        path: &Path,
        at: &Path,
        range: Option<Range>,
    ) -> Result<Value, Failure> {
        let fail = |problem| Failure {
            path: path.clone(),
            at: at.clone(),
            problem: ReadProblem::Value(problem),
        };
        let info = info(self.registry, ty).map_err(fail)?;
        // A text in place of a value of another kind may stand for one, as the document's
        // references say.
        if let (Some(references), Value::Text(text)) = (self.references, &document)
            && !matches!(info.kind, Kind::Text)
            && let Some(given) = references.value(&info.name, text)
        {
            let value = given.map_err(|message| fail(Problem::Invalid(message)))?;
            self.registry
                .check(&info.name, &value)
                .map_err(|error| fail(error.problem().clone()))?;
            let number = match value {
                Value::Int(number) => Some(Number::Int(number)),
                Value::UInt(number) => Some(Number::UInt(number)),
                Value::F32(number) => Some(Number::F32(number)),
                Value::F64(number) => Some(Number::F64(number)),
                _ => None,
            };
            if let (Some(range), Some(number)) = (range, number)
                && !range.holds(number)
            {
                return Err(fail(Problem::OutOfRange {
                    value: number,
                    range,
                }));
            }
            return Ok(value);
        }
        let version = self.version(info);
        let document = if version == info.version {
            document
        } else {
            self.registry
                .upgrade(&info.name, version, document, self.partial)
                .map_err(|error| fail(error.problem().clone()))?
        };
        let wrong = |document: &Value| {
            fail(Problem::WrongKind {
                expected: info.name.clone(),
                found: kind(document),
            })
        };
        let within = |number: Number, value: Value| match range {
            Some(range) if !range.holds(number) => Err(fail(Problem::OutOfRange {
                value: number,
                range,
            })),
            _ => Ok(value),
        };
        Ok(match (&info.kind, document) {
            (Kind::Bool, Value::Bool(boolean)) => Value::Bool(boolean),
            (Kind::Int { .. }, Value::Int(number)) => {
                self.fits(info, Value::Int(number)).map_err(fail)?;
                within(Number::Int(number), Value::Int(number))?
            }
            (Kind::UInt { .. }, document @ (Value::Int(_) | Value::Text(_))) => {
                let number = match &document {
                    Value::Int(number) => u64::try_from(*number).ok(),
                    Value::Text(digits) if digits.bytes().all(|byte| byte.is_ascii_digit()) => {
                        digits.parse().ok()
                    }
                    _ => None,
                }
                .ok_or_else(|| wrong(&document))?;
                self.fits(info, Value::UInt(number)).map_err(fail)?;
                within(Number::UInt(number), Value::UInt(number))?
            }
            (Kind::F32, Value::F64(number)) => {
                // A NaN keeps its sign, which a cast does not promise.
                let narrow = match number {
                    number if number.is_nan() && number.is_sign_negative() => -f32::NAN,
                    number if number.is_nan() => f32::NAN,
                    number => number as f32,
                };
                if number.is_finite() && narrow.is_infinite() {
                    return Err(fail(Problem::OutOfRange {
                        value: Number::F64(number),
                        range: Range {
                            min: f64::from(f32::MIN),
                            min_excluded: false,
                            max: f64::from(f32::MAX),
                        },
                    }));
                }
                within(Number::F32(narrow), Value::F32(narrow))?
            }
            (Kind::F32, Value::Int(number)) => {
                within(Number::F32(number as f32), Value::F32(number as f32))?
            }
            (Kind::F64, Value::F64(number)) => within(Number::F64(number), Value::F64(number))?,
            (Kind::F64, Value::Int(number)) => {
                within(Number::F64(number as f64), Value::F64(number as f64))?
            }
            (Kind::Text, Value::Text(text)) => Value::Text(text),
            (Kind::Asset, Value::Text(text)) => Value::Asset(match self.references {
                Some(references) => references
                    .asset(&text)
                    .ok_or_else(|| fail(Problem::Invalid(format!("no asset is named `{text}`"))))?,
                None => parse_uuid(&text).ok_or_else(|| {
                    fail(Problem::Invalid(format!("`{text}` is not an asset ID")))
                })?,
            }),
            (Kind::Entity, document) => {
                let Some(references) = self.references else {
                    return Err(fail(Problem::Invalid(ENTITY.to_owned())));
                };
                let Value::Text(name) = &document else {
                    return Err(wrong(&document));
                };
                Value::Entity(references.entity(name).ok_or_else(|| {
                    fail(Problem::Invalid(format!("no entity is named `{name}`")))
                })?)
            }
            (Kind::Option(inner), Value::List(mut elements)) if elements.len() <= 1 => {
                Value::Option(match elements.pop() {
                    Some(held) => Some(Box::new(self.value(
                        inner,
                        held,
                        path,
                        &at.clone().index(0),
                        range,
                    )?)),
                    None => None,
                })
            }
            (Kind::List { element, len }, Value::List(elements)) => {
                if let Some(len) = *len
                    && elements.len() != len
                {
                    return Err(fail(Problem::WrongLength {
                        expected: len,
                        found: elements.len(),
                    }));
                }
                let mut read = Vec::with_capacity(elements.len());
                for (index, element_document) in elements.into_iter().enumerate() {
                    read.push(self.value(
                        element,
                        element_document,
                        &path.clone().index(index),
                        &at.clone().index(index),
                        range,
                    )?);
                }
                Value::List(read)
            }
            (Kind::Struct(fields), document) => {
                Value::Struct(self.fields(info, fields, document, path, at)?)
            }
            (Kind::Enum(variants), Value::Text(name)) => {
                let variant = variants
                    .iter()
                    .find(|variant| variant.name == name || variant.aliases.contains(&name))
                    .ok_or_else(|| {
                        fail(Problem::UnknownVariant {
                            variant: name.clone(),
                            known: variants
                                .iter()
                                .map(|variant| variant.name.clone())
                                .collect(),
                        })
                    })?;
                if !variant.fields.is_empty() {
                    return Err(fail(Problem::Invalid(format!(
                        "the variant `{name}` holds fields, which the document leaves out"
                    ))));
                }
                Value::Enum(variant.name.clone(), Vec::new())
            }
            (Kind::Enum(variants), Value::Struct(mut entries)) if entries.len() == 1 => {
                let (name, fields_document) = entries.pop().expect("one entry");
                let variant = variants
                    .iter()
                    .find(|variant| variant.name == name || variant.aliases.contains(&name))
                    .ok_or_else(|| {
                        fail(Problem::UnknownVariant {
                            variant: name.clone(),
                            known: variants
                                .iter()
                                .map(|variant| variant.name.clone())
                                .collect(),
                        })
                    })?;
                let fields = self.fields(
                    info,
                    &variant.fields,
                    fields_document,
                    path,
                    &at.clone().field(name),
                )?;
                Value::Enum(variant.name.clone(), fields)
            }
            (_, document) => return Err(wrong(&document)),
        })
    }

    /// The fields that `document` holds of a struct or a variant whose fields `fields` describes,
    /// of the type `owner`, in the order of `fields`.
    fn fields(
        &mut self,
        owner: &TypeInfo,
        fields: &[FieldInfo],
        document: Value,
        path: &Path,
        at: &Path,
    ) -> Result<Vec<(String, Value)>, Failure> {
        let fail = |at: &Path, problem| Failure {
            path: path.clone(),
            at: at.clone(),
            problem: ReadProblem::Value(problem),
        };
        let wrong = |document: &Value| {
            fail(
                at,
                Problem::WrongKind {
                    expected: owner.name.clone(),
                    found: kind(document),
                },
            )
        };
        if is_tuple(fields) {
            let documents = match document {
                document if fields.len() == 1 => vec![document],
                Value::List(elements) if elements.len() == fields.len() => elements,
                Value::List(elements) => {
                    return Err(fail(
                        at,
                        Problem::WrongLength {
                            expected: fields.len(),
                            found: elements.len(),
                        },
                    ));
                }
                document => return Err(wrong(&document)),
            };
            let mut read = Vec::with_capacity(fields.len());
            for (index, (field, element)) in fields.iter().zip(documents).enumerate() {
                // A tuple of one is its element; a longer one, an array of them.
                let element_at = if fields.len() == 1 {
                    at.clone()
                } else {
                    at.clone().index(index)
                };
                let field_path = path.clone().field(field.name.clone());
                let value =
                    self.value(&field.ty, element, &field_path, &element_at, field.range)?;
                read.push((field.name.clone(), value));
            }
            return Ok(read);
        }
        let Value::Struct(entries) = document else {
            return Err(wrong(&document));
        };
        let mut given: Vec<Option<(String, Value)>> = vec![None; fields.len()];
        for (key, value) in entries {
            let key_at = at.clone().field(key.clone());
            match fields
                .iter()
                .position(|field| field.name == key || field.aliases.contains(&key))
            {
                Some(index) if given[index].is_some() => {
                    return Err(fail(
                        &key_at,
                        Problem::DuplicateField(fields[index].name.clone()),
                    ));
                }
                Some(index) => given[index] = Some((key, value)),
                None => {
                    let unknown = fail(
                        &key_at,
                        Problem::UnknownField {
                            field: key,
                            known: fields.iter().map(|field| field.name.clone()).collect(),
                        },
                    );
                    match self.mode {
                        Mode::Strict => return Err(unknown),
                        Mode::Lenient => self.warnings.push(unknown),
                    }
                }
            }
        }
        let mut read = Vec::with_capacity(fields.len());
        for (field, given) in fields.iter().zip(given) {
            let field_path = path.clone().field(field.name.clone());
            match given {
                Some((key, document)) => {
                    let key_at = at.clone().field(key);
                    let kind = info(self.registry, &field.ty)
                        .map_err(|problem| fail(&key_at, problem))?
                        .kind
                        .clone();
                    // A whole value's option field holds the value the document gives; a part's is
                    // written as options elsewhere are.
                    let value = match kind {
                        Kind::Option(inner) if !self.partial => Value::Option(Some(Box::new(
                            self.value(&inner, document, &field_path, &key_at, field.range)?,
                        ))),
                        _ => self.value(&field.ty, document, &field_path, &key_at, field.range)?,
                    };
                    read.push((field.name.clone(), value));
                }
                None if self.partial => {}
                None => {
                    let default = self
                        .registry
                        .field_default(owner, field)
                        .ok_or_else(|| fail(at, Problem::MissingField(field.name.clone())))?;
                    read.push((field.name.clone(), default));
                }
            }
        }
        Ok(read)
    }

    /// Refuses `number`, an integer of the type `info`, if it does not fit the type's width.
    fn fits(&self, info: &TypeInfo, number: Value) -> Result<(), Problem> {
        self.registry
            .check(&info.name, &number)
            .map_err(|error| error.problem().clone())
    }

    /// The version of the type `info` that the document holds: the one it says, under the type's
    /// name or one of its aliases, or 1.
    fn version(&self, info: &TypeInfo) -> u32 {
        std::iter::once(&info.name)
            .chain(&info.aliases)
            .find_map(|name| self.versions.get(name).copied())
            .unwrap_or(1)
    }
}

/// The description of the type named `ty`.
fn info<'a>(registry: &'a Registry, ty: &str) -> Result<&'a TypeInfo, Problem> {
    registry
        .get(ty)
        .map(|registration| registration.info())
        .ok_or_else(|| Problem::NotRegistered(ty.to_owned()))
}

/// What a value as a document holds it is, in TOML's words.
fn kind(document: &Value) -> &'static str {
    match document {
        Value::Struct(_) => "a table",
        Value::List(_) => "an array",
        Value::Text(_) => "text",
        Value::Int(_) => "an integer",
        Value::F32(_) | Value::F64(_) => "a float",
        Value::Bool(_) => "a boolean",
        other => other.kind(),
    }
}

/// An asset ID as text: `850ad551-5098-42f9-ae42-26ed6174951a`.
pub(crate) fn uuid(id: u128) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        id >> 96,
        (id >> 80) & 0xffff,
        (id >> 64) & 0xffff,
        (id >> 48) & 0xffff,
        id & 0xffff_ffff_ffff
    )
}

/// The asset ID that `text` writes as [`uuid`] does, in either case.
pub(crate) fn parse_uuid(text: &str) -> Option<u128> {
    let groups: Vec<&str> = text.split('-').collect();
    let lengths: Vec<usize> = groups.iter().map(|group| group.len()).collect();
    if lengths != [8, 4, 4, 4, 12]
        || !groups
            .iter()
            .all(|group| group.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return None;
    }
    u128::from_str_radix(&groups.concat(), 16).ok()
}
