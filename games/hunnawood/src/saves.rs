//! The game's saves (`docs/adr/0088-saving-and-resuming-a-run.md`): one file, `hunnawood/save.toml`
//! in the user's data directory unless `--save` names another, which holds what differs between the
//! run and a new game by day (`slime_world::World::save`), the version of the game that wrote it,
//! and the layout of the zone it was made on. A save is checked before it is resumed ([`resume`]):
//! one made on another layout is not resumed, as the run starts again when the layout changes under
//! hot reload, which carries the run through a save otherwise ([`reload`]); nor is one that a later
//! version of the game wrote, one that holds what this version does not know, which writing it
//! back would lose, or one whose run cannot be played. One copy of the game at a time holds a
//! save's file ([`SaveFile`]).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{File, TryLockError};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use slime_serialize::MAX_SIZE;
use slime_world::{DocumentError, Loaded, SceneLibrary, World};

use crate::rules::Game;
use crate::zone::Layout;

/// The game's directory in the user's data directory.
const DIRECTORY: &str = "hunnawood";
/// The save's file in the game's directory.
const FILE: &str = "save.toml";
/// The keys under which a save says, in what it says of itself ([`World::about`]), the version of
/// the game that wrote it, and the layout of the zone it was made on ([`Layout::to_text`]).
const VERSION: &str = "version";
const ZONE: &str = "zone";
/// The key under which a save of 0.4 says the fingerprint of the zone's layout it was made on
/// ([`Layout::fingerprint`]), which it is compared by, as 0.4 compared it.
const LAYOUT_0_4: &str = "layout";
/// The version of the game, which its saves say.
pub const THIS_VERSION: &str = env!("CARGO_PKG_VERSION");

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
/// ([`start`]); the version of the game; and the layout of that new game's zone, as its scenes place
/// it, whatever the run changed of it.
///
/// # Errors
///
/// When the zone cannot be played, or the world cannot be saved ([`World::save`]).
pub fn save(world: &World, scenes: &mut SceneLibrary) -> Result<String, String> {
    let (start, start_game) = start(scenes)?;
    let layout = start_game.zone().layout(&start).to_text();
    let about = BTreeMap::from([
        (VERSION.to_owned(), THIS_VERSION.to_owned()),
        (ZONE.to_owned(), layout),
    ]);
    world
        .save(&start, &about)
        .map_err(|error| slime_app::error_chain(&error))
}

/// Why a save is not resumed.
#[derive(Debug)]
pub enum NotResumed {
    /// It is not a save that this program reads: not TOML, of a later version of its format, not a
    /// save.
    Unreadable(DocumentError),
    /// A later version of the game wrote it, as it says: the version it says.
    Later(String),
    /// It was made on another layout of the zone.
    OtherLayout,
    /// It holds what this version of the game does not know, which writing it back would lose:
    /// what loading it left out, as it warned of it ([`Loaded::warnings`]).
    Unknown(Vec<String>),
    /// The run it holds cannot be played: why ([`Game::find`]).
    Unplayable(String),
}

/// The run that the save `text` holds, resumed in `start`, a new game by day of `scenes`
/// ([`start`]), and what loading it set aside. The save is checked first: what it says of itself,
/// the version of the game that wrote it and the layout it was made on, then what loading it left
/// out, and last the game found again in the world loaded, which loading may have changed whole.
/// A save of 0.4 says no version, and the fingerprint of its layout, which is compared with the
/// fingerprint of `start`'s, as 0.4 compared it.
///
/// # Errors
///
/// In the order checked: [`NotResumed::Unreadable`] when it cannot be read; [`NotResumed::Later`]
/// when it says a later version than this one (`later`); [`NotResumed::OtherLayout`] when it says
/// another layout of the zone than `start`'s ([`Layout::matches`]), or none;
/// [`NotResumed::Unknown`] when loading it warned of what this version does not know;
/// [`NotResumed::Unplayable`] when the run loaded cannot be played.
pub fn resume(
    text: &str,
    start: (World, Game),
    scenes: &mut SceneLibrary,
) -> Result<(World, Game, Loaded), NotResumed> {
    let about = World::about(text).map_err(NotResumed::Unreadable)?;
    if let Some(version) = about.get(VERSION).filter(|version| later(version)) {
        return Err(NotResumed::Later(version.clone()));
    }
    let (mut world, game) = start;
    let layout = game.zone().layout(&world);
    let same = match (about.get(ZONE), about.get(LAYOUT_0_4)) {
        (Some(zone), _) => Layout::from_text(zone).is_some_and(|zone| zone.matches(&layout)),
        (None, Some(fingerprint)) => *fingerprint == layout.fingerprint(),
        (None, None) => false,
    };
    if !same {
        return Err(NotResumed::OtherLayout);
    }
    let loaded = world.load(text, scenes).map_err(NotResumed::Unreadable)?;
    if !loaded.warnings.is_empty() {
        let unknown = loaded.warnings.iter().map(ToString::to_string).collect();
        return Err(NotResumed::Unknown(unknown));
    }
    let game = Game::find(&world, false).map_err(NotResumed::Unplayable)?;
    Ok((world, game, loaded))
}

