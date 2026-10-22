//! A game as a simulation and a presentation (`docs/adr/0107-the-execution-model.md`): the two
//! traits that a game implements, which a host runs.

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use slime_gfx::RenderTarget;
use slime_platform::Event;
use slime_reflect::{Reflect, ReflectError, Registry, Value};
use slime_world::{Entity, World};

use crate::error::AppError;
use crate::host::{Context, SimulationContext};

/// A game's rules: what changes its world, on game time, from the input given as data
/// (`docs/adr/0107-the-execution-model.md`). It has no GPU and no window, and does not end the
/// process: it asks its host to stop ([`SimulationContext::request_exit`]), and the host decides
/// what stopping means. The game's program runs it with its [`Presentation`] ([`run`](crate::run));
/// a test runs it alone, without a GPU ([`Host`](crate::Host)).
///
/// Every method has a default that does nothing. A host calls, in the phases of a frame:
///
/// 1. [`start`](Self::start), once, before the first frame;
/// 2. for each frame, once the host applied the commands queued since the last one,
///    [`fixed_step`](Self::fixed_step) once per fixed step of game time due in that frame (zero or
///    more times), then [`update`](Self::update) once; the host then computes the world's
///    transforms, and the presentation reads the world;
/// 3. [`stop`](Self::stop), once, after the last frame, if the run started.
///
/// Each method receives the world, which it changes, and a [`SimulationContext`]: the input, both
/// times, and the run's stop.
///
/// ```
/// use slime_app::{AppConfig, Host, Simulation, SimulationContext};
/// use slime_world::World;
///
/// /// Counts the steps of game time.
/// #[derive(Default)]
/// struct Clock {
///     steps: u32,
/// }
///
/// impl Simulation for Clock {
///     fn fixed_step(&mut self, _ctx: &mut SimulationContext<'_>, _world: &mut World) {
///         self.steps += 1;
///     }
/// }
///
/// let (mut host, mut clock) = (Host::new(&AppConfig::default()), Clock::default());
/// host.start(&mut clock).expect("it starts");
/// let step = AppConfig::default().fixed_step;
/// host.context_mut().set_speed(0.5);
/// for _ in 0..4 {
///     host.frame(&mut clock, step);
/// }
/// assert_eq!(clock.steps, 2, "at half the speed, a step every other frame");
/// ```
pub trait Simulation {
    /// Called once, before the first frame: where a simulation puts its world in the host's, when
    /// it built it beforehand. In a windowed run, after the window and the GPU are open; neither
    /// the simulation nor the presentation runs a frame when this method or the presentation's
    /// [`Presentation::start`] fails.
    ///
    /// # Errors
    ///
    /// A simulation returns an error when it cannot start: no frame runs, neither `stop` is called,
    /// and [`run`](crate::run) returns [`RunError::Init`](crate::RunError::Init) with this error.
    fn start(
        &mut self,
        ctx: &mut SimulationContext<'_>,
        world: &mut World,
    ) -> Result<(), AppError> {
        let _ = (ctx, world);
        Ok(())
    }

    /// Called once per fixed step of game time, at phase 2 of a frame. The step lasts
    /// [`GameTime::step`](crate::time::GameTime::step) of game time, whatever the speed. What it
    /// moves in the world is drawn between where it was before the step and where the step leaves
    /// it ([`World::interpolated_transform`]).
    fn fixed_step(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let _ = (ctx, world);
    }

    /// Called once per frame, at phase 3, after the frame's fixed steps, while game time is paused
    /// too. What it changes in the world is drawn at once. The input's presses and releases are
    /// those since the last frame: a simulation that reads a key going down reads it here, which
    /// every frame runs, where a fixed step runs in some frames only.
    fn update(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let _ = (ctx, world);
    }

    /// Called once, after the last frame, however the run stopped, unless it did not start or the
    /// process ends before the run stops (see [`run`](crate::run)): a game saves its run here.
    fn stop(&mut self, ctx: &mut SimulationContext<'_>, world: &mut World) {
        let _ = (ctx, world);
    }

    /// Saves the run in `world` to `file`, as a game saves its own runs: what a tool that drives
    /// the game asks through the development channel, at the start of a frame
    /// (`docs/adr/0122-the-development-channel.md`, `docs/adr/0125-saves-per-scene.md`).
    ///
    /// # Errors
    ///
    /// Why the run is not saved: by default, that the game does not save its runs.
    fn save(&mut self, world: &World, file: &Path) -> Result<(), AppError> {
        let _ = (world, file);
        Err("the game does not save its runs".into())
    }

