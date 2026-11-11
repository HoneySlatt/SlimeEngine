//! A stack of themes read from KDL: each value of the kind that its group gives, its names and
//! colour functions worked out once for the whole stack, without recursion on names
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
//!
//! A theme is a list of groups, `color`, `length`, `duration`, `number` and `text`, each a list of
//! values, a node named by the value with one argument. A value is written as its kind writes it,
//! or names another value of its kind, `$name`. A theme stacked later gives a name again to change
//! its value; the first theme to give a name sets its kind.

use std::collections::HashMap;
use std::ops::Range;

use slime_color::LinearColor;
use uwui_kdl::{Node, Value as Kdl};

use super::{call, lighten, mix, srgb};
use crate::document::{DocumentError, DocumentErrors};

/// The most names that a chain of names follows, each naming the next, before it is refused.
pub(crate) const CHAIN: usize = 64;

/// The most colour functions nested in one another, `mix(lighten($a, 0.1), …)`, before a value is
/// refused.
pub(crate) const NESTING: usize = 8;

/// A theme's groups, each the kind of the values it gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Group {
    Color,
    Length,
    Duration,
    Number,
    Text,
}

impl Group {
    const ALL: [Self; 5] = [
        Self::Color,
        Self::Length,
        Self::Duration,
        Self::Number,
        Self::Text,
    ];

    /// Its name, as a theme writes it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Length => "length",
            Self::Duration => "duration",
            Self::Number => "number",
            Self::Text => "text",
        }
    }

    /// A value of its kind, with its article, as an error names it.
    pub(crate) fn a(self) -> &'static str {
        match self {
            Self::Color => "a color",
            Self::Length => "a length",
            Self::Duration => "a duration",
            Self::Number => "a number",
            Self::Text => "a text",
        }
    }
}

/// A theme's value, worked out.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ThemeValue {
    Color(LinearColor),
    /// In logical pixels.
    Length(f32),
    /// In seconds, never negative, as a style holds a transition's.
    Duration(f32),
    Number(f32),
    Text(String),
}

impl ThemeValue {
    /// The group of its kind.
    pub(crate) fn group(&self) -> Group {
        match self {
            Self::Color(_) => Group::Color,
            Self::Length(_) => Group::Length,
            Self::Duration(_) => Group::Duration,
            Self::Number(_) => Group::Number,
            Self::Text(_) => Group::Text,
        }
    }
}

/// A value as a theme writes it, before its names are worked out.
#[derive(Debug)]
enum Written {
    Value(ThemeValue),
    /// `$name`: another value of its kind.
    Name(String),
    /// A colour function, `mix($a, $b, 20%)`, read into its expression.
    Function(Expression),
}

/// A colour function's argument, or the function itself.
#[derive(Debug)]
enum Expression {
    Name(String),
    Color(LinearColor),
    Share(f32),
    Call(Function, Vec<Expression>),
}

#[derive(Clone, Copy, Debug)]
enum Function {
    Mix,
    Lighten,
    Darken,
    Alpha,
}

/// A value given by a theme of the stack, where it is.
#[derive(Debug)]
struct Given {
    /// The theme, by its place in the stack.
    theme: usize,
    group: Group,
    written: Written,
    /// Where the value's node lies in its theme's text.
    span: Range<usize>,
}

/// The values of a stack of themes, worked out.
#[derive(Clone, Debug, Default)]
pub(crate) struct Values {
    values: HashMap<String, ThemeValue>,
    /// The theme that gives each name, the last that gives it, and where in its text: what an
    /// error about a token's value names.
    places: HashMap<String, (usize, usize)>,
}

impl Values {
    /// The value named `name`; none where no theme gives it, or its value is wrong.
    pub(crate) fn get(&self, name: &str) -> Option<&ThemeValue> {
        self.values.get(name)
    }

    /// The theme that gives `name`, by its place in the stack, and the byte where its value lies.
    pub(crate) fn place(&self, name: &str) -> Option<(usize, usize)> {
        self.places.get(name).copied()
    }
}