/// Whether `version`, as a save says it, is a later version of the game than this one: its numbers,
/// separated by dots, compared in turn, as `0.10.0` after `0.4.0`. A version that is not numbers so,
/// which no version of the game writes, is a later one unless it is this one.
fn later(version: &str) -> bool {
    let numbers = |version: &str| -> Option<Vec<u64>> {
        version
            .split('.')
            .map(|number| number.parse().ok())
            .collect()
    };
    match (numbers(version), numbers(THIS_VERSION)) {
        (Some(save), Some(this)) => save > this,
        _ => version != THIS_VERSION,
    }
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
/// - when its zone's layout is not `before`'s ([`Layout::matches`]), a new run starts in it, at the
///   time of day that `starts_at_night` gives, as R starts one, since the hero could stand in a new
///   wall, or the key have moved after it was taken;
/// - otherwise the run is carried into it: what the run changed of `before` is saved, and loaded
///   into the new game (`World::carry`), the hero where it stood, the key, the door and the time of
///   day as they were, the scenes as they are now, and the game found again in the world carried
///   ([`Game::find`]). What the run changed of the layout itself, a collider through the world
///   panel, does not start it again.
///
/// # Errors
///
/// When the new game cannot be played, as [`start`] says, or the run cannot be carried into it, or
/// played once carried. `world` is left as it was then.
pub fn reload(
    world: &World,
    before: &(World, Game),
    scenes: &mut SceneLibrary,
    starts_at_night: bool,
) -> Result<Reloaded, String> {
    let (mut after, mut game) = start(scenes)?;
    game.set_starts_at_night(starts_at_night);
    let layout = |(world, game): (&World, &Game)| game.zone().layout(world);
    if !layout((&before.0, &before.1)).matches(&layout((&after, &game))) {
        game.restart(&mut after);
        return Ok(Reloaded {
            world: after,
            game,
            restarted: true,
            loaded: None,
        });
    }
    let (world, loaded) = world
        .carry(&before.0, after, scenes)
        .map_err(|error| slime_app::error_chain(&error))?;
    let game = Game::find(&world, starts_at_night)
        .map_err(|why| format!("the run carried cannot be played: {why}"))?;
    Ok(Reloaded {
        world,
        game,
        restarted: false,
        loaded: Some(loaded),
    })
}

/// A save's file, which this copy of the game holds, so that one copy at a time resumes and writes
/// the save: no other holds it while this one does. It holds the lock of a file beside it,
/// `<file>.lock`, which the system lets go when the program ends, however it ends, a crash
/// included; the lock file stays, empty.
pub struct SaveFile {
    path: PathBuf,
    /// The lock file, locked while this copy holds the save's file.
    _lock: File,
}

impl SaveFile {
    /// Holds the save's file `path`, its directory made if it has none.
    ///
    /// # Errors
    ///
    /// When another copy of the game holds it, or its lock file cannot be made or locked, as on a
    /// file system without locks: a message that says why.
    pub fn hold(path: PathBuf) -> Result<Self, String> {
        let shown = path.display();
        let cannot = |error: io::Error| format!("the save in {shown} cannot be held: {error}");
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
                "another copy of the game holds the save in {shown}, and writes it"
            )),
            Err(TryLockError::Error(error)) => Err(cannot(error)),
        }
    }

    /// The save's file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The text of the save in `file`, read up to the size of the largest document that a world reads
/// ([`MAX_SIZE`]), not further: a larger file is refused before it is read.
///
/// # Errors
///
/// As the file system's operations; [`io::ErrorKind::InvalidData`] when the file is not text, or is
/// larger than a save.
pub fn read(file: &Path) -> io::Result<String> {
    let too_large = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("it is larger than a save, which holds at most {MAX_SIZE} bytes"),
        )
    };
    let opened = File::open(file)?;
    if opened.metadata()?.len() > MAX_SIZE as u64 {
        return Err(too_large());
    }
    let mut text = String::new();
    // Bounded all the same, should the file grow while it is read.
    opened.take(MAX_SIZE as u64 + 1).read_to_string(&mut text)?;
    if text.len() > MAX_SIZE {
        return Err(too_large());
    }
    Ok(text)
}

