//! A family's messages described in lines, and held to its version
//! (`docs/adr/0308-m6d9a2c-cut-in-four-each-familys-messages-described-in-lines-read-from-its-types-a-kept-copy-for-each-version.md`).
//!
//! It serves the families' tests, in this crate and in a carrier's: it reads and writes files only
//! when a test calls [`check`].
//!
//! A family's test reads the JSON Schema of its messages' types, as `schemars` writes it, and
//! [`Description`] turns it into one line for each message, each value and each field, sorted,
//! prose dropped:
//!
//! ```text
//! request interface/elements
//! request interface/elements . object
//! request interface/elements cursor string|null optional
//! reply elements elements[].path string
//! ```
//!
//! A line names the message's kind, `request`, `reply` or `event`, and its name; then the path of a
//! value from the message's content, `.` for the content itself, `[]` for an array's items, `{}`
//! for a map's values, `<label>` for a case of a *choice*, which an enumeration is, labelled by its
//! tag, `<kind="press">`, its constant, `<="a">`, the definition that it names, `<Name>`, or its
//! type, `<null>`; its type, as JSON Schema names it, `choice` for a choice and `any` for a value of
//! any type; its constraints, sorted, as `format="uint32"`; and `optional` for a field that a
//! message may leave out. A value of a type that holds itself, below the type, is a line
//! `ref Name`. A line that a test writes by hand, where no type describes the message, ends with
//! `(by hand)`.
//!
//! The walk fails on a keyword of JSON Schema that it does not know, a reference that names no
//! definition, two cases of a choice with one label, and a field or a label that holds a space,
//! `<` or `>`, so that no part of a message escapes the lines. What `schemars` does not see escapes them: a field read or written by a function of
//! its own (`serde`'s `with`, `deserialize_with`, `serialize_with`) is described as its type says.
//!
//! [`check`] holds a family to its version. Its *description*, `<family>.txt`, holds its lines now;
//! its *kept copy* for each version, `<family>.<version>.txt`, the lines of that version when it was
//! raised, never edited after. Unless the version is raised, these fail:
//!
//! - a line of the kept copy that the messages no longer give: a value removed or changed;
//! - a line that a request of the kept copy gains, but within a new case of a choice named by its
//!   tag or its constant: a program of before would ignore a field that it does not know, and could
//!   take a case named by its type for another;
//! - a new case of a choice in a reply or an event: a tool of before could not read it.
//!
//! A message added, a field added to a reply or an event, a new case named by its tag or its
//! constant in a request, which a program of before refuses as it refuses a request that it does
//! not know, and a comment reworded, pass.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Map, Value};

/// The environment variable that the families' tests read to have [`check`] write a family's
/// description, and the kept copy of a version that has none, when it holds `write`:
/// `UWUI_DESCRIBE=write cargo test`.
pub const WRITE_VARIABLE: &str = "UWUI_DESCRIBE";

/// Why a family's messages cannot be described, or fail their version.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DescriptionError {
    /// A schema that the walk does not read: a keyword that it does not know, a reference that
    /// names no definition, two cases of a choice with one label, a field or a label that holds a
    /// space, `<` or `>`, a field named both beside a definition and in it, or a message that is
    /// not shaped as a family's messages are.
    #[error("the schema at `{at}` does not read: {why}")]
    Schema {
        /// Where, as a line names it.
        at: String,
        /// Why.
        why: String,
    },
    /// The family's description does not show its messages as they are.
    #[error(
        "the description {file} does not show the family's messages as they are: write it again \
         with {WRITE_VARIABLE}=write, and check its changes against its version"
    )]
    Unwritten {
        /// The description's file.
        file: String,
    },
    /// No kept copy of the family's version.
    #[error(
        "no kept copy {file} of the family's version: write it with {WRITE_VARIABLE}=write once \
         the version is raised"
    )]
    NotKept {
        /// The kept copy's file.
        file: String,
    },
    /// Lines that change what the version held, without the version raised.
    #[error(
        "the messages change what version {version} of `{family}` held, without raising it: \
         raise the family's version, and write its kept copy with {WRITE_VARIABLE}=write\n{}",
        .lines.join("\n")
    )]
    Unraised {
        /// The family.
        family: String,
        /// Its version.
        version: u32,
        /// Each line removed or changed, `- `, and each line gained that the version cannot
        /// take, `+ `.
        lines: Vec<String>,
    },
    /// A file that cannot be read or written.
    #[error("{file}: {source}")]
    File {
        /// The file.
        file: String,
        /// Why.
        source: io::Error,
    },
}

/// What a message is, as its line names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// What a tool sends: a request, which a program answers, or the core's cancel, which it
    /// does not. A field that it gains changes its version: a program of before would ignore it.
    Request,
    /// A program's answer to a request.
    Reply,
    /// An event, which a program tells.
    Event,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Reply => "reply",
            Self::Event => "event",
        }
    }
}

/// The lines of a family's messages, gathered message by message.
#[derive(Debug, Default)]
pub struct Description {
    lines: BTreeSet<String>,
    /// Whether a line was written by hand.
    by_hand: bool,
}

