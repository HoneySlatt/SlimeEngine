# slime-app

Running a game: its rules as a simulation and what the player sees as a presentation, run by a
host through the phases of each frame, on real time and on game time; the main loop, starting and
stopping.

## Status

Games run headless or with one window that receives keyboard and mouse input and that they draw
into with the GPU; the phases of a frame, game time that pauses, slows down and steps, the input
as data, commands applied at the start of a frame, the main loop, exit requests, errors at
startup, clean shutdown on Ctrl+C and other termination signals, profiling spans and diagnostics
work; a windowed run that asks for it goes on with a new GPU device after a lost one; a run
built with the `development` feature and started with a token answers the tools that drive it
through the development channel; a run built with the `accessibility` feature gives its
presentation's interface to screen readers. What is
missing is listed in `docs/PLAN.md` (known debt).

## Role

- Runs a game as a *simulation*, its rules, and a *presentation*, what the player sees, through the
  phases of a frame, each of which says who may change the world
  (`docs/adr/0107-the-execution-model.md`,
  `docs/adr/0121-a-game-as-a-simulation-and-a-presentation.md`): the simulation changes it, the
  presentation reads it, and asks for changes as commands, which the host applies at the start of
  the next frame.
- Owns the game's world (`slime-world`), its real time and its game time, its input and the
  commands queued for it: the *host*. Tells the world before and after each fixed step, so that
  what moves during a step is drawn between its two transforms
  (`docs/adr/0084-what-the-world-draws.md`).
- Runs with or without a window. Running without one (headless) is a normal configuration, which
  automated tests use, and dedicated servers could; a test drives a host itself, without a GPU.
- With a window, opens the GPU that draws into it, and gives each frame an image of the window to
  draw into, which it then shows.
- With its `development` feature, which a game turns on by default and a build for shipping leaves
  out, answers the tools that drive a run started with a token, through the development channel
  (`slime-channel`, `docs/adr/0122-the-development-channel.md`).
- With its `accessibility` feature, which the examples turn on, and the editor once it has a window,
  and games leave out until Stage 7, gives screen readers what the presentation's interface means,
  through AccessKit's adapter of the window (`docs/adr/0146-screen-readers.md`).
- Sets up the engine's diagnostics.

## Entry points

A game implements `Simulation`, its rules, and hands it to `run` with its `Presentation`, or `()`
for none:

```rust
use slime_app::{AppConfig, Simulation, SimulationContext};
use slime_world::World;

#[derive(Default)]
struct Game {
    fixed_steps: u32,
}

impl Simulation for Game {
    fn fixed_step(&mut self, _ctx: &mut SimulationContext<'_>, _world: &mut World) {
        self.fixed_steps += 1;
    }

    fn update(&mut self, ctx: &mut SimulationContext<'_>, _world: &mut World) {
        if ctx.time().real().elapsed().as_secs() >= 1 {
            ctx.request_exit();
        }
    }
}

let mut game = Game::default();
// The default configuration runs headless, and never calls the presentation; set `window` to open
// one.
slime_app::run(&mut game, &mut (), &AppConfig::default()).expect("this game starts");
assert_eq!(game.fixed_steps, 60);
```

With a window, the presentation draws each frame. This one clears the window to a color; the
`window` example (`examples/window.rs`) also creates GPU objects in `start` and draws a triangle:

```rust,no_run
use slime_app::{AppConfig, Context, Presentation};
use slime_color::LinearColor;
use slime_gfx::{RenderPassDescriptor, RenderTarget};
use slime_platform::WindowConfig;

struct Sky;

impl Presentation for Sky {
    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let gpu = ctx.gpu().expect("windowed runs have a GPU");
        let sky = LinearColor::rgb(0.1, 0.3, 0.8);
        let pass = RenderPassDescriptor {
            target,
            clear: Some(sky),
            depth: None,
            timer: None,
            resolve: None,
        };
        gpu.render(&pass, |_pass| {});
    }
}

let config = AppConfig {
    window: Some(WindowConfig::default()),
    ..AppConfig::default()
};
// A failed run has been logged, with each of its causes: the caller only reports the failure.
if slime_app::run(&mut (), &mut Sky, &config).is_err() {
    std::process::exit(1);
}
```

