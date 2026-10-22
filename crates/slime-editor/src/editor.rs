//! The editor's core: a level changed through commands, grouped into steps, undone and done again,
//! and journaled (`docs/adr/0134-the-editors-core.md`).

use std::io;
use std::path::{Path, PathBuf};

use slime_app::HeldFile;
use slime_serialize::ReadError;
use slime_world::{
    Command, CommandTextError, DocumentError, Entity, PersistentId, SceneError, SceneLibrary,
    SetAside, World, WorldError,
};

use crate::history::{Held, History, Step};
use crate::journal::{self, Entry, Journal};
use crate::level::{self, Level};

/// The editor's core, without its interface: a level, a scene file opened into a world, changed
/// through [`Command`]s grouped into steps, each done whole or not at all, undone and done again,
/// and journaled; what is selected; and the level written back to its file
/// (`docs/adr/0109-stable-references-and-undo.md`, `docs/adr/0134-the-editors-core.md`).
///
/// The editor holds no world: it changes the one that it is given, which holds the level, and
/// which nothing else changes meanwhile, so that what it undoes is what it did. It holds the
/// library of the scenes that the level places, which reading a command that places one needs.
/// Its history holds commands by the persistent identities of their entities, which hold where
/// handles do not: undoing a deletion creates the entities again, under new handles.
///
/// A step's commands are applied in their order, each as the world is when it comes, so that a
/// command may name what a command before it created; when one is refused, those before it are
/// undone and the step is not made. Undoing a step applies, in the opposite order, the commands
/// that applying its own gave back; doing it again, those that undoing gave back. A command changes
/// only what is under the level's root: one that names an entity of another level, the root
/// itself to delete or move, or no parent, is refused.
pub struct Editor {
    level: Level,
    scenes: SceneLibrary,
    history: History,
    selection: Vec<PersistentId>,
    journal: Option<Journal>,
    /// The selection as the journal last says it, so that one made apart from a step is journaled
    /// before the next entry.
    journaled: Vec<PersistentId>,
    /// How many entries of a journal [`Editor::recover`] played.
    recovered: usize,
}

impl Editor {
    /// Opens the level of the scene file `file` into `world`, its scene spawned under `key`, as the
    /// game that plays it spawns it, so that the identities that the editor gives are the game's.
    /// The file's asset ID is read from its metadata file, `<file>.meta`, as the cooker reads it,
    /// and the prefabs that the level places are resolved through `scenes`, as cooked.
    ///
    /// # Errors
    ///
    /// [`EditError::Io`] if the file or its metadata file cannot be read; [`EditError::Metadata`]
    /// if the metadata file is not one; [`EditError::Scene`] if the scene cannot be resolved, as
    /// [`SceneLibrary::resolve`] says; [`EditError::Spawn`] if it cannot be spawned under `key`,
    /// as [`World::spawn_scene`] says.
    pub fn open(
        world: &mut World,
        mut scenes: SceneLibrary,
        file: &Path,
        key: &str,
    ) -> Result<Self, EditError> {
        let level = Level::open(world, &mut scenes, file, key)?;
        Ok(Self::of(level, scenes))
    }

    /// An editor of `level`, without a step.
    fn of(level: Level, scenes: SceneLibrary) -> Self {
        Self {
            level,
            scenes,
            history: History::default(),
            selection: Vec::new(),
            journal: None,
            journaled: Vec::new(),
            recovered: 0,
        }
    }

    /// The key that the level's scene is spawned under, the first part of every identity that the
    /// editor gives.
    pub fn key(&self) -> &str {
        &self.level.key
    }

    /// The level's scene file.
    pub fn file(&self) -> &Path {
        &self.level.file
    }

    /// The root of the level in `world`, if it is alive.
    pub fn root(&self, world: &World) -> Option<Entity> {
        world.find(&PersistentId::new([self.key()]))
    }

    /// An identity that no entity of `world` has, for an entity that the editor creates in the
    /// level, named after `name`, as [`World::new_id`] gives it: `zone/torch-0rdx`.
    pub fn new_id(&self, world: &World, name: &str) -> PersistentId {
        world.new_id(self.key(), name)
    }

    /// The library of the scenes that the level places, through which a command that places a
    /// prefab finds it ([`SceneLibrary::resolve`]).
    pub fn scenes(&mut self) -> &mut SceneLibrary {
        &mut self.scenes
    }

