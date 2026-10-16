//! Documents as TOML text: parsed into values as documents hold them, with the place of each in the
//! text, and written by the engine's own writer, which gives the same bytes for the same document
//! on every platform and with every version of the parser.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::Range as Span;

use slime_reflect::{Path, Problem, Value};
use toml_edit::{Item, Key, Table};

use crate::MAX_DEPTH;
use crate::error::ReadProblem;

/// A document's text parsed: its root table, as a document holds values, and the place in the text
/// of each of its parts and of their keys, by their paths in it. The root's is the text's start.
pub(crate) struct Parsed {
    pub root: Vec<(String, Value)>,
    pub spans: HashMap<Path, Span<usize>>,
    pub keys: HashMap<Path, Span<usize>>,
}

/// Why a text cannot be parsed, and where in it, if the parser says.
pub(crate) type ParseError = (Option<Span<usize>>, ReadProblem);

/// Parses `text`: tables as [`Value::Struct`], arrays as [`Value::List`], strings, integers, floats
/// and booleans as [`Value::Text`], [`Value::Int`], [`Value::F64`] and [`Value::Bool`].
pub(crate) fn parse(text: &str) -> Result<Parsed, ParseError> {
    let document = toml_edit::Document::parse(text).map_err(|error| {
        (
            error.span(),
            ReadProblem::Syntax(error.message().to_owned()),
        )
    })?;
    let mut parser = Parser {
        spans: HashMap::from([(Path::default(), 0..0)]),
        keys: HashMap::new(),
    };
    let root = parser.table(document.as_table(), &Path::default(), 1)?;
    Ok(Parsed {
        root,
        spans: parser.spans,
        keys: parser.keys,
    })
}

struct Parser {
    spans: HashMap<Path, Span<usize>>,
    keys: HashMap<Path, Span<usize>>,
}

impl Parser {
    /// Notes where the part at `path` lies, if the parser says.
    fn place(&mut self, path: &Path, span: Option<Span<usize>>) {
        if let Some(span) = span {
            self.spans.insert(path.clone(), span);
        }
    }

    /// Notes where the key of the part at `path` lies, if the parser says.
    fn key(&mut self, path: &Path, key: Option<&Key>) {
        if let Some(span) = key.and_then(Key::span) {
            self.keys.insert(path.clone(), span);
        }
    }

    /// Refuses a part nested deeper than [`MAX_DEPTH`].
    fn depth(&self, path: &Path, depth: usize) -> Result<(), ParseError> {
        if depth > MAX_DEPTH {
            return Err((
                self.spans.get(path).cloned(),
                ReadProblem::TooDeep { limit: MAX_DEPTH },
            ));
        }
        Ok(())
    }

    fn table(
        &mut self,
        table: &Table,
        path: &Path,
        depth: usize,
    ) -> Result<Vec<(String, Value)>, ParseError> {
        self.depth(path, depth)?;
        let mut fields = Vec::new();
        for (key, item) in table.iter() {
            let at = path.clone().field(key);
            self.key(&at, table.key(key));
            self.place(
                &at,
                item.span().or_else(|| table.key(key).and_then(Key::span)),
            );
            fields.push((key.to_owned(), self.item(item, &at, depth + 1)?));
        }
        Ok(fields)
    }

    fn item(&mut self, item: &Item, path: &Path, depth: usize) -> Result<Value, ParseError> {
        match item {
            Item::Value(value) => self.value(value, path, depth),
            Item::Table(table) => Ok(Value::Struct(self.table(table, path, depth)?)),
            Item::ArrayOfTables(tables) => {
                self.depth(path, depth)?;
                let mut elements = Vec::new();
                for (index, table) in tables.iter().enumerate() {
                    let at = path.clone().index(index);
                    self.place(&at, table.span());
                    elements.push(Value::Struct(self.table(table, &at, depth + 1)?));
                }
                Ok(Value::List(elements))
            }
            // A table lists no item that holds nothing.
            Item::None => Ok(Value::Struct(Vec::new())),
        }
    }

