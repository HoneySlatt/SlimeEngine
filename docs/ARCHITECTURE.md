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
├── Cargo.lock           the exact version of every dependency, checked in
├── crates/              engine crates, one directory per crate, named after the package
│   ├── slime-app/       application lifecycle and main loop
│   ├── slime-devui/     dev overlay (egui), temporary tooling
│   ├── slime-gfx/       GPU layer (wgpu)
│   ├── slime-platform/  window, its events and input
│   └── slime-render/    renderer: meshes, PBR materials, camera, lights, shadows, HDR, tone mapping
├── deny.toml            dependency license, advisory and source rules
├── docs/                roadmap, plan, decisions (adr/), glossary, dependencies, this file
├── examples/            runnable examples, one per feature, with their tests (`slime-examples`)
├── flake.nix            Nix development shell
├── flake.lock           pinned inputs of the development shell
├── rust-toolchain.toml  pinned Rust toolchain
├── scripts/check.sh     every automated check, as CI runs them
├── AGENTS.md            rules for every contributor
├── CHANGELOG.md         notable changes, per release
├── CONTRIBUTING.md      environment, checks, how a change is made and reviewed
├── LICENSE              MPL-2.0
└── README.md            what the project is, its state, where to read next
```

[ADR-0004](adr/0004-workspace-layout-and-crate-naming.md) sets the layout and naming rules: the
root is a virtual workspace, engine crates are `crates/slime-<role>/`, and a directory or a crate
is created only when code needs it.

## Crates

| Crate | Role | Depends on | Status |
|---|---|---|---|
| [`slime-platform`](../crates/slime-platform/README.md) | The window, its events and keyboard and mouse input; the only crate that uses the windowing library (winit) | `winit`, `raw-window-handle`, `thiserror`, `tracing` | One window; window, keyboard and mouse events |
| [`slime-app`](../crates/slime-app/README.md) | Application lifecycle, main loop, frame timing, diagnostics setup, headless mode | `slime-platform`, `slime-gfx`, `ctrlc`, `thiserror`, `tracing`, `tracing-subscriber` | Headless runs, and windowed runs drawn by the GPU |
| [`slime-gfx`](../crates/slime-gfx/README.md) | The GPU layer; the only crate that uses wgpu, apart from the dev overlay through its `wgpu` feature | `wgpu`, `pollster`, `raw-window-handle`, `thiserror`, `tracing` | Opens a GPU device; draws into offscreen images, 8-bit or 16-bit float, and reads them back; draws into a window; later passes sample what earlier ones drew; depth-only passes; textures and cube textures whose levels the caller gives, half-precision floats among their formats |
| [`slime-render`](../crates/slime-render/README.md) | The renderer: draws instances of meshes with their materials, lit by directional, point and spot lights, seen from a camera, into render targets, and reports what a frame cost; built on `slime-gfx`'s types, without wgpu | `slime-gfx`, `glam`, `thiserror`, `tracing` | Meshes with tangents, glTF's metallic-roughness materials (five textures, masks, double-sided), a perspective camera, the depth test, up to 16 directional, point and spot lights in physical units (`KHR_lights_punctual`), a procedural sky computed on the CPU that surfaces reflect (image-based lighting) and that is drawn behind them with its sun's disc, the shadows of the first directional light that casts them (one shadow map, 3 × 3 PCF), mirrored instances drawn with their front faces, the scene drawn in HDR with its light exposed, an energy-conserving bloom, then tone mapped, the passes ordered by a render graph and recorded four to a submission, statistics with the GPU time of each pass |
| [`slime-devui`](../crates/slime-devui/README.md) | The dev overlay: egui drawn over the engine's frames, fed with the engine's events; temporary tooling until the editor's UI framework exists | `egui`, `egui-wgpu`, `slime-gfx` (with its `wgpu` feature), `slime-platform` | Panels over a frame; no clipboard or input methods |
| [`slime-examples`](../examples/README.md) | Runnable examples; not part of the engine | `slime-app`, `slime-devui`, `slime-gfx`, `slime-platform`, `slime-render`, `gltf`, `png`, `tracing` | `hello`, `headless`, `window`, `offscreen`, `scene`; the reference image tests; a temporary glTF loader |

## Dependency rules

```text
slime-examples ──▶ slime-app ──┬──▶ slime-platform ──▶ winit
       │                       └──▶ slime-gfx ──────▶ wgpu
       ├──▶ slime-render ──────────▶ slime-gfx
       ├──▶ slime-devui ───┬───────▶ slime-gfx (feature wgpu) ──▶ wgpu
       │                   ├───────▶ egui, egui-wgpu ──────────▶ wgpu
       │                   └───────▶ slime-platform (events)
       └──▶ slime-platform, slime-gfx (their types: events, GPU objects)
