//! The game's end of the development channel: a run started with a token answers the tools that
//! drive it, at the start of each frame (`docs/adr/0122-the-development-channel.md`).

use std::collections::VecDeque;
use std::io::Write as _;
use std::time::Duration;

use slime_channel::{
    ANNOUNCEMENT, ComponentText, EntitySummary, GameState, Incoming, Listener, LogLine,
    MAX_ENTITIES, MAX_LOG_LINES, Reply, Request, TOKEN_ENV_VAR, Token,
};
use slime_gfx::{Gpu, RenderTarget};
use slime_platform::KeyCode;
use slime_reflect::{Kind, Path, Reflect as _, Registry, Value};
use slime_serialize::{Composer, Document, Mode, References};
use slime_world::{Command, Entity, Name, PersistentId, World};

use crate::host::Host;
use crate::log_lines;

/// How many frames the measurements of the frames' times cover.
const FRAMES_MEASURED: usize = 120;

/// A run's development channel, open when the environment gave a token.
pub(crate) struct DevChannel {
    listener: Listener,
    /// The captures asked for, which the next frame drawn answers.
    captures: Vec<Incoming>,
    /// The steps asked for, each with the count of steps run that answers it: answered once the
    /// frames have run them.
    steps: Vec<(Incoming, u64)>,
    /// The time between the starts of the last frames.
    frames: VecDeque<Duration>,
}

impl DevChannel {
    /// Listens when the environment gives a token ([`TOKEN_ENV_VAR`]), and writes where it listens
    /// on the standard output; `None` without a token, or when it cannot listen, which the log says.
    pub(crate) fn open() -> Option<Self> {
        let text = std::env::var(TOKEN_ENV_VAR).ok()?;
        let token = match text.parse::<Token>() {
            Ok(token) => token,
            Err(error) => {
                tracing::error!("the development channel does not listen: {error}");
                return None;
            }
        };
        let program = std::env::current_exe()
            .ok()
            .and_then(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let listener = match Listener::open(token, &program) {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(%error, "the development channel cannot listen");
                return None;
            }
        };
        let address = listener.address();
        let mut stdout = std::io::stdout().lock();
        // Whoever started the game reads it there; a closed output only leaves it unread.
        let _ = writeln!(stdout, "{ANNOUNCEMENT}{address}").and_then(|()| stdout.flush());
        tracing::info!(%address, "the development channel listens");
        Some(Self {
            listener,
            captures: Vec::new(),
            steps: Vec::new(),
            frames: VecDeque::new(),
        })
    }

    /// At the start of a frame that represents `real` time, after `frames` frames: answers the
    /// requests that arrived, which change the host as a frame's start may. A step is answered once
    /// the frames have run every step asked for, so that what the tool asks next comes after them;
    /// a capture waits for the frame's image ([`DevChannel::capture`]); `windowed` says whether
    /// the run has one.
    pub(crate) fn answer(&mut self, host: &mut Host, frames: u64, real: Duration, windowed: bool) {
        self.frames.push_back(real);
        if self.frames.len() > FRAMES_MEASURED {
            self.frames.pop_front();
        }
        let (run, asked) = (
            host.context().time().game().steps(),
            host.context().steps_asked(),
        );
        let mut waiting = Vec::new();
        for (incoming, until) in self.steps.drain(..) {
            if run >= until {
                incoming.answer(Reply::Done);
            } else if asked == 0 {
                // Game time ran again, which dropped the steps not run.
                incoming.answer(failed(
                    "game time runs again: the steps not run yet were dropped",
                ));
            } else {
                waiting.push((incoming, until));
            }
        }
        self.steps = waiting;
        while let Some(incoming) = self.listener.next() {
            if matches!(incoming.request(), Request::Capture) && windowed {
                self.captures.push(incoming);
                continue;
            }
            let reply = self.reply(host, frames, incoming.request(), windowed);
            if matches!(incoming.request(), Request::Step { .. }) && reply == Reply::Done {
                let ctx = host.context();
                let until = ctx.time().game().steps() + u64::from(ctx.steps_asked());
                self.steps.push((incoming, until));
            } else {
                incoming.answer(reply);
            }
        }
    }

    /// Whether a capture waits for this frame's image.
    pub(crate) fn wants_capture(&self) -> bool {
        !self.captures.is_empty()
    }

