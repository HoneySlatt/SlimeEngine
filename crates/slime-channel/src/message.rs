//! The messages of the protocol, and their encoding.

use serde::{Deserialize, Serialize};
use uwui_agent::{Fault, Value};

use crate::error::ChannelError;

/// What a tool asks a game once its hello is welcomed, each answered by a [`Reply`]: requests of
/// the families of play, the world, editing and the interface ([`FAMILIES`](crate::FAMILIES)), each
/// named on the wire by its method, `play/pause`. Its `serde` form writes and reads every request
/// but [`Request::Edit`], which crosses as the [`Edit`] that it holds, `editing/undo`: serializing
/// it alone fails, and reading never gives it.
///
/// Entities are named by their persistent identities, `zone/door`; components by their types'
/// names, `Transform`; parts of components by property paths, `translation.x`; values, given and
/// answered, as text in the engine's notation, `12.5`, `{ x = 1.0, y = 0.0, z = 2.0 }`
/// (`slime_serialize::format`), which the game reads with the types it knows. Keys are named by
/// their position on the keyboard, as `slime_platform::KeyCode` names them: `KeyW`, `Escape`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Request {
    /// The game's state: [`Reply::State`].
    #[serde(rename = "play/state")]
    State,
    /// Pauses game time.
    #[serde(rename = "play/pause")]
    Pause,
    /// Lets game time run again, which drops the steps asked for and not run yet.
    #[serde(rename = "play/resume")]
    Resume,
    /// Runs that many fixed steps of game time, while it is paused, over as many frames as they take:
    /// answered once they have all run.
    #[serde(rename = "play/step")]
    Step {
        /// How many.
        steps: u32,
    },
    /// Sets the speed of game time, a finite number above zero.
    #[serde(rename = "play/speed")]
    Speed {
        /// The speed.
        speed: f64,
    },
    /// Asks the game to stop, as its window closed would.
    #[serde(rename = "play/stop")]
    Stop,
    /// Minimizes the game's window, as the user would, and answers at once: the compositor hides
    /// it. Only the user shows it again: on Wayland, a program cannot. It fails in a run without a
    /// window.
    #[serde(rename = "play/minimize")]
    Minimize,
    /// The entities whose name or persistent identity holds `filter`, whatever its case:
    /// [`Reply::Entities`], at most [`MAX_ENTITIES`](crate::MAX_ENTITIES) of them.
    #[serde(rename = "world/entities")]
    Entities {
        /// What to look for; empty for every entity.
        filter: String,
    },
    /// An entity and its components: [`Reply::Entity`].
    #[serde(rename = "world/entity")]
    Entity {
        /// Its persistent identity.
        entity: String,
    },
    /// Sets a part of a component, as the world's `Command::SetProperty` does.
    #[serde(rename = "world/set_property")]
    SetProperty {
        /// The entity, by its persistent identity.
        entity: String,
        /// The component's type, by its name.
        component: String,
        /// The part, as a property path; empty for the whole component.
        path: String,
        /// The new value, as text in the engine's notation.
        value: String,
    },
    /// Puts a key down, as the player would, from the next frame on.
    #[serde(rename = "play/press")]
    Press {
        /// The key, by its position.
        key: String,
    },
    /// Lets a key go.
    #[serde(rename = "play/release")]
    Release {
        /// The key, by its position.
        key: String,
    },
    /// The image that the game shows, or the part of it that one of its views draws:
    /// [`Reply::Image`], once the next frame is drawn. It fails in a run without a window, where
    /// the window's system does not let its images be copied, and for a view that the frame does
    /// not draw.
    #[serde(rename = "play/capture")]
    Capture {
        /// The view, by the name that [`GameState::views`] gives; empty for the whole image.
        view: String,
    },
    /// Sets a part of the settings of one of the game's views, as [`Request::SetProperty`] sets a
    /// part of a component, from the game's next frame: its render scale, anti-aliasing, mode of
    /// debugging or passes, which [`ViewSummary::settings`] gives; a new render scale once the
    /// view's size has held still, as its renderer makes its targets. It fails for a view that the
    /// last frame did not draw, or whose settings the game does not give, and for a value that its
    /// type, or the range of its field, refuses.
    #[serde(rename = "play/set_view_settings")]
    SetViewSettings {
        /// The view, by the name that [`GameState::views`] gives.
        view: String,
        /// The part, as a property path, `debug`; empty for the whole settings.
        path: String,
        /// The new value, as text in the engine's notation, `"Normals"`.
        value: String,
    },
    /// The entity drawn at a pixel of the game's image, or of one of its views: [`Reply::Picked`],
    /// once the game has picked it, a frame or more later (`docs/adr/0130-picking.md`). It fails in
    /// a run without a window, for a view that the frame does not draw, for a pixel outside the
    /// image or the view, and in a game that does not pick, or that gives no answer, as one whose
    /// GPU device was lost.
    #[serde(rename = "play/pick")]
    Pick {
        /// The view, by the name that [`GameState::views`] gives; empty for the whole image, where
        /// the pixel is the last view's that the frame drew there.
        view: String,
        /// The pixel's column, from the left of the image or the view.
        x: u32,
        /// Its row, from the top.
        y: u32,
    },
    /// One of the game's reports, by the name that [`GameState::reports`] gives: [`Reply::Report`].
    /// It fails for a name that the game gives no report under.
    #[serde(rename = "play/report")]
    Report {
        /// The report's name, `assets`.
        name: String,
    },
    /// The lines of the game's log after the line numbered `after`: [`Reply::Logs`], at most
    /// [`MAX_LOG_LINES`](crate::MAX_LOG_LINES) of them.
    #[serde(rename = "play/logs")]
    Logs {
        /// The number of the last line already read; 0 for the oldest that the game keeps.
        after: u64,
    },
    /// Saves the run to a file, as the game saves its own runs. It fails for a game that does not
    /// save its runs.
    #[serde(rename = "play/save")]
    Save {
        /// The file, a path on the game's machine.
        file: String,
    },
    /// Loads the run of the save in a file in place of the run, as the game resumes its own saves.
    /// It fails for a game that does not save its runs, and for a save that it does not resume,
    /// which leaves the run as it is.
    #[serde(rename = "play/load")]
    Load {
        /// The file, a path on the game's machine.
        file: String,
    },
    /// Applies `commands`, each the text of a command of the world (`slime_world::Command::write`),
    /// read as the game's world is when it comes and applied through the game's rules, in their
    /// order: what an editor sends the game that it plays (`docs/adr/0165-editing-while-playing.md`).
    /// It fails at the first command that the game refuses, saying which and why, those before it
    /// applied and those after it not. An editor makes each command a step of its own.
    #[serde(rename = "world/apply")]
    Apply {
        /// The commands, as text.
        commands: Vec<String>,
    },
    /// Asks an editor, a program that edits a level rather than plays it
    /// (`docs/adr/0135-the-editors-program.md`). It fails in a game, which is not an editor.
    #[serde(skip)]
    Edit {
        /// What it asks.
        edit: Edit,
    },
    /// The elements of the game's interface that mean something, as its last frame left them
    /// (`docs/adr/0143-what-agents-see-of-the-interface.md`), in the order of its tree:
    /// [`Reply::Elements`], at most [`MAX_ELEMENTS`](crate::MAX_ELEMENTS) of them. It fails in a
    /// game that has no interface, as one that runs without a window.
    #[serde(rename = "interface/elements")]
    Elements {
        /// The role of those wanted, `Button`, whatever its case; empty for any.
        role: String,
        /// What their names hold, whatever its case; empty for any.
        name: String,
    },
    /// The element of the game's interface that the pointer would reach at a pixel of the game's
    /// image, or the nearest above it that means something: [`Reply::Elements`], one or none. It
    /// fails in a game that has no interface, as one that runs without a window.
    #[serde(rename = "interface/element_at")]
    ElementAt {
        /// The pixel's column, from the left of the image.
        x: u32,
        /// Its row, from the top.
        y: u32,
    },
    /// Does `action` in the game's interface, as a person's input would: the interface takes it at
    /// once, and the game reads what it asks as the frame that answers goes on. It fails in a game
    /// that has no interface, as one that runs without a window, for a path that names no element,
    /// for an action that the element does not take now, as a disabled button's press, and for an
    /// offset that is not finite.
    #[serde(rename = "interface/act")]
    Act {
        /// The element, by its path, `main/bar/open`; empty for an action of no element, a
        /// navigation intent or a command.
        element: String,
        /// What to do.
        action: InterfaceAction,
    },
    /// The part of the game's image where an element of its interface shows: [`Reply::Image`],
    /// once the next frame is drawn. It fails in a run without a window, in a game that has no
    /// interface, and for an element that does not show.
    #[serde(rename = "interface/capture_element")]
    CaptureElement {
        /// The element, by its path.
        element: String,
    },
}

