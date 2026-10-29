//! The flying speed last used in each level, at which the viewports fly as the level opens again:
//! a file of the user's state directory, a line per level, its speed in metres per second, a tab
//! and the level's whole path, the last flown first.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// How many levels the file keeps.
pub(crate) const KEPT: usize = 100;

/// The speeds that `file` holds, each with its level; none when it does not exist. A line that is
/// not a speed and a path is passed over.
///
/// # Errors
///
/// When the file exists and cannot be read as text.
pub(crate) fn read(file: &Path) -> io::Result<Vec<(PathBuf, f64)>> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text
            .lines()
            .filter_map(|line| {
                let (speed, level) = line.split_once('\t')?;
                Some((PathBuf::from(level), speed.parse().ok()?))
            })
            .take(KEPT)
            .collect()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// The speed that `speeds` holds for `level`, if any.
pub(crate) fn of(speeds: &[(PathBuf, f64)], level: &Path) -> Option<f64> {
    speeds
        .iter()
        .find(|(each, _)| each == level)
        .map(|&(_, speed)| speed)
}

/// `speeds` with `level` flown at `speed` first, once, and at most [`KEPT`] of them.
pub(crate) fn flown(speeds: &[(PathBuf, f64)], level: &Path, speed: f64) -> Vec<(PathBuf, f64)> {
    std::iter::once((level.to_owned(), speed))
        .chain(speeds.iter().filter(|(each, _)| each != level).cloned())
        .take(KEPT)
        .collect()
}

/// Writes `speeds` to `file`, whole or not at all, its directory made if it has none.
///
/// # Errors
///
/// When the directory cannot be made or the file written.
pub(crate) fn write(file: &Path, speeds: &[(PathBuf, f64)]) -> io::Result<()> {
    if let Some(directory) = file.parent() {
        fs::create_dir_all(directory)?;
    }
    let mut text = String::new();
    for (level, speed) in speeds {
        text.push_str(&format!("{speed}\t{}\n", level.display()));
    }
    slime_cook::write(file, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_level_flown_comes_first_once_with_its_speed() {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-speeds-{}", std::process::id()));
        let file = directory.join("state").join("speeds");
        assert!(read(&file).expect("none yet").is_empty());
        let speeds = flown(&[], Path::new("/levels/yard.scene"), 5.0);
        let speeds = flown(&speeds, Path::new("/levels/zone.scene"), 80.0);
        let speeds = flown(&speeds, Path::new("/levels/yard.scene"), 7.2);
        write(&file, &speeds).expect("written");
        let read = read(&file).expect("read");
        assert_eq!(
            read,
            [
                (PathBuf::from("/levels/yard.scene"), 7.2),
                (PathBuf::from("/levels/zone.scene"), 80.0),
            ]
        );
        assert_eq!(of(&read, Path::new("/levels/zone.scene")), Some(80.0));
        assert_eq!(of(&read, Path::new("/levels/other.scene")), None);
        fs::write(&file, "fast\t/levels/a.scene\n12\n").expect("written");
        assert!(
            super::read(&file).expect("read").is_empty(),
            "lines passed over"
        );
        let _ = fs::remove_dir_all(&directory);
    }
}
