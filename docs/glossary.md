# Glossary

Terms that this project defines, or uses with a narrower meaning than usual. Standard Rust and
game-development vocabulary is not repeated here. Entries are in alphabetical order.

**Adapter.** A GPU as wgpu sees it, through one graphics API: a discrete or integrated GPU, or a
software driver such as lavapipe or WARP. The engine opens one adapter's *device* to use it.

**ADR (Architecture Decision Record).** A short document in [`docs/adr/`](adr/README.md) that
records one decision: its context, the decision, the alternatives rejected and the consequences.

**Alpha mask.** Not drawing the fragments of a surface whose alpha falls below a cutoff, so that
a texture can cut shapes out of it: glTF's `MASK` alpha mode (`slime_render::AlphaMode`).

**Ambient light.** A constant light that reached every surface from every direction, the stand-in
for the light of the surroundings until the *environment* replaced it
([ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Anti-aliasing.** Smoothing the steps that an edge makes across the pixels: without it, a nearly
horizontal edge steps by a whole pixel at long intervals. The renderer uses *multisample
anti-aliasing* ([ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)).

**Application time.** The sum of the deltas of all frames so far (`FrameTime::elapsed`). It
falls behind wall-clock time when a frame delta is limited, for example after a pause.

**Asset.** Data that a game loads rather than builds in code: a texture, a mesh, a material, a
scene. Each comes from a *source asset* and reaches the game as *cooked data*
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Asset ID.** The identity of an *asset*: a random 128-bit UUID kept in the *metadata file* beside
its source, so that it stays the same when the file is moved or renamed. A *sub-asset*'s ID is
derived from its file's ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Bind group.** A set of GPU resources that a pipeline's shaders read together, bound at one
`@group` index: uniform buffers, textures and samplers (`slime_gfx::BindGroup`). Its
*bind group layout* says which bindings it has and what each holds (`slime_gfx::BindGroupLayout`,
[ADR-0026](adr/0026-resource-binding-and-depth.md)).

**Block compression (BC).** GPU texture formats that store each block of 4 × 4 texels in a fixed
number of bytes, which the GPU decodes as it samples: BC7, one byte per texel, for colour and other
data, and BC5, two channels, for normal maps. Cooked textures are compressed in them
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Bloom.** The light a lens or the eye scatters around where it lands, which the renderer adds by
spreading a share of each pixel's exposed light, its strength (`Scene::bloom`), into a halo that
falls with the square of the distance; energy-conserving: the light is taken from where it lands,
so that a uniform image is unchanged ([ADR-0040](adr/0040-bloom.md)).

**Bloom chain.** The levels of the *bloom*, each half the size of the one before: the scene's light
filtered down into them, then averaged back up into a blurred copy that the tone-mapping pass
mixes in.

**BRDF (bidirectional reflectance distribution function).** How much of the light arriving at a
surface from one direction it reflects towards another: what the *metallic-roughness model*
computes.

**BRDF table (scale and bias).** The table, by n·v and roughness, of the two numbers A and B such
that a surface reflecting f0 at normal incidence reflects f0 A + B of a uniform *environment*
(single scattering): the second factor of the *split sum*. The renderer computes it on the CPU once
per process, 32 × 32 ([ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Candela (cd).** The unit of luminous intensity, lumens per steradian: the strength of a point or
spot light (`intensity`). A surface facing such a light d metres away receives intensity / d² *lux*.

**Camera-relative rendering.** Drawing in world space moved so that the camera's eye is at the
origin: the renderer subtracts the eye from every position in `f64` on the CPU, so that the GPU's
32-bit floats hold small numbers near the camera, as precise 10 km from the world's origin as at
it ([ADR-0048](adr/0048-rendering-relative-to-the-camera.md)).

**Cascade (cascaded shadow maps).** One of the *shadow maps* that together hold the shadows of a
directional light: what the camera sees up to the light's shadow distance is split along its axis
into slices, and each slice gets its own map, as many texels across for less ground near the camera,
so that shadows there are finer. The renderer draws four, each twice as deep as the one before, in
the tiles of one *shadow atlas* ([ADR-0064](adr/0064-cascaded-shadow-maps.md)), and shades a point
with the finest cascade that holds it, which fades into the next across a band at its edge
([ADR-0065](adr/0065-reading-the-cascades.md)).

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

**Cooked data.** Files in the engine's own formats that a game loads without parsing or converting
anything, made from *source assets* by *cooking*. They are never committed: the cooker keeps them
in a cache under `target/` ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Cooking.** Turning *source assets* into *cooked data*: importing them, computing what a game would
otherwise compute as it loads them (mipmaps, tangents) and compressing textures. The engine's
program `slime-cook` cooks, before the game runs ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Cube texture (cube map).** Six square images on the faces of a cube, which shaders read by
direction: the face is the axis along which the direction is longest
(`Gpu::create_cube_texture`).

**Culling.** Not drawing the triangles that face one way, usually the back faces of closed
meshes, which their front faces hide anyway (`slime_gfx::CullMode`). A triangle faces the front
when its vertices wind counter-clockwise on screen. The renderer chooses it per draw, from the
material's double-sidedness and from whether the instance is *mirrored* (ADR-0035).

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
a *shadow map* is read. Depths are reversed (*reversed depth*).

**Device (GPU).** The engine's connection to an adapter: it creates GPU resources, and its
*queue* receives the work to execute (`slime_gfx::Gpu`). It is *lost* when the driver crashes or
is reset, the GPU is removed, or it runs out of memory: its objects can then no longer be used,
and the run stops ([ADR-0022](adr/0022-losing-the-gpu-device.md)), unless the application chose to
go on with a new device, for which it creates its objects again
([ADR-0075](adr/0075-recovering-from-a-lost-gpu-device.md)).

**Dithering.** Adding a little noise to values before they are rounded to the steps of an output,
so that the rounding keeps their average and a slow gradient shows an even grain instead of bands
one step apart. The renderer dithers 8-bit outputs by up to one step
([ADR-0051](adr/0051-dithering-8-bit-outputs.md)).

**Double-sided material.** A material whose back faces are drawn too, shaded with their normals
reversed (glTF).

**Driver.** The loop that decides when frames run and what time each one represents. `run` has a
headless driver (frames back to back, simulated time) and a windowed driver, called back by the
windowing library's event loop, which measures real time, asks for frames at a limited rate and
runs them on *redraw requests*. All drivers run frames the same way
([ADR-0010](adr/0010-main-loop-ownership.md)). Not to be confused with a GPU driver.

**Dynamic offset.** The start of the range of a *uniform buffer* that a *bind group* binds, given
each time the group is bound (`RenderPass::set_bind_group`) rather than when the group is created,
as a multiple of `Gpu::uniform_offset_alignment`. One buffer and one group then serve many draws,
one range each: the renderer's instances ([ADR-0027](adr/0027-the-render-crate.md)).

**Emission.** The light a surface gives off, added to the light it reflects: glTF's emissive
factor, its strength (`KHR_materials_emissive_strength`) and its texture, in *nits*.

**Engine-owned type.** A type of an engine crate that holds an object of a third-party library and
exposes only what the engine needs, so that the library's types never appear in a public API: the
GPU objects of `slime_gfx` hold wgpu's, `slime_platform::Window` holds winit's
([ADR-0011](adr/0011-windowing-with-winit.md), [ADR-0016](adr/0016-gpu-layer-with-wgpu.md),
[ADR-0017](adr/0017-gpu-resource-types.md)).

**Environment.** The surroundings of a scene as the renderer draws and reflects them: a *sky* drawn
on the CPU into a *cube texture* and its *prefiltered levels* (`slime_render::Environment`,
[ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Example.** A runnable program in the `examples/` directory that demonstrates one feature.

**Exposure.** What the scene's light is multiplied by before it is written into the *HDR* target,
then tone mapped (`slime_render::Scene::exposure`): a linear factor; 1 leaves it as it is, 2 is one
photographic stop brighter. The scene pass applies it (*pre-exposure*,
[ADR-0037](adr/0037-lights-in-physical-units-and-pre-exposure.md)).

**Exposure value (EV100).** A photographer's measure of an *exposure*, in stops, at the
sensitivity ISO 100: log2(N² / t) for an aperture N and a shutter time t in seconds; one more is
one stop darker. About 15 for direct sunlight, 7 for a lit room. Its exposure is
1 / (1.2 × 2^EV100), the convention of Frostbite and Filament; the `scene` example's slider moves in
it.

**Filtered importance sampling.** Reading each sample of a Monte Carlo integral of a texture from
a blurred level of it whose texels cover the solid angle the sample stands for, which removes the
noise of sharp features with few samples (Křivánek and Colbert); the *prefiltered levels* are
computed so.

**Firefly.** A tiny, very bright pixel, such as a highlight smaller than a pixel, whose *bloom*
blinks as its coverage of the pixel changes from frame to frame. The renderer does not filter them
(ADR-0040).

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

**Furnace test.** A white object inside a uniform *environment* must reflect exactly its radiance,
and disappear into it: any loss or gain of light shows.

**glTF.** The Khronos Group's format for 3D scenes: meshes, materials, textures and a node
hierarchy, as a JSON file with binary buffers, or as one GLB file with everything embedded. The
cooker, `slime-cook`, imports both ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**GPU time.** How long the GPU took to run a render pass, measured by timestamps written at its
start and its end (`slime_gfx::GpuTimer`) and read back a few frames later; `None` on adapters
without timestamp queries. A `GpuTimer` times the passes of one *submission*, each apart. The
renderer's statistics report one per kind of pass (`scene_gpu_time`, `tone_mapping_gpu_time`,
the bloom's passes summed) with the CPU time of the call (`slime_render::RenderStats`).

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

**Hot reload.** Replacing, in a running game, the *cooked data* or the shaders that changed on
disk, without restarting the game. Stage 4b brings it; until then, a game reads its data and its
shaders once ([ADR-0072](adr/0072-stage-4b-live-assets.md)).

**Illuminance.** The light a surface facing a light receives from it, in *lux*, which the *BRDF*
turns into the light the surface reflects: a `DirectionalLight`'s `illuminance`, and what a point
or spot light gives at a distance (ADR-0035, ADR-0037).

**Image-based lighting (IBL).** Lighting surfaces with the light of their whole surroundings, read
from images of it, here an *environment*, rather than from *punctual lights* alone.

**Image test.** A test that renders offscreen and compares the result with a *reference image*,
pixel by pixel, within a tolerance that absorbs how drivers round colors and filter textures
([ADR-0018](adr/0018-reference-image-tests.md), amended by
[ADR-0028](adr/0028-image-tests-tolerate-a-few-pixels.md) and
[ADR-0030](adr/0030-image-tests-strict-on-the-reference-driver.md)).

**Imported target.** A target of the *render graph* that outlives the frame and that the graph
does not create: the output, which may be a window's image that changes every frame, and the
*shadow map*. It holds what it held before the frame, so a pass may sample it without an earlier
pass drawing into it ([ADR-0041](adr/0041-render-graph.md)).

**Importer.** The part of *cooking* that reads one kind of source file, glTF or an image, into the
engine's types.

**Index buffer.** A buffer of indices into a vertex buffer, so that the triangles of a mesh share
vertices (`slime_gfx::IndexBuffer`).

**Instance (render).** One thing the renderer draws: a *mesh*, a *material* and a transform from
the mesh's space to world space (`slime_render::Instance`). A render *scene* is a flat list of
them, built for each frame.

**Interpolation factor.** The fraction of a fixed step accumulated but not yet run
(`FixedStep::alpha`), between 0 and 1. Used to draw a state between the last two fixed steps.
Applications read it from `Context::fixed_step_alpha`.

**Irradiance.** The light that reaches a surface from every direction of its hemisphere, each
weighted by the cosine of its incidence, in *lux*: a matte surface of albedo c reflects
c × irradiance / π. The roughest of the *prefiltered levels* holds the irradiance divided by π.

**KTX2.** The Khronos Group's file format for GPU textures, which holds every mipmap level of a
texture in its GPU format: the format of cooked textures
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Label (GPU object).** The name given to a GPU object when it is created, required by every
creation method of `slime_gfx::Gpu`. Error messages and GPU capture tools show it.

**lavapipe.** Mesa's Vulkan driver that renders on the CPU. The checks and CI use it on Linux, so
that GPU tests give the same results on every machine, including machines without a GPU. It is
the *reference driver* of the image tests.

**Leaf card.** A quad textured with leaves, whose texture's alpha cuts them out (an *alpha mask*):
many of them make the foliage of a tree for a fraction of the triangles that modelled leaves
would take.

**Linear color.** A color whose values are proportional to the light they stand for, so that
light adds and multiplies them as they are: twice the value is twice the light. Displays and
image files store colors *sRGB-encoded* instead. Every color of the engine's API is linear, a
`slime_gfx::LinearColor` ([ADR-0045](adr/0045-one-linear-color-type.md)).

**Logical key.** What a key means with the current keyboard layout and modifiers
(`slime_platform::Key`): on a French AZERTY keyboard, the first letter key of the top row gives
the logical key `"a"`. Suits shortcuts and text. See also *physical key*.

**Logical pixel.** A pixel of the window as the user sees it; the *scale factor*
(`Window::scale_factor`, `Event::ScaleFactorChanged`) says how many *physical pixels*, the
display's, it covers. The sizes of `WindowConfig` are logical; `Window::inner_size`,
`Event::CursorMoved` and render targets are physical. egui calls logical pixels *points*.

**Lux (lx).** The unit of *illuminance*, lumens per square metre: the strength of a directional
light (`illuminance`); about 100 000 for direct sunlight, 400 at sunrise.

**Manifest (assets).** The list of what was cooked for a game: each *asset*'s ID, kind, cooked file
and size, and the IDs it needs to load ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Material.** How a surface looks, with glTF's *metallic-roughness model*: a base color,
metallic and roughness, normal, occlusion and emissive textures each read with a *sampler*, an
alpha mode and double-sidedness (`slime_render::Material`, from `MaterialData`;
[ADR-0035](adr/0035-physically-based-materials.md)).

**Mesh.** Triangles given as vertices (a position, a normal, texture coordinates and a *tangent*
each) and indices, uploaded to the GPU (`slime_render::Mesh`, from `MeshData`).

**Metadata file.** The file `<file>.meta` committed beside a *source asset*, which holds its *asset
ID* ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Metallic-roughness model.** glTF's physically based material: a dielectric's diffuse color
under a specular layer, or a metal's reflection tinted by its color, blended by *metallic*, with
highlights as wide as the *roughness* (glTF 2.0, Appendix B; ADR-0035).

**Milestone.** A small, verifiable step of a stage, listed with its goal, its done criteria and
how to test it in [`docs/PLAN.md`](PLAN.md). Milestones are numbered `M<stage>.<n>`, for example
M1.3.

**Mirror (source assets).** The copies of the sample game's downloaded *source assets*, attached to
the releases of a GitHub repository of their own, from which `scripts/fetch-assets.sh` fetches
them ([ADR-0070](adr/0070-the-sample-games-source-assets.md)).

**Mirrored instance.** An instance whose transform has a negative determinant, such as a scale of
−1 along one axis: it turns front faces clockwise on screen, and the renderer culls its other
faces so that it keeps its front ones, as glTF does (ADR-0035).

**Mipmaps.** Copies of a texture at half, a quarter, ... of its size, down to one pixel: the
*levels of detail* among which a *sampler* chooses by the size the texture is drawn at, so that a
minified texture does not alias. The engine computes them on the CPU when the texture is created
(`Gpu::create_texture`), in linear light for the sRGB formats, or takes them from the caller
(`Gpu::create_texture_with_levels`, `Gpu::create_cube_texture`); linear filtering blends two levels.

**Multiple scattering.** Light that bounces between the microfacets of a rough surface before it
leaves, which a single-scattering *BRDF* loses: a white rough metal would look grey. The renderer
gives it back in the *environment*'s light, as the Khronos glTF Sample Viewer does, not in the
lights'.

**Multisample anti-aliasing (MSAA).** *Anti-aliasing* in which the GPU tests whether a triangle
covers each pixel at several points, its *samples (multisampling)*, and shades the pixel once for
the samples it covers; the *resolve* averages them into one pixel. It smooths the edges of
triangles, not what a shader draws inside them.

**Nit (cd/m²).** The unit of luminance, candelas per square metre: the light that surfaces reflect
and emit and the *sky*'s, as the renderer computes them before the *exposure*.

**Normal matrix.** The matrix by which the renderer transforms an instance's normals so that they
stay perpendicular to surfaces scaled unevenly: the cofactor matrix of the transform's 3 × 3 part,
proportional to its inverse transpose, which, unlike the inverse, stays finite for a mesh
flattened along one axis. It is negated for a *mirrored instance*, by the same test that chooses
the culling.

**Normal offset.** Looking a point up in the *shadow map* moved off its surface along its normal,
towards the light, rather than pushing the stored depths back: the renderer moves it along its
triangle's own normal by 4.5 of the map's texels times the sine of the light's incidence, which
keeps flat and curved surfaces from shadowing themselves while a shadow moves by at most 4.5 texels
([ADR-0050](adr/0050-a-shadow-region-that-follows-the-camera.md),
[ADR-0063](adr/0063-a-smoother-shadow-filter.md), [ADR-0065](adr/0065-reading-the-cascades.md)). For
a spot light's tile, whose depths run along the light's axis, the sine of the angle between the
triangle's own normal and that axis replaces the incidence's
([ADR-0062](adr/0062-looking-spot-shadows-up-off-the-triangle.md)).

**Normal texture.** A texture of normals in *tangent space*, which replace a mesh's normals to give
it detail without more triangles: red along the tangent, green up the image, blue along the
normal, each from −1 to 1 stored as 0 to 1 (glTF).

**Occlusion texture.** A texture whose red channel says how much of the *environment*'s light
reaches each point of a surface, darkening creases (glTF).

**Offscreen rendering.** Drawing into an image that no window shows, a *render target*, for
example to read it back to the CPU. Image tests render this way, without a windowing system.

**Overlay (dev).** Panels drawn over the engine's frames with egui, to show statistics and
tooling over a running application (`slime_devui::Overlay`,
[ADR-0031](adr/0031-dev-overlay-with-egui.md)). Removed once what replaces it does at least as
well ([ADR-0052](adr/0052-amend-roadmap-before-0-1.md)).

**Owner.** The person who validates the project's direction and its architecture decisions.

**Percentage-closer filtering (PCF).** Reading a *shadow map* by comparing a point's depth with
several stored depths around it and averaging the results, so that the edges of shadows are soft
instead of blocky. A *comparison sampler* with linear filtering does it over 2 × 2 pixels in the
GPU; the renderer makes nine such comparisons over 5 × 5 texels, placed and weighed as Castaño
proposed, so that a shadow's edge fades smoothly over about six texels
([ADR-0063](adr/0063-a-smoother-shadow-filter.md)).

**Peter-panning.** A shadow that starts a little away from the object that casts it, because the
*depth bias* pushes the stored depths back, or the *normal offset* moves the point looked up: in
the renderer, by up to 4.5 of the shadow map's texels.

**Physical key.** The position of a key on the keyboard, whatever the layout
(`slime_platform::KeyCode`), named after the character it produces on a US keyboard: the first
letter key of the top row is `KeyQ` on every layout, whether it is labelled Q (QWERTY) or A
(AZERTY). Suits movement keys. See also *logical key*.

**Physical pixel.** A pixel of the display. See *logical pixel*.

**Pre-exposure.** Multiplying the scene's light by the *exposure* in the scene pass, before the
*HDR* target is written, so that light in physical units fits *half precision*
([ADR-0037](adr/0037-lights-in-physical-units-and-pre-exposure.md)); the target holds that light
divided by 64, so that up to 64 × 65 504 keeps its energy for the *bloom*
([ADR-0040](adr/0040-bloom.md)).

**Prefiltered levels.** The levels of an *environment*'s *cube texture* past the first: the sky
averaged under the reflection lobe of a growing roughness, so that one lookup gives what a rough
surface reflects ([ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Present (an image).** Hand a drawn image to the window, which shows it at one of the display's
next refreshes, as the *present mode* says.

**Present mode.** How a window shows the images presented to it (`slime_gfx::PresentMode`). With
*Mailbox*, the engine's default, each image replaces the one waiting to be shown and presenting
never waits; with *FIFO*, every image is shown in order, one per refresh, and presenting waits
while earlier images are still to be shown ([ADR-0021](adr/0021-frame-pacing.md)).

**Punctual light.** A light without size, from one direction or one point, as glTF's
`KHR_lights_punctual` defines it (`slime_render::Light`): a directional light, whose strength is an
*illuminance*; a point light, whose strength is an intensity in *candelas*, shining in every
direction; and a spot light, a point light in a cone, full within its inner angle and dark beyond
its outer one. A point or spot light's optional range is the distance at which its light reaches
zero, smoothly.

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

**Render graph.** The renderer's description of a frame as passes, each declaring the targets it
draws into and the ones it samples (`crates/slime-render/src/graph.rs`, private): it orders the
passes by those targets, creates the *transient targets*, records the passes, four to a
*submission*, and gives each one's *GPU time* ([ADR-0041](adr/0041-render-graph.md)). It is built
again when the frame's *shape* changes.

**Render target.** An image the GPU draws into (`slime_gfx::RenderTarget`): an offscreen image,
which can be read back to the CPU and sampled by later passes, or the image of a window for one
frame, which can be neither.

**Resolve.** Averaging the *samples (multisampling)* of each pixel of a multisampled *render
target* into an ordinary one, which later passes can read. The renderer weighs each sample by its
brightness, so that the edges of bright surfaces are smooth too
([ADR-0060](adr/0060-resolving-the-samples-weighed-by-their-brightness.md)).

**Reversed depth (reversed-Z).** Depth that runs from 1 at the near plane to 0 infinitely far,
the far plane being at infinity ([ADR-0047](adr/0047-reversed-depth-with-an-infinite-far-plane.md)).
A perspective projection crowds far distances near one end of the depth range; reversed, that end
is 0, where floating-point depths are finest, so that 32-bit depths keep a precision relative to
the distance, at any distance. Passes clear *depth targets* to 0, a depth test keeps the greater
depth, and a *depth bias* is negative.

**Sample.** The example that demonstrates the exit criterion of a stage. Every stage ends with
one.

**Sample (multisampling).** One of the points of a pixel at which *multisample anti-aliasing*
tests coverage and depth; not the *sample* of a stage.

**Sampler.** How shaders read a texture: nearest or linear filtering, and what lies beyond its
edges (repeat, mirror or clamp) (`slime_gfx::Sampler`).

**Scene (render).** What the renderer draws in one frame: a camera, *punctual lights*, an
*environment*, an *exposure* and *instances* (`slime_render::Scene`). Not a scene graph: the
renderer keeps nothing between frames, and the world model of Stage 5 will produce such scenes.

**Shadow acne.** Speckles of shadow that a surface casts on itself where its depth and the one
stored in the *shadow map* differ only by rounding or by the map's texels; a *depth bias* and a
*normal offset* remove them.

**Shadow atlas.** One *depth target* split into square tiles, each a *shadow map*: of one spot light
([ADR-0058](adr/0058-shadows-for-every-spot-light-in-one-atlas.md)), or of one *cascade* of the
directional light ([ADR-0064](adr/0064-cascaded-shadow-maps.md)), each in an atlas of its own.

**Shadow map.** A *depth target* drawn from a light's point of view by a *depth-only pass*: a
point is in shadow where something nearer to the light is stored at its place in the map. The
renderer draws one each frame for each *cascade* of the first directional light that casts shadows,
through an orthographic projection that covers the cascade's slice of what the camera sees, its
texels on a grid fixed in the world ([ADR-0038](adr/0038-shadow-mapping.md),
[ADR-0050](adr/0050-a-shadow-region-that-follows-the-camera.md)), and one for each spot light
that casts shadows, in perspective.

**Shape (of a frame).** What decides the renderer's passes and targets: the output's size, whether
the shadow pass runs and whether the bloom's passes do. The *render graph* is built again, and its
GPU times start over, when it changes ([ADR-0041](adr/0041-render-graph.md)).

**Simulated time.** Frame deltas supplied as a constant duration instead of being measured, so
that a run gives the same results on every machine. Headless runs use it.

**Skeleton (crate).** A crate that exists with its documentation and inherits the workspace
policies, but holds no code yet.

**Sky.** The procedural daytime sky from which the renderer computes an *environment*
(`slime_render::Sky`): a gradient from the zenith to the horizon, a haze into the ground below,
and an optional sun, whose glow, the light the sky scatters around it, it holds. The sun's disc is
drawn by the *sky pass*, not held.

**Sky pass.** The render pass that draws the *environment* and the *sun's disc* where no surface
was drawn, after the scene pass.

**Source asset.** A file that people make or download and that *cooking* imports: a glTF scene, a
PNG or JPEG image. Its *metadata file* is committed beside it; the file itself is committed too, or
fetched from the *mirror* ([ADR-0069](adr/0069-the-asset-pipeline.md),
[ADR-0070](adr/0070-the-sample-games-source-assets.md)).

**Spiral of death.** A fixed-step loop that falls further behind at each frame because running
the overdue steps takes longer than the time they cover. `FixedStep` avoids it by limiting the
steps per frame and dropping the rest.

**Split sum.** Karis's approximation of what a surface reflects of its *environment*: a
*prefiltered level* read along the reflection, times a Fresnel term from the *BRDF table*.

**sRGB encoding.** The transfer function of the sRGB standard (IEC 61966-2-1), which spends more
of a byte's 256 values on dark colors than on bright ones, as the eye distinguishes them: the
values of displays, image files, color pickers and web colors. A *linear color* of 0.216 is
encoded as 128 out of 255. The GPU decodes the texels of sRGB textures and encodes what it writes
into sRGB targets; `LinearColor::from_srgb8` decodes bytes on the CPU.

**Stage.** A step of [`docs/ROADMAP.md`](ROADMAP.md), ordered by dependency, with a goal and an exit
criterion. Stages are numbered from 0; a stage may be split (Stage 3a and 3b), and a stage added
later takes a letter (Stage 3c), so that the numbers cited elsewhere stay right.

**Sub-asset.** An *asset* inside a source file, such as a mesh, a material or a scene of a glTF
file, whose ID is derived from the file's ID, its kind and its name
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Submission.** Recorded GPU work handed to the GPU at once (`slime_gfx::Gpu::submit`, or the single
pass of `Gpu::render`): one call into the driver, and a unit of synchronization on the GPU. Each
costs CPU time, so the renderer records its passes four to a submission
([ADR-0041](adr/0041-render-graph.md)).

**Sun's disc.** The disc of the sun's apparent size, 0.533°, that the *sky pass* draws, whose
radiance gives a surface facing it the sun's illuminance; it lights nothing, the scene's
directional light carrying the sun's light.

**Surface.** The link between the GPU and a window: the images the window shows. Each frame, the
engine *acquires* one, the application draws into it, and the engine *presents* it
(`slime_gfx::Surface`). The graphics API can report it *lost*: it is then created again, and given
up, which stops the run, when that fails or when it is lost again each time
([ADR-0025](adr/0025-giving-up-a-lost-surface.md)).

**Task pool.** Threads that run small pieces of work, *tasks*, as they come, each on the first
thread free. `slime-render` has one of its own, private, which loads scenes in the background: each
task reads and uploads one mesh or one texture
([ADR-0073](adr/0073-loading-scenes-in-the-background.md)).

**Tangent.** A direction in a surface, along which the texture's u coordinate grows. With the
normal and the *handedness* w (±1, −1 where the texture is mirrored on the surface), it gives the
bitangent, cross(normal, tangent) × w, and with it the *tangent space* in which *normal textures*
are stored (`MeshData::tangents`, glTF's convention).

**Termination signal.** A request from outside the process to stop it: SIGINT (Ctrl+C in a
terminal), SIGTERM (a service manager or `kill`) or SIGHUP (the terminal closing). The first one
stops a run cleanly; a second one, half a second or more later, ends the process at once
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

**Texture role.** What a cooked texture is used for: colors, data or normals, which sets how the
cooker averages its levels and compresses it ([ADR-0071](adr/0071-cooked-textures.md)).

**Timestamp query.** A GPU feature that writes the GPU's clock at a point of its command stream;
two of them, around a render pass, give its *GPU time* (`slime_gfx::GpuTimer`). Optional: the
device asks for it only when the adapter offers it (`Gpu::can_time`,
[ADR-0031](adr/0031-dev-overlay-with-egui.md)).

**Tone mapping.** Mapping the *HDR* colors of a scene to the range a display shows, 0 to 1, with a
curve that compresses highlights instead of clipping them. The renderer's tone-mapping pass draws a
*fullscreen triangle* that applies to the exposed light Khronos PBR Neutral, the default of the
Khronos glTF Sample Viewer, which keeps hues and lowers dark and mid tones by up to 0.04, an offset
meant to balance the reflection of their surroundings that surfaces get from image-based lighting
([ADR-0034](adr/0034-hdr-rendering-and-tone-mapping.md)).

**Tooling (engine's).** Code that serves development and ships with no game: the dev *overlay*
today. It alone may reach wgpu, through the `wgpu` feature of `slime-gfx`
([ADR-0031](adr/0031-dev-overlay-with-egui.md)).

**Transient target.** A target of the *render graph* that it creates, at the output's size divided
by a power of two, and whose content lives for one frame: the renderer's high-dynamic-range and
depth targets and the bloom's levels. It is kept from frame to frame, and reused by the graph of the
next *shape* that declares it at the same size ([ADR-0041](adr/0041-render-graph.md)).

**Uniform buffer.** A buffer of values that a whole draw reads, such as a camera's matrices,
declared `var<uniform>` in WGSL and read through a *bind group* (`slime_gfx::UniformBuffer`).

**WARP.** Microsoft's software driver for Direct3D 12, part of Windows. wgpu uses it on Windows
machines without a GPU, such as the CI runners.