/// What a tool does in a game's interface ([`Request::Act`]), as a person's input would.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InterfaceAction {
    /// Presses a button, as a click does.
    Press,
    /// Gives an element the focus.
    Focus,
    /// Scrolls the content of a box that scrolls or of a list to an offset, in logical pixels from
    /// its start and top.
    Scroll {
        /// The offset from the start.
        x: f32,
        /// The offset from the top.
        y: f32,
    },
    /// Gives a field, a slider, two panels, a list to choose from or a color well a value, as a
    /// person who replaced what it holds and committed would (`docs/adr/0146-screen-readers.md`,
    /// `docs/adr/0148-widgets.md`, `docs/adr/0151-rich-widgets.md`,
    /// `docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`): the game reads the change and
    /// the commit of a field's, a slider's, a list's or a color well's.
    SetValue {
        /// The text; for a field of numbers, an expression of a finite number, in a unit of its
        /// family where it has one, `sqrt(2)/2`, `10cm`; for a slider, a finite number, as Rust
        /// reads one; for a list to choose from, one of its options or the place of one; for a
        /// color well, a hexadecimal code, `#ff8000` or `#ff800080`.
        value: String,
    },
    /// Drops a draggable element on a target of drops, as a person who dragged it there would
    /// (`docs/adr/0168-rearranging-the-hierarchy.md`).
    Drop {
        /// The target, by its path.
        onto: String,
        /// Where in the target, from its top, 0, to its bottom, 1.
        at: f32,
    },
    /// Moves the focus to the next element, as Tab does.
    Next,
    /// Moves the focus to the previous element, as Shift+Tab does.
    Previous,
    /// Moves the focus up, as the up arrow does.
    Up,
    /// Moves the focus down.
    Down,
    /// Moves the focus left.
    Left,
    /// Moves the focus right.
    Right,
    /// Accepts, as Enter does: presses the button that holds the focus.
    Accept,
    /// Cancels, as Escape does.
    Cancel,
    /// Gives the game a command, as a shortcut does.
    Command {
        /// The command's name, `undo`.
        name: String,
    },
}

