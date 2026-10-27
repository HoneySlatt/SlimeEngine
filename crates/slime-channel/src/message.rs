//! The messages of the protocol, and their encoding.

use std::fmt;
use std::sync::OnceLock;

use slime_reflect::{Reflect, Registry};
use slime_serialize::Mode;

use crate::error::ChannelError;

/// The first message of a connection, and only that one: the version of the protocol that the tool
/// speaks, and the token that the game was started with. Its `Debug` form hides the token.
#[derive(Clone, PartialEq, Reflect)]
#[reflect(name = "ChannelHello")]
pub struct Hello {
    /// [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION) of the tool.
    pub version: u32,
    /// The token, as its text.
    pub token: String,
}

impl fmt::Debug for Hello {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hello")
            .field("version", &self.version)
            .field("token", &"hidden")
            .finish()
    }
}

/// What a tool asks a game once its [`Hello`] is accepted, one message at a time, each answered by
/// a [`Reply`].
///
/// Entities are named by their persistent identities, `zone/door`; components by their types'
/// names, `Transform`; parts of components by property paths, `translation.x`; values, given and
/// answered, as text in the engine's notation, `12.5`, `{ x = 1.0, y = 0.0, z = 2.0 }`
/// (`slime_serialize::format`), which the game reads with the types it knows. Keys are named by
/// their position on the keyboard, as `slime_platform::KeyCode` names them: `KeyW`, `Escape`.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelRequest")]
pub enum Request {
    /// The game's state: [`Reply::State`].
    State,
    /// Pauses game time.
    Pause,
    /// Lets game time run again, which drops the steps asked for and not run yet.
    Resume,
    /// Runs that many fixed steps of game time, while it is paused, over as many frames as they take:
    /// answered once they have all run.
    Step {
        /// How many.
        steps: u32,
    },
    /// Sets the speed of game time, a finite number above zero.
    Speed {
        /// The speed.
        speed: f64,
    },
    /// Asks the game to stop, as its window closed would.
    Stop,
    /// The entities whose name or persistent identity holds `filter`, whatever its case:
    /// [`Reply::Entities`], at most [`MAX_ENTITIES`](crate::MAX_ENTITIES) of them.
    Entities {
        /// What to look for; empty for every entity.
        filter: String,
    },
    /// An entity and its components: [`Reply::Entity`].
    Entity {
        /// Its persistent identity.
        entity: String,
    },
    /// Sets a part of a component, as the world's `Command::SetProperty` does.
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
    Press {
        /// The key, by its position.
        key: String,
    },
    /// Lets a key go.
    Release {
        /// The key, by its position.
        key: String,
    },
    /// The image that the game shows, or the part of it that one of its views draws:
    /// [`Reply::Image`], once the next frame is drawn. It fails in a run without a window, where
    /// the window's system does not let its images be copied, and for a view that the frame does
    /// not draw.
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
    Report {
        /// The report's name, `assets`.
        name: String,
    },
    /// The lines of the game's log after the line numbered `after`: [`Reply::Logs`], at most
    /// [`MAX_LOG_LINES`](crate::MAX_LOG_LINES) of them.
    Logs {
        /// The number of the last line already read; 0 for the oldest that the game keeps.
        after: u64,
    },
    /// Saves the run to a file, as the game saves its own runs. It fails for a game that does not
    /// save its runs.
    Save {
        /// The file, a path on the game's machine.
        file: String,
    },
    /// Loads the run of the save in a file in place of the run, as the game resumes its own saves.
    /// It fails for a game that does not save its runs, and for a save that it does not resume,
    /// which leaves the run as it is.
    Load {
        /// The file, a path on the game's machine.
        file: String,
    },
    /// Asks an editor, a program that edits a level rather than plays it
    /// (`docs/adr/0135-the-editors-program.md`). It fails in a game, which is not an editor.
    Edit {
        /// What it asks.
        edit: Edit,
    },
    /// The elements of the game's interface that mean something, as its last frame left them
    /// (`docs/adr/0143-what-agents-see-of-the-interface.md`), in the order of its tree:
    /// [`Reply::Elements`], at most [`MAX_ELEMENTS`](crate::MAX_ELEMENTS) of them. It fails in a
    /// game that has no interface, as one that runs without a window.
    Elements {
        /// The role of those wanted, `Button`, whatever its case; empty for any.
        role: String,
        /// What their names hold, whatever its case; empty for any.
        name: String,
    },
    /// The element of the game's interface that the pointer would reach at a pixel of the game's
    /// image, or the nearest above it that means something: [`Reply::Elements`], one or none. It
    /// fails in a game that has no interface, as one that runs without a window.
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
    Act {
        /// The element, by its path, `#0/bar/open`; empty for an action of no element, a
        /// navigation intent or a command.
        element: String,
        /// What to do.
        action: InterfaceAction,
    },
    /// The part of the game's image where an element of its interface shows: [`Reply::Image`],
    /// once the next frame is drawn. It fails in a run without a window, in a game that has no
    /// interface, and for an element that does not show.
    CaptureElement {
        /// The element, by its path.
        element: String,
    },
}

