# Changelog

Notable changes to SlimeEngine, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are git tags (`v0.0.1`, ...);
nothing is promised stable before 1.0 (see [`docs/ROADMAP.md`](docs/ROADMAP.md)).

## [Unreleased]

### Added

- Windows support for what exists: CI runs clippy, the tests and the headless samples on a Windows
  runner, publishes the examples built for Windows, and the Linux checks compile the code for
  Windows too ([ADR-0015](docs/adr/0015-platforms-and-ci.md)).
- `slime-gfx`, the GPU layer: opens a GPU device without a window, with Vulkan on Linux and
  Direct3D 12 on Windows; the checks run its tests on lavapipe (Linux) and WARP (Windows)
  ([ADR-0016](docs/adr/0016-gpu-layer-with-wgpu.md)).
- Offscreen rendering in `slime-gfx`: render targets, vertex buffers, WGSL shader modules and
  render pipelines as engine-owned types with a label each, a render pass, and reading an image
  back to the CPU. A shader or pipeline that wgpu rejects is returned as an error with the
  compiler's message ([ADR-0017](docs/adr/0017-gpu-resource-types.md)).
- The `offscreen` example, which writes a triangle rendered without a window to a PNG file, and an
  image test that compares the same render with a reference image, within a tolerance, on
  lavapipe, WARP and hardware GPUs ([ADR-0018](docs/adr/0018-reference-image-tests.md)).
- Drawing into the window: windowed runs open the GPU for their window (`Gpu::new_for_window`,
  `Surface`), and the new `App::render` draws each frame into the window's image, which the engine
  then presents. `Context::gpu` and `Context::surface_format` let the application create its GPU
  objects in `init`. The `window` example draws the samples' triangle
  ([ADR-0020](docs/adr/0020-rendering-in-the-main-loop.md)).
- `TextureFormat::Bgra8UnormSrgb`, the usual format of windows; render targets in this format
  read back as RGBA, like the others.
- `AppConfig::present_mode` (`PresentMode::Mailbox` or `PresentMode::Fifo`): by default, presenting
  an image never waits, so a hidden window keeps running at the scheduled rate; the frame scheduler
  paces frames on every platform ([ADR-0021](docs/adr/0021-frame-pacing.md)). The `window` example
  takes `--present-mode`.
- Profiling spans for the drawing part of a frame: `acquire`, `render` with `submit` inside, and
  `present`.

### Changed

- The log filter used when `SLIME_LOG` is unset is now `slime_app::DEFAULT_LOG_FILTER`: `info` as
  before, except for wgpu-hal's details about the GPU and the Vulkan loader's messages about
  drivers, whose errors still appear ([ADR-0019](docs/adr/0019-default-log-filter.md)).
- `App::init` returns `Result<(), AppError>`: when it fails, no frame runs, `shutdown` is not
  called, and `run` returns the error. `run` returns a `RunError` (`Init`, `Platform` or `Gfx`)
  instead of a `PlatformError`.
- With a window, frames run on redraw requests: the engine asks for them at most
  `AppConfig::max_frame_rate` times per second, and the windowing system's own requests, during a
  resize for example, run frames too.
- The windowed check in a headless Weston draws with lavapipe.

### Removed

- `softbuffer`, with which the `window` example filled its window from the CPU.

## [0.0.1] - 2026-10-12

The first release, which closes Stage 1 of the roadmap: a very small application that is
architecturally sound. It was tagged `v0.0.1` once keyboard and mouse input had been checked by
hand; its CI run is green.

### Added

- `slime-app`: the `App` trait and `run`, which runs an application headless (frames back to back
  with simulated time, deterministic) or with one window (real time between frames, on average at
  most 60 frames per second by default). The engine owns the main loop
  ([ADR-0010](docs/adr/0010-main-loop-ownership.md)).
- Frame timing: `FrameClock` limits how much time one frame represents, `FixedStep` splits frame
  time into fixed steps and drops the backlog instead of spiralling
  ([ADR-0009](docs/adr/0009-frame-timing.md)).
- Diagnostics: logging through `tracing` filtered by `SLIME_LOG`, panics logged before the
  standard hook runs, a startup line describing the engine and the machine, profiling spans for
  each frame ([ADR-0008](docs/adr/0008-errors-logging-and-assertions.md)).
- Clean shutdown on SIGINT, SIGTERM and SIGHUP; a second signal ends the process at once
  ([ADR-0012](docs/adr/0012-clean-shutdown-on-termination-signals.md)).
- `slime-platform`: one window on Wayland or X11 through winit, with its window, keyboard and mouse
  events as engine-owned types, and the `raw-window-handle` traits
  ([ADR-0011](docs/adr/0011-windowing-with-winit.md)).
- Examples: `hello` (diagnostics), `headless` (a run without a window), `window` (a window filled
  from the CPU, its events logged; also runs headless with `--headless`).
- Development: a Nix shell with a pinned toolchain, `scripts/check.sh` running every check
  including the samples without a display and inside a headless Weston compositor, a GitHub
  Actions workflow running that script, and the decisions recorded in `docs/adr/`.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt). In short: no rendering yet (the
`window` example fills its window from the CPU), the frame rate is limited by sleeping, `App`
methods cannot return errors, modifier state, input methods, touch, gestures and gamepads are not
reported, Linux only.
