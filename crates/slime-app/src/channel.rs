//! The game's end of the development channel: a run started with a token answers the tools that
//! drive it, at the start of each frame (`docs/adr/0122-the-development-channel.md`).

use std::collections::VecDeque;
use std::io::Write as _;
use std::sync::{Arc, PoisonError};
use std::time::Duration;

use slime_channel::{
    ANNOUNCEMENT, ComponentText, EntitySummary, GameState, Incoming, Listener, LogLine,
    MAX_ENTITIES, MAX_LOG_LINES, Reply, Request, TOKEN_ENV_VAR, Token, ViewSummary,
};
use slime_gfx::{Gpu, Image, RenderTarget};
use slime_platform::KeyCode;
use slime_reflect::{Kind, Path, Registry, Value};
use slime_serialize::{Composer, Document, Mode};
use slime_world::{Command, Entity, Identities, Name, PersistentId, World};

use crate::game::{PickAnswer, PickSlot, Presentation, Reflected, ShownView, Simulation};
use crate::host::Host;
use crate::log_lines;

/// How many frames the measurements of the frames' times cover.
const FRAMES_MEASURED: usize = 120;

/// A run's development channel, open when the environment gave a token.
pub(crate) struct DevChannel {
    listener: Listener,
    /// The captures asked for, which the next frame drawn answers.
    captures: Vec<Incoming>,
    /// The pickings asked for, each with where the presentation gives its answer
    /// ([`Presentation::pick`]): answered once it gives it, a frame or more later.
    picks: Vec<(Incoming, PickSlot)>,
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
            picks: Vec::new(),
            steps: Vec::new(),
            frames: VecDeque::new(),
        })
    }

    /// At the start of a frame that represents `real` time, after `frames` frames: answers the
    /// requests that arrived, which change the host as a frame's start may. A step is answered once
    /// the frames have run every step asked for, so that what the tool asks next comes after them;
    /// a capture waits for the frame's image ([`DevChannel::capture`]), and a picking for the
    /// answer that the presentation gives a frame or more later ([`Presentation::pick`]);
    /// `windowed` says whether the run has one. A save and a load are `simulation`'s
    /// ([`Simulation::save`]); the views that the last frame drew, and their settings,
    /// `presentation`'s ([`Presentation::views`]).
    pub(crate) fn answer(
        &mut self,
        host: &mut Host,
        simulation: &mut dyn Simulation,
        presentation: &mut dyn Presentation,
        frames: u64,
        real: Duration,
        windowed: bool,
    ) {
        let views = presentation.views();
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
        let mut waiting = Vec::new();
        for (incoming, slot) in self.picks.drain(..) {
            // Counted before the answer is looked for: once the presentation let go of it, no
            // answer can come after.
            let alone = Arc::strong_count(&slot) == 1;
            let answer = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
            match answer {
                Some(entity) => incoming.answer(picked(host.world(), entity)),
                // The presentation dropped it: no answer comes.
                None if alone => incoming.answer(failed(
                    "the game gave no answer: its GPU device was lost, or the view went away",
                )),
                None => waiting.push((incoming, slot)),
            }
        }
        self.picks = waiting;
        while let Some(incoming) = self.listener.next() {
            if matches!(incoming.request(), Request::Capture { .. }) && windowed {
                self.captures.push(incoming);
                continue;
            }
            if let Request::Pick { view, x, y } = incoming.request() {
                match pick(presentation, &views, view, (*x, *y), windowed) {
                    Ok(slot) => self.picks.push((incoming, slot)),
                    Err(message) => incoming.answer(failed(message)),
                }
                continue;
            }
            let reply = self.reply(
                host,
                simulation,
                presentation,
                &views,
                frames,
                incoming.request(),
                windowed,
            );
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

    /// Answers the captures that wait with `target`, the frame's image, drawn and not yet shown, or
    /// with the part of it that one of `views` draws.
    pub(crate) fn capture(&mut self, gpu: &Gpu, target: &RenderTarget, views: &[ShownView]) {
        let image = gpu.read_render_target(target);
        for incoming in self.captures.drain(..) {
            let Request::Capture { view } = incoming.request().clone() else {
                unreachable!("only captures wait for the frame's image");
            };
            let image = match &image {
                Ok(image) => image,
                Err(error) => {
                    incoming.answer(failed(error));
                    continue;
                }
            };
            if view.is_empty() {
                let reply = Reply::Image {
                    width: image.width,
                    height: image.height,
                };
                incoming.answer_with(reply, image.pixels.clone());
            } else if let Some(shown) = views.iter().find(|shown| shown.name == view) {
                match cropped(image, shown) {
                    Some((width, height, pixels)) => {
                        incoming.answer_with(Reply::Image { width, height }, pixels);
                    }
                    None => incoming.answer(failed(format!(
                        "the view `{view}` lies outside the image, of {} × {} pixels",
                        image.width, image.height
                    ))),
                }
            } else {
                incoming.answer(failed(no_view(&view, views)));
            }
        }
    }

    /// The reply to `request`.
    #[expect(
        clippy::too_many_arguments,
        reason = "what the requests reach: the host, the game's two halves, and the frame's views"
    )]
    fn reply(
        &self,
        host: &mut Host,
        simulation: &mut dyn Simulation,
        presentation: &mut dyn Presentation,
        views: &[ShownView],
        frames: u64,
        request: &Request,
        windowed: bool,
    ) -> Reply {
        let ctx = host.context_mut();
        match request {
            Request::State => Reply::State {
                state: self.state(host, presentation, views, frames),
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
            } => set_property(simulation, host.world_mut(), entity, component, path, value)
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
            Request::Report { name } => {
                report(presentation, name).map_or_else(failed, |text| Reply::Report { text })
            }
            Request::SetViewSettings { view, path, value } => {
                set_view_settings(host.world(), presentation, views, view, path, value)
                    .map_or_else(failed, |()| Reply::Done)
            }
            Request::Capture { .. } => {
                debug_assert!(!windowed, "a windowed run's capture waits for its image");
                failed("a run without a window shows no image")
            }
            Request::Pick { .. } => unreachable!("`answer` asks the presentation to pick"),
            Request::Logs { after } => Reply::Logs {
                lines: log_lines::after(*after)
                    .into_iter()
                    .take(MAX_LOG_LINES)
                    .map(|(number, text)| LogLine { number, text })
                    .collect(),
            },
            Request::Save { file } => simulation
                .save(host.world(), std::path::Path::new(file))
                .map_or_else(
                    |error| failed(crate::error_chain(&*error)),
                    |()| Reply::Done,
                ),
            Request::Load { file } => simulation
                .load(host.world_mut(), std::path::Path::new(file))
                .map_or_else(
                    |error| failed(crate::error_chain(&*error)),
                    |()| Reply::Done,
                ),
            Request::Edit { edit } => simulation
                .edit(host.world_mut(), edit)
                .unwrap_or_else(|error| failed(crate::error_chain(&*error))),
        }
    }

    /// The game's state at the start of a frame, after `frames` frames, the last of which drew
    /// `views`, whose settings `presentation` gives.
    fn state(
        &self,
        host: &Host,
        presentation: &dyn Presentation,
        views: &[ShownView],
        frames: u64,
    ) -> GameState {
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
            views: views
                .iter()
                .map(|shown| ViewSummary {
                    name: shown.name.clone(),
                    settings: presentation
                        .view_settings(&shown.name)
                        .map(|settings| reflected_text(&settings))
                        .unwrap_or_default(),
                })
                .collect(),
            reports: presentation
                .reports()
                .into_iter()
                .map(|(name, _)| name)
                .collect(),
        }
    }
}

