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
    /// The image that the game shows: [`Reply::Image`], once the next frame is drawn. It fails in a
    /// run without a window, and where the window's system does not let its images be copied.
    Capture,
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
    /// The image that the game shows, whose pixels follow the message: `width` × `height` pixels,
    /// rows from top to bottom, four bytes each, red, green, blue and alpha, sRGB-encoded.
    Image {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// Lines of the game's log.
    Logs {
        /// The lines, oldest first.
        lines: Vec<LogLine>,
    },
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
            Request::Capture,
            Request::Logs { after: 7 },
            Request::Save {
                file: text("/tmp/run.toml"),
            },
            Request::Load {
                file: text("/tmp/run.toml"),
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
            Reply::Logs {
                lines: vec![LogLine {
                    number: 3,
                    text: text("INFO the zone is drawn"),
                }],
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
