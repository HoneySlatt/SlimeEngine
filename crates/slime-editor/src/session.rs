//! The editor's core as the simulation of its program: the editor of the level that is open, its
//! journal, and the one handler of what the window and the development channel ask
//! (`docs/adr/0135-the-editors-program.md`,
//! `docs/adr/0156-the-editors-window-and-its-operations.md`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use slime_app::{AppError, HeldFile, Simulation, SimulationContext};
use slime_assets::CookedAssets;
use slime_channel::{Edit, EditorHistory, OperationSummary, Reply};
use slime_world::{Command, PersistentId, SceneLibrary, World};

use crate::editor::{EditError, Editor};
use crate::operation::{self, Changes, OPERATIONS};
use crate::program::{Options, Program};
use crate::shared::{Notice, Request, Shared};
use crate::{journal, level, recent};

/// Why the program has no journal when the environment gives no data directory.
const NO_DATA_DIRECTORY: &str =
    "the environment gives no data directory: give the journal's file with --journal";

/// The directory of the user's data and state directories where the editor keeps its journals and
/// its lists of recent levels.
pub(crate) const DIRECTORY: &str = "slime-editor";

/// What the first run of a program tells the user, once.
const HINT: &str = "Ctrl+Shift+P finds every operation of the editor by its name.";

/// The editor's program as a simulation: the editor's core, once a level is opened.
pub(crate) struct Session {
    program: Program,
    options: Options,
    editor: Option<Editor>,
    journal: PathBuf,
    /// Why the journal cannot be written, once it could not: the program stops at the next frame,
    /// so as not to go on with what a crash would lose.
    broken: Option<String>,
    /// What the core shares with the window.
    shared: Shared,
    /// Whether an operation asked the program to quit, which the next update does.
    quit: bool,
    /// The file of the levels opened last, in the user's state directory; none to keep no list.
    recent: Option<PathBuf>,
    /// The directory of the levels' journals, in the user's data directory; none when the
    /// environment gives none, where `--journal` names the first level's alone.
    journals: Option<PathBuf>,
}

impl Session {
    /// The session of `program`, run with `options`, sharing `shared` with its window, keeping
    /// the levels opened last in `recent` and the levels' journals in `journals`, but the one that
    /// `--journal` names.
    pub(crate) fn new(
        program: &Program,
        options: Options,
        shared: Shared,
        recent: Option<PathBuf>,
        journals: Option<PathBuf>,
    ) -> Self {
        shared.borrow_mut().program = program.name;
        shared.borrow_mut().lighting = Some(program.lighting);
        Self {
            program: program.clone(),
            options,
            editor: None,
            journal: PathBuf::new(),
            broken: None,
            shared,
            quit: false,
            recent,
            journals,
        }
    }

    fn editor(&mut self) -> Result<&mut Editor, AppError> {
        self.editor
            .as_mut()
            .ok_or_else(|| "the editor has not opened its level".into())
    }

