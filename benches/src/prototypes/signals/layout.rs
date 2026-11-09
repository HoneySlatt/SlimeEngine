//! Layouts: an interface's elements written in a file of KDL 2, apart from the view's code, which
//! binds each named element by its name and fills each slot with a view (M6d.1d2a). A changed
//! file read while the application runs shows at the next frame with the state kept: the elements
//! are matched with the tree's by their names ([`Signals::relayout`]).
//!
//! The format: one `layout <name> { <its root element> }` node at the top for each layout.
//!
//! ```kdl
//! layout cover {
//!     pressable cover class=cover {
//!         image art
//!         label title
//!     }
//! }
//! ```
//!
//! - An element is a node whose name is its kind: `column`, `row`, `stack` and `pressable` hold
//!   the elements of their block; `label`, `button`, `image`, `search-field`, `grid` and `list`
//!   hold none; a `slot` is the place of a view that the code makes. A layout's root is not a slot.
//! - Its first argument, a string, is its name: optional, but for a slot; unique in its layout.
//! - Its properties, each a string: `class`, its classes, apart by spaces; `text`, a label's or a
//!   button's; `placeholder`, a search field's; `action`, the command that a press on a button or
//!   a pressable runs, as a shortcut that names it does.
//! - Elements nest [`DEPTH`] deep at most.
//!
//! In code, [`layout`] makes a layout's elements, [`Laid::bind`] gives a named one the methods
//! that code gives an element it makes, [`Laid::slot`] fills a slot with a dynamic view, and
//! [`Laid::el`] gives the root. A binding or a slot whose name the layout lacks is not applied: the
//! application notes it ([`Signals::notes`]).
//!
//! Limits. kdl 6.7.1 reads by recursion, without a bound: a block within a block, a node commented
//! out by `/-` after another, each piece at the top that it cannot read. On a thread's stack of
//! 2 MiB, a debug build overflowed with 71 blocks within each other (66 read), 100 `/- ` in a row
//! (30 read) or 1,000 `a ) ` (300 read); a release build with 1,001 blocks, 1,000 `/- ` or 10,000
//! `a ) ` (101, 100 and 1,000 read). The program then aborts where an error is due. [`read`]
//! looks at the text before kdl reads it, its strings and comments aside, and refuses the cases
//! found: blocks past [`DEPTH`], a `}` or a `)` that closes nothing, any `[` or `]`, and more than
//! [`DEPTH`] `/-` in a row. kdl may recurse without a bound on others, which no test has found.
//! uwui's own reader bounds its depth (ADR-0225). Lines are counted by KDL's newlines.

#[cfg(test)]
mod tests;

use std::fmt::Arguments;
use std::rc::Rc;
use std::sync::Arc;

use kdl::{KdlDocument, KdlEntry, KdlError, KdlNode};
use uwui::display::ImageKey;
use uwui::prelude::Element as Make;

use super::{Content, El, RwSignal, Signals, View, dyn_view, enter, leaf};

/// The deepest that a layout's elements nest, its root 1 deep.
pub(crate) const DEPTH: usize = 64;

/// The layouts that a file holds, each by its name: what a view's code makes its elements from.
/// A clone shares them; two are equal when they hold the same.
#[derive(Clone, PartialEq)]
pub struct Layouts(Rc<Vec<(String, Node)>>);

impl Layouts {
    /// The root element of the layout named `name`.
    pub fn get(&self, name: &str) -> Option<&Node> {
        self.0
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, root)| root)
    }
}

/// An element of a layout, as the file writes it.
#[derive(PartialEq)]
pub struct Node {
    kind: Kind,
    name: Option<Arc<str>>,
    classes: Vec<String>,
    text: Option<String>,
    placeholder: Option<String>,
    action: Option<Arc<str>>,
    children: Vec<Node>,
}

/// What an element is: its node's name.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Column,
    Row,
    Stack,
    Pressable,
    Label,
    Button,
    Image,
    SearchField,
    Grid,
    List,
    Slot,
}

