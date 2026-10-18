//! Documents as TOML text: parsed into values as documents hold them, and written by the engine's
//! own writer, which gives the same bytes for the same document on every platform and with every
//! version of the parser.

use std::fmt::Write as _;
use std::ops::Range as Span;

use slime_reflect::{Path, Problem, Step, Value};
use toml_edit::{Document, Item, Key, Table};

use crate::MAX_DEPTH;
use crate::error::ReadProblem;

/// A document's text parsed: the parser's document, which knows where each part lies in the text
/// ([`locate`]), and the root table as a document holds values.
pub(crate) struct Parsed<'a> {
    pub document: Document<&'a str>,
    pub root: Vec<(String, Value)>,
}

/// Why a text cannot be parsed, and where in it, if the parser says.
pub(crate) type ParseError = (Option<Span<usize>>, ReadProblem);

/// Parses `text`: tables as [`Value::Struct`], arrays as [`Value::List`], strings, integers, floats
/// and booleans as [`Value::Text`], [`Value::Int`], [`Value::F64`] and [`Value::Bool`]. Where each
/// part lies is not noted: [`locate`] finds it when an error needs it.
pub(crate) fn parse(text: &str) -> Result<Parsed<'_>, ParseError> {
    let document = Document::parse(text).map_err(|error| {
        (
            error.span(),
            ReadProblem::Syntax(error.message().to_owned()),
        )
    })?;
    let root = table(document.as_table(), 1)?;
    Ok(Parsed { document, root })
}

/// Refuses a part nested deeper than [`MAX_DEPTH`], at `span`.
fn deep(span: Option<Span<usize>>, depth: usize) -> Result<(), ParseError> {
    if depth > MAX_DEPTH {
        return Err((span, ReadProblem::TooDeep { limit: MAX_DEPTH }));
    }
    Ok(())
}

fn table(table: &Table, depth: usize) -> Result<Vec<(String, Value)>, ParseError> {
    deep(table.span(), depth)?;
    table
        .iter()
        .map(|(key, item)| Ok((key.to_owned(), self::item(item, depth + 1)?)))
        .collect()
}

fn item(item: &Item, depth: usize) -> Result<Value, ParseError> {
    match item {
        Item::Value(value) => self::value(value, depth),
        Item::Table(inner) => Ok(Value::Struct(table(inner, depth)?)),
        Item::ArrayOfTables(tables) => {
            deep(tables.span(), depth)?;
            let elements = tables
                .iter()
                .map(|inner| Ok(Value::Struct(table(inner, depth + 1)?)))
                .collect::<Result<_, ParseError>>()?;
            Ok(Value::List(elements))
        }
        // A table lists no item that holds nothing.
        Item::None => Ok(Value::Struct(Vec::new())),
    }
}

fn value(value: &toml_edit::Value, depth: usize) -> Result<Value, ParseError> {
    use toml_edit::Value as Toml;

    deep(value.span(), depth)?;
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
        Toml::Array(array) => Value::List(
            array
                .iter()
                .map(|element| self::value(element, depth + 1))
                .collect::<Result<_, _>>()?,
        ),
        Toml::InlineTable(inner) => Value::Struct(
            inner
                .iter()
                .map(|(key, element)| Ok((key.to_owned(), self::value(element, depth + 1)?)))
                .collect::<Result<_, ParseError>>()?,
        ),
    })
}