impl Description {
    /// No line yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the message `name` of `kind`, whose content's schema is `schema`, from the root
    /// schema whose definitions it names.
    ///
    /// # Errors
    ///
    /// [`DescriptionError::Schema`] for a schema that the walk does not read.
    pub fn message(
        &mut self,
        kind: Kind,
        name: &str,
        schema: &Value,
    ) -> Result<(), DescriptionError> {
        let defs = definitions(schema);
        let prefix = format!("{} {name}", kind.name());
        self.lines.insert(prefix.clone());
        let mut walk = Walk {
            defs: &defs,
            prefix: &prefix,
            lines: &mut self.lines,
            stack: Vec::new(),
        };
        walk.value(schema, ".", false)
    }

    /// Adds each case of the enumeration whose schema is `schema`, a message of `kind` named by
    /// its member `tag`, its content in its member `content` (`serde`'s `tag` and `content`) or
    /// beside its tag when none, for the cases whose names `wanted` takes.
    ///
    /// # Errors
    ///
    /// [`DescriptionError::Schema`] for a schema that the walk does not read, as a case that names
    /// no `tag`.
    pub fn tagged(
        &mut self,
        kind: Kind,
        schema: &Value,
        tag: &str,
        content: Option<&str>,
        wanted: impl Fn(&str) -> bool,
    ) -> Result<(), DescriptionError> {
        let defs = definitions(schema);
        let cases = schema
            .get("oneOf")
            .and_then(Value::as_array)
            .ok_or_else(|| shape(kind.name(), "not an enumeration of tagged cases"))?;
        for case in cases {
            let properties = case.get("properties").and_then(Value::as_object);
            let name = properties
                .and_then(|properties| properties.get(tag))
                .and_then(|tag| tag.get("const"))
                .and_then(Value::as_str)
                .ok_or_else(|| shape(kind.name(), &format!("a case that names no `{tag}`")))?;
            if !wanted(name) {
                continue;
            }
            let prefix = format!("{} {name}", kind.name());
            self.lines.insert(prefix.clone());
            let mut walk = Walk {
                defs: &defs,
                prefix: &prefix,
                lines: &mut self.lines,
                stack: Vec::new(),
            };
            match content {
                // Its content alone: the case's other members are its tag and its content.
                Some(content) => {
                    if let Some(inner) = properties.and_then(|properties| properties.get(content)) {
                        walk.value(inner, ".", false)?;
                    }
                }
                // Its members beside the tag.
                None => {
                    let mut case = case.clone();
                    if let Some(properties) =
                        case.get_mut("properties").and_then(Value::as_object_mut)
                    {
                        properties.remove(tag);
                    }
                    if let Some(required) = case.get_mut("required").and_then(Value::as_array_mut) {
                        required.retain(|name| name != tag);
                    }
                    walk.value(&case, ".", false)?;
                }
            }
        }
        Ok(())
    }

    /// Adds a line written by hand, for what no type describes, as the core's envelope:
    /// `line`, then `(by hand)`.
    pub fn line(&mut self, line: &str) {
        self.lines.insert(format!("{line} (by hand)"));
        self.by_hand = true;
    }

    /// The lines, sorted.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }
}

/// Holds the family `family`, at `version`, described by `description`, to the files of
/// `directory`: its description `<family>.txt` and its kept copy `<family>.<version>.txt`. With
/// `write`, writes the description first, and the kept copy of a version that has none; a kept
/// copy that exists is never written.
///
/// # Errors
///
/// [`DescriptionError::Unwritten`] for a description that does not show the messages as they are;
/// [`DescriptionError::NotKept`] for no kept copy of `version`; [`DescriptionError::Unraised`] for
/// lines that change what the version held; [`DescriptionError::File`] for a file that cannot be
/// read or written.
pub fn check(
    directory: &Path,
    family: &str,
    version: u32,
    description: &Description,
    write: bool,
) -> Result<(), DescriptionError> {
    let text = description.text(family, version);
    let file = directory.join(format!("{family}.txt"));
    let kept_file = directory.join(format!("{family}.{version}.txt"));
    if write {
        written(&file, &text)?;
        if read(&kept_file)?.is_none() {
            written(&kept_file, &text)?;
        }
    }
    if read(&file)? != Some(text) {
        return Err(DescriptionError::Unwritten {
            file: file.display().to_string(),
        });
    }
    let kept = read(&kept_file)?.ok_or_else(|| DescriptionError::NotKept {
        file: kept_file.display().to_string(),
    })?;
    let lines = unraised(&kept, description);
    if lines.is_empty() {
        Ok(())
    } else {
        Err(DescriptionError::Unraised {
            family: family.to_owned(),
            version,
            lines,
        })
    }
}

impl Description {
    /// The text of the description of `family` at `version`: a header, then the lines.
    fn text(&self, family: &str, version: u32) -> String {
        let mut text = format!(
            "# The messages of the family `{family}`, version {version}: one line for each message, \
             value and field,\n# read from their types (`uwui_agent::description`). Written by the \
             family's test with {WRITE_VARIABLE}=write.\n"
        );
        if self.by_hand {
            text.push_str(
                "# A line that ends with `(by hand)` is written by the test, where no type \
                 describes it.\n",
            );
        }
        for line in self.lines() {
            // Writing to a `String` does not fail.
            let _ = writeln!(text, "{line}");
        }
        text
    }
}