/// What a tool asks an editor ([`Request::Edit`]). Entities are named by their persistent
/// identities; commands are given as text, each a document of the engine's notation that
/// `slime_world::Command::write` writes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Edit {
    /// Makes a step of `commands`, whole or not at all, named `label`, and selects `selection`
    /// after it, if it is given.
    #[serde(rename = "editing/step")]
    Step {
        /// The step's name, which the history shows.
        label: String,
        /// The commands, as text, applied in their order.
        commands: Vec<String>,
        /// What to select after the step; none to keep what is selected.
        selection: Option<Vec<String>>,
    },
    /// Undoes the last step done. It fails when no step is done.
    #[serde(rename = "editing/undo")]
    Undo,
    /// Does again the last step undone. It fails when no step is undone.
    #[serde(rename = "editing/redo")]
    Redo,
    /// The editor's level, history and selection: [`Reply::History`].
    #[serde(rename = "editing/history")]
    History,
    /// The text of the editor's journal: [`Reply::Journal`].
    #[serde(rename = "editing/journal")]
    Journal,
    /// Selects entities, which is not a step.
    #[serde(rename = "editing/select")]
    Select {
        /// The entities selected.
        selection: Vec<String>,
    },
    /// Writes the level back to its file: [`Reply::Written`].
    #[serde(rename = "editing/write")]
    Write,
    /// The editor's operations, every action that its menus, its shortcuts, its palette and agents
    /// run by name: [`Reply::Operations`].
    #[serde(rename = "editing/operations")]
    Operations,
    /// Where the game that the editor plays listens, and its token, which a tool connects to it
    /// with: [`Reply::Played`]. It fails while no game plays, or one starts.
    #[serde(rename = "editing/played")]
    Played,
    /// Runs the operation `name` with `arguments`, each given as text, in the order of the
    /// operation's parameters: the operation's reply, [`Reply::Done`] or what it gives, as
    /// [`Reply::Written`] for writing the level.
    #[serde(rename = "editing/run")]
    Run {
        /// The operation's name: `edit.undo`.
        name: String,
        /// Its arguments, as text: a path for `file.open`.
        arguments: Vec<String>,
    },
}