    /// Answers the captures that wait with `target`, the frame's image, drawn and not yet shown.
    pub(crate) fn capture(&mut self, gpu: &Gpu, target: &RenderTarget) {
        let image = gpu.read_render_target(target);
        for incoming in self.captures.drain(..) {
            match &image {
                Ok(image) => incoming.answer_with(
                    Reply::Image {
                        width: image.width,
                        height: image.height,
                    },
                    image.pixels.clone(),
                ),
                Err(error) => incoming.answer(failed(error)),
            }
        }
    }

    /// The reply to `request`.
    fn reply(&self, host: &mut Host, frames: u64, request: &Request, windowed: bool) -> Reply {
        let ctx = host.context_mut();
        match request {
            Request::State => Reply::State {
                state: self.state(host, frames),
            },
            Request::Pause => {
                ctx.pause();
                Reply::Done
            }
            Request::Resume => {
                ctx.resume();
                Reply::Done
            }
            Request::Step { steps } => {
                if !ctx.paused() {
                    return failed("game time runs: pause it first");
                }
                ctx.ask_steps(*steps);
                Reply::Done
            }
            Request::Speed { speed } => {
                if !(speed.is_finite() && *speed > 0.0) {
                    return failed(format!(
                        "{speed} is no speed: a finite number above zero is"
                    ));
                }
                ctx.set_speed(*speed);
                Reply::Done
            }
            Request::Stop => {
                ctx.request_exit();
                Reply::Done
            }
            Request::Entities { filter } => entities(host.world(), filter),
            Request::Entity { entity } => entity_text(host.world(), entity).unwrap_or_else(failed),
            Request::SetProperty {
                entity,
                component,
                path,
                value,
            } => set_property(host.world_mut(), entity, component, path, value)
                .map_or_else(failed, |()| Reply::Done),
            Request::Press { key } | Request::Release { key } => {
                let Some(code) = KeyCode::from_name(key) else {
                    return failed(format!(
                        "no key `{key}`: keys are named by their position, `KeyW`, `Escape`"
                    ));
                };
                let keys = host.input_mut().keys_mut();
                if matches!(request, Request::Press { .. }) {
                    keys.press(code);
                } else {
                    keys.release(code);
                }
                Reply::Done
            }
            Request::Capture => {
                debug_assert!(!windowed, "a windowed run's capture waits for its image");
                failed("a run without a window shows no image")
            }
            Request::Logs { after } => Reply::Logs {
                lines: log_lines::after(*after)
                    .into_iter()
                    .take(MAX_LOG_LINES)
                    .map(|(number, text)| LogLine { number, text })
                    .collect(),
            },
        }
    }

    /// The game's state at the start of a frame, after `frames` frames.
    fn state(&self, host: &Host, frames: u64) -> GameState {
        let ctx = host.context();
        let (real, game) = (ctx.time().real(), ctx.time().game());
        let milliseconds = |time: Duration| time.as_secs_f64() * 1000.0;
        let measured = self.frames.len().max(1) as f64;
        GameState {
            frame: frames,
            paused: ctx.paused(),
            held: ctx.held(),
            speed: ctx.speed(),
            steps: game.steps(),
            game_seconds: game.elapsed().as_secs_f64(),
            real_seconds: real.elapsed().as_secs_f64(),
            frame_ms: self.frames.iter().copied().map(milliseconds).sum::<f64>() / measured,
            longest_frame_ms: self
                .frames
                .iter()
                .copied()
                .map(milliseconds)
                .fold(0.0, f64::max),
            entities: host.world().len() as u64,
        }
    }
}

/// The reply that says why a request failed.
fn failed(why: impl ToString) -> Reply {
    Reply::Failed {
        message: why.to_string(),
    }
}

/// The entity of `world` and its name, as a list of entities gives it.
fn summary(world: &World, entity: Entity) -> EntitySummary {
    EntitySummary {
        identity: world
            .persistent_id(entity)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        name: world
            .get::<Name>(entity)
            .map(|name| name.0.clone())
            .unwrap_or_default(),
    }
}

/// The entities of `world` whose name or identity holds `filter`, whatever its case.
fn entities(world: &World, filter: &str) -> Reply {
    let filter = filter.to_lowercase();
    let mut found = world
        .entities()
        .map(|entity| summary(world, entity))
        .filter(|entity| {
            entity.identity.to_lowercase().contains(&filter)
                || entity.name.to_lowercase().contains(&filter)
        });
    let entities: Vec<EntitySummary> = found.by_ref().take(MAX_ENTITIES).collect();
    Reply::Entities {
        entities,
        more: found.count() as u64,
    }
}

