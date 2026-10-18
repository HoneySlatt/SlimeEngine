//! The game's saves (`docs/adr/0088-saving-and-resuming-a-run.md`): one file, `hunnawood/save.toml`
//! in the user's data directory unless `--save` names another, which holds what differs between the
//! run and a new game by day (`slime_world::World::save`), and the layout of the zone it was made on.
//! A save made on another layout is not resumed, as the run starts again when the layout changes
//! under hot reload, which carries the run through a save otherwise ([`reload`]).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use slime_world::{DocumentError, Light, Loaded, SceneLibrary, World};

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

/// A new game of the zone and the hero as `scenes` resolves them, by day, in a world of its own:
/// the world that saves differ from, whatever `--night` says, so that a save made at night is
/// resumed at night.
///
/// # Errors
///
/// When the zone cannot be played, as [`Game::spawn`] says.
pub fn start(scenes: &mut SceneLibrary) -> Result<(World, Game), String> {
    let mut world = World::new();
    let game = Game::spawn(&mut world, scenes, false)?;
    Ok((world, game))
}

/// The save of the run in `world`, a game of `scenes`: what differs from a new game by day
/// ([`start`]), and the fingerprint of that new game's layout, the zone's as its scenes place it,
/// whatever the run changed of it. The lights of the campfire are put back to the intensity they
/// flicker around first, as the zone gives it, since the view makes them flicker at each frame and
/// a save would hold each frame's.
///
/// # Errors
///
/// When the zone cannot be played, or the world cannot be saved ([`World::save`]).
pub fn save(world: &mut World, scenes: &mut SceneLibrary) -> Result<String, String> {
    unflicker(world);
    let (start, start_game) = start(scenes)?;
    let layout = start_game.zone().layout(&start).fingerprint();
    let about = BTreeMap::from([(LAYOUT.to_owned(), layout)]);
    world
        .save(&start, &about)
        .map_err(|error| slime_app::error_chain(&error))
}

/// Why a save is not resumed.
#[derive(Debug)]
pub enum NotResumed {
    /// It was made on another layout of the zone.
    OtherLayout,
    /// It is not a save that this program reads: not TOML, of a later version, not a save.
    Unreadable(DocumentError),
}

/// The run that the save `text` holds, resumed in `start`, a new game by day of `scenes`
/// ([`start`]), and what loading it set aside.
///
/// # Errors
///
/// [`NotResumed::OtherLayout`] when the save says another layout of the zone than `start`'s, or
/// none; [`NotResumed::Unreadable`] when it cannot be read.
pub fn resume(
    text: &str,
    start: (World, Game),
    scenes: &mut SceneLibrary,
) -> Result<(World, Game, Loaded), NotResumed> {
    let about = World::about(text).map_err(NotResumed::Unreadable)?;
    let (mut world, game) = start;
    if about.get(LAYOUT) != Some(&game.zone().layout(&world).fingerprint()) {
        return Err(NotResumed::OtherLayout);
    }
    let loaded = world.load(text, scenes).map_err(NotResumed::Unreadable)?;
    Ok((world, game, loaded))
}

/// A run reloaded after the game's scenes changed on disk ([`reload`]).
pub struct Reloaded {
    /// The world of the run, in place of the one before.
    pub world: World,
    /// The game that plays in it.
    pub game: Game,
    /// Whether the run started again, the zone's layout having changed, rather than went on.
    pub restarted: bool,
    /// What carrying the run set aside, and the scenes that changed since the game was made: what
    /// loading a save reports ([`World::load`]).
    pub loaded: Option<Loaded>,
}

/// The run in `world` after the scenes of `scenes` changed: `before` is the new game by day of the
/// scenes before they changed ([`start`]). The new game of the scenes as they are now decides:
///
/// - when its zone's layout is not `before`'s, by its fingerprint, a new run starts in it, at the
///   time of day that `starts_at_night` gives, as R starts one, since the hero could stand in a new
///   wall, or the key have moved after it was taken;
/// - otherwise the run is carried into it: what the run changed of `before` is saved, and loaded
///   into the new game (`World::carry`), the hero where it stood, the key, the door and the time of
///   day as they were, the scenes as they are now. What the run changed of the layout itself, a
///   collider through the world panel, does not start it again.
///
/// # Errors
///
/// When the new game cannot be played, as [`start`] says, or the run cannot be carried into it.
/// `world` is left as it was then, but for the campfire's light, put back to the intensity it
/// flickers around.
pub fn reload(
    world: &mut World,
    before: &(World, Game),
    scenes: &mut SceneLibrary,
    starts_at_night: bool,
) -> Result<Reloaded, String> {
    let (mut after, mut game) = start(scenes)?;
    game.set_starts_at_night(starts_at_night);
    let layout = |(world, game): (&World, &Game)| game.zone().layout(world).fingerprint();
    if layout((&before.0, &before.1)) != layout((&after, &game)) {
        game.restart(&mut after);
        return Ok(Reloaded {
            world: after,
            game,
            restarted: true,
            loaded: None,
        });
    }
    unflicker(world);
    let (world, loaded) = world
        .carry(&before.0, after, scenes)
        .map_err(|error| slime_app::error_chain(&error))?;
    Ok(Reloaded {
        world,
        game,
        restarted: false,
        loaded: Some(loaded),
    })
}

