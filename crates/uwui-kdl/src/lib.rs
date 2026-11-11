//! uwui's reader of KDL 2.0, the notation of every document of uwui: its layouts, a theme's
//! parts, values and rules, its shortcuts and its docks' layouts
//! (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
//!
//! [`read`] reads a document into a [`Document`]: a flat tree of nodes, each with its name, its
//! annotation, its entries (arguments and properties, in their order, duplicates kept) and its
//! children, and each placed in the text by its span. A string of one line without escapes is borrowed
//! from the text; one that does is decoded when it is asked for. The reader checks the syntax
//! alone: what a node means, and whether a property given twice is refused, is the typed layer's
//! work above it.
//!
//! Content from outside is read within [`Limits`]: the text's size, the depth of nested children,
//! and the count of nodes and entries, which bounds the memory that a document takes. The reader
//! never recurses, and its time grows with the text alone. An error names its line and column.
//!
//! ```
//! let document = uwui_kdl::read("button back text=Back { icon arrow-left }").expect("read");
//! let button = document.nodes().next().expect("a node");
//! assert_eq!(button.name().text(), "button");
//! assert_eq!(button.arguments().next().expect("an argument").value().to_string(), "back");
//! assert_eq!(button.property("text").expect("a property").to_string(), "Back");
//! assert_eq!(button.children().count(), 1);
//! ```
//!
//! [`Document::normal_form`] writes a document in the normal form of KDL's official tests, which
//! the tests compare: comments and spaces gone, properties in alphabetical order, the last of a
//! name kept, strings quoted only where they must be, numbers in decimal.

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

mod chars;
mod normal;
mod number;
mod read;
mod string;

/// What a document may hold before it is refused, so that content from outside takes bounded
/// time and memory (`docs/adr/0190-uwuis-requirements.md`).
///
/// A program may raise them for documents of its own; raised for content from outside, they bound
/// it less. Whatever `size` says, a text beyond 4 GiB is refused: spans are kept in 32 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most bytes of text.
    pub size: usize,
    /// The most levels of children below the document's own nodes: a node at the top is at depth
    /// 0, its children at depth 1. Slashdashed nodes count, as the text holds them.
    pub depth: usize,
    /// The most nodes and entries read, together. Slashdashed ones count: each is read before it
    /// is left out.
    pub items: usize,
}

impl Limits {
    /// The limits of [`read`]: 16 MiB, 64 levels, and 2^20 nodes and entries. A document holds some
    /// six bytes for each byte of a document of rules and values, and 48 for each node: 2^20 nodes
    /// in 2 MiB of text, the most that these limits allow, hold 50 MB once read, and allocate
    /// three times as much in all as their arrays grow, the old ones given back (`ui.kdl.read`,
    /// `benches/README.md`).
    pub const DEFAULT: Self = Self {
        size: 16 << 20,
        depth: 64,
        items: 1 << 20,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Reads `text` as a KDL 2.0 document, within [`Limits::DEFAULT`].
///
/// # Errors
///
/// The first place where `text` is not KDL 2.0, or passes a limit; but inside a multi-line
/// string, whose indentation is known only at its end, an unknown escape is named before a line
/// indented wrongly above it.
pub fn read(text: &str) -> Result<Document<'_>, Error> {
    read_within(text, &Limits::DEFAULT)
}

/// Reads `text` as a KDL 2.0 document, within `limits`.
///
/// # Errors
///
/// The first place where `text` is not KDL 2.0, or passes a limit; but inside a multi-line
/// string, whose indentation is known only at its end, an unknown escape is named before a line
/// indented wrongly above it.
pub fn read_within<'t>(text: &'t str, limits: &Limits) -> Result<Document<'t>, Error> {
    read::document(text, limits)
}

/// Why a text is not a document: what is wrong, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    /// The line, from 1.
    pub line: u32,
    /// The column, from 1, in characters: a character of several bytes counts once.
    pub column: u32,
    /// The byte where it is wrong, from the text's start.
    pub offset: usize,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for Error {}

/// A document read: its nodes, in a flat tree whose items borrow the text.
#[derive(Clone, Debug)]
pub struct Document<'t> {
    text: &'t str,
    /// Every node; the children of one node lie next to each other, in their order.
    nodes: Vec<NodeData>,
    /// Every entry; the entries of one node lie next to each other, in their order.
    entries: Vec<EntryData>,
    /// The document's own nodes, in `nodes`.
    roots: Range<u32>,
}

/// A node as the document keeps it.
#[derive(Clone, Debug)]
struct NodeData {
    name: Token,
    annotation: Option<Token>,
    /// Its entries, in [`Document::entries`].
    entries: Range<u32>,
    /// Its children, in [`Document::nodes`]; empty without a block or with an empty one.
    children: Range<u32>,
    /// From its annotation or name to its end: its last entry, or its block's `}`.
    span: Range<u32>,
}

