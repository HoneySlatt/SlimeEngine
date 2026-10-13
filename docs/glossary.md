# Glossary

Terms that this project defines, or uses with a narrower meaning than usual. Standard Rust and
game-development vocabulary is not repeated here. Entries are in alphabetical order.

**Adapter.** A GPU as wgpu sees it, through one graphics API: a discrete or integrated GPU, or a
software driver such as lavapipe or WARP. The engine opens one adapter's *device* to use it.

**ADR (Architecture Decision Record).** A short document in [`docs/adr/`](adr/README.md) that
records one decision: its context, the decision, the alternatives rejected and the consequences.

**Application time.** The sum of the deltas of all frames so far (`FrameTime::elapsed`). It
falls behind wall-clock time when a frame delta is limited, for example after a pause.

**Bind group.** A set of GPU resources that a pipeline's shaders read together, bound at one
`@group` index: uniform buffers, textures and samplers (`slime_gfx::BindGroup`). Its
*bind group layout* says which bindings it has and what each holds (`slime_gfx::BindGroupLayout`,
[ADR-0026](adr/0026-resource-binding-and-depth.md)).

**Checks (the).** The automated checks of the *definition of done*, which `scripts/check.sh` runs
and the Linux CI job runs unchanged: formatting, clippy (for Windows too), the build, the tests on
lavapipe, rustdoc, the dependency rules, the samples without a display server and in a headless
Weston compositor, and the links between documents ([`CONTRIBUTING.md`](../CONTRIBUTING.md#checks)).

**Client of the engine.** A crate or program built on the engine's public API only, which no engine
crate depends on: the examples, the dev *overlay* (`slime-devui`) and, later, the editor.

**Comparison sampler.** A sampler that compares a reference depth with the depths stored in a
*depth target* instead of reading them: each pixel read gives 1 where the comparison holds and 0
where it does not, and linear filtering blends the four nearest results
(`slime_gfx::ComparisonSampler`, [ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)).
Shadow maps are read this way. See *percentage-closer filtering*.

**Culling.** Not drawing the triangles that face one way, usually the back faces of closed
meshes, which their front faces hide anyway (`slime_gfx::CullMode`). A triangle faces the front
when its vertices wind counter-clockwise on screen.

**Debug group.** A named section of the commands of a render pass, shown by GPU capture tools such
as RenderDoc (`RenderPass::debug_group`).

**Definition of done.** The conditions a milestone must meet before it is reported as finished,
listed in [`AGENTS.md`](../AGENTS.md#definition-of-done).

**Depth bias.** An offset that a pipeline adds to the depth of what it draws before the depth
test: a constant part, and a part proportional to how steeply the triangle's depth changes from
pixel to pixel (`slime_gfx::DepthBias`). A *shadow map* is drawn with one, so that a surface does
not shadow itself where its depth and the stored one differ only by rounding, which would cover
it with dark specks (*shadow acne*).

**Depth-only pass.** A render pass without a *render target*, which writes depth only into a
*depth target*, with pipelines that have no fragment stage (`Gpu::render_depth`): how a *shadow
map* is drawn.

**Depth target.** A texture holding, for each pixel, the depth of what was drawn there, with
which a render pass keeps the nearest surface whatever the order the surfaces are drawn in: the
*depth test* (`slime_gfx::DepthTarget`, `slime_gfx::DepthState`). Later passes can sample it, as
a *shadow map* is read.

**Device (GPU).** The engine's connection to an adapter: it creates GPU resources, and its
*queue* receives the work to execute (`slime_gfx::Gpu`). It is *lost* when the driver crashes or
is reset, the GPU is removed, or it runs out of memory: its objects can then no longer be used,
and the run stops ([ADR-0022](adr/0022-losing-the-gpu-device.md)).

**Driver.** The loop that decides when frames run and what time each one represents. `run` has a
headless driver (frames back to back, simulated time) and a windowed driver, called back by the
windowing library's event loop, which measures real time, asks for frames at a limited rate and
runs them on *redraw requests*. All drivers run frames the same way
([ADR-0010](adr/0010-main-loop-ownership.md)). Not to be confused with a GPU driver.

**Dynamic offset.** The start of the range of a *uniform buffer* that a *bind group* binds, given
each time the group is bound (`RenderPass::set_bind_group`) rather than when the group is created,
as a multiple of `Gpu::uniform_offset_alignment`. One buffer and one group then serve many draws,
one range each: the renderer's instances ([ADR-0027](adr/0027-the-render-crate.md)).

**Engine-owned type.** A type of an engine crate that holds an object of a third-party library and
exposes only what the engine needs, so that the library's types never appear in a public API: the
GPU objects of `slime_gfx` hold wgpu's, `slime_platform::Window` holds winit's
([ADR-0011](adr/0011-windowing-with-winit.md), [ADR-0016](adr/0016-gpu-layer-with-wgpu.md),
[ADR-0017](adr/0017-gpu-resource-types.md)).

**Example.** A runnable program in the `examples/` directory that demonstrates one feature.

**Exposure.** What the scene's light is multiplied by before *tone mapping*
(`slime_render::Scene::exposure`): a linear factor; 1 leaves it as it is, 2 is one photographic
stop brighter. The tone-mapping pass applies it
([ADR-0034](adr/0034-hdr-rendering-and-tone-mapping.md)).

**Fixed step.** A duration of time, constant for a run, in which code that must not depend on
the frame rate advances (`slime_app::time::FixedStep`). A frame runs zero or more fixed steps.

**Frame.** One iteration of the main loop. Its *delta* is the time it represents: the time since
the previous frame, limited to a maximum (`slime_app::time::FrameClock`).

**Frame scheduler (frame limiter).** The part of the windowed driver that schedules frames and
waits between them, so that the engine asks for at most `AppConfig::max_frame_rate` frames per
second on average. With the default *present mode*, it alone paces the frames
([ADR-0021](adr/0021-frame-pacing.md), which calls it the timer, or the schedule).

**Fullscreen triangle.** One triangle larger than the target, which covers every pixel of it,
built by the vertex shader from the vertex index without a vertex buffer: how a pass that
processes a whole image, such as tone mapping, draws.

**glTF.** The Khronos Group's format for 3D scenes: meshes, materials, textures and a node
hierarchy, as a JSON file with binary buffers, or as one GLB file with everything embedded. The
examples load GLB files with a temporary loader ([ADR-0029](adr/0029-temporary-gltf-loading.md));
the asset pipeline of Stage 4 will import them.

**GPU time.** How long the GPU took to run a render pass, measured by timestamps written at its
start and its end (`slime_gfx::GpuTimer`) and read back a few frames later; `None` on adapters
without timestamp queries. Each timed pass has a `GpuTimer` of its own. The renderer's statistics
report one per pass (`scene_gpu_time`, `tone_mapping_gpu_time`) with the CPU time of the call
(`slime_render::RenderStats`).

**Graphics API.** The interface between programs and GPU drivers: Vulkan (the engine's choice on
Linux) and Direct3D 12 (on Windows). wgpu translates the engine's calls into one of them.

**Half-precision float.** A 16-bit floating-point number (IEEE 754 binary16): 11 significant bits,
values up to ±65 504. The channels of `Rgba16Float` render targets, which the engine converts to
`f32` by hand when it reads them back (`Gpu::read_float_render_target`).

**Headless.** Running the engine without a window, renderer or audio output. It is a normal
configuration, used by automated tests and, later, by dedicated servers.

**High dynamic range (HDR).** Colors not limited to 0 to 1, such as the light a scene reflects
before it is mapped to the display's range by *tone mapping*. The engine draws them into
`Rgba16Float` render targets: the renderer draws the scene into one at the output's size, which
its tone-mapping pass reads.

**Image test.** A test that renders offscreen and compares the result with a *reference image*,
pixel by pixel, within a tolerance that absorbs how drivers round colors and filter textures
([ADR-0018](adr/0018-reference-image-tests.md), amended by
[ADR-0028](adr/0028-image-tests-tolerate-a-few-pixels.md) and
[ADR-0030](adr/0030-image-tests-strict-on-the-reference-driver.md)).

**Index buffer.** A buffer of indices into a vertex buffer, so that the triangles of a mesh share
vertices (`slime_gfx::IndexBuffer`).

**Instance (render).** One thing the renderer draws: a *mesh*, a *material* and a transform from
the mesh's space to world space (`slime_render::Instance`). A render *scene* is a flat list of
them, built for each frame.

**Interpolation factor.** The fraction of a fixed step accumulated but not yet run
(`FixedStep::alpha`), between 0 and 1. Used to draw a state between the last two fixed steps.
`run` does not pass it to applications yet.

**Label (GPU object).** The name given to a GPU object when it is created, required by every
creation method of `slime_gfx::Gpu`. Error messages and GPU capture tools show it.

**lavapipe.** Mesa's Vulkan driver that renders on the CPU. The checks and CI use it on Linux, so
that GPU tests give the same results on every machine, including machines without a GPU. It is
the *reference driver* of the image tests.

**Logical key.** What a key means with the current keyboard layout and modifiers
(`slime_platform::Key`): on a French AZERTY keyboard, the first letter key of the top row gives
the logical key `"a"`. Suits shortcuts and text. See also *physical key*.

**Logical pixel.** A pixel of the window as the user sees it; the *scale factor*
(`Window::scale_factor`, `Event::ScaleFactorChanged`) says how many *physical pixels*, the
display's, it covers. The sizes of `WindowConfig` are logical; `Window::inner_size`,
`Event::CursorMoved` and render targets are physical. egui calls logical pixels *points*.

**Material.** How a surface looks: a base color, which a texture read with a *sampler* may
modulate, and which the scene's light multiplies (`slime_render::Material`, from
`MaterialData`).

**Mesh.** Triangles given as vertices (a position, a normal and texture coordinates each) and
indices, uploaded to the GPU (`slime_render::Mesh`, from `MeshData`).

**Milestone.** A small, verifiable step of a stage, listed with its goal, its done criteria and
how to test it in [`docs/PLAN.md`](PLAN.md). Milestones are numbered `M<stage>.<n>`, for example
M1.3.

**Mipmaps.** Copies of a texture at half, a quarter, ... of its size, down to one pixel: the
*levels of detail* among which a *sampler* chooses by the size the texture is drawn at, so that a
minified texture does not alias. The engine computes them on the CPU when the texture is created
(`Gpu::create_texture`), in linear light for the sRGB formats; linear filtering blends two levels.

**Normal matrix.** The matrix by which the renderer transforms an instance's normals so that they
stay perpendicular to surfaces scaled unevenly: the cofactor matrix of the transform's 3 × 3 part,
proportional to its inverse transpose, which, unlike the inverse, stays finite for a mesh
flattened along one axis.

**Offscreen rendering.** Drawing into an image that no window shows, a *render target*, for
example to read it back to the CPU. Image tests render this way, without a windowing system.

**Overlay (dev).** Panels drawn over the engine's frames with egui, to show statistics and
tooling while the engine has no UI framework of its own (`slime_devui::Overlay`,
[ADR-0031](adr/0031-dev-overlay-with-egui.md)). Temporary: the editor's UI framework of Stage 6
replaces it.

**Owner.** The person who validates the project's direction and its architecture decisions.

**Percentage-closer filtering (PCF).** Reading a *shadow map* by comparing a point's depth with
several stored depths around it and averaging the results, so that the edges of shadows are soft
instead of blocky. A *comparison sampler* with linear filtering does it over 2 × 2 pixels in the
GPU.

**Physical key.** The position of a key on the keyboard, whatever the layout
(`slime_platform::KeyCode`), named after the character it produces on a US keyboard: the first
letter key of the top row is `KeyQ` on every layout, whether it is labelled Q (QWERTY) or A
(AZERTY). Suits movement keys. See also *logical key*.

**Physical pixel.** A pixel of the display. See *logical pixel*.

**Present (an image).** Hand a drawn image to the window, which shows it at one of the display's
next refreshes, as the *present mode* says.

**Present mode.** How a window shows the images presented to it (`slime_gfx::PresentMode`). With
*Mailbox*, the engine's default, each image replaces the one waiting to be shown and presenting
never waits; with *FIFO*, every image is shown in order, one per refresh, and presenting waits
while earlier images are still to be shown ([ADR-0021](adr/0021-frame-pacing.md)).

**Redraw request.** A notice from the windowing system that the window should be drawn now:
either the engine asked for it (`Window::request_redraw`), or the system did on its own, for
example during a resize. The windowed driver runs a frame for each one
([ADR-0020](adr/0020-rendering-in-the-main-loop.md)).

**Reference driver.** The driver that rendered the *reference images* and is held to the strict
tolerance of the *image tests*: lavapipe, recognized by its adapter name
([ADR-0030](adr/0030-image-tests-strict-on-the-reference-driver.md)).

**Reference image.** A PNG file stored in the repository that an *image test* compares its render
with. The references are rendered with lavapipe and rendered again only on purpose: after a
deliberate change of what the test draws, or after an update of lavapipe that changes them
([ADR-0030](adr/0030-image-tests-strict-on-the-reference-driver.md)).

**Render target.** An image the GPU draws into (`slime_gfx::RenderTarget`): an offscreen image,
which can be read back to the CPU and sampled by later passes, or the image of a window for one
frame, which can be neither.

**Sample.** The example that demonstrates the exit criterion of a stage. Every stage ends with
one.

**Sampler.** How shaders read a texture: nearest or linear filtering, and what lies beyond its
edges (repeat, mirror or clamp) (`slime_gfx::Sampler`).

**Scene (render).** What the renderer draws in one frame: a camera, a directional light, a
clear color, an *exposure* and *instances* (`slime_render::Scene`). Not a scene graph: the
renderer keeps nothing between frames, and the world model of Stage 5 will produce such scenes.

**Shadow map.** A *depth target* drawn from a light's point of view by a *depth-only pass*: a
point is in shadow where something nearer to the light is stored at its place in the map.

**Simulated time.** Frame deltas supplied as a constant duration instead of being measured, so
that a run gives the same results on every machine. Headless runs use it.

**Skeleton (crate).** A crate that exists with its documentation and inherits the workspace
policies, but holds no code yet.

**Spiral of death.** A fixed-step loop that falls further behind at each frame because running
the overdue steps takes longer than the time they cover. `FixedStep` avoids it by limiting the
steps per frame and dropping the rest.

**Stage.** A step of [`docs/ROADMAP.md`](ROADMAP.md), ordered by dependency, with a goal and an
exit criterion. Stages are numbered from 0; a stage may be split (Stage 3a and 3b).

**Surface.** The link between the GPU and a window: the images the window shows. Each frame, the
engine *acquires* one, the application draws into it, and the engine *presents* it
(`slime_gfx::Surface`). The graphics API can report it *lost*: it is then created again, and given
up, which stops the run, when that fails or when it is lost again each time
([ADR-0025](adr/0025-giving-up-a-lost-surface.md)).

**Termination signal.** A request from outside the process to stop it: SIGINT (Ctrl+C in a
terminal), SIGTERM (a service manager or `kill`) or SIGHUP (the terminal closing). The first one
stops a run cleanly; a second one ends the process at once
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

**Timestamp query.** A GPU feature that writes the GPU's clock at a point of its command stream;
two of them, around a render pass, give its *GPU time* (`slime_gfx::GpuTimer`). Optional: the
device asks for it only when the adapter offers it (`Gpu::can_time`,
[ADR-0031](adr/0031-dev-overlay-with-egui.md)).

**Tone mapping.** Mapping the *HDR* colors of a scene to the range a display shows, 0 to 1, with a
curve that compresses highlights instead of clipping them. The renderer's tone-mapping pass draws
a *fullscreen triangle* that applies Khronos PBR Neutral, the default of the Khronos glTF Sample
Viewer, which keeps authored base colors and hues
([ADR-0034](adr/0034-hdr-rendering-and-tone-mapping.md)).

**Tooling (engine's).** Code that serves development and ships with no game: the dev *overlay*
today. It alone may reach wgpu, through the `wgpu` feature of `slime-gfx`
([ADR-0031](adr/0031-dev-overlay-with-egui.md)).

**Uniform buffer.** A buffer of values that a whole draw reads, such as a camera's matrices,
declared `var<uniform>` in WGSL and read through a *bind group* (`slime_gfx::UniformBuffer`).

**WARP.** Microsoft's software driver for Direct3D 12, part of Windows. wgpu uses it on Windows
machines without a GPU, such as the CI runners.
