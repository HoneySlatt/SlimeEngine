//! The editor's program: an application of the engine whose simulation is the editor's core and
//! whose presentation is its window, which a person and the tools that drive it through the
//! development channel use alike (`docs/adr/0134-the-editors-core.md`,
//! `docs/adr/0135-the-editors-program.md`, `docs/adr/0156-the-editors-window-and-its-operations.md`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use slime_app::AppConfig;
use slime_platform::WindowConfig;
use slime_world::World;

use crate::session::{DIRECTORY, Session};
use crate::shared::Shared;
use crate::window::Window;

/// What a program of the editor gives [`run`]: a game's editor package, which knows the game's
/// types (`docs/adr/0110-how-the-editor-runs-a-games-code.md`), or the engine's example.
#[derive(Clone, Debug)]
pub struct Program {
    /// The program's name, which its usage and its window's title say.
    pub name: &'static str,
    /// The level that it opens, unless `--level` names another: a scene file of the game's
    /// sources, beside its metadata file.
    pub level: PathBuf,
    /// The key that the game spawns the level under, the first part of the identities of its
    /// entities: `zone`.
    pub key: &'static str,
    /// The cooked data whose scenes the level places, unless `--assets` names another directory.
    pub assets: PathBuf,
    /// The source assets that the cooked data is cooked from, which the program cooks into it
    /// first when it is missing, as the `editor` example does with the examples'; none for a
    /// program whose data takes long to cook, which the user cooks.
    pub sources: Option<PathBuf>,
    /// Registers the game's component types in a world, before the level is opened; nothing for a
    /// program of the engine's types only.
    pub register: fn(&mut World),
    /// How the viewport lights the level, as the game lights it, which the editor cannot know.
    pub lighting: Lighting,
    /// The command that starts the game's program for Play (`docs/adr/0164-play.md`): the editor
    /// gives it a token, the private copies of the scenes that hold steps not written, and a pipe
    /// as its standard input; none for a program that plays no game, whose Play is refused.
    pub play: Option<fn(&Played<'_>) -> std::process::Command>,
    /// How cargo builds the game's program, which Play builds as it starts it and again as its
    /// code changes (`docs/adr/0166-building-the-game-again.md`); none for a program whose game is
    /// built already, as one shipped, which Play starts as it finds it.
    pub build: Option<Build>,
    /// The project's default layout of the window's panels, which the window shows at its first
    /// start and as *Reset layout* asks, in place of the editor's own
    /// (`docs/adr/0174-layouts-kept-floating-panels-and-closed-panels.md`): the text of a layout
    /// file, as the one that keeps the user's layout in their state directory ([`run`]), its
    /// `default` key naming the project's default and its version, `hunnawood 1`, in place of the
    /// editor's, `editor 1`. A project's author arranges the window, copies that file into the
    /// package, gives it as `include_str!("../editor.layout")`, and names its version anew each
    /// time the arrangement should replace the layouts that users keep. A text that cannot be read
    /// gives the editor's own default, the user told, and the layout that the user keeps against
    /// the project's is neither read nor written until it can be. None for the editor's own
    /// default.
    pub layout: Option<&'static str>,
}

/// How cargo builds a game's program, which [`Program::build`] gives
/// (`docs/adr/0166-building-the-game-again.md`).
#[derive(Clone, Debug)]
pub struct Build {
    /// A directory of the game's workspace, where cargo runs: the editor package's own,
    /// `env!("CARGO_MANIFEST_DIR")`.
    pub directory: PathBuf,
    /// The arguments that name the game's program to cargo:
    /// `["--package", "hunnawood", "--bin", "hunnawood"]`.
    pub arguments: &'static [&'static str],
}

/// What Play hands [`Program::play`], which starts the game on it (`docs/adr/0164-play.md`).
#[derive(Clone, Copy, Debug)]
pub struct Played<'a> {
    /// The cooked data that the editor reads, which the game is to read too.
    pub assets: &'a Path,
    /// A save of the editor's own, in a directory of Play's, which the game is to save its run to,
    /// rather than to the user's save.
    pub save: &'a Path,
    /// Whether the game is to resume the run of `save`, which the game that played before saved
    /// as its code changed, rather than start a new run
    /// (`docs/adr/0166-building-the-game-again.md`).
    pub resume: bool,
    /// Whether the editor runs without a window, as agents run it: the game then runs without one
    /// too, where it can.
    pub headless: bool,
    /// The game's program that cargo built for Play, if [`Program::build`] gives how, or on
    /// Windows its copy, which Play starts.
    pub(crate) built: Option<&'a Path>,
}

impl Played<'_> {
    /// The game's program: the one that cargo built for Play, when [`Program::build`] gives how,
    /// or on Windows its copy, of the same name, which leaves the next build free to replace it
    /// (`docs/adr/0166-building-the-game-again.md`); otherwise the program named `name`, `hunnawood`, beside the editor's own, where cargo builds
    /// a workspace's programs, its extension on Windows given, or `name` alone, which the system's
    /// path finds, when the editor cannot tell where its own program is.
    pub fn program(&self, name: &str) -> PathBuf {
        if let Some(built) = self.built {
            return built.to_path_buf();
        }
        let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        std::env::current_exe()
            .ok()
            .and_then(|own| Some(own.parent()?.join(&file)))
            .unwrap_or_else(|| PathBuf::from(file))
    }
}

