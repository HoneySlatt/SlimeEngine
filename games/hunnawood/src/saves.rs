//! The game's saves (`docs/adr/0088-saving-and-resuming-a-run.md`): one file, `hunnawood/save.toml`
//! in the user's data directory unless `--save` names another, which holds what the run changed of
//! a new game by day, the world as the game made it and settled it (`slime_world::World::save`),
//! the version of the game that wrote it, and the layout of the zone it was made on. A save is
//! checked before it is resumed ([`resume`]): one made on another layout is not resumed, as the run
//! starts again when the layout changes under hot reload, which carries the run through a save
//! otherwise ([`reload`]); nor is one that an earlier version of the engine wrote, whose saves this
//! one no longer reads, one that a later version of the game wrote, one that holds what this
//! version does not know, which writing it back would lose, or one whose run cannot be played. One
//! copy of the game at a time holds a save's file (`slime_app::HeldFile`).

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
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
/// The version of the game, which its saves say.
pub const THIS_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The save's file when `--save` names none: `hunnawood/save.toml` in the user's data directory, if
/// the environment gives one.
pub fn default_file() -> Option<PathBuf> {
    slime_app::data_directory().map(|directory| directory.join(DIRECTORY).join(FILE))
}

/// A new game of the zone and the hero as `scenes` resolves them, by day, in a world of its own,
/// settled (`World::settle`): the world that saves differ from, whatever `--night` says, so that a
/// save made at night is resumed at night.
///
/// # Errors
///
/// When the zone cannot be played, as [`Game::spawn`] says.
pub fn start(scenes: &mut SceneLibrary) -> Result<(World, Game), String> {
    let mut world = World::new();
    let game = Game::spawn(&mut world, scenes, false)?;
    world.settle();
    Ok((world, game))
}

/// The save of the run in `world`: what it changed of the new game by day that it started from
/// ([`start`]); the version of the game; and `layout`, the layout of that new game's zone, as its
/// scenes placed it, whatever the run changed of it.
///
/// # Errors
///
/// When the world cannot be saved ([`World::save`]).
pub fn save(world: &World, layout: &Layout) -> Result<String, String> {
    let about = BTreeMap::from([
        (VERSION.to_owned(), THIS_VERSION.to_owned()),
        (ZONE.to_owned(), layout.to_text()),
    ]);
    world
        .save(&about)
        .map_err(|error| slime_app::error_chain(&error))
}

/// Why a save is not resumed.
#[derive(Debug)]
pub enum NotResumed {
    /// It is not a save that this program reads: not TOML, of a later version of its format, not a
    /// save.
    Unreadable(DocumentError),
    /// An earlier version of the engine wrote it, whose saves this one no longer reads: why
    /// ([`DocumentError::Layout`]).
    Earlier(DocumentError),
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

impl NotResumed {
    /// The end of the name that a save not resumed for this reason is set aside under
    /// (`slime_app::set_aside`).
    pub fn aside(&self) -> &'static str {
        match self {
            Self::Unreadable(_) => "unreadable",
            Self::Earlier(_) => "earlier-version",
            Self::Later(_) => "later-version",
            Self::OtherLayout => "other-layout",
            Self::Unknown(_) => "unknown-content",
            Self::Unplayable(_) => "unplayable",
        }
    }
}

impl fmt::Display for NotResumed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(error) | Self::Earlier(error) => {
                f.write_str(&slime_app::error_chain(error))
            }
            Self::Later(version) => write!(
                f,
                "version {version} of the game wrote it, a later one than this one, {THIS_VERSION}"
            ),
            Self::OtherLayout => f.write_str("it was made on another layout of the zone"),
            Self::Unknown(unknown) => write!(
                f,
                "it holds what this version of the game does not know, which writing it back \
                 would lose: {}",
                unknown.join("; ")
            ),
            Self::Unplayable(why) => write!(f, "its run cannot be played: {why}"),
        }
    }
}

impl std::error::Error for NotResumed {}