/// The values of `themes`, each `(name, text)`, stacked in their order: of several themes, the
/// last that gives a name gives its value.
///
/// # Errors
///
/// Each error found, up to its cap, at its line: a text that is not KDL; a node that is not a
/// group, or a value that is not one argument of its group's kind; a name given twice in a theme,
/// or given by a later theme in another group than the first; `#null`; a duration without its
/// unit, negative or too long, a unit on a name or of another kind; a number that is not finite;
/// a name that no theme gives, or that names a value of another kind; a chain of names that comes
/// back to itself or is longer than [`CHAIN`]; a colour function unknown, given what it does not
/// take, or nested deeper than [`NESTING`].
pub(crate) fn read(themes: &[(&str, &str)]) -> Result<Values, DocumentErrors> {
    let mut errors = DocumentErrors::new();
    let mut given: HashMap<String, Given> = HashMap::new();
    // The group of each name, as the first theme to give it set it, and where.
    let mut kinds: HashMap<String, (Group, usize, usize)> = HashMap::new();
    // The names given wrongly, already told: what names them is not told again.
    let mut wrong: Vec<String> = Vec::new();
    for (index, &(name, text)) in themes.iter().enumerate() {
        let document = match uwui_kdl::read(text) {
            Ok(document) => document,
            Err(error) => {
                errors.push(DocumentError {
                    document: name.to_owned(),
                    error: crate::document::Fault::Kdl(error),
                });
                continue;
            }
        };
        let fault = |offset: usize, message: String| DocumentError::at(name, text, offset, message);
        let mut in_this: HashMap<String, usize> = HashMap::new();
        let mut groups: Vec<(Group, usize)> = Vec::new();
        for group_node in document.nodes() {
            let group_name = group_node.name().text();
            let Some(group) = Group::ALL.into_iter().find(|g| g.name() == group_name) else {
                errors.push(fault(
                    group_node.span().start,
                    format!(
                        "`{group_name}` is not a group: a theme holds `color`, `length`, \
                         `duration`, `number` and `text`, each a list of values"
                    ),
                ));
                continue;
            };
            if let Some(&(_, first)) = groups.iter().find(|(given, _)| *given == group) {
                let (line, _) = line(text, first);
                errors.push(fault(
                    group_node.span().start,
                    format!("the group `{group_name}` is given twice, first at line {line}"),
                ));
                continue;
            }
            groups.push((group, group_node.span().start));
            if group_node.annotation().is_some() || group_node.entries().len() > 0 {
                errors.push(fault(
                    group_node.span().start,
                    format!("the group `{group_name}` holds its values in a block alone"),
                ));
                continue;
            }
            for node in group_node.children() {
                let value_name = node.name().text().into_owned();
                let start = node.span().start;
                if let Some(&first) = in_this.get(&value_name) {
                    let (line, _) = line(text, first);
                    errors.push(fault(
                        start,
                        format!(
                            "`{value_name}` is given twice in this theme, first at line {line}"
                        ),
                    ));
                    continue;
                }
                in_this.insert(value_name.clone(), start);
                if let Some(&(kind, theme, at)) = kinds.get(&value_name)
                    && kind != group
                {
                    let (first_name, first_text) = themes[theme];
                    let (line, _) = line(first_text, at);
                    errors.push(fault(
                        start,
                        format!(
                            "`{value_name}` is {} here, but {} in `{first_name}`, at line {line}: \
                             the first theme to give a name sets its kind",
                            group.a(),
                            kind.a()
                        ),
                    ));
                    continue;
                }
                kinds
                    .entry(value_name.clone())
                    .or_insert((group, index, start));
                match written(group, &node) {
                    Ok(written) => {
                        wrong.retain(|name| *name != value_name);
                        given.insert(
                            value_name,
                            Given {
                                theme: index,
                                group,
                                written,
                                span: node.span(),
                            },
                        );
                    }
                    Err((offset, message)) => {
                        given.remove(&value_name);
                        wrong.push(value_name.clone());
                        errors.push(fault(offset, format!("`{value_name}`: {message}")));
                    }
                }
            }
        }
    }
    let values = resolve(themes, &given, &wrong, &mut errors);
    if errors.is_empty() {
        let places = given
            .iter()
            .map(|(name, given)| (name.clone(), (given.theme, given.span.start)))
            .collect();
        Ok(Values { values, places })
    } else {
        Err(errors)
    }
}

