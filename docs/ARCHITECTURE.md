# Architecture

This document maps the code: which crates exist, what each one is responsible for, how they
depend on each other, and where to start reading. It describes the repository as it is now;
[`ROADMAP.md`](ROADMAP.md) describes where it is going, and [`adr/`](adr/README.md) records why
it is built this way.

## Repository layout

```text
.
├── .github/workflows/   CI: scripts/check.sh on Linux; tests and samples on Windows
├── Cargo.toml           virtual workspace: shared metadata, lints and dependency versions
├── crates/              engine crates, one directory per crate, named after the package
│   ├── slime-app/       application lifecycle and main loop
│   ├── slime-gfx/       GPU layer (wgpu)
│   ├── slime-platform/  window, its events and input
│   └── slime-render/    renderer: meshes, materials, camera, instances
├── deny.toml            dependency license, advisory and source rules
├── docs/                roadmap, plan, decisions (adr/), glossary, dependencies, this file
├── examples/            runnable examples, one per feature, with their tests (`slime-examples`)
├── flake.nix            Nix development shell
├── rust-toolchain.toml  pinned Rust toolchain
├── scripts/check.sh     every automated check, as CI runs them
└── AGENTS.md            rules for every contributor
```

[ADR-0004](adr/0004-workspace-layout-and-crate-naming.md) sets the layout and naming rules: the
root is a virtual workspace, engine crates are `crates/slime-<role>/`, and a directory or a crate
is created only when code needs it.

## Crates

| Crate | Role | Depends on | Status |
|---|---|---|---|
| [`slime-platform`](../crates/slime-platform/README.md) | The window, its events and keyboard and mouse input; the only crate that uses the windowing library (winit) | `winit`, `raw-window-handle`, `thiserror`, `tracing` | One window; window, keyboard and mouse events |
| [`slime-app`](../crates/slime-app/README.md) | Application lifecycle, main loop, frame timing, diagnostics setup, headless mode | `slime-platform`, `slime-gfx`, `ctrlc`, `thiserror`, `tracing`, `tracing-subscriber` | Headless runs, and windowed runs drawn by the GPU |
| [`slime-gfx`](../crates/slime-gfx/README.md) | The GPU layer; the only crate that uses wgpu | `wgpu`, `pollster`, `raw-window-handle`, `thiserror`, `tracing` | Opens a GPU device, draws into offscreen images and reads them back, draws into a window |
| [`slime-render`](../crates/slime-render/README.md) | The renderer: draws instances of meshes with their materials, seen from a camera, into render targets; built on `slime-gfx`'s types, without wgpu | `slime-gfx`, `glam`, `thiserror`, `tracing` | Meshes, materials of one color, a perspective camera, the depth test, one directional light; textures to come |
| [`slime-examples`](../examples/README.md) | Runnable examples; not part of the engine | `slime-app`, `slime-gfx`, `slime-platform`, `slime-render`, `png`, `tracing` | `hello`, `headless`, `window`, `offscreen`, `scene`; the reference image tests |

## Dependency rules

```text
slime-examples ──▶ slime-app ──┬──▶ slime-platform ──▶ winit
       │                       └──▶ slime-gfx ──────▶ wgpu
       ├──▶ slime-render ──────────▶ slime-gfx
       └──▶ slime-platform, slime-gfx (their types: events, GPU objects)
```

- Dependencies between crates point one way: `slime-app` depends on `slime-platform` and
  `slime-gfx`, never the reverse, and `slime-platform` and `slime-gfx` do not depend on each
  other: the window reaches the GPU layer only as `raw-window-handle` traits.
- `slime-render` depends on `slime-gfx` only and does not use wgpu
  ([ADR-0027](adr/0027-the-render-crate.md)). `slime-app` does not depend on it: an application
  creates its `Renderer` in `init`, from the GPU of its `Context`, and calls it in `render`.
- Only `slime-platform` uses winit, and winit's types never appear in a public API
  ([ADR-0011](adr/0011-windowing-with-winit.md)). `slime_platform::Window` exposes the
  `raw-window-handle` traits so that graphics libraries can draw into it.
- Only `slime-gfx` uses wgpu, and wgpu's types never appear in a public API
  ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). `slime-gfx` depends on no other engine crate,
  so that it works without a windowing system.
- A headless run never touches the windowing system or the GPU: it does not create an event loop
  or open a device, so the windowing libraries and the Vulkan loader, which are loaded at run
  time, are not even needed. The checks run the headless sample with no display server to prove
  it.

## Main flows

### Diagnostics setup