/// The run that the save `text` holds, resumed in `start`, a new game by day of `scenes`
/// ([`start`]), and what loading it set aside. The save is checked first: what it says of itself,
/// the layout of saves that it was written in, the version of the game that wrote it and the layout
/// of the zone it was made on, then what loading it left out, and last the game found again in the
/// world loaded, which loading may have changed whole.
///
/// # Errors
///
/// In the order checked: [`NotResumed::Unreadable`] when it cannot be read, and
/// [`NotResumed::Earlier`] when an earlier version of the engine wrote it; [`NotResumed::Later`]
/// when it says a later version than this one (`later`); [`NotResumed::OtherLayout`] when it says
/// another layout of the zone than `start`'s ([`Layout::matches`]), or none;
/// [`NotResumed::Unknown`] when loading it warned of what this version does not know;
/// [`NotResumed::Unplayable`] when the run loaded cannot be played.
pub fn resume(
    text: &str,
    start: (World, Game),
    scenes: &mut SceneLibrary,
) -> Result<(World, Game, Loaded), NotResumed> {
    let about = World::about(text).map_err(|error| match error {
        DocumentError::Layout { found, read } if found < read => NotResumed::Earlier(error),
        error => NotResumed::Unreadable(error),
    })?;
    if let Some(version) = about.get(VERSION).filter(|version| later(version)) {
        return Err(NotResumed::Later(version.clone()));
    }
    let (mut world, game) = start;
    let layout = game.zone().layout(&world);
    let same = about
        .get(ZONE)
        .and_then(|zone| Layout::from_text(zone))
        .is_some_and(|zone| zone.matches(&layout));
    if !same {
        return Err(NotResumed::OtherLayout);
    }
    // The save names the scenes that a new game spawns, the zone and the hero: it spawns none that
    // the game would prepare.
    let loaded = world
        .load(text, scenes, &mut |_, _| {})
        .map_err(NotResumed::Unreadable)?;
    if !loaded.warnings.is_empty() {
        let unknown = loaded.warnings.iter().map(ToString::to_string).collect();
        return Err(NotResumed::Unknown(unknown));
    }
    let game = Game::find(&world, false).map_err(NotResumed::Unplayable)?;
    Ok((world, game, loaded))
}

/// Whether `version`, as a save says it, is a later version of the game than this one: its numbers,
/// separated by dots, compared in turn, as `0.10.0` after `0.4.0`. A version that is not numbers
/// so, which no version of the game writes, is a later one unless it is this one.
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
    /// The layout of the new game's zone, which the run's saves say from now on.
    pub layout: Layout,
    /// Whether the run started again, the zone's layout having changed, rather than went on.
    pub restarted: bool,
    /// What carrying the run set aside, and the scenes that changed since the game was made: what
    /// loading a save reports ([`World::load`]).
    pub loaded: Option<Loaded>,
}

