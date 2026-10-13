# Changelog

Notable changes to SlimeEngine, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are git tags (`v0.0.1`, ...);
nothing is promised stable before 1.0 (see [`docs/ROADMAP.md`](docs/ROADMAP.md)).

## [Unreleased]

### Added

- `slime-render`, the first renderer: `Renderer` draws a `Scene` (a camera, a directional light,
  a clear color and instances of meshes with their materials) into any render target, with a
  depth test and back-face culling, and returns `RenderStats` (draw calls, triangles, the CPU time
  of the call and, on adapters that can time their work, the GPU time of its pass); meshes come
  from positions, normals, texture coordinates and indices, materials have a base color, which a
  texture may modulate, `Camera` gives its matrices in WebGPU's convention, and `glam` is
  re-exported for the math ([ADR-0027](docs/adr/0027-the-render-crate.md)). `DirectionalLight`
  lights surfaces in proportion to the cosine between their normal and the direction to the light,
  plus an ambient light; normals follow the rotation of their instance and stay perpendicular to
  surfaces scaled unevenly.
- The `scene` example, the sample of Stage 3a: a glTF scene drawn by the renderer, lit by a
  directional light, seen from a camera that orbits it until the mouse takes over (dragging turns
  it, the wheel zooms). It draws `examples/assets/scene.glb`, made in Blender by
  `examples/assets/make_scene.py` and released under CC0, or any GLB file with embedded buffers
  and PNG images given with `--scene`, through a temporary loader in the examples
  ([ADR-0029](docs/adr/0029-temporary-gltf-loading.md)). `--frames <count>` closes it after that
  many frames and fails if none of them drew the scene, which lets `scripts/check.sh` run it in
  the headless Weston. Two image tests, compared with
  references rendered with lavapipe: the sample scene from two camera poses, and shapes drawn by
  the renderer (cubes and a smooth sphere, two of them with a checker texture).
- `slime-devui`, a temporary crate: `Overlay` draws egui panels over the engine's frames, fed
  with the engine's events, through a new `wgpu` feature of `slime-gfx` that gives the engine's
  tooling the wgpu objects behind its types ([ADR-0031](docs/adr/0031-dev-overlay-with-egui.md)).
  The `scene` example shows the frame time, the CPU and GPU times of its render, its draw calls
  and triangles in an overlay; F1 hides it.
- GPU timing: `Gpu::create_timer` gives a `GpuTimer` on adapters with timestamp queries, which a
  render pass uses to measure how long it takes on the GPU, read back a few frames later without
  waiting; `Gpu::can_time` tells.
- The GPU resources a renderer needs, in `slime-gfx`: index buffers and indexed draws, uniform
  buffers written through `Gpu::write_uniform_buffer`, bind group layouts and bind groups that
  pipelines declare and render passes bind, with dynamic offsets so that one uniform buffer holds
  the values of many draws (`Gpu::uniform_offset_alignment` gives their alignment), depth targets
  with a depth test and face culling declared by the pipeline, and render passes that keep the
  target's content instead of clearing it
  ([ADR-0026](docs/adr/0026-resource-binding-and-depth.md)). `RenderTarget::label` gives a
  target's label back.
- Textures and samplers in `slime-gfx`: `Gpu::create_texture` uploads an image that shaders
  sample, in an sRGB format (decoded when read) or in the new linear `TextureFormat::Rgba8Unorm`,
  which `TextureFormat::is_srgb` tells apart; `Gpu::create_sampler` sets the filters and the
  address modes that glTF samplers express; both are bound through bind groups.
- Textures have mipmaps, computed on the CPU when they are created, in linear light for the sRGB
  formats; samplers read the level of detail that fits, and linear filtering blends two levels
  ([ADR-0032](docs/adr/0032-mipmaps-computed-at-texture-creation.md)).
- `Surface::check` and `GfxError::SurfaceLost`: a window's surface that is lost for good (it cannot
  be created again, or is lost again each time) stops the run like a lost device, instead of an
  error logged at every frame while the run went on without drawing
  ([ADR-0025](docs/adr/0025-giving-up-a-lost-surface.md)).

### Changed

- The image tests keep the tolerance of 2 out of 255, exceeded by no pixel, on lavapipe, which
  rendered the references; on other drivers, which filter textures and place edges differently,
  a pixel may differ by up to 8, and 0.1 % of the pixels by more
  ([ADR-0028](docs/adr/0028-image-tests-tolerate-a-few-pixels.md),
  [ADR-0030](docs/adr/0030-image-tests-strict-on-the-reference-driver.md)). The tests print what
  they measured with `--nocapture` (how many pixels differ by more than 2, 4, 8 and 16), and the
  Windows CI job runs them that way.
- `Gpu::render` takes a `RenderPassDescriptor` (the target, whether to clear it, an optional depth
  target, an optional `GpuTimer`) instead of a target and a clear color.
  `RenderPipelineDescriptor` gains `bind_group_layouts`, `depth` and `cull`, and pipeline layouts
  are always explicit: a shader binding that no layout holds is rejected as `GfxError::Pipeline`.
- The default log filter also hides the error that winit's client-side decorations log when no
  session bus or desktop portal answers (`sctk_adwaita::config=off`,
  [ADR-0019](docs/adr/0019-default-log-filter.md)).
- The Windows CI artifact also contains `offscreen.exe` and `scene.exe`; the Windows job runs the
  release build of the headless samples, and runs the tests one at a time, since WARP's shader
  compiler crashes now and then when several threads compile shaders at once.