```

- Dependencies between crates point one way: `slime-app` depends on `slime-platform` and
  `slime-gfx`, never the reverse, and `slime-platform` and `slime-gfx` do not depend on each
  other: the window reaches the GPU layer only as `raw-window-handle` traits.
- `slime-render` depends on `slime-gfx` only and does not use wgpu
  ([ADR-0027](adr/0027-the-render-crate.md)). `slime-app` does not depend on it: an application
  creates its `Renderer` in `init`, from the GPU of its `Context`, and calls it in `render`.
- `slime-devui` is the one crate besides `slime-gfx` that reaches wgpu, through the `wgpu` feature
  of `slime-gfx`, to draw egui with egui's own renderer ([ADR-0031](adr/0031-dev-overlay-with-egui.md)).
  The engine's own tests may enable the feature too, as `slime-app`'s windowed lifecycle test
  does to destroy the GPU device. `slime-devui` is tooling, a client of the engine like the
  examples: no engine crate depends on it, and
  games do not enable the feature.
- Only `slime-platform` uses winit, and winit's types never appear in a public API
  ([ADR-0011](adr/0011-windowing-with-winit.md)). `slime_platform::Window` exposes the
  `raw-window-handle` traits so that graphics libraries can draw into it.
- Only `slime-gfx` uses wgpu, and wgpu's types never appear in a public API
  ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)), except through its `wgpu` feature, for the dev
  overlay (above). `slime-gfx` depends on no other engine crate, so that it works without a
  windowing system.
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
    ├── surface.acquire(label)       the window's image, its passes named "window, frame
    │                                <index>", or none: skip drawing, after an empty submission
    │                                that lets wgpu release the frame's writes; span "acquire"
    ├── app.render(ctx, target)      draws into the image; span "render"
    └── image.present()              hands it to the window; span "present"
```

