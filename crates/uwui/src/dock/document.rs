//! A dock's layout as a document of KDL, written and read by hand
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).

use std::fmt;

use uwui_kdl::{Limits, Node as KdlNode, Value};

use super::{Floating, Layout, Node, Share};
use crate::document::{DocumentError, DocumentErrors, Fault};

/// The version of the format of a layout's document that this crate writes and reads; a document
/// of a later one is not read ([`ReadError::Later`]).
pub const VERSION: i64 = 1;

/// How strictly [`read`] takes what a document holds that it does not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    /// What it does not know, a node, a property or an argument, is left out with a warning: a
    /// layout that a program saved, which a later program of the same version may have written.
    Lenient,
    /// What it does not know is an error: a default written by hand, whose mistakes its author
    /// must see.
    Strict,
}

/// A layout read from its document.
#[derive(Debug)]
pub struct Read {
    /// The layout as the document writes it, not yet normalized: a [`Dock`](super::Dock)
    /// normalizes the layout it shows, and [`Layout::fit`] does.
    pub layout: Layout,
    /// The name and version of the default that it was made from, `editor 1`, if it names one.
    pub default: Option<String>,
    /// What it held that the reader does not know, left out, read leniently; none when it held
    /// nothing so.
    pub warnings: Option<DocumentErrors>,
}

/// Why a document gives no layout.
#[derive(Debug)]
pub enum ReadError {
    /// It writes a later version of the format than [`VERSION`]: it is not read, so that what a
    /// later program wrote is neither misread nor lost.
    Later {
        /// The version that it writes.
        version: i64,
    },
    /// It is not a layout of this version: each error found, at its line.
    Unreadable(DocumentErrors),
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Later { version } => write!(
                formatter,
                "the layout is of the format's version {version}, later than {VERSION}, the last \
                 that this program reads"
            ),
            Self::Unreadable(errors) => errors.fmt(formatter),
        }
    }
}

impl std::error::Error for ReadError {}

/// Why a layout cannot be written as a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    /// Its splits lie within each other deeper than a document is read
    /// ([`Limits::DEFAULT`]), some 31 levels.
    TooDeep,
    /// A panel's key, or the default's name, starts with `$`, which names a theme's value in uwui's
    /// documents.
    Dollar(String),
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooDeep => formatter.write_str(
                "the layout's splits lie within each other deeper than a document is read",
            ),
            Self::Dollar(name) => write!(
                formatter,
                "`{name}` starts with `$`, which names a theme's value: no panel nor default is \
                 named so"
            ),
        }
    }
}

impl std::error::Error for WriteError {}

/// `layout` as a document, made from the default `default` if it names one: `version 1`, then
/// `default "editor 1"`, then its docked panels, `Split` or `Tabs`, then each floating panel, then
/// the panels closed, if any.
///
/// ```kdl
/// version 1
/// default "editor 1"
/// Split side_by_side=#true {
///     children share=0.25 { Tabs chosen=0 { panels hierarchy } }
///     children share=0.75 { Tabs chosen=0 { panels viewport } }
/// }
/// floating panel=history { origin 300 200; size 320 400 }
/// closed history
/// ```
///
/// A share, a place or a size reads back to the same `f32`, but one that is not a number, which
/// reads back as one, its bits not kept.
///
/// # Errors
///
/// When the layout's splits lie within each other deeper than a document is read, or a panel's key
/// or `default` starts with `$`.
pub fn write(layout: &Layout, default: Option<&str>) -> Result<String, WriteError> {
    let mut text = String::new();
    text.push_str("version ");
    uwui_kdl::write_integer(&mut text, VERSION);
    text.push('\n');
    if let Some(default) = default {
        text.push_str("default ");
        write_name(&mut text, default)?;
        text.push('\n');
    }
    write_node(&mut text, &layout.root, 0)?;
    text.push('\n');
    for floating in &layout.floating {
        text.push_str("floating panel=");
        write_name(&mut text, &floating.panel)?;
        text.push_str(" { origin ");
        write_pair(&mut text, floating.origin);
        text.push_str("; size ");
        write_pair(&mut text, floating.size);
        text.push_str(" }\n");
    }
    if !layout.closed.is_empty() {
        text.push_str("closed");
        for panel in &layout.closed {
            text.push(' ');
            write_name(&mut text, panel)?;
        }
        text.push('\n');
    }
    Ok(text)
}

