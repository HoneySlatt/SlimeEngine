//! The editor's program: an application of the engine whose simulation is the editor's core and
//! whose presentation is its window, which a person and the tools that drive it through the
//! development channel use alike (`docs/adr/0134-the-editors-core.md`,
//! `docs/adr/0135-the-editors-program.md`, `docs/adr/0156-the-editors-window-and-its-operations.md`).

use std::ffi::OsString;
use std::path::PathBuf;
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
}

/// Runs `program`, the editor's program, until the user, a tool or a signal stops it, and returns
/// how it ended. It opens the level of `--level`, or the program's, into a world, in a window of its
/// own, with its menus, its regions and its status bar
/// (`docs/adr/0156-the-editors-window-and-its-operations.md`), holds its journal and listens on the
/// development channel when it is started with a token, as a game does
/// (`docs/adr/0122-the-development-channel.md`), so that a tool or an agent drives it through
/// [`Edit`](slime_channel::Edit) requests, its operations run by name among them. With
/// `--headless`, it runs without a window, for tools and agents alone. Either way, it runs a frame
/// only when something asks for one ([`AppConfig::on_demand`]), and runs no game's rules. Closing
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
/// The levels that it opened last, which its File menu lists, are kept in
/// `slime-editor/<program's name>.recent` in the user's state directory
/// ([`slime_app::state_directory`]); the first run, which finds none, tells the user one hint. When
/// the program's cooked data is missing and it names its sources ([`Program::sources`]), it cooks
/// them first.
///
/// Options: `--level <file>`, `--assets <directory>`, `--journal <file>`, `--discard`,
/// `--headless`.
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
    let headless = options.headless;
    let mut session = Session::new(program, options, shared.clone(), recent, journals());
    let config = AppConfig {
        on_demand: true,
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
        shared.borrow_mut().window = true;
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
         [--discard] [--headless]",
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
            }))
        );
        assert_eq!(parsed(&["--help"]), Ok(None));
        assert_eq!(parsed(&["--level"]), Err("--level needs a path".to_owned()));
        assert_eq!(
            parsed(&["--frames", "2"]),
            Err("unknown option --frames".to_owned())
        );
    }
}