An engine program calls `slime_app::init_diagnostics()` first. It installs a global `tracing`
subscriber writing to standard error (unless one already exists), filtered by the `SLIME_LOG`
environment variable, then a panic hook that logs panics before the standard hook prints them,
then logs one line describing the engine and the machine. From then on, every `tracing` event
from the engine, the game or a dependency goes through that subscriber. Source:
[`crates/slime-app/src/diagnostics.rs`](../crates/slime-app/src/diagnostics.rs); conventions:
[ADR-0008](adr/0008-errors-logging-and-assertions.md).

### Running an application

A game implements `slime_app::App` and calls `slime_app::run`, which owns the main loop
([ADR-0010](adr/0010-main-loop-ownership.md)). A *driver* decides when frames run and what time
they represent; every driver runs a frame the same way (`Frames::run_one` in
[`crates/slime-app/src/app.rs`](../crates/slime-app/src/app.rs)):

```text
frame                                span "frame", trace level
├── FrameClock::advance(delta)       the time the frame represents, limited
├── FixedStep::advance(delta)        how many fixed steps are due: n
├── app.fixed_update(ctx, step)      n times, span "fixed_update"
├── app.update(ctx, &frame)          once, span "update"
└── with a window:
    ├── surface.acquire(label)       the window's image, named "window, frame <index>", or
    │                                none: skip drawing; span "acquire"
    ├── app.render(ctx, target)      draws into the image; span "render"
    └── image.present()              hands it to the window; span "present"
```

The run stops before the next frame once the application has called `ctx.request_exit()`, the
window was closed, the process received a termination signal, or `config.frame_limit` frames have
run. The frame in progress always finishes, and `app.shutdown` is always called after an
`app.init` that succeeded; when `app.init` returns an error, the run stops at once and `run`
returns it as `RunError::Init`. The call order is checked by
[`crates/slime-app/tests/lifecycle.rs`](../crates/slime-app/tests/lifecycle.rs) headless, and by
[`crates/slime-app/tests/windowed_lifecycle.rs`](../crates/slime-app/tests/windowed_lifecycle.rs)
with a window, which the checks run in a headless Weston compositor (ADR-0024).

**Headless driver** (`AppConfig::window` is `None`), in `app.rs`:

```text
slime_app::run
├── init_diagnostics()
├── app.init(ctx)
├── loop: frame with delta = the fixed step (simulated time), back to back
└── app.shutdown(ctx)
```

**Windowed driver** (`AppConfig::window` is `Some`), in
[`crates/slime-app/src/windowed.rs`](../crates/slime-app/src/windowed.rs). winit owns the event
loop; `slime-platform` adapts its callbacks to the `EventHandler` trait, which the windowed driver
implements:

```text
slime_app::run
├── init_diagnostics()
└── slime_platform::run_event_loop(window config, driver)
    ├── winit "resumed"          create the window → driver.window_created
    │                            → Gpu::new_for_window: the GPU and the window's surface;
    │                              ctx gets the window and the GPU; app.init(ctx)
    ├── winit "window_event"     translate (event.rs) → driver.event
    │                            → Resized: surface.resize; app.on_event(ctx, &event);
    │                              CloseRequested → stop
    ├── winit "RedrawRequested"  → driver.redraw_requested → frame with the measured delta
    ├── winit "about_to_wait"    → driver.about_to_wait
    │                            → if the next frame is due: window.request_redraw();
    │                              return WaitUntil(next frame) or Exit
    └── winit "exiting"          → driver.exiting → app.shutdown(ctx); the surface, the GPU and
                                   the window are released
```

Frames run on redraw requests ([ADR-0020](adr/0020-rendering-in-the-main-loop.md)). The engine
asks for one every `1 / AppConfig::max_frame_rate` seconds: between them the event loop sleeps
until the next frame is due or an event arrives. A frame that starts a little late does not shift
the schedule, which keeps the average rate; after falling a whole period behind, the schedule
restarts instead of catching up with frames run back to back (`next_due` in `windowed.rs`). The
windowing system also asks for redraws on its own, during a resize for example, and each of them
runs a frame too; such a frame restarts the schedule as well, so the next frame the engine asks for
comes one period after it. On Windows, this is what keeps the window drawn while the user resizes it: the
system then runs its own loop, in which `about_to_wait` is not called.

### Opening the GPU