impl Kind {
    /// Each kind, by the name that the file writes it with.
    const NAMES: [(&'static str, Self); 11] = [
        ("column", Self::Column),
        ("row", Self::Row),
        ("stack", Self::Stack),
        ("pressable", Self::Pressable),
        ("label", Self::Label),
        ("button", Self::Button),
        ("image", Self::Image),
        ("search-field", Self::SearchField),
        ("grid", Self::Grid),
        ("list", Self::List),
        ("slot", Self::Slot),
    ];

    fn named(name: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .find(|(held, _)| *held == name)
            .map(|&(_, kind)| kind)
    }

    fn name(self) -> &'static str {
        Self::NAMES
            .iter()
            .find(|(_, kind)| *kind == self)
            .map_or("", |&(name, _)| name)
    }

    /// Whether it holds the elements of a block.
    fn holds_elements(self) -> bool {
        matches!(
            self,
            Self::Column | Self::Row | Self::Stack | Self::Pressable
        )
    }

    /// Whether it takes the property `key`.
    fn takes(self, key: &str) -> bool {
        match key {
            "class" => self != Self::Slot,
            "text" => matches!(self, Self::Label | Self::Button),
            "placeholder" => self == Self::SearchField,
            "action" => matches!(self, Self::Button | Self::Pressable),
            _ => false,
        }
    }
}

/// Reads `text`, a file of KDL 2, into layouts.
///
/// # Errors
///
/// A text that names the line of what is wrong, from 1, and what: KDL's own error; a node at the
/// top other than a layout, a layout's name twice, a layout that does not hold exactly one root
/// element, or whose root is a slot; an unknown kind or property, or one that the kind does not
/// take; a name twice in one layout; a slot without a name; a block on a kind that holds no
/// elements; a value that is not a string, or given twice; elements nested past [`DEPTH`], which
/// names the layout; a `}` that closes no block.
pub fn read(text: &str) -> Result<Layouts, String> {
    nesting(text)?;
    let document = KdlDocument::parse_v2(text).map_err(|error| unread(text, &error))?;
    let mut layouts: Vec<(String, Node)> = Vec::new();
    for node in document.nodes() {
        // Its line counted only for an error: counting each node's would read the text again.
        let at = || line(text, node.span().offset());
        let named = node.name().value();
        if named != "layout" || node.ty().is_some() {
            return Err(format!(
                "line {}: `{named}` at the top of the file, which holds layouts alone: \
                 `layout <name> {{ <its root element> }}`",
                at()
            ));
        }
        let name = match node.entries() {
            [entry] if entry.name().is_none() => {
                string(text, entry, format_args!("a layout's name"))?
            }
            _ => {
                return Err(format!(
                    "line {}: a layout takes its name alone: `layout <name> {{ … }}`",
                    at()
                ));
            }
        };
        if layouts.iter().any(|(held, _)| held == name) {
            return Err(format!("line {}: a second layout named `{name}`", at()));
        }
        let roots = node.children().map_or(&[][..], KdlDocument::nodes);
        let [root] = roots else {
            return Err(format!(
                "line {}: the layout `{name}` holds {} elements at its top, where a layout \
                 holds one, its root",
                at(),
                roots.len()
            ));
        };
        let root = element(text, root, name, 1, &mut Vec::new())?;
        if root.kind == Kind::Slot {
            return Err(format!(
                "line {}: the root of the layout `{name}` is a slot, where it is an element",
                at()
            ));
        }
        layouts.push((name.to_owned(), root));
    }
    Ok(Layouts(Rc::new(layouts)))
}

