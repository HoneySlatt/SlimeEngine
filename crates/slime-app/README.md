# slime-app

The application lifecycle: starting an engine application, running its main loop, and shutting it
down.

## Status

Applications run headless or with one window that receives keyboard and mouse input and that they
draw into with the GPU; the main loop, frame timing, exit requests, errors at startup, clean
shutdown on Ctrl+C and other termination signals, profiling spans and diagnostics work. What is
missing is listed in `docs/PLAN.md` (known debt).

## Role

- Starts an application, runs its frames, and shuts it down, with or without a window. Running
  without a window (headless) is a normal configuration, used by automated tests and later by
  dedicated servers.
- With a window, opens the GPU that draws into it, and gives each frame an image of the window to
  draw into, which it then shows.
- Sets up the engine's diagnostics and owns frame timing.

## Entry points

A game implements the `App` trait and hands it to `run`:

```rust
use std::time::Duration;
use slime_app::time::FrameTime;
use slime_app::{App, AppConfig, Context};

#[derive(Default)]
struct Game {
    fixed_steps: u32,
}

impl App for Game {
    fn fixed_update(&mut self, _ctx: &mut Context, _step: Duration) {
        self.fixed_steps += 1;
    }

    fn update(&mut self, ctx: &mut Context, frame: &FrameTime) {
        if frame.elapsed() >= Duration::from_secs(1) {
            ctx.request_exit();
        }
    }
}

let mut game = Game::default();
// The default configuration runs headless; set `window` to open one.
slime_app::run(&mut game, &AppConfig::default()).expect("this application starts");
assert_eq!(game.fixed_steps, 60);
```

With a window, `render` draws each frame. This one clears the window to a color; the `window`
example (`examples/window.rs`) also creates GPU objects in `init` and draws a triangle:

```rust,no_run
use slime_app::{App, AppConfig, Context};
use slime_gfx::{Color, RenderPassDescriptor, RenderTarget};
use slime_platform::WindowConfig;

struct Game;

impl App for Game {
    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        let gpu = ctx.gpu().expect("windowed runs have a GPU");
        let sky = Color { r: 0.1, g: 0.3, b: 0.8, a: 1.0 };
        let pass = RenderPassDescriptor { target, clear: Some(sky), depth: None, timer: None };
        gpu.render(&pass, |_pass| {});
    }
}

let config = AppConfig {
    window: Some(WindowConfig::default()),
    ..AppConfig::default()
};
if let Err(error) = slime_app::run(&mut Game, &config) {
    eprintln!("cannot run the game: {error}");
}
```

- `App`: the methods the engine calls, in this order: `init` once; then for each frame,
  `fixed_update` once per fixed step, `update` once and, with a window, `render` once; then
  `shutdown` once. With a window, `on_event` receives each window event between frames. All have
  defaults that do nothing.
  - `init` can fail: it returns `Result<(), AppError>`, where `AppError` is any boxed error (`?`
    converts errors into it). After a failed `init`, no frame runs, `shutdown` is not called and
    `run` returns the error.
  - `render(ctx, target)` draws the frame into `target`, the window's image, with `ctx.gpu()`;
    the engine shows the image afterwards. It is not called in frames where the window cannot
    show a new image: while it is minimized, or when its next image is not ready in time on
    Vulkan (on Direct3D 12, the engine waits up to one second for it, then draws the frame late).