/// Puts the campfire's lights of `world` back to the intensity that they flicker around, as the
/// zone gives it: the view makes them flicker at each frame, and what is saved would hold each
/// frame's.
fn unflicker(world: &mut World) {
    world.query_mut::<(&Flickers, &mut Light)>(|_, (flickers, light)| {
        if let Light::Point { intensity, .. } = light {
            *intensity = flickers.intensity;
        }
    });
}

/// Writes `text` to `file`, its directory made if it has none, through a file beside it, written
/// to the disk, then renamed over it: `file` holds the save before or the new one, never part of
/// one, a power cut included. The file beside it does not outlive an error.
///
/// # Errors
///
/// As the file system's operations.
pub fn write(file: &Path, text: &str) -> io::Result<()> {
    if let Some(directory) = file.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let written = beside(file, "new");
    let renamed = File::create(&written)
        .and_then(|mut new| {
            new.write_all(text.as_bytes())?;
            // On the disk before the rename, which a file system may otherwise put there first.
            new.sync_all()
        })
        .and_then(|()| std::fs::rename(&written, file));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&written);
    }
    renamed
}

/// Moves `file` aside, to `<file>.<why>`, in place of the one set aside so before, so that the next
/// save does not write over it; returns where.
///
/// # Errors
///
/// As the file system's operations.
pub fn set_aside(file: &Path, why: &str) -> io::Result<PathBuf> {
    let aside = beside(file, why);
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
        let aside = set_aside(&file, "unreadable").expect("set aside");
        assert_eq!(aside, beside(&file, "unreadable"));
        assert!(!file.exists());
        assert_eq!(std::fs::read_to_string(&aside).expect("read"), "second");
        // Set aside again, in place of the one set aside before.
        write(&file, "third").expect("written");
        set_aside(&file, "unreadable").expect("set aside again");
        assert_eq!(std::fs::read_to_string(&aside).expect("read"), "third");
        // A file that cannot be written leaves nothing beside it: here, a directory in its place.
        std::fs::create_dir(&file).expect("a directory");
        assert!(write(&file, "fourth").is_err());
        assert!(!beside(&file, "new").exists());
        std::fs::remove_dir_all(&directory).expect("removed");
    }
}

#[cfg(test)]
mod runs {
    use slime_render::glam::Vec2;

    use super::*;
    use crate::rules::tests::{turned_box, zone};
    use crate::zone::Collider;

    #[test]
    fn a_run_is_resumed_only_from_a_save_of_its_layout_and_never_panics() {
        let plan = zone([0.0, 0.0], vec![turned_box([5.0, 5.0], [1.0, 2.0], 0.3)]);
        let mut scenes = plan.scenes();
        let new_game = |scenes: &mut SceneLibrary| start(scenes).expect("the zone can be played");
        let (mut world, game) = new_game(&mut scenes);
        for _ in 0..30 {
            game.step(&mut world, Vec2::X, 1.0 / 60.0);
        }
        // What the run changed of the layout, as the world panel changes it, does not make it another.
        world.query_mut::<&mut Collider>(|_, collider| collider.height = 3.5);
        let text = save(&mut world, &mut scenes).expect("saved");
        let new = new_game(&mut scenes);
        let (resumed, resumed_game, loaded) = resume(&text, new, &mut scenes).expect("resumed");
        assert!(loaded.set_aside.is_empty() && loaded.scenes_changed.is_empty());
        assert_eq!(world.first_difference(&resumed), None, "{text}");
        assert_eq!(resumed_game.hero(&resumed), game.hero(&world));
        // Another layout: the zone's start moved.
        let mut moved = plan.clone();
        moved.start = Vec2::new(1.0, 0.0);
        let mut other = moved.scenes();
        let elsewhere = start(&mut other).expect("the zone can be played");
        assert!(matches!(
            resume(&text, elsewhere, &mut other),
            Err(NotResumed::OtherLayout)
        ));
        // Not a save, a save of a later format, one that says no layout.
        for refused in ["layout = [", "\"$format\" = 2\n", ""] {
            let new = new_game(&mut scenes);
            let resumed = resume(refused, new, &mut scenes);
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
            let new = new_game(&mut scenes);
            let _ = resume(&text[..end], new, &mut scenes);
        }
    }
}