/// How the viewport lights the level: the game's sky, exposure and bloom, which its code chooses,
/// and which a game's editor package gives the editor (`docs/adr/0158-the-viewport.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    /// The sky around the level, as `slime_world_render::FrameBuilder::new` takes it; none for a
    /// black sky.
    pub sky: Option<slime_render::Sky>,
    /// The scene's exposure, as `slime_render::Scene::exposure` says.
    pub exposure: f32,
    /// The bloom's strength, as `slime_render::Scene::bloom` says.
    pub bloom: f32,
}

impl Default for Lighting {
    /// No sky, an exposure of 1 and no bloom: the level's own lights alone, their light shown as it
    /// is, which tests start from.
    fn default() -> Self {
        Self {
            sky: None,
            exposure: 1.0,
            bloom: 0.0,
        }
    }
}

/// The editor's directory of journals in the user's data directory, if the environment gives one.
pub(crate) fn journals() -> Option<PathBuf> {
    slime_app::data_directory().map(|directory| directory.join(DIRECTORY))
}

/// The options of a program of the editor.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Options {
    pub(crate) level: PathBuf,
    pub(crate) assets: PathBuf,
    pub(crate) journal: Option<PathBuf>,
    pub(crate) discard: bool,
    pub(crate) headless: bool,
    /// How many frames it runs before it quits, on its schedule rather than on demand: what
    /// `hyperfine` times a start with (`docs/adr/0189-first-measures-of-time.md`).
    pub(crate) frames: Option<u64>,
}

/// Runs `program`, the editor's program, until the user, a tool or a signal stops it, and returns
/// how it ended. It opens the level of `--level`, or the program's, into a world, in a window of its
/// own, with its menus, its regions and its status bar
/// (`docs/adr/0156-the-editors-window-and-its-operations.md`), holds its journal and listens on the
/// development channel when it is started with a token, as a game does
/// (`docs/adr/0122-the-development-channel.md`), so that a tool or an agent drives it through
/// [`Edit`](slime_channel::Edit) requests, its operations run by name among them; started so, it
/// opens no dialog of the system, which the agent could not see: *Open a level…* waits for the
/// level's path typed (`docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md`). With
/// `--headless`, it runs without a window, for tools and agents alone. Either way, it runs a frame
/// only when something asks for one ([`AppConfig::on_demand`]), and runs no game's rules; with
/// `--frames <count>`, it runs that many frames on its schedule, then quits, which is how
/// `hyperfine` times its start (`docs/adr/0189-first-measures-of-time.md`). Its log says when its
/// window presents its first image and when it first draws its level whole, every mesh loaded,
/// each time since the start ([`slime_app::since_start`]). Closing
/// the window, or the operation `file.quit`, quits at once: the journal keeps what the level's file
/// does not hold.
///
/// Its journal is `slime-editor/<the level's asset ID>.journal` in the user's data directory
/// ([`slime_app::data_directory`]), or `<asset ID>.2.journal` and on for a copy of the level, which
/// keeps its asset ID, unless `--journal` names another file, for the level that it opens first;
/// one program at a time holds it ([`slime_app::HeldFile`]), and a journal of another level's file
/// is refused. When the journal holds steps that the level's file does not, which a crash or a quit
/// without writing leaves, the editor recovers them as it opens the level
/// ([`Editor::recover`](crate::Editor::recover)), with its history, and goes on with the same
/// journal; `--discard` starts from the file instead, the journal set aside beside it, as
/// `<journal>.discarded`, or `.discarded.2` and on ([`slime_app::set_aside`]). A journal that
/// cannot be read or played again stops the start, kept as it was, with a message that names
/// `--discard`, and one that cannot be written stops the program.
///
/// The levels that it opened last, which its File menu lists, are kept in `slime-editor/<program's
/// name>.recent` in the user's state directory ([`slime_app::state_directory`]); the first run,
/// which finds none, tells the user one hint. The keys that the user chose, Blender's way or not,
/// are kept in `slime-editor/keys` there, for every program of the editor; the window's layout in
/// `slime-editor/<program's name>.layout`, written once it stands still and as the window closes, a
/// layout made from another default than the program's ([`Program::layout`]) set aside as
/// `<file>.replaced` and one that cannot be read as `<file>.unreadable`, the default shown then and
/// the user told. When the program's cooked data is missing and it names its sources
/// ([`Program::sources`]), it cooks them first.
///
/// Options: `--level <file>`, `--assets <directory>`, `--journal <file>`, `--discard`,
/// `--headless`, `--frames <count>`.
///
/// # Panics
///
/// As [`Editor::apply`](crate::Editor::apply), which only a world changed apart from the editor
/// brings: the program changes its world through its editor alone.
pub fn run(program: &Program) -> ExitCode {
    let options = match parse(program, std::env::args_os().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}\n{}", usage(program));
            return ExitCode::FAILURE;
        }
    };
    let shared = Shared::default();
    let recent = slime_app::state_directory().map(|directory| {
        directory
            .join(DIRECTORY)
            .join(format!("{}.recent", program.name))
    });
    // One choice of keys for every program of the editor: a person's hands are the same.
    shared.borrow_mut().keys =
        slime_app::state_directory().map(|directory| directory.join(DIRECTORY).join("keys"));
    // One file of speeds for every program, by level.
    shared.borrow_mut().speeds =
        slime_app::state_directory().map(|directory| directory.join(DIRECTORY).join("speeds"));
    // A layout for each program, the project's.
    shared.borrow_mut().layout = slime_app::state_directory().map(|directory| {
        directory
            .join(DIRECTORY)
            .join(format!("{}.layout", program.name))
    });
    let headless = options.headless;
    let frames = options.frames;
    let mut session = Session::new(program, options, shared.clone(), recent, journals());
    let config = AppConfig {
        // A run that counts its frames runs them all.
        on_demand: frames.is_none(),
        frame_limit: frames,
        window: (!headless).then(|| WindowConfig {
            title: program.name.to_owned(),
            width: 1280,
            height: 800,
        }),
        restore_lost_device: true,
        ..AppConfig::default()
    };
    let ran = if headless {
        slime_app::run(&mut session, &mut (), &config)
    } else {
        let mut state = shared.borrow_mut();
        state.window = true;
        state.dialogs = std::env::var_os(slime_channel::TOKEN_ENV_VAR).is_none();
        drop(state);
        slime_app::run(&mut session, &mut Window::new(shared), &config)
    };
    match ran {
        Ok(()) => ExitCode::SUCCESS,
        // The run logged why it failed.
        Err(_) => ExitCode::FAILURE,
    }
}