/// The element that `node` writes, `depth` deep in the layout `layout`, whose names so far
/// `names` holds, each with the byte where it is written.
fn element(
    text: &str,
    node: &KdlNode,
    layout: &str,
    depth: usize,
    names: &mut Vec<(Arc<str>, usize)>,
) -> Result<Node, String> {
    let at = || line(text, node.span().offset());
    if depth > DEPTH {
        return Err(too_deep(at(), &format!("`{layout}`")));
    }
    let named = node.name().value();
    if let Some(ty) = node.ty() {
        return Err(format!(
            "line {}: `({})`, a type, which an element does not take",
            at(),
            ty.value()
        ));
    }
    let Some(kind) = Kind::named(named) else {
        let kinds: Vec<&str> = Kind::NAMES.iter().map(|&(name, _)| name).collect();
        return Err(format!(
            "line {}: no element of the kind `{named}`; the kinds: {}",
            at(),
            kinds.join(", ")
        ));
    };
    let mut el = Node {
        kind,
        name: None,
        classes: Vec::new(),
        text: None,
        placeholder: None,
        action: None,
        children: Vec::new(),
    };
    let mut classes = false;
    for entry in node.entries() {
        let at = || line(text, entry.span().offset());
        let Some(key) = entry.name() else {
            if el.name.is_some() {
                return Err(format!(
                    "line {}: a second argument, where an element takes its name alone",
                    at()
                ));
            }
            let name = string(text, entry, format_args!("an element's name"))?;
            if let Some((_, first)) = names.iter().find(|(held, _)| **held == *name) {
                return Err(format!(
                    "line {}: the name `{name}` twice in the layout `{layout}`, first at line {}",
                    at(),
                    line(text, *first)
                ));
            }
            let name: Arc<str> = name.into();
            names.push((Arc::clone(&name), entry.span().offset()));
            el.name = Some(name);
            continue;
        };
        let key = key.value();
        if !kind.takes(key) {
            return Err(format!(
                "line {}: no property `{key}` on a `{}`",
                at(),
                kind.name()
            ));
        }
        let value = string(text, entry, format_args!("`{key}`"))?;
        let given = match key {
            "class" => {
                el.classes = value.split_whitespace().map(str::to_owned).collect();
                std::mem::replace(&mut classes, true)
            }
            "text" => el.text.replace(value.to_owned()).is_some(),
            "placeholder" => el.placeholder.replace(value.to_owned()).is_some(),
            _ => el.action.replace(value.into()).is_some(),
        };
        if given {
            return Err(format!("line {}: `{key}` given twice", at()));
        }
    }
    if kind == Kind::Slot && el.name.is_none() {
        return Err(format!(
            "line {}: a slot without a name, by which the view fills it",
            at()
        ));
    }
    if let Some(block) = node.children() {
        if !kind.holds_elements() {
            return Err(format!(
                "line {}: a block on a `{}`, which holds no elements",
                at(),
                kind.name()
            ));
        }
        for child in block.nodes() {
            el.children
                .push(element(text, child, layout, depth + 1, names)?);
        }
    }
    Ok(el)
}

/// The string that `entry` holds, `what` it is.
fn string<'e>(text: &str, entry: &'e KdlEntry, what: Arguments<'_>) -> Result<&'e str, String> {
    let at = || line(text, entry.span().offset());
    if let Some(ty) = entry.ty() {
        return Err(format!(
            "line {}: `({})`, a type, which {what} does not take",
            at(),
            ty.value()
        ));
    }
    entry
        .value()
        .as_string()
        .ok_or_else(|| format!("line {}: {what} is a string, not `{}`", at(), entry.value()))
}

/// KDL's own error, its first diagnostics by their lines.
fn unread(text: &str, error: &KdlError) -> String {
    const TOLD: usize = 3;
    let mut told: Vec<String> = error
        .diagnostics
        .iter()
        .take(TOLD)
        .map(|diagnostic| {
            let at = line(text, diagnostic.span.offset());
            format!("line {at}: {diagnostic}")
        })
        .collect();
    if told.is_empty() {
        return "the file does not read as KDL 2".to_owned();
    }
    if error.diagnostics.len() > TOLD {
        told.push(format!("{} more", error.diagnostics.len() - TOLD));
    }
    told.join("; ")
}

/// The line, from 1, of the byte `offset` of `text`, by KDL's newlines.
fn line(text: &str, offset: usize) -> usize {
    let before = &text.as_bytes()[..offset.min(text.len())];
    let mut lines = 1;
    let mut at = 0;
    while at < before.len() {
        match newline(before, at) {
            Some(length) => {
                lines += 1;
                at += length;
            }
            None => at += 1,
        }
    }
    lines
}

/// The length of the newline that `bytes` holds at `at`, as KDL 2 reads one: CR LF, CR, LF, VT,
/// FF, NEL, LS or PS.
fn newline(bytes: &[u8], at: usize) -> Option<usize> {
    let rest = bytes.get(at..)?;
    if rest.starts_with(b"\r\n") {
        Some(2)
    } else if matches!(rest.first(), Some(b'\r' | b'\n' | 0x0B | 0x0C)) {
        Some(1)
    } else if rest.starts_with("\u{85}".as_bytes()) {
        Some(2)
    } else if rest.starts_with("\u{2028}".as_bytes()) || rest.starts_with("\u{2029}".as_bytes()) {
        Some(3)
    } else {
        None
    }
}