/// An entry as the document keeps it.
#[derive(Clone, Debug)]
struct EntryData {
    /// Its name, for a property.
    name: Option<Token>,
    annotation: Option<Token>,
    value: ValueData,
    /// From its annotation, name or value to its value's end.
    span: Range<u32>,
}

/// A value as the document keeps it.
#[derive(Clone, Debug)]
enum ValueData {
    String(Token),
    /// Its text as written, which [`Number`] reads when asked.
    Number(Range<u32>),
    Bool(bool),
    Null,
}

/// A string as the document keeps it: where it lies, and how it is written, which says how to
/// decode it.
#[derive(Clone, Debug)]
struct Token {
    span: Range<u32>,
    form: Form,
}

/// How a string is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    /// An identifier, `node`.
    Bare,
    /// Quoted, `"…"`, holding no escape: its text lies between the quotes.
    Plain,
    /// Quoted, `"…"`, holding escapes.
    Escaped,
    /// Raw, `#"…"#`, its text between the quotes and the hashes.
    Raw,
    /// Multi-line, `"""…"""`, its indentation and escapes undone when decoded.
    MultiLine,
    /// Raw and multi-line, `#"""…"""#`, its indentation undone when decoded.
    RawMultiLine,
}

impl<'t> Document<'t> {
    /// The text it was read from.
    #[must_use]
    pub fn text(&self) -> &'t str {
        self.text
    }

    /// Its own nodes, in their order.
    #[must_use]
    pub fn nodes(&self) -> Nodes<'_, 't> {
        Nodes {
            document: self,
            range: self.roots.clone(),
        }
    }

    /// The document written in the normal form of KDL's official tests: no comments, one node a
    /// line, four spaces of indentation for each level, a node's arguments then its properties in
    /// alphabetical order, only the last property of a name kept, then its children if it has any;
    /// strings quoted only where they must be; numbers in decimal, without their underscores, `+`
    /// or leading zeros, a float's exponent written after `E` with its sign; and a newline after
    /// the last node, or alone for a document without nodes.
    ///
    /// A number written in binary, octal or hexadecimal is converted exactly, at any length, in
    /// time quadratic in its length: a number of 100 000 hexadecimal digits takes a second, one that
    /// fills the default limit of size hours. Its
    /// output grows with the square of a node's depth, by its indentation. Floats keep their
    /// digits, as the official tests expect: `1.50` and `1.5` are equal values written apart.
    #[must_use]
    pub fn normal_form(&self) -> String {
        normal::write(self)
    }
}

/// Nodes of a document: its own, or a node's children.
#[derive(Clone, Debug)]
pub struct Nodes<'d, 't> {
    document: &'d Document<'t>,
    range: Range<u32>,
}

impl<'d, 't> Iterator for Nodes<'d, 't> {
    type Item = Node<'d, 't>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.range.next()?;
        Some(Node {
            document: self.document,
            index,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl ExactSizeIterator for Nodes<'_, '_> {}

/// A node of a document.
#[derive(Clone, Copy, Debug)]
pub struct Node<'d, 't> {
    document: &'d Document<'t>,
    index: u32,
}

impl<'d, 't> Node<'d, 't> {
    fn data(&self) -> &'d NodeData {
        &self.document.nodes[self.index as usize]
    }

    /// Its name.
    #[must_use]
    pub fn name(&self) -> Str<'d, 't> {
        Str::new(self.document, &self.data().name)
    }

    /// Its annotation, `(type)`, if it has one.
    #[must_use]
    pub fn annotation(&self) -> Option<Str<'d, 't>> {
        self.data()
            .annotation
            .as_ref()
            .map(|token| Str::new(self.document, token))
    }

    /// Its entries, arguments and properties, in their order, a property given twice twice.
    #[must_use]
    pub fn entries(&self) -> Entries<'d, 't> {
        Entries {
            document: self.document,
            range: self.data().entries.clone(),
        }
    }

    /// Its arguments, the entries without a name, in their order.
    pub fn arguments(&self) -> impl Iterator<Item = Entry<'d, 't>> + use<'d, 't> {
        self.entries().filter(|entry| entry.name().is_none())
    }

    /// The value of its last property named `name`, as KDL reads a property given twice.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<Value<'d, 't>> {
        self.entries()
            .filter(|entry| entry.name().is_some_and(|given| given.text() == name))
            .last()
            .map(|entry| entry.value())
    }

    /// Its children, in their order; none without a block.
    #[must_use]
    pub fn children(&self) -> Nodes<'d, 't> {
        Nodes {
            document: self.document,
            range: self.data().children.clone(),
        }
    }

    /// Where it lies in the text, in bytes: from its annotation or name to its last entry or its
    /// block's end.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        widen(&self.data().span)
    }
}

