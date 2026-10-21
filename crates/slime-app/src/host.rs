//! The host: what runs a game's simulation, and owns its world, its times, its input and the
//! commands queued for it (`docs/adr/0107-the-execution-model.md`).

use std::time::Duration;

use slime_gfx::{GfxError, Gpu, TextureFormat};
use slime_platform::{Input, Window};
use slime_world::{Command, World, WorldError};

use crate::app::AppConfig;
use crate::error::AppError;
use crate::game::Simulation;
use crate::signal;
use crate::time::{FixedStep, FrameClock, GameClock, Time};

/// What runs a game's [`Simulation`] (`docs/adr/0107-the-execution-model.md`): it owns the world,
/// real time and game time, the input and the commands queued, and runs the first four phases of
/// each frame. The game's program runs it through [`run`](crate::run), which adds the window, the
/// GPU and the presentation; a test, and one day a dedicated server, drive it themselves: they
/// press and release its keys ([`Host::input_mut`]), pause and step it, send it commands
/// ([`Host::context_mut`]), and run its frames ([`Host::frame`]), without a GPU.
///
/// A frame ([`Host::frame`]) runs, in this order:
///
/// 1. *start*: the host applies the commands queued since the last frame ([`Context::send`]), in
///    the order they came; the input holds what came since the last frame;
/// 2. *fixed steps*: the simulation's [`Simulation::fixed_step`], once per fixed step of game time
///    due, the world told before and after each ([`World::begin_fixed_step`],
///    [`World::end_fixed_step`]);
/// 3. *update*: the simulation's [`Simulation::update`];
/// 4. *transforms*: the world's global transforms computed ([`World::update_transforms`]), so that
///    the presentation reads them at the cost of a lookup.
///
/// It then ends the input's frame ([`Input::end_frame`]). [`run`](crate::run) adds phases 5 and 6,
/// the presentation's extraction and rendering, which read the world without writing it.
pub struct Host {
    pub(crate) world: World,
    input: Input,
    pub(crate) context: Context,
    clock: FrameClock,
}

impl Host {
    /// A host of an empty world, whose frames and fixed steps follow `config`: its fixed step, its
    /// most fixed steps per frame and its longest frame. Game time runs at a speed of 1.
    ///
    /// # Panics
    ///
    /// If `config.fixed_step`, `config.max_fixed_steps_per_frame` or `config.max_frame_delta` is
    /// zero.
    pub fn new(config: &AppConfig) -> Self {
        let fixed = FixedStep::new(config.fixed_step, config.max_fixed_steps_per_frame);
        Self {
            world: World::new(),
            input: Input::default(),
            context: Context::new(GameClock::new(fixed)),
            clock: FrameClock::new(config.max_frame_delta),
        }
    }

    /// The game's world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// The game's world, which the host changes between frames, as at the start of one.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// The input that the simulation reads.
    pub fn input(&self) -> &Input {
        &self.input
    }

    /// The input, whose keys and buttons a host without a window, such as a test, presses and
    /// releases between frames.
    pub fn input_mut(&mut self) -> &mut Input {
        &mut self.input
    }

    /// The host's context: both times, game time's speed and pause, the commands, the run's stop.
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// The host's context, to pause, slow down and step game time, to send commands, or to stop.
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Starts `simulation`: its [`Simulation::start`].
    ///
    /// # Errors
    ///
    /// The simulation's, when it cannot start.
    pub fn start(&mut self, simulation: &mut impl Simulation) -> Result<(), AppError> {
        let (mut ctx, world) = self.simulation_context();
        simulation.start(&mut ctx, world)
    }

