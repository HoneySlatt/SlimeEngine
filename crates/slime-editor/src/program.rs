//! The editor's program: an application of the engine, without a window until Stage 6c, whose
//! simulation is the editor's core, which tools drive through the development channel
//! (`docs/adr/0134-the-editors-core.md`, `docs/adr/0135-the-editors-program.md`).

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use slime_app::{AppConfig, AppError, HeldFile, Simulation, SimulationContext};
use slime_assets::CookedAssets;
use slime_channel::{Edit, EditorHistory, Reply};
use slime_world::{Command, PersistentId, SceneLibrary, World};

use crate::editor::{EditError, Editor};
use crate::journal;

/// What a program of the editor gives [`run`]: a game's editor package, which knows the game's
/// types (`docs/adr/0110-how-the-editor-runs-a-games-code.md`), or the engine's example.
#[derive(Clone, Debug)]
pub struct Program {
    /// The program's name, which its usage says.
    pub name: &'static str,
    /// The level that it opens, unless `--level` names another: a scene file of the game's
    /// sources, beside its metadata file.
    pub level: PathBuf,
    /// The key that the game spawns the level under, the first part of the identities of its
    /// entities: `zone`.
    pub key: &'static str,
    /// The cooked data whose scenes the level places, unless `--assets` names another directory.
    pub assets: PathBuf,
    /// Registers the game's component types in a world, before the level is opened; nothing for a
    /// program of the engine's types only.
    pub register: fn(&mut World),
}

/// The options of a program of the editor.
#[derive(Debug, PartialEq)]
struct Options {
    level: PathBuf,
    assets: PathBuf,
    journal: Option<PathBuf>,
    discard: bool,
}

/// Why the program has no journal when the environment gives no data directory.
const NO_DATA_DIRECTORY: &str =
    "the environment gives no data directory: give the journal's file with --journal";

/// The directory of the user's data directory where the editor keeps its journals.
const DIRECTORY: &str = "slime-editor";

/// Runs `program`, the editor's program, until a tool or a signal stops it, and returns how it
/// ended: it opens the level of `--level`, or the program's, into a world, holds its journal and
/// listens on the development channel when it is started with a token, as a game does
/// (`docs/adr/0122-the-development-channel.md`), so that a tool or an agent drives it through
/// [`Edit`] requests. It has no window: its frames are paced at the engine's default rate
/// ([`AppConfig::pace_headless`]), and it runs no game's rules.
///
/// Its journal is `slime-editor/<the level's asset ID>.journal` in the user's data directory
/// ([`slime_app::data_directory`]), or `<asset ID>.2.journal` and on for a copy of the level, which
/// keeps its asset ID, unless `--journal` names another file; one program at a time holds it
/// ([`slime_app::HeldFile`]), and a journal of another level's file is refused. When the journal
/// holds steps that the level's file does not, which a crash or a stop without writing leaves, the
/// editor recovers them as it starts ([`Editor::recover`]), with its history, and goes on with the
/// same journal; `--discard` starts from the file instead, the journal set aside beside it, as
/// `<journal>.discarded`, or `.discarded.2` and on ([`slime_app::set_aside`]). A journal that cannot
/// be read or played again stops the start, kept as it was, with a message that names `--discard`,
/// and one that cannot be written stops the program. Stopping does not write the level: its
/// journal keeps what it changed, which the next start recovers.
///
/// Options: `--level <file>`, `--assets <directory>`, `--journal <file>`, `--discard`.
///
/// # Panics
///
/// As [`Editor::apply`], which only a world changed apart from the editor brings: the program
/// changes its world through its editor alone.
pub fn run(program: &Program) -> ExitCode {
    let options = match parse(program, std::env::args_os().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}\n{}", usage(program));
            return ExitCode::FAILURE;
        }
    };
    let mut session = Session::new(program, options);
    let config = AppConfig {
        pace_headless: true,
        ..AppConfig::default()
    };
    match slime_app::run(&mut session, &mut (), &config) {
        Ok(()) => ExitCode::SUCCESS,
        // The run logged why it failed.
        Err(_) => ExitCode::FAILURE,
    }
}

fn usage(program: &Program) -> String {
    format!(
        "usage: {} [--level <scene file>] [--assets <cooked directory>] [--journal <file>] \
         [--discard]",
        program.name
    )
}

