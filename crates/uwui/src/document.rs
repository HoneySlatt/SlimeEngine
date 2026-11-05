//! The documents that the interface reads, style sheets, themes and tables of shortcuts: how a
//! program gives them, and why one cannot be taken.

use std::fmt;

use slime_reflect::Registry;
use slime_serialize::{Document, Mode, ReadError};

/// A document as a program has it: a text, as its author wrote it, or its binary encoding, as the
/// cooker writes it.
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
    pub error: ReadError,
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