/// The first newline at or past `from` in `bytes`; their length if none.
fn line_end(bytes: &[u8], from: usize) -> usize {
    (from..bytes.len())
        .find(|&at| newline(bytes, at).is_some())
        .unwrap_or(bytes.len())
}

/// The error of an element past [`DEPTH`] at line `at`, in the layout `layout`.
fn too_deep(at: usize, layout: &str) -> String {
    format!("line {at}: the layout {layout} nests elements deeper than {DEPTH}")
}

/// Checks how deep the blocks of `text` nest, its strings and comments aside, before kdl reads
/// it, which it does by recursion: a block 1 deep is a layout's, so that a file that reads holds
/// none past `DEPTH + 1`. Refused too, since kdl recurses once for each: a `}` or a `)` that
/// closes nothing, a `[` or a `]`, which KDL never holds outside a string, and more than `DEPTH`
/// nodes commented out by `/-` in a row.
fn nesting(text: &str) -> Result<(), String> {
    let bytes = text.as_bytes();
    let mut depth = 0;
    // Where the node at the top, whose block holds what is read now, starts.
    let mut top = 0;
    // The type annotations `(…)` open, and the `/-` met since the last other token.
    let mut open = 0;
    let mut commented = 0;
    let mut at = 0;
    while at < bytes.len() {
        if !bytes[at].is_ascii_whitespace() && !bytes[at..].starts_with(b"/-") {
            commented = 0;
        }
        match (bytes[at], bytes.get(at + 1)) {
            (b'/', Some(b'/')) => {
                at = line_end(bytes, at);
                continue;
            }
            (b'/', Some(b'-')) => {
                commented += 1;
                if commented > DEPTH {
                    return Err(format!(
                        "line {}: more than {DEPTH} nodes commented out by `/-` in a row",
                        line(text, at)
                    ));
                }
                at += 2;
                continue;
            }
            (b'(', _) => open += 1,
            (b')', _) if open == 0 => {
                return Err(format!(
                    "line {}: a `)` that closes nothing",
                    line(text, at)
                ));
            }
            (b')', _) => open -= 1,
            (b'[' | b']', _) => {
                return Err(format!(
                    "line {}: a `{}`, which KDL never holds outside a string",
                    line(text, at),
                    bytes[at] as char
                ));
            }
            (b'/', Some(b'*')) => {
                at = comment_end(bytes, at);
                continue;
            }
            (b'"', _) => {
                at = string_end(bytes, at);
                continue;
            }
            (b'#', _) => {
                at = raw_end(bytes, at);
                continue;
            }
            (b'{', _) => {
                depth += 1;
                if depth > DEPTH + 1 {
                    return Err(too_deep(line(text, at), &layout_named(text, top)));
                }
            }
            (b'}', _) if depth == 0 => {
                return Err(format!(
                    "line {}: a `}}` that closes no block",
                    line(text, at)
                ));
            }
            (b'}', _) => {
                depth -= 1;
                if depth == 0 {
                    top = at + 1;
                }
            }
            (b';', _) if depth == 0 => top = at + 1,
            _ if depth == 0 && newline(bytes, at).is_some() => top = at + 1,
            _ => {}
        }
        at += 1;
    }
    Ok(())
}

/// Past the comment `/* … */` that starts at `at`, which holds others.
fn comment_end(bytes: &[u8], mut at: usize) -> usize {
    let mut depth = 0;
    while at < bytes.len() {
        match (bytes[at], bytes.get(at + 1)) {
            (b'/', Some(b'*')) => {
                depth += 1;
                at += 2;
            }
            (b'*', Some(b'/')) => {
                depth -= 1;
                at += 2;
                if depth == 0 {
                    return at;
                }
            }
            _ => at += 1,
        }
    }
    at
}

/// Past the string that starts at `at`, `"…"` or `"""…"""`, whose `\` escapes what follows. A
/// string of one line that a newline reaches ends there: kdl reads the lines after it as nodes.
fn string_end(bytes: &[u8], at: usize) -> usize {
    let lines = bytes[at..].starts_with(b"\"\"\"");
    let quotes: &[u8] = if lines { b"\"\"\"" } else { b"\"" };
    let mut at = at + quotes.len();
    while at < bytes.len() {
        if !lines && newline(bytes, at).is_some() {
            return at;
        }
        if bytes[at] == b'\\' {
            at += 2;
        } else if bytes[at..].starts_with(quotes) {
            return at + quotes.len();
        } else {
            at += 1;
        }
    }
    bytes.len()
}

