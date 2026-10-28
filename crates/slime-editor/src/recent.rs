//! The levels that a program of the editor opened last, which its File menu lists
//! (`docs/adr/0156-the-editors-window-and-its-operations.md`): a file of the user's state
//! directory, one whole path per line, the last opened first.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// How many levels the list keeps.
pub(crate) const KEPT: usize = 10;

/// The levels that `file` lists; none when it does not exist. A line that is empty is passed over.
///
/// # Errors
///
/// When the file exists and cannot be read as text.
pub(crate) fn read(file: &Path) -> io::Result<Vec<PathBuf>> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text
            .lines()
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .take(KEPT)
            .collect()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// `levels` with `opened` first, once, and at most [`KEPT`] of them.
pub(crate) fn opened(levels: &[PathBuf], opened: &Path) -> Vec<PathBuf> {
    std::iter::once(opened.to_owned())
        .chain(levels.iter().filter(|level| *level != opened).cloned())
        .take(KEPT)
        .collect()
}

/// Writes `levels` to `file`, whole or not at all, its directory made if it has none.
///
/// # Errors
///
/// When the directory cannot be made or the file written.
pub(crate) fn write(file: &Path, levels: &[PathBuf]) -> io::Result<()> {
    if let Some(directory) = file.parent() {
        fs::create_dir_all(directory)?;
    }
    let mut text = String::new();
    for level in levels {
        text.push_str(&level.display().to_string());
        text.push('\n');
    }
    slime_cook::write(file, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_level_opened_comes_first_once_and_the_list_keeps_ten() {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-recent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let file = directory.join("deep").join("editor.recent");
        assert_eq!(read(&file).expect("none yet"), Vec::<PathBuf>::new());
        let mut levels = Vec::new();
        for number in 0..12 {
            levels = opened(&levels, Path::new(&format!("/levels/{number}.scene")));
        }
        levels = opened(&levels, Path::new("/levels/5.scene"));
        write(&file, &levels).expect("written");
        let read = read(&file).expect("read");
        assert_eq!(read.len(), KEPT);
        assert_eq!(read[0], Path::new("/levels/5.scene"));
        assert_eq!(read[1], Path::new("/levels/11.scene"));
        assert_eq!(
            read.iter()
                .filter(|level| level.ends_with("5.scene"))
                .count(),
            1
        );
        fs::remove_dir_all(&directory).expect("removed");
    }
}