/// The spaces of one level of indentation.
const INDENTATION: &str = "    ";

/// Writes `node` at `depth` in the document, without the newline after it: its children indented
/// below it, and a stack on one line. A split's children are its own; its depth bounds them, the document's limit of depth too.
fn write_node(text: &mut String, node: &Node, depth: usize) -> Result<(), WriteError> {
    // A stack lies at `depth`, its node of panels one below.
    if depth + 1 > Limits::DEFAULT.depth {
        return Err(WriteError::TooDeep);
    }
    match node {
        Node::Tabs { panels, chosen } => {
            text.push_str("Tabs chosen=");
            uwui_kdl::write_integer(text, i64::from(*chosen));
            text.push_str(" { panels");
            for panel in panels {
                text.push(' ');
                write_name(text, panel)?;
            }
            text.push_str(" }");
        }
        Node::Split {
            side_by_side,
            children,
        } => {
            text.push_str("Split side_by_side=");
            text.push_str(if *side_by_side { "#true" } else { "#false" });
            text.push_str(" {\n");
            for child in children {
                indent(text, depth + 1);
                text.push_str("children share=");
                uwui_kdl::write_f32(text, child.share);
                if let Node::Tabs { .. } = child.node {
                    text.push_str(" { ");
                    write_node(text, &child.node, depth + 2)?;
                    text.push_str(" }\n");
                } else {
                    text.push_str(" {\n");
                    indent(text, depth + 2);
                    write_node(text, &child.node, depth + 2)?;
                    text.push('\n');
                    indent(text, depth + 1);
                    text.push_str("}\n");
                }
            }
            indent(text, depth);
            text.push('}');
        }
    }
    Ok(())
}

/// `depth` levels of indentation.
fn indent(text: &mut String, depth: usize) {
    for _ in 0..depth {
        text.push_str(INDENTATION);
    }
}

/// Writes a panel's key or a default's name, refused if it starts with `$`.
fn write_name(text: &mut String, name: &str) -> Result<(), WriteError> {
    if name.starts_with('$') {
        return Err(WriteError::Dollar(name.to_owned()));
    }
    uwui_kdl::write_string(text, name);
    Ok(())
}

/// Writes two numbers, a place's or a size's, apart by a space.
fn write_pair(text: &mut String, pair: [f32; 2]) {
    uwui_kdl::write_f32(text, pair[0]);
    text.push(' ');
    uwui_kdl::write_f32(text, pair[1]);
}