    fn value(
        &mut self,
        value: &toml_edit::Value,
        path: &Path,
        depth: usize,
    ) -> Result<Value, ParseError> {
        use toml_edit::Value as Toml;

        self.depth(path, depth)?;
        Ok(match value {
            Toml::String(text) => Value::Text(text.value().clone()),
            Toml::Integer(number) => Value::Int(*number.value()),
            Toml::Float(number) => Value::F64(*number.value()),
            Toml::Boolean(boolean) => Value::Bool(*boolean.value()),
            Toml::Datetime(_) => {
                return Err((
                    value.span(),
                    ReadProblem::Value(Problem::Invalid(
                        "a date or a time, which the engine's values do not hold".to_owned(),
                    )),
                ));
            }
            Toml::Array(array) => {
                let mut elements = Vec::new();
                for (index, element) in array.iter().enumerate() {
                    let at = path.clone().index(index);
                    self.place(&at, element.span());
                    elements.push(self.value(element, &at, depth + 1)?);
                }
                Value::List(elements)
            }
            Toml::InlineTable(table) => {
                let mut fields = Vec::new();
                for (key, element) in table.iter() {
                    let at = path.clone().field(key);
                    self.key(&at, table.key(key));
                    self.place(
                        &at,
                        element
                            .span()
                            .or_else(|| table.key(key).and_then(Key::span)),
                    );
                    fields.push((key.to_owned(), self.value(element, &at, depth + 1)?));
                }
                Value::Struct(fields)
            }
        })
    }
}

/// Where a value is written: on its key's line, or under a header of its own.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    /// On its key's line: a number, a boolean, text, an array that holds no table of sections, a
    /// table that holds nothing but numbers, booleans and text.
    Inline,
    /// A table under its own header, `[a.b]`.
    Section,
    /// An array of tables, one at least under its own header: each table under `[[a.b]]`.
    Sections,
}

fn place(value: &Value) -> Place {
    match value {
        Value::Struct(fields) if !fields.iter().all(|(_, value)| scalar(value)) => Place::Section,
        Value::List(elements)
            if !elements.is_empty()
                && elements
                    .iter()
                    .all(|element| matches!(element, Value::Struct(_)))
                && elements
                    .iter()
                    .any(|element| place(element) == Place::Section) =>
        {
            Place::Sections
        }
        _ => Place::Inline,
    }
}

fn scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Bool(_) | Value::Int(_) | Value::F32(_) | Value::F64(_) | Value::Text(_)
    )
}

/// Writes the document whose root table holds `fields`: each table's values written on their
/// keys' lines first, in their order, then the tables under headers of their own, in their order,
/// a blank line before each header; LF line endings, and one at the end.
pub(crate) fn print(fields: &[(String, Value)]) -> String {
    let mut out = String::new();
    body(&mut out, &[], fields);
    out
}

fn body(out: &mut String, header: &[&str], fields: &[(String, Value)]) {
    for (key, value) in fields {
        if place(value) == Place::Inline {
            out.push_str(&key_text(key));
            out.push_str(" = ");
            inline(out, value);
            out.push('\n');
        }
    }
    for (key, value) in fields {
        let path: Vec<&str> = header.iter().copied().chain([key.as_str()]).collect();
        let named = path.iter().map(|key| key_text(key)).collect::<Vec<_>>();
        match (place(value), value) {
            // A table that holds nothing but tables under headers of their own has no header: theirs
            // name it.
            (Place::Section, Value::Struct(inner)) => {
                if inner.iter().any(|(_, value)| place(value) == Place::Inline) {
                    gap(out);
                    let _ = writeln!(out, "[{}]", named.join("."));
                }
                body(out, &path, inner);
            }
            (Place::Sections, Value::List(elements)) => {
                for element in elements {
                    if let Value::Struct(inner) = element {
                        gap(out);
                        let _ = writeln!(out, "[[{}]]", named.join("."));
                        body(out, &path, inner);
                    }
                }
            }
            _ => {}
        }
    }
}

/// A blank line before a header, unless it is the first line.
fn gap(out: &mut String) {
    if !out.is_empty() {
        out.push('\n');
    }
}

/// Writes `value` on one line.
fn inline(out: &mut String, value: &Value) {
    match value {
        Value::Bool(boolean) => out.push_str(if *boolean { "true" } else { "false" }),
        Value::Int(number) => {
            let _ = write!(out, "{number}");
        }
        Value::F32(number) => out.push_str(&float(
            number.is_nan(),
            number.is_sign_negative(),
            number.is_infinite(),
            || format!("{number:e}"),
        )),
        Value::F64(number) => out.push_str(&float(
            number.is_nan(),
            number.is_sign_negative(),
            number.is_infinite(),
            || format!("{number:e}"),
        )),
        Value::Text(text) => quoted(out, text),
        Value::List(elements) => {
            out.push('[');
            for (index, element) in elements.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                inline(out, element);
            }
            out.push(']');
        }
        Value::Struct(fields) if fields.is_empty() => out.push_str("{}"),
        Value::Struct(fields) => {
            out.push_str("{ ");
            for (index, (key, element)) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&key_text(key));
                out.push_str(" = ");
                inline(out, element);
            }
            out.push_str(" }");
        }
        other => unreachable!("a document holds no {}", other.kind()),
    }
}