/// The reply that says why a request failed.
fn failed(why: impl ToString) -> Reply {
    Reply::Failed {
        message: why.to_string(),
    }
}

/// Why a request that names `view` fails when the frame drew `views`, none of which is named so.
fn no_view(view: &str, views: &[ShownView]) -> String {
    format!(
        "the frame draws no view `{view}`: it draws {}",
        named(views)
    )
}

/// The names of `views`, as a message gives them.
fn named(views: &[ShownView]) -> String {
    let names: Vec<&str> = views.iter().map(|shown| shown.name.as_str()).collect();
    if names.is_empty() {
        "none by name".to_owned()
    } else {
        format!("`{}`", names.join("`, `"))
    }
}

/// `reflected` as a document in the engine's notation.
fn reflected_text(reflected: &Reflected) -> String {
    let mut composer = Composer::new(&reflected.registry);
    composer
        .put(&[], &reflected.ty, &reflected.value)
        .and_then(|()| composer.finish())
        .unwrap_or_else(|error| format!("# cannot be written: {error}"))
}

/// The report of `presentation`'s named `name`, as a document in the engine's notation.
fn report(presentation: &dyn Presentation, name: &str) -> Result<String, String> {
    let reports = presentation.reports();
    match reports.iter().find(|(named, _)| named == name) {
        Some((_, report)) => Ok(reflected_text(report)),
        None => {
            let names: Vec<&str> = reports.iter().map(|(named, _)| named.as_str()).collect();
            Err(format!(
                "the game gives no report `{name}`: it gives {}",
                if names.is_empty() {
                    "none".to_owned()
                } else {
                    format!("`{}`", names.join("`, `"))
                }
            ))
        }
    }
}