/// A line split into its message, its kind and name, and its path; none for a message's first.
fn parts(line: &str) -> (&str, Option<&str>) {
    let mut words = line.match_indices(' ');
    let message = match words.nth(1) {
        Some((end, _)) => &line[..end],
        None => return (line, None),
    };
    let path = line[message.len() + 1..].split(' ').next();
    (message, path)
}

/// The paths, from the outermost, of the cases of a choice that `path` stands in: `a<x>` and
/// `a<x>.b<y>` for `a<x>.b<y>.c`.
fn cases(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('>').map(move |(end, _)| &path[..=end])
}

/// The path of the choice of the case at `case`: `a` for `a<x>`.
fn choice(case: &str) -> &str {
    case.rfind('<').map_or(case, |start| &case[..start])
}

/// The lines of `kept` that `now` no longer gives, `- line`, and the lines that `now` gains and
/// the version cannot take, `+ line`: as the module says.
fn unraised(kept: &str, now: &Description) -> Vec<String> {
    let kept: BTreeSet<&str> = kept
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect();
    let kept_paths: BTreeSet<(&str, &str)> = kept
        .iter()
        .filter_map(|line| match parts(line) {
            (message, Some(path)) => Some((message, path)),
            (_, None) => None,
        })
        .collect();
    let kept_messages: BTreeSet<&str> = kept.iter().map(|line| parts(line).0).collect();
    let mut lines: Vec<String> = kept
        .iter()
        .filter(|line| !now.lines.contains(**line))
        .map(|line| format!("- {line}"))
        .collect();
    for line in now.lines().filter(|line| !kept.contains(line)) {
        let (message, path) = parts(line);
        let Some(path) = path.filter(|_| kept_messages.contains(message)) else {
            // A message added.
            continue;
        };
        // A new case of a choice that the kept copy holds.
        let new_case = cases(path).find(|case| {
            !kept_paths.contains(&(message, case)) && kept_paths.contains(&(message, choice(case)))
        });
        let gained = if line.starts_with("request ") {
            // A new case passes when it is named, by its tag or its constant: a program of before
            // refuses it then, where it could take a case named by its type for another.
            !new_case.is_some_and(|case| case[choice(case).len()..].contains('='))
        } else {
            new_case.is_some()
        };
        if gained {
            lines.push(format!("+ {line}"));
        }
    }
    lines
}

/// The text of `file`, or none where it does not exist.
fn read(file: &Path) -> Result<Option<String>, DescriptionError> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(DescriptionError::File {
            file: file.display().to_string(),
            source,
        }),
    }
}

/// Writes `text` into `file`.
fn written(file: &Path, text: &str) -> Result<(), DescriptionError> {
    fs::write(file, text).map_err(|source| DescriptionError::File {
        file: file.display().to_string(),
        source,
    })
}

/// The definitions that a root schema's references name, `#/$defs/Name`.
fn definitions(schema: &Value) -> Map<String, Value> {
    schema
        .get("$defs")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn shape(at: &str, why: &str) -> DescriptionError {
    DescriptionError::Schema {
        at: at.to_owned(),
        why: why.to_owned(),
    }
}

/// The keywords that only say what a schema means to a person, or where it starts.
const PROSE: [&str; 5] = ["$schema", "title", "description", "examples", "$defs"];

/// The keywords that constrain a value, written into its line as `keyword=value`.
const CONSTRAINTS: [&str; 12] = [
    "const",
    "default",
    "format",
    "maxItems",
    "maxLength",
    "maximum",
    "minItems",
    "minLength",
    "minimum",
    "exclusiveMaximum",
    "exclusiveMinimum",
    "uniqueItems",
];

/// The keywords that the walk follows into the parts of a value: `enum` gives the cases of a
/// choice of constants, and `additionalProperties`, when it is not a constraint, the type of a
/// map's values.
const PARTS: [&str; 9] = [
    "additionalProperties",
    "enum",
    "type",
    "properties",
    "required",
    "items",
    "oneOf",
    "anyOf",
    "$ref",
];

/// A walk of one message's schema, which adds its lines.
struct Walk<'a> {
    defs: &'a Map<String, Value>,
    prefix: &'a str,
    lines: &'a mut BTreeSet<String>,
    /// The definitions being walked, so that a type that holds itself ends its walk.
    stack: Vec<String>,
}