/// What a tool does in a game's interface ([`Request::Act`]), as a person's input would.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelInterfaceAction")]
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
    /// Gives a field, a slider or two panels a value, as a person who replaced what it holds and
    /// committed would (`docs/adr/0146-screen-readers.md`, `docs/adr/0148-widgets.md`): the game
    /// reads the change and the commit of a field's or a slider's.
    SetValue {
        /// The text; for a field of numbers, a finite number, as Rust reads one.
        value: String,
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
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelEdit")]
pub enum Edit {
    /// Makes a step of `commands`, whole or not at all, named `label`, and selects `selection`
    /// after it, if it is given.
    Step {
        /// The step's name, which the history shows.
        label: String,
        /// The commands, as text, applied in their order.
        commands: Vec<String>,
        /// What to select after the step; none to keep what is selected.
        selection: Option<Vec<String>>,
    },
    /// Undoes the last step done. It fails when no step is done.
    Undo,
    /// Does again the last step undone. It fails when no step is undone.
    Redo,
    /// The editor's level, history and selection: [`Reply::History`].
    History,
    /// The text of the editor's journal: [`Reply::Journal`].
    Journal,
    /// Selects entities, which is not a step.
    Select {
        /// The entities selected.
        selection: Vec<String>,
    },
    /// Writes the level back to its file: [`Reply::Written`].
    Write,
}

/// What a game answers to a [`Request`].
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelReply")]
pub enum Reply {
    /// The connection is open: the game's protocol, and the name of its program.
    Welcome {
        /// [`PROTOCOL_VERSION`](crate::PROTOCOL_VERSION) of the game.
        version: u32,
        /// The game's program.
        program: String,
    },
    /// The connection is refused, and closed: why.
    Refused {
        /// Why.
        reason: String,
    },
    /// The request is done.
    Done,
    /// The request failed: why. The connection stays open.
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
    /// What a picking found: the entity drawn at the pixel, by its persistent identity and its
    /// name, both empty where none is, and its identity alone empty for an entity that has none.
    Picked {
        /// The entity, or nothing.
        entity: EntitySummary,
        /// Whether an entity is drawn there.
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
}

/// An editor's level, history and selection, as [`Reply::History`] gives them.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelEditorHistory")]
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
}

/// A game's state, as a frame's start finds it.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelGameState")]
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
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelView")]
pub struct ViewSummary {
    /// Its name, `map`.
    pub name: String,
    /// Its settings, as a document in the engine's notation (`slime_serialize::write`), the
    /// renderer's `ViewSettings` for a view of `slime-render`; empty for a view whose settings the
    /// game does not give.
    pub settings: String,
}

/// An entity, as a list of entities names it.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelEntity")]
pub struct EntitySummary {
    /// Its persistent identity; empty for an entity that has none.
    pub identity: String,
    /// Its name; empty for none.
    pub name: String,
}

/// An element of a game's interface, as a list of elements names it.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelElement")]
pub struct ElementSummary {
    /// Its path, which names it from one frame to the next: `#0/bar/open`.
    pub path: String,
    /// Its role, as AccessKit names it: `Button`.
    pub role: String,
    /// Its name; empty for none.
    pub name: String,
    /// What a field holds, as the user left it, a slider's number or two panels' first share; none
    /// for another element.
    pub value: Option<String>,
    /// Its states: `hovered`, `pressed`, `focused`, `disabled`, `checked`, `expanded`,
    /// `selected`, and `has_popup` for what a press opens a layer beside, an item of a menu that
    /// opens a submenu.
    pub states: Vec<String>,
    /// What can be done to it now: `press`, `focus`, `scroll`, `set_value`.
    pub actions: Vec<String>,
    /// The part of it that shows, in pixels of the game's image: its left column, its top row, its
    /// width and its height; none where it does not show.
    pub bounds: Option<[u32; 4]>,
}

/// A component of an entity, as text.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelComponent")]
pub struct ComponentText {
    /// Its type's name.
    pub name: String,
    /// Its value, as a document in the engine's notation (`slime_serialize::write`).
    pub value: String,
}

/// A line of a game's log.
#[derive(Clone, Debug, PartialEq, Reflect)]
#[reflect(name = "ChannelLogLine")]
pub struct LogLine {
    /// Its number, from 1 at the game's start, one more for each line.
    pub number: u64,
    /// The line, as the log writes it, without its time.
    pub text: String,
}

/// The registry of the protocol's messages.
fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = Registry::new();
        registry.register::<Hello>();
        registry.register::<Request>();
        registry.register::<Reply>();
        registry
    })
}