/// The line and the column of the byte `offset` of `text`, as the reader counts them.
fn line(text: &str, offset: usize) -> (u32, u32) {
    let error = uwui_kdl::Error::at(text, offset, "");
    (error.line, error.column)
}

/// What the value node `node` of the group `group` writes; else the byte where it is wrong, and
/// what is wrong.
fn written(group: Group, node: &Node<'_, '_>) -> Result<Written, (usize, String)> {
    let at = node.span().start;
    if node.annotation().is_some() {
        return Err((at, "a value's name takes no type".to_owned()));
    }
    if node.children().len() > 0 {
        return Err((at, "a value holds no block".to_owned()));
    }
    let mut entries = node.entries();
    let (Some(entry), None) = (entries.next(), entries.next()) else {
        return Err((at, format!("a value is one argument, {}", group.a())));
    };
    let at = entry.span().start;
    if entry.name().is_some() {
        return Err((
            at,
            format!("a value is one argument, {}, not a property", group.a()),
        ));
    }
    let unit = entry.annotation().map(|unit| unit.text());
    let value = entry.value();
    if let Kdl::String(string) = &value {
        let string = string.text();
        if let Some(name) = string.strip_prefix('$')
            && !(group == Group::Text && name.starts_with('$'))
        {
            if name.is_empty() {
                return Err((at, "`$` names no value: `$name`".to_owned()));
            }
            if unit.is_some() {
                return Err((
                    at,
                    "a name takes the unit of its value's group: it writes none".to_owned(),
                ));
            }
            return Ok(Written::Name(name.to_owned()));
        }
    }
    let unit_refused = |unit: &str| Err((at, format!("{} takes no unit `({unit})`", group.a())));
    match (group, value) {
        (_, Kdl::Null) => Err((
            at,
            "`#null` removes no value: a theme gives a value, or leaves it to the themes below"
                .to_owned(),
        )),
        (Group::Color, Kdl::String(string)) => {
            if let Some(unit) = &unit {
                return unit_refused(unit);
            }
            let text = string.text();
            if let Some(hex) = text.strip_prefix('#') {
                return srgb(hex)
                    .map(|color| Written::Value(ThemeValue::Color(color)))
                    .ok_or_else(|| {
                        (
                            at,
                            format!("`{text}` is not a color: `#rrggbb` or `#rrggbbaa`"),
                        )
                    });
            }
            expression(&text, Group::Color, 0)
                .map(Written::Function)
                .map_err(|message| (at, message))
        }
        (Group::Text, Kdl::String(string)) => {
            if let Some(unit) = &unit {
                return unit_refused(unit);
            }
            let text = string.text();
            // `$$` writes a text that starts with `$`.
            let text = text.strip_prefix('$').unwrap_or(&text);
            Ok(Written::Value(ThemeValue::Text(text.to_owned())))
        }
        (Group::Length | Group::Duration | Group::Number, Kdl::Number(number)) => {
            let number = number.to_f64();
            if !number.is_finite() {
                return Err((at, "a number that is not finite".to_owned()));
            }
            match (group, unit.as_deref()) {
                (Group::Length, None | Some("px")) => {
                    Ok(Written::Value(ThemeValue::Length(number as f32)))
                }
                (Group::Number, None) => Ok(Written::Value(ThemeValue::Number(number as f32))),
                (Group::Duration, Some(unit @ ("ms" | "s"))) => {
                    let seconds = if unit == "ms" {
                        number / 1000.0
                    } else {
                        number
                    };
                    if seconds < 0.0 {
                        return Err((at, "a duration is never negative".to_owned()));
                    }
                    let seconds = seconds as f32;
                    if std::time::Duration::try_from_secs_f32(seconds).is_err() {
                        return Err((at, "a duration too long to hold".to_owned()));
                    }
                    Ok(Written::Value(ThemeValue::Duration(seconds)))
                }
                (Group::Duration, None) => Err((
                    at,
                    "a duration writes its unit, `(ms)120` or `(s)0.12`".to_owned(),
                )),
                (Group::Duration, Some(unit)) => Err((
                    at,
                    format!("a duration is in `(ms)` or `(s)`, not `({unit})`"),
                )),
                (Group::Length, Some(unit)) => Err((
                    at,
                    format!("a length is in logical pixels, bare or `(px)`, not `({unit})`"),
                )),
                (_, Some(unit)) => unit_refused(unit),
                (Group::Color | Group::Text, None) => {
                    unreachable!("a number of a group of numbers")
                }
            }
        }
        (group, _) => Err((at, format!("the value is not {}", group.a()))),
    }
}

