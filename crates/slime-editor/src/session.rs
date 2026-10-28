//! The editor's core as the simulation of its program: the editor of the level that is open, its
//! journal, and the one handler of what the window and the development channel ask
//! (`docs/adr/0135-the-editors-program.md`,
//! `docs/adr/0156-the-editors-window-and-its-operations.md`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use slime_app::{AppError, HeldFile, Simulation, SimulationContext, Waker};
use slime_assets::{AssetId, AssetKind, CookedAssets};
use slime_channel::{Edit, EditorHistory, OperationSummary, Reply};
use slime_reflect::{Path as ValuePath, Reflect};
use slime_render::glam::DVec3;
use slime_world::{
    Command, Entity, GlobalTransform, Name, PersistentId, SceneLibrary, Transform, World,
};
use slime_world_render::Picker;

use crate::editor::{EditError, Editor};
use crate::handles;
use crate::operation::{self, Changes, OPERATIONS};
use crate::play::{Control, Event, Player};
use crate::program::Played;
use crate::program::{Options, Program};
use crate::shared::{CONSOLE_LINES, Notice, Playing, Request, Shared};
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
    /// What casts the rays that drop what is selected, made at the first drop
    /// (`edit.drop`).
    picker: Option<Picker>,
    /// The last operation that made a step, with its arguments, which `edit.repeat` runs again.
    last: Option<(&'static str, Vec<String>)>,
    /// The game that Play runs, while it runs, and how many times Play started, which names its
    /// directories (`docs/adr/0164-play.md`).
    player: Option<Player>,
    plays: u32,
    /// What wakes the program's loop from Play's thread, as the program starts.
    waker: Option<Waker>,
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
            picker: None,
            last: None,
            player: None,
            plays: 0,
            waker: None,
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
        // The picker reads the cooked data opened again with the level.
        self.picker = None;
        self.shared.borrow_mut().opened += 1;
        self.opened(&whole);
        self.list_prefabs(world);
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
            let done: Vec<String> = editor.done().map(str::to_owned).collect();
            let undone: Vec<String> = editor.undone().map(str::to_owned).collect();
            if done != shared.done || undone != shared.undone {
                shared.history += 1;
            }
            shared.done = done;
            shared.undone = undone;
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
            Request::Gesture(commands) => self
                .editor()?
                .gesture(world, commands)
                .map(|()| Reply::Done),
            Request::EndGesture(label) => self
                .editor()?
                .end_gesture(world, &label)
                .map(|()| Reply::Done),
            Request::CancelGesture => {
                self.editor()?.cancel_gesture(world);
                Ok(Reply::Done)
            }
            Request::Apply {
                label,
                commands,
                selection,
            } => self
                .editor()?
                .apply(world, &label, commands, selection)
                .map(|()| Reply::Done),
            Request::Paste { label, text } => self
                .editor()?
                .paste(world, &label, &text)
                .map(|_| Reply::Done),
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
        // A window pastes what the system's clipboard holds, which it reads.
        if operation.changes == Changes::Window
            || (operation.name == "edit.paste" && self.shared.borrow().window)
        {
            if operation.changes == Changes::Level {
                self.last = Some((operation.name, arguments));
            }
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
            "edit.drop" | "edit.drop_bottom" => {
                let bottom = operation.name == "edit.drop_bottom";
                Request::Apply {
                    label: operation.label.to_owned(),
                    commands: self.drops(world, bottom)?,
                    selection: None,
                }
            }
            "edit.copy" => {
                let text = self.copied(world)?;
                let mut shared = self.shared.borrow_mut();
                shared.clipboard = Some(text);
                shared.copies += 1;
                return Ok(Reply::Done);
            }
            "edit.paste" => {
                let text = self.shared.borrow().clipboard.clone();
                Request::Paste {
                    label: operation.label.to_owned(),
                    text: text.ok_or("nothing was copied")?,
                }
            }
            "edit.duplicate" => Request::Paste {
                label: operation.label.to_owned(),
                text: self.copied(world)?,
            },
            "edit.create" => self.created(world)?,
            "edit.delete" => {
                let commands: Vec<Command> = self
                    .outermost(world)?
                    .into_iter()
                    .filter_map(|entity| world.persistent_id(entity))
                    .map(|entity| Command::Delete { entity })
                    .collect();
                Request::Apply {
                    label: operation.label.to_owned(),
                    commands,
                    selection: Some(Vec::new()),
                }
            }
            "edit.group" => self.grouped(world)?,
            "edit.place" => self.placed(world, &arguments[0])?,
            "edit.go_to_step" => return self.go_to_step(world, &arguments[0]),
            "play.start" => return self.play(world),
            "play.pause" | "play.step" | "play.speed" | "play.stop" => {
                return self.control(operation.name, &arguments);
            }
            "edit.repeat" => {
                let (name, arguments) = self.last.clone().ok_or("no operation made a step yet")?;
                return self.run(world, name, arguments);
            }
            name if name.starts_with("edit.nudge_") => {
                let direction = match name {
                    "edit.nudge_left" => DVec3::NEG_X,
                    "edit.nudge_right" => DVec3::X,
                    "edit.nudge_forward" => DVec3::NEG_Z,
                    "edit.nudge_back" => DVec3::Z,
                    "edit.nudge_up" => DVec3::Y,
                    _ => DVec3::NEG_Y,
                };
                Request::Apply {
                    label: operation.label.to_owned(),
                    commands: self.nudged(world, direction * handles::SNAP_DISTANCE)?,
                    selection: None,
                }
            }
            name => unreachable!("the operation `{name}` has no request"),
        };
        let reply = self.answer(world, request)?;
        if operation.changes == Changes::Level {
            self.last = Some((operation.name, arguments));
        }
        Ok(reply)
    }

    /// The outermost of the entities selected below the level's root, each without an ancestor
    /// among them (`docs/adr/0162-creating-copying-and-arranging.md`).
    ///
    /// # Errors
    ///
    /// Without a level open, or when nothing below the level's root is selected.
    fn outermost(&self, world: &World) -> Result<Vec<Entity>, AppError> {
        let editor = self.editor.as_ref().ok_or("no level is open")?;
        let selected: Vec<Entity> = editor
            .selection()
            .iter()
            .filter(|id| id.parts().len() > 1)
            .filter_map(|id| world.find(id))
            .collect();
        let outermost = handles::outermost(world, &selected);
        if outermost.is_empty() {
            return Err("nothing below the level's root is selected".into());
        }
        Ok(outermost)
    }

    /// The text that copies the outermost of what is selected ([`Editor::copy`]).
    fn copied(&self, world: &World) -> Result<String, AppError> {
        let roots: Vec<PersistentId> = self
            .outermost(world)?
            .into_iter()
            .filter_map(|entity| world.persistent_id(entity))
            .collect();
        let editor = self.editor.as_ref().ok_or("no level is open")?;
        Ok(editor.copy(world, &roots)?)
    }

    /// The step that creates an entity named `entity` at the level's root, at the middle of what is
    /// selected or at the origin, and selects it.
    fn created(&self, world: &World) -> Result<Request, AppError> {
        let editor = self.editor.as_ref().ok_or("no level is open")?;
        let root = editor.root(world).ok_or("the level's root is not alive")?;
        let at = self
            .outermost(world)
            .ok()
            .and_then(|selected| handles::middle(world, &selected))
            .unwrap_or(DVec3::ZERO);
        let id = editor.new_id(world, "entity");
        let transform = Transform {
            translation: handles::into_parent(&handles::placed_at(world, Some(root)), at),
            ..Transform::IDENTITY
        };
        Ok(Request::Apply {
            label: "Create an entity".to_owned(),
            commands: vec![Command::Create {
                entity: id.clone(),
                parent: Some(PersistentId::new([editor.key()])),
                rank: None,
                components: vec![
                    ("Name".to_owned(), Name("entity".to_owned()).to_value()),
                    ("Transform".to_owned(), transform.to_value()),
                ],
            }],
            selection: Some(vec![id]),
        })
    }

    /// The step that puts the outermost of what is selected under a new entity named `group`, at
    /// their middle, under the parent that they share or at the level's root, each where it was in
    /// the world, and selects the group.
    ///
    /// # Errors
    ///
    /// As [`Session::outermost`], and for an entity that a placement holds but for its root, which
    /// a scene file cannot move out of its place.
    fn grouped(&self, world: &World) -> Result<Request, AppError> {
        let selected = self.outermost(world)?;
        if let Some(&held) = selected.iter().find(|&&entity| {
            world
                .placement_root(entity)
                .is_some_and(|root| root != entity)
        }) {
            let id = world.persistent_id(held).map(|id| id.to_string());
            return Err(format!(
                "`{}` belongs to a placement, out of which it cannot be moved",
                id.unwrap_or_default()
            )
            .into());
        }
        let editor = self.editor.as_ref().ok_or("no level is open")?;
        let root = editor.root(world).ok_or("the level's root is not alive")?;
        let first = world.parent(selected[0]);
        let parent = if selected.iter().all(|&entity| world.parent(entity) == first) {
            first.unwrap_or(root)
        } else {
            root
        };
        let parent_id = world
            .persistent_id(parent)
            .ok_or("the parent has no identity")?;
        let around = handles::placed_at(world, Some(parent));
        let pivot = handles::middle(world, &selected).unwrap_or(around.translation);
        let id = editor.new_id(world, "group");
        // The group turned and scaled as its parent is, where the middle is.
        let group = GlobalTransform {
            translation: pivot,
            ..around
        };
        let mut commands = vec![Command::Create {
            entity: id.clone(),
            parent: Some(parent_id),
            rank: None,
            components: vec![
                ("Name".to_owned(), Name("group".to_owned()).to_value()),
                (
                    "Transform".to_owned(),
                    Transform {
                        translation: handles::into_parent(&around, pivot),
                        ..Transform::IDENTITY
                    }
                    .to_value(),
                ),
            ],
        }];
        for entity in selected {
            let Some(entity_id) = world.persistent_id(entity) else {
                continue;
            };
            commands.push(Command::Reparent {
                entity: entity_id.clone(),
                parent: Some(id.clone()),
                rank: None,
            });
            // Where it was in the world, if it is placed.
            if let (Some(local), Some(at)) = (
                world.get::<Transform>(entity),
                world.interpolated_transform(entity, 1.0),
            ) {
                let moved = Transform {
                    translation: handles::into_parent(&group, at.translation),
                    rotation: (group.rotation.as_dquat().inverse() * at.rotation.as_dquat())
                        .normalize()
                        .as_quat(),
                    scale: at.scale / group.scale.max(f32::EPSILON),
                    ..*local
                };
                commands.push(set_transform(entity_id, &moved));
            }
        }
        Ok(Request::Apply {
            label: "Group".to_owned(),
            commands,
            selection: Some(vec![id]),
        })
    }

    /// The step that places the prefab `prefab`, its asset ID or its name, as the palette shows
    /// it, at the level's root, its root at the middle of what is selected or at the world's
    /// origin, not turned nor scaled, under an identity named after its file, and selects it
    /// (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).
    ///
    /// # Errors
    ///
    /// Without a level open, for a prefab that the level cannot place, one that cannot be resolved,
    /// and one that places the level, which would then hold itself.
    fn placed(&mut self, world: &World, prefab: &str) -> Result<Request, AppError> {
        let asked: Option<AssetId> = prefab.parse().ok();
        let (id, name) = self
            .shared
            .borrow()
            .prefabs
            .iter()
            .find(|(id, name)| name == prefab || asked == Some(*id))
            .cloned()
            .ok_or_else(|| format!("`{prefab}` is not a prefab that the level can place"))?;
        let at = self
            .outermost(world)
            .ok()
            .and_then(|selected| handles::middle(world, &selected))
            .unwrap_or(DVec3::ZERO);
        let editor = self.editor.as_mut().ok_or("no level is open")?;
        let root = editor.root(world).ok_or("the level's root is not alive")?;
        let resolved = editor
            .scenes()
            .resolve(world.registry(), id)
            .map_err(|error| format!("`{name}` cannot be placed: {error}"))?;
        if resolved.places(editor.asset()) {
            return Err(format!("`{name}` places the level, which would then hold itself").into());
        }
        // Named after its file's name, without its directories and its extensions.
        let file = name.split(": ").next().unwrap_or(&name);
        let base = file
            .rsplit('/')
            .next()
            .and_then(|file| file.split('.').next())
            .filter(|base| !base.is_empty())
            .unwrap_or("prefab");
        let placement = editor.new_id(world, base);
        let transform = Transform {
            translation: handles::into_parent(&handles::placed_at(world, Some(root)), at),
            ..Transform::IDENTITY
        };
        Ok(Request::Apply {
            label: "Place a prefab".to_owned(),
            commands: vec![
                Command::Place {
                    entity: placement.clone(),
                    parent: Some(PersistentId::new([editor.key()])),
                    rank: None,
                    prefab: resolved,
                },
                Command::Insert {
                    entity: placement.clone(),
                    component: "Transform".to_owned(),
                    value: transform.to_value(),
                },
            ],
            selection: Some(vec![placement]),
        })
    }

    /// Starts the game that the program plays (`docs/adr/0164-play.md`), on the level as the editor
    /// holds it: the level's scene file as writing it would give, written as a private copy into a
    /// new directory of Play's own, which the game reads in place of its cooked file, whether the
    /// cooked data is older than the file or not, the user's files left alone; and a save of the
    /// editor's own there. Play's thread starts it, and tells the editor when it runs.
    ///
    /// # Errors
    ///
    /// For a program that plays no game, a game that plays already, without a level open, for a
    /// level that cannot be written as a scene file ([`Editor::text`]), and when Play's directory
    /// cannot be made or written, which is removed then.
    fn play(&mut self, world: &mut World) -> Result<Reply, AppError> {
        if self.player.is_some() {
            return Err("a game plays already: it stops first".into());
        }
        let start = self.program.play.ok_or("the program plays no game")?;
        let waker = self.waker.clone().ok_or("the program has not started")?;
        let editor = self.editor.as_ref().ok_or("no level is open")?;
        let text = editor.text(world)?;
        // A directory that no other program made, of a name that it could not have guessed.
        let random = std::hash::BuildHasher::hash_one(
            &std::collections::hash_map::RandomState::new(),
            (std::process::id(), self.plays),
        );
        let directory = std::env::temp_dir().join(format!(
            "slime-editor-play-{}-{}-{random:016x}",
            std::process::id(),
            self.plays
        ));
        self.plays += 1;
        fs::create_dir(&directory)?;
        let written = fs::write(directory.join(format!("{}.scene", editor.asset())), text);
        if let Err(error) = written {
            let _ = fs::remove_dir_all(&directory);
            return Err(error.into());
        }
        let save = directory.join("save.toml");
        let mut command = start(&Played {
            assets: &self.options.assets,
            save: &save,
            headless: self.options.headless,
        });
        command.env(slime_world::PLAY_SCENES_ENV_VAR, &directory);
        self.player = Some(Player::start(command, directory, waker));
        let mut shared = self.shared.borrow_mut();
        shared.play = Playing::Starting;
        shared.played = true;
        shared.console.clear();
        shared.console_revision += 1;
        Ok(Reply::Done)
    }

    /// Asks the game that Play runs to pause or resume, step, run at a speed or stop, as the
    /// operation `name` says, `arguments` the speed's.
    ///
    /// # Errors
    ///
    /// When no game plays, while one starts but to stop it, and for a speed that is not a finite
    /// number above zero.
    fn control(&mut self, name: &str, arguments: &[String]) -> Result<Reply, AppError> {
        let player = self
            .player
            .as_ref()
            .ok_or("no game plays: Play starts one")?;
        let mut shared = self.shared.borrow_mut();
        if name == "play.stop" {
            player.control(Control::Stop);
            return Ok(Reply::Done);
        }
        let Playing::Running { paused, speed } = shared.play else {
            return Err("the game starts: it runs first".into());
        };
        shared.play = match name {
            "play.pause" => {
                player.control(Control::Pause(!paused));
                Playing::Running {
                    paused: !paused,
                    speed,
                }
            }
            "play.step" => {
                player.control(Control::Step);
                Playing::Running {
                    paused: true,
                    speed,
                }
            }
            _ => {
                let speed: f64 = arguments[0]
                    .parse()
                    .ok()
                    .filter(|speed: &f64| speed.is_finite() && *speed > 0.0)
                    .ok_or_else(|| format!("`{}` is not a speed above zero", arguments[0]))?;
                player.control(Control::Speed(speed));
                Playing::Running { paused, speed }
            }
        };
        Ok(Reply::Done)
    }

    /// Follows what Play's threads told: the game running, game time as the game says it stands,
    /// the lines of its log, which the console shows and the editor's log writes, a control that
    /// it refused, and its end, which the user is told of, the editor going on; the editor's log
    /// says each, which agents read. The window is built again for what changes the toolbar, not
    /// for lines of the log, which the console follows by its own count.
    fn follow_play(&mut self) {
        let Some(events) = self.player.as_ref().map(Player::events) else {
            return;
        };
        let mut changed = false;
        for event in events {
            changed |= !matches!(event, Event::Log(_));
            match event {
                Event::Started => {
                    tracing::info!("Play: the game runs");
                    self.shared.borrow_mut().play = Playing::Running {
                        paused: false,
                        speed: 1.0,
                    };
                }
                Event::Time { paused, speed } => {
                    self.shared.borrow_mut().play = Playing::Running { paused, speed };
                }
                Event::Log(lines) => {
                    let mut shared = self.shared.borrow_mut();
                    for line in lines {
                        tracing::info!(target: "game", "{line}");
                        if shared.console.len() == CONSOLE_LINES {
                            shared.console.pop_front();
                        }
                        shared.console.push_back(line);
                    }
                    shared.console_revision += 1;
                }
                Event::Refused(why) => {
                    tracing::warn!("Play: the game refused: {why}");
                    self.tell(true, format!("The game refused: {why}"));
                }
                Event::Ended { how, cleanly } => {
                    if cleanly {
                        tracing::info!("Play: {how}");
                    } else {
                        tracing::warn!("Play: {how}");
                    }
                    self.player = None;
                    self.shared.borrow_mut().play = Playing::Stopped;
                    self.tell(!cleanly, how);
                }
            }
        }
        if changed {
            self.publish();
        }
    }

    /// Puts in the shared value the prefabs that the level just opened can place
    /// ([`prefabs`]).
    fn list_prefabs(&mut self, world: &World) {
        let assets = self.shared.borrow().assets.clone();
        let (Some(assets), Some(editor)) = (assets, self.editor.as_mut()) else {
            return;
        };
        let level = editor.asset();
        let listed = prefabs(&assets, level, editor.scenes(), world.registry());
        self.shared.borrow_mut().prefabs = listed;
    }

    /// Undoes or does again the steps that bring the level to where `steps`, a count of steps done
    /// after the first kept, puts it, each journaled as an undo or a redo: what a row of the
    /// history panel goes to (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).
    ///
    /// # Errors
    ///
    /// Without a level open, for a count that is not a number or that is more than the steps done
    /// and undone, and as an undo or a redo fails: the level then stays at the step reached, the
    /// undos and redos before journaled.
    fn go_to_step(&mut self, world: &mut World, steps: &str) -> Result<Reply, AppError> {
        let editor = self.editor.as_ref().ok_or("no level is open")?;
        let (done, undone) = (editor.done().count(), editor.undone().count());
        let steps: usize = steps
            .parse()
            .map_err(|_| format!("`{steps}` is not a count of steps"))?;
        if steps > done + undone {
            return Err(format!(
                "the history holds {} steps, done and undone: none goes to {steps}",
                done + undone
            )
            .into());
        }
        for _ in steps..done {
            self.answer(world, Request::Undo)?;
        }
        for _ in done..steps {
            self.answer(world, Request::Redo)?;
        }
        Ok(Reply::Done)
    }

    /// The commands that move the outermost of what is selected by `by`, in the world.
    fn nudged(&self, world: &World, by: DVec3) -> Result<Vec<Command>, AppError> {
        Ok(self
            .outermost(world)?
            .into_iter()
            .filter_map(|entity| {
                let id = world.persistent_id(entity)?;
                let mut moved = *world.get::<Transform>(entity)?;
                let at = world.interpolated_transform(entity, 1.0)?;
                let parent = handles::placed_at(world, world.parent(entity));
                moved.translation = handles::into_parent(&parent, at.translation + by);
                Some(set_transform(id, &moved))
            })
            .collect())
    }

    /// The commands that drop each entity selected onto what lies below its origin, or below its
    /// meshes' lowest point if `bottom`, as the CPU's ray meets it, without physics
    /// (`docs/adr/0161-the-open-step-and-the-handles.md`): its own meshes and those below it in the
    /// hierarchy let the ray through. An entity with nothing below it stays.
    ///
    /// # Errors
    ///
    /// Without a level open, or when nothing lies below what is selected.
    fn drops(&mut self, world: &World, bottom: bool) -> Result<Vec<Command>, AppError> {
        let assets = self
            .shared
            .borrow()
            .assets
            .clone()
            .ok_or("no level is open")?;
        let picker = self.picker.get_or_insert_with(|| Picker::new(assets));
        let selection: Vec<Entity> = self
            .editor
            .as_ref()
            .ok_or("no level is open")?
            .selection()
            .iter()
            .filter_map(|identity| world.find(identity))
            .collect();
        let mut commands = Vec::new();
        for entity in handles::outermost(world, &selection) {
            let (Some(at), Some(local), Some(id)) = (
                world.interpolated_transform(entity, 1.0),
                world.get::<Transform>(entity),
                world.persistent_id(entity),
            ) else {
                continue;
            };
            // What the ray starts from: the origin, or the lowest point of what it draws.
            let from = match bottom.then(|| picker.bounds(world, entity, 1.0)).flatten() {
                Some((low, _)) => DVec3::new(at.translation.x, low.y, at.translation.z),
                None => at.translation,
            };
            let own = |hit: &Entity| {
                std::iter::successors(Some(*hit), |&each| world.parent(each))
                    .any(|each| each == entity)
            };
            let Some(hit) = picker
                .cast_through(world, 1.0, (from, DVec3::NEG_Y))
                .into_iter()
                .find(|hit| !own(&hit.entity))
            else {
                continue;
            };
            let parent = handles::placed_at(world, world.parent(entity));
            let mut moved = *local;
            moved.translation =
                handles::into_parent(&parent, at.translation - DVec3::Y * hit.distance);
            commands.push(set_transform(id, &moved));
        }
        if commands.is_empty() {
            return Err("nothing lies below what is selected".into());
        }
        Ok(commands)
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
    fn start(
        &mut self,
        ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        self.waker = Some(ctx.waker());
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
        self.list_prefabs(world);
        if first_run {
            self.tell(false, HINT.to_owned());
        }
        self.publish();
        Ok(())
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        self.follow_play();
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
            selection: None,
        };
        match self.handle(world, request) {
            // The step is made: the program stops, as `broke` says.
            Err(_) if self.broken.is_some() => Ok(()),
            made => made.map(|_| ()),
        }
    }

    /// An agent's requests wait while a person's gesture is under way, and from the press that
    /// starts one, whose request the window sent but the core has not answered yet
    /// (`docs/adr/0161-the-open-step-and-the-handles.md`).
    fn edits_wait(&self) -> bool {
        self.editor.as_ref().is_some_and(Editor::in_gesture)
            || self
                .shared
                .borrow()
                .requests
                .iter()
                .any(|request| matches!(request, Request::Gesture(_)))
    }

    fn edit(&mut self, world: &mut World, edit: &Edit) -> Result<Reply, AppError> {
        let request = request_of(edit)?;
        self.handle(world, request)
    }
}

