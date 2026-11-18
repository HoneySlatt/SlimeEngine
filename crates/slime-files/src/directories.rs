//! Where a program keeps the files that it writes for the user.

use std::ffi::OsString;
use std::path::PathBuf;

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
}
