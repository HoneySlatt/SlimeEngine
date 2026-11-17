//! Why a document that the interface reads, a style sheet, a theme, a table of shortcuts or a dock's
//! layout, cannot be taken; and the version of its format that each of them carries
//! (`docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md`).

use std::fmt;

use uwui_kdl::{Document, Node, Value};

/// Why a style sheet, a theme, a table of shortcuts or a dock's layout cannot be taken: the
/// document at fault, and what is wrong, and where in it.
#[derive(Debug)]
pub struct DocumentError {
    /// The name of the document at fault.
    pub document: String,
    /// What is wrong, and where in it: its line, its column and what is wrong, every document of
    /// the interface being KDL
    /// (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
    pub error: uwui_kdl::Error,
    /// The version of the document's format, when it is later than the last that this crate
    /// reads: the document was refused whole, and a program that writes such a file never writes
    /// over it, so that what a later program wrote is neither misread nor lost
    /// (`docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md`). Such an error is
    /// kept even past [`DocumentErrors::MOST`]. None for every other error, a text that is not KDL
    /// among them: a program that writes a file leaves alone, or sets aside, one that it cannot
    /// read, whatever its version.
    pub later: Option<i64>,
}

impl DocumentError {
    /// The error `message` at the byte `offset` of `text`, the document `document`'s text.
    pub(crate) fn at(
        document: &str,
        text: &str,
        offset: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            document: document.to_owned(),
            error: uwui_kdl::Error::at(text, offset, message),
            later: None,
        }
    }

    /// The error `error`, which the reader of KDL found in the document `document`.
    pub(crate) fn unreadable(document: &str, error: uwui_kdl::Error) -> Self {
        Self {
            document: document.to_owned(),
            error,
            later: None,
        }
    }
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.document, self.error)
    }
}

impl std::error::Error for DocumentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Why style sheets and themes, tables of shortcuts, or a dock's layout cannot be taken, or what a
/// layout read leniently left out: each error found, up to [`DocumentErrors::MOST`], but a
/// document's of a later version, always kept ([`DocumentError::later`]); the interface keeps the
/// style or the shortcuts it had. A document's version, and a node `version` out of its place, are
/// told before the rest of the document. The themes are read first, every value of each in turn,
/// then the names that the values give one another, then the tokens; the style sheets are read
/// only once the themes are taken, in their order, each rule in the order of its file, and in a
/// rule its `select` first, wherever it is written, then what it sets, then whether its kind reads
/// it.
#[derive(Debug)]
pub struct DocumentErrors {
    errors: Vec<DocumentError>,
    /// Whether more errors were found than are kept.
    more: bool,
}

impl DocumentErrors {
    /// The most errors kept: past them, a document is likely not what it is read as, and more
    /// would tell nothing new.
    pub const MOST: usize = 16;

    /// No error yet.
    pub(crate) fn new() -> Self {
        Self {
            errors: Vec::new(),
            more: false,
        }
    }

    /// Keeps `error`, if fewer than [`DocumentErrors::MOST`] are kept.
    pub(crate) fn push(&mut self, error: DocumentError) {
        self.push_with(|| error);
    }

    /// Keeps `error`, a document's of a later version, however many are kept: a program must
    /// learn that a file of its is of a later version, whatever errors the other documents hold.
    /// One at most for each document read.
    fn push_later(&mut self, error: DocumentError) {
        self.errors.push(error);
    }

    /// Keeps the error that `make` gives, if fewer than [`DocumentErrors::MOST`] are kept; past
    /// them, `make` is not called: placing an error at its line reads its text up to it, which a
    /// document of many errors would pay for each.
    pub(crate) fn push_with(&mut self, make: impl FnOnce() -> DocumentError) {
        if self.errors.len() < Self::MOST {
            self.errors.push(make());
        } else {
            self.more = true;
        }
    }

    /// Whether it holds no error.
    pub(crate) fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// The errors kept, in the order they were found, at least one; more than
    /// [`DocumentErrors::MOST`] only with errors of documents of a later version.
    #[must_use]
    pub fn errors(&self) -> &[DocumentError] {
        &self.errors
    }

    /// Whether more errors were found than [`DocumentErrors::MOST`], which were left out.
    #[must_use]
    pub fn more(&self) -> bool {
        self.more
    }
}

impl From<DocumentError> for DocumentErrors {
    fn from(error: DocumentError) -> Self {
        Self {
            errors: vec![error],
            more: false,
        }
    }
}

impl fmt::Display for DocumentErrors {
    /// Each error on a line of its own, then a line saying that more were left out, if they were.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (number, error) in self.errors.iter().enumerate() {
            if number > 0 {
                formatter.write_str("\n")?;
            }
            write!(formatter, "{error}")?;
        }
        if self.more {
            write!(formatter, "\nand more, past the first {}", Self::MOST)?;
        }
        Ok(())
    }
}

