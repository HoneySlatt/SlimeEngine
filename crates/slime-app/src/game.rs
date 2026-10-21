//! A game as a simulation and a presentation (`docs/adr/0107-the-execution-model.md`): the two
//! traits that a game implements, which a host runs.

use slime_gfx::RenderTarget;
use slime_platform::Event;
use slime_world::World;

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