/// What a game answers to a [`Request`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reply", rename_all = "snake_case")]
pub enum Reply {
    /// The request is done.
    Done,
    /// The request failed: why. The connection stays open. On the wire, an error of the core's,
    /// [`Fault::REFUSED`] when the game refused it; a tool reads every error of the core's so.
    Failed {
        /// Why.
        message: String,
    },
    /// The game's state.
    State {
        /// The state.
        state: GameState,
    },
    /// The entities found.
    Entities {
        /// The first of them, in the world's order.
        entities: Vec<EntitySummary>,
        /// How many more were found and left out.
        more: u64,
    },
    /// An entity and its components.
    Entity {
        /// The entity.
        entity: EntitySummary,
        /// The persistent identity of its parent; empty for none.
        parent: String,
        /// Its components, by their types' names, in the world's order.
        components: Vec<ComponentText>,
    },
    /// The image that the game shows, or the part of it that a view draws, whose pixels follow the
    /// message: `width` × `height` pixels, rows from top to bottom, four bytes each, red, green, blue
    /// and alpha, sRGB-encoded.
    Image {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// What a picking found: the entity picked at the pixel, by its persistent identity and its
    /// name, both empty where none is, and its identity alone empty for an entity that has none.
    Picked {
        /// The entity, or nothing.
        entity: EntitySummary,
        /// Whether an entity is picked there: none where nothing is drawn, nor where the game
        /// picks nothing of what it draws, as the editor does what its user locked.
        found: bool,
    },
    /// The elements of a game's interface found.
    Elements {
        /// The first of them, in the order of the interface's tree.
        elements: Vec<ElementSummary>,
        /// How many more were found and left out.
        more: u64,
    },
    /// A report of the game's.
    Report {
        /// The report, as a document in the engine's notation (`slime_serialize::write`).
        text: String,
    },
    /// Lines of the game's log.
    Logs {
        /// The lines, oldest first.
        lines: Vec<LogLine>,
    },
    /// An editor's level, history and selection.
    History {
        /// The history.
        history: EditorHistory,
    },
    /// The text of an editor's journal.
    Journal {
        /// The journal, as the editor wrote it.
        text: String,
    },
    /// An editor's level written back to its file: what the file could not hold as the editor's
    /// world holds it, each as a sentence; none when it holds all of it.
    Written {
        /// What was left out.
        left_out: Vec<String>,
    },
    /// Where the game that an editor plays listens, and its token.
    Played {
        /// Its address, as `127.0.0.1:41899`.
        address: String,
        /// Its token, as text.
        token: String,
    },
    /// An editor's operations, in the order of its registry.
    Operations {
        /// The operations.
        operations: Vec<OperationSummary>,
    },
}

/// An operation of an editor, as [`Reply::Operations`] lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OperationSummary {
    /// The name that runs it: `file.save`.
    pub name: String,
    /// What a person reads in a menu or the palette: `Save`.
    pub label: String,
    /// The names of its parameters, in the order of [`Edit::Run`]'s arguments: `path`.
    pub parameters: Vec<String>,
    /// What it changes: `history`, a step undone or done again; `file`, the level's file or which
    /// level is open; `selection`, what is selected; `level`, the level, by a step that enters the
    /// history, as dropping what is selected does; `clipboard`, what the editor copied; `window`,
    /// what the editor's window shows; `program`, whether it runs. Those of `level` alone make
    /// steps, besides the commands of [`Edit::Step`].
    pub changes: String,
}