- `Simulation`: a game's rules, which change its world on game time from the input given as data,
  without a GPU or a window: `start` once, then for each frame `fixed_step` once per fixed step
  of game time due and `update` once, then `stop` once. Each receives the world, to change, and a
  `SimulationContext`: the input (`slime_platform::Input`, the keys and mouse buttons held and
  those pressed and released since the last frame), both times, and the run's stop: it asks to
  stop (`request_exit`), or fails the run (`exit_with_error`), and its host decides what stopping
  means. A game that saves its runs implements `save` and `load` too, to a file given, which the
  development channel asks for (`docs/adr/0125-saves-per-scene.md`); by default they refuse.
  `apply` applies each command that the host receives, from the presentation, the dev overlay or
  a tool, at the start of a frame: by default to the world, while the editor makes steps of them;
  `edit`, with the `development` feature, answers an editor's requests, which a game leaves to its
  default refusal (`docs/adr/0135-the-editors-program.md`).
- `Presentation`: what the player sees, which reads the world and never writes it: every method
  receives it as `&World` or not at all, so that a presentation that writes the world does not
  compile. `start` once the window and the GPU are open; `on_event` for each event of the window
  that its interface did not take, which returns whether the presentation takes it for itself, as
  a dev overlay takes the keys typed into a field: the simulation's input then does not see a key
  or a button put down, and always sees one let go; for each frame, `extract`, which reads the
  world and makes what the frame draws, then, when the window can show a new image, `render`,
  which draws it into the window's image without the world
  (`docs/adr/0111-the-frame-contract.md`), and `views`, which
  names the views that it drew there, each with its rectangle (`ShownView`), so that a tool
  captures one through the development channel (`docs/adr/0126-views-apart-from-the-renderer.md`),
  with `view_settings` and `set_view_settings`, which hand a tool a view's settings as reflection
  describes them (`Reflected`), and take them back changed
  (`docs/adr/0127-the-settings-of-a-view.md`), and `reports`, the named values, as described, that
  it gives tools, an asset cache's and its views' statistics
  (`docs/adr/0128-asset-requests.md`), and `pick`, which picks the entity at a pixel of a view for a
  tool and gives it to a `PickAnswer` once it knows it (`docs/adr/0130-picking.md`), and `ui`,
  its interface, if it has one, which a tool reads, acts on and captures element by element
  (`docs/adr/0143-what-agents-see-of-the-interface.md`), and which the run connects to the
  window: each event goes to the interface first, and to `on_event` and the game's input only
  when the interface did not take it; the interface gets the system's clipboard, before each
  frame the time since the last, by which its transitions go on
  (`docs/adr/0149-the-looks-foundations.md`), and after each frame the window shows the pointer's
  shape that it asks and allows the input method while one
  of its fields takes text (`docs/adr/0145-text-edited.md`), and, with the `accessibility`
  feature, screen readers get what changed in what it means;
  `gpu_lost` and `gpu_restored` when the run replaces a lost GPU device, which a game asks for with
  `AppConfig::restore_lost_device`: it drops its GPU objects in the first, before the new device
  opens, and creates them again in the second
  (`docs/adr/0075-recovering-from-a-lost-gpu-device.md`); `stop` once. Only windowed runs call
  it.
  - A `start` can fail: it returns `Result<(), AppError>`, where `AppError` is any boxed error (`?`
    converts errors into it). After a failed `start`, no frame runs, neither `stop` is called and
    `run` returns the error. `gpu_restored` can fail too, which stops the run with the error. The
    other methods return nothing: one that hits an error it cannot handle passes it to
    `exit_with_error`, which stops the run as an exit request does, both `stop` methods included,
    and makes `run` return it (`docs/adr/0043-application-errors-during-a-run.md`).
  - `render(ctx, target)` is not called in frames where the window cannot show a new image: while
    it is minimized, or when its next image is not ready in time on Vulkan (on Direct3D 12, the
    engine waits up to one second for it, then draws the frame late; on Windows with Vulkan, wgpu
    treats a wait of more than one second as a lost device, and the run stops).