/// The entity of `world` whose persistent identity is `identity`, and that identity.
fn find(world: &World, identity: &str) -> Result<(Entity, PersistentId), String> {
    let id: PersistentId = identity
        .parse()
        .map_err(|error| format!("`{identity}` is no persistent identity: {error}"))?;
    let entity = world
        .find(&id)
        .ok_or_else(|| format!("no entity alive has the persistent identity `{identity}`"))?;
    Ok((entity, id))
}

/// The entity of `world` named `identity`, its parent and its components as text.
fn entity_text(world: &World, identity: &str) -> Result<Reply, String> {
    let (entity, _) = find(world, identity)?;
    let names = Identities(world);
    let components = world
        .component_names(entity)
        .into_iter()
        .filter_map(|name| {
            let value = world.component_value(entity, name)?;
            let mut composer = Composer::new(world.registry()).naming(&names);
            let text = composer
                .put(&[], name, &value)
                .and_then(|()| composer.finish())
                .unwrap_or_else(|error| format!("# cannot be written: {error}"));
            Some(ComponentText {
                name: name.to_owned(),
                value: text,
            })
        })
        .collect();
    let parent = world
        .parent(entity)
        .and_then(|parent| world.persistent_id(parent))
        .map(|id| id.to_string())
        .unwrap_or_default();
    Ok(Reply::Entity {
        entity: summary(world, entity),
        parent,
        components,
    })
}

/// Sets the part at `path` of the component of `world`'s entity `identity` to `text`, a value in the
/// engine's notation, as a command does at the start of a frame.
fn set_property(
    world: &mut World,
    identity: &str,
    component: &str,
    path: &str,
    text: &str,
) -> Result<(), String> {
    let (entity, id) = find(world, identity)?;
    let path: Path = path
        .parse()
        .map_err(|error| format!("`{path}` is no property path: {error}"))?;
    let current = world
        .component_value(entity, component)
        .ok_or_else(|| format!("`{identity}` holds no component `{component}`"))?;
    let registry = world.registry();
    let ty_name = registry
        .get(component)
        .map(|registration| registration.info().name.clone())
        .ok_or_else(|| format!("no component type `{component}`"))?;
    let ty = registry
        .type_at(&ty_name, &current, &path)
        .map_err(|error| error.to_string())?;
    let value = read_value(registry, &ty, text, &Identities(world))?;
    let command = Command::SetProperty {
        entity: id,
        component: ty_name,
        path,
        value,
    };
    world.apply(&command).map_err(|error| error.to_string())
}

/// The value of the type named `ty` that `text`, in the engine's notation, writes. An option is
/// given as its value, or as `[]` for none.
fn read_value(
    registry: &Registry,
    ty: &str,
    text: &str,
    names: &Identities<'_>,
) -> Result<Value, String> {
    let read = |ty: &str, text: &str| -> Result<Value, String> {
        let document = format!("value = {text}\n");
        Document::parse(registry, &document, Mode::Strict)
            .and_then(|mut document| document.read(&["value"], ty, Some(names)))
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "no value".to_owned())
    };
    let option = registry
        .get(ty)
        .and_then(|registration| match &registration.info().kind {
            Kind::Option(inner) => Some(inner.clone()),
            _ => None,
        });
    match option {
        // `[]`, or `[value]`, as documents write options elsewhere than in fields; else the value.
        Some(inner) => read(ty, text)
            .or_else(|_| read(&inner, text).map(|value| Value::Option(Some(Box::new(value))))),
        None => read(ty, text),
    }
}

/// The world's entities, named by their persistent identities, as the channel names them.
struct Identities<'a>(&'a World);

impl References for Identities<'_> {
    /// An entity without a persistent identity, or no entity, is written `""`, as saves write no
    /// entity: a component that refers to one is read whole, but cannot be written back.
    fn name(&self, entity: u64) -> Option<String> {
        let id = Entity::from_value(&Value::Entity(entity))
            .ok()
            .and_then(|entity| self.0.persistent_id(entity));
        Some(id.map_or_else(String::new, |id| id.to_string()))
    }

    fn entity(&self, name: &str) -> Option<u64> {
        let entity = self.0.find(&name.parse().ok()?)?;
        match entity.to_value() {
            Value::Entity(bits) => Some(bits),
            _ => None,
        }
    }
}