    /// Runs the first four phases of a frame of `simulation` that represents `real` time, limited
    /// to the longest frame: the commands queued, the fixed steps that game time holds, the update
    /// and the world's transforms, as [`Host`] says. Each fixed step is a `trace`-level profiling
    /// span named `fixed_step`, and the update with the transforms one named `update`.
    pub fn frame(&mut self, simulation: &mut impl Simulation, real: Duration) {
        self.context.time.real = self.clock.advance(real);
        self.apply_commands();
        let context = &mut self.context;
        let (steps, delta, alpha) = context.clock.advance(context.time.real.delta());
        let game = &mut context.time.game;
        (game.step, game.delta, game.alpha) = (context.clock.step(), delta, alpha);
        for _ in 0..steps {
            let _span = tracing::trace_span!("fixed_step").entered();
            self.world.begin_fixed_step();
            let (mut ctx, world) = self.simulation_context();
            simulation.fixed_step(&mut ctx, world);
            self.world.end_fixed_step();
            self.context.time.game.steps += 1;
        }
        {
            let _span = tracing::trace_span!("update").entered();
            let (mut ctx, world) = self.simulation_context();
            simulation.update(&mut ctx, world);
            // So that the presentation reads the transforms that the update changed at the cost of
            // a lookup.
            self.world.update_transforms();
        }
        self.input.end_frame();
    }

    /// Stops `simulation`: applies the commands queued since the last frame, which no frame will,
    /// then calls its [`Simulation::stop`], so that what it saves holds them.
    pub fn stop(&mut self, simulation: &mut impl Simulation) {
        self.apply_commands();
        let (mut ctx, world) = self.simulation_context();
        simulation.stop(&mut ctx, world);
    }

    /// Applies the commands queued since the last frame, in their order, and keeps those that the
    /// world refuses, with why, in place of the last frame's.
    fn apply_commands(&mut self) {
        let context = &mut self.context;
        context.refused.clear();
        for command in std::mem::take(&mut context.commands) {
            if let Err(error) = self.world.apply(&command) {
                tracing::warn!(?command, "a command is refused: {error}");
                context.refused.push((command, error));
            }
        }
    }

    /// What the simulation receives with the world.
    fn simulation_context(&mut self) -> (SimulationContext<'_>, &mut World) {
        let ctx = SimulationContext {
            input: &self.input,
            time: &self.context.time,
            exit: &mut self.context.exit,
        };
        (ctx, &mut self.world)
    }
}

/// What the host gives a [`Simulation`] with the world: the input, both times, and the run's stop.
/// It holds no GPU and no window.
pub struct SimulationContext<'a> {
    input: &'a Input,
    time: &'a Time,
    exit: &'a mut Exit,
}

impl SimulationContext<'_> {
    /// The keys and mouse buttons held, and those pressed and released since the last frame.
    pub fn input(&self) -> &Input {
        self.input
    }

    /// Real time and game time, as this point of the frame has them.
    pub fn time(&self) -> &Time {
        self.time
    }

    /// Asks the host to stop: [`Context::request_exit`].
    pub fn request_exit(&mut self) {
        self.exit.because(ExitReason::Requested);
    }

    /// Stops the run because of `error`: [`Context::exit_with_error`], for which a simulation, with
    /// no GPU, meets no loss of a device.
    pub fn exit_with_error(&mut self, error: impl Into<AppError>) {
        self.exit.fail(error.into());
    }

    /// Whether the run is going to stop: [`Context::exit_requested`].
    pub fn exit_requested(&self) -> bool {
        self.exit.requested()
    }
}

/// What the host gives a presentation, and whoever drives it: the window and the GPU when it has
/// them, both times, game time's speed and pause, the commands that change the world, and the
/// run's stop. The world is given apart, read only, to the presentation's methods that may read it.
#[derive(Debug)]
pub struct Context {
    window: Option<Window>,
    gpu: Option<Gpu>,
    surface_format: Option<TextureFormat>,
    exit: Exit,
    pub(crate) time: Time,
    clock: GameClock,
    /// The commands sent since the last frame's start.
    commands: Vec<Command>,
    /// The commands that the world refused at this frame's start, and why.
    refused: Vec<(Command, WorldError)>,
}

impl Context {
    fn new(clock: GameClock) -> Self {
        Self {
            window: None,
            gpu: None,
            surface_format: None,
            exit: Exit::default(),
            time: Time::default(),
            clock,
            commands: Vec::new(),
            refused: Vec::new(),
        }
    }