    /// Loads the run of the save in `file` in place of the run in `world`, as a game resumes its
    /// own saves: what a tool asks as for [`save`](Self::save). A save that the game does not
    /// resume leaves the run as it is.
    ///
    /// # Errors
    ///
    /// Why the save is not loaded: by default, that the game does not save its runs.
    fn load(&mut self, world: &mut World, file: &Path) -> Result<(), AppError> {
        let _ = (world, file);
        Err("the game does not save its runs".into())
    }
}

/// A game with no rules, as a program that only draws.
impl Simulation for () {}

/// What the player sees and hears of a game: the camera, the drawing, the dev overlay
/// (`docs/adr/0107-the-execution-model.md`). It reads the world, and never writes it: every method
/// receives it as `&World`, or not at all. A change of the world that it asks for, as the dev
/// overlay's edits, is a command ([`Context::send`]), which the host applies at the start of the
/// next frame. It keeps state of its own, its camera's, and runs on real time.
///
/// Every method has a default that does nothing. Only windowed runs call them; headless runs
/// ignore the presentation. [`run`](crate::run) calls:
///
/// 1. [`start`](Self::start), once, after the simulation's start, the window and the GPU open;
/// 2. for each frame, once the simulation ran its phases and the host computed the world's
///    transforms, [`extract`](Self::extract), which reads the world and makes what the frame draws
///    (phase 5), then, when the window can show a new image, [`render`](Self::render), which draws
///    it without the world (phase 6): the two calls of `docs/adr/0111-the-frame-contract.md`;
/// 3. [`stop`](Self::stop), once, after the last frame, before the simulation's stop, if the run
///    started.
///
/// Between frames, it receives each event of the window ([`on_event`](Self::on_event)), and the
/// loss of the GPU device ([`gpu_lost`](Self::gpu_lost), [`gpu_restored`](Self::gpu_restored)).
///
/// A presentation that writes the world does not compile:
///
/// ```compile_fail,E0596
/// use slime_app::{Context, Presentation};
/// use slime_world::World;
///
/// struct Painter;
///
/// impl Presentation for Painter {
///     fn extract(&mut self, _ctx: &mut Context, world: &World) {
///         world.spawn();
///     }
/// }
/// ```
pub trait Presentation {
    /// Called once, after the simulation's [`Simulation::start`], when the window and the GPU are
    /// open ([`Context::window`], [`Context::gpu`]): where a presentation creates its GPU objects,
    /// for targets of [`Context::surface_format`], from the world as the simulation started it.
    ///
    /// # Errors
    ///
    /// A presentation returns an error when it cannot start, for example because a shader does not
    /// compile: no frame runs, neither `stop` is called, and [`run`](crate::run) returns
    /// [`RunError::Init`](crate::RunError::Init) with this error.
    fn start(&mut self, ctx: &mut Context, world: &World) -> Result<(), AppError> {
        let _ = (ctx, world);
        Ok(())
    }

    /// Called for each event of the window, between frames, before the host's input follows it
    /// ([`Input::handle`](slime_platform::Input::handle)). Returns whether the presentation takes
    /// the event for itself, as a dev overlay does with the keys typed into a field and the clicks
    /// on a panel: the simulation's input then does not see a key or a button that it puts down. A
    /// key or a button let go always reaches the input, so that none stays held. The default takes
    /// none.
    ///
    /// When the event is [`Event::CloseRequested`], the run stops after this call, as after
    /// [`Context::request_exit`].
    fn on_event(&mut self, ctx: &mut Context, event: &Event) -> bool {
        let _ = (ctx, event);
        false
    }

    /// Called once per frame, at phase 5, after the simulation's phases and the world's transforms,
    /// before the window's image is acquired: reads the world, read only, and makes what
    /// [`render`](Self::render) draws, as `slime-world-render`'s frame builder does. It runs in
    /// frames that the window cannot show too, which `render` then does not draw. What moved during
    /// a fixed step is drawn at [`GameTime::alpha`](crate::time::GameTime::alpha) of the way
    /// between its last two steps.
    fn extract(&mut self, ctx: &mut Context, world: &World) {
        let _ = (ctx, world);
    }