/// The layout that `text`, a layout's document named `name` in its errors, holds, as [`write()`]
/// writes it, read as `reading` says.
///
/// The document starts with its version, `version 1`: a later one is not read
/// ([`ReadError::Later`]). Then, in any order, `default`, once; the docked panels, `Split` or
/// `Tabs`, once; `floating` for each floating panel; and `closed`, once. A split's children are
/// nodes `children`, each with its `share` and one child, a split or a stack; a stack's `panels`
/// are the arguments of its node `panels`, and `chosen` the place of the one shown. A node, a
/// property or an argument that it does not know is left out with a warning, read leniently, and an
/// error, read strictly. Where a field means one value, a value of the wrong kind, a field given
/// twice, an annotation, which has no meaning yet, and a panel or a default named with a `$` at its
/// start, which names a theme's value, are errors, as a missing field is, but `default`,
/// `floating` and `closed`, which a layout may lack. A share, a place and a size that are no finite
/// number are read as they are, which [`Layout::normalize`] mends.
///
/// The reader walks the tree of splits by recursion, bounded by the document's limit of depth.
///
/// # Errors
///
/// [`ReadError::Later`], for a later version; [`ReadError::Unreadable`], with each error found, up
/// to [`DocumentErrors::MOST`], at its line, for a text that is not KDL or not a layout's document
/// of this version.
pub fn read(name: &str, text: &str, reading: Reading) -> Result<Read, ReadError> {
    let document = uwui_kdl::read(text).map_err(|error| {
        ReadError::Unreadable(
            DocumentError {
                document: name.to_owned(),
                error: Fault::Kdl(error),
            }
            .into(),
        )
    })?;
    let mut reader = Reader {
        name,
        text,
        reading,
        errors: DocumentErrors::new(),
        warnings: DocumentErrors::new(),
    };
    let mut nodes = document.nodes();
    match nodes.next() {
        Some(node) if node.name().text() == "version" => {
            if let Some(version) = reader.version(node)
                && version > VERSION
            {
                return Err(ReadError::Later { version });
            }
        }
        Some(node) => reader.fail(
            node.span().start,
            "a layout starts with its version, `version 1`",
        ),
        None => reader.fail(0, "a layout starts with its version, `version 1`"),
    }
    let mut default = Once::default();
    let mut root = Once::default();
    let mut closed = Once::default();
    let mut floating = Vec::new();
    for node in nodes {
        let at = node.span().start;
        match &*node.name().text() {
            "version" => reader.fail(at, "`version` is given twice"),
            "default" => {
                let read = reader.default(node);
                default.set(&mut reader, at, "default", read);
            }
            "Split" | "Tabs" => {
                let read = reader.tree(node);
                root.set(
                    &mut reader,
                    at,
                    "the docked panels, `Split` or `Tabs`,",
                    read,
                );
            }
            "floating" => floating.extend(reader.floating(node)),
            "closed" => {
                let read = reader.closed(node);
                closed.set(&mut reader, at, "closed", read);
            }
            other => reader.unknown(
                at,
                format!(
                    "`{other}` is not a node of a layout: `version`, `default`, `Split`, `Tabs`, \
                     `floating` or `closed`"
                ),
            ),
        }
    }
    let root = root.value;
    if root.is_none() && reader.errors.is_empty() {
        reader.fail(
            text.len(),
            "the layout lacks its docked panels, a node `Split` or `Tabs`",
        );
    }
    let Some(root) = root.filter(|_| reader.errors.is_empty()) else {
        return Err(ReadError::Unreadable(reader.errors));
    };
    Ok(Read {
        layout: Layout {
            root,
            floating,
            closed: closed.value.unwrap_or_default(),
        },
        default: default.value,
        warnings: (!reader.warnings.is_empty()).then_some(reader.warnings),
    })
}

/// What reads a layout's document: its errors and warnings, gathered.
struct Reader<'a> {
    name: &'a str,
    text: &'a str,
    reading: Reading,
    errors: DocumentErrors,
    warnings: DocumentErrors,
}

/// A field given once: a second is an error.
struct Once<T> {
    value: Option<T>,
    given: bool,
}

impl<T> Default for Once<T> {
    fn default() -> Self {
        Self {
            value: None,
            given: false,
        }
    }
}

impl<T> Once<T> {
    /// Takes `read`, the field `what` at the byte `at`, an error if it was given before.
    fn set(&mut self, reader: &mut Reader<'_>, at: usize, what: &str, read: Option<T>) {
        if std::mem::replace(&mut self.given, true) {
            reader.fail(at, format!("{what} is given twice"));
        } else {
            self.value = read;
        }
    }
}

/// The entries of a node, sorted by what the reader knows of them, each value with the byte where
/// its entry starts, which an error about it names.
struct Entries<'d, 't> {
    /// Its arguments, in their order.
    arguments: Vec<(usize, Value<'d, 't>)>,
    /// Its properties that the reader knows, by their places in the names asked for.
    properties: Vec<Option<(usize, Value<'d, 't>)>>,
}