/// What `text` writes in a place that takes a value of `wanted`, a color or a share, a number:
/// read into its expression, at `depth` functions within others. A parser of bounded depth, which
/// recurses at most [`NESTING`] times.
fn expression(text: &str, wanted: Group, depth: usize) -> Result<Expression, String> {
    let text = text.trim();
    if let Some(name) = text.strip_prefix('$') {
        return if name.is_empty() {
            Err("`$` names no value: `$name`".to_owned())
        } else {
            Ok(Expression::Name(name.to_owned()))
        };
    }
    if wanted == Group::Number {
        return share(text).map(Expression::Share);
    }
    if let Some(hex) = text.strip_prefix('#') {
        return srgb(hex)
            .map(Expression::Color)
            .ok_or_else(|| format!("`{text}` is not a color: `#rrggbb` or `#rrggbbaa`"));
    }
    let Some((name, arguments)) = call(text) else {
        return Err(format!(
            "`{text}` is no color: `#rrggbb`, a value's name, `$name`, or a function such as \
             `mix($a, $b, 20%)`"
        ));
    };
    if depth >= NESTING {
        return Err(format!(
            "colour functions nested deeper than {NESTING}, one within another"
        ));
    }
    let function = match name {
        "mix" => Function::Mix,
        "lighten" => Function::Lighten,
        "darken" => Function::Darken,
        "alpha" => Function::Alpha,
        _ => {
            return Err(format!(
                "no color function `{name}`: `mix`, `lighten`, `darken` or `alpha`"
            ));
        }
    };
    let wanted = match function {
        Function::Mix => 3,
        _ => 2,
    };
    if arguments.len() != wanted {
        return Err(usage(function));
    }
    let arguments = arguments
        .into_iter()
        .enumerate()
        .map(|(place, argument)| expression(argument, argument_group(function, place), depth + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Expression::Call(function, arguments))
}

/// How `function` is called, as an error tells it.
fn usage(function: Function) -> String {
    match function {
        Function::Mix => "`mix` takes two colors and a share: `mix($a, $b, 20%)`".to_owned(),
        Function::Lighten => {
            "`lighten` takes a color and a share of lightness: `lighten($a, 0.08)`".to_owned()
        }
        Function::Darken => {
            "`darken` takes a color and a share of lightness: `darken($a, 0.08)`".to_owned()
        }
        Function::Alpha => "`alpha` takes a color and its opacity: `alpha($a, 40%)`".to_owned(),
    }
}

/// The share that `text` writes: a number, `0.2`, or a percentage, `20%`, read as `f32` and the
/// percentage scaled in `f32`, as the colour functions always read it.
pub(crate) fn share(text: &str) -> Result<f32, String> {
    let text = text.trim();
    let (number, scale) = match text.strip_suffix('%') {
        Some(number) => (number, 0.01),
        None => (text, 1.0),
    };
    number
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|share| share.is_finite())
        .map(|share| share * scale)
        .ok_or_else(|| format!("`{text}` is no share: a number, `0.2`, or a percentage, `20%`"))
}