    /// The journal's file of the level `level`, whose whole path is `whole`: `journal` when it is
    /// given, else one of the session's directory of journals ([`journal_in`]).
    fn journal_file(
        &self,
        world: &World,
        level: &Path,
        whole: &Path,
        journal: Option<&Path>,
    ) -> Result<PathBuf, AppError> {
        if let Some(journal) = journal {
            return Ok(journal.to_owned());
        }
        let metadata = slime_cook::scan::metadata_path(level);
        let text = fs::read_to_string(&metadata)
            .map_err(|error| format!("{}: {error}", metadata.display()))?;
        let id = slime_cook::metadata::read(&text)
            .map_err(|error| format!("{}: {error}", metadata.display()))?;
        let directory = self.journals.as_deref().ok_or(NO_DATA_DIRECTORY)?;
        Ok(journal_in(directory, &id.to_string(), whole, |file| {
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

    /// Opens the level `level` into `world`, which holds the program's types and nothing else, with
    /// the journal `journal`, or the level's own in the user's data directory, set aside first with
    /// `discard`: its editor, recovered from the journal when it holds steps that the level's file
    /// does not, and its journal's file. `world` holds what was opened when it fails.
    fn open(
        &self,
        world: &mut World,
        level: &Path,
        journal: Option<&Path>,
        discard: bool,
    ) -> Result<(Editor, PathBuf), AppError> {
        let whole = level::whole(level);
        let assets = CookedAssets::open(&self.options.assets).map_err(|error| {
            format!(
                "the cooked data in {}: {error}",
                self.options.assets.display()
            )
        })?;
        // The viewport loads the meshes that the level places from the same data.
        self.shared.borrow_mut().assets = Some(assets.clone());
        let scenes = SceneLibrary::of(assets);
        let journal = self.journal_file(world, level, &whole, journal)?;
        let held = HeldFile::hold(journal.clone())?;
        let shown = journal.display().to_string();
        let discard_it =
            || format!("start with --discard to open the level from its file, {shown} set aside");
        if discard && fs::symlink_metadata(&journal).is_ok() {
            let aside = slime_app::set_aside(&journal, "discarded")
                .map_err(|error| format!("{shown} cannot be set aside: {error}"))?;
            tracing::info!(
                journal = %aside.display(),
                "the journal is set aside, and the level opened from its file"
            );
        }
        let kept = match fs::read(&journal) {
            Ok(bytes) => Some(String::from_utf8(bytes).map_err(|_| {
                format!(
                    "the journal {shown} is not text, which a crash may leave; {}",
                    discard_it()
                )
            })?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!("{shown} cannot be read: {error}; {}", discard_it()).into());
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
                    level,
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
                            discard_it()
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
                let mut editor = Editor::open(world, scenes, level, self.program.key)?;
                editor.keep_journal(world, held)?;
                editor
            }
        };
        tracing::info!(
            level = %level.display(),
            journal = %shown,
            "the editor opened its level"
        );
        Ok((editor, journal))
    }

    /// Opens the level `level` in place of the one open, as [`Session::open`] does, into a new
    /// world that replaces `world`: the level open stays as it is when the new one cannot be
    /// opened, and its journal keeps what its file does not hold, which opening it again
    /// recovers. The level that the program opened first keeps the journal that `--journal`
    /// names; another has its own. Opening the level that is open does nothing.
    fn open_other(&mut self, world: &mut World, level: &Path) -> Result<(), AppError> {
        let whole = level::whole(level);
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| level::whole(editor.file()) == whole)
        {
            return Ok(());
        }
        let journal = (whole == level::whole(&self.options.level))
            .then(|| self.options.journal.clone())
            .flatten();
        let mut fresh = World::new();
        (self.program.register)(&mut fresh);
        let (editor, journal) = self.open(&mut fresh, level, journal.as_deref(), false)?;
        *world = fresh;
        self.editor = Some(editor);
        self.journal = journal;
        self.shared.borrow_mut().opened += 1;
        self.opened(&whole);
        Ok(())
    }

    /// Puts `whole`, a level just opened, first among the recent levels, and writes the list.
    fn opened(&mut self, whole: &Path) {
        let mut shared = self.shared.borrow_mut();
        shared.recent = recent::opened(&shared.recent, whole);
        if let Some(file) = &self.recent
            && let Err(error) = recent::write(file, &shared.recent)
        {
            tracing::warn!(file = %file.display(), %error, "the recent levels cannot be written");
        }
    }

    /// Writes into the shared value what the window shows of the core.
    fn publish(&self) {
        let mut shared = self.shared.borrow_mut();
        if let Some(editor) = &self.editor {
            shared.level = Some(editor.file().to_owned());
            shared.done = editor.done().map(str::to_owned).collect();
            shared.undone = editor.undone().map(str::to_owned).collect();
            shared.unwritten = editor.unwritten();
            shared.selection = editor.selection().to_vec();
        }
        shared.revision += 1;
    }

    /// Answers `request`, whether the window or the development channel made it: the one handler
    /// of the core (`docs/adr/0156-the-editors-window-and-its-operations.md`). A journal that
    /// cannot be written once the change is made stops the program, as [`Session::broke`] says.
    ///
    /// # Errors
    ///
    /// Why the request cannot be answered: a step refused, nothing to undo, an operation that
    /// does not exist or given other arguments, a window's operation without a window.
    pub(crate) fn handle(
        &mut self,
        world: &mut World,
        request: Request,
    ) -> Result<Reply, AppError> {
        let answered = self.answer(world, request);
        self.publish();
        answered
    }

    fn answer(&mut self, world: &mut World, request: Request) -> Result<Reply, AppError> {
        let journal = self.journal.clone();
        let reply = match request {
            Request::Step {
                label,
                commands,
                selection,
            } => {
                let texts: Vec<&str> = commands.iter().map(String::as_str).collect();
                self.editor()?
                    .apply_text(world, &label, &texts, selection)
                    .map(|()| Reply::Done)
            }
            Request::Apply { label, commands } => self
                .editor()?
                .apply(world, &label, commands, None)
                .map(|()| Reply::Done),
            Request::Undo => match self.editor()?.undo(world) {
                Ok(false) => return Err("no step is done".into()),
                undone => undone.map(|_| Reply::Done),
            },
            Request::Redo => match self.editor()?.redo(world) {
                Ok(false) => return Err("no step is undone".into()),
                redone => redone.map(|_| Reply::Done),
            },
            Request::History => {
                let editor = self.editor()?;
                Ok(Reply::History {
                    history: EditorHistory {
                        level: editor.file().display().to_string(),
                        done: editor.done().map(str::to_owned).collect(),
                        undone: editor.undone().map(str::to_owned).collect(),
                        selection: editor.selection().iter().map(ToString::to_string).collect(),
                        recovered: editor.recovered() as u64,
                        journal: journal.display().to_string(),
                        unwritten: editor.unwritten() as u64,
                    },
                })
            }
            Request::Journal => Ok(Reply::Journal {
                text: fs::read_to_string(&journal)
                    .map_err(|error| format!("{}: {error}", journal.display()))?,
            }),
            Request::Select(selection) => {
                self.editor()?.select(selection);
                Ok(Reply::Done)
            }
            Request::Write => self.editor()?.write(world).map(|left_out| Reply::Written {
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
            Request::Operations => Ok(Reply::Operations {
                operations: OPERATIONS
                    .iter()
                    .map(|operation| OperationSummary {
                        name: operation.name.to_owned(),
                        label: operation.label.to_owned(),
                        parameters: operation
                            .parameters
                            .iter()
                            .map(|&parameter| parameter.to_owned())
                            .collect(),
                        changes: operation.changes.word().to_owned(),
                    })
                    .collect(),
            }),
            Request::Run { name, arguments } => return self.run(world, &name, arguments),
        };
        match reply {
            // What was asked is made: the program stops, as `broke` says.
            Err(EditError::Journal(why)) => Err(self.broke(why)),
            reply => reply.map_err(Into::into),
        }
    }

    /// Runs the operation `name` with `arguments`, as [`Session::handle`] says.
    fn run(
        &mut self,
        world: &mut World,
        name: &str,
        arguments: Vec<String>,
    ) -> Result<Reply, AppError> {
        let Some(operation) = operation::operation(name) else {
            return Err(format!("the editor has no operation `{name}`").into());
        };
        if arguments.len() != operation.parameters.len() {
            return Err(format!(
                "`{name}` takes {} arguments, {}, not {}",
                operation.parameters.len(),
                operation.parameters.join(", "),
                arguments.len()
            )
            .into());
        }
        if operation.changes == Changes::Window {
            let mut shared = self.shared.borrow_mut();
            if !shared.window {
                return Err(
                    format!("`{name}` needs the editor's window, which it runs without").into(),
                );
            }
            shared.window_operations.push_back(operation.name);
            return Ok(Reply::Done);
        }
        let request = match operation.name {
            "file.open" => {
                self.open_other(world, Path::new(&arguments[0]))?;
                return Ok(Reply::Done);
            }
            "file.quit" => {
                self.quit = true;
                return Ok(Reply::Done);
            }
            "file.save" => Request::Write,
            "edit.undo" => Request::Undo,
            "edit.redo" => Request::Redo,
            "edit.select_none" => Request::Select(Vec::new()),
            name => unreachable!("the operation `{name}` has no request"),
        };
        self.answer(world, request)
    }

    /// Tells the window `text`, a failure or not.
    fn tell(&self, failure: bool, text: String) {
        self.shared
            .borrow_mut()
            .notices
            .push(Notice { failure, text });
    }
}

/// The journal of the level whose whole path is `whole` and whose asset ID is `id`, in `directory`:
/// `<id>.journal`, `<id>.2.journal` and on, the first that does not exist or whose level is that
/// file, as `level_of` reads it from a journal's first entry; one that cannot be read is another
/// level's, and kept. A copy of a level, which has its asset ID, has a journal of its own so.
pub(crate) fn journal_in(
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

/// The request that the development channel's `edit` makes.
fn request_of(edit: &Edit) -> Result<Request, AppError> {
    let identities = |texts: &[String]| -> Result<Vec<PersistentId>, AppError> {
        texts
            .iter()
            .map(|text| {
                text.parse::<PersistentId>()
                    .map_err(|error| AppError::from(error.to_string()))
            })
            .collect()
    };
    Ok(match edit {
        Edit::Step {
            label,
            commands,
            selection,
        } => Request::Step {
            label: label.clone(),
            commands: commands.clone(),
            selection: selection.as_deref().map(identities).transpose()?,
        },
        Edit::Undo => Request::Undo,
        Edit::Redo => Request::Redo,
        Edit::History => Request::History,
        Edit::Journal => Request::Journal,
        Edit::Select { selection } => Request::Select(identities(selection)?),
        Edit::Write => Request::Write,
        Edit::Operations => Request::Operations,
        Edit::Run { name, arguments } => Request::Run {
            name: name.clone(),
            arguments: arguments.clone(),
        },
    })
}

impl Simulation for Session {
    fn start(&mut self, _: &mut SimulationContext<'_>, world: &mut World) -> Result<(), AppError> {
        (self.program.register)(world);
        if let Some(sources) = &self.program.sources
            && !self.options.assets.exists()
        {
            tracing::info!(
                sources = %sources.display(),
                cooked = %self.options.assets.display(),
                "the cooked data is missing: the editor cooks it first"
            );
            slime_cook::cook(sources, &self.options.assets).map_err(|error| {
                format!(
                    "{} cannot be cooked into {}: {error}",
                    sources.display(),
                    self.options.assets.display()
                )
            })?;
        }
        let first_run = match &self.recent {
            Some(file) => {
                let first = fs::symlink_metadata(file).is_err();
                match recent::read(file) {
                    Ok(levels) => self.shared.borrow_mut().recent = levels,
                    Err(error) => {
                        tracing::warn!(file = %file.display(), %error, "the recent levels cannot be read");
                    }
                }
                first
            }
            None => false,
        };
        let level = self.options.level.clone();
        let (editor, journal) = self.open(
            world,
            &level,
            self.options.journal.as_deref(),
            self.options.discard,
        )?;
        self.editor = Some(editor);
        self.journal = journal;
        self.opened(&level::whole(&level));
        if first_run {
            self.tell(false, HINT.to_owned());
        }
        self.publish();
        Ok(())
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let requests = std::mem::take(&mut self.shared.borrow_mut().requests);
        for request in requests {
            if let Err(error) = self.handle(world, request) {
                self.tell(true, error.to_string());
            }
            // What follows would go unjournaled: the program stops instead.
            if self.broken.is_some() {
                break;
            }
        }
        if self.quit {
            ctx.request_exit();
        }
        if let Some(why) = self.broken.take() {
            ctx.exit_with_error(format!(
                "the editor's journal cannot be written, and the editor stops so as not to go on \
                 with what a crash would lose: {why}"
            ));
        }
    }

    /// A command that a tool sends, or the presentation through `Context::send`, is a step of the
    /// editor's history.
    fn apply(&mut self, world: &mut World, command: &Command) -> Result<(), AppError> {
        let label = format!("{} {}", command.kind(), command.entity());
        let request = Request::Apply {
            label,
            commands: vec![command.clone()],
        };
        match self.handle(world, request) {
            // The step is made: the program stops, as `broke` says.
            Err(_) if self.broken.is_some() => Ok(()),
            made => made.map(|_| ()),
        }
    }

    fn edit(&mut self, world: &mut World, edit: &Edit) -> Result<Reply, AppError> {
        let request = request_of(edit)?;
        self.handle(world, request)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use slime_app::{AppConfig, Host};

    use super::*;
    use crate::program::Lighting;

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
                sources: None,
                register: |_| {},
                lighting: Lighting::default(),
            };
            let mut arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
            arguments.extend(["--journal".into(), self.journal().into_os_string()]);
            let options = crate::program::parse(&program, arguments)
                .expect("options")
                .expect("not --help");
            let mut session = Session::new(
                &program,
                options,
                Shared::default(),
                None,
                Some(self.directory.join("journals")),
            );
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