/// The prefabs of `assets` that a level of the asset ID `level` can place: the scenes and the scene
/// files but the level, each named by its source file, in their order; a file of several scenes,
/// as a glTF file may be, names each by the file and the scene's name, `props.glb: tree`, which
/// `scenes` resolves, or its asset ID where it cannot.
fn prefabs(
    assets: &CookedAssets,
    level: AssetId,
    scenes: &mut SceneLibrary,
    registry: &slime_reflect::Registry,
) -> Vec<(AssetId, String)> {
    let mut prefabs: Vec<(AssetId, String)> = assets
        .manifest()
        .iter()
        .filter(|(id, entry)| {
            *id != level && matches!(entry.kind, AssetKind::Scene | AssetKind::SceneFile)
        })
        .map(|(id, entry)| (id, entry.source.clone()))
        .collect();
    prefabs.sort_by(|a, b| a.1.cmp(&b.1));
    let shared: Vec<bool> = prefabs
        .iter()
        .map(|(_, source)| prefabs.iter().filter(|(_, other)| other == source).count() > 1)
        .collect();
    for ((id, name), shared) in prefabs.iter_mut().zip(shared) {
        if shared {
            let scene = scenes
                .resolve(registry, *id)
                .map_or_else(|_| id.to_string(), |scene| scene.name().to_owned());
            *name = format!("{name}: {scene}");
        }
    }
    prefabs
}