/// The names that `expression` names, at any depth, each with the group it must be of.
fn names(expression: &Expression, wanted: Group, out: &mut Vec<(String, Group)>) {
    match expression {
        Expression::Name(name) => out.push((name.clone(), wanted)),
        Expression::Color(_) | Expression::Share(_) => {}
        Expression::Call(function, arguments) => {
            for (place, argument) in arguments.iter().enumerate() {
                names(argument, argument_group(*function, place), out);
            }
        }
    }
}

/// The group of the argument at `place` of `function`: a color, or a share, a number.
fn argument_group(function: Function, place: usize) -> Group {
    match (function, place) {
        (Function::Mix, 0 | 1) | (_, 0) => Group::Color,
        _ => Group::Number,
    }
}

/// The values that `given` writes, each name followed through the others once, without recursion:
/// a value is worked out once those it names are, along a stack of the names being worked out.
fn resolve(
    themes: &[(&str, &str)],
    given: &HashMap<String, Given>,
    wrong: &[String],
    errors: &mut DocumentErrors,
) -> HashMap<String, ThemeValue> {
    let mut values: HashMap<String, ThemeValue> = HashMap::new();
    // The names whose value is wrong, or names what is wrong, already told.
    let mut failed: Vec<&str> = wrong.iter().map(String::as_str).collect();
    let fault = |given: &Given, message: String| {
        let (name, text) = themes[given.theme];
        DocumentError::at(name, text, given.span.start, message)
    };
    let mut names_sorted: Vec<&String> = given.keys().collect();
    names_sorted.sort();
    for start in names_sorted {
        if values.contains_key(start.as_str()) || failed.contains(&start.as_str()) {
            continue;
        }
        let mut stack: Vec<&str> = vec![start];
        while let Some(&name) = stack.last() {
            let value = &given[name];
            // The names it needs, each with the group it must be of.
            let mut needed = Vec::new();
            match &value.written {
                Written::Value(_) => {}
                Written::Name(other) => needed.push((other.clone(), value.group)),
                Written::Function(expression) => names(expression, Group::Color, &mut needed),
            }
            let mut waiting = None;
            let mut wrong = None;
            for (other, group) in &needed {
                if failed.contains(&other.as_str()) {
                    wrong = Some(None);
                    break;
                }
                if let Some(found) = values.get(other) {
                    if found.group() != *group {
                        wrong = Some(Some(format!(
                            "`{name}` names `{other}`, which is {}, not {}",
                            found.group().a(),
                            group.a()
                        )));
                        break;
                    }
                    continue;
                }
                let Some((other, other_given)) = given.get_key_value(other.as_str()) else {
                    wrong = Some(Some(format!("no theme gives a value named `{other}`")));
                    break;
                };
                if other_given.group != *group {
                    wrong = Some(Some(format!(
                        "`{name}` names `{other}`, which is {}, not {}",
                        other_given.group.a(),
                        group.a()
                    )));
                    break;
                }
                if stack.contains(&other.as_str()) {
                    wrong = Some(Some(format!("`{other}` names itself through other values")));
                    break;
                }
                if stack.len() >= CHAIN {
                    wrong = Some(Some(format!(
                        "a chain of more than {CHAIN} names, each naming the next"
                    )));
                    break;
                }
                waiting = Some(other.as_str());
                break;
            }
            if let Some(message) = wrong {
                // Every name on the stack names what is wrong: the one that does is told, the
                // others left out.
                if let Some(message) = message {
                    errors.push(fault(value, message));
                }
                failed.append(&mut stack);
                break;
            }
            if let Some(next) = waiting {
                stack.push(next);
                continue;
            }
            let worked = match &value.written {
                Written::Value(value) => Ok(value.clone()),
                Written::Name(other) => Ok(values[other.as_str()].clone()),
                Written::Function(expression) => {
                    evaluate_color(expression, &values).map(ThemeValue::Color)
                }
            };
            match worked {
                Ok(worked) => {
                    values.insert(name.to_owned(), worked);
                    stack.pop();
                }
                Err(message) => {
                    errors.push(fault(value, message));
                    failed.append(&mut stack);
                }
            }
        }
    }
    values
}