    /// Keeps a journal of the editor's steps in `file`, a file that this program holds
    /// ([`HeldFile`]), which starts anew: what [`Editor::recover`] plays again, to recover an editor
    /// that crashed, or a session of tests or of an agent. It starts from the level as it is, before
    /// any step, as the editor opened it or last wrote it; each entry is handed to the system as it
    /// is made, so that a crash of the editor loses none.
    ///
    /// A journal is text: entries, each a line that names it, then a document of the engine's
    /// notation, empty for some. The engine's writer writes no comment, so that each line that
    /// starts with `#` starts an entry:
    ///
    /// - `# level`, the first: the level's `key`, its `file`, by its whole path, and the `text`
    ///   of the file;
    /// - `# do`: a step, its `label`, and the `selection` after it, if it chose one; then each of
    ///   its commands, a line `# command` and the command's text ([`Command::write`]);
    /// - `# undo` and `# redo`: the last step done undone, and the last step undone done again;
    /// - `# select`: a `selection` made apart from a step ([`Editor::select`]);
    /// - `# written`: the level written back to its file ([`Editor::write`]).
    ///
    /// # Errors
    ///
    /// [`EditError::Journal`] if the editor made a step since it opened the level, which a journal
    /// that starts from the level could not undo, or if the file cannot be written.
    pub fn keep_journal(&mut self, world: &World, file: HeldFile) -> Result<(), EditError> {
        if !self.history.is_empty() {
            return Err(EditError::Journal(
                "a journal starts from the level before any step, which it may undo".to_owned(),
            ));
        }
        let mut journal = Journal::create(file)?;
        journal.add(&journal::level(world, &self.level)?)?;
        self.journal = Some(journal);
        self.journaled = Vec::new();
        Ok(())
    }

    /// Applies `commands` to `world` as one step named `label`, and selects `selection` after it,
    /// if it is given: the step is done whole, or not at all ([`Editor`]). A new step forgets the
    /// steps undone.
    ///
    /// # Errors
    ///
    /// [`EditError::Empty`] for a step without commands; [`EditError::OutOfLevel`] for a command
    /// that would change what is not under the level's root, [`EditError::Refused`] for one that
    /// the world refuses, or that names an entity that it does not have, and
    /// [`EditError::Restore`] for a restoration, which only undoing applies: the world is as it was
    /// then, and the step is not made; [`EditError::Text`] for a command that cannot be written as
    /// text, which the journal and the history hold, likewise; [`EditError::Io`] and
    /// [`EditError::Journal`] if the journal cannot be written, once the step is made.
    ///
    /// # Panics
    ///
    /// If a command's inverse no longer applies to undo a refused step, which only a world changed
    /// apart from the editor brings.
    pub fn apply(
        &mut self,
        world: &mut World,
        label: &str,
        commands: Vec<Command>,
        selection: Option<Vec<PersistentId>>,
    ) -> Result<(), EditError> {
        let mut commands = commands.into_iter();
        self.make(world, label, selection, |_, _, _| commands.next().map(Ok))
    }

    /// Applies the commands that `texts` write ([`Command::write`]) as [`Editor::apply`] does, each
    /// read as the world is when it comes, so that it may name what a command before it created:
    /// what a tool sends, and what a journal holds.
    ///
    /// # Errors
    ///
    /// As [`Editor::apply`], and [`EditError::Text`] for a text that is not a command, as
    /// [`Command::read`] says.
    ///
    /// # Panics
    ///
    /// As [`Editor::apply`].
    pub fn apply_text(
        &mut self,
        world: &mut World,
        label: &str,
        texts: &[&str],
        selection: Option<Vec<PersistentId>>,
    ) -> Result<(), EditError> {
        self.make(world, label, selection, |world, scenes, index| {
            let text = texts.get(index)?;
            Some(Command::read(text, world, scenes).map_err(|error| text_error(index, error)))
        })
    }