/// The command that sets `entity`'s whole transform to `transform`.
fn set_transform(entity: PersistentId, transform: &Transform) -> Command {
    Command::SetProperty {
        entity,
        component: "Transform".to_owned(),
        path: ValuePath::default(),
        value: transform.to_value(),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use slime_app::{AppConfig, Host};

    use super::*;

    /// The prefabs that a level can place: the scenes and the scene files of the cooked data but the
    /// level, by their sources, in their order; the scenes of one file named by the file and each
    /// scene, or by its asset ID where it cannot be resolved
    /// (`docs/adr/0163-placing-prefabs-and-the-history-panel.md`).
    #[test]
    fn the_prefabs_are_the_scenes_but_the_level_each_named_once() {
        let directory =
            std::env::temp_dir().join(format!("slime-editor-prefabs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a directory");
        let mut manifest = slime_assets::Manifest::new();
        let entry = |kind, source: &str| slime_assets::ManifestEntry {
            kind,
            file: format!("{}.cooked", source.replace(['/', '.'], "_")),
            size: 0,
            needs: Vec::new(),
            source: source.to_owned(),
        };
        let id = AssetId::from_u128;
        manifest.insert(id(1), entry(AssetKind::SceneFile, "yard.scene"));
        manifest.insert(id(2), entry(AssetKind::SceneFile, "lamp.scene"));
        manifest.insert(id(3), entry(AssetKind::Scene, "props.glb"));
        manifest.insert(id(4), entry(AssetKind::Scene, "props.glb"));
        manifest.insert(id(5), entry(AssetKind::Mesh, "props.glb"));
        manifest.insert(id(6), entry(AssetKind::Scene, "a/sample.glb"));
        fs::write(
            directory.join(slime_assets::MANIFEST_FILE),
            manifest.to_bytes(),
        )
        .expect("written");
        let assets = CookedAssets::open(&directory).expect("opened");
        let world = World::new();
        let mut scenes = SceneLibrary::of(assets.clone());
        let listed = prefabs(&assets, id(1), &mut scenes, world.registry());
        let names: Vec<&str> = listed.iter().map(|(_, name)| name.as_str()).collect();
        let (three, four) = (
            format!("props.glb: {}", id(3)),
            format!("props.glb: {}", id(4)),
        );
        assert_eq!(names, ["a/sample.glb", "lamp.scene", &three, &four]);
        let _ = fs::remove_dir_all(&directory);
    }
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
                play: None,
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
