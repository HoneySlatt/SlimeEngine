# Changelog

Notable changes to SlimeEngine, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are git tags (`v0.0.1`, ...);
nothing is promised stable before 1.0 (see [`docs/ROADMAP.md`](docs/ROADMAP.md)).

## [Unreleased]

### Added

- Multi-pass rendering in `slime-gfx`
  ([ADR-0033](docs/adr/0033-gpu-layer-for-multi-pass-rendering.md)): render targets in the new
  `TextureFormat::Rgba16Float`, 16-bit floating-point colors beyond 0 to 1, read back with
  `Gpu::read_float_render_target` as a `FloatImage`; render targets and depth targets that later
  passes sample (`BindingResource::RenderTarget`, and `BindingResource::DepthTarget` at the new
  `BindingType::DepthTexture`); pipelines without a vertex buffer, for a fullscreen triangle built
  from the vertex index; depth-only pipelines and passes (`Gpu::render_depth`); comparison samplers
  (`Gpu::create_comparison_sampler`, `BindingType::ComparisonSampler`), for shadow lookups with
  percentage-closer filtering; and a depth bias on pipelines (`DepthBias`).
- `Gpu::max_texture_dimension()` and `Gpu::max_buffer_size()`: the largest width or height of a
  texture, a render target, a depth target or a window's image (8192 with the default limits),
  and the largest buffer (256 MiB). A loader checks the images and meshes of a file against them
  before creating them, as the examples' glTF loader does.
- HDR rendering and tone mapping in `slime-render`
  ([ADR-0034](docs/adr/0034-hdr-rendering-and-tone-mapping.md)): the scene is drawn into a 16-bit
  floating-point target with its depth target, both following the output's size, then a
  tone-mapping pass multiplies it by `Scene::exposure` and maps it to the output with Khronos PBR
  Neutral, which keeps authored base colors and hues and compresses highlights instead of
  clipping them. `RenderStats::scene_gpu_time` and `RenderStats::tone_mapping_gpu_time` give the
  GPU time of each pass. The `scene` example shows both and has an exposure slider.

### Changed

- `RenderStats::gpu_time` is replaced by `scene_gpu_time` and `tone_mapping_gpu_time`; `Scene`
  has a new field, `exposure`; `Renderer::new` takes the output's format, any render-target
  format. Images change: a white surface lit by a light of 1 shows as 240 instead of 255,
  mid-tones lose 0.04 (0.5 shows as 181 instead of 188), highlights no longer clip, and dark
  colors are darker and more saturated (the samples' background shows as (34, 44, 60) instead of
  (63, 69, 80)).
- `RenderPipelineDescriptor::vertex_layout` is an `Option`, and `fragment: Option<FragmentState>`
  replaces `fragment_entry_point` and `target_format`. `DepthState` gains `bias` and is no longer
  `Eq` or `Hash`. Offscreen render targets and depth targets carry the usage that lets shaders
  sample them. `Gpu::create_texture` and `Gpu::read_render_target` panic on `Rgba16Float`, a
  format of render targets only, read with `Gpu::read_float_render_target`.
- `GpuTimer::latest` takes no argument and no longer polls the device. Times come back when later
  work is submitted (the next frame) or in `Gpu::wait_idle`.
- The `submit` profiling span covers only the queue submission. wgpu's validation and encoding of
  a pass's commands now count in the caller's span (`render` in a windowed run).
- The second termination signal no longer logs a warning before it ends the process; exit code
  130 says what happened.
- `slime_devui::Overlay::render` takes an `FnMut` closure instead of an `FnOnce`, since egui may
  call it twice in a frame. A closure that moves a captured value into egui (for example
  `ui.label(name)` with an owned `String`) must borrow it instead.
- `slime_devui::Overlay::render` panics with a clear message when the target does not have the
  overlay's format. It used to panic only on frames that drew something, with a wgpu validation
  error.
- The dev overlay makes one queue submission per frame instead of two.
- When a step of the Windows CI job fails, the job publishes the images that the failing image
  tests rendered on WARP, as the artifact `image-test-failures-windows`. Its test step no longer
  stops at the first test program that fails.
