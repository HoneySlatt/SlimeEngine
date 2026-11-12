//! A style sheet read from KDL, after the rules that [`super`]'s documentation gives: its rules, each
//! a node whose children are `select` and the groups of a [`Style`], read by the code that
//! `uwui-macros`'s derives generate for each struct and enum of a style, and by the readers here of
//! each type of a leaf ([`Field`]), of shadows and of selectors
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`). Each error is placed
//! at its line and gathered, up to [`DocumentErrors::MOST`], the fields written rightly read still.

use std::borrow::Cow;
use std::collections::HashMap;

use slime_color::LinearColor;
use uwui_kdl::{Entry, Node, Value as Kdl};

pub(crate) use super::values::Refusal;
use super::values::{self, Group, ThemeValue, Values, number, shown, theme_name};
use super::{Ancestor, KindName, Rule, Selector, StateName, reads};
use crate::document::{DocumentError, DocumentErrors};
use crate::style::{Dimension, Shadow, Style, Track};

/// What a number of a style measures, as the derive marks its field: what a value written there,
/// or named, must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Measure {
    /// Logical pixels, bare or `(px)`; a theme's `length`.
    Length,
    /// Seconds, written `(ms)` or `(s)`, never negative; a theme's `duration`.
    Duration,
    /// A number without a unit; a theme's `number`.
    Number,
}

/// A document being read: the themes' values that it names, and the errors found, each placed in
/// its text.
pub(crate) struct Reading<'a> {
    pub(crate) values: &'a Values,
    name: &'a str,
    text: &'a str,
    errors: &'a mut DocumentErrors,
}

impl Reading<'_> {
    /// Keeps the error `fault`, placed at its line, up to the errors' cap.
    pub(crate) fn fail(&mut self, (offset, message): Refusal) {
        self.fail_with(offset, |_| message);
    }

    /// Keeps the error at the byte `offset` whose message `message` writes from the document's
    /// text, up to the errors' cap; past it, `message` is not called: a message that names the
    /// line of another place reads the text up to it, which a document of many errors would pay
    /// for each.
    fn fail_with(&mut self, offset: usize, message: impl FnOnce(&str) -> String) {
        let (name, text) = (self.name, self.text);
        self.errors
            .push_with(|| DocumentError::at(name, text, offset, message(text)));
    }
}

/// Where a field is written: a property, or a child node.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Item<'d, 't> {
    Property(Entry<'d, 't>),
    Node(Node<'d, 't>),
}

impl Item<'_, '_> {
    /// The byte where it starts.
    pub(crate) fn offset(&self) -> usize {
        match self {
            Self::Property(entry) => entry.span().start,
            Self::Node(node) => node.span().start,
        }
    }
}

/// A type of a style's leaf, as a document writes it: a value, as a property or an argument, or a
/// child node.
pub(crate) trait Field: Sized {
    /// Whether it is written as a child node, rather than a property.
    const NODE: bool = false;

    /// The value that `entry` writes, a property or an argument, of `measure` where it is a
    /// number; a type that is no number, or reads its own numbers, a size, does not read
    /// `measure`.
    fn from_entry(
        entry: &Entry<'_, '_>,
        measure: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        let _ = (measure, reading);
        Err((
            entry.span().start,
            "this is written as a child node".to_owned(),
        ))
    }

    /// The value that the child node `node` writes; none where it is wrong, its errors kept in
    /// `reading`.
    fn from_node(node: &Node<'_, '_>, measure: Measure, reading: &mut Reading<'_>) -> Option<Self> {
        let _ = measure;
        reading.fail((
            node.span().start,
            "this is written as a property".to_owned(),
        ));
        None
    }
}

/// Reads the rules of the style sheet `text`, named `name` in its errors, the themes' values that
/// they name from `values`, each error kept in `errors`.
pub(crate) fn read(
    name: &str,
    text: &str,
    values: &Values,
    errors: &mut DocumentErrors,
) -> Vec<Rule> {
    let document = match uwui_kdl::read(text) {
        Ok(document) => document,
        Err(error) => {
            errors.push(DocumentError {
                document: name.to_owned(),
                error,
            });
            return Vec::new();
        }
    };
    let mut reading = Reading {
        values,
        name,
        text,
        errors,
    };
    let mut rules = Vec::new();
    // Each rule's name, borrowed from the text where it holds no escape, and where it starts.
    let mut names: HashMap<Cow<'_, str>, usize> = HashMap::new();
    for node in document.nodes() {
        let rule = node.name().text();
        let start = node.span().start;
        if let Some(&first) = names.get(&rule) {
            reading.fail_with(start, |text| {
                format!(
                    "the rule `{rule}` is given twice, first at line {}",
                    line(text, first)
                )
            });
            continue;
        }
        names.insert(rule, start);
        let mut selector = None;
        for child in node
            .children()
            .filter(|child| child.name().text() == "select")
        {
            if let Some(&(_, first)) = selector.as_ref() {
                reading.fail_with(child.span().start, |text| {
                    format!(
                        "`select` is given twice in a rule, first at line {}",
                        line(text, first)
                    )
                });
            } else {
                selector = Some((read_selector(&child, &mut reading), child.span().start));
            }
        }
        let selector = selector.map(|(selector, _)| selector).unwrap_or_default();
        let mut declarations = Vec::new();
        let mut places = Vec::new();
        Style::read_fields(&node, &mut reading, &["select"], &mut |declaration, at| {
            declarations.push(declaration);
            places.push(at);
        });
        if let Some(kind) = selector.kind {
            for (declaration, &at) in declarations.iter().zip(&places) {
                if !reads(kind, declaration) {
                    reading.fail((
                        at,
                        format!(
                            "a rule for `{kind:?}` sets `{}`, which a `{kind:?}` does not read",
                            declaration.path()
                        ),
                    ));
                }
            }
        }
        rules.push(Rule {
            selector,
            declarations,
        });
    }
    rules
}

/// The line of the byte `offset` of `text`.
fn line(text: &str, offset: usize) -> u32 {
    uwui_kdl::Error::at(text, offset, "").line
}

/// Reads the fields that `node` writes, its properties and its children, each named among
/// `fields`, `(name, whether it is written as a child node)`, giving `read` each field's place in
/// `fields` and where it is written; the children named in `also` are left to the caller. A node's
/// type, its arguments, a field given twice, written in the wrong form, or of no name among them,
/// are refused.
pub(crate) fn fields<'d, 't>(
    node: &Node<'d, 't>,
    reading: &mut Reading<'_>,
    fields: &[(&str, bool)],
    also: &[&str],
    read: &mut dyn FnMut(usize, Item<'d, 't>, &mut Reading<'_>),
) {
    let at = node.span().start;
    let name = node.name().text();
    if node.annotation().is_some() {
        reading.fail((at, format!("`{name}` takes no type")));
    }
    // Where each field was first given, by its place among `fields`.
    let mut given: Vec<Option<usize>> = vec![None; fields.len()];
    let items = node
        .entries()
        .map(Item::Property)
        .chain(node.children().map(Item::Node));
    for item in items {
        let field = match &item {
            Item::Property(entry) => match entry.name() {
                Some(field) => field.text(),
                None => {
                    reading.fail((
                        entry.span().start,
                        format!(
                            "`{name}` holds its fields as properties, `field=value`, and child \
                             nodes, not arguments"
                        ),
                    ));
                    continue;
                }
            },
            Item::Node(child) => child.name().text(),
        };
        let offset = item.offset();
        if also.contains(&field.as_ref()) {
            // The caller reads these children; written as a property, it would read nothing.
            if let Item::Property(_) = item {
                reading.fail((
                    offset,
                    format!("`{field}` is written as a child node, `{field} …`, not a property"),
                ));
            }
            continue;
        }
        let Some(index) = fields.iter().position(|(known, _)| *known == field) else {
            let known: Vec<String> = also
                .iter()
                .chain(fields.iter().map(|(known, _)| known))
                .map(|known| format!("`{known}`"))
                .collect();
            reading.fail((
                offset,
                format!(
                    "`{field}` is none of what `{name}` holds: {}",
                    known.join(", ")
                ),
            ));
            continue;
        };
        if let Some(first) = given[index] {
            reading.fail_with(offset, |text| {
                format!(
                    "`{field}` is given twice in `{name}`, first at line {}",
                    line(text, first)
                )
            });
            continue;
        }
        given[index] = Some(offset);
        match (&item, fields[index].1) {
            (Item::Property(_), true) => reading.fail((
                offset,
                format!("`{field}` is written as a child node, `{field} …`, not a property"),
            )),
            (Item::Node(_), false) => reading.fail((
                offset,
                format!("`{field}` is written as a property, `{field}=…`, not a child node"),
            )),
            _ => read(index, item, reading),
        }
    }
}

/// Reads the leaf of the type `T` that `item` writes, of `measure`, giving `out` its value; keeps
/// its errors in `reading`.
pub(crate) fn leaf<T: Field>(
    item: Item<'_, '_>,
    measure: Measure,
    reading: &mut Reading<'_>,
    out: &mut dyn FnMut(T),
) {
    match item {
        Item::Property(entry) => match T::from_entry(&entry, measure, reading) {
            Ok(value) => out(value),
            Err(fault) => reading.fail(fault),
        },
        Item::Node(node) => {
            if let Some(value) = T::from_node(&node, measure, reading) {
                out(value);
            }
        }
    }
}

/// Reads the array of `N` elements of the type `T` that `item`, a child node, writes as its
/// arguments, each of `measure`; none where it is wrong, its errors kept in `reading`.
pub(crate) fn array<T: Field, const N: usize>(
    item: Item<'_, '_>,
    measure: Measure,
    reading: &mut Reading<'_>,
) -> Option<[T; N]> {
    let Item::Node(node) = item else {
        return None;
    };
    let elements: Vec<T> = arguments(&node, measure, reading)?;
    let count = elements.len();
    elements.try_into().ok().or_else(|| {
        reading.fail((
            node.span().start,
            format!("`{}` holds {N} values, not {count}", node.name().text()),
        ));
        None
    })
}

/// The values that `node` writes as its arguments, each of the type `T` and of `measure`; none where
/// it is wrong, its errors kept in `reading`. A node of arguments has no type, property or block.
fn arguments<T: Field>(
    node: &Node<'_, '_>,
    measure: Measure,
    reading: &mut Reading<'_>,
) -> Option<Vec<T>> {
    let name = node.name().text();
    let mut whole = true;
    if node.annotation().is_some() {
        reading.fail((node.span().start, format!("`{name}` takes no type")));
        whole = false;
    }
    if node.children().len() > 0 {
        reading.fail((
            node.span().start,
            format!("`{name}` holds its values as arguments, not in a block"),
        ));
        whole = false;
    }
    let mut values = Vec::with_capacity(node.entries().len());
    for entry in node.entries() {
        if entry.name().is_some() {
            reading.fail((
                entry.span().start,
                format!("`{name}` holds its values as arguments, not properties"),
            ));
            whole = false;
            continue;
        }
        match T::from_entry(&entry, measure, reading) {
            Ok(value) => values.push(value),
            Err(fault) => {
                reading.fail(fault);
                whole = false;
            }
        }
    }
    whole.then_some(values)
}

/// The theme's value named `name`, of `group`, for the entry at `at`.
fn named<'v>(
    reading: &'v Reading<'_>,
    name: &str,
    group: Group,
    at: usize,
) -> Result<&'v ThemeValue, Refusal> {
    match reading.values.get(name) {
        Some(value) if value.group() == group => Ok(value),
        Some(value) => Err((
            at,
            format!(
                "`${name}` is {}, where {} goes",
                value.group().a(),
                group.a()
            ),
        )),
        None => Err((at, format!("no theme gives a value named `{name}`"))),
    }
}

/// The value of a theme's `name` of a number's `group`, in an `f32`.
fn named_number(
    reading: &Reading<'_>,
    name: &str,
    group: Group,
    at: usize,
) -> Result<f32, Refusal> {
    match named(reading, name, group, at)? {
        ThemeValue::Length(number) | ThemeValue::Duration(number) | ThemeValue::Number(number) => {
            Ok(*number)
        }
        _ => unreachable!("a value of a number's group is a number"),
    }
}

impl Field for f32 {
    fn from_entry(
        entry: &Entry<'_, '_>,
        measure: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        let group = match measure {
            Measure::Length => Group::Length,
            Measure::Duration => Group::Duration,
            Measure::Number => Group::Number,
        };
        match theme_name(entry)? {
            Some(name) => named_number(reading, &name, group, entry.span().start),
            None => number(entry, group),
        }
    }
}

impl Field for u16 {
    fn from_entry(
        entry: &Entry<'_, '_>,
        _: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        let at = entry.span().start;
        let whole = |number: f64| {
            (number.fract() == 0.0 && (0.0..=f64::from(u16::MAX)).contains(&number))
                .then_some(number as u16)
        };
        if let Some(name) = theme_name(entry)? {
            let number = named_number(reading, &name, Group::Number, at)?;
            return whole(f64::from(number)).ok_or_else(|| {
                (
                    at,
                    format!(
                        "the value `{name}` is a number that is not whole from 0 to 65535, which \
                         this field holds"
                    ),
                )
            });
        }
        let written = match entry.value() {
            Kdl::Number(written) if entry.annotation().is_none() => whole(written.to_f64()),
            // What a number refuses, a unit or another value, is told as it tells it.
            _ => number(entry, Group::Number).map(|_| None)?,
        };
        written.ok_or_else(|| (at, "a whole number from 0 to 65535 goes here".to_owned()))
    }
}

impl Field for bool {
    fn from_entry(entry: &Entry<'_, '_>, _: Measure, _: &Reading<'_>) -> Result<Self, Refusal> {
        match entry.value() {
            Kdl::Bool(value) if entry.annotation().is_none() => Ok(value),
            _ => Err((
                entry.span().start,
                format!("`#true` or `#false` goes here, not {}", shown(entry)),
            )),
        }
    }
}