/// Writes `text` to `file`, its directory made if it has none, through a file beside it of this
/// process's own, `<file>.<process id>.new`, written to the disk, then renamed over it: `file` holds
/// the save before or the new one, never part of one, a power cut included, and never a mix of two
/// writers'. The file beside it does not outlive an error; a process killed while it writes leaves
/// it.
///
/// # Errors
///
/// As the file system's operations.
pub fn write(file: &Path, text: &str) -> io::Result<()> {
    if let Some(directory) = file.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let written = beside(file, &format!("{}.new", std::process::id()));
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

/// Moves `file` aside, to `<file>.<why>`, or `<file>.<why>.2`, `.3` and on, the first of these
/// names that nothing has, so that neither the next save nor one set aside later writes over it;
/// returns where. One copy of the game at a time holds a save's file ([`SaveFile`]), so that no
/// other takes the name meanwhile.
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

    /// A directory of the test's own, named after it and the process, made empty.
    fn directory(test: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("hunnawood-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a directory");
        directory
    }

    #[test]
    fn a_save_is_written_whole_or_not_at_all_through_a_file_of_its_own() {
        let directory = directory("written");
        let file = directory.join("deep").join(FILE);
        write(&file, "first").expect("written, its directory made");
        // Another copy of the game, or an older version of it, half-way through its write: this
        // one writes through a file of its own, and leaves the other's.
        let others = beside(&file, "new");
        std::fs::write(&others, "another writer's").expect("written");
        write(&file, "second").expect("written over");
        assert_eq!(std::fs::read_to_string(&file).expect("read"), "second");
        assert_eq!(
            std::fs::read_to_string(&others).expect("left"),
            "another writer's"
        );
        let own = beside(&file, &format!("{}.new", std::process::id()));
        assert!(!own.exists(), "the file written beside it is renamed");
        // A file that cannot be written leaves nothing beside it: here, a directory in its place.
        std::fs::remove_file(&file).expect("removed");
        std::fs::create_dir(&file).expect("a directory");
        assert!(write(&file, "third").is_err());
        assert!(!own.exists());
        std::fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn saves_set_aside_keep_names_of_their_own() {
        let directory = directory("aside");
        let file = directory.join(FILE);
        let mut asides = Vec::new();
        for (text, why) in [
            ("first", "unreadable"),
            ("second", "unreadable"),
            ("third", "other-layout"),
            ("fourth", "unreadable"),
        ] {
            write(&file, text).expect("written");
            asides.push(set_aside(&file, why).expect("set aside"));
            assert!(!file.exists());
        }
        let names: Vec<String> = asides
            .iter()
            .map(|aside| {
                aside
                    .file_name()
                    .expect("a name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            names,
            [
                "save.toml.unreadable",
                "save.toml.unreadable.2",
                "save.toml.other-layout",
                "save.toml.unreadable.3"
            ]
        );
        for (aside, text) in asides.iter().zip(["first", "second", "third", "fourth"]) {
            assert_eq!(std::fs::read_to_string(aside).expect("kept"), text);
        }
        std::fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_save_is_read_up_to_the_size_of_the_largest_one() {
        let directory = directory("read");
        let file = directory.join(FILE);
        std::fs::write(&file, "a = 1\n").expect("written");
        assert_eq!(read(&file).expect("read"), "a = 1\n");
        std::fs::write(&file, vec![b' '; MAX_SIZE + 1]).expect("written");
        let error = read(&file).expect_err("larger than a save");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{error}");
        std::fs::write(&file, [0xff, 0xfe]).expect("written");
        assert_eq!(
            read(&file).expect_err("not text").kind(),
            io::ErrorKind::InvalidData
        );
        std::fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn one_copy_of_the_game_at_a_time_holds_a_save() {
        let directory = directory("held");
        let file = directory.join("deep").join(FILE);
        let held = SaveFile::hold(file.clone()).expect("held, its directory made");
        assert_eq!(held.path(), file);
        let Err(why) = SaveFile::hold(file.clone()) else {
            panic!("a save held twice");
        };
        assert!(why.contains("another copy of the game"), "{why}");
        // Another save beside it is another file.
        SaveFile::hold(directory.join("other.toml")).expect("held");
        drop(held);
        SaveFile::hold(file).expect("held once let go");
        std::fs::remove_dir_all(&directory).expect("removed");
    }

    #[test]
    fn a_later_version_is_one_whose_numbers_come_after() {
        assert!(!later(THIS_VERSION));
        for earlier in ["0.0.1", "0.3.0", "0", "0.4"] {
            assert!(!later(earlier), "{earlier}");
        }
        for after in [
            "99.0.0",
            "0.10.0",
            "1",
            "0.4.0.1",
            "0.4.0-dev",
            "banana",
            "",
        ] {
            assert!(later(after), "{after}");
        }
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
        world.query_mut::<&mut Collider>(|_, mut collider| collider.height = 3.5);
        let text = save(&world, &mut scenes).expect("saved");
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

    /// A save of a run in the zone `plan`, its hero moved, after `change` changed the run; the
    /// scenes of the zone.
    fn saved(
        plan: &crate::rules::tests::Plan,
        change: impl FnOnce(&mut World, &Game),
    ) -> (String, SceneLibrary) {
        let mut scenes = plan.scenes();
        let (mut world, game) = start(&mut scenes).expect("the zone can be played");
        game.step(&mut world, Vec2::X, 1.0 / 60.0);
        change(&mut world, &game);
        (save(&world, &mut scenes).expect("saved"), scenes)
    }

    #[test]
    fn a_save_whose_run_cannot_be_played_is_not_resumed() {
        let plan = zone([0.0, 0.0], Vec::new());
        // The key's entity removed, the hero's, the zone's start without its marker: as a save
        // edited by hand, or written by a game with a bug.
        type Change = fn(&mut World, &Game);
        let cases: [(Change, &str); 3] = [
            (
                |world, game| {
                    world.despawn(game.zone().item);
                },
                "0 markers `item`",
            ),
            (
                |world, game| {
                    world.despawn(game.hero_entity());
                },
                "no hero",
            ),
            (
                |world, game| {
                    world.remove::<crate::zone::Marker>(game.zone().start);
                },
                "0 markers `start`",
            ),
        ];
        for (change, why) in cases {
            let (text, mut scenes) = saved(&plan, change);
            let new = start(&mut scenes).expect("the zone can be played");
            match resume(&text, new, &mut scenes) {
                Err(NotResumed::Unplayable(reason)) => assert!(reason.contains(why), "{reason}"),
                other => panic!("{why}: {:?}", other.err()),
            }
        }
        // A save that the game can play is resumed with a game whose entities are the world's: a
        // new run starts in it, and plays.
        let (text, mut scenes) = saved(&plan, |_, _| {});
        let new = start(&mut scenes).expect("the zone can be played");
        let (mut world, game, _) = resume(&text, new, &mut scenes).expect("resumed");
        game.restart(&mut world);
        game.step(&mut world, Vec2::X, 1.0 / 60.0);
    }

    #[test]
    fn a_save_of_a_later_version_or_holding_what_this_one_does_not_know_is_not_resumed() {
        let plan = zone([0.0, 0.0], Vec::new());
        let (text, mut scenes) = saved(&plan, |_, _| {});
        let version = format!("version = \"{THIS_VERSION}\"");
        assert!(text.contains(&version), "{text}");
        let resumed = |text: &str, scenes: &mut SceneLibrary| {
            let new = start(scenes).expect("the zone can be played");
            resume(text, new, scenes)
        };
        // An earlier version's is resumed.
        let earlier = text.replacen(&version, "version = \"0.4.0\"", 1);
        resumed(&earlier, &mut scenes).expect("an earlier version's");
        let later = text.replacen(&version, "version = \"99.0.0\"", 1);
        assert!(matches!(
            resumed(&later, &mut scenes),
            Err(NotResumed::Later(version)) if version == "99.0.0"
        ));
        // A component of a type that this version does not know, a variant, a field.
        for (unknown, name) in [
            (
                "[added.hero]\n\"hunnawood::Chest\" = { gold = 3 }\n",
                "hunnawood::Chest",
            ),
            (
                "[changed.zone]\n\"hunnawood::Clock\" = { time = \"Dusk\" }\n",
                "Dusk",
            ),
            (
                "[changed.zone]\n\"hunnawood::Clock\" = { weather = \"rain\" }\n",
                "weather",
            ),
        ] {
            let text = format!("{text}\n{unknown}");
            match resumed(&text, &mut scenes) {
                Err(NotResumed::Unknown(warnings)) => {
                    assert!(warnings.iter().any(|w| w.contains(name)), "{warnings:?}");
                }
                other => panic!("{unknown}: {:?}", other.err()),
            }
        }
    }

    #[test]
    fn a_save_is_resumed_on_its_layout_given_a_few_units_in_the_last_place() {
        // The start on a boundary of the rounding of 0.4's fingerprint, and the save made on the
        // next number up: as the platforms' maths, or an update that does not change the zone,
        // may give the same layout.
        let word = |number: f32| (number * 10_000.0).round();
        let at = (1..1000)
            .map(|n| (n as f32 + 0.5) / 10_000.0)
            .find(|&at| word(at) != word(at.next_up()))
            .expect("a number on a boundary");
        let (text, mut made_on) = saved(&zone([at, 0.0], Vec::new()), |_, _| {});
        let mut scenes = zone([at.next_up(), 0.0], Vec::new()).scenes();
        let new = start(&mut scenes).expect("the zone can be played");
        let fingerprint = |(world, game): &(World, Game)| game.zone().layout(world).fingerprint();
        assert_ne!(
            fingerprint(&start(&mut made_on).expect("playable")),
            fingerprint(&new),
            "0.4 would have set the save aside"
        );
        resume(&text, new, &mut scenes).expect("the same layout");
    }
}