/// Sets the part at `path` of the settings of `presentation`'s view `view`, one of `views`, to
/// `text`, a value in the engine's notation, which the settings' reflection checks; `world` names
/// the entities that a value may refer to.
fn set_view_settings(
    world: &World,
    presentation: &mut dyn Presentation,
    views: &[ShownView],
    view: &str,
    path: &str,
    text: &str,
) -> Result<(), String> {
    if !views.iter().any(|shown| shown.name == view) {
        return Err(no_view(view, views));
    }
    let mut settings = presentation
        .view_settings(view)
        .ok_or_else(|| format!("the game does not give the settings of its view `{view}`"))?;
    let path: Path = path
        .parse()
        .map_err(|error| format!("`{path}` is no property path: {error}"))?;
    let ty = settings
        .registry
        .type_at(&settings.ty, &settings.value, &path)
        .map_err(|error| error.to_string())?;
    let value = read_value(&settings.registry, &ty, text, &Identities(world))?;
    settings
        .registry
        .set(&settings.ty, &mut settings.value, &path, value)
        .map_err(|error| error.to_string())?;
    presentation.set_view_settings(view, &settings)
}

/// The part of `image` that `view` covers, cut at the image's edges, as a presentation's views of
/// an earlier frame may pass them: its width, its height and its pixels, rows from top to bottom,
/// four bytes each; `None` when the view covers none of the image.
fn cropped(image: &Image, view: &ShownView) -> Option<(u32, u32, Vec<u8>)> {
    let right = view.x.saturating_add(view.width).min(image.width);
    let bottom = view.y.saturating_add(view.height).min(image.height);
    if view.x >= right || view.y >= bottom {
        return None;
    }
    let row_bytes = (right - view.x) as usize * 4;
    let pixels = (view.y..bottom)
        .flat_map(|row| {
            let start = (row as usize * image.width as usize + view.x as usize) * 4;
            &image.pixels[start..start + row_bytes]
        })
        .copied()
        .collect();
    Some((right - view.x, bottom - view.y, pixels))
}

/// Asks `presentation` to pick at `(x, y)` of the view named `view` among `views`, or of the whole
/// image when `view` is empty, where the last view drawn there is picked; returns where its answer
/// comes, or why it cannot pick there.
fn pick(
    presentation: &mut dyn Presentation,
    views: &[ShownView],
    view: &str,
    (x, y): (u32, u32),
    windowed: bool,
) -> Result<PickSlot, String> {
    if !windowed {
        return Err("a run without a window shows no image to pick in".to_owned());
    }
    let inside = |shown: &&ShownView| {
        (shown.x..shown.x + shown.width).contains(&x)
            && (shown.y..shown.y + shown.height).contains(&y)
    };
    let (name, at) = if view.is_empty() {
        let shown = views.iter().rev().find(inside).ok_or_else(|| {
            format!(
                "no view of the game is drawn at ({x}, {y}): it draws {}",
                named(views)
            )
        })?;
        (&shown.name, (x - shown.x, y - shown.y))
    } else {
        let shown = views
            .iter()
            .find(|shown| shown.name == view)
            .ok_or_else(|| no_view(view, views))?;
        if x >= shown.width || y >= shown.height {
            return Err(format!(
                "({x}, {y}) lies outside the view `{view}`, of {} × {} pixels",
                shown.width, shown.height
            ));
        }
        (&shown.name, (x, y))
    };
    let (answer, slot) = PickAnswer::new();
    presentation.pick(name, at, answer)?;
    Ok(slot)
}