/// The options that `arguments` give, `None` for `--help`, which prints the usage.
fn parse(
    program: &Program,
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<Option<Options>, String> {
    let mut options = Options {
        level: program.level.clone(),
        assets: program.assets.clone(),
        journal: None,
        discard: false,
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
            Some("--help") => {
                println!("{}", usage(program));
                return Ok(None);
            }
            _ => return Err(format!("unknown option {}", argument.to_string_lossy())),
        }
    }
    Ok(Some(options))
}

/// The editor's program as a simulation: the editor's core, once the level is opened.
struct Session {
    program: Program,
    options: Options,
    editor: Option<Editor>,
    journal: PathBuf,
    /// Why the journal cannot be written, once it could not: the program stops at the next frame,
    /// so as not to go on with what a crash would lose.
    broken: Option<String>,
}

impl Session {
    fn new(program: &Program, options: Options) -> Self {
        Self {
            program: program.clone(),
            options,
            editor: None,
            journal: PathBuf::new(),
            broken: None,
        }
    }

    fn editor(&mut self) -> Result<&mut Editor, AppError> {
        self.editor
            .as_mut()
            .ok_or_else(|| "the editor has not opened its level".into())
    }

    /// The journal's file of the level, as [`run`] says: `--journal`'s, or one of the editor's
    /// directory in the user's data directory ([`journal_in`]).
    fn journal_file(&self, world: &World, whole: &Path) -> Result<PathBuf, AppError> {
        if let Some(journal) = &self.options.journal {
            return Ok(journal.clone());
        }
        let metadata = slime_cook::scan::metadata_path(&self.options.level);
        let text = fs::read_to_string(&metadata)
            .map_err(|error| format!("{}: {error}", metadata.display()))?;
        let id = slime_cook::metadata::read(&text)
            .map_err(|error| format!("{}: {error}", metadata.display()))?;
        let directory = slime_app::data_directory()
            .ok_or(NO_DATA_DIRECTORY)?
            .join(DIRECTORY);
        Ok(journal_in(&directory, &id.to_string(), whole, |file| {
            let text = fs::read_to_string(file).ok()?;
            journal::level_of(world, &text).map(|(_, file)| file)
        }))
    }

    /// Notes that the journal cannot be written, `why`, after a change was made: the program stops.
    fn broke(&mut self, why: String) -> AppError {
        let error = format!("the editor's journal cannot be written, and the editor stops: {why}");
        self.broken = Some(why);
        error.into()
    }
}

/// The journal of the level whose whole path is `whole` and whose asset ID is `id`, in `directory`:
/// `<id>.journal`, `<id>.2.journal` and on, the first that does not exist or whose level is that
/// file, as `level_of` reads it from a journal's first entry; one that cannot be read is another
/// level's, and kept. A copy of a level, which has its asset ID, has a journal of its own so.
fn journal_in(
    directory: &Path,
    id: &str,
    whole: &Path,
    level_of: impl Fn(&Path) -> Option<String>,
) -> PathBuf {
    let whole = whole.display().to_string();
    let candidate = |count: u32| match count {
        1 => directory.join(format!("{id}.journal")),
        _ => directory.join(format!("{id}.{count}.journal")),
    };
    (1..)
        .map(candidate)
        .find(|journal| {
            fs::symlink_metadata(journal).is_err()
                || level_of(journal).as_deref() == Some(whole.as_str())
        })
        .expect("a journal's name is free past every journal")
}