/// A float in TOML: `nan`, `-nan`, `inf` or `-inf`, or the shortest digits that read back as the
/// same number, `exponential` writing them as Rust's `{:e}` does; in plain decimals from 1e-5 to
/// below 1e16, with a fraction, `1.0`, and as `1.5e-7` past them.
fn float(nan: bool, negative: bool, infinite: bool, exponential: impl Fn() -> String) -> String {
    let sign = if negative { "-" } else { "" };
    if nan {
        return format!("{sign}nan");
    }
    if infinite {
        return format!("{sign}inf");
    }
    let text = exponential();
    let (mantissa, exponent) = text.split_once('e').expect("`{:e}` writes an exponent");
    let exponent: i32 = exponent.parse().expect("`{:e}` writes a whole exponent");
    if !(-5..16).contains(&exponent) {
        return text;
    }
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    // How many of the digits come before the point: the first is the units' at exponent 0.
    let before = exponent + 1;
    if before <= 0 {
        let zeros = "0".repeat(before.unsigned_abs() as usize);
        format!("{sign}0.{zeros}{digits}")
    } else if before as usize >= digits.len() {
        let zeros = "0".repeat(before as usize - digits.len());
        format!("{sign}{digits}{zeros}.0")
    } else {
        let (whole, fraction) = digits.split_at(before as usize);
        format!("{sign}{whole}.{fraction}")
    }
}

/// A key: bare when it holds only ASCII letters, digits, `_` and `-`, quoted otherwise.
fn key_text(key: &str) -> String {
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare {
        key.to_owned()
    } else {
        let mut out = String::new();
        quoted(&mut out, key);
        out
    }
}

/// `text` as a TOML basic string: quotes, backslashes and control characters escaped, the short
/// escapes where TOML has them, `\u` and four hexadecimal digits otherwise.
fn quoted(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() && (c as u32) < 0x80 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(number: f64) -> String {
        let mut out = String::new();
        inline(&mut out, &Value::F64(number));
        out
    }

    #[test]
    fn floats_are_written_in_their_shortest_digits() {
        for (number, text) in [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (0.1, "0.1"),
            (-2.5, "-2.5"),
            (140.0, "140.0"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e16"),
            (1.5e-5, "0.000015"),
            (1.5e-6, "1.5e-6"),
            (f64::MAX, "1.7976931348623157e308"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
            (-f64::NAN, "-nan"),
        ] {
            assert_eq!(written(number), text);
        }
        let mut out = String::new();
        inline(&mut out, &Value::F32(0.1));
        assert_eq!(out, "0.1", "a 32-bit float in its own shortest digits");
    }

    #[test]
    fn text_and_keys_are_quoted_where_toml_asks() {
        let mut out = String::new();
        quoted(&mut out, "a \"b\" \\ c\nd\u{1}é");
        assert_eq!(out, r#""a \"b\" \\ c\nd\u0001é""#);
        assert_eq!(key_text("intensity"), "intensity");
        assert_eq!(key_text("hunnawood::Door"), "\"hunnawood::Door\"");
        assert_eq!(key_text(""), "\"\"");
    }

    #[test]
    fn small_tables_are_inline_and_large_ones_under_headers() {
        let number = |n| Value::F64(n);
        let vector = Value::Struct(vec![("x".to_owned(), number(1.0))]);
        let spot = Value::Struct(vec![
            ("color".to_owned(), vector.clone()),
            ("intensity".to_owned(), number(140.0)),
        ]);
        let root = vec![
            ("name".to_owned(), Value::Text("lantern".to_owned())),
            (
                "light".to_owned(),
                Value::Struct(vec![("Spot".to_owned(), spot)]),
            ),
            ("at".to_owned(), vector.clone()),
            (
                "posts".to_owned(),
                Value::List(vec![vector.clone(), vector]),
            ),
        ];
        assert_eq!(
            print(&root),
            "name = \"lantern\"\n\
             at = { x = 1.0 }\n\
             posts = [{ x = 1.0 }, { x = 1.0 }]\n\
             \n\
             [light.Spot]\n\
             color = { x = 1.0 }\n\
             intensity = 140.0\n"
        );
    }
}