impl<'a> Reader<'a> {
    /// The error `message` at the byte `at`.
    fn fail(&mut self, at: usize, message: impl Into<String>) {
        let (name, text) = (self.name, self.text);
        self.errors
            .push_with(|| DocumentError::at(name, text, at, message));
    }

    /// What the reader does not know, at the byte `at`: a warning, read leniently, and an error,
    /// strictly.
    fn unknown(&mut self, at: usize, message: impl Into<String>) {
        let (name, text) = (self.name, self.text);
        let found = || DocumentError::at(name, text, at, message);
        match self.reading {
            Reading::Lenient => self.warnings.push_with(found),
            Reading::Strict => self.errors.push_with(found),
        }
    }

    /// `node`'s entries: its arguments, and its properties among `known`, the others unknown; an
    /// annotation on it or on an entry, and a property of `known` given twice, are errors. Its
    /// children are left to the caller.
    fn entries<'d, 't>(&mut self, node: KdlNode<'d, 't>, known: &[&str]) -> Entries<'d, 't> {
        if let Some(annotation) = node.annotation() {
            self.fail(
                annotation.span().start,
                "an annotation has no meaning in a layout",
            );
        }
        let mut entries = Entries {
            arguments: Vec::new(),
            properties: vec![None; known.len()],
        };
        for entry in node.entries() {
            let at = entry.span().start;
            if entry.annotation().is_some() {
                self.fail(at, "an annotation has no meaning in a layout");
            }
            let Some(name) = entry.name() else {
                entries.arguments.push((at, entry.value()));
                continue;
            };
            let name = name.text();
            match known.iter().position(|each| *each == name) {
                Some(place) if entries.properties[place].is_some() => {
                    self.fail(at, format!("`{name}` is given twice"));
                }
                Some(place) => entries.properties[place] = Some((at, entry.value())),
                None => self.unknown(
                    at,
                    format!("`{name}` is not a property of `{}`", node.name().text()),
                ),
            }
        }
        entries
    }

    /// The arguments of `node` past the first `count`, unknown.
    fn extra_arguments(&mut self, node: KdlNode<'_, '_>, count: usize) {
        if let Some(extra) = node.arguments().nth(count) {
            self.unknown(
                extra.span().start,
                format!(
                    "`{}` takes {count} argument{}",
                    node.name().text(),
                    if count == 1 { "" } else { "s" }
                ),
            );
        }
    }

    /// Each child of `node`, unknown: for a node that takes none.
    fn no_children(&mut self, node: KdlNode<'_, '_>) {
        for child in node.children() {
            self.unknown(
                child.span().start,
                format!(
                    "`{}` is not a child of `{}`",
                    child.name().text(),
                    node.name().text()
                ),
            );
        }
    }

    /// The version that `node`, `version 1`, writes; none, with an error, for a node that writes
    /// none, or one that is not a whole number from 1.
    fn version(&mut self, node: KdlNode<'_, '_>) -> Option<i64> {
        let entries = self.entries(node, &[]);
        self.extra_arguments(node, 1);
        self.no_children(node);
        let (at, version) = match entries.arguments.first() {
            Some((at, Value::Number(number))) => {
                (*at, number.to_i64().filter(|version| *version >= 1))
            }
            Some((at, _)) => (*at, None),
            None => (node.span().start, None),
        };
        if version.is_none() {
            self.fail(
                at,
                "a layout's version is a whole number from 1, `version 1`",
            );
        }
        version
    }

    /// The name of the default that `node`, `default "editor 1"`, names.
    fn default(&mut self, node: KdlNode<'_, '_>) -> Option<String> {
        let entries = self.entries(node, &[]);
        self.extra_arguments(node, 1);
        self.no_children(node);
        let Some((at, value)) = entries.arguments.first() else {
            self.fail(
                node.span().start,
                "`default` names the default that the layout was made from, `default \"editor 1\"`",
            );
            return None;
        };
        self.name(*at, value)
    }

