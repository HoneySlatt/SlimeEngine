//! A file that one program at a time holds.

use std::fmt;
use std::fs::{File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use crate::{beside, write};

/// A file that this program holds, so that one program at a time writes it, a game's save or an
/// editor's journal: no other holds it while this one does. It holds the lock of a file beside it,
/// `<file>.lock`, which the system lets go when the program ends, however it ends, a crash
/// included; the lock file stays, empty.
///
/// Holding a file is a promise between the programs that hold it: the system does not stop a
/// program that writes the file without holding it.
#[derive(Debug)]
pub struct HeldFile {
    path: PathBuf,
    /// The lock file, locked while this program holds the file.
    _lock: File,
}

impl HeldFile {
    /// Holds the file `path`, its directory made if it has none, as [`write`](fn@crate::write) makes
    /// it, each directory made synced into the one that holds it.
    ///
    /// # Errors
    ///
    /// [`HoldError::Held`] when another program holds it; [`HoldError::Io`] when its directory or
    /// its lock file cannot be made, a directory made synced, or the lock taken, as on a file system
    /// without locks.
    pub fn hold(path: PathBuf) -> Result<Self, HoldError> {
        let cannot = |source: io::Error| HoldError::Io {
            path: path.clone(),
            source,
        };
        write::make_directories(write::parent(&path)).map_err(cannot)?;
        let lock = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(beside(&path, "lock"))
            .map_err(cannot)?;
        match lock.try_lock() {
            Ok(()) => Ok(Self { path, _lock: lock }),
            Err(TryLockError::WouldBlock) => Err(HoldError::Held { path }),
            Err(TryLockError::Error(source)) => Err(cannot(source)),
        }
    }

    /// The file held.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Why a file cannot be held ([`HeldFile::hold`]).
#[derive(Debug)]
#[non_exhaustive]
pub enum HoldError {
    /// Another program holds the file, and writes it: a copy of this one, or of another.
    Held {
        /// The file.
        path: PathBuf,
    },
    /// The file's directory or its lock file cannot be made, or the lock taken.
    Io {
        /// The file.
        path: PathBuf,
        /// The file system's error.
        source: io::Error,
    },
}

impl fmt::Display for HoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held { path } => write!(
                f,
                "another program holds {}, and writes it: a copy of this one, or of another",
                path.display()
            ),
            Self::Io { path, source } => write!(f, "{} cannot be held: {source}", path.display()),
        }
    }
}

impl std::error::Error for HoldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Held { .. } => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_program_at_a_time_holds_a_file() {
        let directory = crate::test_directory("held");
        let file = directory.join("deep").join("save.toml");
        let held = HeldFile::hold(file.clone()).expect("held, its directory made");
        assert_eq!(held.path(), file);
        let Err(HoldError::Held { path }) = HeldFile::hold(file.clone()) else {
            panic!("a file held twice");
        };
        assert_eq!(path, file);
        // Another file beside it is another file.
        HeldFile::hold(directory.join("other.toml")).expect("held");
        drop(held);
        HeldFile::hold(file).expect("held once let go");
        std::fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_file_whose_directory_cannot_be_made_is_not_held_and_says_why() {
        let directory = crate::test_directory("unheld");
        // A file where its directory would be.
        let blocking = directory.join("save");
        std::fs::write(&blocking, "").expect("written");
        let file = blocking.join("save.toml");
        let Err(error @ HoldError::Io { .. }) = HeldFile::hold(file.clone()) else {
            panic!("a file held below a file");
        };
        assert!(std::error::Error::source(&error).is_some());
        assert!(
            error
                .to_string()
                .starts_with(&format!("{} cannot be held: ", file.display())),
            "{error}"
        );
        std::fs::remove_dir_all(&directory).expect("removed");
    }
}