/// The run in `world` after the scenes of `scenes` changed: `layout` is the layout of the zone of
/// the new game by day that the run started from ([`start`]), of the scenes before they changed.
/// The new game of the scenes as they are now decides:
///
/// - when its zone's layout is not `layout` ([`Layout::matches`]), a new run starts in it, at the
///   time of day that `starts_at_night` gives, as R starts one, since the hero could stand in a new
///   wall, or the key have moved after it was taken;
/// - otherwise the run is carried into it: what the run changed is saved, and loaded into the new
///   game (`World::carry`), the hero where it stood, the key, the door and the time of day as they
///   were, the scenes as they are now, and the game found again in the world carried
///   ([`Game::find`]). What the run changed of the layout itself, a collider through the world
///   panel, does not start it again.
///
/// # Errors
///
/// When the new game cannot be played, as [`start`] says, or the run cannot be carried into it, or
/// played once carried. `world` is left as it was then.
pub fn reload(
    world: &World,
    layout: &Layout,
    scenes: &mut SceneLibrary,
    starts_at_night: bool,
) -> Result<Reloaded, String> {
    let (mut after, mut game) = start(scenes)?;
    game.set_starts_at_night(starts_at_night);
    let new = game.zone().layout(&after);
    if !layout.matches(&new) {
        game.restart(&mut after);
        return Ok(Reloaded {
            world: after,
            game,
            layout: new,
            restarted: true,
            loaded: None,
        });
    }
    // The new game names the scenes that the run spawned, the zone and the hero: carrying spawns
    // none that the game would prepare.
    let (world, loaded) = world
        .carry(after, scenes, &mut |_, _| {})
        .map_err(|error| slime_app::error_chain(&error))?;
    let game = Game::find(&world, starts_at_night)
        .map_err(|why| format!("the run carried cannot be played: {why}"))?;
    Ok(Reloaded {
        world,
        game,
        layout: new,
        restarted: false,
        loaded: Some(loaded),
    })
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
/// process's own, `<file>.<process id>.new`, written to the disk, then renamed over it: `file`
/// holds the save before or the new one, never part of one, a power cut included, and never a mix
/// of two writers'. The file beside it does not outlive an error; a process killed while it writes
/// leaves it.
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
    fn a_later_version_is_one_whose_numbers_come_after() {
        assert!(!later(THIS_VERSION));
        for earlier in ["0.0.1", "0.3.0", "0", "0.4"] {
            assert!(!later(earlier), "{earlier}");
        }
        // Relative to this version, so that the test holds in every version.
        let [major, minor]: [u64; 2] = [0, 1].map(|at| {
            THIS_VERSION
                .split('.')
                .nth(at)
                .and_then(|number| number.parse().ok())
                .expect("numbers")
        });
        for after in [
            "99.0.0".to_owned(),
            // A minor version of one more digit: after as a number, not as text.
            format!("{major}.{}.0", minor + 10),
            format!("{}", major + 1),
            format!("{THIS_VERSION}.1"),
            format!("{THIS_VERSION}-dev"),
            "banana".to_owned(),
            String::new(),
        ] {
            assert!(later(&after), "{after}");
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
        let layout = game.zone().layout(&world);
        for _ in 0..30 {
            game.step(&mut world, Vec2::X, 1.0 / 60.0);
        }
        // What the run changed of the layout, as the world panel changes it, does not make it
        // another.
        world.query_mut::<&mut Collider>(|_, mut collider| collider.height = 3.5);
        let text = save(&world, &layout).expect("saved");
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
        // Not a save, a save of a later format, an empty file; a save that says no layout of saves,
        // as those of 0.3, is one of an earlier version of the engine.
        for refused in ["layout = [", "\"$format\" = 2\n", "", "next_created = 1\n"] {
            let new = new_game(&mut scenes);
            let resumed = resume(refused, new, &mut scenes);
            assert!(
                matches!(
                    (refused, &resumed),
                    ("next_created = 1\n", Err(NotResumed::Earlier(_)))
                        | (_, Err(NotResumed::Unreadable(_)))
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
        let layout = game.zone().layout(&world);
        game.step(&mut world, Vec2::X, 1.0 / 60.0);
        change(&mut world, &game);
        (save(&world, &layout).expect("saved"), scenes)
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
        // The start on a boundary of a rounding to ten-thousandths, and the save made on the next
        // number up: as the platforms' maths, or an update that does not change the zone, may give
        // the same layout.
        let word = |number: f32| (number * 10_000.0).round();
        let at = (1..1000)
            .map(|n| (n as f32 + 0.5) / 10_000.0)
            .find(|&at| word(at) != word(at.next_up()))
            .expect("a number on a boundary");
        let (text, _) = saved(&zone([at, 0.0], Vec::new()), |_, _| {});
        let mut scenes = zone([at.next_up(), 0.0], Vec::new()).scenes();
        let new = start(&mut scenes).expect("the zone can be played");
        resume(&text, new, &mut scenes).expect("the same layout");
    }
}