/// Entries of a node.
#[derive(Clone, Debug)]
pub struct Entries<'d, 't> {
    document: &'d Document<'t>,
    range: Range<u32>,
}

impl<'d, 't> Iterator for Entries<'d, 't> {
    type Item = Entry<'d, 't>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.range.next()?;
        Some(Entry {
            document: self.document,
            index,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl ExactSizeIterator for Entries<'_, '_> {}

/// An entry of a node: an argument, or a property, which has a name.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'d, 't> {
    document: &'d Document<'t>,
    index: u32,
}

impl<'d, 't> Entry<'d, 't> {
    fn data(&self) -> &'d EntryData {
        &self.document.entries[self.index as usize]
    }

    /// Its name, for a property; none for an argument.
    #[must_use]
    pub fn name(&self) -> Option<Str<'d, 't>> {
        self.data()
            .name
            .as_ref()
            .map(|token| Str::new(self.document, token))
    }

    /// Its value's annotation, `(type)`, if it has one.
    #[must_use]
    pub fn annotation(&self) -> Option<Str<'d, 't>> {
        self.data()
            .annotation
            .as_ref()
            .map(|token| Str::new(self.document, token))
    }

    /// Its value.
    #[must_use]
    pub fn value(&self) -> Value<'d, 't> {
        match &self.data().value {
            ValueData::String(token) => Value::String(Str::new(self.document, token)),
            ValueData::Number(span) => Value::Number(Number {
                text: &self.document.text[widen(span)],
                span: widen(span),
            }),
            ValueData::Bool(value) => Value::Bool(*value),
            ValueData::Null => Value::Null,
        }
    }

    /// Where it lies in the text, in bytes: from its annotation, name or value to its value's end.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        widen(&self.data().span)
    }
}

/// A value of an entry.
#[derive(Clone, Debug)]
pub enum Value<'d, 't> {
    /// A string, an identifier or quoted.
    String(Str<'d, 't>),
    /// A number, as written.
    Number(Number<'t>),
    /// `#true` or `#false`.
    Bool(bool),
    /// `#null`.
    Null,
}

impl fmt::Display for Value<'_, '_> {
    /// The value as text: a string's text, a number as written, `#true`, `#false` or `#null`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(string) => formatter.write_str(&string.text()),
            Self::Number(number) => formatter.write_str(number.text),
            Self::Bool(true) => formatter.write_str("#true"),
            Self::Bool(false) => formatter.write_str("#false"),
            Self::Null => formatter.write_str("#null"),
        }
    }
}

/// A string of a document: a name, an annotation, or a value.
#[derive(Clone, Debug)]
pub struct Str<'d, 't> {
    document: &'d Document<'t>,
    token: &'d Token,
}

impl<'d, 't> Str<'d, 't> {
    fn new(document: &'d Document<'t>, token: &'d Token) -> Self {
        Self { document, token }
    }

    /// Its text: borrowed from the document where it is written without escapes or indentation,
    /// decoded otherwise.
    #[must_use]
    pub fn text(&self) -> Cow<'t, str> {
        string::decode(self.document.text, &self.token.span, self.token.form)
    }

    /// Where it lies in the text, in bytes, its quotes and hashes included.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        widen(&self.token.span)
    }
}

/// A number of a document, as written: in decimal, `1_000`, `-1.5e3`, in hexadecimal, octal or
/// binary, `0xff`, `0o17`, `0b101`, or `#inf`, `#-inf` and `#nan`.
#[derive(Clone, Debug)]
pub struct Number<'t> {
    text: &'t str,
    span: Range<usize>,
}

impl<'t> Number<'t> {
    /// Its text, as written.
    #[must_use]
    pub fn text(&self) -> &'t str {
        self.text
    }

    /// Its value, if it is written as a whole number, in any base, that an `i64` holds: `1.5e3`
    /// is written as a float, and gives none.
    #[must_use]
    pub fn to_i64(&self) -> Option<i64> {
        number::integer(self.text)
    }

    /// Its value as a float, rounded to the nearest, infinite or not a number for `#inf`, `#-inf`
    /// and `#nan`.
    #[must_use]
    pub fn to_f64(&self) -> f64 {
        number::float(self.text)
    }

    /// Where it lies in the text, in bytes.
    #[must_use]
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }
}

/// A span kept in `u32`, as a span of `usize`: the reader refuses a text beyond 4 GiB.
fn widen(span: &Range<u32>) -> Range<usize> {
    span.start as usize..span.end as usize
}