/// `message` in the engine's binary encoding.
pub(crate) fn encode<T: Reflect>(message: &T) -> Result<Vec<u8>, ChannelError> {
    let malformed = |error: &dyn std::fmt::Display| ChannelError::Malformed(error.to_string());
    let text = slime_serialize::write(registry(), &T::type_name(), &message.to_value())
        .map_err(|error| malformed(&error))?;
    slime_serialize::encode(&text).map_err(|error| malformed(&error))
}

/// The message that `bytes`, in the engine's binary encoding, hold.
pub(crate) fn decode<T: Reflect>(bytes: &[u8]) -> Result<T, ChannelError> {
    let malformed = |error: &dyn std::fmt::Display| ChannelError::Malformed(error.to_string());
    let read = slime_serialize::read_binary(registry(), &T::type_name(), bytes, Mode::Strict)
        .map_err(|error| malformed(&error))?;
    T::from_value(&read.value).map_err(|error| malformed(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_of_message_comes_back_from_its_encoding() {
        let hello = Hello {
            version: 1,
            token: "0".repeat(32),
        };
        assert_eq!(
            decode::<Hello>(&encode(&hello).expect("encoded")).expect("decoded"),
            hello
        );
        assert_eq!(
            format!("{hello:?}"),
            "Hello { version: 1, token: \"hidden\" }"
        );
        let text = |text: &str| text.to_owned();
        let requests = [
            Request::State,
            Request::Pause,
            Request::Resume,
            Request::Step { steps: 3 },
            Request::Speed { speed: 0.25 },
            Request::Stop,
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
            Request::Elements {
                role: text("Button"),
                name: text("open"),
            },
            Request::ElementAt { x: 40, y: 12 },
            Request::Act {
                element: text("#0/bar/open"),
                action: InterfaceAction::Press,
            },
            Request::Act {
                element: text("#0/content/list"),
                action: InterfaceAction::Scroll { x: 0.0, y: 120.0 },
            },
            Request::Act {
                element: String::new(),
                action: InterfaceAction::Command { name: text("undo") },
            },
            Request::Act {
                element: text("#0/bar/name"),
                action: InterfaceAction::SetValue { value: text("Ada") },
            },
            Request::CaptureElement {
                element: text("#1/dialog"),
            },
        ];
        for request in requests {
            let bytes = encode(&request).expect("encoded");
            assert_eq!(decode::<Request>(&bytes).expect("decoded"), request);
        }
        let summary = EntitySummary {
            identity: text("hero"),
            name: String::new(),
        };
        let replies = [
            Reply::Welcome {
                version: 1,
                program: text("hunnawood"),
            },
            Reply::Refused {
                reason: text("the token is not the game's"),
            },
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
                        path: text("#0/bar/open"),
                        role: text("Button"),
                        name: text("open"),
                        value: None,
                        states: vec![text("hovered"), text("focused")],
                        actions: vec![text("press"), text("focus")],
                        bounds: Some([0, 0, 120, 28]),
                    },
                    ElementSummary {
                        path: text("#0/content/list/#57"),
                        role: text("ListItem"),
                        name: String::new(),
                        value: None,
                        states: Vec::new(),
                        actions: Vec::new(),
                        bounds: None,
                    },
                    ElementSummary {
                        path: text("#0/bar/scale"),
                        role: text("SpinButton"),
                        name: text("scale"),
                        value: Some(text("12.5")),
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
                },
            },
            Reply::Journal {
                text: text("# do\n"),
            },
            Reply::Written {
                left_out: vec![text("zone/door: a component of no known type")],
            },
        ];
        for reply in replies {
            let bytes = encode(&reply).expect("encoded");
            assert_eq!(decode::<Reply>(&bytes).expect("decoded"), reply);
        }
    }

    #[test]
    fn bytes_that_are_not_a_message_are_refused() {
        let reply = encode(&Reply::Done).expect("encoded");
        for wrong in [&b""[..], b"SLDB", &reply[..reply.len() - 1]] {
            assert!(matches!(
                decode::<Request>(wrong),
                Err(ChannelError::Malformed(_))
            ));
        }
        assert!(
            matches!(decode::<Request>(&reply), Err(ChannelError::Malformed(_))),
            "a reply is no request"
        );
    }
}