/// An editor's level, history and selection, as [`Reply::History`] gives them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditorHistory {
    /// The level's file, a path on the editor's machine.
    pub level: String,
    /// The labels of the steps done, the oldest first.
    pub done: Vec<String>,
    /// The labels of the steps undone, the next to be done again first.
    pub undone: Vec<String>,
    /// What is selected, by persistent identity.
    pub selection: Vec<String>,
    /// How many entries of a journal that a crash, or a stop without writing, left the editor
    /// played again as it started: what it recovered.
    pub recovered: u64,
    /// The editor's journal, a path on its machine.
    pub journal: String,
    /// How many steps the level's file does not hold, as the editor counts them
    /// (`slime_editor::Editor::unwritten`).
    pub unwritten: u64,
}

/// A game's state, as a frame's start finds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    /// The frames run so far.
    pub frame: u64,
    /// Whether game time is paused.
    pub paused: bool,
    /// Whether the game holds game time, apart from the pause, as while its zone loads: it stands
    /// still then, but for the steps asked for while paused.
    pub held: bool,
    /// The speed of game time.
    pub speed: f64,
    /// The fixed steps of game time run so far.
    pub steps: u64,
    /// The game time that they covered, in seconds.
    pub game_seconds: f64,
    /// The real time that the frames covered, in seconds.
    pub real_seconds: f64,
    /// The average time between the starts of the last frames, up to 120 of them, in milliseconds:
    /// in a headless run, the time that its frames simulate, a fixed step each.
    pub frame_ms: f64,
    /// The longest of those times, in milliseconds.
    pub longest_frame_ms: f64,
    /// The entities of the world.
    pub entities: u64,
    /// The views that the last frame drew into the window's image, which a capture and a change
    /// of settings name ([`Request::Capture`], [`Request::SetViewSettings`]); none for a game that
    /// names none, or without a window.
    pub views: Vec<ViewSummary>,
    /// The names of the reports that the game gives, which [`Request::Report`] reads: `assets`, its
    /// asset cache's, `views`, its views' statistics; none for a game that gives none.
    pub reports: Vec<String>,
}

/// A view that a frame drew, as a game's state lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewSummary {
    /// Its name, `map`.
    pub name: String,
    /// Its settings, as a document in the engine's notation (`slime_serialize::write`), the
    /// renderer's `ViewSettings` for a view of `slime-render`; empty for a view whose settings the
    /// game does not give.
    pub settings: String,
}

/// An entity, as a list of entities names it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntitySummary {
    /// Its persistent identity; empty for an entity that has none.
    pub identity: String,
    /// Its name; empty for none.
    pub name: String,
}

/// An element of a game's interface, as a list of elements names it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElementSummary {
    /// Its path, which names it from one frame to the next: `main/bar/open`.
    pub path: String,
    /// Its role, as AccessKit names it: `Button`.
    pub role: String,
    /// Its name; empty for none.
    pub name: String,
    /// What a field holds, as the user left it, a slider's number, two panels' first share or a
    /// list to choose from's chosen option; none for another element.
    pub value: Option<String>,
    /// Its description, its tooltip; none for none.
    pub description: Option<String>,
    /// The text that a field shows while it holds nothing; none for none.
    pub placeholder: Option<String>,
    /// Its states: `hovered`, `pressed`, `focused`, `disabled`, `checked`, `expanded`,
    /// `selected`, and `has_popup` for what a press opens a layer beside, an item of a menu that
    /// opens a submenu or a list to choose from.
    pub states: Vec<String>,
    /// What can be done to it now: `press`, `focus`, `scroll`, `set_value`.
    pub actions: Vec<String>,
    /// The part of it that shows, in pixels of the game's image: its left column, its top row, its
    /// width and its height; none where it does not show.
    pub bounds: Option<[u32; 4]>,
}

/// A component of an entity, as text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentText {
    /// Its type's name.
    pub name: String,
    /// Its value, as a document in the engine's notation (`slime_serialize::write`).
    pub value: String,
}

