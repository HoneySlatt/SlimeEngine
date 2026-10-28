//! Where a program keeps the files that it writes for the user, and how it holds one alone.

use std::ffi::OsString;
use std::fs::{File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

/// The user's data directory, where a program keeps the files that it writes for the user, a game's
/// saves or an editor's journals, each in a directory of its own: `%APPDATA%` on Windows; elsewhere
/// `$XDG_DATA_HOME`, else `$HOME/.local/share`, as the XDG base directories say. `None` when the
/// environment gives none, or only relative paths, which the XDG base directories say to ignore.
pub fn data_directory() -> Option<PathBuf> {
    directory_in(Directory::Data, cfg!(windows), |name| {
        std::env::var_os(name)
    })
}

/// The user's state directory, where a program keeps what it remembers of the user's work and
/// which the user would not copy to another machine, an editor's recent files or its layout, each
/// in a directory of its own: `%LOCALAPPDATA%` on Windows; elsewhere `$XDG_STATE_HOME`, else
/// `$HOME/.local/state`, as the XDG base directories say. `None` as [`data_directory`] says.
pub fn state_directory() -> Option<PathBuf> {
    directory_in(Directory::State, cfg!(windows), |name| {
        std::env::var_os(name)
    })
}

/// The user's directories that a program writes in.
#[derive(Clone, Copy)]
enum Directory {
    Data,
    State,
}

/// The `directory`, on Windows or elsewhere as `windows` says, as `var` gives the environment's
/// variables.
fn directory_in(
    directory: Directory,
    windows: bool,
    var: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    let absolute = |name: &str| {
        var(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let (windows_variable, variable, below_home) = match directory {
        Directory::Data => ("APPDATA", "XDG_DATA_HOME", "share"),
        Directory::State => ("LOCALAPPDATA", "XDG_STATE_HOME", "state"),
    };
    if windows {
        return absolute(windows_variable);
    }
    absolute(variable).or_else(|| Some(absolute("HOME")?.join(".local").join(below_home)))
}

/// A file that this program holds, so that one program at a time writes it, a game's save or an
/// editor's journal: no other holds it while this one does. It holds the lock of a file beside it,
/// `<file>.lock`, which the system lets go when the program ends, however it ends, a crash
/// included; the lock file stays, empty.
#[derive(Debug)]
pub struct HeldFile {
    path: PathBuf,
    /// The lock file, locked while this program holds the file.
    _lock: File,
}

impl HeldFile {
    /// Holds the file `path`, its directory made if it has none.
    ///
    /// # Errors
    ///
    /// When another program holds it, or its lock file cannot be made or locked, as on a file
    /// system without locks: a message that says why.
    pub fn hold(path: PathBuf) -> Result<Self, String> {
        let shown = path.display();
        let cannot = |error: io::Error| format!("{shown} cannot be held: {error}");
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory).map_err(cannot)?;
        }
        let lock = File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(beside(&path, "lock"))
            .map_err(cannot)?;
        match lock.try_lock() {
            Ok(()) => Ok(Self { path, _lock: lock }),
            Err(TryLockError::WouldBlock) => Err(format!(
                "another program holds {shown}, and writes it: a copy of this one, or of another"
            )),
            Err(TryLockError::Error(error)) => Err(cannot(error)),
        }
    }

    /// The file held.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Moves `file` aside, to `<file>.<why>`, or `<file>.<why>.2`, `.3` and on, the first of these
/// names that nothing has, so that a file set aside later does not write over it, and returns where:
/// a game's save that it does not resume, an editor's journal that it does not recover. The program
/// that holds `file` ([`HeldFile`]) is the one that takes the name.
///
/// # Errors
///
/// As the file system's operations.
pub fn set_aside(file: &Path, why: &str) -> io::Result<PathBuf> {
    let aside = (1..=u32::MAX)
        .map(|count| match count {
            1 => beside(file, why),
            _ => beside(file, &format!("{why}.{count}")),
        })
        .find(|aside| std::fs::symlink_metadata(aside).is_err())
        .ok_or_else(|| io::Error::other("every name to set it aside under is taken"))?;
    std::fs::rename(file, &aside)?;
    Ok(aside)
}

/// The file beside `file` whose name is `file`'s and `.<extension>`.
fn beside(file: &Path, extension: &str) -> PathBuf {
    let mut name = file.as_os_str().to_owned();
    name.push(".");
    name.push(extension);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The environment of `variables`, names and values.
    fn environment(variables: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let variables: Vec<(String, OsString)> = variables
            .iter()
            .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
            .collect();
        move |name| {
            variables
                .iter()
                .find(|(variable, _)| variable == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn the_data_directory_is_the_one_the_environment_gives() {
        // Absolute on the platform that runs the test: Windows wants a drive.
        let at = |path: &str| {
            if cfg!(windows) {
                format!("C:{path}")
            } else {
                path.to_owned()
            }
        };
        let (data, home) = (at("/data"), at("/home/player"));
        let linux = |variables: &[(&str, &str)]| {
            directory_in(Directory::Data, false, environment(variables))
        };
        assert_eq!(
            linux(&[("XDG_DATA_HOME", &data), ("HOME", &home)]),
            Some(PathBuf::from(&data))
        );
        let share = Some(PathBuf::from(&home).join(".local").join("share"));
        assert_eq!(linux(&[("HOME", &home)]), share);
        // A relative path is ignored, as the XDG base directories say.
        assert_eq!(linux(&[("XDG_DATA_HOME", "data"), ("HOME", &home)]), share);
        assert_eq!(linux(&[]), None);
        let roaming = at("/Users/player/AppData/Roaming");
        let windows = |variables: &[(&str, &str)]| {
            directory_in(Directory::Data, true, environment(variables))
        };
        assert_eq!(
            windows(&[("APPDATA", &roaming), ("XDG_DATA_HOME", &data)]),
            Some(PathBuf::from(&roaming))
        );
        assert_eq!(windows(&[("HOME", &home)]), None);
    }

    #[test]
    fn the_state_directory_is_the_one_the_environment_gives() {
        let at = |path: &str| {
            if cfg!(windows) {
                format!("C:{path}")
            } else {
                path.to_owned()
            }
        };
        let (state, home) = (at("/state"), at("/home/player"));
        let linux = |variables: &[(&str, &str)]| {
            directory_in(Directory::State, false, environment(variables))
        };
        assert_eq!(
            linux(&[("XDG_STATE_HOME", &state), ("XDG_DATA_HOME", &at("/data"))]),
            Some(PathBuf::from(&state))
        );
        let below = Some(PathBuf::from(&home).join(".local").join("state"));
        assert_eq!(linux(&[("HOME", &home)]), below);
        assert_eq!(
            linux(&[("XDG_STATE_HOME", "state"), ("HOME", &home)]),
            below
        );
        let local = at("/Users/player/AppData/Local");
        let windows = |variables: &[(&str, &str)]| {
            directory_in(Directory::State, true, environment(variables))
        };
        assert_eq!(
            windows(&[("LOCALAPPDATA", &local), ("APPDATA", &at("/Roaming"))]),
            Some(PathBuf::from(&local))
        );
        assert_eq!(windows(&[("APPDATA", &at("/Roaming"))]), None);
    }
    #[test]
    fn one_program_at_a_time_holds_a_file() {
        let directory = std::env::temp_dir().join(format!("slime-app-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        let file = directory.join("deep").join("save.toml");
        let held = HeldFile::hold(file.clone()).expect("held, its directory made");
        assert_eq!(held.path(), file);
        let Err(why) = HeldFile::hold(file.clone()) else {
            panic!("a file held twice");
        };
        assert!(why.contains("another program holds"), "{why}");
        // Another file beside it is another file.
        HeldFile::hold(directory.join("other.toml")).expect("held");
        drop(held);
        HeldFile::hold(file).expect("held once let go");
        std::fs::remove_dir_all(&directory).expect("removed");
    }
    #[test]
    fn files_set_aside_keep_names_of_their_own() {
        let directory =
            std::env::temp_dir().join(format!("slime-app-aside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a directory");
        let file = directory.join("save.toml");
        let mut names = Vec::new();
        for (text, why) in [
            ("first", "unreadable"),
            ("second", "unreadable"),
            ("third", "other-layout"),
            ("fourth", "unreadable"),
        ] {
            std::fs::write(&file, text).expect("written");
            let aside = set_aside(&file, why).expect("set aside");
            assert!(!file.exists());
            assert_eq!(std::fs::read_to_string(&aside).expect("kept"), text);
            names.push(
                aside
                    .file_name()
                    .expect("a name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        assert_eq!(
            names,
            [
                "save.toml.unreadable",
                "save.toml.unreadable.2",
                "save.toml.other-layout",
                "save.toml.unreadable.3"
            ]
        );
        std::fs::remove_dir_all(&directory).expect("removed");
    }
}