fn usage(program: &Program) -> String {
    format!(
        "usage: {} [--level <scene file>] [--assets <cooked directory>] [--journal <file>] \
         [--discard] [--headless] [--frames <count>]",
        program.name
    )
}

/// The options that `arguments` give, `None` for `--help`, which prints the usage.
pub(crate) fn parse(
    program: &Program,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Option<Options>, String> {
    let mut options = Options {
        level: program.level.clone(),
        assets: program.assets.clone(),
        journal: None,
        discard: false,
        headless: false,
        frames: None,
    };
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} needs a path"))
        };
        match argument.to_str() {
            Some("--level") => options.level = value("--level")?,
            Some("--assets") => options.assets = value("--assets")?,
            Some("--journal") => options.journal = Some(value("--journal")?),
            Some("--discard") => options.discard = true,
            Some("--headless") => options.headless = true,
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a frame count")?;
                let count = value
                    .to_str()
                    .and_then(|value| value.parse().ok())
                    .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?;
                options.frames = Some(count);
            }
            Some("--help") => {
                println!("{}", usage(program));
                return Ok(None);
            }
            _ => return Err(format!("unknown option {}", argument.to_string_lossy())),
        }
    }
    Ok(Some(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program() -> Program {
        Program {
            name: "editor",
            level: PathBuf::from("yard.scene"),
            key: "yard",
            assets: PathBuf::from("cooked"),
            sources: None,
            register: |_| {},
            lighting: Lighting::default(),
            play: None,
            build: None,
            layout: None,
        }
    }

    #[test]
    fn options_name_another_level_assets_and_journal_or_discard_the_journal() {
        let parsed = |arguments: &[&str]| parse(&program(), arguments.iter().map(OsString::from));
        assert_eq!(
            parsed(&[]),
            Ok(Some(Options {
                level: PathBuf::from("yard.scene"),
                assets: PathBuf::from("cooked"),
                journal: None,
                discard: false,
                headless: false,
                frames: None,
            }))
        );
        assert_eq!(
            parsed(&[
                "--level",
                "a.scene",
                "--assets",
                "b",
                "--journal",
                "c",
                "--discard",
                "--headless"
            ]),
            Ok(Some(Options {
                level: PathBuf::from("a.scene"),
                assets: PathBuf::from("b"),
                journal: Some(PathBuf::from("c")),
                discard: true,
                headless: true,
                frames: None,
            }))
        );
        assert_eq!(parsed(&["--help"]), Ok(None));
        assert_eq!(parsed(&["--level"]), Err("--level needs a path".to_owned()));
        assert_eq!(
            parsed(&["--frames", "1"]).map(|options| options.and_then(|options| options.frames)),
            Ok(Some(1))
        );
        assert_eq!(
            parsed(&["--frames"]),
            Err("--frames needs a frame count".to_owned())
        );
        assert_eq!(
            parsed(&["--frames", "one"]),
            Err("invalid frame count `one`".to_owned())
        );
        assert_eq!(
            parsed(&["--watch"]),
            Err("unknown option --watch".to_owned())
        );
    }
}