/// The reply that names `entity` of `world`, found by a picking, or nothing: an entity destroyed
/// since it was drawn is nothing.
fn picked(world: &World, entity: Option<Entity>) -> Reply {
    match entity.filter(|&entity| world.contains(entity)) {
        Some(entity) => Reply::Picked {
            entity: summary(world, entity),
            found: true,
        },
        None => Reply::Picked {
            entity: EntitySummary {
                identity: String::new(),
                name: String::new(),
            },
            found: false,
        },
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
/// engine's notation, as a command does at the start of a frame, through `simulation`
/// ([`Simulation::apply`]).
fn set_property(
    simulation: &mut dyn Simulation,
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
    simulation
        .apply(world, &command)
        .map_err(|error| crate::error_chain(&*error))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A view's part of the image, cut at the image's edges; none for a view beyond them.
    #[test]
    fn a_capture_of_a_view_is_its_part_of_the_image() {
        // 4 × 3 pixels, each holding its column and row.
        let image = Image {
            width: 4,
            height: 3,
            pixels: (0..3_u8)
                .flat_map(|y| (0..4_u8).flat_map(move |x| [x, y, 0, 255]))
                .collect(),
        };
        let view = |x, y, width, height| ShownView {
            name: "map".to_owned(),
            x,
            y,
            width,
            height,
        };
        let (width, height, pixels) = cropped(&image, &view(1, 1, 2, 2)).expect("inside");
        assert_eq!((width, height), (2, 2));
        assert_eq!(
            pixels,
            [
                [1, 1, 0, 255],
                [2, 1, 0, 255],
                [1, 2, 0, 255],
                [2, 2, 0, 255]
            ]
            .concat()
        );
        let (width, height, pixels) = cropped(&image, &view(3, 2, 5, 5)).expect("across");
        assert_eq!((width, height, pixels), (1, 1, vec![3, 2, 0, 255]));
        assert_eq!(cropped(&image, &view(4, 0, 2, 2)), None);
        assert_eq!(cropped(&image, &view(0, 0, 0, 2)), None);
    }

    /// The settings of the view of [`Dials`], as reflection describes them.
    #[derive(Clone, Copy, Debug, PartialEq, slime_reflect::Reflect)]
    #[reflect(name = "test::Dial")]
    struct Dial {
        /// How far it is turned.
        #[reflect(range(min = 0.0, max = 1.0))]
        level: f32,
        /// Whether it is on.
        on: bool,
    }

    /// A presentation that names a view, `dial`, whose settings a tool reads and changes, and gives
    /// them as a report too.
    struct Dials {
        dial: Dial,
    }

    impl Presentation for Dials {
        fn reports(&self) -> Vec<(String, Reflected)> {
            vec![("dial".to_owned(), Reflected::new(&self.dial))]
        }

        fn view_settings(&self, name: &str) -> Option<Reflected> {
            (name == "dial").then(|| Reflected::new(&self.dial))
        }

        fn set_view_settings(&mut self, name: &str, settings: &Reflected) -> Result<(), String> {
            assert_eq!(
                name, "dial",
                "the channel names the views that the frame drew"
            );
            self.dial = settings.get().map_err(|error| error.to_string())?;
            Ok(())
        }
    }

    /// A tool reads a view's settings as text, and sets a part of them, or the whole, within the
    /// ranges of their fields; a view that the frame did not draw, or whose settings the game does
    /// not give, is refused. A report is read by its name.
    #[test]
    fn a_tool_reads_and_changes_the_settings_of_a_view_and_reads_a_report() {
        let world = World::new();
        let mut dials = Dials {
            dial: Dial {
                level: 0.5,
                on: false,
            },
        };
        let views = ["dial", "map"].map(|name| ShownView {
            name: name.to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        });
        let text = reflected_text(&dials.view_settings("dial").expect("given"));
        assert!(text.contains("level = 0.5"), "{text}");
        assert!(text.contains("on = false"), "{text}");
        let mut set = |view: &str, path: &str, value: &str| {
            set_view_settings(&world, &mut dials, &views, view, path, value)
        };
        assert_eq!(set("dial", "level", "0.25"), Ok(()));
        assert_eq!(set("dial", "", "{ level = 1.0, on = true }"), Ok(()));
        // Out of its range, of another type, a part it does not have.
        for (path, value) in [("level", "1.5"), ("on", "\"yes\""), ("speed", "1.0")] {
            assert!(set("dial", path, value).is_err(), "{path} = {value}");
        }
        let refused = set("map", "level", "0.5").expect_err("not given");
        assert_eq!(
            refused,
            "the game does not give the settings of its view `map`"
        );
        let refused = set("yard", "level", "0.5").expect_err("not drawn");
        assert_eq!(
            refused,
            "the frame draws no view `yard`: it draws `dial`, `map`"
        );
        assert_eq!(
            dials.dial,
            Dial {
                level: 1.0,
                on: true
            }
        );
        // Its report, read by name.
        let text = report(&dials, "dial").expect("given");
        assert!(text.contains("level = 1.0"), "{text}");
        assert_eq!(
            report(&dials, "assets").expect_err("not given"),
            "the game gives no report `assets`: it gives `dial`"
        );
        // A presentation that keeps the default refuses every change.
        let refused = set_view_settings(&world, &mut (), &views, "dial", "level", "0.5");
        assert!(refused.is_err());
    }

    /// A presentation that picks, keeping each picking asked for, which the test answers.
    #[derive(Default)]
    struct Clicks {
        asked: Vec<(String, (u32, u32), PickAnswer)>,
    }

    impl Presentation for Clicks {
        fn pick(&mut self, view: &str, at: (u32, u32), answer: PickAnswer) -> Result<(), String> {
            self.asked.push((view.to_owned(), at, answer));
            Ok(())
        }
    }

    /// A tool picks at a pixel of a view, counted from its corner, or of the whole image, where the
    /// last view drawn there is picked; the answer names the entity once the presentation gives
    /// it, and nothing for an entity destroyed since; a pixel outside, a run without a window and a
    /// game that does not pick are refused.
    #[test]
    fn a_tool_picks_at_a_pixel_of_a_view_or_of_the_image() {
        let mut world = World::new();
        let door = world.spawn();
        world.insert(door, Name("door".to_owned()));
        let views =
            [("yard", 0, 0, 100, 50), ("map", 80, 0, 20, 20)].map(|(name, x, y, width, height)| {
                ShownView {
                    name: name.to_owned(),
                    x,
                    y,
                    width,
                    height,
                }
            });
        let mut clicks = Clicks::default();
        let in_map = pick(&mut clicks, &views, "", (85, 10), true).expect("picked");
        let in_yard = pick(&mut clicks, &views, "yard", (85, 10), true).expect("picked");
        let asked: Vec<(&str, (u32, u32))> = clicks
            .asked
            .iter()
            .map(|(view, at, _)| (view.as_str(), *at))
            .collect();
        assert_eq!(asked, [("map", (5, 10)), ("yard", (85, 10))]);
        let mut answers = std::mem::take(&mut clicks.asked)
            .into_iter()
            .map(|(_, _, answer)| answer);
        answers.next().expect("asked").give(Some(door));
        answers.next().expect("asked").give(None);
        let given = |slot: &PickSlot| slot.lock().unwrap().expect("given");
        let Reply::Picked { entity, found } = picked(&world, given(&in_map)) else {
            panic!("a picking answers with what it found");
        };
        assert!(found);
        assert_eq!(entity.name, "door");
        assert_eq!(
            picked(&world, given(&in_yard)),
            Reply::Picked {
                entity: EntitySummary {
                    identity: String::new(),
                    name: String::new()
                },
                found: false
            }
        );
        world.despawn(door);
        assert!(matches!(
            picked(&world, Some(door)),
            Reply::Picked { found: false, .. }
        ));
        for (view, at, windowed) in [
            ("map", (20, 0), true),
            ("", (100, 0), true),
            ("minimap", (0, 0), true),
            ("yard", (0, 0), false),
        ] {
            assert!(
                pick(&mut clicks, &views, view, at, windowed).is_err(),
                "{view} at {at:?}, windowed {windowed}"
            );
        }
        assert!(pick(&mut (), &views, "yard", (0, 0), true).is_err());
    }
}
