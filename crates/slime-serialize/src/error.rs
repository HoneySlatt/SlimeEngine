//! Why a document cannot be read, and where.

use std::fmt;
use std::path::{Path as FilePath, PathBuf};

use slime_reflect::{Path, Problem};

/// Why a document cannot be read, or what a lenient reading left out of it
/// ([`Mode::Lenient`](crate::Mode::Lenient)), and where: the file, if one was read; the line and the
/// column, from 1, in a text, the byte, from 0, in the binary encoding; and the path of the part at
/// fault in the value.
///
/// Its `Display` and `Error` are written by hand rather than derived: its cause is its problem's.
#[derive(Debug)]
pub struct ReadError(Box<Detail>);

/// What a [`ReadError`] holds, boxed so that a result that holds an error stays small.
#[derive(Debug)]
struct Detail {
    file: Option<PathBuf>,
    at: Option<At>,
    path: Path,
    problem: ReadProblem,
}

/// Where the part at fault lies in a document.
#[derive(Clone, Copy, Debug)]
enum At {
    /// In a text: its line and its column, from 1, the column counted in characters.
    Text(usize, usize),
    /// In the binary encoding: its first byte, from 0.
    Byte(usize),
}

/// What is wrong, in a [`ReadError`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadProblem {
    /// The file cannot be read.
    #[error("cannot be read")]
    Io(#[source] std::io::Error),
    /// A document larger than the largest one read ([`MAX_SIZE`](crate::MAX_SIZE)).
    #[error("{size} bytes, more than the {limit} that a document may hold")]
    TooLarge {
        /// Its size, in bytes.
        size: u64,
        /// The largest size read.
        limit: usize,
    },
    /// Values nested deeper than the deepest read ([`MAX_DEPTH`](crate::MAX_DEPTH)).
    #[error("values nested deeper than the {limit} levels that a document may hold")]
    TooDeep {
        /// The deepest nesting read.
        limit: usize,
    },
    /// A file that is not text in UTF-8.
    #[error("not UTF-8 text")]
    NotText,
    /// Text that is not TOML: the parser's message.
    #[error("not TOML: {0}")]
    Syntax(String),
    /// Bytes that are not the engine's binary encoding of a document: what is wrong with them.
    #[error("not the binary encoding of a document: {0}")]
    Binary(String),
    /// A document of a version of the format newer than the one this program knows
    /// ([`format::VERSION`](crate::format::VERSION)).
    #[error(
        "a document of version {found} of the format, newer than version {known}, which this \
         program knows"
    )]
    NewerFormat {
        /// The version the document says.
        found: i64,
        /// The version this program writes and reads.
        known: u32,
    },
    /// A document in a version of the binary encoding newer than the one this program knows
    /// ([`format::BINARY_VERSION`](crate::format::BINARY_VERSION)).
    #[error(
        "a document in version {found} of the binary encoding, newer than version {known}, which \
         this program knows"
    )]
    NewerEncoding {
        /// The version the document says.
        found: u8,
        /// The version this program writes and reads.
        known: u8,
    },
    /// A value that is not one of its type, as reflection says: a kind, a field, a variant, a
    /// number out of its range, a version.
    #[error("{0}")]
    Value(Problem),
}

impl ReadError {
    pub(crate) fn new(at: Option<(usize, usize)>, path: Path, problem: ReadProblem) -> Self {
        Self(Box::new(Detail {
            file: None,
            at: at.map(|(line, column)| At::Text(line, column)),
            path,
            problem,
        }))
    }

    /// The error at the byte `at` of a document in the binary encoding.
    pub(crate) fn at_byte(at: usize, path: Path, problem: ReadProblem) -> Self {
        Self(Box::new(Detail {
            file: None,
            at: Some(At::Byte(at)),
            path,
            problem,
        }))
    }

    /// The error for `file`, which cannot be read.
    pub(crate) fn io(file: &FilePath, error: std::io::Error) -> Self {
        Self::new(None, Path::default(), ReadProblem::Io(error)).in_file(file)
    }

    /// The error, in `file`.
    pub(crate) fn in_file(mut self, file: &FilePath) -> Self {
        self.0.file = Some(file.to_owned());
        self
    }

    /// The file read, if one was.
    pub fn file(&self) -> Option<&FilePath> {
        self.0.file.as_deref()
    }

    /// The line and the column of the part at fault in the text, from 1, the column counted in
    /// characters; `None` when the problem has no place in a text, as when a file cannot be read, or
    /// when the document is in the binary encoding.
    pub fn line_and_column(&self) -> Option<(usize, usize)> {
        match self.0.at {
            Some(At::Text(line, column)) => Some((line, column)),
            _ => None,
        }
    }

    /// The byte of the part at fault in a document in the binary encoding, from 0; `None` for a
    /// text, or when the problem has no place in the bytes, as when they are too many.
    pub fn byte(&self) -> Option<usize> {
        match self.0.at {
            Some(At::Byte(at)) => Some(at),
            _ => None,
        }
    }

    /// The path of the part at fault in the value read, empty for the value itself.
    pub fn path(&self) -> &Path {
        &self.0.path
    }

    /// What is wrong.
    pub fn problem(&self) -> &ReadProblem {
        &self.0.problem
    }
}

/// `scene.toml:3:9: `light.intensity`: -1 is not ≥ 0`, or `byte 120: ` for the binary encoding,
/// each part there if known.
impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Detail {
            file,
            at,
            path,
            problem,
        } = &*self.0;
        match (file, at) {
            (Some(file), Some(At::Text(line, column))) => {
                write!(f, "{}:{line}:{column}: ", file.display())?
            }
            (Some(file), Some(At::Byte(at))) => write!(f, "{}: byte {at}: ", file.display())?,
            (Some(file), None) => write!(f, "{}: ", file.display())?,
            (None, Some(At::Text(line, column))) => write!(f, "line {line}, column {column}: ")?,
            (None, Some(At::Byte(at))) => write!(f, "byte {at}: ")?,
            (None, None) => {}
        }
        if !path.is_empty() {
            write!(f, "`{path}`: ")?;
        }
        write!(f, "{problem}")
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.0.problem)
    }
}