    /// Makes a step of the commands that `next` gives, one at a time, by their indices, until it
    /// gives none.
    fn make(
        &mut self,
        world: &mut World,
        label: &str,
        selection: Option<Vec<PersistentId>>,
        mut next: impl FnMut(&World, &mut SceneLibrary, usize) -> Option<Next>,
    ) -> Result<(), EditError> {
        let mut texts = Vec::new();
        let mut undo: Vec<Held> = Vec::new();
        for index in 0.. {
            let Some(command) = next(world, &mut self.scenes, index) else {
                break;
            };
            let applied = command.and_then(|command| {
                self.check(index, &command)?;
                let text = command
                    .write(world)
                    .map_err(|error| text_error(index, error))?;
                let inverse = world.apply(&command).map_err(|error| EditError::Refused {
                    index,
                    error: Box::new(error),
                })?;
                match held(world, inverse) {
                    Ok(inverse) => Ok((text, inverse)),
                    Err(failed) => {
                        let (inverse, error) = *failed;
                        world
                            .apply(&inverse)
                            .expect("a command's inverse applies right after it");
                        Err(EditError::Text {
                            index,
                            error: Box::new(error),
                        })
                    }
                }
            });
            match applied {
                Ok((text, inverse)) => {
                    texts.push(text);
                    undo.push(inverse);
                }
                Err(error) => {
                    undo.reverse();
                    self.run(world, undo)
                        .expect("a step's commands undo in the opposite order");
                    return Err(error);
                }
            }
        }
        if texts.is_empty() {
            return Err(EditError::Empty);
        }
        undo.reverse();
        let before = self.selection.clone();
        if let Some(selection) = &selection {
            self.selection.clone_from(selection);
        }
        self.history.push_done(
            Step {
                label: label.to_owned(),
                commands: undo,
                before: before.clone(),
                after: self.selection.clone(),
            },
            true,
        );
        self.journal(world, &before, || {
            journal::step(world, label, selection.as_deref(), &texts)
        })
    }

    /// Refuses `command`, the step's `index`, if it would change what is not under the level's
    /// root: an entity of another level, the root itself deleted or moved, or an entity put at the
    /// world's root.
    fn check(&self, index: usize, command: &Command) -> Result<(), EditError> {
        let inside = |id: &PersistentId| id.parts()[0] == self.level.key;
        let below = |id: &PersistentId| inside(id) && id.parts().len() > 1;
        let out = |entity: &PersistentId| {
            Err(EditError::OutOfLevel {
                index,
                entity: entity.clone(),
            })
        };
        match command {
            Command::Create { entity, parent, .. }
            | Command::Place { entity, parent, .. }
            | Command::Reparent { entity, parent, .. } => {
                if !below(entity) || !parent.as_ref().is_some_and(inside) {
                    return out(entity);
                }
            }
            Command::Delete { entity } => {
                if !below(entity) {
                    return out(entity);
                }
            }
            Command::SetProperty { entity, .. }
            | Command::Insert { entity, .. }
            | Command::Remove { entity, .. } => {
                if !inside(entity) {
                    return out(entity);
                }
            }
            Command::Restore(_) => return Err(EditError::Restore { index }),
        }
        Ok(())
    }

    /// Undoes the last step done, and selects what was selected before it; `false` if no step
    /// is done.
    ///
    /// # Errors
    ///
    /// [`EditError::Refused`] or [`EditError::Text`] for a command that the world refuses, or that
    /// cannot be read or written, which only a change of the world made apart from the editor
    /// brings: the world is as it was then, and the step stays done. [`EditError::Io`] and
    /// [`EditError::Journal`] if the journal cannot be written, once the step is undone.
    ///
    /// # Panics
    ///
    /// If a command's inverse no longer applies to put the world back after a refusal, which only
    /// a world changed apart from the editor brings.
    pub fn undo(&mut self, world: &mut World) -> Result<bool, EditError> {
        let Some(step) = self.history.pop_done() else {
            return Ok(false);
        };
        let Step {
            label,
            commands,
            before,
            after,
        } = step;
        match self.run(world, commands) {
            Ok(redo) => {
                let current = std::mem::replace(&mut self.selection, before.clone());
                self.history.push_undone(Step {
                    label,
                    commands: redo,
                    before,
                    after,
                });
                self.journal(world, &current, || Ok("# undo\n".to_owned()))?;
                Ok(true)
            }
            Err(failed) => {
                let (commands, error) = *failed;
                self.history.push_done(
                    Step {
                        label,
                        commands,
                        before,
                        after,
                    },
                    false,
                );
                Err(error)
            }
        }
    }

    /// Does again the last step undone, and selects what was selected after it; `false` if no
    /// step is undone.
    ///
    /// # Errors
    ///
    /// As [`Editor::undo`], the step staying undone.
    ///
    /// # Panics
    ///
    /// As [`Editor::undo`].
    pub fn redo(&mut self, world: &mut World) -> Result<bool, EditError> {
        let Some(step) = self.history.pop_undone() else {
            return Ok(false);
        };
        let Step {
            label,
            commands,
            before,
            after,
        } = step;
        match self.run(world, commands) {
            Ok(undo) => {
                let current = std::mem::replace(&mut self.selection, after.clone());
                self.history.push_done(
                    Step {
                        label,
                        commands: undo,
                        before,
                        after,
                    },
                    false,
                );
                self.journal(world, &current, || Ok("# redo\n".to_owned()))?;
                Ok(true)
            }
            Err(failed) => {
                let (commands, error) = *failed;
                self.history.push_undone(Step {
                    label,
                    commands,
                    before,
                    after,
                });
                Err(error)
            }
        }
    }

