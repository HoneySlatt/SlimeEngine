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
    /// (`docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md`). A document's
    /// version is read before the rest of it: read alone, such a document gives this error alone.
    /// None for every other error.
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
/// layout read leniently left out: each error found, up to [`DocumentErrors::MOST`]; the interface
/// keeps the style or the shortcuts it had. Each document's version is read before the rest of it
/// ([`DocumentError::later`]). The themes are read first, every value of each in turn, then the names that the
/// values give one another, then the tokens; the style sheets are read only once the themes are
/// taken, in their order, each rule in the order of its file, and in a rule its `select` first,
/// wherever it is written, then what it sets, then whether its kind reads it.
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

    /// The errors kept, in the order they were found, at least one.
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

/// A format of uwui's documents that carries its version: a theme, a style sheet or a table of
/// shortcuts (`docs/adr/0286-every-document-of-uwui-versioned-as-dock-layouts-are.md`).
///
/// A document's first node, `version 1`, writes the version of its format, a whole number from 1,
/// alone; a document without it is of version 1, the format of before the versions. A document of
/// a later version than the reader's is refused whole, the error naming both versions and keeping
/// the later one ([`DocumentError::later`]). A document of an earlier version is migrated as it is
/// read: a change of a format's meaning comes with a new version, its migration and a fixture of
/// the version before it in the reader's tests. A node `version` anywhere but first is refused, so
/// no rule of a style sheet is named so.
///
/// A dock's layout carries its version too, read by its own reader, which requires it
/// ([`dock::read`](crate::dock::read)).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Format {
    /// What a document of the format is, as its messages name it: `the theme`.
    pub(crate) what: &'static str,
    /// The last version of the format that this crate reads, and writes.
    pub(crate) version: i64,
}

impl Format {
    /// The nodes of `document`, the text `text` named `name` in its errors, past its version,
    /// each error about the version kept in `errors`; none for a document of a later version than
    /// [`Format::version`], which is not read.
    ///
    /// A version written wrongly, `version "2"`, is an error, and the rest of the document is read
    /// as version 1, so that its other errors are told too.
    pub(crate) fn nodes<'d, 't>(
        self,
        document: &'d Document<'t>,
        name: &str,
        text: &str,
        errors: &mut DocumentErrors,
    ) -> Option<impl Iterator<Item = Node<'d, 't>> + use<'d, 't>> {
        let is_version = |node: &Node<'_, '_>| node.name().text() == "version";
        for (place, node) in document.nodes().enumerate() {
            if !is_version(&node) {
                continue;
            }
            if place > 0 {
                errors.push_with(|| {
                    DocumentError::at(
                        name,
                        text,
                        node.span().start,
                        format!(
                            "`version` is given once, the first node of {}: `version {}`",
                            self.what, self.version
                        ),
                    )
                });
                continue;
            }
            match version_of(&node) {
                Ok((at, version)) if version > self.version => {
                    errors.push(DocumentError {
                        later: Some(version),
                        ..DocumentError::at(
                            name,
                            text,
                            at,
                            format!(
                                "{} is of the format's version {version}, later than {}, the last \
                                 that this program reads",
                                self.what, self.version
                            ),
                        )
                    });
                    return None;
                }
                Ok(_) => {}
                Err(at) => errors.push_with(|| {
                    DocumentError::at(
                        name,
                        text,
                        at,
                        format!(
                            "`version` writes one whole number from 1 alone, the version of the \
                             format of {}: `version {}`",
                            self.what, self.version
                        ),
                    )
                }),
            }
        }
        Some(document.nodes().filter(move |node| !is_version(node)))
    }
}

/// The version that `node`, `version 1`, writes, and the byte where it is written; the byte of
/// what is wrong for a node that writes more or less than one whole number from 1.
fn version_of(node: &Node<'_, '_>) -> Result<(usize, i64), usize> {
    let start = node.span().start;
    if node.annotation().is_some() {
        return Err(start);
    }
    if let Some(child) = node.children().next() {
        return Err(child.span().start);
    }
    let mut entries = node.entries();
    let first = entries.next().ok_or(start)?;
    if let Some(extra) = entries.next() {
        return Err(extra.span().start);
    }
    let at = first.span().start;
    match first.value() {
        Value::Number(number) if first.name().is_none() && first.annotation().is_none() => number
            .to_i64()
            .filter(|version| *version >= 1)
            .map(|version| (at, version))
            .ok_or(at),
        _ => Err(at),
    }
}