impl std::error::Error for DocumentErrors {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.errors
            .first()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

/// A format of uwui's documents, which carries its version: a theme, a style sheet, a table of
/// shortcuts or a dock's layout (`docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md`).
///
/// A document's first node, `version 1`, writes the version of its format, a whole number from 1,
/// alone. Its first argument is read first: a whole number above [`Format::version`] is a later
/// version, whatever else the node holds, so that a later format may write more on it. A document
/// of a later version is refused whole, the error naming both versions and keeping the later one
/// ([`DocumentError::later`]). A version that is not a whole number from 1 that an `i64` holds is
/// an error, and the rest of the document is read as of the reader's version, so that its other
/// errors are told too. A node `version` anywhere but first is refused, so that no rule of a style
/// sheet is named `version`, the owner's choice of 2026-11-17.
///
/// A theme, a style sheet or a table of shortcuts without the node is of version 1, the format of
/// before the versions ([`Format::nodes`]); a dock's layout requires it, read by its own reader
/// through [`Format::version`]. A change of the meaning of a theme, a sheet or a table comes with
/// a new version, its migration as it is read, and a fixture of the version before it in the
/// reader's tests; a layout's format follows its own rule
/// (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Format {
    /// What a document of the format is, as its messages name it: `the theme`.
    pub(crate) what: &'static str,
    /// The last version of the format that this crate reads, which a program that writes a
    /// document of it gives it.
    pub(crate) version: i64,
    /// What a node of the document is named by its author, which is never `version`: `rule`, in
    /// a style sheet, whose rules are named freely; none where nodes' names are the format's.
    pub(crate) named: Option<&'static str>,
}

/// What a document's first node, `version 1`, writes.
pub(crate) enum Version {
    /// A version that this crate reads, the node holding its number alone, or not, at the byte
    /// of what it holds beyond.
    Known { more: Option<usize> },
    /// A later version than the reader's, whatever else the node holds.
    Later(i64),
    /// No whole number from 1: the byte of what is written in its place.
    Wrong(usize),
}

impl Format {
    /// What `node`, a document's first node named `version`, writes. A later version is told
    /// first, whatever else the node holds; then a version written wrongly; then what the node
    /// holds beyond its number, an annotation, an entry or a child, which the caller refuses or
    /// leaves out with a warning.
    pub(crate) fn version(self, node: &Node<'_, '_>) -> Version {
        let first = node.arguments().next();
        let at = first
            .as_ref()
            .map_or(node.span().start, |entry| entry.span().start);
        let version = first.as_ref().and_then(|entry| match entry.value() {
            Value::Number(number) => number.to_i64().filter(|version| *version >= 1),
            _ => None,
        });
        let Some(version) = version else {
            return Version::Wrong(at);
        };
        if version > self.version {
            return Version::Later(version);
        }
        let more = node
            .annotation()
            .map(|annotation| annotation.span().start)
            .or_else(|| {
                first
                    .and_then(|entry| entry.annotation())
                    .map(|annotation| annotation.span().start)
            })
            .or_else(|| node.entries().nth(1).map(|entry| entry.span().start))
            .or_else(|| {
                node.entries()
                    .find(|entry| entry.name().is_some())
                    .map(|entry| entry.span().start)
            })
            .or_else(|| node.children().next().map(|child| child.span().start));
        Version::Known { more }
    }

    /// Why a document of version `version` is not read.
    pub(crate) fn later(self, version: i64) -> String {
        format!(
            "{} is of the format's version {version}, later than {}, the last that this program \
             reads",
            self.what, self.version
        )
    }

    /// Why a node `version` is refused that writes no version, or more than its version.
    pub(crate) fn wrong(self) -> String {
        format!(
            "`version` writes one whole number from 1 alone, the version of the format of {}: \
             `version {}`{}",
            self.what,
            self.version,
            self.never_named()
        )
    }

    /// Why a node `version` that is not a document's first is refused.
    pub(crate) fn out_of_place(self) -> String {
        format!(
            "`version` is given once, the first node of {}: `version {}`{}",
            self.what,
            self.version,
            self.never_named()
        )
    }

    /// What a message about a node `version` adds where an author names the nodes.
    fn never_named(self) -> String {
        self.named
            .map(|node| format!("; no {node} is named `version`"))
            .unwrap_or_default()
    }

    /// The nodes of `document`, a theme, a style sheet or a table of shortcuts, the text `text`
    /// named `name` in its errors, past its version, each error about the version kept in
    /// `errors`; none for a document of a later version than [`Format::version`], which is not
    /// read. A document without its version is of version 1.
    pub(crate) fn nodes<'d, 't>(
        self,
        document: &'d Document<'t>,
        name: &str,
        text: &str,
        errors: &mut DocumentErrors,
    ) -> Option<impl Iterator<Item = Node<'d, 't>> + use<'d, 't>> {
        let is_version = |node: &Node<'_, '_>| node.name().text() == "version";
        let fail = |errors: &mut DocumentErrors, at: usize, message: String| {
            errors.push_with(|| DocumentError::at(name, text, at, message));
        };
        for (place, node) in document.nodes().enumerate() {
            if !is_version(&node) {
                continue;
            }
            if place > 0 {
                fail(errors, node.span().start, self.out_of_place());
                continue;
            }
            match self.version(&node) {
                Version::Later(version) => {
                    let at = node
                        .arguments()
                        .next()
                        .map_or(0, |entry| entry.span().start);
                    errors.push_later(DocumentError {
                        later: Some(version),
                        ..DocumentError::at(name, text, at, self.later(version))
                    });
                    return None;
                }
                Version::Wrong(at) | Version::Known { more: Some(at) } => {
                    fail(errors, at, self.wrong());
                }
                Version::Known { more: None } => {}
            }
        }
        Some(document.nodes().filter(move |node| !is_version(node)))
    }
}