The run stops before the next frame once the application has called `ctx.request_exit()` or
`ctx.exit_with_error(error)`, the window was closed, the process received a termination signal, or
`config.frame_limit` frames have run. The frame in progress always finishes, and `app.shutdown` is
always called after an `app.init` that succeeded; when `app.init` returns an error, the run stops
at once and `run` returns it as `RunError::Init`, and an error passed to `exit_with_error` is
returned as `RunError::App` once `app.shutdown` has run. The call order is checked by
[`crates/slime-app/tests/lifecycle.rs`](../crates/slime-app/tests/lifecycle.rs) headless, and by
[`crates/slime-app/tests/windowed_lifecycle.rs`](../crates/slime-app/tests/windowed_lifecycle.rs)
with a window, which the checks run in a headless Weston compositor (ADR-0024). The profiling
spans of each frame are checked by
[`crates/slime-app/tests/profiling_spans.rs`](../crates/slime-app/tests/profiling_spans.rs)
headless, and by the `spans` scenario of `windowed_lifecycle.rs` with a window.

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
    ├── winit "window_event"     Translator::translate (event.rs) → driver.event
    │                            → Resized: surface.resize; app.on_event(ctx, &event);
    │                              CloseRequested → stop
    ├── winit "window_event"     RedrawRequested → driver.redraw_requested → frame with the
    │                            measured delta
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
lavapipe on Linux, WARP on Windows; the checks force lavapipe on machines that have a GPU by
restricting the Vulkan loader to it with `VK_DRIVER_FILES`. Headless runs never open the GPU.

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
├── create_render_target(label, width, height, format)   the image to draw into, 8-bit or
│                                                        Rgba16Float; later passes can sample it
├── create_depth_target(label, width, height)            the depth of what is drawn, for passes
│                                                        that test depth; later passes can sample
│                                                        it (a shadow map)
├── create_shader_module(label, wgsl)                    rejected → GfxError::Shader
├── create_bind_group_layout(label, entries)             what the shaders read: uniform buffers,
│                                                        textures and render targets, depth
│                                                        targets, samplers, comparison samplers
├── create_render_pipeline(&descriptor)                  rejected → GfxError::Pipeline; declares
│                                                        its vertex layout (or none: vertices from
│                                                        the vertex index), its fragment stage (or
│                                                        none: depth only), its bind group layouts,
│                                                        depth test and bias, culling
├── create_vertex_buffer(label, bytes)
├── create_index_buffer(label, indices)
├── create_uniform_buffer(label, bytes)                  written again with write_uniform_buffer
├── create_texture(label, width, height, format, pixels) an image that shaders sample, its levels
│                                                        of detail computed from its 8-bit pixels
├── create_texture_with_levels(label, width, height, format, levels)
│                                                        the same with the caller's levels, in any
│                                                        format (Rgba16Float: f32_to_f16)
├── create_cube_texture(label, size, format, levels)     six faces per level, read by direction
├── create_sampler(label, &descriptor)                   how they read it
├── create_comparison_sampler(label, compare, filter)    how they compare a depth with a depth
│                                                        target's (shadow lookups, PCF)
├── create_bind_group(label, &layout, entries)           the resources, bound together
├── uniform_offset_alignment()                           the step of a bind group's dynamic
│                                                        offsets into a uniform buffer
├── max_texture_dimension()                              the largest side of a texture or a
│                                                        target (8192 with the default limits)
├── max_buffer_size()                                    the largest buffer (256 MiB with the
│                                                        default limits)
├── create_timer(label, passes)                          a GpuTimer for up to `passes` passes of a
│                                                        submission, when can_time() says the
│                                                        adapter has timestamp queries
├── render(&RenderPassDescriptor { target, clear, depth, timer }, |pass| ...)
│                                                        one command encoder and render pass; the
│                                                        closure sets the pipeline, the bind groups
│                                                        and the buffers, and draws; then submitted,
│                                                        timed by the timer when there is one
├── render_depth(&DepthPassDescriptor { depth, timer }, |pass| ...)
│                                                        a pass without a render target: clears the
│                                                        depth target, draws depth-only pipelines
├── submit(label, |commands| ...)                        several passes (commands.render,
│                                                        commands.render_depth) in one command
│                                                        encoder, then one submission; a timer's
│                                                        passes numbered in recording order
├── read_render_target(&target)                          copy into a buffer, wait for the GPU,
│                                                        map it → Image (8-bit RGBA, top row first)
└── read_float_render_target(&target)                    an Rgba16Float target → FloatImage (f32)
```

A frame of several passes runs them one after another: each submitted by its `render` or
`render_depth`, or several in one submission (`submit`), which costs the CPU less
([ADR-0041](adr/0041-render-graph.md)). A later pass binds the targets an earlier one drew into
(`BindingResource::RenderTarget`, `BindingResource::DepthTarget`). A pass cannot sample what it
draws into: wgpu refuses it and the engine panics
([ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)).
[`crates/slime-gfx/tests/passes.rs`](../crates/slime-gfx/tests/passes.rs) shows each case.

Each GPU object is an engine-owned type holding its wgpu object
([ADR-0017](adr/0017-gpu-resource-types.md); bind groups and depth:
[ADR-0026](adr/0026-resource-binding-and-depth.md); float and sampled targets, depth-only passes:
[ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)). Shader modules and pipelines are
created inside a wgpu error scope, so that a rejection comes back as an error with wgpu's message,
and every creation inside an out-of-memory scope, so that memory that runs out comes back as
`GfxError::OutOfMemory` and the caller can react
([ADR-0044](adr/0044-running-out-of-gpu-memory.md)); any other wgpu error is a bug and panics
([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)). The test
[`examples/tests/offscreen.rs`](../examples/tests/offscreen.rs) compares the image with a
reference PNG within a tolerance ([ADR-0018](adr/0018-reference-image-tests.md)).

### Rendering a scene

`slime-render` draws meshes with the objects above. The application builds a `Scene` for each
frame, a flat list of instances seen from a camera, and the renderer draws it into any render
target, the window's image or an offscreen one, in passes: the shadow map, the scene in high
dynamic range, the sky behind it, the bloom's chain, then tone mapping into the target, ordered
and recorded by a render graph
([`crates/slime-render/src/renderer.rs`](../crates/slime-render/src/renderer.rs),
[`crates/slime-render/src/graph.rs`](../crates/slime-render/src/graph.rs), rules:
[ADR-0027](adr/0027-the-render-crate.md), [ADR-0034](adr/0034-hdr-rendering-and-tone-mapping.md),
[ADR-0035](adr/0035-physically-based-materials.md),
[ADR-0037](adr/0037-lights-in-physical-units-and-pre-exposure.md),
[ADR-0038](adr/0038-shadow-mapping.md),
[ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md),
[ADR-0040](adr/0040-bloom.md), [ADR-0041](adr/0041-render-graph.md)):

```text
renderer = Renderer::new(&gpu, format)                 the output's format; three shaders and nine
│                                                      pipelines: two depth-only ones for the
│                                                      shadow pass (the second discards what
│                                                      masked materials discard), three for the
│                                                      scene (depth test, into Rgba16Float),
│                                                      culling back faces, front faces or none,
│                                                      the sky's, the bloom's two (down and up)
│                                                      and the tone-mapping one (each a
│                                                      fullscreen triangle); the BRDF table,
│                                                      computed once per process; a black
│                                                      environment; the 2048 × 2048 shadow map
│                                                      and the group that reads it, bind group 3
├── create_environment(&gpu, &Sky)                     the sky drawn on the CPU into a 128-texel
│                                                      cube, prefiltered into four levels on
│                                                      every core (environment.rs), uploaded in
│                                                      Rgba16Float; its sun's disc
├── create_mesh(&gpu, label, &MeshData)                vertex and index buffers, after a check;
│                                                      48-byte vertices with their tangent; the
│                                                      box around the positions
├── create_material(&gpu, label, &MaterialData)        a 48-byte uniform buffer and five texture
│                                                      and sampler pairs (one white pixel for
│                                                      each slot left empty) in a bind group; a
│                                                      texture in the wrong format for its slot
│                                                      panics
└── render(&gpu, &target, &Scene { camera, lights, environment, exposure, bloom, instances })
    │                                                  → RenderStats, or the error of a creation
    │                                                  (ADR-0044); draws the first 16 lights
    ├── the light-space matrix of the first directional light that casts shadows: an orthographic
    │   projection covering the instances' boxes as that light sees them
    ├── the frame uniform ← camera.view_projection(aspect), the sky's directions from clip space,
    │   the camera's position, the exposure, the light-space matrix, the number of lights and the
    │   shadowed light's index, the sun's disc (direction, radiance), and the lights (64 bytes
    │   each, at most 16); bind group 0, with the environment, the BRDF table and their sampler,
    │   built again when the scene names another environment (a black one for none)
    ├── the instance buffer ← each instance's transform, normal        bind group 2, one dynamic
    │   matrix and handedness (−1 when it mirrors), one slot each, at  offset per instance
    │   the device's alignment; grows when a scene needs it, shrinks back after a peak
    ├── the frame's render graph, built again when the frame's shape changes (the target's size,
    │   the shadow pass or the bloom starting or stopping): the passes below, each declaring the
    │   targets it draws into and samples, ordered by them; the HDR target (Rgba16Float), the depth
    │   target and, while the bloom runs, its levels (the output divided by 2^i, rounded to the
    │   nearest, to a side of 8 texels), created at the target's size; the groups that read them,
    │   built from the passes' declarations; a GpuTimer per submission
    ├── the passes, in the graph's order, four to a submission named after the target, each timed
    │   by its submission's GpuTimer when the adapter can:
    │   ├── the shadow pass into the shadow map, a depth-only pass skipped when no light casts
    │   │   shadows: for each instance, the shadow pipeline its material asks for (masked or not),
    │   │   its bind groups and buffers, draw_indexed
    │   ├── the scene pass into the HDR target, cleared to black, the shadow map's group bound at
    │   │   group 3: for each instance, the pipeline that its material's double-sidedness and its
    │   │   transform's mirroring choose (set when it changes), its material's bind group 1, its
    │   │   slot, its buffers, draw_indexed
    │   ├── the sky pass into the HDR target, keeping it and the depth target: one triangle at the
    │   │   far plane, drawn where the depth test finds no surface, reads the environment along
    │   │   each pixel's direction, adds the sun's disc, times the exposure
    │   ├── unless the scene's bloom is 0: the bloom's passes (bloom.wgsl): each level filtered
    │   │   down from the one above with Jimenez's 13 taps, then each averaged up with the coarser
    │   │   average through a 3 × 3 tent, every level with the same weight, every read mirrored at
    │   │   the edges; no pass reads what it writes
    │   └── the tone-mapping pass into the target: one fullscreen triangle reads each pixel's texel
    │       (textureLoad), mixes in the bloom's first average by the scene's strength, multiplies
    │       by 64, applies Khronos PBR Neutral and writes it opaque; an sRGB target encodes it
    └── RenderStats: the scene pass's draw calls and triangles, the shadow pass's draw calls, the
        CPU time of this call, and the GPU time of each pass, the bloom's summed, a few frames old,
        the latest that came back