- `Host`: what runs a simulation, and owns its world, real time and game time, its input and the
  commands queued for it. `Host::new(&config)` makes one around an empty world (`Host::world_mut`),
  and `Host::start` and `Host::stop` call the simulation's. `Host::frame(&mut simulation, real)`
  runs the first four phases of a frame: the commands applied, in the order they came; the fixed
  steps that game time holds; the update; the world's transforms. A test, or one day a dedicated
  server, drives it itself: it presses and releases keys (`Host::input_mut`), pauses, slows down and
  steps game time and sends commands (`Host::context_mut`), without a GPU.
- `run(&mut simulation, &mut presentation, &config)`: the host of a game's program. It sets up
  diagnostics and a handler for termination signals, then runs the game until it requests an exit,
  the window is closed, the process receives SIGINT (Ctrl+C), SIGTERM or SIGHUP, or
  `AppConfig::frame_limit` is reached. In every case the frame in progress finishes and both `stop`
  methods run, the presentation's first; a second signal, half a second or more after the first,
  ends the process at once (exit code 130) for a game that no longer finishes its frames; one that
  comes sooner, as when a closing terminal sends SIGHUP twice, is ignored
  (`docs/adr/0012-clean-shutdown-on-termination-signals.md`).
  - Headless: frames run back to back with simulated time: each frame represents one fixed step,
    so runs are deterministic. Neither the windowing system nor the GPU is touched, and the
    presentation is not called. `AppConfig::pace_headless` schedules them at
    `AppConfig::max_frame_rate` instead, the loop sleeping in between, as the editor's program,
    which waits for its tools, asks (`docs/adr/0135-the-editors-program.md`).
  - With a window, `run` must be called from the main thread (it panics otherwise). The window and
    the GPU exist before the `start` methods. Frames represent the real time between them. The
    engine asks for a frame on average at most `AppConfig::max_frame_rate` times per second (60 by
    default) and sleeps in between; a frame runs each time the windowing system lets the window
    draw, which also happens on its own initiative, for example during a resize
    (`docs/adr/0020-rendering-in-the-main-loop.md`). The images are shown as
    `AppConfig::present_mode` says: by default, each one replaces the image waiting to be shown,
    so the display never makes a frame wait, and the schedule alone paces the frames, hidden window
    included (`docs/adr/0021-frame-pacing.md`). With `PresentMode::Fifo`, every image is shown
    in step with the display, which can block a frame: up to one second when the compositor
    rarely shows a hidden window.
  - It returns a `RunError`: `Init` when a `start` fails, `App` when `gpu_restored` failed or the
    game passed an error to `exit_with_error`, `Platform` when the window cannot be opened (for
    example without a display server) or its event loop fails, `Gfx` when no GPU can draw into the
    window or when the GPU device or the window's surface is lost for good during the run. After a
    loss, the run stops like after an exit request, both `stop` methods included
    (`docs/adr/0022-losing-the-gpu-device.md`, `docs/adr/0025-giving-up-a-lost-surface.md`),
    unless the game set `AppConfig::restore_lost_device`: the run then opens a new device and goes
    on. It gives up at a loss that comes after three new devices within five minutes, and when no
    device should or can replace the lost one; it does not try when it is stopping anyway. A loss
    that the presentation meets, an error of a lost device that it passes to
    `Context::exit_with_error` or returns from `gpu_restored`, is the run's to handle, as one the
    run found (`docs/adr/0103-a-lost-device-is-recovered-wherever-the-loss-is-found.md`). Only
    one windowed run, or attempt, is possible per process. A failed run is logged at the `error`
    level with each of its causes (`the run failed: cannot draw into the window: no GPU adapter is
    available: ...`), which the error's own message, the most general, does not give.
  - The signal handler replaces any other handler the process had for these signals, except one
    installed earlier through the `ctrlc` crate, which is kept. It stays installed after `run`
    returns: a signal then no longer ends the process by itself, and only the second one received
    in the process ends it (exit code 130).
  - On Windows, Ctrl+C and Ctrl+Break in a console stop the run the same way; closing the console
    window ends the process at once, without the `stop` methods, and logging off and shutting down
    end it without them too.