/// Where the part at `at`, a path through the values of `document`, lies in its text: its key's
/// place if `key` and it has one, its value's otherwise; the place of the nearest part that holds
/// it when the document has no such part, as for a part that an upgrade made; the text's start for
/// the root.
pub(crate) fn locate(document: &Document<&str>, at: &Path, key: bool) -> Option<Span<usize>> {
    use toml_edit::Value as Toml;

    #[derive(Clone, Copy)]
    enum Node<'a> {
        Table(&'a Table),
        Item(&'a Item),
        Value(&'a Toml),
    }
    let mut node = Node::Table(document.as_table());
    let mut found = Some(0..0);
    for step in at.steps() {
        let next = match (node, step) {
            (Node::Table(table) | Node::Item(Item::Table(table)), Step::Field(name)) => table
                .get_key_value(name)
                .map(|(key, item)| (Some(key), Node::Item(item))),
            (
                Node::Item(Item::Value(Toml::InlineTable(table)))
                | Node::Value(Toml::InlineTable(table)),
                Step::Field(name),
            ) => table
                .get_key_value(name)
                .map(|(key, item)| (Some(key), Node::Item(item))),
            (Node::Item(Item::ArrayOfTables(tables)), &Step::Index(index)) => {
                tables.get(index).map(|table| (None, Node::Table(table)))
            }
            (
                Node::Item(Item::Value(Toml::Array(array))) | Node::Value(Toml::Array(array)),
                &Step::Index(index),
            ) => array.get(index).map(|element| (None, Node::Value(element))),
            _ => None,
        };
        let Some((named, next)) = next else {
            break;
        };
        let key_span = named.and_then(Key::span);
        let span = match next {
            Node::Table(table) => table.span(),
            Node::Item(item) => item.span(),
            Node::Value(value) => value.span(),
        };
        found = if key {
            key_span.or(span).or(found)
        } else {
            span.or(key_span).or(found)
        };
        node = next;
    }
    found
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

/// Where each of `fields` is written in its table: as [`place`] says, but once one goes under a header,
/// the tables after it go under headers too, small ones included, so that a table's tables keep
/// their order, which the entities of a scene file need; numbers, booleans, texts and arrays that hold
/// no table stay on their keys' lines, before the first header, as TOML wants them.
fn placements(fields: &[(String, Value)]) -> Vec<Place> {
    let first = fields
        .iter()
        .position(|(_, value)| place(value) != Place::Inline);
    fields
        .iter()
        .enumerate()
        .map(|(index, (_, value))| {
            let after = first.is_some_and(|first| index > first);
            match (place(value), value) {
                (Place::Inline, Value::Struct(_)) if after => Place::Section,
                (Place::Inline, Value::List(elements))
                    if after
                        && !elements.is_empty()
                        && elements
                            .iter()
                            .all(|element| matches!(element, Value::Struct(_))) =>
                {
                    Place::Sections
                }
                (place, _) => place,
            }
        })
        .collect()
}

fn body(out: &mut String, header: &[&str], fields: &[(String, Value)]) {
    let placed = placements(fields);
    for ((key, value), place) in fields.iter().zip(&placed) {
        if *place == Place::Inline {
            out.push_str(&key_text(key));
            out.push_str(" = ");
            inline(out, value);
            out.push('\n');
        }
    }
    for ((key, value), place) in fields.iter().zip(placed) {
        let path: Vec<&str> = header.iter().copied().chain([key.as_str()]).collect();
        let named = path.iter().map(|key| key_text(key)).collect::<Vec<_>>();
        match (place, value) {
            // A table that holds nothing but tables under headers of their own has no header: theirs
            // name it. One that holds nothing has one, which makes it.
            (Place::Section, Value::Struct(inner)) => {
                if inner.is_empty() || placements(inner).contains(&Place::Inline) {
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
            || f32_digits(*number),
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

/// The digits that a reader brings back to `number`: its own shortest ones, unless reading them as
/// a 64-bit float, as TOML's parser does, then rounding to 32 bits would give another `f32`; then
/// those of the same number as an `f64`, which it is exactly. The shortest digits of an `f32`
/// round to it directly, but rounding twice can miss it: 7.038531e-26 is one.
fn f32_digits(number: f32) -> String {
    let shortest = format!("{number:e}");
    match shortest.parse::<f64>() {
        Ok(read) if (read as f32).to_bits() == number.to_bits() => shortest,
        _ => format!("{:e}", f64::from(number)),
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
    fn small_tables_are_inline_and_large_ones_under_headers_in_their_order() {
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
        // Before the light, on their keys' lines; after it, under headers, so that they come after it.
        assert_eq!(
            print(&root),
            "name = \"lantern\"\n\
             \n\
             [light.Spot]\n\
             color = { x = 1.0 }\n\
             intensity = 140.0\n\
             \n\
             [at]\n\
             x = 1.0\n\
             \n\
             [[posts]]\n\
             x = 1.0\n\
             \n\
             [[posts]]\n\
             x = 1.0\n"
        );
        let small_first = vec![root[2].clone(), root[1].clone()];
        assert_eq!(
            print(&small_first),
            "at = { x = 1.0 }\n\n[light.Spot]\ncolor = { x = 1.0 }\nintensity = 140.0\n"
        );
    }
}