    /// Called once per frame that the window can show, at phase 6, after
    /// [`extract`](Self::extract): draws what it made into `target`, the window's image, with the
    /// GPU of [`Context::gpu`], without the world. The host shows the image once this method
    /// returns.
    ///
    /// It does not run in frames where the window cannot show a new image: while it has a size
    /// of zero (minimized), or when its next image is not ready in time on Vulkan (see
    /// [`Surface::acquire`](slime_gfx::Surface::acquire); Direct3D 12 waits for it instead, and on
    /// Windows with Vulkan, wgpu treats a wait of more than one second as a lost device, which
    /// stops the run). The simulation's phases of such frames still run. A frame that draws nothing
    /// into `target`, as this default does, shows it cleared to black.
    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let _ = (ctx, target);
    }

    /// The views that the last [`render`](Self::render) drew into the window's image, each by its
    /// name and its rectangle, through which a tool captures the part of the image that one draws
    /// (the development channel's capture, ADR-0126). A rectangle lies inside the image. The
    /// default names none: a tool then captures the whole image.
    fn views(&self) -> Vec<ShownView> {
        Vec::new()
    }

    /// The settings of the view that the last [`render`](Self::render) drew under `name`, one of
    /// [`views`](Self::views), as reflection describes them: what a tool reads and changes through
    /// the development channel, as the renderer's `ViewSettings` for a view of `slime-render`
    /// (ADR-0127). `None` for a view whose settings a tool does not change, as the default gives
    /// for every view.
    fn view_settings(&self, name: &str) -> Option<Reflected> {
        let _ = name;
        None
    }

    /// Gives the view named `name` the settings that a tool changed: those that
    /// [`view_settings`](Self::view_settings) gave, a part of them changed, which reflection
    /// checked against their type and the ranges of their fields. The view draws with them from
    /// the next frame, as its renderer applies them: `slime-render` makes a view's targets at a new
    /// render scale once the size has held still.
    ///
    /// # Errors
    ///
    /// Why the presentation refuses them, which the tool is told, as the default does for every
    /// view.
    fn set_view_settings(&mut self, name: &str, settings: &Reflected) -> Result<(), String> {
        let _ = settings;
        Err(format!(
            "the game does not change the settings of its view `{name}`"
        ))
    }

    /// The reports that the presentation gives tools, each by its name and described by
    /// reflection, which the development channel lists in the game's state and reads one by one
    /// (`docs/adr/0128-asset-requests.md`): as the renderer's `CacheReport` of an asset cache,
    /// under `assets`, and its views' `RenderStats`, under `views`. They are made when a tool asks,
    /// not at each frame. The default gives none.
    fn reports(&self) -> Vec<(String, Reflected)> {
        Vec::new()
    }

    /// Picks, for a tool, the entity drawn at pixel `(x, y)` of the view that the last
    /// [`render`](Self::render) drew under `view`, one of [`views`](Self::views), counted from its
    /// rectangle's top left corner, within it (the development channel's picking,
    /// `docs/adr/0130-picking.md`): gives it to `answer` once it knows it, as a view of
    /// `slime-render` answers its picking, a frame or more later. `answer` dropped without an
    /// answer tells the tool that none came, as a lost GPU device leaves it.
    ///
    /// # Errors
    ///
    /// Why it cannot pick there, which the tool is told, as the default does for every view.
    fn pick(&mut self, view: &str, (x, y): (u32, u32), answer: PickAnswer) -> Result<(), String> {
        let _ = (x, y, answer);
        Err(format!("the game does not pick in its view `{view}`"))
    }

    /// Called between frames when the GPU device was lost and the run is about to open a new one,
    /// as a game asks with
    /// [`AppConfig::restore_lost_device`](crate::AppConfig::restore_lost_device): the presentation
    /// drops every GPU object it made with the lost device, which [`Context::gpu`] still is, and
    /// every clone of that `Gpu`, its loads in progress included, whatever their state. The run
    /// then releases its own and opens the new device, which Direct3D 12 does only once nothing
    /// holds the lost one (ADR-0075). Objects kept past this call make the new device open on
    /// another adapter, if any, and panic if used with it. The default does nothing, which suits a
    /// presentation that keeps no GPU object between frames.
    fn gpu_lost(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }

    /// Called between frames once the run opened a new GPU device after
    /// [`gpu_lost`](Self::gpu_lost): [`Context::gpu`] is the new device, and
    /// [`Context::surface_format`] the format of the window's images, which another adapter may
    /// change. The presentation creates its GPU objects again here, as in [`start`](Self::start),
    /// from the world as it is. The default does nothing.
    ///
    /// The run does not restore, and stops as without the setting, once it has opened three devices
    /// within five minutes, as a device lost again soon after each new one, when the device was
    /// lost because memory ran out, when only a CPU adapter is left where the lost device was a
    /// GPU's, or when no device opens ([`Surface::restore`](slime_gfx::Surface::restore)); nor when
    /// the run is stopping anyway, or the window's surface was lost for good. When no device opens
    /// after `gpu_lost`, [`Context::gpu`] is `None` from then on, `stop` included.
    ///
    /// # Errors
    ///
    /// A presentation returns an error when it cannot go on with the new device. The run then stops
    /// as after [`Context::exit_with_error`], and [`run`](crate::run) returns
    /// [`RunError::App`](crate::RunError::App) with this error; but for the loss of the new device,
    /// which the run handles as any other loss (`exit_with_error` says how), opening another device
    /// within the limit above.
    fn gpu_restored(&mut self, ctx: &mut Context, world: &World) -> Result<(), AppError> {
        let _ = (ctx, world);
        Ok(())
    }

    /// Called once, after the last frame, however the run stopped, before the simulation's
    /// [`Simulation::stop`], unless the run did not start or the process ends before it stops (see
    /// [`run`](crate::run)). The window and the GPU still exist, but for a GPU lost that no new one
    /// replaced after [`gpu_lost`](Self::gpu_lost); a presentation that keeps a clone of the window
    /// should drop it here.
    fn stop(&mut self, ctx: &mut Context) {
        let _ = ctx;
    }
}