    /// Applies `commands` in their order, and returns the commands that undo them, in the order in
    /// which they apply; when one is refused, undoes those applied, and returns `commands` and why.
    fn run(
        &mut self,
        world: &mut World,
        commands: Vec<Held>,
    ) -> Result<Vec<Held>, Box<(Vec<Held>, EditError)>> {
        let mut undo: Vec<Held> = Vec::with_capacity(commands.len());
        let mut failure = None;
        for (index, held_command) in commands.iter().enumerate() {
            let command = match self.command(world, held_command) {
                Ok(command) => command,
                Err(error) => {
                    failure = Some(text_error(index, error));
                    break;
                }
            };
            let inverse = match world.apply(&command) {
                Ok(inverse) => inverse,
                Err(error) => {
                    failure = Some(EditError::Refused {
                        index,
                        error: Box::new(error),
                    });
                    break;
                }
            };
            // Written as the world is right after its command: the identities that it names alive.
            match held(world, inverse) {
                Ok(inverse) => undo.push(inverse),
                Err(failed) => {
                    let (inverse, error) = *failed;
                    world
                        .apply(&inverse)
                        .expect("a command's inverse applies right after it");
                    failure = Some(EditError::Text {
                        index,
                        error: Box::new(error),
                    });
                    break;
                }
            }
        }
        let Some(error) = failure else {
            undo.reverse();
            return Ok(undo);
        };
        for inverse in undo.iter().rev() {
            let inverse = self
                .command(world, inverse)
                .expect("an inverse written as the world was reads back");
            world
                .apply(&inverse)
                .expect("a command's inverse applies in the opposite order");
        }
        Err(Box::new((commands, error)))
    }

    /// The command that `held` holds, read as `world` is now.
    fn command(&mut self, world: &World, held: &Held) -> Result<Command, CommandTextError> {
        match held {
            Held::Text(text) => Command::read(text, world, &mut self.scenes),
            Held::Restore(deleted) => Ok(Command::Restore(deleted.clone())),
        }
    }

    /// Adds the entries that `entries` makes to the journal, if the editor keeps one, after the
    /// selection `before` the entries, if the journal does not say it yet.
    fn journal(
        &mut self,
        world: &World,
        before: &[PersistentId],
        entries: impl FnOnce() -> Result<String, EditError>,
    ) -> Result<(), EditError> {
        let Some(kept) = &mut self.journal else {
            return Ok(());
        };
        let mut text = String::new();
        if before != self.journaled {
            text = journal::select(world, before)?;
        }
        text.push_str(&entries()?);
        kept.add(&text)?;
        self.journaled.clone_from(&self.selection);
        Ok(())
    }

    /// Selects `selection`, the entities by their persistent identities: not a step, but what a
    /// step undone or done again selects again ([`Editor::undo`], [`Editor::redo`]). The journal
    /// says it before its next entry.
    pub fn select(&mut self, selection: Vec<PersistentId>) {
        self.selection = selection;
    }

    /// What is selected, by persistent identity, in the order given.
    pub fn selection(&self) -> &[PersistentId] {
        &self.selection
    }

    /// How many entries of a journal the editor played again as it was recovered
    /// ([`Editor::recover`]); 0 for an editor opened from its level's file.
    pub fn recovered(&self) -> usize {
        self.recovered
    }

    /// The labels of the steps done, the oldest first: what [`Editor::undo`] undoes, from the last.
    pub fn done(&self) -> impl ExactSizeIterator<Item = &str> {
        self.history.done()
    }

    /// The labels of the steps undone, the next that [`Editor::redo`] does again first.
    pub fn undone(&self) -> impl ExactSizeIterator<Item = &str> {
        self.history.undone()
    }

    /// Sets the bytes of memory that the history takes at most,
    /// [`HISTORY_BUDGET`](crate::HISTORY_BUDGET) unless set, and forgets the steps that go past it,
    /// as `HISTORY_BUDGET` says.
    pub fn set_history_budget(&mut self, bytes: usize) {
        self.history.set_budget(bytes);
    }