/// A line of a game's log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    /// Its number, from 1 at the game's start, one more for each line.
    pub number: u64,
    /// The line, as the log writes it, without its time.
    pub text: String,
}

/// What a game tells a connection that subscribed to it, each event a state: its current value
/// once the subscription is answered, then each change, a newer value replacing one not yet written
/// (`docs/adr/0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md`).
/// Each is named on the wire by its method, `play/time`, which its family declares
/// ([`FAMILIES`](crate::FAMILIES)).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Event {
    /// How game time stands.
    #[serde(rename = "play/time")]
    Time(GameTime),
}

/// How game time stands, as [`Event::Time`] tells it: three values that change seldom.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameTime {
    /// Whether game time is paused.
    pub paused: bool,
    /// The speed of game time, a finite number above zero.
    pub speed: f64,
    /// Whether the game holds game time, apart from the pause, as while its zone loads
    /// ([`GameState::held`]): the first time that tells it false gives the player control.
    pub held: bool,
}

impl Event {
    /// The name of [`Event::Time`], which a tool subscribes to.
    pub const TIME: &'static str = "play/time";

    /// The method and the parameters that carry the event.
    pub(crate) fn call(&self) -> Result<(String, Value), ChannelError> {
        Ok(uwui_agent::call(self)?)
    }

    /// The event that `method` and `params` make, or why none.
    pub(crate) fn read(method: &str, params: Value) -> Result<Self, Fault> {
        uwui_agent::read_call(method, params)
    }
}

impl Request {
    /// The method and the parameters that carry the request.
    pub(crate) fn call(&self) -> Result<(String, Value), ChannelError> {
        Ok(match self {
            Self::Edit { edit } => uwui_agent::call(edit)?,
            request => uwui_agent::call(request)?,
        })
    }

    /// The request that `method` of `family`, `play`, and `params` make, or why none.
    pub(crate) fn read(family: &str, method: &str, params: Value) -> Result<Self, Fault> {
        if family == EDITING {
            uwui_agent::read_call(method, params).map(|edit| Self::Edit { edit })
        } else {
            uwui_agent::read_call(method, params)
        }
    }
}

/// The family of an editor's requests, which [`Request::Edit`] carries.
const EDITING: &str = "editing";

impl Reply {
    /// The outcome that carries the reply: [`Reply::Failed`] as a [`Fault::REFUSED`].
    pub(crate) fn outcome(&self) -> Result<Result<Value, Fault>, ChannelError> {
        Ok(match self {
            Self::Failed { message } => Err(Fault::new(Fault::REFUSED, message.clone())),
            reply => Ok(uwui_agent::write(reply)?),
        })
    }

