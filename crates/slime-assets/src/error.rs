//! Why cooked data cannot be read.

use std::io;
use std::path::PathBuf;

use crate::AssetId;

/// Why a cooked file cannot be read: it is not what it claims to be, or another version of the
/// cooker wrote it.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CookedError {
    /// The file is not of its kind, or is damaged.
    #[error("the {what} is not one the cooker writes: {reason}")]
    Malformed {
        /// What the file was read as: `texture`, `manifest`.
        what: &'static str,
        /// What is wrong.
        reason: String,
    },
    /// The file was cooked by another version of the cooker, whose format this engine does not
    /// read: cooking again replaces it.
    #[error(
        "the {what} was cooked in version {found} of its format, and this engine reads version \
         {expected}: cook again with `slime-cook cook`"
    )]
    Version {
        /// What the file was read as.
        what: &'static str,
        /// The version the file holds.
        found: u32,
        /// The version this engine reads.
        expected: u32,
    },
}

/// Why an asset cannot be loaded from a directory of cooked data.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// A file of the directory cannot be read: the directory was not cooked, or not entirely.
    #[error("{}: {source}; cook it with `slime-cook cook`", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The operating system's error.
        source: io::Error,
    },
    /// A file of the directory is not what the manifest says it is.
    #[error("{}: {source}", path.display())]
    Cooked {
        /// The file.
        path: PathBuf,
        /// What is wrong with it.
        source: CookedError,
    },
    /// The manifest lists no asset of this ID: it was not cooked, or not for this game.
    #[error("no asset {id} was cooked into {}", directory.display())]
    NotCooked {
        /// The ID asked for.
        id: AssetId,
        /// The directory of cooked data.
        directory: PathBuf,
    },
    /// The asset of this ID is of another kind than asked.
    #[error("the asset {id} is a {found}, not a {expected}")]
    WrongKind {
        /// The ID asked for.
        id: AssetId,
        /// Its kind, as the manifest says.
        found: &'static str,
        /// The kind asked for.
        expected: &'static str,
    },
}
