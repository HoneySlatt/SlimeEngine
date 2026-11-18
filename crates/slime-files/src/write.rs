//! A file written whole or not at all.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::beside;

/// The number of the next file written beside its target in this process, so that two threads
/// that write the same file at once each write a file of their own.
static NEXT_PART: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` whole or not at all: `path` holds what it held before, or `bytes`,
/// never a part of them, when the program is stopped or the power cut half-way.
///
/// The bytes go to a file beside it, `<path>.<process>-<count>.part`, forced to the disk, which is
/// then renamed to `path`; its directory is then synced, so that the new name is on the disk too
/// once `write` returns, where the system allows it:
///
/// - On Linux and the other Unix systems, the directory is synced. A file system that cannot sync
///   a directory, which refuses it as an invalid or unsupported request, gives no error.
/// - On Windows, the directory is flushed through a handle open for writing, which NTFS alone
///   turns into its names on the disk (Microsoft's MS-FSA, 2.1.5.7); a directory whose handle the
///   system refuses gives no error. Microsoft promises no more for a rename: NTFS logs it, so a
///   crash leaves the old file or the new one whole, but a power cut just after `write` returns
///   may bring the old one back. `std::fs::rename` asks for no write-through.
///
/// Directories missing above `path` are made, and each directory that holds a new one synced.
/// Two processes, or two threads, that write the same file at once each write a file of their
/// own; the last renamed wins. The file beside it does not outlive an error; a crash or a power
/// cut half-way leaves it beside `path`, where nothing reads it.
///
/// # Errors
///
/// The file system's error, when a directory cannot be made, the file beside it created, written,
/// forced to the disk or renamed, `path` as it was; or when its directory cannot be synced, `path`
/// already holding `bytes`, which a power cut may then undo.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let directory = parent(path);
    make_directories(directory)?;
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
        return Err(error);
    }
    sync_directory(directory)
}

/// The directory that holds `path`: `.` for a bare name.
fn parent(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Makes `directory` and those missing above it, each synced into the one that holds it.
fn make_directories(directory: &Path) -> io::Result<()> {
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

/// Forces `directory`'s names to the disk, where the system allows it.
#[cfg(unix)]
fn sync_directory(directory: &Path) -> io::Result<()> {
    match File::open(directory).and_then(|directory| directory.sync_all()) {
        // EINVAL: a file system that cannot sync a directory, as some network and FUSE ones.
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

/// Forces `directory`'s names to the disk, where the system allows it: through a handle of the
/// directory open for writing, which `FlushFileBuffers` needs, and which only
/// `FILE_FLAG_BACKUP_SEMANTICS` opens. A directory whose handle the system refuses, as one where
/// the user may not add files, is not synced.
#[cfg(windows)]
fn sync_directory(directory: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    /// Opens a directory rather than refusing it (`CreateFileW`'s flags).
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    match File::options()
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(directory)
        .and_then(|directory| directory.sync_all())
    {
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => Ok(()),
        synced => synced,
    }
}

/// Forces `directory`'s names to the disk where the system allows it: on a system neither Unix nor
/// Windows, which the engine does not build for yet, nothing.
#[cfg(not(any(unix, windows)))]
fn sync_directory(_directory: &Path) -> io::Result<()> {
    Ok(())
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
        assert!(write(&file, b"lost").is_err());
        assert_eq!(names(&directory), ["save.toml"]);
        assert_eq!(
            fs::read_to_string(file.join("inside")).expect("kept"),
            "kept"
        );
        // A file where a directory above it would be: nothing is made.
        let blocked = directory.join("blocked");
        fs::write(&blocked, "a file").expect("written");
        assert!(write(&blocked.join("save.toml"), b"lost").is_err());
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
    fn a_bare_name_is_written_in_the_current_directory() {
        assert_eq!(parent(Path::new("save.toml")), Path::new("."));
        assert_eq!(parent(Path::new("saves/save.toml")), Path::new("saves"));
    }
}