impl Field for String {
    fn from_entry(
        entry: &Entry<'_, '_>,
        _: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        let at = entry.span().start;
        if let Some(name) = theme_name(entry)? {
            return match named(reading, &name, Group::Text, at)? {
                ThemeValue::Text(text) => Ok(text.clone()),
                _ => unreachable!("a value of the group `text` is a text"),
            };
        }
        let Kdl::String(string) = entry.value() else {
            return Err((at, format!("a text goes here, not {}", shown(entry))));
        };
        if let Some(unit) = entry.annotation() {
            return Err((at, format!("a text takes no type `({})`", unit.text())));
        }
        let text = string.text();
        // `$$` writes a text that starts with `$`.
        Ok(text.strip_prefix('$').unwrap_or(&text).to_owned())
    }
}

impl Field for LinearColor {
    fn from_entry(
        entry: &Entry<'_, '_>,
        _: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        let at = entry.span().start;
        if let Some(name) = theme_name(entry)? {
            return match named(reading, &name, Group::Color, at)? {
                ThemeValue::Color(color) => Ok(*color),
                _ => unreachable!("a value of the group `color` is a color"),
            };
        }
        let Kdl::String(string) = entry.value() else {
            return Err((at, format!("a color goes here, not {}", shown(entry))));
        };
        if let Some(unit) = entry.annotation() {
            return Err((at, format!("a color takes no type `({})`", unit.text())));
        }
        values::color(&string.text(), reading.values).map_err(|message| (at, message))
    }
}

