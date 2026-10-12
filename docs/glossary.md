# Glossary

Terms that this project defines, or uses with a narrower meaning than usual. Standard Rust and
game-development vocabulary is not repeated here. Entries are in alphabetical order.

**ADR (Architecture Decision Record).** A short document in [`docs/adr/`](adr/README.md) that
records one decision: its context, the decision, the alternatives rejected and the consequences.

**Adapter.** A GPU as wgpu sees it, through one graphics API: a discrete or integrated GPU, or a
software driver such as lavapipe or WARP. The engine opens one adapter's *device* to use it.

**Application time.** The sum of the deltas of all frames so far (`FrameTime::elapsed`). It
falls behind wall-clock time when a frame delta is limited, for example after a pause.

**Bind group.** A set of GPU resources that a pipeline's shaders read together, bound at one
`@group` index: uniform buffers for now, textures and samplers later (`slime_gfx::BindGroup`). Its
*bind group layout* says which bindings it has and what each holds (`slime_gfx::BindGroupLayout`,
[ADR-0026](adr/0026-resource-binding-and-depth.md)).

**Culling.** Not drawing the triangles that face one way, usually the back faces of closed
meshes, which their front faces hide anyway (`slime_gfx::CullMode`). A triangle faces the front
when its vertices wind counter-clockwise on screen.

**Debug group.** A named section of the commands of a render pass, shown by GPU capture tools such
as RenderDoc (`RenderPass::debug_group`).

**Definition of done.** The conditions a milestone must meet before it is reported as finished,
listed in [`AGENTS.md`](../AGENTS.md#definition-of-done).

**Depth target.** A texture holding, for each pixel, the depth of what was drawn there, with
which a render pass keeps the nearest surface whatever the order the surfaces are drawn in: the
*depth test* (`slime_gfx::DepthTarget`, `slime_gfx::DepthState`).

**Device (GPU).** The engine's connection to an adapter: it creates GPU resources, and its
*queue* receives the work to execute (`slime_gfx::Gpu`). It is *lost* when the driver crashes or
is reset, the GPU is removed, or it runs out of memory: its objects can then no longer be used,
and the run stops ([ADR-0022](adr/0022-losing-the-gpu-device.md)).

**Driver.** The loop that decides when frames run and what time each one represents. `run` has a
headless driver (frames back to back, simulated time) and a windowed driver, called back by the
windowing library's event loop, which measures real time, asks for frames at a limited rate and
runs them on *redraw requests*. All drivers run frames the same way
([ADR-0010](adr/0010-main-loop-ownership.md)). Not to be confused with a GPU driver.

**Example.** A runnable program in the `examples/` directory that demonstrates one feature.

**Fixed step.** A duration of time, constant for a run, in which code that must not depend on
the frame rate advances (`slime_app::time::FixedStep`). A frame runs zero or more fixed steps.

**Frame.** One iteration of the main loop. Its *delta* is the time it represents: the time since
the previous frame, limited to a maximum (`slime_app::time::FrameClock`).

**Frame scheduler (frame limiter).** The part of the windowed driver that schedules frames and
waits between them, so that the engine asks for at most `AppConfig::max_frame_rate` frames per
second on average. With the default *present mode*, it alone paces the frames
([ADR-0021](adr/0021-frame-pacing.md), which calls it the timer, or the schedule).

**Graphics API.** The interface between programs and GPU drivers: Vulkan (the engine's choice on
Linux) and Direct3D 12 (on Windows). wgpu translates the engine's calls into one of them.

**Headless.** Running the engine without a window, renderer or audio output. It is a normal
configuration, used by automated tests and, later, by dedicated servers.

**Image test.** A test that renders offscreen and compares the result with a *reference image*,
pixel by pixel, within a tolerance that absorbs how drivers round colors
([ADR-0018](adr/0018-reference-image-tests.md)).

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
that GPU tests give the same results on every machine, including machines without a GPU.

**Logical key.** What a key means with the current keyboard layout and modifiers
(`slime_platform::Key`): on a French AZERTY keyboard, the first letter key of the top row gives
the logical key `"a"`. Suits shortcuts and text. See also *physical key*.

**Material.** How a surface looks: a base color for now (`slime_render::Material`, from
`MaterialData`), which the scene's light multiplies. Textures join it later in Stage 3a.

**Mesh.** Triangles given as vertices (a position, a normal and texture coordinates each) and
indices, uploaded to the GPU (`slime_render::Mesh`, from `MeshData`).

**Milestone.** A small, verifiable step of a stage, listed with its goal, its done criteria and
how to test it in [`docs/PLAN.md`](PLAN.md). Milestones are numbered `M<stage>.<n>`, for example
M1.3.

**Offscreen rendering.** Drawing into an image that no window shows, a *render target*, for
example to read it back to the CPU. Image tests render this way, without a windowing system.

**Owner.** The person who validates the project's direction and its architecture decisions.

**Physical key.** The position of a key on the keyboard, whatever the layout
(`slime_platform::KeyCode`), named after the character it produces on a US keyboard: the first
letter key of the top row is `KeyQ` on every layout, whether it is labelled Q (QWERTY) or A
(AZERTY). Suits movement keys. See also *logical key*.

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

**Reference image.** A PNG file stored in the repository that an *image test* compares its render
with. The references are rendered with lavapipe and rendered again only after a deliberate change
of what the test draws.

**Render target.** An image the GPU draws into, which can be read back to the CPU
(`slime_gfx::RenderTarget`).

**Sample.** The example that demonstrates the exit criterion of a stage. Every stage ends with
one.

**Scene (render).** What the renderer draws in one frame: a camera, a directional light, a
clear color and *instances* (`slime_render::Scene`). Not a scene graph: the renderer keeps nothing between frames,
and the world model of Stage 5 will produce such scenes.

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

**Uniform buffer.** A buffer of values that a whole draw reads, such as a camera's matrices,
declared `var<uniform>` in WGSL and read through a *bind group* (`slime_gfx::UniformBuffer`).

**WARP.** Microsoft's software driver for Direct3D 12, part of Windows. wgpu uses it on Windows
machines without a GPU, such as the CI runners.