impl Walk<'_> {
    /// The value at `path`, `.` for a message's content: its line, then its parts.
    fn value(
        &mut self,
        schema: &Value,
        path: &str,
        optional: bool,
    ) -> Result<(), DescriptionError> {
        let name = self.reference(schema, path)?.map(str::to_owned);
        if let Some(name) = &name {
            if self.stack.contains(name) {
                self.lines
                    .insert(format!("{} {path} ref {name}", self.prefix));
                return Ok(());
            }
            self.stack.push(name.clone());
        }
        let resolved = self.resolved(schema, path)?;
        // A choice of constants, as `schemars` writes an enumeration of unit cases without their
        // comments: its cases, as it writes them with their comments.
        let constants: Option<Vec<Value>> =
            resolved
                .get("enum")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .map(
                            |value| serde_json::json!({ "type": json_type(value), "const": value }),
                        )
                        .collect()
                });
        let kind = if constants.is_some() {
            "choice".to_owned()
        } else {
            type_of(&resolved)
        };
        let mut constraints = Vec::new();
        for (keyword, value) in resolved.as_object().into_iter().flatten() {
            let constraint = CONSTRAINTS.contains(&keyword.as_str())
                || (keyword == "additionalProperties" && value.is_boolean());
            if constraint {
                constraints.push(format!("{keyword}={value}"));
            } else if !PROSE.contains(&keyword.as_str()) && !PARTS.contains(&keyword.as_str()) {
                return Err(self.unread(
                    path,
                    format!("the keyword `{keyword}` is not one that the walk knows"),
                ));
            }
        }
        // Sorted, whatever the order of the schema's members.
        constraints.sort();
        let mut line = format!("{} {path} {kind}", self.prefix);
        for constraint in constraints {
            let _ = write!(line, " {constraint}");
        }
        if optional {
            line.push_str(" optional");
        }
        self.lines.insert(line);
        self.fields(&resolved, path)?;
        if let Some(items) = resolved.get("items") {
            self.value(items, &format!("{path}[]"), false)?;
        }
        if let Some(map) = resolved
            .get("additionalProperties")
            .filter(|map| map.is_object())
        {
            self.value(map, &format!("{path}{{}}"), false)?;
        }
        let mut choices: Vec<&[Value]> = ["oneOf", "anyOf"]
            .iter()
            .filter_map(|choice| resolved.get(*choice).and_then(Value::as_array))
            .map(Vec::as_slice)
            .collect();
        if let Some(constants) = &constants {
            choices.push(constants);
        }
        for cases in choices {
            let mut labels = BTreeSet::new();
            for case in cases {
                let label = self.label(case, path)?;
                if !plain(&label) {
                    return Err(self.unread(
                        path,
                        format!("the case `{label}` holds a space, `<` or `>`"),
                    ));
                }
                if !labels.insert(label.clone()) {
                    return Err(self.unread(path, format!("two cases labelled `{label}`")));
                }
                self.value(case, &format!("{path}<{label}>"), false)?;
            }
        }
        if name.is_some() {
            self.stack.pop();
        }
        Ok(())
    }

    /// The fields of an object, each at `path` and its name.
    fn fields(&mut self, schema: &Value, path: &str) -> Result<(), DescriptionError> {
        let required: BTreeSet<&str> = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, field) in properties {
                if !plain(name) {
                    return Err(self.unread(
                        path,
                        format!("the field `{name}` holds a space, `<` or `>`"),
                    ));
                }
                let at = if path == "." {
                    name.clone()
                } else {
                    format!("{path}.{name}")
                };
                self.value(field, &at, !required.contains(name.as_str()))?;
            }
        }
        Ok(())
    }

    /// The definition that `schema` names, `Name` for `#/$defs/Name`; none where it names none.
    fn reference<'s>(
        &self,
        schema: &'s Value,
        path: &str,
    ) -> Result<Option<&'s str>, DescriptionError> {
        let Some(reference) = schema.get("$ref") else {
            return Ok(None);
        };
        reference
            .as_str()
            .and_then(|reference| reference.strip_prefix("#/$defs/"))
            .filter(|name| self.defs.contains_key(*name))
            .map(Some)
            .ok_or_else(|| {
                self.unread(
                    path,
                    format!(
                        "the reference {reference} names no definition: a message's type that \
                         holds itself is not walked"
                    ),
                )
            })
    }

    /// The schema that `schema` names through `$ref`, its own members beside the reference kept
    /// above the definition's; `schema` itself when it names none.
    fn resolved(&self, schema: &Value, path: &str) -> Result<Value, DescriptionError> {
        let Some(name) = self.reference(schema, path)? else {
            return Ok(schema.clone());
        };
        let mut resolved = self.defs[name].clone();
        if let (Some(resolved), Some(own)) = (resolved.as_object_mut(), schema.as_object()) {
            for (keyword, value) in own {
                match (keyword.as_str(), resolved.get_mut(keyword)) {
                    ("$ref", _) => {}
                    // A case's own members beside a definition's, as `schemars` writes a tagged
                    // case that holds a type: its tag beside the type's fields.
                    ("properties", Some(Value::Object(theirs))) => {
                        for (field, schema) in value.as_object().into_iter().flatten() {
                            if theirs.insert(field.clone(), schema.clone()).is_some() {
                                return Err(self.unread(
                                    path,
                                    format!(
                                        "the field `{field}` named both beside and in `{name}`"
                                    ),
                                ));
                            }
                        }
                    }
                    ("required", Some(Value::Array(theirs))) => {
                        theirs.extend(value.as_array().into_iter().flatten().cloned());
                    }
                    _ => {
                        resolved.insert(keyword.clone(), value.clone());
                    }
                }
            }
        }
        Ok(resolved)
    }

    /// The label of a case of a choice: the member that tags it and its value, `kind="press"`;
    /// the definition that it names; its constant, `="a"`; or its type.
    fn label(&self, case: &Value, path: &str) -> Result<String, DescriptionError> {
        let resolved = self.resolved(case, path)?;
        if let Some(properties) = resolved.get("properties").and_then(Value::as_object) {
            for (name, property) in properties {
                if let Some(value) = property.get("const") {
                    return Ok(format!("{name}={value}"));
                }
            }
        }
        if let Some(name) = self.reference(case, path)? {
            return Ok(name.to_owned());
        }
        if let Some(value) = resolved.get("const") {
            return Ok(format!("={value}"));
        }
        Ok(type_of(&resolved))
    }

    fn unread(&self, path: &str, why: String) -> DescriptionError {
        DescriptionError::Schema {
            at: format!("{} {path}", self.prefix),
            why,
        }
    }
}