- `scripts/check.sh` also runs clippy on the windowed lifecycle test of `slime-app`, on Linux and
  for Windows. Because it is outside the default test run (`test = false`), `--all-targets` used
  to leave it out.

### Fixed

- Key events pair up the same way on every platform (`slime-platform`). A key still down when the
  window loses the keyboard focus is released just before `Event::Focused(false)`. This is new on
  Wayland, where such a key stayed down for applications that track held keys. A key already down
  when the window gains the focus is no longer reported as a new press on X11 and Windows; it is
  not reported at all, its release included, until it is pressed again. Before, holding Escape
  while a launcher or a dialog closed quit the `window` and `scene` examples. On Windows, which
  reports only the release of Print Screen, that key now gives no event.
- On Windows, ending the event loop no longer waits for the last frame's deadline, and no longer
  calls `EventHandler::about_to_wait` once more before `exiting`. After a failed window creation,
  the loop no longer waits for a message without limit before returning the error. Not checked on
  Windows yet.
- On Windows, the window is hidden when the event loop ends, instead of staying on screen,
  unresponsive, until the process exits. Not checked on Windows yet.
- On X11, a window whose size times the scale factor exceeds 65 535 physical pixels no longer
  panics inside winit. That is 16 384 logical pixels at a scale factor of 4, and 65 535 is the
  most the protocol allows. The size is now limited to 65 535 physical pixels per side.
- The documentation of `slime-platform` was wrong in three places, now corrected:
  - On X11, losing the X server ends the process; only on Wayland does the loop end cleanly.
  - A missing libxkbcommon panics instead of returning `PlatformError::Connect`.
  - Mouse positions can lie outside the window during a drag and, on X11, when the window gains
    the focus.
- A `SLIME_LOG` value with a stray comma or a space around a directive, such as `slime_app=trace,`
  or ` debug`, no longer silences the modules it does not name. Empty directives and the spaces
  around directives are dropped before the value is read. A value with no directive left uses the
  default filter.
- A second termination signal now ends the process with exit code 130 even while standard error
  is blocked, for example a pipe that nobody reads. The thread that handles signals no longer
  writes the log line itself.
- Frames that get no window image (a minimized window, an image not ready in time) now give wgpu
  a submission, as frames with an undrawn image already did. GPU writes an application makes
  outside `App::render` (`Gpu::write_uniform_buffer`, `Gpu::create_texture`) no longer pile up for
  as long as the window is minimized.
- Mipmaps of textures with an odd width or height now include the last column and row of each
  odd-sized level. Each level used to be computed without them, so the smallest levels of a
  texture whose sides are not powers of two showed only its top-left part, stretched. Along an odd
  side longer than one pixel, each pixel of the next level now weights the three pixels it
  overlaps by how much of each it covers
  ([ADR-0032](docs/adr/0032-mipmaps-computed-at-texture-creation.md)). Textures whose sides are
  powers of two are unchanged.
- Creating a vertex, index or uniform buffer no longer panics once the GPU device is lost. Before,
  a mesh, a material or a larger instance buffer created after a driver reset panicked inside
  wgpu.
- On Vulkan, `Gpu::create_render_pipeline` and `Gpu::create_shader_module` return
  `GfxError::DeviceLost` when the driver's failure to create the object loses the device. Before,
  they returned `Ok` with an unusable object.
- A renderer with a GPU timer no longer panics inside wgpu in the frame where a Vulkan driver
  reports the device lost: the run stops with `GfxError::DeviceLost`, as documented.
- A windowed run whose GPU device is lost before the window's first image, or before the first
  image after its surface was created again, stops with `GfxError::DeviceLost` instead of
  panicking inside wgpu.
- `slime-render` lights an instance flattened along one axis (a scale of 0 on one axis, such as a
  glTF node with scale [1, 1, 0]), whose normals were NaN. Normals are now transformed by the
  cofactor matrix of the instance's transform, which also stays finite at extreme scales.