impl Simulation for Session {
    fn start(&mut self, _: &mut SimulationContext<'_>, world: &mut World) -> Result<(), AppError> {
        (self.program.register)(world);
        let level = self.options.level.clone();
        let whole = crate::level::whole(&level);
        let assets = CookedAssets::open(&self.options.assets).map_err(|error| {
            format!(
                "the cooked data in {}: {error}",
                self.options.assets.display()
            )
        })?;
        let scenes = SceneLibrary::of(assets);
        self.journal = self.journal_file(world, &whole)?;
        let held = HeldFile::hold(self.journal.clone())?;
        let shown = self.journal.display().to_string();
        let discard =
            || format!("start with --discard to open the level from its file, {shown} set aside");
        if self.options.discard && fs::symlink_metadata(&self.journal).is_ok() {
            let aside = slime_app::set_aside(&self.journal, "discarded")
                .map_err(|error| format!("{shown} cannot be set aside: {error}"))?;
            tracing::info!(
                journal = %aside.display(),
                "the journal is set aside, and the level opened from its file"
            );
        }
        let kept = match fs::read(&self.journal) {
            Ok(bytes) => Some(String::from_utf8(bytes).map_err(|_| {
                format!(
                    "the journal {shown} is not text, which a crash may leave; {}",
                    discard()
                )
            })?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!("{shown} cannot be read: {error}; {}", discard()).into());
            }
        };
        if let Some(text) = &kept
            && let Some((key, file)) = journal::level_of(world, text)
            && (key != self.program.key || file != whole.display().to_string())
        {
            return Err(format!(
                "the journal {shown} is the journal of {file}, not of {}: give the journal of this \
                 level with --journal",
                whole.display()
            )
            .into());
        }
        let editor = match kept.filter(|text| journal::unwritten(text)) {
            Some(text) => {
                let editor = Editor::recover(
                    world,
                    scenes,
                    &level,
                    self.program.key,
                    &text,
                    Some(held),
                )
                // A journal of another level is refused above: a journal that cannot be read here is
                // one that a crash tore.
                .map_err(|error| match error {
                    EditError::Replay { .. } | EditError::Changed(_) | EditError::Journal(_) => {
                        format!(
                            "the journal {shown} holds what the level's file does not, and cannot \
                             be played again: {error}; {}",
                            discard()
                        )
                    }
                    error => format!("the journal {shown} cannot be played again: {error}"),
                })?;
                tracing::info!(
                    entries = editor.recovered(),
                    done = editor.done().len(),
                    "the editor recovered what its journal holds and the level's file does not"
                );
                editor
            }
            None => {
                let mut editor = Editor::open(world, scenes, &level, self.program.key)?;
                editor.keep_journal(world, held)?;
                editor
            }
        };
        tracing::info!(
            level = %level.display(),
            journal = %shown,
            "the editor opened its level"
        );
        self.editor = Some(editor);
        Ok(())
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, _: &mut World) {
        if let Some(why) = self.broken.take() {
            ctx.exit_with_error(format!(
                "the editor's journal cannot be written, and the editor stops so as not to go on \
                 with what a crash would lose: {why}"
            ));
        }
    }

    /// A command that the dev overlay or a tool sends is a step of the editor's history.
    fn apply(&mut self, world: &mut World, command: &Command) -> Result<(), AppError> {
        let label = format!("{} {}", command.kind(), command.entity());
        match self
            .editor()?
            .apply(world, &label, vec![command.clone()], None)
        {
            // The step is made: the program stops, as `broke` says.
            Err(EditError::Journal(why)) => {
                self.broke(why);
                Ok(())
            }
            made => made.map_err(Into::into),
        }
    }

    fn edit(&mut self, world: &mut World, edit: &Edit) -> Result<Reply, AppError> {
        let journal = self.journal.clone();
        let editor = self.editor()?;
        let identities = |texts: &[String]| -> Result<Vec<PersistentId>, AppError> {
            texts
                .iter()
                .map(|text| {
                    text.parse::<PersistentId>()
                        .map_err(|error| AppError::from(error.to_string()))
                })
                .collect()
        };
        let reply = match edit {
            Edit::Step {
                label,
                commands,
                selection,
            } => {
                let selection = selection.as_deref().map(identities).transpose()?;
                let texts: Vec<&str> = commands.iter().map(String::as_str).collect();
                editor
                    .apply_text(world, label, &texts, selection)
                    .map(|()| Reply::Done)
            }
            Edit::Undo => match editor.undo(world) {
                Ok(false) => return Err("no step is done".into()),
                undone => undone.map(|_| Reply::Done),
            },
            Edit::Redo => match editor.redo(world) {
                Ok(false) => return Err("no step is undone".into()),
                redone => redone.map(|_| Reply::Done),
            },
            Edit::History => Ok(Reply::History {
                history: EditorHistory {
                    level: editor.file().display().to_string(),
                    done: editor.done().map(str::to_owned).collect(),
                    undone: editor.undone().map(str::to_owned).collect(),
                    selection: editor.selection().iter().map(ToString::to_string).collect(),
                    recovered: editor.recovered() as u64,
                    journal: journal.display().to_string(),
                },
            }),
            Edit::Journal => Ok(Reply::Journal {
                text: fs::read_to_string(&journal)
                    .map_err(|error| format!("{}: {error}", journal.display()))?,
            }),
            Edit::Select { selection } => {
                editor.select(identities(selection)?);
                Ok(Reply::Done)
            }
            Edit::Write => editor.write(world).map(|left_out| Reply::Written {
                left_out: left_out
                    .into_iter()
                    .map(|aside| match aside.component {
                        Some(component) => {
                            format!("{}, {component}: {}", aside.entity, aside.reason)
                        }
                        None => format!("{}: {}", aside.entity, aside.reason),
                    })
                    .collect(),
            }),
        };
        match reply {
            // What was asked is made: the program stops, as `broke` says.
            Err(EditError::Journal(why)) => Err(self.broke(why)),
            reply => reply.map_err(Into::into),
        }
    }
}