/// The length that `entry` names or writes where a size or a track goes, `$name` or a number bare
/// or `(px)`; else the number that it writes in `unit`, `%` or `fr`, which the caller reads.
///
/// # Errors
///
/// What `what`, the size or the track, is not: another unit, or what is not a number.
fn length_or(
    entry: &Entry<'_, '_>,
    reading: &Reading<'_>,
    unit: &str,
    what: &str,
) -> Result<Result<f32, f64>, Refusal> {
    let at = entry.span().start;
    if let Some(name) = theme_name(entry)? {
        return named_number(reading, &name, Group::Length, at).map(Ok);
    }
    let Kdl::Number(written) = entry.value() else {
        return Err((at, format!("{what}, not {}", shown(entry))));
    };
    match entry.annotation().map(|annotation| annotation.text()) {
        None => number(entry, Group::Length).map(Ok),
        Some(written_unit) if written_unit == "px" => number(entry, Group::Length).map(Ok),
        Some(written_unit) if written_unit == unit => {
            let value = written.to_f64();
            if !value.is_finite() || !(value as f32).is_finite() {
                return Err((
                    at,
                    "a number that is not finite, or past what an `f32` holds".to_owned(),
                ));
            }
            Ok(Err(value))
        }
        Some(other) => Err((at, format!("{what}, not `({other})`"))),
    }
}