/// Past the raw string that starts at `at`, `#"…"#` or `#"""…"""#` with as many `#` at both
/// ends, which escapes nothing; past its `#` alone for a keyword, `#true` or `#null`.
fn raw_end(bytes: &[u8], at: usize) -> usize {
    let hashes = bytes[at..].iter().take_while(|&&byte| byte == b'#').count();
    let opened = at + hashes;
    if bytes.get(opened) != Some(&b'"') {
        return opened;
    }
    let quotes: &[u8] = if bytes[opened..].starts_with(b"\"\"\"") {
        b"\"\"\""
    } else {
        b"\""
    };
    let lines = quotes.len() == 3;
    let closes = |at: usize| {
        bytes[at..].starts_with(quotes)
            && bytes
                .get(at + quotes.len()..at + quotes.len() + hashes)
                .is_some_and(|after| after.iter().all(|&byte| byte == b'#'))
    };
    // A raw string of one line that a newline reaches ends there, as a string does.
    let end = if lines {
        bytes.len()
    } else {
        line_end(bytes, opened + 1)
    };
    (opened + quotes.len()..end)
        .find(|&at| closes(at))
        .map_or(end, |at| at + quotes.len() + hashes)
}

/// The layout whose node starts at `top` of `text`, by its name as far as a plain look finds it:
/// the first word after the node's.
fn layout_named(text: &str, top: usize) -> String {
    let header = text.get(top..).unwrap_or_default().trim_start();
    let after = header
        .split_once(char::is_whitespace)
        .map_or("", |(_, after)| after.trim_start());
    let name = match after.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next(),
        None => after
            .split(|character: char| character.is_whitespace() || "{;".contains(character))
            .next(),
    };
    match name.filter(|name| !name.is_empty()) {
        Some(name) => format!("`{name}`"),
        None => format!("at line {}", line(text, top)),
    }
}

/// A layout's elements being made: its root, its named elements bound and its slots filled by
/// the view's code, and what binding them noted.
pub struct Laid<'n> {
    /// The layout's name, which the notes give.
    layout: &'n str,
    root: El,
    notes: Vec<String>,
}

/// The elements of the layout `name` of `layouts`, to bind. A view made again when the layouts'
/// signal changes reads them from it, and passes them on to the views that it holds, which are
/// made again with it; the layout is then made again. An empty column, noted, where `layouts`
/// holds no layout of that name.
pub fn layout<'n>(layouts: &Layouts, name: &'n str) -> Laid<'n> {
    let (root, notes) = match layouts.get(name) {
        Some(root) => (made(root), Vec::new()),
        None => (
            leaf(Make::column()),
            vec![format!("no layout `{name}` in the file")],
        ),
    };
    Laid {
        layout: name,
        root,
        notes,
    }
}

impl Laid<'_> {
    /// The element named `name` given what `bind` makes of it, through the methods that code
    /// gives an element that it makes. Not applied, and noted, where the layout names no element
    /// so.
    #[must_use]
    pub fn bind(mut self, name: &str, bind: impl FnOnce(El) -> El) -> Self {
        match named(&mut self.root, name, false) {
            Some(found) => {
                let el = std::mem::replace(found, leaf(Make::column()));
                let key = el.name.clone();
                *found = bind(el);
                // Keyed by its name in the tree still, whatever element the binding made.
                found.name = key;
            }
            None => self.notes.push(format!(
                "the binding of `{name}` names no element of the layout `{}`",
                self.layout
            )),
        }
        self
    }

    /// The slot named `name` filled by `view`, made again, the one before disposed, when what it
    /// reads changes, as [`dyn_view`] makes it. Not filled, and noted, where the layout names no
    /// slot so.
    #[must_use]
    pub fn slot<V: View>(mut self, name: &str, view: impl Fn() -> Option<V> + 'static) -> Self {
        match named(&mut self.root, name, true) {
            Some(found) => found.content = dyn_view(view).content,
            None => self.notes.push(format!(
                "the slot `{name}` names no slot of the layout `{}`",
                self.layout
            )),
        }
        self
    }

    /// The root element, which takes the methods of an element. It loses its name: it is keyed
    /// where it is put, a grid's cell by its key, a slot's view by the slot's name.
    pub fn el(self) -> El {
        let mut root = self.root;
        root.name = None;
        root.notes.extend(self.notes);
        root
    }
}

