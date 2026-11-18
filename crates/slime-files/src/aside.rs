//! A file that does not read, moved aside rather than lost.

use std::io;
use std::path::{Path, PathBuf};

use crate::beside;

/// Moves `file` aside, to `<file>.<why>`, or `<file>.<why>.2`, `.3` and on, the first of these
/// names that nothing has, so that a file set aside later does not write over it, and returns where:
/// a game's save that it does not resume, an editor's journal that it does not recover. The program
/// that holds `file` ([`HeldFile`](crate::HeldFile)) is the one that takes the name.
///
/// # Errors
///
/// As the file system's operations, `file` left where it was.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_set_aside_keep_names_of_their_own() {
        let directory = crate::test_directory("aside");
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