- `AppConfig`: the window (`None`, the default, runs headless), frame limit, fixed step
  (1/60 s), maximum fixed steps per frame (8), maximum frame delta (250 ms), maximum frame rate
  (60), present mode (`PresentMode::Mailbox`) and whether a windowed run replaces a lost GPU device
  (`restore_lost_device`, `false`). Write it as a struct literal ending with
  `..AppConfig::default()`.
- `Context`: what the presentation, and whoever drives a host, can ask from the engine: the window
  (`Context::window`), the GPU that draws into it (`Context::gpu`) and the format of its images
  (`Context::surface_format`), all `None` when headless; both times (`Context::time`); game time's
  speed, pause and one step (`set_speed`, `pause`, `resume`, `step`), applied from the next frame,
  and its hold, which a game sets while it is not ready to be played, apart from the pause
  (`hold`, `held`);
  the commands (`Context::send`), and those that the world refused at this frame's start
  (`Context::refused`); stopping the run (`request_exit`), or failing it (`exit_with_error`); and
  whether it is going to stop (`exit_requested`), which reports a termination signal as soon as it
  arrives, even during a `start` or in the middle of a frame.
- `time`: `Time` holds a frame's real time, `FrameTime` (index, delta, elapsed time), and its game
  time, `GameTime`: the fixed steps run, the game time the frame added, and the fraction of a step
  accumulated but not run (`alpha`), at which the world draws what moved during the last step
  between its last two transforms (`World::interpolated_transform`). Game time advances at a
  speed, 1 by default, not at all while paused, and its fixed steps keep their duration at every
  speed, so that a simulation gives the same results at every speed. `FrameClock` turns the time
  between frames into a `FrameTime` and limits how much time one frame can represent; `FixedStep`
  splits time into steps of a fixed duration. Both take durations rather than reading a clock
  (`docs/adr/0009-frame-timing.md`).
- `init_diagnostics()`: installs the log subscriber (output on standard error, filtered by the
  `SLIME_LOG` environment variable; events that cannot be written because standard error is gone
  are dropped) and a panic hook that logs panics, then logs the engine version, operating system,
  CPU architecture and number of logical CPUs. `run` calls it; call it
  yourself only in a program that does not use `run`. Later calls do nothing. `LOG_ENV_VAR` is the
  name of that environment variable. When it is unset, `DEFAULT_LOG_FILTER` applies: `info` and
  above, except for wgpu-hal's details about the GPU, the Vulkan loader's messages, the error
  that winit's client-side decorations log when no desktop portal answers, and the spans of the
  D-Bus library through which screen readers are served on Linux
  (`docs/adr/0019-default-log-filter.md`). With the `SLIME_TRACE` environment variable
  (`TRACE_ENV_VAR`) naming a file, it also writes there a trace of every span of every thread,
  whatever `SLIME_LOG` keeps, in the Chrome Trace Event format that Perfetto and `chrome://tracing`
  open (`docs/adr/0104-the-build-ci-and-measurements.md`): the phases of each frame and each
  submission to the GPU, not yet the renderer's passes.
- `error_chain(&error)`: an error's message followed by the message of each of its causes
  (`error: cause: cause of cause`), the line `run` logs for a failed run. A program that does not
  use `run`, as the `offscreen` example, logs its errors with it.
- `data_directory()`: the user's data directory, where a program keeps the files that it writes for
  the user, a game's saves or an editor's journals. `HeldFile::hold(path)` holds a file for this
  program alone, through the lock of a file beside it, which the system lets go however the program
  ends: one copy of a game writes its save, one editor its journal. `set_aside(file, why)` renames a
  file that cannot be used, beside it, as `<file>.<why>`, or `<file>.<why>.2` and on, never over a
  file set aside before: a save that a game cannot read, a journal that an editor discards
  (`docs/adr/0135-the-editors-program.md`).