/// Whether a field's name or a case's label can stand in a line's path: no space, which ends the
/// path, and no `<` or `>`, which mark a case.
fn plain(name: &str) -> bool {
    !name.contains([' ', '<', '>'])
}

/// A schema's type as JSON Schema names it, several joined by `|` in their order; `choice` for a
/// choice that names none, and `any` for none.
fn type_of(schema: &Value) -> String {
    match schema.get("type") {
        Some(Value::String(name)) => name.clone(),
        Some(Value::Array(names)) => names
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("|"),
        _ if schema.get("oneOf").is_some() || schema.get("anyOf").is_some() => "choice".to_owned(),
        _ => "any".to_owned(),
    }
}

/// The type of JSON that a constant has, as JSON Schema names it.
fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
#[expect(
    dead_code,
    reason = "these tests' messages are read for their schemas, never made"
)]
mod tests {
    use std::collections::BTreeMap;

    use schemars::{JsonSchema, schema_for};

    use super::*;

    /// A family of one request and one reply, as a version kept them.
    mod kept {
        use super::*;

        #[derive(JsonSchema)]
        #[serde(tag = "method", content = "params")]
        pub(super) enum Request {
            /// Asks.
            #[serde(rename = "f/ask")]
            Ask {
                /// How much.
                much: u32,
                /// What to do.
                action: Action,
            },
        }

        #[derive(JsonSchema)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub(super) enum Action {
            Press,
            Scroll { by: f32 },
        }

        #[derive(JsonSchema)]
        #[serde(tag = "reply", rename_all = "snake_case")]
        pub(super) enum Reply {
            /// Answers.
            Answer {
                /// What.
                what: String,
                /// Its shape.
                shape: Shape,
            },
        }

