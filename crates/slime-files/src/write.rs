//! A file written whole or not at all.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::beside;

/// The number of the next file written beside its target in this process, so that two threads
/// that write the same file at once each write a file of their own.
static NEXT_PART: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` whole or not at all: `path` holds what it held before, or `bytes`,
/// never a part of them, when the program is stopped or the power cut half-way.
///
/// The bytes go to a file beside it, forced to the disk, which is then renamed to `path`; its
/// directory is then synced, so that the new name is on the disk too once `write` returns, where the
/// system allows it:
///
/// - On Linux and the other Unix systems, the directory is synced, as POSIX asks for a rename to
///   last.
/// - On Windows, the directory is flushed through a handle open for writing, the request that
///   Microsoft's MS-FSA (2.1.5.7) describes as persisting a directory's structure, on NTFS alone.
///   Microsoft promises nothing more of a rename's durability: a power cut just after `write`
///   returns may bring the old file back, whole. `std::fs::rename` asks for no write-through.
/// - A file system that refuses to sync a directory as an invalid or unsupported request, as some
///   network and FUSE ones do, gives no error.
///
/// Directories missing above `path` are made, each synced into the one that holds it. Two
/// processes, or two threads, that write the same file at once each write a file of their own; the
/// last renamed wins. The file beside it does not outlive an error; a crash or a power cut half-way
/// leaves it beside `path`, where nothing reads it. A symbolic link at `path` is replaced by the
/// file, not followed, and the file takes a new file's permissions, not those of the file it
/// replaces.
///
/// # Errors
///
/// [`WriteError::NotWritten`] when a directory cannot be made, or the file beside it created,
/// written, forced to the disk or renamed: `path` as it was. [`WriteError::NotSynced`] when the
/// file is renamed but its directory cannot be synced: `path` holds `bytes`, which a power cut may
/// still undo.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), WriteError> {
    let not_written = |source| WriteError::NotWritten {
        path: path.to_owned(),
        source,
    };
    let directory = parent(path);
    make_directories(directory).map_err(not_written)?;
    let part = beside(
        path,
        &format!(
            "{}-{}.part",
            std::process::id(),
            NEXT_PART.fetch_add(1, Ordering::Relaxed)
        ),
    );
    let written = File::create(&part)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            // On the disk before the rename, which a file system may otherwise put there first.
            file.sync_all()
        })
        .and_then(|()| fs::rename(&part, path));
    if let Err(error) = written {
        let _ = fs::remove_file(&part);
        return Err(not_written(error));
    }
    sync_directory(directory).map_err(|source| WriteError::NotSynced {
        path: path.to_owned(),
        source,
    })
}

/// Why [`write()`] did not write a file whole and durably.
#[derive(Debug)]
#[non_exhaustive]
pub enum WriteError {
    /// The file is not written: it holds what it held before, or does not exist.
    NotWritten {
        /// The file.
        path: PathBuf,
        /// The file system's error.
        source: io::Error,
    },
    /// The file is written, but its directory cannot be synced: a power cut may still bring back
    /// what it held before. A caller takes the file as written, and may tell the user.
    NotSynced {
        /// The file.
        path: PathBuf,
        /// The file system's error.
        source: io::Error,
    },
}

impl WriteError {
    /// Whether the file holds the bytes given, though not durably ([`WriteError::NotSynced`]).
    pub fn is_written(&self) -> bool {
        matches!(self, Self::NotSynced { .. })
    }

    /// The file system's error.
    pub fn io_error(&self) -> &io::Error {
        match self {
            Self::NotWritten { source, .. } | Self::NotSynced { source, .. } => source,
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotWritten { path, source } => {
                write!(f, "{} cannot be written: {source}", path.display())
            }
            Self::NotSynced { path, source } => write!(
                f,
                "{} is written, but its directory cannot be synced, so that a power cut may bring \
                 back what it held before: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.io_error())
    }
}

/// An error of [`write()`] as the file system's, its kind kept, for callers that return
/// [`io::Error`].
impl From<WriteError> for io::Error {
    fn from(error: WriteError) -> Self {
        Self::new(error.io_error().kind(), error)
    }
}

/// The directory that holds `path`: `.` for a bare name.
pub(crate) fn parent(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Makes `directory` and those missing above it, each synced into the one that holds it.
pub(crate) fn make_directories(directory: &Path) -> io::Result<()> {
    if directory.is_dir() {
        return Ok(());
    }
    let above = parent(directory);
    // The root and a bare name's `.` are directories, which ends the recursion at the latest there.
    if above != directory {
        make_directories(above)?;
    }
    match fs::create_dir(directory) {
        Ok(()) => sync_directory(above),
        // Made by another program or thread meanwhile.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && directory.is_dir() => Ok(()),
        Err(error) => Err(error),
    }
}

/// Forces `directory`'s names to the disk, where the system allows it ([`write()`]).
fn sync_directory(directory: &Path) -> io::Result<()> {
    match open_directory(directory).and_then(|directory| directory.sync_all()) {
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::InvalidInput | io::ErrorKind::Unsupported
            ) =>
        {
            Ok(())
        }
        synced => synced,
    }
}

/// `directory`, open so that it can be synced.
#[cfg(unix)]
fn open_directory(directory: &Path) -> io::Result<File> {
    File::open(directory)
}

/// `directory`, open so that it can be synced: for writing, which `FlushFileBuffers` needs, with
/// `FILE_FLAG_BACKUP_SEMANTICS`, without which Windows opens no directory.
#[cfg(windows)]
fn open_directory(directory: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    /// Opens a directory rather than refusing it (`CreateFileW`'s flags).
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    File::options()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The files in `directory`, by name, sorted.
    fn names(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .expect("listed")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_file_is_written_whole_through_a_file_of_its_own_its_directories_made() {
        let directory = crate::test_directory("written");
        let file = directory.join("deep").join("deeper").join("save.toml");
        write(&file, b"first").expect("written, its directories made");
        assert_eq!(fs::read(&file).expect("read"), b"first");
        // Another writer half-way through its write: this one writes through a file of its own,
        // and leaves the other's.
        let others = beside(&file, "1-0.part");
        fs::write(&others, "another writer's").expect("written");
        write(&file, b"second").expect("written over");
        assert_eq!(fs::read(&file).expect("read"), b"second");
        assert_eq!(
            fs::read_to_string(&others).expect("left"),
            "another writer's"
        );
        // Nothing but the file and the other writer's is left beside it.
        assert_eq!(
            names(&directory.join("deep").join("deeper")),
            ["save.toml", "save.toml.1-0.part"]
        );
        write(&file, b"").expect("emptied");
        assert_eq!(fs::read(&file).expect("read"), b"");
        fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_file_that_cannot_be_written_is_left_as_it_was_with_nothing_beside_it() {
        let directory = crate::test_directory("unwritten");
        // A directory in the file's place: the rename fails once the file beside it is written.
        let file = directory.join("save.toml");
        fs::create_dir(&file).expect("a directory");
        fs::write(file.join("inside"), "kept").expect("written");
        assert!(matches!(
            write(&file, b"lost"),
            Err(WriteError::NotWritten { .. })
        ));
        assert_eq!(names(&directory), ["save.toml"]);
        assert_eq!(
            fs::read_to_string(file.join("inside")).expect("kept"),
            "kept"
        );
        // A file where a directory above it would be: nothing is made.
        let blocked = directory.join("blocked");
        fs::write(&blocked, "a file").expect("written");
        let error = write(&blocked.join("save.toml"), b"lost").expect_err("not written");
        assert!(!error.is_written(), "{error}");
        assert_eq!(fs::read_to_string(&blocked).expect("kept"), "a file");
        fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn threads_that_write_one_file_at_once_each_write_it_whole() {
        let directory = crate::test_directory("threads");
        let file = directory.join("save.toml");
        let texts: Vec<Vec<u8>> = (0..8u8).map(|thread| vec![b'a' + thread; 4096]).collect();
        std::thread::scope(|scope| {
            for text in &texts {
                let file = &file;
                scope.spawn(move || {
                    for _ in 0..16 {
                        write(file, text).expect("written");
                    }
                });
            }
        });
        assert!(texts.contains(&fs::read(&file).expect("read")));
        assert_eq!(names(&directory), ["save.toml"]);
        fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_bare_name_lies_in_the_current_directory() {
        assert_eq!(parent(Path::new("save.toml")), Path::new("."));
        assert_eq!(parent(Path::new("saves/save.toml")), Path::new("saves"));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_whose_directory_cannot_be_synced_is_written_and_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let directory = crate::test_directory("unsynced");
        let file = directory.join("save.toml");
        // A directory that its user may add files to but not read, so not open to sync.
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o300)).expect("its mode");
        let readable = fs::read_dir(&directory).is_ok();
        let written = write(&file, b"after");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).expect("its mode");
        // A user whom no mode stops, as root, opens it all the same.
        if !readable {
            let error = written.expect_err("not synced");
            assert!(error.is_written(), "{error}");
            assert!(error.to_string().contains("is written, but"), "{error}");
            let io: io::Error = error.into();
            assert_eq!(io.kind(), io::ErrorKind::PermissionDenied);
        }
        assert_eq!(fs::read(&file).expect("read"), b"after");
        fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_directory_of_the_test_s_own_is_synced() {
        // On Windows, the only check that the handle of a directory opens as `open_directory` asks.
        let directory = crate::test_directory("synced");
        sync_directory(&directory).expect("synced");
        fs::remove_dir_all(&directory).expect("removed");
    }

    /// What syncing the directory adds to a write whole, by size: `docs/measurements.md`, "S1a,
    /// safe files". Run at rest: `cargo test --release -p slime-files write_timed -- --ignored
    /// --nocapture`, with `TMPDIR` on the disk to measure.
    #[test]
    #[ignore = "a measure, run by hand at rest"]
    fn write_timed() {
        let directory = crate::test_directory("timed");
        for (size, count) in [(4096, 300), (1 << 20, 100)] {
            let bytes = vec![7u8; size];
            let mut times = [Vec::new(), Vec::new()];
            for index in 0..count {
                let file = directory.join(format!("{}", index % 50));
                // Alternated, so that both see the same load.
                for (synced, times) in times.iter_mut().enumerate() {
                    let start = std::time::Instant::now();
                    if synced == 1 {
                        write(&file, &bytes).expect("written");
                    } else {
                        let part = beside(&file, "part");
                        let mut written = File::create(&part).expect("created");
                        written.write_all(&bytes).expect("written");
                        written.sync_all().expect("synced");
                        fs::rename(&part, &file).expect("renamed");
                    }
                    times.push(start.elapsed());
                }
            }
            for (synced, times) in times.iter_mut().enumerate() {
                times.sort();
                println!(
                    "{size} bytes, directory synced {}: median {:?}, 90th percentile {:?}",
                    synced == 1,
                    times[times.len() / 2],
                    times[times.len() * 9 / 10]
                );
            }
        }
        fs::remove_dir_all(&directory).expect("removed");
    }
}