    /// `value`, a panel's key or a default's name, at the byte `at`: a string that does not start
    /// with `$`.
    fn name(&mut self, at: usize, value: &Value<'_, '_>) -> Option<String> {
        let Value::String(string) = value else {
            self.fail(at, format!("`{value}` is not a name: a string"));
            return None;
        };
        let name = string.text();
        if name.starts_with('$') {
            self.fail(
                string.span().start,
                format!("`{name}` starts with `$`, which names a theme's value, never a panel"),
            );
            return None;
        }
        Some(name.into_owned())
    }

    /// The names that `node`'s arguments give, each a panel's key; none, with an error, where one
    /// is not a name.
    fn names(&mut self, node: KdlNode<'_, '_>) -> Option<Vec<String>> {
        let entries = self.entries(node, &[]);
        self.no_children(node);
        let names: Vec<Option<String>> = entries
            .arguments
            .iter()
            .map(|(at, value)| self.name(*at, value))
            .collect();
        names.into_iter().collect()
    }

    /// The panels closed that `node`, `closed history console`, names.
    fn closed(&mut self, node: KdlNode<'_, '_>) -> Option<Vec<String>> {
        self.names(node)
    }

    /// The part of the docked panels that `node`, `Split` or `Tabs`, writes.
    fn tree(&mut self, node: KdlNode<'_, '_>) -> Option<Node> {
        if node.name().text() == "Tabs" {
            self.tabs(node)
        } else {
            self.split(node)
        }
    }

    /// The stack that `node`, `Tabs chosen=0 { panels hierarchy console }`, writes.
    fn tabs(&mut self, node: KdlNode<'_, '_>) -> Option<Node> {
        let entries = self.entries(node, &["chosen"]);
        self.extra_arguments(node, 0);
        let at = node.span().start;
        let chosen = match &entries.properties[0] {
            Some((_, Value::Number(number))) => {
                let chosen = number
                    .to_i64()
                    .and_then(|chosen| u32::try_from(chosen).ok());
                if chosen.is_none() {
                    self.fail(
                        number.span().start,
                        "`chosen` is the place of the panel shown, a whole number from 0",
                    );
                }
                chosen
            }
            Some((given, other)) => {
                self.fail(
                    *given,
                    format!("`chosen` is the place of the panel shown, not `{other}`"),
                );
                None
            }
            None => {
                self.fail(at, "`Tabs` lacks `chosen`, the place of the panel shown");
                None
            }
        };
        let mut panels = Once::default();
        for child in node.children() {
            let child_at = child.span().start;
            if child.name().text() == "panels" {
                let read = self.names(child);
                panels.set(self, child_at, "`panels`", read);
            } else {
                self.unknown(
                    child_at,
                    format!(
                        "`{}` is not a child of `Tabs`: `panels`",
                        child.name().text()
                    ),
                );
            }
        }
        if !panels.given {
            self.fail(at, "`Tabs` lacks `panels`, its panels' keys");
        }
        Some(Node::Tabs {
            panels: panels.value?,
            chosen: chosen?,
        })
    }

    /// The split that `node`, `Split side_by_side=#true { children share=0.5 { … } … }`, writes.
    fn split(&mut self, node: KdlNode<'_, '_>) -> Option<Node> {
        let entries = self.entries(node, &["side_by_side"]);
        self.extra_arguments(node, 0);
        let at = node.span().start;
        let side_by_side = match &entries.properties[0] {
            Some((_, Value::Bool(side_by_side))) => Some(*side_by_side),
            Some((given, other)) => {
                self.fail(
                    *given,
                    format!("`side_by_side` is `#true` or `#false`, not `{other}`"),
                );
                None
            }
            None => {
                self.fail(
                    at,
                    "`Split` lacks `side_by_side`, whether its children lie side by side",
                );
                None
            }
        };
        let mut children = Vec::new();
        let mut whole = true;
        for child in node.children() {
            if child.name().text() == "children" {
                let read = self.share(child);
                whole &= read.is_some();
                children.extend(read);
            } else {
                self.unknown(
                    child.span().start,
                    format!(
                        "`{}` is not a child of `Split`: `children`",
                        child.name().text()
                    ),
                );
            }
        }
        Some(Node::Split {
            side_by_side: side_by_side?,
            children: whole.then_some(children)?,
        })
    }