/// Whether `entry` writes `Auto`, as a size or a track does.
fn auto(entry: &Entry<'_, '_>) -> bool {
    matches!(entry.value(), Kdl::String(string) if string.text() == "Auto")
        && entry.annotation().is_none()
}

impl Field for Dimension {
    fn from_entry(
        entry: &Entry<'_, '_>,
        _: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        if auto(entry) {
            return Ok(Self::Auto);
        }
        let what = "a size is `Auto`, a length, or a share of its parent's, `(%)50`";
        Ok(match length_or(entry, reading, "%", what)? {
            Ok(pixels) => Self::Pixels(pixels),
            // A share divided in `f64` and rounded once: `(%)0.7` is the `f32` nearest 0.007, as
            // a share written `0.007` is.
            Err(percent) => Self::Share((percent / 100.0) as f32),
        })
    }
}

impl Field for Track {
    fn from_entry(
        entry: &Entry<'_, '_>,
        _: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        if auto(entry) {
            return Ok(Self::Auto);
        }
        let what = "a track is `Auto`, a length, or a share of the space left, `(fr)1`";
        Ok(match length_or(entry, reading, "fr", what)? {
            Ok(pixels) => Self::Pixels(pixels),
            Err(fraction) => Self::Fraction(fraction as f32),
        })
    }
}