/// The color that `expression` gives, the names it holds already worked out in `values`, each of
/// the group that its place takes.
fn evaluate_color(
    expression: &Expression,
    values: &HashMap<String, ThemeValue>,
) -> Result<LinearColor, String> {
    match expression {
        Expression::Color(color) => Ok(*color),
        Expression::Name(name) => match values.get(name) {
            Some(ThemeValue::Color(color)) => Ok(*color),
            _ => Err(format!("the value `{name}` is no color")),
        },
        Expression::Share(_) => Err("a share where a color goes".to_owned()),
        Expression::Call(function, arguments) => {
            let color = |place: usize| evaluate_color(&arguments[place], values);
            let share = |place: usize| match &arguments[place] {
                Expression::Share(share) => Ok(*share),
                Expression::Name(name) => match values.get(name) {
                    Some(ThemeValue::Number(share)) => Ok(*share),
                    _ => Err(format!("the value `{name}` is no number")),
                },
                _ => Err(usage(*function)),
            };
            Ok(match function {
                Function::Mix => mix(color(0)?, color(1)?, share(2)?),
                Function::Lighten => lighten(color(0)?, share(1)?),
                Function::Darken => lighten(color(0)?, -share(1)?),
                Function::Alpha => LinearColor {
                    a: share(1)?.clamp(0.0, 1.0),
                    ..color(0)?
                },
            })
        }
    }
}

/// The color that `text`, a color written in a style sheet, gives: `#rrggbb`, a value's name or a
/// colour function, the names it holds taken from `values`.
pub(crate) fn color(text: &str, values: &Values) -> Result<LinearColor, String> {
    let expression = expression(text, Group::Color, 0)?;
    evaluate_color(&expression, &values.values)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values of `theme`, alone in its stack.
    fn values(theme: &str) -> Values {
        read(&[("a.theme", theme)]).unwrap_or_else(|errors| panic!("{errors}"))
    }

    #[test]
    fn a_text_is_given_named_or_written_with_two_dollars_when_it_starts_with_one() {
        let values =
            values("text {\n    tick \"\\u{E06C}\"\n    mark $tick\n    price \"$$5\"\n}\n");
        let text = |name: &str| values.get(name).cloned();
        assert_eq!(text("tick"), Some(ThemeValue::Text("\u{E06C}".to_owned())));
        assert_eq!(text("mark"), Some(ThemeValue::Text("\u{E06C}".to_owned())));
        assert_eq!(text("price"), Some(ThemeValue::Text("$5".to_owned())));
        // A lone `$` names nothing: it is refused, as everywhere.
        assert!(read(&[("a.theme", "text {\n    plain \"$\"\n}\n")]).is_err());
    }

    #[test]
    fn a_duration_in_milliseconds_is_the_same_in_seconds() {
        let values = values("duration {\n    fast (ms)120\n    also (s)0.12\n}\n");
        assert_eq!(values.get("fast"), Some(&ThemeValue::Duration(0.12)));
        assert_eq!(values.get("fast"), values.get("also"));
    }

    #[test]
    fn a_value_s_place_is_the_last_theme_that_gives_it() {
        let values = read(&[
            ("base.theme", "length {\n    gap 4\n}\n"),
            ("project.theme", "\nlength {\n    gap 6\n}\n"),
        ])
        .expect("two themes");
        assert_eq!(values.get("gap"), Some(&ThemeValue::Length(6.0)));
        let (theme, offset) = values.place("gap").expect("given");
        assert_eq!((theme, offset), (1, "\nlength {\n    ".len()));
    }
}