        #[derive(JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub(super) enum Shape {
            Round,
            Square,
        }
    }

    fn schema<T: JsonSchema>() -> Value {
        schema_for!(T).to_value()
    }

    fn described<Request: JsonSchema, Reply: JsonSchema>() -> Description {
        let mut description = Description::new();
        description
            .tagged(
                Kind::Request,
                &schema::<Request>(),
                "method",
                Some("params"),
                |_| true,
            )
            .expect("described");
        description
            .tagged(Kind::Reply, &schema::<Reply>(), "reply", None, |_| true)
            .expect("described");
        description
    }

    fn kept() -> String {
        described::<kept::Request, kept::Reply>()
            .lines()
            .map(|line| format!("{line}\n"))
            .collect()
    }

    fn lines(description: &Description) -> Vec<&str> {
        description.lines().collect()
    }

    /// The lines of one message of a request, `f/m`, whose content's schema is `schema`.
    fn message(schema: &Value) -> Result<Vec<String>, DescriptionError> {
        let mut description = Description::new();
        description.message(Kind::Request, "f/m", schema)?;
        Ok(description.lines().map(str::to_owned).collect())
    }

    #[test]
    fn a_family_is_described_by_its_messages_values_and_fields() {
        assert_eq!(
            lines(&described::<kept::Request, kept::Reply>()),
            [
                "reply answer",
                "reply answer . object",
                "reply answer shape choice",
                "reply answer shape<=\"round\"> string const=\"round\"",
                "reply answer shape<=\"square\"> string const=\"square\"",
                "reply answer what string",
                "request f/ask",
                "request f/ask . object",
                "request f/ask action choice",
                "request f/ask action<kind=\"press\"> object",
                "request f/ask action<kind=\"press\">.kind string const=\"press\"",
                "request f/ask action<kind=\"scroll\"> object",
                "request f/ask action<kind=\"scroll\">.by number format=\"float\"",
                "request f/ask action<kind=\"scroll\">.kind string const=\"scroll\"",
                "request f/ask much integer format=\"uint32\" minimum=0",
            ]
        );
    }

    #[test]
    fn a_field_that_a_request_gains_changes_its_version() {
        #[derive(JsonSchema)]
        #[serde(tag = "method", content = "params")]
        enum Request {
            #[serde(rename = "f/ask")]
            Ask {
                much: u32,
                action: kept::Action,
                #[serde(default)]
                more: Option<u32>,
            },
        }
        assert_eq!(
            unraised(&kept(), &described::<Request, kept::Reply>()),
            ["+ request f/ask more integer|null default=null format=\"uint32\" minimum=0 optional"]
        );
    }

    #[test]
    fn a_value_removed_or_changed_changes_its_version() {
        #[derive(JsonSchema)]
        #[serde(tag = "reply", rename_all = "snake_case")]
        enum Reply {
            Answer {
                what: Option<String>,
                shape: kept::Shape,
            },
        }
        assert_eq!(
            unraised(&kept(), &described::<kept::Request, Reply>()),
            ["- reply answer what string"]
        );
        #[derive(JsonSchema)]
        #[serde(tag = "method", content = "params")]
        enum Request {
            #[serde(rename = "f/ask")]
            Ask { action: kept::Action },
        }
        assert_eq!(
            unraised(&kept(), &described::<Request, kept::Reply>()),
            ["- request f/ask much integer format=\"uint32\" minimum=0"]
        );
    }

    #[test]
    fn a_message_removed_changes_its_version() {
        #[derive(JsonSchema)]
        #[serde(tag = "reply", rename_all = "snake_case")]
        enum Reply {
            Other,
        }
        let unraised = unraised(&kept(), &described::<kept::Request, Reply>());
        assert!(
            unraised.contains(&"- reply answer".to_owned()),
            "{unraised:?}"
        );
    }

    #[test]
    fn a_new_case_passes_in_a_request_and_fails_in_a_reply() {
        #[derive(JsonSchema)]
        #[serde(tag = "method", content = "params")]
        enum Request {
            #[serde(rename = "f/ask")]
            Ask { much: u32, action: Action },
        }
        #[derive(JsonSchema)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Action {
            Press,
            Scroll { by: f32 },
            Zoom { by: f32 },
        }
        assert!(unraised(&kept(), &described::<Request, kept::Reply>()).is_empty());
        #[derive(JsonSchema)]
        #[serde(tag = "reply", rename_all = "snake_case")]
        enum Reply {
            Answer { what: String, shape: Shape },
        }
        /// Shapes, with comments: `schemars` writes them otherwise, and their lines are the same.
        #[derive(JsonSchema)]
        #[serde(rename_all = "snake_case")]
        enum Shape {
            /// Round.
            Round,
            /// Square.
            Square,
            /// Three sides.
            Triangle,
        }
        assert_eq!(
            unraised(&kept(), &described::<kept::Request, Reply>()),
            ["+ reply answer shape<=\"triangle\"> string const=\"triangle\""]
        );
    }

    #[test]
    fn a_field_added_to_a_reply_a_request_added_or_a_comment_reworded_keep_its_version() {
        #[derive(JsonSchema)]
        #[serde(tag = "method", content = "params")]
        enum Request {
            /// Asks for some of it, worded again.
            #[serde(rename = "f/ask")]
            Ask { much: u32, action: kept::Action },
            #[serde(rename = "f/tell")]
            Tell { what: String },
        }
        #[derive(JsonSchema)]
        #[serde(tag = "reply", rename_all = "snake_case")]
        enum Reply {
            Answer {
                what: String,
                shape: kept::Shape,
                #[serde(default)]
                why: Option<Why>,
            },
        }
        #[derive(JsonSchema)]
        struct Why {
            because: String,
        }
        assert!(unraised(&kept(), &described::<Request, Reply>()).is_empty());
    }

    #[test]
    fn a_change_at_a_message_s_root_changes_its_version() {
        #[derive(JsonSchema)]
        #[serde(tag = "reply", rename_all = "snake_case", deny_unknown_fields)]
        enum Reply {
            Answer { what: String, shape: kept::Shape },
        }
        assert_eq!(
            unraised(&kept(), &described::<kept::Request, Reply>()),
            ["- reply answer . object"]
        );
    }

    #[test]
    fn maps_arrays_choices_and_constants_are_walked() {
        #[derive(JsonSchema)]
        struct Content {
            counts: BTreeMap<String, u32>,
            open: Option<bool>,
            loose: Loose,
        }
        #[derive(JsonSchema)]
        #[serde(deny_unknown_fields)]
        struct Loose {
            names: Vec<String>,
        }
        assert_eq!(
            message(&schema::<Content>()).expect("described"),
            [
                "request f/m",
                "request f/m . object",
                "request f/m counts object",
                "request f/m counts{} integer format=\"uint32\" minimum=0",
                "request f/m loose object additionalProperties=false",
                "request f/m loose.names array",
                "request f/m loose.names[] string",
                "request f/m open boolean|null optional",
            ]
        );
        // A content that is no object, and the cases of a choice that name definitions.
        #[derive(JsonSchema)]
        #[serde(untagged)]
        enum Either {
            One(One),
            Two(Two),
        }
        #[derive(JsonSchema)]
        struct One {
            a: u8,
        }
        #[derive(JsonSchema)]
        struct Two {
            b: u8,
        }
        assert_eq!(
            message(&schema::<Either>()).expect("described"),
            [
                "request f/m",
                "request f/m . choice",
                "request f/m .<One> object",
                "request f/m .<One>.a integer format=\"uint8\" maximum=255 minimum=0",
                "request f/m .<Two> object",
                "request f/m .<Two>.b integer format=\"uint8\" maximum=255 minimum=0",
            ]
        );
    }

    #[test]
    fn a_type_that_holds_itself_ends_its_walk() {
        #[derive(JsonSchema)]
        struct Content {
            tree: Tree,
        }
        #[derive(JsonSchema)]
        struct Tree {
            children: Vec<Tree>,
        }
        assert_eq!(
            message(&schema::<Content>()).expect("described"),
            [
                "request f/m",
                "request f/m . object",
                "request f/m tree object",
                "request f/m tree.children array",
                "request f/m tree.children[] ref Tree",
            ]
        );
    }

    #[test]
    fn what_the_walk_cannot_read_fails() {
        let unknown = serde_json::json!({
            "type": "object",
            "properties": { "much": { "type": "integer", "multipleOf": 2 } }
        });
        let at_the_root = serde_json::json!({ "type": "object", "multipleOf": 2 });
        let unnamed = serde_json::json!({ "$ref": "#" });
        let named_twice = serde_json::json!({
            "anyOf": [
                { "type": "object", "properties": { "a": { "type": "string" } } },
                { "type": "object", "properties": { "b": { "type": "string" } } }
            ]
        });
        for (schema, why) in [
            (unknown, "multipleOf"),
            (at_the_root, "multipleOf"),
            (unnamed, "names no definition"),
            (named_twice, "two cases labelled `object`"),
        ] {
            let error = message(&schema).expect_err("unread");
            assert!(error.to_string().contains(why), "{error}");
        }
    }

    /// The lines of a message `m/m` of `kind`, whose content's type is `T`.
    fn one<T: JsonSchema>(kind: Kind) -> Description {
        let mut description = Description::new();
        description
            .message(kind, "m/m", &schema::<T>())
            .expect("described");
        description
    }

    fn text(description: &Description) -> String {
        description
            .lines()
            .map(|line| format!("{line}\n"))
            .collect()
    }

    /// A content with a choice of tagged cases, as a version kept it, and two later ones: one whose
    /// existing case gains a field, one with a new case.
    mod choices {
        use super::*;

        #[derive(JsonSchema)]
        pub(super) struct Kept {
            pub(super) choice: KeptChoice,
        }
        #[derive(JsonSchema)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub(super) enum KeptChoice {
            Move { by: f32 },
        }

        #[derive(JsonSchema)]
        pub(super) struct Field {
            pub(super) choice: FieldChoice,
        }
        #[derive(JsonSchema)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub(super) enum FieldChoice {
            Move { by: f32, fast: bool },
        }

        #[derive(JsonSchema)]
        pub(super) struct Case {
            pub(super) choice: CaseChoice,
        }
        #[derive(JsonSchema)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        pub(super) enum CaseChoice {
            Move { by: f32 },
            Stop,
        }
    }

    #[test]
    fn a_field_within_an_existing_case_fails_in_a_request_and_passes_elsewhere() {
        for kind in [Kind::Request, Kind::Reply, Kind::Event] {
            let kept = text(&one::<choices::Kept>(kind));
            let unraised = unraised(&kept, &one::<choices::Field>(kind));
            if kind == Kind::Request {
                assert_eq!(
                    unraised,
                    ["+ request m/m choice<kind=\"move\">.fast boolean"]
                );
            } else {
                assert!(unraised.is_empty(), "{kind:?}: {unraised:?}");
            }
        }
    }

    #[test]
    fn a_new_case_passes_in_a_request_and_fails_elsewhere() {
        for kind in [Kind::Request, Kind::Reply, Kind::Event] {
            let kept = text(&one::<choices::Kept>(kind));
            let unraised = unraised(&kept, &one::<choices::Case>(kind));
            if kind == Kind::Request {
                assert!(unraised.is_empty(), "{unraised:?}");
            } else {
                assert_eq!(unraised.len(), 2, "{kind:?}: {unraised:?}");
                assert!(unraised.iter().all(|line| line.contains("<kind=\"stop\">")));
            }
        }
    }

    #[test]
    fn a_new_case_named_by_its_type_fails_in_a_request() {
        #[derive(JsonSchema)]
        struct Kept {
            value: KeptValue,
        }
        #[derive(JsonSchema)]
        #[serde(untagged)]
        enum KeptValue {
            Number(u32),
        }
        #[derive(JsonSchema)]
        struct Later {
            value: LaterValue,
        }
        #[derive(JsonSchema)]
        #[serde(untagged)]
        enum LaterValue {
            Number(u32),
            Text(String),
        }
        let kept = text(&one::<Kept>(Kind::Request));
        assert_eq!(
            unraised(&kept, &one::<Later>(Kind::Request)),
            ["+ request m/m value<string> string"]
        );
    }

    #[test]
    fn a_tagged_case_that_holds_a_type_keeps_the_type_s_fields() {
        #[derive(JsonSchema)]
        #[serde(tag = "reply", rename_all = "snake_case")]
        enum Reply {
            Answer(Body),
        }
        #[derive(JsonSchema)]
        struct Body {
            what: String,
        }
        let mut description = Description::new();
        description
            .tagged(Kind::Reply, &schema::<Reply>(), "reply", None, |_| true)
            .expect("described");
        assert_eq!(
            lines(&description),
            [
                "reply answer",
                "reply answer . object",
                "reply answer what string"
            ]
        );
    }

    #[test]
    fn a_name_or_a_label_that_a_path_cannot_hold_fails() {
        #[derive(JsonSchema)]
        struct Spaced {
            #[serde(rename = "a b")]
            spaced: u8,
        }
        #[derive(JsonSchema)]
        struct Marked {
            choice: Choice,
        }
        #[derive(JsonSchema)]
        enum Choice {
            #[serde(rename = "a>b")]
            Marked,
        }
        for error in [
            Description::new().message(Kind::Request, "m/m", &schema::<Spaced>()),
            Description::new().message(Kind::Request, "m/m", &schema::<Marked>()),
        ] {
            let error = error.expect_err("refused");
            assert!(error.to_string().contains("holds a space"), "{error}");
        }
    }

    #[test]
    fn an_enumeration_that_is_not_tagged_so_fails() {
        let untagged = serde_json::json!({ "type": "object" });
        let error = Description::new()
            .tagged(Kind::Reply, &untagged, "reply", None, |_| true)
            .expect_err("refused");
        assert!(error.to_string().contains("not an enumeration"), "{error}");
        let other_tag = schema::<kept::Reply>();
        let error = Description::new()
            .tagged(Kind::Reply, &other_tag, "kind", None, |_| true)
            .expect_err("refused");
        assert!(error.to_string().contains("names no `kind`"), "{error}");
    }

    /// A directory of the system's temporary one, removed with what it holds when dropped, even
    /// by a test that fails.
    struct Directory(std::path::PathBuf);

    impl Directory {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "uwui-agent-description-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("made");
            Self(path)
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_family_is_held_to_its_files() {
        let directory = Directory::new("held");
        let directory = directory.0.as_path();
        let now = described::<kept::Request, kept::Reply>();
        let error = check(directory, "f", 1, &now, false).expect_err("no description");
        assert!(
            matches!(error, DescriptionError::Unwritten { .. }),
            "{error}"
        );
        fs::write(directory.join("f.txt"), now.text("f", 1)).expect("written");
        let error = check(directory, "f", 1, &now, false).expect_err("no kept copy");
        assert!(matches!(error, DescriptionError::NotKept { .. }), "{error}");
        // A kept copy that held a line that the messages no longer give.
        fs::write(
            directory.join("f.1.txt"),
            format!("{}request f/gone\n", now.text("f", 1)),
        )
        .expect("written");
        let error = check(directory, "f", 1, &now, false).expect_err("unraised");
        assert!(
            matches!(&error, DescriptionError::Unraised { lines, .. } if lines == &["- request f/gone"]),
            "{error}"
        );
        fs::write(directory.join("f.1.txt"), now.text("f", 1)).expect("written");
        check(directory, "f", 1, &now, false).expect("held");
    }

    #[test]
    fn writing_writes_the_description_and_a_new_version_s_kept_copy_never_an_old_one() {
        let directory = Directory::new("write");
        let directory = directory.0.as_path();
        let now = described::<kept::Request, kept::Reply>();
        check(directory, "f", 1, &now, true).expect("written and held");
        assert_eq!(
            fs::read_to_string(directory.join("f.txt")).expect("read"),
            now.text("f", 1)
        );
        assert_eq!(
            fs::read_to_string(directory.join("f.1.txt")).expect("read"),
            now.text("f", 1)
        );
        // A request that gains a field: written again, the kept copy left as it was.
        #[derive(JsonSchema)]
        #[serde(tag = "method", content = "params")]
        enum Request {
            #[serde(rename = "f/ask")]
            Ask {
                much: u32,
                action: kept::Action,
                more: u32,
            },
        }
        let later = described::<Request, kept::Reply>();
        let error = check(directory, "f", 1, &later, true).expect_err("unraised");
        assert!(
            matches!(error, DescriptionError::Unraised { .. }),
            "{error}"
        );
        assert_eq!(
            fs::read_to_string(directory.join("f.txt")).expect("read"),
            later.text("f", 1)
        );
        assert_eq!(
            fs::read_to_string(directory.join("f.1.txt")).expect("read"),
            now.text("f", 1)
        );
        // The version raised: a new kept copy.
        check(directory, "f", 2, &later, true).expect("raised");
        assert_eq!(
            fs::read_to_string(directory.join("f.2.txt")).expect("read"),
            later.text("f", 2)
        );
    }

    #[test]
    fn lines_written_by_hand_are_marked_so() {
        let mut description = Description::new();
        description.line("envelope message . object");
        assert_eq!(lines(&description), ["envelope message . object (by hand)"]);
        assert!(
            description
                .text("core", 2)
                .contains("ends with `(by hand)`")
        );
    }
}