    /// The reply that an outcome carries: every error as [`Reply::Failed`], with its message.
    pub(crate) fn read(outcome: Result<Value, Fault>) -> Result<Self, ChannelError> {
        match outcome {
            Ok(value) => uwui_agent::read(value).map_err(|fault| {
                ChannelError::Malformed(format!("a reply that does not read: {}", fault.message))
            }),
            Err(fault) => Ok(Self::Failed {
                message: fault.message,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_message_comes_back_from_its_method_and_its_parameters() {
        let text = |text: &str| text.to_owned();
        let requests = [
            Request::State,
            Request::Pause,
            Request::Resume,
            Request::Step { steps: 3 },
            Request::Speed { speed: 0.25 },
            Request::Stop,
            Request::Minimize,
            Request::Entities {
                filter: text("lant"),
            },
            Request::Entity {
                entity: text("hero"),
            },
            Request::SetProperty {
                entity: "zone/door".to_owned(),
                component: "Transform".to_owned(),
                path: "translation.x".to_owned(),
                value: "1.5".to_owned(),
            },
            Request::Press { key: text("KeyW") },
            Request::Release { key: text("KeyW") },
            Request::Capture { view: text("map") },
            Request::Report {
                name: text("assets"),
            },
            Request::Pick {
                view: text("yard"),
                x: 12,
                y: 34,
            },
            Request::SetViewSettings {
                view: text("map"),
                path: text("debug"),
                value: text("\"Normals\""),
            },
            Request::Logs { after: 7 },
            Request::Save {
                file: text("/tmp/run.toml"),
            },
            Request::Apply {
                commands: vec![text("[Delete]\nentity = \"zone/door\"\n")],
            },
            Request::Edit { edit: Edit::Played },
            Request::Load {
                file: text("/tmp/run.toml"),
            },
            Request::Edit {
                edit: Edit::Step {
                    label: text("Move the door"),
                    commands: vec![text("[Delete]\nentity = \"zone/door\"\n")],
                    selection: Some(vec![text("zone")]),
                },
            },
            Request::Edit {
                edit: Edit::Step {
                    label: text("Light the yard"),
                    commands: Vec::new(),
                    selection: None,
                },
            },
            Request::Edit {
                edit: Edit::Step {
                    label: text("Select nothing"),
                    commands: Vec::new(),
                    selection: Some(Vec::new()),
                },
            },
            Request::Edit { edit: Edit::Undo },
            Request::Edit { edit: Edit::Redo },
            Request::Edit {
                edit: Edit::History,
            },
            Request::Edit {
                edit: Edit::Journal,
            },
            Request::Edit {
                edit: Edit::Select {
                    selection: vec![text("zone/door"), text("zone/well")],
                },
            },
            Request::Edit {
                edit: Edit::Select {
                    selection: Vec::new(),
                },
            },
            Request::Edit { edit: Edit::Write },
            Request::Edit {
                edit: Edit::Operations,
            },
            Request::Edit {
                edit: Edit::Run {
                    name: text("file.open"),
                    arguments: vec![text("/levels/yard.scene")],
                },
            },
            Request::Elements {
                role: text("Button"),
                name: text("open"),
            },
            Request::ElementAt { x: 40, y: 12 },
            Request::Act {
                element: text("main/bar/open"),
                action: InterfaceAction::Press,
            },
            Request::Act {
                element: text("main/content/list"),
                action: InterfaceAction::Scroll { x: 0.0, y: 120.0 },
            },
            Request::Act {
                element: String::new(),
                action: InterfaceAction::Command { name: text("undo") },
            },
            Request::Act {
                element: text("main/bar/name"),
                action: InterfaceAction::SetValue { value: text("Ada") },
            },
            Request::Act {
                element: text("main/tree/rows/#2/name"),
                action: InterfaceAction::Drop {
                    onto: text("main/tree/rows/#0/name"),
                    at: 0.75,
                },
            },
            Request::CaptureElement {
                element: text("dialog/dialog"),
            },
        ];
        for request in requests {
            let (method, params) = request.call().expect("written");
            let (family, _) = method.split_once('/').expect("a family's method");
            assert!(
                crate::FAMILIES.iter().any(|&(name, _, _)| name == family),
                "{method} names a family that the channel serves"
            );
            assert_eq!(Request::read(family, &method, params), Ok(request));
        }
        let summary = EntitySummary {
            identity: text("hero"),
            name: String::new(),
        };
        let replies = [
            Reply::Done,
            Reply::Failed {
                message: text("game time runs: pause it first"),
            },
            Reply::State {
                state: GameState {
                    frame: 9,
                    paused: true,
                    held: false,
                    speed: 1.0,
                    steps: 8,
                    game_seconds: 0.125,
                    real_seconds: 0.15,
                    frame_ms: 16.7,
                    longest_frame_ms: 20.0,
                    entities: 400,
                    views: vec![ViewSummary {
                        name: text("map"),
                        settings: text("render_scale = 0.5\n"),
                    }],
                    reports: vec![text("assets")],
                },
            },
            Reply::Entities {
                entities: vec![summary.clone()],
                more: 2,
            },
            Reply::Entity {
                entity: summary,
                parent: text("zone"),
                components: vec![ComponentText {
                    name: text("Light"),
                    value: text("intensity = 12.5\n"),
                }],
            },
            Reply::Image {
                width: 4,
                height: 2,
            },
            Reply::Report {
                text: text("handles = [6, 0, 0]\n"),
            },
            Reply::Elements {
                elements: vec![
                    ElementSummary {
                        path: text("main/bar/open"),
                        role: text("Button"),
                        name: text("open"),
                        value: None,
                        description: Some(text("Opens a scene")),
                        placeholder: None,
                        states: vec![text("hovered"), text("focused")],
                        actions: vec![text("press"), text("focus")],
                        bounds: Some([0, 0, 120, 28]),
                    },
                    ElementSummary {
                        path: text("main/content/list/#57"),
                        role: text("ListItem"),
                        name: String::new(),
                        value: None,
                        description: None,
                        placeholder: None,
                        states: Vec::new(),
                        actions: Vec::new(),
                        bounds: None,
                    },
                    ElementSummary {
                        path: text("main/bar/scale"),
                        role: text("SpinButton"),
                        name: text("scale"),
                        value: Some(text("12.5")),
                        description: None,
                        placeholder: Some(text("a scale")),
                        states: Vec::new(),
                        actions: Vec::new(),
                        bounds: None,
                    },
                ],
                more: 0,
            },
            Reply::Picked {
                entity: EntitySummary {
                    identity: text("yard/sample/crate"),
                    name: text("crate"),
                },
                found: true,
            },
            Reply::Logs {
                lines: vec![LogLine {
                    number: 3,
                    text: text("INFO the zone is drawn"),
                }],
            },
            Reply::History {
                history: EditorHistory {
                    level: text("/levels/yard.scene"),
                    done: vec![text("Move the door")],
                    undone: vec![text("Light the yard")],
                    selection: vec![text("zone/door")],
                    recovered: 4,
                    journal: text("/data/yard.journal"),
                    unwritten: 2,
                },
            },
            Reply::Journal {
                text: text("# do\n"),
            },
            Reply::Written {
                left_out: vec![text("zone/door: a component of no known type")],
            },
            Reply::Played {
                address: text("127.0.0.1:41899"),
                token: text("0123"),
            },
            Reply::Operations {
                operations: vec![OperationSummary {
                    name: text("file.open"),
                    label: text("Open"),
                    parameters: vec![text("path")],
                    changes: text("file"),
                }],
            },
        ];
        for reply in replies {
            let outcome = reply.outcome().expect("written");
            assert_eq!(
                outcome.is_err(),
                matches!(reply, Reply::Failed { .. }),
                "a failure crosses as an error of the core's"
            );
            assert_eq!(Reply::read(outcome).expect("read"), reply);
        }
    }

    #[test]
    fn a_method_that_its_family_lacks_or_parameters_that_do_not_read_are_refused() {
        let refused = |family: &str, method: &str, params: Value| {
            Request::read(family, method, params).map_err(|fault| fault.code)
        };
        assert_eq!(
            refused("play", "play/rewind", Value::Null),
            Err(Fault::NO_METHOD)
        );
        assert_eq!(
            refused("editing", "editing/rewind", Value::Null),
            Err(Fault::NO_METHOD)
        );
        let steps = uwui_agent::write(&"three").expect("written");
        assert_eq!(refused("play", "play/step", steps), Err(Fault::BAD_PARAMS));
        assert_eq!(
            refused("play", "play/step", Value::Null),
            Err(Fault::BAD_PARAMS),
            "a step without its count"
        );
        assert!(
            matches!(
                Reply::read(Ok(Value::Null)),
                Err(ChannelError::Malformed(_))
            ),
            "a result that is no reply"
        );
        assert_eq!(
            Reply::read(Err(Fault::new(Fault::NOT_SERVED, "not served"))).expect("read"),
            Reply::Failed {
                message: "not served".to_owned()
            },
            "every error of the core's reads as a failure"
        );
    }

    #[test]
    fn every_event_comes_back_from_its_method_and_is_declared_by_its_family() {
        let events = [Event::Time(GameTime {
            paused: true,
            speed: 0.5,
            held: false,
        })];
        for event in events {
            let (method, params) = event.call().expect("written");
            assert!(
                crate::events().contains(&method),
                "{method} is declared by its family"
            );
            assert_eq!(Event::read(&method, params), Ok(event));
        }
        assert_eq!(
            crate::events(),
            ["play/time"],
            "each declared event has its variant"
        );
    }
}