    /// Writes the level back to its scene file from `world`, as [`World::write_scene`] writes it,
    /// with the comments of the file as the editor opened or last wrote it, which go back before
    /// what they preceded ([`slime_serialize::keep_comments`]); whole or not at all, through a file
    /// beside it that is renamed ([`slime_cook::write`]). Returns what the file could not hold as
    /// the world holds it ([`Written::left_out`](slime_world::Written::left_out)). The history
    /// stays: a step undone after the level is written changes the level since.
    ///
    /// # Errors
    ///
    /// [`EditError::NoLevel`] if the level's root is not alive in `world`; [`EditError::Write`] if
    /// the scene cannot be written; [`EditError::Changed`] if the file is not the text that the
    /// editor read or last wrote, which another program changed since: the editor does not write
    /// over it; [`EditError::Io`] if the file cannot be read or written; [`EditError::Journal`] if
    /// the journal cannot be written, once the level is.
    pub fn write(&mut self, world: &mut World) -> Result<Vec<SetAside>, EditError> {
        let left_out = self.level.write(world)?;
        let selection = self.selection.clone();
        self.journal(world, &selection, || Ok("# written\n".to_owned()))?;
        Ok(left_out)
    }

    /// An editor of the level of the scene file `file`, spawned under `key`, recovered from
    /// `journal`, a journal that an editor kept of that file ([`Editor::keep_journal`]): the level
    /// opened into `world` from the text that the journal's first entry holds, then each step made,
    /// undone and done again, each selection and each writing of the level played again in their
    /// order, without writing the file. What an editor that crashed had done since it started its
    /// journal comes back so, with its history, and a session of tests or of an agent is played
    /// again. With `keep`, a file that this program holds, the editor goes on with the journal
    /// there once every entry is played: `journal` is written there whole first, unless the file
    /// holds it already, as the journal's own file does.
    ///
    /// # Errors
    ///
    /// [`EditError::Journal`] if `journal` is not a journal, does not start with the level's entry,
    /// or is the journal of another key or of another file; [`EditError::Changed`] if the level's
    /// file is not the text that the journal last wrote, or opened if it wrote none: the file
    /// changed apart from the editor; [`EditError::Replay`] for an entry that cannot be played, a
    /// step that the world refuses, an undo or a redo of a step that the history does not hold; and
    /// the errors of [`Editor::open`]. The world holds what was played then, and a program that
    /// recovers starts again from a new world; `keep` is not written to then.
    ///
    /// # Panics
    ///
    /// As [`Editor::apply`].
    pub fn recover(
        world: &mut World,
        mut scenes: SceneLibrary,
        file: &Path,
        key: &str,
        journal: &str,
        keep: Option<HeldFile>,
    ) -> Result<Self, EditError> {
        let mut entries = journal::entries(world, journal)?.into_iter();
        let Some((
            _,
            Entry::Level {
                key: kept,
                file: kept_file,
                text,
            },
        )) = entries.next()
        else {
            return Err(EditError::Journal(
                "it does not start with the level's entry, `# level`".to_owned(),
            ));
        };
        if kept != key {
            return Err(EditError::Journal(format!(
                "it is the journal of the level `{kept}`, not `{key}`"
            )));
        }
        let whole = level::whole(file);
        if kept_file != whole.display().to_string() {
            return Err(EditError::Journal(format!(
                "it is the journal of {kept_file}, not of {}",
                whole.display()
            )));
        }
        let on_disk = level::read(file)?;
        let level = Level::open_as(world, &mut scenes, file, key, text)?;
        let mut editor = Self::of(level, scenes);
        for (line, entry) in entries {
            editor
                .play(world, entry)
                .map_err(|error| EditError::Replay {
                    line,
                    error: Box::new(error),
                })?;
            editor.recovered += 1;
        }
        if editor.level.text != on_disk {
            return Err(EditError::Changed(file.to_owned()));
        }
        if let Some(keep) = keep {
            editor.journal = Some(Journal::resume(keep, journal)?);
            editor.journaled.clone_from(&editor.selection);
        }
        Ok(editor)
    }