`slime_gfx::Gpu::new()` creates a wgpu instance limited to the platform's graphics API (Vulkan on
Linux, Direct3D 12 on Windows), asks for the high-performance adapter, logs it, and opens its
device and queue ([`crates/slime-gfx/src/gpu.rs`](../crates/slime-gfx/src/gpu.rs),
[ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). Without a GPU, the adapter is a software one:
lavapipe on Linux when `VK_DRIVER_FILES` points at it, WARP on Windows. Headless runs never open
the GPU.

`slime_gfx::Gpu::new_for_window(window, width, height, present_mode)` does the same for a window:
it creates the window's surface from the window's `raw-window-handle` traits first, asks only for
adapters that can present to it, and configures it in the first sRGB format the window offers and
the requested present mode, FIFO when the window does not offer Mailbox
([`crates/slime-gfx/src/surface.rs`](../crates/slime-gfx/src/surface.rs)). The surface keeps the
window alive, which is why the windowed driver releases it before the event loop closes its
connection to the windowing system.

### Rendering offscreen

Drawing into an image without a window, as the `offscreen` example and the reference image test
do with the triangle of [`examples/common/triangle.rs`](../examples/common/triangle.rs):

```text
gpu = Gpu::new()
├── create_render_target(label, width, height, format)   the image to draw into
├── create_depth_target(label, width, height)            the depth of what is drawn, for passes
│                                                        that test depth
├── create_shader_module(label, wgsl)                    rejected → GfxError::Shader
├── create_bind_group_layout(label, entries)             what the shaders read: uniform buffers
├── create_render_pipeline(&descriptor)                  rejected → GfxError::Pipeline; declares
│                                                        its bind group layouts, depth test, culling
├── create_vertex_buffer(label, bytes)
├── create_index_buffer(label, indices)
├── create_uniform_buffer(label, bytes)                  written again with write_uniform_buffer
├── create_bind_group(label, &layout, entries)           the uniform buffers, bound together
├── render(&RenderPassDescriptor { target, clear, depth }, |pass| ...)
│                                                        one command encoder and render pass; the
│                                                        closure sets the pipeline, the bind groups
│                                                        and the buffers, and draws; then submitted
└── read_render_target(&target)                          copy into a buffer, wait for the GPU,
                                                         map it → Image (RGBA, top row first)
```

Each GPU object is an engine-owned type holding its wgpu object
([ADR-0017](adr/0017-gpu-resource-types.md); bind groups and depth:
[ADR-0026](adr/0026-resource-binding-and-depth.md)). Shader modules and pipelines are created inside a
wgpu error scope, so that a rejection comes back as an error with wgpu's message; any other wgpu
error is a bug and panics ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). The test
[`examples/tests/offscreen.rs`](../examples/tests/offscreen.rs) compares the image with a
reference PNG within a tolerance ([ADR-0018](adr/0018-reference-image-tests.md)).

### Rendering a scene

`slime-render` draws meshes with the objects above. The application builds a `Scene` for each
frame, a flat list of instances seen from a camera, and the renderer draws it into any render
target, the window's image or an offscreen one
([`crates/slime-render/src/renderer.rs`](../crates/slime-render/src/renderer.rs), rules:
[ADR-0027](adr/0027-the-render-crate.md)):

```text
renderer = Renderer::new(&gpu, format)                 the shader, the pipeline (depth test, back-
│                                                      face culling), the bind group layouts
├── create_mesh(&gpu, label, &MeshData)                vertex and index buffers, after a check
├── create_material(&gpu, label, &MaterialData)        a uniform buffer and its bind group
└── render(&gpu, &target, &Scene { camera, clear, instances }) → RenderStats
    ├── the depth target, created at the target's size, again when it changes
    ├── the frame uniform ← camera.view_projection(aspect), the light  bind group 0
    ├── the instance buffer ← each instance's transform and normal     bind group 2, one dynamic
    │   matrix, one slot each, at the device's alignment; grows when   offset per instance
    │   a scene needs it
    └── one render pass, cleared: for each instance, its material's
        bind group 1, its slot, its buffers, draw_indexed
```

The shader ([`shader.wgsl`](../crates/slime-render/src/shader.wgsl)) transforms each vertex by
its instance's matrix then the camera's, and its normal by the instance's normal matrix (the
inverse transpose, so that normals stay perpendicular to surfaces scaled unevenly); each fragment
is lit by the scene's directional light in proportion to the cosine between its normal and the
direction to the light (Lambert), plus the ambient light, times its material's base color.
Conventions: world space is right-handed with +Y up, the camera looks down −Z, front faces wind
counter-clockwise, depth runs from 0 to 1, colors are linear and the sRGB target encodes them.

### Rendering in a window

The same objects draw into a window. Each frame of a windowed run, `Frames::run_one` asks the
surface for the window's image (`Surface::acquire`), lets the application draw into its target
(`App::render`, usually with `Gpu::render`), then presents it (`SurfaceFrame::present`). The
surface handles what the graphics API reports about the window, so that the application only sees
whether there is an image to draw:

| Reported | Handled by |
|---|---|
| A new size (from `Event::Resized`) | `Surface::resize`; the surface is configured again before the next image |
| A size above the largest image the device accepts (8192 pixels in a dimension with the default limits) | Limited to it, with a warning; the images are then smaller than the window |
| A size of zero (minimized window) | No image; the frame runs without drawing |
| Outdated configuration | Configured again, then one more try; no image this frame if still outdated |
| Suboptimal image | Drawn and presented, then configured again |
| Image not ready in time, window occluded | No image this frame |
| Surface lost | Created again from the window; no image this frame. Lost for good (it cannot be created again, or is lost again each time): no image, and the run stops before the next frame, like after a lost device |
| Device lost (see [Losing the GPU](#losing-the-gpu)) | No image; the run stops before the next frame |

How the window shows the images is the *present mode* (`AppConfig::present_mode`). With Mailbox,
the default, presenting never waits, and the frame scheduler alone paces frames, whether the
window is visible or not; windows that do not offer it fall back to FIFO, with which presenting
can block a frame. The measurements behind this choice are in
[ADR-0021](adr/0021-frame-pacing.md).

In GPU capture tools such as RenderDoc, each render pass is named after its target, so the window's
pass of frame 42 appears as `window, frame 42`, and `RenderPass::debug_group` names groups of
commands inside it; the samples' triangle is drawn in a group named `triangle`. Debug builds pass
these names to the driver ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)).

### Losing the GPU

The driver can lose the device: it crashes or is reset, the GPU is removed, or it runs out of
memory. `slime-gfx` records the loss in wgpu's device-lost callback, and from then on its error
handler logs wgpu's errors instead of panicking, since every use of the device fails. Before each
frame, the windowed driver checks `Gpu::check()` with the other reasons to stop: after a loss, the
run stops like after an exit request, and `run` returns `RunError::Gfx` with
`GfxError::DeviceLost` ([ADR-0022](adr/0022-losing-the-gpu-device.md)). The window's surface can be
lost for good as well, when it cannot be created again after a loss or is lost again each time it
is: `Surface::check` reports it, and the run stops the same way with `GfxError::SurfaceLost`
([ADR-0025](adr/0025-giving-up-a-lost-surface.md)).

### Termination signals

`run` installs, once per process, a handler for SIGINT (Ctrl+C), SIGTERM and SIGHUP
([`crates/slime-app/src/signal.rs`](../crates/slime-app/src/signal.rs)). The handler runs on a
thread of its own and only sets a flag; before each frame, the driver records the flag in the
`Context` as an exit request, so the run stops like any other: the frame in progress finishes and
`app.shutdown` runs. A second signal ends the process at once with exit code 130
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

### Frame timing

`slime_app::time` has no clock of its own: the driver gives `FrameClock` the duration of each
frame (measured, or simulated in headless runs), gets a `FrameTime` back, and gives its delta to
`FixedStep`, which says how many fixed steps the frame must run. Source:
[`crates/slime-app/src/time.rs`](../crates/slime-app/src/time.rs); rules:
[ADR-0009](adr/0009-frame-timing.md).

## Platforms

The engine targets Linux (`x86_64-unknown-linux-gnu`) and Windows (`x86_64-pc-windows-msvc`)
([ADR-0015](adr/0015-platforms-and-ci.md)). Platform differences are kept in few places:

- winit hides the windowing systems (Wayland and X11 on Linux, Win32 on Windows). The only
  platform-specific code in `slime-platform` names the windowing system in the log
  (`backend_name` in `event_loop.rs`). One difference shapes the main loop: on Windows, the
  system runs its own loop while the user resizes or moves a window, which is why frames run on
  redraw requests.
- `ctrlc` hides signals (Unix) and console events (Windows); `crates/slime-app/src/signal.rs`
  describes how each platform behaves.
- wgpu hides the graphics APIs; `slime-gfx` only chooses which one to ask for on each platform
  (`DEFAULT_BACKENDS` in `crates/slime-gfx/src/gpu.rs`).
- Tests that cannot run on a platform are compiled out with `#[cfg]`, and their file says why: the
  signal tests run on Unix only.

`scripts/check.sh` compiles the whole workspace for Windows on Linux, and CI runs the tests and
the headless samples on a Windows runner.

## Where to start reading

1. [`README.md`](../README.md) for what the project is and its state.
2. This file for the crate map.
3. [`PLAN.md`](PLAN.md) for the milestone in progress.
4. The `README.md` of the crate you are about to change.
5. [`examples/`](../examples/README.md) for runnable programs that use the engine.

## `unsafe` code

No crate contains `unsafe` code. [ADR-0005](adr/0005-lints-and-unsafe-policy.md) says when it is
allowed and how it must be written.
