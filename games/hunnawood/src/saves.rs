//! The game's saves (`docs/adr/0088-saving-and-resuming-a-run.md`): one file, `hunnawood/save.toml`
//! in the user's data directory unless `--save` names another, which holds what differs between the
//! run and a new game by day (`slime_world::World::save`), and the layout of the zone it was made on.
//! A save made on another layout is not resumed, as the run starts again when the layout changes
//! under hot reload.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use slime_assets::CookedScene;
use slime_world::{DocumentError, Light, Loaded, World};

use crate::rules::Game;
use crate::zone::Flickers;

/// The game's directory in the user's data directory.
const DIRECTORY: &str = "hunnawood";
/// The save's file in the game's directory.
const FILE: &str = "save.toml";
/// The key under which a save says the layout of the zone it was made on, in what it says of
/// itself ([`World::about`]).
const LAYOUT: &str = "layout";

/// The user's data directory, as `var` gives the environment's variables: `%APPDATA%` on Windows;
/// elsewhere `$XDG_DATA_HOME`, else `$HOME/.local/share`, as the XDG base directories say. `None`
/// when the environment gives none, or only relative paths, which the XDG base directories say to
/// ignore.
pub fn data_directory(windows: bool, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| {
        var(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    if windows {
        return absolute("APPDATA");
    }
    absolute("XDG_DATA_HOME").or_else(|| Some(absolute("HOME")?.join(".local").join("share")))
}

/// The save's file when `--save` names none: `hunnawood/save.toml` in the user's data directory, if
/// the environment gives one.
pub fn default_file() -> Option<PathBuf> {
    data_directory(cfg!(windows), |name| std::env::var_os(name))
        .map(|directory| directory.join(DIRECTORY).join(FILE))
}

/// A new game of the zone `zone` and the hero `hero`, by day, in a world of its own: the world that
/// saves differ from, whatever `--night` says, so that a save made at night is resumed at night.
///
/// # Errors
///
/// When the zone cannot be played, as [`Game::spawn`] says.
pub fn start(zone: &CookedScene, hero: &CookedScene) -> Result<(World, Game), String> {
    let mut world = World::new();
    let game = Game::spawn(&mut world, zone, hero, false)?;
    Ok((world, game))
}

/// The save of the run that `game` plays in `world`, a game of `zone` and `hero`: what differs from
/// a new game by day ([`start`]), and the layout of the zone. The lights of the campfire are put
/// back to the intensity they flicker around first, as the zone gives it, since the view makes them
/// flicker at each frame and a save would hold each frame's.
///
/// # Errors
///
/// When the zone cannot be played, or the world cannot be saved
/// ([`World::save`]).
pub fn save(
    world: &mut World,
    game: &Game,
    zone: &CookedScene,
    hero: &CookedScene,
) -> Result<String, String> {
    world.query_mut::<(&Flickers, &mut Light)>(|_, (flickers, light)| {
        if let Light::Point { intensity, .. } = light {
            *intensity = flickers.intensity;
        }
    });
    let (start, _) = start(zone, hero)?;
    let about = BTreeMap::from([(LAYOUT.to_owned(), game.zone().layout(world).fingerprint())]);
    world
        .save(&start, &about)
        .map_err(|error| slime_app::error_chain(&error))
}

/// Why a save is not resumed.
#[derive(Debug)]
pub enum NotResumed {
    /// It was made on another layout of the zone.
    OtherLayout,
    /// It is not a save that this program reads: not text, not TOML, of a later version.
    Unreadable(DocumentError),
}

/// The run that the save `text` holds, resumed in `start`, a new game by day ([`start`]), and what
/// loading it set aside.
///
/// # Errors
///
/// [`NotResumed::OtherLayout`] when the save says another layout of the zone, or none;
/// [`NotResumed::Unreadable`] when it cannot be read.
pub fn resume(text: &str, start: (World, Game)) -> Result<(World, Game, Loaded), NotResumed> {
    let about = World::about(text).map_err(NotResumed::Unreadable)?;
    let (mut world, game) = start;
    if about.get(LAYOUT) != Some(&game.zone().layout(&world).fingerprint()) {
        return Err(NotResumed::OtherLayout);
    }
    let loaded = world.load(text).map_err(NotResumed::Unreadable)?;
    Ok((world, game, loaded))
}

/// Writes `text` to `file`, its directory made if it has none, through a file beside it that is
/// then renamed over it, so that `file` holds the save before or the new one, never part of one.
///
/// # Errors
///
/// As the file system's operations.
pub fn write(file: &Path, text: &str) -> io::Result<()> {
    if let Some(directory) = file.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let written = beside(file, "new");
    std::fs::write(&written, text)?;
    std::fs::rename(&written, file)
}

/// Moves `file` aside, to `<file>.unreadable`, in place of the one set aside before, so that the
/// next save does not write over it; returns where.
///
/// # Errors
///
/// As the file system's operations.
pub fn set_aside(file: &Path) -> io::Result<PathBuf> {
    let aside = beside(file, "unreadable");
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
        let linux = |variables: &[(&str, &str)]| data_directory(false, environment(variables));
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
        let windows = |variables: &[(&str, &str)]| data_directory(true, environment(variables));
        assert_eq!(
            windows(&[("APPDATA", &roaming), ("XDG_DATA_HOME", &data)]),
            Some(PathBuf::from(&roaming))
        );
        assert_eq!(windows(&[("HOME", &home)]), None);
    }

    #[test]
    fn a_save_is_written_whole_or_not_at_all_and_set_aside_when_unreadable() {
        let directory =
            std::env::temp_dir().join(format!("hunnawood-saves-{}", std::process::id()));
        let file = directory.join("deep").join(FILE);
        write(&file, "first").expect("written, its directory made");
        write(&file, "second").expect("written over");
        assert_eq!(std::fs::read_to_string(&file).expect("read"), "second");
        assert!(
            !beside(&file, "new").exists(),
            "the file written beside it is renamed"
        );
        let aside = set_aside(&file).expect("set aside");
        assert_eq!(aside, beside(&file, "unreadable"));
        assert!(!file.exists());
        assert_eq!(std::fs::read_to_string(&aside).expect("read"), "second");
        std::fs::remove_dir_all(&directory).expect("removed");
    }
}

#[cfg(test)]
mod runs {
    use slime_render::glam::Vec2;

    use super::*;
    use crate::rules::tests::{no_hero, zone};

    #[test]
    fn a_run_is_resumed_only_from_a_save_of_its_layout_and_never_panics() {
        let plan = zone([0.0, 0.0], Vec::new());
        let scene = plan.scene();
        let new_game = || start(&scene, &no_hero()).expect("the zone can be played");
        let (mut world, game) = new_game();
        for _ in 0..30 {
            game.step(&mut world, Vec2::X, 1.0 / 60.0);
        }
        let text = save(&mut world, &game, &scene, &no_hero()).expect("saved");
        let (resumed, resumed_game, loaded) = resume(&text, new_game()).expect("resumed");
        assert!(loaded.set_aside.is_empty() && loaded.scenes_changed.is_empty());
        assert_eq!(world.first_difference(&resumed), None, "{text}");
        assert_eq!(resumed_game.hero(&resumed), game.hero(&world));
        // Another layout: the zone's start moved.
        let mut moved = plan.clone();
        moved.start = Vec2::new(1.0, 0.0);
        let other = moved.scene();
        let elsewhere = start(&other, &no_hero()).expect("the zone can be played");
        assert!(matches!(
            resume(&text, elsewhere),
            Err(NotResumed::OtherLayout)
        ));
        // Not a save, a save of a later format, one that says no layout.
        for refused in ["layout = [", "\"$format\" = 2\n", ""] {
            let resumed = resume(refused, new_game());
            assert!(
                matches!(
                    (refused, &resumed),
                    ("", Err(NotResumed::OtherLayout)) | (_, Err(NotResumed::Unreadable(_)))
                ),
                "{refused:?}: {:?}",
                resumed.as_ref().err()
            );
        }
        // Cut short anywhere, a save is refused or resumed, never with a panic.
        for end in (0..text.len()).filter(|&end| text.is_char_boundary(end)) {
            let _ = resume(&text[..end], new_game());
        }
    }
}