- `scripts/check.sh` also runs the `scene` example and the windowed lifecycle test of `slime-app`
  in the headless Weston ([ADR-0024](docs/adr/0024-windowed-lifecycle-check-in-weston.md)), and
  `cargo deny` checks the dependencies of the supported targets only
  ([ADR-0023](docs/adr/0023-dependency-checks-for-the-supported-targets.md)).
- The `window` example exits with a failure when a run limited by `--frames` drew nothing, so
  that the windowed check in a headless Weston proves that frames are drawn, as
  [ADR-0013](docs/adr/0013-windowed-smoke-test-in-weston.md) says.

### Removed

- `GfxError::Wait`, which no method could return: wgpu's indefinite wait reports no error.

### Fixed

- On Windows, the log uses colors only in Windows Terminal: the older console host printed the
  escape sequences as text.
- The examples report a command-line argument that is not valid text as a usage error instead of
  panicking, and the `offscreen` example names the file it could not write.
- A windowed run without any usable graphics driver reports `no GPU adapter is available` with
  wgpu's explanation, instead of `cannot create a surface for the window` with an empty cause.
- A `SLIME_LOG` value without a bare level, such as `slime_app=trace`, no longer silences every
  other module: `info` applies to the modules it does not name, as the documentation said. A
  misspelt level, which `tracing` reads as a module name, no longer silences everything either.
- On Wayland and X11, a key release no longer carries the text of the press in `KeyEvent::text`,
  which the documentation promised to be `None` and which Windows already left empty. The
  documentation now says that named keys such as Enter carry their control character.
- `Gpu::wait_idle`, `Gpu::create_shader_module`, `Gpu::create_render_pipeline` and
  `Gpu::read_render_target` return `GfxError::DeviceLost` once the device is lost, as documented,
  instead of `Ok` with objects that cannot be used.
- A `WindowConfig` with a zero width or height, a size above 16 384 logical pixels, or a title
  containing a NUL byte is rejected with a clear message before the window is created, instead of
  panicking inside the windowing library (a zero size on Wayland, a NUL byte on Wayland and X11) or
  overflowing in its decorations.
- A shader module or render pipeline that the graphics driver's compiler rejects, after wgpu's
  own validation accepted it, is returned as `GfxError::Shader` or `GfxError::Pipeline` instead
  of panicking.
- `Gpu::create_vertex_buffer` rejects empty data with a message naming the buffer, instead of
  letting the first `set_vertex_buffer` panic inside wgpu; its rustdoc now lists its panics.
- A windowed run whose `render` draws nothing into the window's image no longer grows at every
  frame (about 180 kB per frame, measured with lavapipe): the engine gives wgpu a submission for
  such frames, without which it never reclaimed the work of the earlier ones.
- Frames that the windowing system asks for on its own (during a resize) or that the application
  requests through `Window::request_redraw` no longer postpone the engine's schedule: after a burst
  of them, the loop went to sleep until the schedule caught up, for seconds, without running frames
  or noticing termination signals.
- A window larger than the images the GPU accepts (8192 pixels in a dimension with the default
  limits) no longer panics the engine: the images are limited to that size, with a warning.
- `FrameClock` no longer panics when the elapsed time would overflow (a maximum frame delta
  near `Duration::MAX`): it saturates.
- Logging to a standard error that is gone (a pipe whose reader exited, a closed terminal) no
  longer aborts the process: the events are dropped and the run goes on.

## [0.0.2] - 2026-10-12

The second release, which closes Stage 2 of the roadmap: the GPU layer. The engine draws a triangle
in its window on Linux (Vulkan) and Windows (Direct3D 12), renders offscreen images that CI
compares with a reference on software drivers, paces frames without stalling when the window is
hidden, names its GPU work for capture tools, and stops cleanly when the GPU device is lost.

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
- Names for GPU capture tools such as RenderDoc: `RenderPass::debug_group` names groups of
  commands, and the window's image of each frame is named `window, frame <index>`.
- A lost GPU device (driver crash or reset, GPU removed, out of memory) stops the run cleanly:
  `run` returns `RunError::Gfx` with `GfxError::DeviceLost`, instead of the engine panicking
  ([ADR-0022](docs/adr/0022-losing-the-gpu-device.md)). `Gpu::check` tells whether the device
  is still usable.

### Changed

- The log filter used when `SLIME_LOG` is unset is now `slime_app::DEFAULT_LOG_FILTER`: `info` as
  before, except for wgpu-hal's details about the GPU and the Vulkan loader's messages about
  drivers, whose errors still appear ([ADR-0019](docs/adr/0019-default-log-filter.md)).
- `App::init` returns `Result<(), AppError>`: when it fails, no frame runs, `shutdown` is not
  called, and `run` returns the error. `run` returns a `RunError` (`Init`, `Platform` or `Gfx`)
  instead of a `PlatformError`.
- With a window, frames run on redraw requests: the engine asks for them at most
  `AppConfig::max_frame_rate` times per second, and the windowing system's own requests, during a
  resize for example, run frames too. In `slime-platform`, `Window::request_redraw` asks for one,
  and `EventHandler` has a new required method, `redraw_requested`, through which the event loop
  reports them.
- The windowed check in a headless Weston draws with lavapipe.

### Removed

- `softbuffer`, with which the `window` example filled its window from the CPU.

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt). In short: no renderer yet (no
depth, textures, lighting or meshes; they come with Stage 3), frames are not synchronized with the
display's refresh, presenting can still stall where Mailbox is not offered, frames pause on
Windows while the window is moved, wgpu's errors panic in release builds too, a lost GPU device is
not replaced, and modifier state, input methods, touch, gestures and gamepads are not reported.

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