/// The element that `node` writes, and those that it holds, before any binding; a slot's shows
/// nothing until it is filled.
fn made(node: &Node) -> El {
    let element = match node.kind {
        Kind::Column | Kind::Slot => Make::column(),
        Kind::Row => Make::row(),
        Kind::Stack => Make::stack(),
        Kind::Pressable => Make::pressable(),
        Kind::Label => Make::label(node.text.clone().unwrap_or_default()),
        Kind::Button => Make::button(node.text.clone().unwrap_or_default()),
        Kind::Image => Make::image(ImageKey(0)),
        Kind::SearchField => Make::search_field(String::new()),
        Kind::Grid => Make::grid(0, [0.0, 0.0]),
        Kind::List => Make::list(0, 0.0),
    };
    let content = match node.kind {
        Kind::Slot => Content::Dyn(Rc::new(|| None::<El>)),
        _ => Content::Children(node.children.iter().map(made).collect()),
    };
    let mut el = El::new(element, content);
    for class in &node.classes {
        el = el.class(class);
    }
    if let Some(placeholder) = &node.placeholder {
        el = el.placeholder(placeholder);
    }
    el.name = node.name.clone();
    el.command = node.action.clone();
    el
}

/// The element named `name` in `el`, or below it: a slot where `slot`, else an element.
fn named<'e>(el: &'e mut El, name: &str, slot: bool) -> Option<&'e mut El> {
    if el.name.as_deref() == Some(name) && matches!(el.content, Content::Dyn(_)) == slot {
        return Some(el);
    }
    let Content::Children(children) = &mut el.content else {
        return None;
    };
    children
        .iter_mut()
        .find_map(|child| named(child, name, slot))
}

impl Signals<RwSignal<Layouts>> {
    /// Reads `text` into layouts and shows them at the next frame, the state kept: sets their
    /// signal, which the laid library's top layout reads ([`super::laid`]), and forgets what the
    /// layouts before noted. Layouts equal to those shown change nothing.
    ///
    /// # Errors
    ///
    /// If `text` does not read ([`read`]): the layouts shown stay.
    pub(crate) fn relayout(&mut self, text: &str) -> Result<(), String> {
        let layouts = read(text)?;
        let shown = self.handle;
        let changed = enter(&self.runtime, || {
            let changed = shown.with(|shown| *shown != layouts);
            if changed {
                shown.update(|shown| *shown = layouts);
            }
            changed
        });
        if changed {
            self.shared.notes.borrow_mut().clear();
        }
        Ok(())
    }
}

/// The reader's rules past the frozen contract: what the check of the blocks' nesting leaves to
/// KDL, and each error.
#[cfg(test)]
mod reads {
    use super::*;

    /// `depth` elements, each a column within the one before, the last a label.
    fn nested(depth: usize) -> String {
        let mut text = String::from("layout deep {\n");
        text.push_str(&"column {\n".repeat(depth - 1));
        text.push_str("label last\n");
        text.push_str(&"}\n".repeat(depth));
        text
    }

    /// `read` of `text`, on a thread of 2 MiB as tests run: an overflow of kdl's stack aborts the
    /// whole run, as it would the program.
    fn read_apart(text: String) -> Result<(), String> {
        std::thread::Builder::new()
            .stack_size(2 << 20)
            .spawn(move || super::read(&text).map(drop))
            .expect("a thread")
            .join()
            .expect("read returns")
    }

    #[test]
    fn a_comment_ends_at_any_of_kdls_newlines() {
        for end in ["\r", "\u{2028}", "\u{85}", "\x0C"] {
            let text = format!("// a comment{end}{}", nested(DEPTH + 2)).replace('\n', end);
            let error = read_apart(text).expect_err("past the depth, whatever the newlines");
            assert!(error.contains("deeper"), "{end:?}: {error}");
        }
        let error = read_apart("layout a {\r    carousel b\r}\r".to_owned()).unwrap_err();
        assert!(error.contains("line 2"), "CR alone counts lines: {error}");
    }