#[cfg(test)]
mod tests {
    use slime_app::Host;

    use super::*;

    fn program() -> Program {
        Program {
            name: "editor",
            level: PathBuf::from("yard.scene"),
            key: "yard",
            assets: PathBuf::from("cooked"),
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
                "--discard"
            ]),
            Ok(Some(Options {
                level: PathBuf::from("a.scene"),
                assets: PathBuf::from("b"),
                journal: Some(PathBuf::from("c")),
                discard: true,
            }))
        );
        assert_eq!(parsed(&["--help"]), Ok(None));
        assert_eq!(parsed(&["--level"]), Err("--level needs a path".to_owned()));
        assert_eq!(
            parsed(&["--frames", "2"]),
            Err("unknown option --frames".to_owned())
        );
    }
    /// A yard of its own, without prefabs, and empty cooked data, in a directory of the test's own.
    struct Yard {
        directory: PathBuf,
    }

    impl Yard {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "slime-editor-program-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&directory);
            fs::create_dir_all(directory.join("cooked")).expect("a directory");
            fs::write(
                directory.join("cooked").join(slime_assets::MANIFEST_FILE),
                slime_assets::Manifest::new().to_bytes(),
            )
            .expect("an empty manifest");
            fs::write(
                directory.join("yard.scene"),
                "# The yard.\n\n[entities]\nwell = { components = { Name = \"well\" } }\n",
            )
            .expect("the yard");
            fs::write(
                directory.join("yard.scene.meta"),
                "id = \"00000000-0000-0000-0000-000000000001\"\n",
            )
            .expect("its metadata");
            Self { directory }
        }

        fn journal(&self) -> PathBuf {
            self.directory.join("yard.journal")
        }

        /// A session of the yard, started with `arguments` and its journal.
        fn start(&self, arguments: &[&str]) -> Result<(Host, Session), String> {
            let program = Program {
                name: "editor",
                level: self.directory.join("yard.scene"),
                key: "yard",
                assets: self.directory.join("cooked"),
                register: |_| {},
            };
            let mut arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
            arguments.extend(["--journal".into(), self.journal().into_os_string()]);
            let options = parse(&program, arguments)
                .expect("options")
                .expect("not --help");
            let mut session = Session::new(&program, options);
            let mut host = Host::new(&AppConfig::default());
            host.start(&mut session)
                .map_err(|error| error.to_string())?;
            Ok((host, session))
        }
    }

    impl Drop for Yard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn step(host: &mut Host, session: &mut Session, label: &str, command: &str) {
        let edit = Edit::Step {
            label: label.to_owned(),
            commands: vec![command.to_owned()],
            selection: None,
        };
        session.edit(host.world_mut(), &edit).expect("made");
    }

    fn done(host: &mut Host, session: &mut Session) -> (Vec<String>, u64) {
        match session
            .edit(host.world_mut(), &Edit::History)
            .expect("the history")
        {
            Reply::History { history } => (history.done, history.recovered),
            reply => panic!("{reply:?}"),
        }
    }

    #[test]
    fn a_session_recovers_what_its_journal_holds_that_the_file_does_not_and_only_that() {
        let yard = Yard::new("recovers");
        let (mut host, mut session) = yard.start(&[]).expect("started");
        step(
            &mut host,
            &mut session,
            "Rename",
            "[SetProperty]\nentity = \"yard/well\"\ncomponent = \"Name\"\npath = \"0\"\nvalue = \"old well\"\n",
        );
        host.stop(&mut session);
        drop(session);
        // Stopped without writing: the step comes back, with its history.
        let (mut host, mut session) = yard.start(&[]).expect("started again");
        assert_eq!(
            done(&mut host, &mut session),
            (vec!["Rename".to_owned()], 1)
        );
        session
            .edit(host.world_mut(), &Edit::Write)
            .expect("written");
        assert!(
            fs::read_to_string(yard.directory.join("yard.scene"))
                .expect("the yard")
                .contains("old well")
        );
        drop(session);
        // Nothing unwritten: the level opens from its file, its journal anew.
        let (mut host, mut session) = yard.start(&[]).expect("started again");
        assert_eq!(done(&mut host, &mut session), (Vec::new(), 0));
    }

    #[test]
    fn a_journal_that_cannot_be_played_again_stays_as_it_was_and_discard_sets_it_aside() {
        let yard = Yard::new("kept");
        let (mut host, mut session) = yard.start(&[]).expect("started");
        step(
            &mut host,
            &mut session,
            "Rename",
            "[SetProperty]\nentity = \"yard/well\"\ncomponent = \"Name\"\npath = \"0\"\nvalue = \"old well\"\n",
        );
        drop(session);
        let journal = fs::read(yard.journal()).expect("the journal");
        // The level changed apart from the editor: its journal cannot be played on it.
        fs::write(yard.directory.join("yard.scene"), "[entities]\n").expect("changed by hand");
        let Err(why) = yard.start(&[]) else {
            panic!("a journal of another text of the level played again");
        };
        assert!(why.contains("--discard"), "{why}");
        assert_eq!(
            fs::read(yard.journal()).expect("kept"),
            journal,
            "the journal is kept whole"
        );
        // Twice set aside, neither over the other.
        for aside in ["yard.journal.discarded", "yard.journal.discarded.2"] {
            let (mut host, mut session) = yard.start(&["--discard"]).expect("discarded");
            assert_eq!(done(&mut host, &mut session), (Vec::new(), 0));
            step(
                &mut host,
                &mut session,
                "Again",
                "[Create]\nentity = \"yard/torch-0rdx\"\nparent = \"yard\"\n",
            );
            drop(session);
            assert!(yard.directory.join(aside).is_file(), "{aside}");
        }
        assert_eq!(
            fs::read(yard.directory.join("yard.journal.discarded")).expect("set aside"),
            journal
        );
    }

    #[test]
    fn a_journal_that_is_not_text_stops_the_start_and_discard_sets_it_aside() {
        let yard = Yard::new("not-text");
        fs::write(yard.journal(), b"# level\nkey = \"caf\xC3").expect("a torn journal");
        let Err(why) = yard.start(&[]) else {
            panic!("a journal that is not text started");
        };
        assert!(
            why.contains("not text") && why.contains("--discard"),
            "{why}"
        );
        yard.start(&["--discard"]).expect("set aside");
        assert!(yard.directory.join("yard.journal.discarded").is_file());
    }

    #[test]
    fn a_journal_whose_last_entry_is_torn_stops_the_start_and_stays_as_it_was() {
        let yard = Yard::new("torn");
        let (mut host, mut session) = yard.start(&[]).expect("started");
        step(
            &mut host,
            &mut session,
            "Light",
            "[Create]\nentity = \"yard/torch-0rdx\"\nparent = \"yard\"\n",
        );
        drop(session);
        let mut journal = fs::read(yard.journal()).expect("the journal");
        journal.truncate(journal.len() - 4);
        fs::write(yard.journal(), &journal).expect("torn");
        let Err(why) = yard.start(&[]) else {
            panic!("a torn journal played again");
        };
        assert!(why.contains("--discard"), "{why}");
        assert_eq!(fs::read(yard.journal()).expect("kept"), journal);
    }

    #[test]
    fn one_editor_at_a_time_holds_a_journal_and_another_level_s_is_refused() {
        let yard = Yard::new("held");
        let (_host, first) = yard.start(&[]).expect("started");
        let Err(why) = yard.start(&[]) else {
            panic!("two editors of one journal");
        };
        assert!(why.contains("another program holds"), "{why}");
        drop(first);
        // A journal of the yard, given to another level under the yard's key.
        let other = Yard::new("held-other");
        fs::copy(yard.journal(), other.journal()).expect("copied");
        let Err(why) = other.start(&[]) else {
            panic!("another level's journal taken");
        };
        assert!(why.contains("is the journal of"), "{why}");
    }

    #[test]
    fn copies_of_a_level_take_journals_of_their_own() {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-copies-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a directory");
        let level_of = |journal: &Path| fs::read_to_string(journal).ok();
        let (a, b) = (
            Path::new("/levels/a/yard.scene"),
            Path::new("/levels/b/yard.scene"),
        );
        let first = journal_in(&directory, "id", a, level_of);
        assert_eq!(first, directory.join("id.journal"));
        fs::write(&first, "/levels/a/yard.scene").expect("the journal of a");
        assert_eq!(journal_in(&directory, "id", a, level_of), first);
        let second = journal_in(&directory, "id", b, level_of);
        assert_eq!(second, directory.join("id.2.journal"));
        fs::write(&second, "/levels/b/yard.scene").expect("the journal of b");
        assert_eq!(journal_in(&directory, "id", b, level_of), second);
        fs::remove_dir_all(&directory).expect("removed");
    }
}