    /// Asks the engine to stop. The current frame finishes, then the presentation's and the
    /// simulation's `stop` are called, and [`run`](crate::run) returns. Asking more than once has
    /// no further effect.
    pub fn request_exit(&mut self) {
        self.exit.because(ExitReason::Requested);
    }

    /// Stops the run because of `error`, which [`run`](crate::run) then returns as
    /// [`RunError::App`](crate::RunError::App): this is how a game fails in a method that returns
    /// no error. The run stops as after [`request_exit`](Self::request_exit): the current frame
    /// finishes, then both `stop` methods are called. They can call it too, and so fail a run that
    /// stopped for another reason.
    ///
    /// Only the first error is returned; a later one is logged as an error. When the GPU device or
    /// the window's surface is lost as well, `run` returns the game's error, and the loss is in the
    /// log, where the engine writes it when it happens; but for an error that the loss causes.
    ///
    /// An error that is, or is caused by, the loss of the GPU device ([`GfxError::DeviceLost`]),
    /// once the device of [`Context::gpu`] is lost, does not stop the run: the loss is the run's to
    /// handle, as when the engine finds it. It opens a new device, as
    /// [`AppConfig::restore_lost_device`] asks, or stops with
    /// [`RunError::Gfx`](crate::RunError::Gfx). The error is logged at the `debug` level. An error
    /// that hides its cause, a message made of it, cannot be told from another one: it stops the
    /// run.
    pub fn exit_with_error(&mut self, error: impl Into<AppError>) {
        let error = error.into();
        if self.gpu().is_some_and(|gpu| gpu.check().is_err()) && is_device_lost(&*error) {
            tracing::debug!(
                "the application met the loss of the GPU device, which the run handles: {error}"
            );
            return;
        }
        self.exit.fail(error);
    }

    /// Whether the run is going to stop: the game requested an exit, the window was closed, the
    /// process received a termination signal, the GPU device was lost and not replaced
    /// ([`AppConfig::restore_lost_device`]), or the window's surface was lost for good.
    ///
    /// A termination signal shows here as soon as it is received, even during a `start` or in the
    /// middle of a frame, so that a long loading or a long update that checks it can stop early.
    /// The loss of the GPU device or of the surface is noticed only between frames.
    pub fn exit_requested(&self) -> bool {
        self.exit.requested()
    }

    /// Real time and game time, as the frame's start set them.
    pub fn time(&self) -> &Time {
        &self.time
    }

    /// The speed of game time, 1 at the start: how much game time a second of real time adds,
    /// while game time is not paused.
    pub fn speed(&self) -> f64 {
        self.clock.speed()
    }

    /// Sets the speed of game time from the next frame on: below 1 to slow the game down, above 1
    /// to hasten it. A fixed step keeps its duration, so that a simulation gives the same results
    /// at every speed: the speed changes how many steps run per second of real time, at most
    /// [`AppConfig::max_fixed_steps_per_frame`] in a frame.
    ///
    /// # Panics
    ///
    /// If `speed` is not a finite number above zero: [`pause`](Self::pause) stops game time.
    pub fn set_speed(&mut self, speed: f64) {
        self.clock.set_speed(speed);
    }

    /// Whether game time is paused.
    pub fn paused(&self) -> bool {
        self.clock.paused()
    }

    /// Pauses game time from the next frame on: no fixed step runs, but those asked for
    /// ([`step`](Self::step)). The frames go on, the simulation's update and the presentation
    /// included, on real time.
    pub fn pause(&mut self) {
        self.clock.set_paused(true);
    }

    /// Lets game time run again, at its speed, from the next frame on; the steps asked for and not
    /// run yet are dropped.
    pub fn resume(&mut self) {
        self.clock.set_paused(false);
    }

    /// Whether the program holds game time ([`hold`](Self::hold)).
    pub fn held(&self) -> bool {
        self.clock.held()
    }

    /// Holds game time from the next frame on, or lets it go, apart from the pause that the player
    /// and the tools set: held and not paused, game time stands still, as a program keeps it while
    /// it is not ready to be played, a zone that loads. A pause set meanwhile stays when the hold
    /// goes; steps asked for while paused run, held or not.
    pub fn hold(&mut self, held: bool) {
        self.clock.set_held(held);
    }