```

The dev overlay ([`crates/slime-devui/src/lib.rs`](../crates/slime-devui/src/lib.rs),
[ADR-0031](adr/0031-dev-overlay-with-egui.md)) draws after the tone-mapping pass, in the output,
in a pass and a submission of its own that keep the target: the application hands it every window
event, which it translates into egui's, and in `render` builds its panels with egui and lets
egui's wgpu renderer draw them, through the wgpu objects that the `wgpu` feature of `slime-gfx`
gives.

The shader ([`shader.wgsl`](../crates/slime-render/src/shader.wgsl)) transforms each vertex by its
instance's matrix then the camera's, and its normal by the instance's normal matrix (the cofactor
matrix of its 3 × 3 part, proportional to the inverse transpose, so that normals stay perpendicular
to surfaces scaled unevenly, and finite for a mesh flattened along one axis); each fragment is
shaded with glTF's metallic-roughness model (ADR-0035): the GGX distribution, Smith's visibility and
Schlick's Fresnel under each of the scene's lights (ADR-0037), whose illuminance at the fragment is
a directional light's illuminance, or a point or spot light's intensity over the square of the
distance (as at 1 cm within 1 cm), brought to zero at its range and, for a spot light, times its
cone's attenuation, as `KHR_lights_punctual` recommends; the shadowed light times the share of the
shadow map's 3 × 3 comparisons, one texel apart, that find the fragment lit, at the position in the
map that the vertex shader computes as the shadow pass does (ADR-0038); the environment's light
(ADR-0039), the diffuse light read from its roughest level along the normal and the specular light
from the level of the roughness along the reflection, weighted by the split sum's Fresnel from the
BRDF table with the light that rough surfaces scatter more than once, times the occlusion; the
emission added; the normal tilted by the normal texture in the frame of the tangent, which follows
the model matrix, and of its bitangent, rebuilt per fragment; back faces of double-sided materials
shaded with the normal reversed; fragments of masked materials below their cutoff discarded; the sum
multiplied by the exposure and divided by 64, so that half precision holds light up to 64 × 65 504
for the bloom (ADR-0040), and brighter light written as 65 504. The material's textures are read at
the texture coordinates of the vertex. Its sky stages turn each pixel of the far plane into a
direction, read the environment's first level along it and add the sun's disc, its edge blended over
a pixel. The bloom's shader ([`bloom.wgsl`](../crates/slime-render/src/bloom.wgsl)) filters that
light down and averages it back up into a blurred copy, which the tone-mapping shader
([`tone_mapping.wgsl`](../crates/slime-render/src/tone_mapping.wgsl)) mixes in before it maps the
light to the display's range. The examples' temporary glTF loader
([`examples/common/gltf.rs`](../examples/common/gltf.rs),
[ADR-0029](adr/0029-temporary-gltf-loading.md), ADR-0035, ADR-0037) builds `MeshData` (tangents read
or computed), materials and textures, each in its slot's format, and the lights of
`KHR_lights_punctual` from a GLB file, and flattens its node hierarchy into instances and lights;
mirrored nodes keep their front faces through the renderer. Conventions: world space is right-handed
with +Y up, distances are in metres and light in physical units (lux, candelas, and nits for the
light of surfaces and the sky), the camera looks down −Z, front faces wind counter-clockwise (a
mirrored instance keeps its front faces), tangents follow glTF, depth runs from 0 to 1, colors are
linear light, drawn in high dynamic range, exposed in the scene pass (ADR-0037), tone mapped
(ADR-0034), then encoded by the output's format.

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
| Image not ready in time (Vulkan), window occluded | No image this frame; on Direct3D 12, wgpu waits up to one second for a late image, then returns it, and the frame is drawn late; on Windows with Vulkan, an image that wgpu handed out but that is still not free after one second loses the device, and the run stops |
| Surface lost, or a configuration the graphics API refuses | Created again from the window; no image this frame. Lost for good (it cannot be created again, or is lost again each time): no image, and the run stops before the next frame, like after a lost device |
| Device lost (see [Losing the GPU](#losing-the-gpu)) | No image; the run stops before the next frame. An image acquired before the loss was found is released instead of presented, so that the surface can be dropped |

How the window shows the images is the *present mode* (`AppConfig::present_mode`). With Mailbox,
the default, presenting never waits, and the frame scheduler alone paces frames, whether the
window is visible or not; windows that do not offer it fall back to FIFO, with which presenting
can block a frame. The measurements behind this choice are in
[ADR-0021](adr/0021-frame-pacing.md).

In GPU capture tools such as RenderDoc, each render pass is named after its target, so the window's
pass of frame 42 appears as `window, frame 42`, and `RenderPass::debug_group` names groups of
commands inside it; the samples' triangle is drawn in a group named `triangle`. On Vulkan, debug
builds pass these names to the driver, and release builds do with `WGPU_DEBUG=1`; Direct3D 12
receives them in every build ([ADR-0016](adr/0016-gpu-layer-with-wgpu.md)).

### Losing the GPU

The driver can lose the device: it crashes or is reset, the GPU is removed, or it runs out of
memory. Memory that runs out in a creation that no caller hears of, made by a library built on
wgpu such as egui's renderer, gives the device up the same way
([ADR-0044](adr/0044-running-out-of-gpu-memory.md)). `slime-gfx` records the loss in wgpu's
device-lost callback, and from then on its error handler logs wgpu's errors instead of panicking,
since every use of the device fails, and creations return `GfxError::DeviceLost`. Before each
frame, the windowed driver checks `Gpu::check()` with the other reasons to stop: after a loss, the
run stops like after an exit request, and `run` returns `RunError::Gfx` with
`GfxError::DeviceLost` ([ADR-0022](adr/0022-losing-the-gpu-device.md)). The window's surface can be
lost for good as well, when it cannot be created again after a loss or is lost again each time it
is: `Surface::check` reports it, and the run stops the same way with `GfxError::SurfaceLost`
([ADR-0025](adr/0025-giving-up-a-lost-surface.md)).

### Termination signals

`run` installs, once per process, a handler for SIGINT (Ctrl+C), SIGTERM and SIGHUP
([`crates/slime-app/src/signal.rs`](../crates/slime-app/src/signal.rs)). The handler runs on a
thread of its own: it sets a flag and hands its log line to another thread, so that a blocked
standard error cannot keep a second signal from being handled; before each frame, the driver
records the flag in the `Context` as an exit request, so the run stops like any other: the frame
in progress finishes and `app.shutdown` runs. A second signal, half a second or more after the
first, ends the process at once with exit code 130; one that comes sooner (a closing terminal sends
SIGHUP twice) is ignored
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

### Frame timing

`slime_app::time` has no clock of its own: the driver gives `FrameClock` the duration of each
frame (measured, or simulated in headless runs), gets a `FrameTime` back, and gives its delta to
`FixedStep`, which says how many fixed steps the frame must run. Source:
[`crates/slime-app/src/time.rs`](../crates/slime-app/src/time.rs); rules:
[ADR-0009](adr/0009-frame-timing.md).

### Threads

The engine runs on one thread today: `run` drives the application from the main thread, and the
renderer records and submits its passes on the thread that calls it. What may cross threads is
nonetheless stated, so that later stages (loading on a task pool, a render thread, the job system)
can rely on it, and checked when each crate's tests compile (a `threads` module in its
`lib.rs`, or in `slime-devui`'s tests):

- **Shared between threads** (`Send` and `Sync`): the `Gpu`, its `Surface` and every GPU object
  it creates (targets, textures, samplers, buffers, bind groups and their layouts, shader modules,
  pipelines), images read back, meshes, materials, environments, cameras, lights, scenes and
  instances, statistics, windows, events, the application's `Context` and `AppConfig`, the dev
  overlay, and every error type. wgpu keeps its error scopes per thread, so shaders and pipelines
  can be created from several threads.
- **Moved between threads, used by one at a time** (`Send` only): a `GpuTimer`, which reads its
  times back through a cell, and so the `Renderer` that holds them; `Renderer::render` takes it
  mutably anyway.

## Platforms

The engine targets Linux (`x86_64-unknown-linux-gnu`) and Windows (`x86_64-pc-windows-msvc`)
([ADR-0015](adr/0015-platforms-and-ci.md)). Platform differences are kept in few places:

- winit hides the windowing systems (Wayland and X11 on Linux, Win32 on Windows).
  `slime-platform` has platform-specific code in three places of `event_loop.rs` and one of
  `event.rs`:
  - `backend_name` names the windowing system in the log.
  - On X11, `resumed` gives winit a maximum window size of 65 535 physical pixels, the protocol's
    limit; beyond it, winit panics.
  - On Windows, `exiting` hides the window, which winit destroys only from inside the event loop.
  - On Windows, `Translator::translate` releases both Shift keys on a key-up of either, since
    Windows sends a single key-up for the two.

  Where winit's events differ between platforms, `slime-platform` makes them agree: `Translator` in
  `event.rs` pairs key and mouse button events across focus changes. One difference shapes the main
  loop: on Windows, the system runs its own loop while the user resizes or moves a window, which is
  why frames run on redraw requests.
- `ctrlc` hides signals (Unix) and console events (Windows); `crates/slime-app/src/signal.rs`
  describes how each platform behaves.
- wgpu hides the graphics APIs; `slime-gfx` only chooses which one to ask for on each platform
  (`DEFAULT_BACKENDS` in `crates/slime-gfx/src/gpu.rs`).
- On Windows, `slime-app` asks the console to interpret the escape sequences that color its log,
  through `nu-ansi-term`, and colors it only when the console accepts (`use_colors` in
  `crates/slime-app/src/diagnostics.rs`): Windows Terminal interprets them already, and the older
  console host once asked.
- Tests that cannot run on a platform are compiled out with `#[cfg]`, and their file says why: the
  signal tests run on Unix only, and the two windowed test programs of `slime-app`
  (`windowed_errors.rs` and `windowed_lifecycle.rs`) on Linux only; having no test harness, they
  keep a `main` that does nothing elsewhere.

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
