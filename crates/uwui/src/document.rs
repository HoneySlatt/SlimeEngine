//! The documents that the interface reads, style sheets, themes and tables of shortcuts: how a
//! program gives them, and why one cannot be taken.

use std::fmt;

use slime_reflect::Registry;
use slime_serialize::{Document, Mode, ReadError};

/// A document read through reflection, a style sheet until M6d.3b2 or a table of shortcuts until
/// M6d.3c, as a program has it: a text, as its author wrote it, or its binary encoding, as the
/// cooker writes it (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
#[derive(Clone, Debug)]
pub(crate) enum Source {
    Text(String),
    Binary(Vec<u8>),
}

impl Source {
    /// The document, parsed strictly, as a source written by a person is, with the types of
    /// `registry`.
    pub(crate) fn parse<'a>(&'a self, registry: &'a Registry) -> Result<Document<'a>, ReadError> {
        match self {
            Self::Text(text) => Document::parse(registry, text, Mode::Strict),
            Self::Binary(bytes) => Document::parse_binary(registry, bytes, Mode::Strict),
        }
    }
}

/// Why a style sheet, a theme or a table of shortcuts cannot be taken: the document at fault, and
/// what is wrong, and where in it.
#[derive(Debug)]
pub struct DocumentError {
    /// The name of the document at fault.
    pub document: String,
    /// What is wrong, and where in it.
    pub error: Fault,
}

/// What is wrong in a document, and where.
#[derive(Debug)]
#[non_exhaustive]
pub enum Fault {
    /// In a document of KDL, a theme: its line, its column and what is wrong.
    Kdl(uwui_kdl::Error),
    /// In a document still read through reflection, a style sheet until M6d.3b2 or a table of
    /// shortcuts until M6d.3c, which then read KDL too
    /// (`docs/adr/0241-every-document-of-uwui-in-kdl-read-by-its-own-reader.md`).
    Reflected(ReadError),
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
            error: Fault::Kdl(uwui_kdl::Error::at(text, offset, message)),
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

impl fmt::Display for Fault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kdl(error) => error.fmt(formatter),
            Self::Reflected(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Kdl(error) => Some(error),
            Self::Reflected(error) => Some(error),
        }
    }
}

/// Why style sheets and themes cannot be taken: each error found, in the order of the documents,
/// up to [`DocumentErrors::MOST`]; the interface keeps the style it had.
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
        if self.errors.len() < Self::MOST {
            self.errors.push(error);
        } else {
            self.more = true;
        }
    }

    /// Whether it holds no error.
    pub(crate) fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// The errors kept, in the order of the documents, at least one.
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