- `run(&mut app, &config)`: sets up diagnostics and a handler for termination signals, then runs
  the application until it calls `Context::request_exit`, the window is closed, the process
  receives SIGINT (Ctrl+C), SIGTERM or SIGHUP, or `AppConfig::frame_limit` is reached. In every
  case the frame in progress finishes and `shutdown` runs; a second signal ends the process at
  once (exit code 130) for an application that no longer finishes its frames
  (`docs/adr/0012-clean-shutdown-on-termination-signals.md`).
  - Headless: frames run back to back with simulated time: each frame represents one fixed step,
    so runs are deterministic. Neither the windowing system nor the GPU is touched.
  - With a window, `run` must be called from the main thread (it panics otherwise). The window and
    the GPU exist before `init`. Frames represent the real time between them. The engine asks for
    a frame on average at most `AppConfig::max_frame_rate` times per second (60 by default) and
    sleeps in between; a frame runs each time the windowing system lets the window draw, which
    also happens on its own initiative, for example during a resize
    (`docs/adr/0020-rendering-in-the-main-loop.md`). The images are shown as
    `AppConfig::present_mode` says: by default, each one replaces the image waiting to be shown,
    so the display never makes a frame wait, and the schedule alone paces the frames, hidden window
    included (`docs/adr/0021-frame-pacing.md`). With `PresentMode::Fifo`, every image is shown
    in step with the display, which can block a frame: up to one second when the compositor
    rarely shows a hidden window.
  - It returns a `RunError`: `Init` when `init` fails, `Platform` when the window cannot be opened
    (for example without a display server) or its event loop fails, `Gfx` when no GPU can draw into
    the window or when the GPU device or the window's surface is lost for good during the run. After
    a loss, the run stops like after an exit request, `shutdown` included
    (`docs/adr/0022-losing-the-gpu-device.md`, `docs/adr/0025-giving-up-a-lost-surface.md`). Only
    one windowed run, or attempt, is possible per process.
  - The signal handler replaces any other handler the process had for these signals, except one
    installed earlier through the `ctrlc` crate, which is kept. It stays installed after `run`
    returns: a signal then no longer ends the process by itself, and only the second one received
    in the process ends it (exit code 130).
  - On Windows, Ctrl+C and Ctrl+Break in a console stop the run the same way; closing the console
    window ends the process at once, without `shutdown`.
- `AppConfig`: the window (`None`, the default, runs headless), frame limit, fixed step
  (1/60 s), maximum fixed steps per frame (8), maximum frame delta (250 ms), maximum frame rate
  (60) and present mode (`PresentMode::Mailbox`). Write it as a struct literal ending with
  `..AppConfig::default()`.
- `Context`: what an application can ask from the engine: the window (`Context::window`), the GPU
  that draws into it (`Context::gpu`) and the format of its images (`Context::surface_format`),
  all `None` when headless; stopping the run (`Context::request_exit`); and whether it is going
  to stop (`Context::exit_requested`).
- `time`: `FrameClock` turns the time between frames into a `FrameTime` (index, delta, elapsed
  time) and limits how much time one frame can represent; `FixedStep` splits frame time into
  steps of a fixed duration. Both take durations rather than reading a clock
  (`docs/adr/0009-frame-timing.md`).
- `init_diagnostics()`: installs the log subscriber (output on standard error, filtered by the
  `SLIME_LOG` environment variable; events that cannot be written because standard error is gone
  are dropped) and a panic hook that logs panics, then logs the engine version, operating system,
  CPU architecture and number of logical CPUs. `run` calls it; call it
  yourself only in a program that does not use `run`. Later calls do nothing. `LOG_ENV_VAR` is the
  name of that environment variable. When it is unset, `DEFAULT_LOG_FILTER` applies: `info` and
  above, except for wgpu-hal's details about the GPU, the Vulkan loader's messages, and the error
  that winit's client-side decorations log when no desktop portal answers
  (`docs/adr/0019-default-log-filter.md`).

Engine code and games log through the `tracing` macros. The conventions for errors, log levels and
assertions are in `docs/adr/0008-errors-logging-and-assertions.md`; why the engine owns the main
loop is in `docs/adr/0010-main-loop-ownership.md`. With
`SLIME_LOG=slime_app=trace,slime_gfx=trace`, each frame's profiling spans are logged with the time
spent in them: `frame`, containing `fixed_update`, `update`, and with a window `acquire` (waiting
for the window's image), `render` (with `submit`, recorded by `slime-gfx`) and `present`. A bare
`SLIME_LOG=trace` logs them too, but also turns on the trace logs of wgpu and of its shader
compiler, among them a line for each command of each draw, written while a pass is encoded: they
flood standard error, and the time spent writing them counts in the spans. In GPU capture tools
such as RenderDoc, the render passes that draw the window's image of each frame, and their command
buffers, are named `window, frame <index>`, the index of the `frame` span; the image itself keeps
the name the graphics API gives it.

## Dependencies

- `ctrlc`: handles termination signals.
- `slime-gfx`: the GPU and the window's surface, for windowed runs.
- `slime-platform`: the window and its events, for windowed runs.
- `thiserror`: derives `RunError`'s `Display` and `Error` implementations.
- `tracing`: the logging macros and profiling spans.
- `tracing-subscriber`: the subscriber that formats, filters and writes log events, including the
  records that dependencies such as wgpu emit through the `log` crate (feature `tracing-log`).

## Must not

- Use the windowing library or wgpu directly: windows and input go through `slime-platform`, the
  GPU through `slime-gfx`.
- Touch the windowing system or the GPU when it runs headless.