    /// While game time is paused, asks for one fixed step, which the next frame runs: as many steps
    /// as asked for, at most [`AppConfig::max_fixed_steps_per_frame`] per frame. While game time
    /// runs, it does nothing.
    pub fn step(&mut self) {
        self.clock.ask_steps(1);
    }

    /// While game time is paused, asks for `steps` more fixed steps, as many calls to
    /// [`step`](Self::step) would.
    #[cfg(feature = "development")]
    pub(crate) fn ask_steps(&mut self, steps: u32) {
        self.clock.ask_steps(steps);
    }

    /// The steps asked for while paused that no frame has run yet.
    #[cfg(feature = "development")]
    pub(crate) fn steps_asked(&self) -> u32 {
        self.clock.asked()
    }

    /// Queues `command`, which the host applies at the start of the next frame, after those sent
    /// before it, or before the simulation stops when no frame comes: how what reads the world
    /// without writing it, a presentation or a tool, changes it
    /// (`docs/adr/0107-the-execution-model.md`).
    pub fn send(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// The commands that the world refused at this frame's start, and why, in their order: each is
    /// logged as a warning too.
    pub fn refused(&self) -> &[(Command, WorldError)] {
        &self.refused
    }

    /// The window of the game, or `None` when it runs headless.
    pub fn window(&self) -> Option<&Window> {
        self.window.as_ref()
    }

    /// The GPU that draws into the window, or `None` when the game runs headless: headless runs
    /// open no GPU.
    pub fn gpu(&self) -> Option<&Gpu> {
        self.gpu.as_ref()
    }

    /// The format of the window's images, which
    /// [`Presentation::render`](crate::Presentation::render) draws into, or `None` when the game
    /// runs headless. Pipelines that draw into the window are created for it.
    pub fn surface_format(&self) -> Option<TextureFormat> {
        self.surface_format
    }

    pub(crate) fn set_window(&mut self, window: Option<Window>) {
        self.window = window;
    }

    pub(crate) fn set_gpu(&mut self, gpu: Option<Gpu>, surface_format: Option<TextureFormat>) {
        self.gpu = gpu;
        self.surface_format = surface_format;
    }

    /// Takes the GPU out, with the format of the window's images.
    pub(crate) fn take_gpu(&mut self) -> Option<Gpu> {
        self.surface_format = None;
        self.gpu.take()
    }

    /// The error passed to `exit_with_error`, if there is one.
    pub(crate) fn take_error(&mut self) -> Option<AppError> {
        self.exit.error.take()
    }

    /// Records why the run stops; the first reason is kept.
    pub(crate) fn exit_because(&mut self, reason: ExitReason) {
        self.exit.because(reason);
    }

    /// Why the run stops, if it does.
    pub(crate) fn exit_reason(&self) -> Option<ExitReason> {
        self.exit.reason
    }
}

/// Whether the run stops, why, and the error it fails with.
#[derive(Debug, Default)]
struct Exit {
    reason: Option<ExitReason>,
    /// The error passed to `exit_with_error`, which `run` returns.
    error: Option<AppError>,
}

impl Exit {
    fn because(&mut self, reason: ExitReason) {
        self.reason.get_or_insert(reason);
    }

    fn fail(&mut self, error: AppError) {
        if self.error.is_some() {
            tracing::error!("the application failed again: {error}");
        } else {
            self.error = Some(error);
        }
        self.because(ExitReason::Failed);
    }

    fn requested(&self) -> bool {
        self.reason.is_some() || signal::received()
    }
}

/// Whether `error`, or one of its causes, is the loss of a GPU device.
fn is_device_lost(error: &(dyn std::error::Error + 'static)) -> bool {
    std::iter::successors(Some(error), |error| error.source()).any(|error| {
        matches!(
            error.downcast_ref::<GfxError>(),
            Some(GfxError::DeviceLost { .. })
        )
    })
}

/// Why a run stops before reaching its frame limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExitReason {
    Requested,
    WindowClosed,
    Signal,
    DeviceLost,
    SurfaceLost,
    Failed,
}