    #[test]
    fn a_string_left_open_does_not_hide_the_blocks_after_it() {
        for open in ["text=\"x", "text=#\"x"] {
            let text = format!(
                "layout a {{\n    label b {open}\n{}{}\"\n}}\n",
                "column {\n".repeat(300),
                "}\n".repeat(300)
            );
            assert!(read_apart(text).is_err(), "{open}");
        }
    }

    #[test]
    fn runs_that_kdl_reads_by_recursion_do_not_read() {
        for run in ["/- ", "a ) ", "a ] ", "a [ ", "} "] {
            let text = format!("layout a {{\n    column\n}}\n{}", run.repeat(10_000));
            assert!(read_apart(text).is_err(), "{run:?}");
        }
        let text = format!("layout a {{\n    column {}b\n}}\n", "/- c ".repeat(10_000));
        assert!(
            read_apart(text).is_ok(),
            "nodes commented out one by one read"
        );
    }

    #[test]
    fn braces_in_strings_and_comments_are_not_blocks() {
        let text = "// a } here\n/* a { here /* and } within */ */\nlayout a {\n    column {\n        \
                    label b text=\"}}\\\"{\"\n        label c text=#\"}\"{\"#\n        button d \
                    text=\"\"\"\n            }\n            \"\"\"\n    }\n}\n";
        let layouts = read(text).expect("it reads");
        let root = layouts.get("a").expect("the layout");
        assert_eq!(root.children[0].text.as_deref(), Some("}}\"{"));
        assert_eq!(root.children[1].text.as_deref(), Some("}\"{"));
        assert_eq!(root.children[2].text.as_deref(), Some("}"));
    }

    #[test]
    fn a_brace_that_closes_no_block_does_not_read() {
        let error = read("layout a {\n    column\n}\n}\n")
            .err()
            .expect("one `}` too many");
        assert!(error.starts_with("line 4:"), "{error}");
    }

    #[test]
    fn elements_nest_64_deep_and_no_deeper() {
        assert!(read(&nested(DEPTH)).is_ok());
        let error = read(&nested(DEPTH + 1))
            .err()
            .expect("an element past the limit");
        assert!(error.contains("`deep`"), "{error}");
        let error = read(&nested(DEPTH + 2))
            .err()
            .expect("a block past the limit");
        assert!(error.contains("`deep`"), "{error}");
    }

    #[test]
    fn each_mistake_names_its_line() {
        let mistakes = [
            ("layout a {\n    label b placeholder=c\n}\n", 2),
            ("layout a {\n    label b text=5\n}\n", 2),
            ("layout a {\n    label b {\n    }\n}\n", 2),
            ("layout a {\n    column {\n        slot\n    }\n}\n", 3),
            ("layout a {\n    label b\n    label c\n}\n", 1),
            ("layout a {\n    slot b\n}\n", 1),
            ("layout a {\n    label b c\n}\n", 2),
            ("style a {\n    label b\n}\n", 1),
            (
                "layout a {\n    label b\n}\nlayout a {\n    label b\n}\n",
                4,
            ),
            ("layout a {\n    label b class=c class=d\n}\n", 2),
            ("layout a {\n    (big)label b\n}\n", 2),
        ];
        for (text, line) in mistakes {
            let error = read(text).err().expect(text);
            assert!(
                error.starts_with(&format!("line {line}:")),
                "{text}: {error}"
            );
        }
    }

    #[test]
    fn classes_are_apart_by_spaces() {
        let layouts = read("layout a {\n    label b class=\"c  d\"\n}\n").expect("it reads");
        assert_eq!(layouts.get("a").expect("the layout").classes, ["c", "d"]);
    }

    #[test]
    fn a_binding_or_a_slot_whose_name_is_gone_is_noted() {
        let layouts =
            read("layout a {\n    column {\n        slot b\n    }\n}\n").expect("it reads");
        let el = layout(&layouts, "a")
            .bind("b", |el| el.class("bound"))
            .slot("c", || None::<El>)
            .el();
        assert_eq!(el.notes.len(), 2, "{:?}", el.notes);
        assert!(el.notes[0].contains("`b`") && el.notes[1].contains("`c`"));
        assert_eq!(
            layout(&layouts, "z").el().notes,
            ["no layout `z` in the file"]
        );
    }
}