- `slime-render` writes an alpha of 1 for every surface it draws, as documented for opaque
  surfaces. It wrote the material's alpha, so an offscreen image read back with
  `Gpu::read_render_target` showed translucent surfaces.
- `Renderer::create_mesh` returns `RenderError::InvalidMesh` for a mesh whose vertices or indices
  need a buffer larger than the device accepts (`Gpu::max_buffer_size`, 256 MiB with the default
  limits: more than 8 388 608 vertices or 67 108 864 indices), instead of panicking in wgpu.
- `Camera`'s documentation names the inputs that give a matrix that is not finite: a `target` at
  `eye`, and a `far` that is not finite.
- The dev overlay keeps its panels in place when the window's scale factor changes. Moving the
  window to a screen with a higher scale used to push them toward the top-left, for good.
- The dev overlay no longer draws an empty frame when egui lays a frame out twice, as on the first
  frame of a grid or of a window being resized.
- The dev overlay no longer panics when the GPU device was lost earlier in the frame. It skips the
  frame, and the windowed run stops with `GfxError::DeviceLost`.
- The examples' glTF loader returns an error instead of panicking or aborting on what the `gltf`
  crate's validation lets through:
  - images larger than the GPU accepts, checked before they are decoded;
  - primitives whose vertices or indices need a larger buffer than the GPU accepts, checked before
    the loader builds them;
  - image data that lies outside the binary chunk, has no MIME type, or is missing;
  - accessors stored in types glTF does not allow, sparse, empty, overlapping, or outside the
    binary chunk. Before, indices it could not read were silently drawn as non-indexed triangles;
  - node hierarchies that are not trees. A cycle used to overflow the stack;
  - an index that names no vertex, in a primitive without normals.
- The loader places node hierarchies of any depth. A valid chain of a few thousand nodes
  overflowed the stack and aborted.
- The loader gives primitives without normals flat normals, as glTF requires, instead of
  smoothing them across shared vertices.
- The loader draws mirrored nodes (a negative determinant) with their front faces outward instead
  of inside-out.
- The loader warns about vertex colors, which it ignores.
- The `scene` example draws large and small GLB files: its camera's near and far planes now fit
  the scene, and the wheel keeps the camera between 0.1 and 20 radii of it.
- In the `scene` example, a drag that ends over the dev overlay no longer leaves the camera
  turning with the mouse.
- The `scene` example no longer replays, when F1 shows its overlay again, all the input received
  while the overlay was hidden: a click on the scene could press a button of the overlay.
- A renderer failure in the `scene` example is reported with its cause.
- `examples/README.md` says that the Windows examples published by CI need the x64 Microsoft
  Visual C++ Redistributable (`VCRUNTIME140.dll`), which a fresh Windows install lacks.
- `CONTRIBUTING.md` tells contributors on WARP to run the tests one at a time
  (`--test-threads=1`), as the Windows CI job does.

## [0.0.3] - 2026-10-13

The third release, which closes Stage 3a of the roadmap: the first renderer. The engine draws a
glTF scene with textured materials, a depth test and a directional light under a camera the mouse
moves, measures what each frame costs on the CPU and on the GPU and shows it in a dev overlay, and
covers the renderer with image tests that run on lavapipe, WARP and hardware GPUs.

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

### Known limitations

See the known debt in [`docs/PLAN.md`](docs/PLAN.md#known-debt). In short: the renderer is a first
one (no physically based materials, shadows, HDR, post-processing, multisampling, transparency,
animation or glTF extensions; they come with Stage 3b and later); glTF files are read by a
temporary loader in the examples, which takes GLB files with embedded buffers and PNG images only;
the dev overlay is temporary tooling that gets no modifier keys; mipmaps are computed on the CPU;
upgrading wgpu waits for egui; and the limitations of 0.0.2 remain: frames are not synchronized
with the display's refresh, presenting can still stall where Mailbox is not offered, frames pause
on Windows while the window is moved, wgpu's errors panic in release builds too, a lost GPU device
is not replaced, and modifier state, input methods, touch, gestures and gamepads are not reported.

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