/// An option left out is left as it is; `#null` empties it, `align_self=#null`, `shadow #null`.
impl<T: Field> Field for Option<T> {
    const NODE: bool = T::NODE;

    fn from_entry(
        entry: &Entry<'_, '_>,
        measure: Measure,
        reading: &Reading<'_>,
    ) -> Result<Self, Refusal> {
        if matches!(entry.value(), Kdl::Null) && entry.annotation().is_none() {
            return Ok(None);
        }
        T::from_entry(entry, measure, reading).map(Some)
    }

    fn from_node(node: &Node<'_, '_>, measure: Measure, reading: &mut Reading<'_>) -> Option<Self> {
        let null =
            |entry: &Entry<'_, '_>| entry.name().is_none() && matches!(entry.value(), Kdl::Null);
        if node.entries().any(|entry| null(&entry)) {
            let alone = node.entries().len() == 1
                && node.entries().all(|entry| entry.annotation().is_none())
                && node.children().len() == 0
                && node.annotation().is_none();
            if alone {
                return Some(None);
            }
            reading.fail((
                node.span().start,
                format!(
                    "`{} #null` empties it, and holds nothing else",
                    node.name().text()
                ),
            ));
            return None;
        }
        T::from_node(node, measure, reading).map(Some)
    }
}