- The `development` feature: a run whose environment gives a token (`slime_channel::TOKEN_ENV_VAR`)
  listens on `127.0.0.1`, writes where on its standard output, and answers, at the start of each
  frame, the requests of the tools that connect with the token: its state, the host's controls, its
  world read and changed through reflection, keys put down and let go, its window's image or the
  part of it that one of its views draws, copied after the frame is drawn and before it is shown,
  its views' settings read and changed, its reports read, the entity at a pixel of a view picked,
  and the elements of its interface listed by role and name, found at a pixel, acted on as a
  person's input acts and captured, through the presentation's `views`, `view_settings`,
  `set_view_settings`, `reports`, `pick` and `ui`,
  the last lines of its log (`slime_channel::MAX_LOG_LINES`), which only such a run keeps, and its
  run saved to a file and loaded from one, by the simulation's `save` and `load`, and an editor's
  requests, by the simulation's `edit`. Without a token,
  it does not listen. The `slime-channel` crate holds the protocol and its limits;
  `tests/channel.rs` drives a headless run through it.
- The `accessibility` feature: a windowed run gives screen readers its window's tree, the window's
  node holding what the presentation's interface means (`slime_ui::tree::Ui::take_meaning_changes`),
  translated into AccessKit's tree, whole when a screen reader asks for it and what changed after
  each frame from then on, while one reads the window; it gives the interface what a screen reader
  asks, a click, the focus, a value or a selection set, a scroll, through its calls, as a person's
  input. Its translation's tests, in `src/accessibility.rs`, run with the feature (`cargo test -p
  slime-app --features accessibility`), which a workspace's run of the tests turns on through the
  examples.

Engine code and games log through the `tracing` macros. The conventions for errors, log levels and
assertions are in `docs/adr/0008-errors-logging-and-assertions.md`; why the engine owns the main
loop is in `docs/adr/0010-main-loop-ownership.md`. With `SLIME_LOG=slime_app=trace,slime_gfx=trace`,
each frame's profiling spans are logged with the time spent in them: `frame`, containing
`fixed_step`, `update`, and with a window `extract`, `acquire` (waiting for the window's image),
`render` (with `submit`, recorded by `slime-gfx`) and `present`. Each line is written to standard
error when its span closes, while the enclosing span is still open, so a span's time includes
writing the lines of the spans it contains; with `slime_gfx=trace`, the renderer's
`RenderStats::cpu_time` likewise includes the `submit` lines of its submissions. Take timing
baselines with `SLIME_LOG` unset, and use the span lines, or a trace (`SLIME_TRACE`), which writes
no line, for where the time goes. A bare
`SLIME_LOG=trace` logs them too, but also turns on the trace logs of wgpu and of its shader
compiler, among them a line for each command of each draw, written while a pass is encoded: they
flood standard error, and the time spent writing them counts in the spans. In GPU capture tools
such as RenderDoc, the render passes that draw the window's image of each frame, and their command
buffers, are named `window, frame <index>`, the index of the `frame` span; the image itself keeps
the name the graphics API gives it.

## Dependencies

- `ctrlc`: handles termination signals.
- `nu-ansi-term`, on Windows only: turns on the colors of the log in the console.
- `slime-gfx`: the GPU and the window's surface, for windowed runs.
- `slime-platform`: the window and its events, for windowed runs, and the input as data.
- `slime-world`: the game's world, which it owns, and its commands.
- `slime-reflect`: the settings and the reports of a presentation, described to the tools that
  read and change them; the world depends on it in every build.
- `slime-ui`: the interface that a presentation gives, which the run connects to the window and
  the development channel reads and acts on; it needs no window nor GPU.
- `slime-channel` and `slime-serialize`, with the `development` feature only: the development
  channel, and the documents through which it reads and writes values as text.
- `accesskit`, with the `accessibility` feature only: the tree that screen readers read, which it
  translates the interface's meaning into; `slime-platform`'s feature of the same name holds
  AccessKit's adapter. Its tests check the translation with `accesskit_consumer`, the tree that
  AccessKit's adapters build.
- `thiserror`: derives `RunError`'s `Display` and `Error` implementations.
- `tracing`: the logging macros and profiling spans.
- `tracing-subscriber`: the subscriber that formats, filters and writes log events, including the
  records that dependencies such as wgpu emit through the `log` crate (feature `tracing-log`).

## Must not

- Use the windowing library or wgpu directly: windows and input go through `slime-platform`, the
  GPU through `slime-gfx`.
- Touch the windowing system or the GPU when it runs headless.