    /// The child of a split that `node`, `children share=0.5 { Tabs … }`, writes.
    fn share(&mut self, node: KdlNode<'_, '_>) -> Option<Share> {
        let entries = self.entries(node, &["share"]);
        self.extra_arguments(node, 0);
        let at = node.span().start;
        let share = match &entries.properties[0] {
            Some((_, Value::Number(number))) => Some(number.to_f32()),
            Some((given, other)) => {
                self.fail(*given, format!("`share` is a number, not `{other}`"));
                None
            }
            None => {
                self.fail(
                    at,
                    "`children` lacks `share`, its share of the split's length",
                );
                None
            }
        };
        let mut part = Once::default();
        for child in node.children() {
            let child_at = child.span().start;
            match &*child.name().text() {
                "Split" | "Tabs" => {
                    let read = self.tree(child);
                    part.set(self, child_at, "the part, `Split` or `Tabs`,", read);
                }
                other => self.unknown(
                    child_at,
                    format!("`{other}` is not a child of `children`: `Split` or `Tabs`"),
                ),
            }
        }
        if !part.given {
            self.fail(at, "`children` lacks its part, a node `Split` or `Tabs`");
        }
        Some(Share {
            share: share?,
            node: part.value?,
        })
    }

    /// The floating panel that `node`, `floating panel=history { origin 300 200; size 320 400 }`,
    /// writes.
    fn floating(&mut self, node: KdlNode<'_, '_>) -> Option<Floating> {
        let entries = self.entries(node, &["panel"]);
        self.extra_arguments(node, 0);
        let at = node.span().start;
        let panel = match &entries.properties[0] {
            Some((given, value)) => self.name(*given, value),
            None => {
                self.fail(at, "`floating` lacks `panel`, its panel's key");
                None
            }
        };
        let (mut origin, mut size) = (Once::default(), Once::default());
        for child in node.children() {
            let child_at = child.span().start;
            match &*child.name().text() {
                "origin" => {
                    let read = self.pair(child);
                    origin.set(self, child_at, "`origin`", read);
                }
                "size" => {
                    let read = self.pair(child);
                    size.set(self, child_at, "`size`", read);
                }
                other => self.unknown(
                    child_at,
                    format!("`{other}` is not a child of `floating`: `origin` or `size`"),
                ),
            }
        }
        for (given, what) in [(origin.given, "origin"), (size.given, "size")] {
            if !given {
                self.fail(at, format!("`floating` lacks `{what}`"));
            }
        }
        Some(Floating {
            panel: panel?,
            origin: origin.value?,
            size: size.value?,
        })
    }

    /// The two numbers that `node`, `origin 300 200`, gives.
    fn pair(&mut self, node: KdlNode<'_, '_>) -> Option<[f32; 2]> {
        let entries = self.entries(node, &[]);
        self.extra_arguments(node, 2);
        self.no_children(node);
        let number = |value: Option<&(usize, Value<'_, '_>)>| match value {
            Some((_, Value::Number(number))) => Some(number.to_f32()),
            _ => None,
        };
        let pair = number(entries.arguments.first()).zip(number(entries.arguments.get(1)));
        if pair.is_none() {
            // At the first argument that is no number, or at the node that lacks one.
            let at = entries
                .arguments
                .iter()
                .find(|(_, value)| !matches!(value, Value::Number(_)))
                .map_or(node.span().start, |(at, _)| *at);
            self.fail(
                at,
                format!(
                    "`{}` is two numbers, `{} 300 200`",
                    node.name().text(),
                    node.name().text()
                ),
            );
        }
        pair.map(|(x, y)| [x, y])
    }
}