/// A list of values is a child node whose arguments are its elements, `fonts Geist "Geist Mono"`.
impl<T: Field> Field for Vec<T> {
    const NODE: bool = true;

    fn from_node(node: &Node<'_, '_>, measure: Measure, reading: &mut Reading<'_>) -> Option<Self> {
        arguments(node, measure, reading)
    }
}

/// A shadow is a child node, set whole: what it leaves out takes [`Shadow::default`]'s value,
/// `shadow color=$shadow blur=8 { offset 0 4 }`.
impl Field for Shadow {
    const NODE: bool = true;

    fn from_node(node: &Node<'_, '_>, _: Measure, reading: &mut Reading<'_>) -> Option<Self> {
        const FIELDS: &[(&str, bool)] = &[
            ("color", false),
            ("offset", true),
            ("blur", false),
            ("spread", false),
        ];
        // A field written wrongly is kept in `reading`, which refuses the sheet: the shadow
        // read of the others is not used.
        let mut shadow = Self::default();
        fields(
            node,
            reading,
            FIELDS,
            &[],
            &mut |index, item, reading| match index {
                0 => leaf(item, Measure::Length, reading, &mut |color| {
                    shadow.color = color
                }),
                1 => {
                    if let Some(offset) = array::<f32, 2>(item, Measure::Length, reading) {
                        shadow.offset = offset;
                    }
                }
                2 => leaf(item, Measure::Length, reading, &mut |blur| {
                    shadow.blur = blur
                }),
                _ => leaf(item, Measure::Length, reading, &mut |spread| {
                    shadow.spread = spread
                }),
            },
        );
        Some(shadow)
    }
}