/// A game that shows nothing, as a program that runs only its rules.
impl Presentation for () {}

/// A view that a presentation drew into the window's image ([`Presentation::views`]): its name, and
/// its rectangle, in pixels from the image's top left corner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownView {
    /// What a tool names it by: `map`.
    pub name: String,
    /// The column of its left edge.
    pub x: u32,
    /// The row of its top edge.
    pub y: u32,
    /// Its width, at least 1.
    pub width: u32,
    /// Its height, at least 1.
    pub height: u32,
}

/// Where a presentation gives the answer of a picking that a tool asked for
/// ([`Presentation::pick`]): the entity drawn at the pixel, or none, once the presentation knows
/// it. Dropped without an answer, it tells the tool that none came.
#[derive(Debug)]
pub struct PickAnswer {
    slot: PickSlot,
}

/// Where a [`PickAnswer`] puts its answer: `None` until given, then the entity found, or none.
pub(crate) type PickSlot = Arc<Mutex<Option<Option<Entity>>>>;

impl PickAnswer {
    /// An answer to give, and where the development channel finds it once given.
    #[cfg_attr(
        not(feature = "development"),
        expect(dead_code, reason = "only the development channel asks for pickings")
    )]
    pub(crate) fn new() -> (Self, PickSlot) {
        let slot = Arc::new(Mutex::new(None));
        (
            Self {
                slot: Arc::clone(&slot),
            },
            slot,
        )
    }

    /// Gives `entity`, the one drawn at the pixel, or `None` where none is.
    pub fn give(self, entity: Option<Entity>) {
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(entity);
    }
}

/// A value described by reflection, with the registry of its type: a view's settings
/// ([`Presentation::view_settings`]), or a report ([`Presentation::reports`]). Through it, a tool
/// reads a presentation's values without `slime-app` knowing their types, and changes a view's
/// settings, its changes checked against the type and the ranges of its fields before the
/// presentation gets them back ([`Presentation::set_view_settings`]).
pub struct Reflected {
    /// Describes its type and the types it holds, through which the development channel reads and
    /// changes it.
    #[cfg_attr(
        not(feature = "development"),
        expect(
            dead_code,
            reason = "the development channel alone reads them through it"
        )
    )]
    pub(crate) registry: Registry,
    /// Its type's name.
    pub(crate) ty: String,
    /// The value.
    pub(crate) value: Value,
}

impl fmt::Debug for Reflected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reflected")
            .field("ty", &self.ty)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}

impl Reflected {
    /// `value`, described by its type's reflection.
    #[must_use]
    pub fn new<T: Reflect>(value: &T) -> Self {
        let mut registry = Registry::new();
        registry.register::<T>();
        Self {
            registry,
            ty: T::type_name(),
            value: value.to_value(),
        }
    }

    /// The value, as a value of `T`.
    ///
    /// # Errors
    ///
    /// When it is not a value of type `T`, with the part at fault.
    pub fn get<T: Reflect>(&self) -> Result<T, ReflectError> {
        T::from_value(&self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_simulation_that_does_not_save_its_runs_refuses_to_save_or_load_one() {
        let mut world = World::new();
        let file = Path::new("run.toml");
        for refused in [().save(&world, file), ().load(&mut world, file)] {
            let refused = refused.expect_err("refused");
            assert_eq!(refused.to_string(), "the game does not save its runs");
        }
        assert!(!file.exists(), "nothing written");
    }
}