    /// Plays `entry` of a journal again, as [`Editor::recover`] says.
    fn play(&mut self, world: &mut World, entry: Entry<'_>) -> Result<(), EditError> {
        let held = |done: bool, what: &str| {
            done.then_some(()).ok_or_else(|| {
                EditError::Journal(format!("it {what} a step that the history does not hold"))
            })
        };
        match entry {
            Entry::Level { .. } => Err(EditError::Journal(
                "a journal holds one entry of the level, its first".to_owned(),
            )),
            Entry::Do {
                label,
                selection,
                commands,
            } => self.apply_text(world, &label, &commands, selection),
            Entry::Undo => held(self.undo(world)?, "undoes"),
            Entry::Redo => held(self.redo(world)?, "does again"),
            Entry::Select(selection) => {
                self.select(selection);
                Ok(())
            }
            Entry::Written => {
                (self.level.text, _) = self.level.written(world)?;
                let selection = self.selection.clone();
                self.journal(world, &selection, || Ok("# written\n".to_owned()))
            }
        }
    }
}

/// The next command of a step, or why it cannot be had.
type Next = Result<Command, EditError>;

/// `inverse` as the history holds it: as text, written as the world is now, or, for a restoration,
/// as it is. The inverse is given back with the error when it cannot be written.
fn held(world: &World, inverse: Command) -> Result<Held, Box<(Command, CommandTextError)>> {
    match inverse {
        Command::Restore(deleted) => Ok(Held::Restore(deleted)),
        inverse => match inverse.write(world) {
            Ok(text) => Ok(Held::Text(text)),
            Err(error) => Err(Box::new((inverse, error))),
        },
    }
}

/// The error of the command `index` of a step whose text cannot be read or written: a refusal, for
/// an entity or a component that the world does not have, as applying the command would say.
fn text_error(index: usize, error: CommandTextError) -> EditError {
    match error {
        CommandTextError::World(error) => EditError::Refused {
            index,
            error: Box::new(error),
        },
        error => EditError::Text {
            index,
            error: Box::new(error),
        },
    }
}

/// Why the editor cannot open or write its level, make, undo or do again a step, or keep or play
/// its journal.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EditError {
    /// A file cannot be read or written.
    #[error("{}: {error}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// Why.
        #[source]
        error: io::Error,
    },
    /// The level's metadata file is not one.
    #[error("{}: {error}", path.display())]
    Metadata {
        /// The metadata file.
        path: PathBuf,
        /// What is wrong in it.
        #[source]
        error: ReadError,
    },
    /// The level's scene cannot be resolved.
    #[error("the level cannot be resolved: {0}")]
    Scene(#[source] SceneError),
    /// The level's scene cannot be spawned.
    #[error("the level cannot be spawned: {0}")]
    Spawn(#[source] Box<WorldError>),
    /// The level cannot be written as a scene file.
    #[error("the level cannot be written: {0}")]
    Write(#[source] DocumentError),
    /// The level's file is not the text that the editor read or last wrote: another program
    /// changed it since, and the editor does not write over it.
    #[error(
        "{} changed apart from the editor since it read or wrote it: it is not written over",
        .0.display()
    )]
    Changed(PathBuf),
    /// The level's root is not alive in the world.
    #[error("the level's root `{0}` is not alive")]
    NoLevel(PersistentId),
    /// A step without commands.
    #[error("a step holds one command at least")]
    Empty,
    /// A command of a step would change what is not under the level's root: an entity of another
    /// level, the root itself deleted or moved, or an entity put at the world's root.
    #[error(
        "command {index}: `{entity}` is not under the level's root, or would leave it: the editor \
         changes what is under it"
    )]
    OutOfLevel {
        /// The command's index in the step, from 0.
        index: usize,
        /// The entity that the command names.
        entity: PersistentId,
    },
    /// A command of a step cannot be read from its text, or written as text.
    #[error("command {index}: {error}")]
    Text {
        /// The command's index in the step, from 0.
        index: usize,
        /// Why.
        #[source]
        error: Box<CommandTextError>,
    },
    /// The world refuses a command of a step, or does not have an entity or a component that it
    /// names.
    #[error("command {index}: {error}")]
    Refused {
        /// The command's index in the step, from 0.
        index: usize,
        /// Why.
        #[source]
        error: Box<WorldError>,
    },
    /// A step holds a restoration, which only undoing a deletion applies.
    #[error("command {index}: a restoration is applied by undoing the deletion that made it")]
    Restore {
        /// The command's index in the step, from 0.
        index: usize,
    },
    /// The journal cannot be written, or read as a journal of the level.
    #[error("the journal: {0}")]
    Journal(String),
    /// An entry of a journal cannot be played again ([`Editor::recover`]).
    #[error("the journal's entry at line {line}: {error}")]
    Replay {
        /// The line that names the entry, from 1.
        line: usize,
        /// Why.
        #[source]
        error: Box<EditError>,
    },
}