/// The variant of `variants`, `(name, variant)`, that `entry` names by its identifier.
pub(crate) fn identifier<T: Copy>(
    entry: &Entry<'_, '_>,
    variants: &[(&str, T)],
) -> Result<T, Refusal> {
    let at = entry.span().start;
    let names = || {
        variants
            .iter()
            .map(|(name, _)| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let Kdl::String(string) = entry.value() else {
        return Err((
            at,
            format!("one of {} goes here, not {}", names(), shown(entry)),
        ));
    };
    if entry.annotation().is_some() {
        return Err((
            at,
            format!("one of {} goes here, which takes no type", names()),
        ));
    }
    let text = string.text();
    variants
        .iter()
        .find(|(name, _)| *name == text)
        .map(|&(_, variant)| variant)
        .ok_or_else(|| (at, format!("`{text}` is not one of {}", names())))
}

/// The name of a class or a key that `entry` writes in a selector. A theme's value is named where a
/// value goes, never in a name: a name that starts with `$` is refused, without an escape, so that
/// a name written as a value is written, `classes $panel`, is not taken as a class that selects
/// nothing.
fn selector_text(entry: &Entry<'_, '_>) -> Result<String, Refusal> {
    let at = entry.span().start;
    let Kdl::String(string) = entry.value() else {
        return Err((at, format!("a name goes here, not {}", shown(entry))));
    };
    if entry.annotation().is_some() {
        return Err((at, "a name takes no type".to_owned()));
    }
    let text = string.text();
    if text.starts_with('$') {
        return Err((
            at,
            "a name never starts with `$`: a theme's value is named where a value goes, never in a \
             name"
                .to_owned(),
        ));
    }
    Ok(text.into_owned())
}

/// The selector that `node`, a rule's `select`, writes: `kind=Button key=tabs`, `classes a b`,
/// `states Hovered`, `inside kind=Menu class=toolbar`. A numbered key is a whole number, `key=3`.
fn read_selector(node: &Node<'_, '_>, reading: &mut Reading<'_>) -> Selector {
    const FIELDS: &[(&str, bool)] = &[
        ("kind", false),
        ("classes", true),
        ("key", false),
        ("states", true),
        ("inside", true),
    ];
    let mut selector = Selector::default();
    fields(
        node,
        reading,
        FIELDS,
        &[],
        &mut |index, item, reading| match (index, item) {
            (0, Item::Property(entry)) => {
                match KindName::from_entry(&entry, Measure::Length, reading) {
                    Ok(kind) => selector.kind = Some(kind),
                    Err(fault) => reading.fail(fault),
                }
            }
            (1, Item::Node(node)) => {
                let mut classes = Vec::new();
                if let Some(entries) = selector_arguments(&node, reading) {
                    for entry in entries {
                        match selector_text(&entry) {
                            Ok(class) => classes.push(class),
                            Err(fault) => reading.fail(fault),
                        }
                    }
                }
                selector.classes = classes;
            }
            (2, Item::Property(entry)) => match key(&entry) {
                Ok(key) => selector.key = Some(key),
                Err(fault) => reading.fail(fault),
            },
            (3, Item::Node(node)) => {
                if let Some(states) = arguments::<StateName>(&node, Measure::Length, reading) {
                    selector.states = states;
                }
            }
            (_, Item::Node(node)) => selector.inside = Some(read_ancestor(&node, reading)),
            (_, Item::Property(_)) => unreachable!("`fields` gives each field in its form"),
        },
    );
    selector
}

/// The entries of `node`, a list of names in a selector, checked as a list of values is.
fn selector_arguments<'d, 't>(
    node: &Node<'d, 't>,
    reading: &mut Reading<'_>,
) -> Option<Vec<Entry<'d, 't>>> {
    let name = node.name().text();
    if node.annotation().is_some() || node.children().len() > 0 {
        reading.fail((
            node.span().start,
            format!("`{name}` holds its names as arguments alone"),
        ));
        return None;
    }
    let mut entries = Vec::new();
    for entry in node.entries() {
        if entry.name().is_some() {
            reading.fail((
                entry.span().start,
                format!("`{name}` holds its names as arguments, not properties"),
            ));
            return None;
        }
        entries.push(entry);
    }
    Some(entries)
}

/// A selector's key as the cascade names it: a name as it is, a number after `#`; a numbered key
/// is written as a whole number, `key=3`, and a name never starts with `#`.
fn key(entry: &Entry<'_, '_>) -> Result<String, Refusal> {
    let at = entry.span().start;
    if let Kdl::Number(number) = entry.value() {
        return match (number.to_i64(), entry.annotation()) {
            (Some(index), None) if index >= 0 => Ok(format!("#{index}")),
            _ => Err((
                at,
                "a numbered key is a whole number from 0, `key=3`".to_owned(),
            )),
        };
    }
    let key = selector_text(entry)?;
    if key.starts_with('#') {
        return Err((
            at,
            "a key named by its number is written as a number, `key=3`".to_owned(),
        ));
    }
    Ok(key)
}

/// The ancestor that `node`, a selector's `inside`, asks for: `inside kind=Menu class=toolbar`.
fn read_ancestor(node: &Node<'_, '_>, reading: &mut Reading<'_>) -> Ancestor {
    const FIELDS: &[(&str, bool)] = &[("kind", false), ("class", false)];
    let mut ancestor = Ancestor::default();
    fields(node, reading, FIELDS, &[], &mut |index, item, reading| {
        let Item::Property(entry) = item else {
            return;
        };
        let read = if index == 0 {
            KindName::from_entry(&entry, Measure::Length, reading)
                .map(|kind| ancestor.kind = Some(kind))
        } else {
            selector_text(&entry).map(|class| ancestor.class = Some(class))
        };
        if let Err(fault) = read {
            reading.fail(fault);
        }
    });
    ancestor
}
