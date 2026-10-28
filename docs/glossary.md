# Glossary

Terms that this project defines, or uses with a narrower meaning than usual. Standard Rust and
game-development vocabulary is not repeated here. Entries are in alphabetical order.

**Action (interface).** What a person or an agent does to an interface: a button pressed, an
*intent* of navigation, or the command of a *shortcut* (`slime_ui::action::Action`). A person's
input becomes actions; an agent sends them; the program reads them from a queue and does what they
ask ([ADR-0142](adr/0142-input-and-focus.md)).

**Adapter.** A GPU as wgpu sees it, through one graphics API: a discrete or integrated GPU, or a
software driver such as lavapipe or WARP. The engine opens one adapter's *device* to use it.

**Adoption (entity).** A game taking an entity out of the scene that spawned it, with its
descendants whose identities are under its own, and giving it a key of its own
(`slime_world::World::adopt`), so that it outlives the scene, as a companion who follows the hero
from one level to the next; its *former identity* is kept, and saves keep it, so that references
written before find it ([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md),
[ADR-0125](adr/0125-saves-per-scene.md)).

**ADR (Architecture Decision Record).** A short document in [`docs/adr/`](adr/README.md) that
records one decision: its context, the decision, the alternatives rejected and the consequences.

**Alpha mask.** Not drawing the fragments of a surface whose alpha falls below a cutoff, so that
a texture can cut shapes out of it: glTF's `MASK` alpha mode (`slime_render::AlphaMode`).

**Ambient light.** A constant light that reached every surface from every direction, the stand-in
for the light of the surroundings until the *environment* replaced it
([ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Anti-aliasing.** Smoothing the steps that an edge makes across the pixels: without it, a nearly
horizontal edge steps by a whole pixel at long intervals. The renderer uses *multisample
anti-aliasing* ([ADR-0056](adr/0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)), or none, as
each *view* chooses ([ADR-0127](adr/0127-the-settings-of-a-view.md)).

**Application time.** The sum of the deltas of all frames so far (`FrameTime::elapsed`). It
falls behind wall-clock time when a frame delta is limited, for example after a pause.

**Asset cache.** The GPU objects of the meshes that a game draws, with their materials and
textures, kept by *asset ID* outside the *world* (`slime_render::AssetCache`): the world names
assets by ID and holds no GPU object. It loads what *requests* ask for in batches, in the
background, each whole, keeps what their *request handles* hold, drops what is *releasable* past
its budgets, and loads again what changed in new *cooked data*; when its *device* is lost, the game
makes a new cache with the new renderer and the requests of the old one
([ADR-0084](adr/0084-what-the-world-draws.md),
[ADR-0120](adr/0120-a-frame-that-owns-what-it-draws.md), [ADR-0128](adr/0128-asset-requests.md)).

**Asset ID.** The identity of an *asset*: a random 128-bit UUID kept in the *metadata file* beside
its source, so that it stays the same when the file is moved or renamed. A *sub-asset*'s ID is
derived from its file's ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Asset request.** Asking the *asset cache* for assets, with a priority (*now*, *soon* or *later*),
which returns a request handle: while a request handle lives, its assets stay in memory or keep
loading; once none does, a load not done is cancelled and a loaded asset is *releasable*, dropped
least recently held first once the cache passes its kind's budget
(`slime_render::AssetCache::request`,
[ADR-0113](adr/0113-asset-requests-residency-and-priorities.md),
[ADR-0128](adr/0128-asset-requests.md)).

**Asset.** Data that a game loads rather than builds in code: a texture, a mesh, a material, a
scene. Each comes from a *source asset* and reaches the game as *cooked data*
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Attachment (bone).** A component that hangs an entity on a bone of another entity's skeleton,
naming the entity and the bone; saves and scene files hold it as any reference
([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md)). Decided, not built yet.

**Backdrop (interface).** The fill that a *layer*'s root lays over the whole window below it,
behind its own box: a modal layer's, which the editor's look gives its theme's scrim, dims what takes
no input while it is open (`PaintStyle::backdrop`,
[ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)).

**Base (save).** What a *world* held when the game *settled* it, which its *saves* compare it
with: the world keeps a copy of each value at its first change after its entity was settled, and
the identity of each entity settled that the run destroys, so that a save costs what changed
([ADR-0125](adr/0125-saves-per-scene.md)).

**Binary encoding (documents).** The engine's own encoding of a *document* in bytes, without its
types: its keys and texts once each, then its values, each a tag and what it holds. A reader reads it
as it reads the text, and its errors name the byte (`slime_serialize::encode`,
[ADR-0094](adr/0094-the-binary-encoding-of-documents.md)).

**Bind group.** A set of GPU resources that a pipeline's shaders read together, bound at one
`@group` index: uniform and storage buffers, textures, render and depth targets, and samplers
(`slime_gfx::BindGroup`). Its
*bind group layout* says which bindings it has and what each holds (`slime_gfx::BindGroupLayout`,
[ADR-0026](adr/0026-resource-binding-and-depth.md)).

**Blending.** Laying a color over what a *render target* holds by its alpha, rather than replacing
it, as a pipeline of the GPU layer may (`slime_gfx::Blend`): glass, smoke, the soft edges of icons
and lines. A material blends with the alpha mode `Blend`, which only *unlit materials* draw before
Stage 9a sorts transparent surfaces by their depth
([ADR-0129](adr/0129-drawing-for-the-editor.md)).

**Block (world).** Sixteen values of a component's storage side by side, whose summary, the latest
tick at which one of them changed, lets a reader of what changed skip those that did not
(`World::changed`, [ADR-0119](adr/0119-the-worlds-ticks-blocks-and-log.md)).

**Block compression (BC).** GPU texture formats that store each block of 4 × 4 texels in a fixed
number of bytes, which the GPU decodes as it samples: BC7, one byte per texel, for colour and other
data, and BC5, two channels, for normal maps. Cooked textures are compressed in them
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Bloom chain.** The levels of the *bloom*, each half the size of the one before: the scene's light
filtered down into them, then averaged back up into a blurred copy that the tone-mapping pass
mixes in.

**Bloom.** The light a lens or the eye scatters around where it lands, which the renderer adds by
spreading a share of each pixel's exposed light, its strength (`Scene::bloom`), into a halo that
falls with the square of the distance; energy-conserving: the light is taken from where it lands,
so that a uniform image is unchanged ([ADR-0040](adr/0040-bloom.md)).

**BRDF (bidirectional reflectance distribution function).** How much of the light arriving at a
surface from one direction it reflects towards another: what the *metallic-roughness model*
computes.

**BRDF table (scale and bias).** The table, by n·v and roughness, of the two numbers A and B such
that a surface reflecting f0 at normal incidence reflects f0 A + B of a uniform *environment*
(single scattering): the second factor of the *split sum*. The renderer computes it on the CPU once
per process, 32 × 32 ([ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Build for shipping.** A game built without its default features, `--no-default-features`, so
without `slime-app`'s `development` feature and the dev *overlay*: it holds neither the
*development channel* nor the overlay. The checks build *Hunna Wood* so, and fail if it holds either
([ADR-0122](adr/0122-the-development-channel.md),
[ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)).

**Camera-relative rendering.** Drawing in world space moved so that the camera's eye is at the
origin: the renderer subtracts the eye from every position, all `f64`s, on the CPU, so that the
GPU's 32-bit floats hold small numbers near the camera, as precise 10 000 km from the world's
origin as at it ([ADR-0048](adr/0048-rendering-relative-to-the-camera.md),
[ADR-0079](adr/0079-the-engines-own-world.md)).

**Candela (cd).** The unit of luminous intensity, lumens per steradian: the strength of a point or
spot light (`intensity`). A surface facing such a light d metres away receives intensity / d² *lux*.

**Canvas.** What the renderer draws in two dimensions, after the *views*, into any target: boxes
with rounded corners and borders, their shadows, boxes filled with a gradient between their corners'
colors ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)), and images, each
clipped, in the order given
(`slime_render::Canvas`, `Renderer::draw_canvas`). Into an sRGB target, its colors mix as their
*sRGB encoding*'s values do, as design tools mix them ([ADR-0137](adr/0137-the-canvas.md)). The
engine's UI framework draws its *display list* through it, glyphs as images of the *glyph atlas*
([ADR-0132](adr/0132-the-engines-ui-framework.md)).

**Cascade (cascaded shadow maps).** One of the *shadow maps* that together hold the shadows of a
directional light: what the camera sees up to the light's shadow distance is split along its axis
into slices, and each slice gets its own map, as many texels across for less ground near the camera,
so that shadows there are finer. The renderer draws four, each twice as deep as the one before, in
the tiles of one *shadow atlas* ([ADR-0064](adr/0064-cascaded-shadow-maps.md)), and shades a point
with the finest cascade that holds it, which fades into the next across a band at its edge
([ADR-0065](adr/0065-reading-the-cascades.md)).

**Checkpoint (plan).** A check of a stage so far that the *owner* makes by hand between two of its
*milestones*, whose findings adjust the plan: Stage 6c has two, the owner trying the editor after
Play, then building a part of the game's zone again by hand as the stage's exit asks
([`docs/PLAN.md`](PLAN.md), [ADR-0155](adr/0155-the-editors-design.md)).

**Checks (the).** The automated checks of the *definition of done*, which `scripts/check.sh` runs
and the three Linux CI jobs run unchanged, one for each of its parts, `headless`, `windowed` and
`game` ([ADR-0123](adr/0123-the-linux-checks-in-two-jobs.md),
[ADR-0155](adr/0155-the-editors-design.md)): formatting, clippy (for Windows too), the
build, the tests on lavapipe, rustdoc, the dependency rules, the source assets and their cooking,
the samples without a display server and, with the sample game, in a headless Weston compositor, and
the links between documents ([`CONTRIBUTING.md`](../CONTRIBUTING.md#checks)).

**Class (element).** A name that the code building an *element* gives it, by which the rules of a
*style sheet* select it (`Element::class`); an element holds any number
([ADR-0141](adr/0141-style-sheets-and-themes.md)).

**Client of the engine.** A crate or program built on the engine's public API only, which no engine
crate depends on: the examples, the games, such as *Hunna Wood*, the editor (`slime-editor`), the
dev *overlay* (`slime-devui`), and the benchmarks (`slime-benches`).

**Cluster (text).** The characters that one or more *glyphs* draw together, and that a caret
never splits: a letter and its marks, a conjunct of Devanagari, a ligature, an emoji sequence
([ADR-0136](adr/0136-the-interfaces-text.md)).

**Color well.** A widget of the engine's interface that shows a color, and opens, as it is pressed,
a *picker* below it: a square of saturation and value, strips of hue and alpha, the color's channels
and its hexadecimal code (`slime_ui::tree::Kind::ColorWell`,
[ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)).

**Command.** A change of the world asked for outside the *phases* of a frame that may make it, by
the editor, the dev *overlay*, a tool, a load or a *presentation*: a value that names its entities
by their *persistent identities* (`slime_world::Command`), queued by the *host*, and applied at the
start of the next frame ([ADR-0107](adr/0107-the-execution-model.md),
[ADR-0109](adr/0109-stable-references-and-undo.md),
[ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md),
[ADR-0134](adr/0134-the-editors-core.md)): a property set, an entity created, a prefab placed, an
entity deleted, a parent and a *rank* given, a component attached or detached, and what a deletion
took out restored. Applying one gives back the command that undoes it, which the editor's history
keeps; it is written as text, which the editor's *journal* holds.

**Comparison sampler.** A sampler that compares a reference depth with the depths stored in a
*depth target* instead of reading them: each pixel read gives 1 where the comparison holds and 0
where it does not, and linear filtering blends the four nearest results
(`slime_gfx::ComparisonSampler`, [ADR-0033](adr/0033-gpu-layer-for-multi-pass-rendering.md)).
Shadow maps are read this way. See *percentage-closer filtering*.

**Component ID.** The number by which a *world* knows a *component* type, given in the order types
are first attached. It stays in the process: files name types through *reflection*
([ADR-0082](adr/0082-the-worlds-storage-and-handles.md)).

**Component.** A piece of data that an *entity* holds, such as its transform, a light or a collider:
an entity holds at most one component of each type, and the *world* stores components by type
([ADR-0079](adr/0079-the-engines-own-world.md)). Every component type has *reflection*
([ADR-0085](adr/0085-reflection.md)), and clones
([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md)).

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
material's double-sidedness and from whether the instance is *mirrored* (ADR-0035). Not to be
confused with *frustum culling*.

**Cursor held.** The cursor that a window keeps, hidden, where it was while the *raw motion* of the
mouse turns what a program turns with it, a camera (`Window::set_cursor_held`,
[ADR-0157](adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)).

**Debug group.** A named section of the commands of a render pass, shown by GPU capture tools such
as RenderDoc (`RenderPass::debug_group`).

**Definition of done.** The conditions a milestone must meet before it is reported as finished,
listed in [`AGENTS.md`](../AGENTS.md#definition-of-done).

**Depth bias.** An offset that a pipeline adds to the depth of what it draws before the depth
test: a constant part, and a part proportional to how steeply the triangle's depth changes from
pixel to pixel (`slime_gfx::DepthBias`). A *shadow map* is drawn with one, so that a surface does
not shadow itself where its depth and the stored one differ only by rounding, which would cover
it with dark specks (*shadow acne*).

**Depth target.** A texture holding, for each pixel, the depth of what was drawn there, with
which a render pass keeps the nearest surface whatever the order the surfaces are drawn in: the
*depth test* (`slime_gfx::DepthTarget`, `slime_gfx::DepthState`). Later passes can sample it, as
a *shadow map* is read. Depths are reversed (*reversed depth*).

**Depth-only pass.** A render pass without a *render target*, which writes depth only into a
*depth target*, with pipelines that have no fragment stage (`Gpu::render_depth`): how a *shadow
map* is drawn.

**Development channel.** The link between a running game and the tools that drive it: the *MCP*
server, tests, and from Stage 6c the editor. A game started with a *token* in its environment
listens on the machine's own address; through it, a tool pauses, steps and reads the game, changes
its world, plays it with keys, captures its image or one of its views, reads and changes its views'
settings, reads its *reports*, picks the entity at a pixel, reads and uses its interface element by
element ([ADR-0143](adr/0143-what-agents-see-of-the-interface.md)), and saves and loads its run
(`slime-channel`,
[ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md),
[ADR-0122](adr/0122-the-development-channel.md)). The *editor's program* listens the same way, and
answers the editor's requests ([ADR-0135](adr/0135-the-editors-program.md)). A *build for
shipping* has none.

**Device (GPU).** The engine's connection to an adapter: it creates GPU resources, and its
*queue* receives the work to execute (`slime_gfx::Gpu`). It is *lost* when the driver crashes or
is reset, the GPU is removed, or it runs out of memory: its objects can then no longer be used,
and the run stops ([ADR-0022](adr/0022-losing-the-gpu-device.md)), unless the application chose to
go on with a new device, for which it creates its objects again
([ADR-0075](adr/0075-recovering-from-a-lost-gpu-device.md)).

**Display list.** What an interface of the engine's UI framework draws in a frame, as data that the
framework owns and hands over: boxes, shadows, texts laid out and parts of images, each with its
clip (`slime_ui::display::DisplayList`). `slime-ui-render` turns it into the renderer's *canvas*,
its texts' glyphs read from the *glyph atlas* ([ADR-0132](adr/0132-the-engines-ui-framework.md),
[ADR-0139](adr/0139-text-drawn-and-the-display-list.md)).

**Dithering.** Adding a little noise to values before they are rounded to the steps of an output,
so that the rounding keeps their average and a slow gradient shows an even grain instead of bands
one step apart. The renderer dithers 8-bit outputs by up to one step
([ADR-0051](adr/0051-dithering-8-bit-outputs.md)).

**Docking (editor).** Arranging an editor's panels inside its window, in splits of any number of
children, each with its share of the space, and stacks of tabs; a panel closed keeps its place, to
come back there. The arrangement, the panels' layout, is reflected data saved per user and per
project, outside the repository ([ADR-0155](adr/0155-the-editors-design.md)). Decided, not built
yet.

**Document (serialization).** A file that holds one *value* of a registered type, or several at
their places as a *save* does, written by the engine's writer and read through the types'
descriptions: a TOML table of its fields, the versions of its types past 1 under `$versions`, and
the conventions of `slime_serialize::format` for what TOML lacks; or the same in the *binary
encoding* ([ADR-0086](adr/0086-serialization.md)).

**Double-sided material.** A material whose back faces are drawn too, shaded with their normals
reversed (glTF).

**Drawings (view).** What a client, an editor or a tool, draws into a *view* besides the scene, at
two points of its frame (`slime_render::Drawings`): in the scene, lines and instances hidden by
what is in front of them, as a grid or the icons of lights; over the image, lines over everything,
as the box of what is selected ([ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md),
[ADR-0129](adr/0129-drawing-for-the-editor.md)).

**Driven (component).** A component that the game, an animation or physics computes again, so
that its value of the moment does not matter: saves, the overrides of a *prefab* and a level
written back leave it as its author wrote it. An entity marks which of its components are driven
with the `Driven` component, which lists their types (`slime_world::Driven`,
[ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md),
[ADR-0124](adr/0124-driven-components-keys-and-created-entities-in-levels.md)).

**Driver.** The loop that decides when frames run and what time each one represents. `run` has a
headless driver (frames back to back, simulated time) and a windowed driver, called back by the
windowing library's event loop, which measures real time, asks for frames at a limited rate and
runs them on *redraw requests*. All drivers run frames the same way
([ADR-0010](adr/0010-main-loop-ownership.md)). Not to be confused with a GPU driver.

**DXC.** Microsoft's DirectX Shader Compiler, `dxcompiler.dll`, which wgpu uses on Direct3D 12 when
it lies beside the program: the engine's Windows programs ship it, and its code draws about twice as
fast as *FXC*'s ([ADR-0066](adr/0066-shipping-dxc-beside-the-programs.md)).

**Dynamic offset.** The start of the range of a *uniform buffer* that a *bind group* binds, given
each time the group is bound (`RenderPass::set_bind_group`) rather than when the group is created,
as a multiple of `Gpu::uniform_offset_alignment`. One buffer and one group then serve many draws,
one range each: the views of the tiles that the renderer's shadow passes draw
(`slime_render::Renderer`). The instances' values are in a *storage buffer* instead
([ADR-0117](adr/0117-frustum-culling-and-instancing.md)).

**Editor's program.** The editor's process, `slime_editor::run`: an application of the engine whose
*simulation* is the editor's core and whose *presentation* is its window, or none with
`--headless`, which opens a level, keeps its *journal*, and answers a person through its window
and the tools and agents that edit the level through the *development channel* alike; a game's
editor package calls it with the game's types, the `editor` example with the engine's
([ADR-0135](adr/0135-the-editors-program.md),
[ADR-0156](adr/0156-the-editors-window-and-its-operations.md)).

**Element (interface).** A part of an interface of the engine's UI framework, in its tree: a box, a
label, an image, a button, a box that scrolls or a list, with its typed style
(`slime_ui::tree::Element`). It lasts from one frame to the next, named among its siblings by its
*key* ([ADR-0140](adr/0140-the-tree-of-elements.md)).

**Emission.** The light a surface gives off, added to the light it reflects: glTF's emissive
factor, its strength (`KHR_materials_emissive_strength`) and its texture, in *nits*.

**Engine-owned type.** A type of an engine crate that holds an object of a third-party library and
exposes only what the engine needs, so that the library's types never appear in a public API: the
GPU objects of `slime_gfx` hold wgpu's, `slime_platform::Window` holds winit's
([ADR-0011](adr/0011-windowing-with-winit.md), [ADR-0016](adr/0016-gpu-layer-with-wgpu.md),
[ADR-0017](adr/0017-gpu-resource-types.md)).

**Entity.** One thing in a *world*: a handle, made of an index and a generation, to which
*components* are attached, with a parent and children in the world's hierarchy. Handles are never
written to files: files and saves name an entity by its *persistent identity*
([ADR-0079](adr/0079-the-engines-own-world.md)).

**EntityRef.** A reference to an entity by its *persistent identity*, which finds the entity's
handle when the entity is loaded, and waits for it otherwise: files and saves write it, whatever
scene its target belongs to
([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)). Decided, not built yet.

**Environment.** The surroundings of a scene as the renderer draws and reflects them: a *sky* drawn
on the CPU into a *cube texture* and its *prefiltered levels* (`slime_render::Environment`,
[ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Example.** A runnable program in the `examples/` directory that demonstrates one feature.

**Exposure value (EV100).** A photographer's measure of an *exposure*, in stops, at the
sensitivity ISO 100: log2(N² / t) for an aperture N and a shutter time t in seconds; one more is
one stop darker. About 15 for direct sunlight, 7 for a lit room. Its exposure is
1 / (1.2 × 2^EV100), the convention of Frostbite and Filament; the `scene` example's slider moves in
it.

**Exposure.** What the scene's light is multiplied by before it is written into the *HDR* target,
then tone mapped (`slime_render::Scene::exposure`): a linear factor; 1 leaves it as it is, 2 is one
photographic stop brighter. The scene pass applies it (*pre-exposure*,
[ADR-0037](adr/0037-lights-in-physical-units-and-pre-exposure.md)).

**Face (font).** One font of a font file, at one point of its variations, a variable font's
weight among them: what draws a run of *glyphs* ([ADR-0136](adr/0136-the-interfaces-text.md)).

**Field (interface).** An element that the user types in: of text, one line or several, or of a
number, which a drag changes too. It keeps what the user types, its selection and its undo, and the
program reads it as actions, *changed* at each edit and *committed* by Enter or as the focus leaves
it, once the user changed it (`Element::text_field`, `Element::number_field`,
[ADR-0145](adr/0145-text-edited.md)).

**Filtered importance sampling.** Reading each sample of a Monte Carlo integral of a texture from
a blurred level of it whose texels cover the solid angle the sample stands for, which removes the
noise of sharp features with few samples (Křivánek and Colbert); the *prefiltered levels* are
computed so.

**Fingerprint (scene).** The FNV-1a hash of what a scene was resolved from: a glTF scene's cooked
bytes, or a scene file's *binary encoding* and its *prefabs*' fingerprints. A *save* names each
scene that its world spawned with its fingerprint, and loading reports the scenes whose fingerprint
changed since ([ADR-0087](adr/0087-the-worlds-documents.md),
[ADR-0091](adr/0091-scene-documents-and-placements.md),
[ADR-0095](adr/0095-cooked-scene-files.md)).

**Firefly.** A tiny, very bright pixel, such as a highlight smaller than a pixel, whose *bloom*
blinks as its coverage of the pixel changes from frame to frame. The renderer does not filter them
(ADR-0040).

**Fixed step.** A duration of time, constant for a run, in which code that must not depend on
the frame rate advances (`slime_app::time::FixedStep`). A frame runs zero or more fixed steps.

**Floor (of limits).** The least that the engine needs of a GPU adapter's limits, such as how many
textures a shader reads or how large a buffer is: WebGPU's default limits. A device opens with
every limit that its adapter offers, and an adapter below the floor is refused, the limit named
(`slime_gfx::GfxError::BelowFloor`, [ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md)).

**Focus (interface).** The element that the keys reach first, a button or a *field*, which Tab moves
in the order of the tree and the arrows to the nearest element on screen, within its scope: the top
*modal root*, or the root that holds it ([ADR-0142](adr/0142-input-and-focus.md)).

**Font stack.** The font families that a text's style names, in order of preference: a character
takes the first font that has it. Without the system's fonts, a character that no font of the
stack has is missing; with them, the system's fallback finds a font, which is named for the
characters of that writing system from then on ([ADR-0136](adr/0136-the-interfaces-text.md)).

**Former identity.** What an entity was called before it left its scene: before a game *adopted* it,
which the world keeps and saves write, so that references written before find it
([ADR-0125](adr/0125-saves-per-scene.md)); or before the editor moved it out of its file, which the
file that holds it then writes, so that saves, overrides and references that named it find it
([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)), decided and not built yet.

**Frame contract.** What the world gives the renderer at each frame, and how: a frame that owns
what it draws, built by `slime-world-render`, drawn in one or more *views*, each object naming a
material of some *kind* ([ADR-0111](adr/0111-the-frame-contract.md)). The frame that owns what it
draws, the kinds, the views and their settings are built.

**Frame scheduler (frame limiter).** The part of the windowed driver that schedules frames and
waits between them, so that the engine asks for at most `AppConfig::max_frame_rate` frames per
second on average. With the default *present mode*, it alone paces the frames
([ADR-0021](adr/0021-frame-pacing.md), which calls it the timer, or the schedule).

**Frame.** One iteration of the main loop. Its *delta* is the time it represents: the time since
the previous frame, limited to a maximum (`slime_app::time::FrameClock`).

**Frames on demand.** A program's loop that runs a frame only when something asks for one, an event
of the window, a request of the *development channel*, the interface while it moves or once its
next change is due, the program or another thread through a waker, rather than at the scheduled
rate: an idle editor runs none (`AppConfig::on_demand`,
[ADR-0156](adr/0156-the-editors-window-and-its-operations.md)).

**Frustum culling.** Leaving out of a pass the *instances* that it cannot show: those whose box,
the smallest box around their mesh, lies outside the volume that the pass's camera or light covers,
its *frustum*. The scene pass does it for the camera's view, the shadow passes for each *cascade*
and each spot light (`slime_render::Renderer`,
[ADR-0117](adr/0117-frustum-culling-and-instancing.md)): an instance that the camera does not see
may still cast a shadow into its view.

**Fullscreen triangle.** One triangle larger than the target, which covers every pixel of it,
built by the vertex shader from the vertex index without a vertex buffer: how a pass that
processes a whole image, such as tone mapping, draws.

**Furnace test.** A white object inside a uniform *environment* must reflect exactly its radiance,
and disappear into it: any loss or gain of light shows.

**FXC.** The older shader compiler of Direct3D, part of Windows, which wgpu uses on Direct3D 12 when
*DXC* is not beside the program, with a warning in the log: its code is slower
([ADR-0066](adr/0066-shipping-dxc-beside-the-programs.md)).

**Game time.** The time of a game's *simulation*, apart from *real time*: it advances at a
speed, 1 by default, less to slow the game down, more to hasten it, and not at all while it is
paused, when the *host* runs one fixed step at a time on demand; its fixed steps keep their
duration at every speed (`slime_app::time::GameTime`, [ADR-0107](adr/0107-the-execution-model.md)).
The frame's real time times the speed is what it advances by.

**Gizmo.** The *handles* that an editor draws over a selection to move, turn or scale it with the
mouse ([ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)); the editor's code calls where
they stand, their *pivot*, their axes and their size, a gizmo (`handles.rs`).

**Handles (editor).** The arrows, squares, rings and cubes that the editor draws over the image of
its viewport, about the selection's *pivot*, which a drag of the left button moves along an axis or
in a plane, turns about an axis or scales, each drag one step of the history through an *open step*
([ADR-0161](adr/0161-the-open-step-and-the-handles.md)). Not the world's entity handles, nor the
asset cache's *request handles*.

**glTF.** The Khronos Group's format for 3D scenes: meshes, materials, textures and a node
hierarchy, as a JSON file with binary buffers, or as one GLB file with everything embedded. The
cooker, `slime-cook`, imports both ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Glyph atlas.** Textures that hold each *glyph*'s image once, for every text that draws it, in
pages of coverage for letters and of colors for emoji (`slime_ui_render::UiRenderer`): a glyph is
rasterized the first time a draw asks for it, and the glyphs that no draw asked for the longest
leave first once the atlas holds its budget
([ADR-0139](adr/0139-text-drawn-and-the-display-list.md)).

**Glyph key.** What names a *glyph*'s image: its *face*, its index there, its size, the *subpixel
position* of its origin, and its representation, coverage or color. Equal keys share one image
([ADR-0136](adr/0136-the-interfaces-text.md)).

**Glyph.** A shape that a font draws: a letter, a form that a letter takes when it joins its
neighbours, a ligature of several, a mark, an emoji. *Shaping* turns text into glyphs.

**GPU time.** How long the GPU took to run a render pass, measured by timestamps written at its
start and its end (`slime_gfx::GpuTimer`) and read back a few frames later; `None` on adapters
without timestamp queries. A `GpuTimer` times the passes of one *submission*, each apart. Each
*view*'s statistics report one per kind of pass (`scene_gpu_time`, `tone_mapping_gpu_time`, the
bloom's passes summed) with the CPU time of its part of the call (`slime_render::RenderStats`,
`View::stats`).

**Graphics API.** The interface between programs and GPU drivers: Vulkan (the engine's choice on
Linux) and Direct3D 12 (on Windows). wgpu translates the engine's calls into one of them.

**Guard (world).** What reaching a *component* to change it gives (`slime_world::Mut`): it reads
as the value, and its first write stamps the value with the world's *tick*, so that readers learn
that it changed; reaching a value without writing it changes nothing
([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md)).

**Half-precision float.** A 16-bit floating-point number (IEEE 754 binary16): 11 significant bits,
values up to ±65 504. The channels of `Rgba16Float` render targets, which the engine converts to
`f32` by hand when it reads them back (`Gpu::read_float_render_target`).

**Hardware tier.** A class of computers that the engine runs on, from handheld PCs and modest
computers at high frame rates to high-end ones: a game picks the lowest it supports, and each
technique of the renderer belongs to a tier ([ADR-0098](adr/0098-the-games-the-engine-aims-at.md)).
Three are named, each by a reference machine and budgets: handheld at 1280 × 800, modest at 1920 ×
1080 and high at 3840 × 2160; an enthusiast tier comes with ray tracing
([ADR-0114](adr/0114-hardware-tiers-and-the-gpus-limits.md),
[ADR-0138](adr/0138-the-high-tier-at-3840-2160.md)).

**Headless.** Running the engine without a window, renderer or audio output. It is a normal
configuration, used by automated tests and, later, by dedicated servers.

**High dynamic range (HDR).** Colors not limited to 0 to 1, such as the light a scene reflects
before it is mapped to the display's range by *tone mapping*. The engine draws them into
`Rgba16Float` render targets: the renderer draws the scene into one per *view*, at the size of the
rectangle it draws into times its *render scale*, which its tone-mapping pass reads.

**Hold (game time).** What a game sets while it is not ready to be played, a zone that loads: held
and not paused, game time stands still, without touching the pause that the player and the tools
set; steps asked for while paused run all the same (`Context::hold`,
[ADR-0122](adr/0122-the-development-channel.md)).

**Host.** What runs a game's *simulation*, and its *presentation* when it has one: it owns the loop,
the world, *game time* and *real time*, the input, the *commands* queued and the decision to stop
(`slime_app::Host`). The game's program (`slime_app::run`), which the editor is to start as its
child process and drive through the *development channel*, a test and one day a dedicated server
are hosts ([ADR-0107](adr/0107-the-execution-model.md),
[ADR-0110](adr/0110-how-the-editor-runs-a-games-code.md)).

**Hot reload.** Replacing, in a running game, the *cooked data* or the shaders that changed on
disk, without restarting the game. `slime-cook watch` cooks again the sources that change, and a
game started with `--watch` loads its scenes again when their cooked data changes, keeping what did
not change ([ADR-0076](adr/0076-hot-reload-of-cooked-data.md)), and the renderer's shaders when
their files change ([ADR-0077](adr/0077-hot-reload-of-shaders.md)). A game on a *world* carries its
run into the world spawned anew from the scenes that changed (`World::carry`,
[ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md)).

**Illuminance.** The light a surface facing a light receives from it, in *lux*, which the *BRDF*
turns into the light the surface reflects: a `DirectionalLight`'s `illuminance`, and what a point
or spot light gives at a distance (ADR-0035, ADR-0037).

**Image part (camera).** The part of a camera's image that a *view* draws, as fractions of the
whole image's width and height (`slime_render::ImagePart`): the whole image's projection is moved
and scaled so that the part fills the view, and parts drawn side by side make the whole image, for
a split screen of one camera or a magnifier. Its shadows are those of the whole image
([ADR-0126](adr/0126-views-apart-from-the-renderer.md)).

**Image test.** A test that renders offscreen and compares the result with a *reference image*,
pixel by pixel, within a tolerance that absorbs how drivers round colors and filter textures
([ADR-0018](adr/0018-reference-image-tests.md), amended by
[ADR-0028](adr/0028-image-tests-tolerate-a-few-pixels.md),
[ADR-0030](adr/0030-image-tests-strict-on-the-reference-driver.md) and
[ADR-0054](adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md)).

**Image-based lighting (IBL).** Lighting surfaces with the light of their whole surroundings, read
from images of it, here an *environment*, rather than from *punctual lights* alone.

**Imported target.** A target of the *render graph* that outlives the frame and that the graph
does not create: the output, which may be a window's image that changes every frame, and the
*shadow atlases*, the sun's cascades' and the spot lights'. It holds what it held before the frame,
so a pass may sample it without an earlier pass drawing into it ([ADR-0041](adr/0041-render-graph.md)).

**Importer.** The part of *cooking* that reads one kind of source file, glTF or an image, into the
engine's types.

**Index buffer.** A buffer of indices into a vertex buffer, so that the triangles of a mesh share
vertices (`slime_gfx::IndexBuffer`).

**Input (as data).** The keys and mouse buttons held, and those pressed and released since the last
frame (`slime_platform::Input`), which the *host* keeps from the window's events, or a test from its
script, and the *simulation* reads ([ADR-0107](adr/0107-the-execution-model.md)).

**Input method (IME).** The system's means of typing what a keyboard has no keys for, Chinese,
Japanese or Korean: the keys compose a text, shown in a field under a line, which the user then
commits (`slime_input::Ime`, [ADR-0145](adr/0145-text-edited.md)).

**Instance (render).** One thing the renderer draws: a *mesh*, a *material* and a transform from
the mesh's space to world space (`slime_render::Instance`). A render *scene* is a flat list of
them, built for each frame.

**Instancing.** Drawing with one draw call the *instances* that share a mesh and a material, each
reading its own values, its transform among them, from a storage buffer by its index
([ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md)). Each pass of the renderer draws so
the instances of one pipeline, material and mesh that it keeps (`slime_render::Renderer`,
[ADR-0117](adr/0117-frustum-culling-and-instancing.md)).

**Intent (navigation).** What a key, and later a gamepad, means to an interface whatever the
program: next, previous, up, down, left, right, accept, cancel (`slime_ui::action::Intent`). The
interface moves the *focus* for the first six ([ADR-0142](adr/0142-input-and-focus.md)).

**Interpolation factor.** The fraction of a fixed step accumulated but not yet run
(`FixedStep::alpha`), between 0 and 1. Used to draw a state between the last two fixed steps:
`World::interpolated_transform` draws what moved during the last one there
([ADR-0084](adr/0084-what-the-world-draws.md)). Games read it from
`GameTime::alpha`.

**Irradiance.** The light that reaches a surface from every direction of its hemisphere, each
weighted by the cosine of its incidence, in *lux*: a matte surface of albedo c reflects
c × irradiance / π. The roughest of the *prefiltered levels* holds the irradiance divided by π.

**Journal (editor).** The editor's *steps*, as text, written to a file as they are made, undone and
done again, each *command* naming entities by their *persistent identities*, after the text of the
level as it was when the journal started: an editor that crashed is recovered from it, with its
history, and a test or an agent replays an editing session
([ADR-0109](adr/0109-stable-references-and-undo.md), [ADR-0134](adr/0134-the-editors-core.md)).
The *editor's program* keeps one per level's file, and recovers by itself, as it starts, a journal
that holds steps that the level's file does not, which a crash or a stop without writing leaves
([ADR-0135](adr/0135-the-editors-program.md)).

**Key (element).** What names an *element* among its siblings, a name or a number, given by the code
that builds it (`slime_ui::tree::Key`); the keys from below its *root* down to it, after the root's
number, make its *path (element)*, which names it for agents, tests and saved layouts
([ADR-0140](adr/0140-the-tree-of-elements.md)).

**Kind (material).** A way of lighting a surface that the renderer knows by registration: its
shaders, its parameters described by reflection, and the passes it draws in. The physically based
material is the first kind, an *unlit material* the second; games register theirs from Stage 9a
([ADR-0111](adr/0111-the-frame-contract.md),
[ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md),
[ADR-0129](adr/0129-drawing-for-the-editor.md); `Material::kind`,
`Renderer::describe_materials`).

**Kit (prefabs).** The *prefabs* that a level is built from, one per variant of a piece of terrain,
a tree, a rock or a prop, each with its collider, placed and arranged in the editor rather than
scattered by a script ([ADR-0155](adr/0155-the-editors-design.md)). *Hunna Wood*'s zone moves onto
one at Stage 6c.

**KTX2.** The Khronos Group's file format for GPU textures, which holds every mipmap level of a
texture in its GPU format: the format of cooked textures
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Label (GPU object).** The name given to a GPU object when it is created, required by every
creation method of `slime_gfx::Gpu`. Error messages and GPU capture tools show it.

**lavapipe.** Mesa's Vulkan driver that renders on the CPU. The checks and CI use it on Linux, so
that GPU tests give the same results on every machine, including machines without a GPU. It is
the *reference driver* of the image tests.

**Layer (interface).** A root of an interface that the interface places over the others, beside an
element, at a point or floating, kept within the window, and that may close as the user leaves it,
a press outside it or Escape: a menu, a list to choose from's options, a tooltip, a floating panel,
a dialog (`Ui::open_layer`, [ADR-0150](adr/0150-layers.md)). Not a *layer* of a scene.

**Layer (scene).** A part of a level that holds a condition on the state of a game's story, such
as "after the quest X", and exists only while it holds; the editor shows or hides each layer
([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md)). Decided for Stage 9b, not built
yet.

**Layout (document).** How a *save* or a *scene file* lays out its sections and keys, which a
reader reads before their values (`slime_world::outline` reads a scene file's); its `layout` key
gives the version of that layout: `layout = 5` for the saves that hold each entity's differences
from its *base*, the only layout of saves read, and 1 for scene files
([ADR-0091](adr/0091-scene-documents-and-placements.md),
[ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md),
[ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md),
[ADR-0125](adr/0125-saves-per-scene.md)).

**Layout (zone).** In *Hunna Wood*, what decides how the game plays in its zone: the size of its
ground, its colliders, where the hero starts and the places of the key, the door, the campfire and
the terrace. A save gives the numbers of the layout of the zone that it was made on, and one of
another layout, a number further from the zone's than a small tolerance, is not resumed; *hot
reload* starts the run again when the layout changes
([ADR-0088](adr/0088-saving-and-resuming-a-run.md),
[ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md)).

**Leaf (reflection).** A part of a *value* that holds no other parts, which an edit sets whole: a
number, a boolean, a text, an option, a reference to an asset or an entity, an empty list, or an
enum whose variant holds no data (`slime_reflect::Registry::leaves`).

**Leaf card.** A quad textured with leaves, whose texture's alpha cuts them out (an *alpha mask*):
many of them make the foliage of a tree for a fraction of the triangles that modelled leaves
would take.

**Line (drawings).** A segment between two points of the world that a client draws into a view, as a
band that faces the camera, a width in the output's pixels wide, its edges smoothed over a pixel
(`slime_render::Line`, [ADR-0129](adr/0129-drawing-for-the-editor.md)).

**Linear color.** A color whose values are proportional to the light they stand for, so that
light adds and multiplies them as they are: twice the value is twice the light. Displays and
image files store colors *sRGB-encoded* instead. Every color of the engine's API is linear, a
`slime_color::LinearColor` ([ADR-0045](adr/0045-one-linear-color-type.md)).

**Log (world).** The entities that left a component's storage, per storage, and those destroyed,
for the world, each with its *tick* (`World::removed`, `World::destroyed`); past a limit, a log
forgets its older half, and a reader older than what it holds is told to read the whole world
again ([ADR-0119](adr/0119-the-worlds-ticks-blocks-and-log.md)).

**Logical key.** What a key means with the current keyboard layout and modifiers
(`slime_platform::Key`): on a French AZERTY keyboard, the first letter key of the top row gives
the logical key `"a"`. Suits shortcuts and text. See also *physical key*.

**Logical pixel.** A pixel of the window as the user sees it; the *scale factor*
(`Window::scale_factor`, `Event::ScaleFactorChanged`) says how many *physical pixels*, the
display's, it covers. The sizes of `WindowConfig` are logical; `Window::inner_size`,
`Event::CursorMoved` and render targets are physical.

**Look (the editor's).** The fonts and the style made for the interfaces of the engine's editor and
of its dev overlay: Geist and Geist Mono, a style sheet, and *themes* stacked, a base of scales, one
of colors, dark, light or high contrast, and a compact one (`slime_ui::look`, built with
`slime-ui`'s feature `editor-look`; [ADR-0148](adr/0148-widgets.md),
[ADR-0149](adr/0149-the-looks-foundations.md)).

**Lux (lx).** The unit of *illuminance*, lumens per square metre: the strength of a directional
light (`illuminance`); about 100 000 for direct sunlight, 400 at sunrise.

**Manifest (assets).** The list of what was cooked for a game: each *asset*'s ID, kind, cooked file
and size, and the IDs it needs to load ([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Marquee (editor).** A rectangle that the user drags over the *viewport* with the left button, which
selects what is drawn inside it, the outermost *placement* of each, as the GPU's *picking* finds it:
what is hidden is not selected ([ADR-0159](adr/0159-picking-and-the-selection.md)).

**Material.** How a surface looks, with glTF's *metallic-roughness model*: a base color,
metallic and roughness, normal, occlusion and emissive textures each read with a *sampler*, an
alpha mode and double-sidedness (`slime_render::Material`, from `MaterialData`;
[ADR-0035](adr/0035-physically-based-materials.md)). Its values can change while a game runs,
but not its textures, its sidedness or the kind of its alpha mode (`Material::set_values`, or
`Material::set_parameters` through reflection). It is of a *kind*, the physically based one; an
*unlit material* is of the other.

**MCP (Model Context Protocol).** An open protocol through which coding agents, such as Claude
Code and Codex, call the tools of other programs. The engine's own server, `slime-mcp`, lets them
drive a running game through its *development channel*, and edit a level through the editor's
program ([ADR-0099](adr/0099-ai-agents-drive-the-engine.md),
[ADR-0122](adr/0122-the-development-channel.md), [ADR-0135](adr/0135-the-editors-program.md)).

**Meaning (interface).** What an interface's tree says to whoever reads it without seeing it, an
agent or a screen reader: each element that a person reads or uses, its *role*, its name, a
field's value, its states, the actions it takes now, where it shows and its *path*; made when asked
(`Ui::meaning`, [ADR-0143](adr/0143-what-agents-see-of-the-interface.md)). What changed in it since
it was last taken is given element by element, each marked as its meaning changes and named by a
number that the interface never gives again (`Ui::take_meaning_changes`,
[ADR-0146](adr/0146-screen-readers.md)).

**Menu bar.** The row of an interface's menus, *File*, *View*, whose items open them below; the
keys move along it and between its menus, and F10 or Alt alone reaches it (`Element::menu_bar`,
[ADR-0150](adr/0150-layers.md)).

**Mesh.** Triangles given as vertices (a position, a normal, texture coordinates and a *tangent*
each) and indices, uploaded to the GPU into a *page* (`slime_render::Mesh`, from `MeshData`).

**Metadata file.** The file `<file>.meta` committed beside a *source asset*, which holds its *asset
ID*, a *document* of a `Metadata` value ([ADR-0069](adr/0069-the-asset-pipeline.md),
[ADR-0086](adr/0086-serialization.md)).

**Metallic-roughness model.** glTF's physically based material: a dielectric's diffuse color
under a specular layer, or a metal's reflection tinted by its color, blended by *metallic*, with
highlights as wide as the *roughness* (glTF 2.0, Appendix B; ADR-0035).

**MikkTSpace.** The common way of computing a mesh's *tangents* from its normals and texture
coordinates, which glTF's *normal textures* assume; the cooker computes them so
(`bevy_mikktspace`, [ADR-0069](adr/0069-the-asset-pipeline.md)).

**Milestone.** A small, verifiable step of a stage, listed with its goal, its done criteria and
how to test it in [`docs/PLAN.md`](PLAN.md). Milestones are numbered `M<stage>.<n>`, for example
M1.3; those of a *pass*, `P<n>`.

**Mipmaps.** Copies of a texture at half, a quarter, ... of its size, down to one pixel: the
*levels of detail* among which a *sampler* chooses by the size the texture is drawn at, so that a
minified texture does not alias. The engine computes them on the CPU when the texture is created
(`Gpu::create_texture`), in linear light for the sRGB formats, or takes them from the caller
(`Gpu::create_texture_with_levels`, `Gpu::create_cube_texture`); linear filtering blends two levels.

**Mirror (source assets).** The copies of the sample game's downloaded *source assets*, attached to
the releases of a GitHub repository of their own, from which `scripts/fetch-assets.sh` fetches
them ([ADR-0070](adr/0070-the-sample-games-source-assets.md)).

**Mirrored instance.** An instance whose transform has a negative determinant, such as a scale of
−1 along one axis: it turns front faces clockwise on screen, and the renderer culls its other
faces so that it keeps its front ones, as glTF does (ADR-0035).

**Modal root.** A root of an interface, a dialog over a window, that keeps the pointer from the
roots below it and the *focus* among its elements while it is there, and gives the focus back when
it is removed (`RootSettings::modal`, [ADR-0142](adr/0142-input-and-focus.md)).

**Mode of debugging (render).** What a *view* shows in place of the lit scene, to see what the
renderer computes (`slime_render::DebugMode`): the normal each surface is shaded with, the share of
the shadowing lights' light that reaches it, or its base color, written as colors as they are,
without the exposure, the bloom or the tone curve; or the light alone, the scene drawn as it is,
every surface white ([ADR-0127](adr/0127-the-settings-of-a-view.md),
[ADR-0159](adr/0159-picking-and-the-selection.md)).

**Multiple scattering.** Light that bounces between the microfacets of a rough surface before it
leaves, which a single-scattering *BRDF* loses: a white rough metal would look grey. The renderer
gives it back in the *environment*'s light, as the Khronos glTF Sample Viewer does, not in the
lights'.

**Multisample anti-aliasing (MSAA).** *Anti-aliasing* in which the GPU tests whether a triangle
covers each pixel at several points, its *samples (multisampling)*, and shades the pixel once for
the samples it covers; the *resolve (multisampling)* averages them into one pixel. It smooths the
edges of triangles, not what a shader draws inside them.

**Native backend.** An implementation of `slime-gfx`'s engine-owned API on a *graphics API*
directly, Vulkan or Direct3D 12, without wgpu; planned at Stage 9c of the roadmap
([ADR-0098](adr/0098-the-games-the-engine-aims-at.md)).

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

**Notification.** A short message that a program gives the interface, which stacks it at the
window's bottom end and takes it away after a time that its kind sets, information, success,
warning or error; screen readers read it as it comes (`Ui::notify`,
[ADR-0151](adr/0151-rich-widgets.md)).

**Occlusion texture.** A texture whose red channel says how much of the *environment*'s light
reaches each point of a surface, darkening creases (glTF).

**Offscreen rendering.** Drawing into an image that no window shows, a *render target*, for
example to read it back to the CPU. Image tests render this way, without a windowing system.

**OKLCH.** A color's lightness, chroma and hue in Oklab, a space of colors in which equal steps
look equal: the interface's *themes* lighten, darken and mix their colors in it, so that a step of
lightness looks the same on blue as on yellow (`slime_color::Oklch`,
[ADR-0149](adr/0149-the-looks-foundations.md)).

**Open in context.** Editing a *placement*'s prefab in place, inside the level that places it, its
changes written to the prefab's file ([ADR-0155](adr/0155-the-editors-design.md)). Decided, not
built yet.

**Open step (editor).** The *commands* of a gesture that a person makes, a handle or a slider
dragged, which the editor's core applies to the level at once but neither journals nor keeps in its
history until the gesture ends: letting go makes them one *step*, Escape undoes them. Agents' steps
wait while one is open (`Editor::gesture`, [ADR-0155](adr/0155-the-editors-design.md),
[ADR-0161](adr/0161-the-open-step-and-the-handles.md)).

**Operation (editor).** One of the actions of the editor, named in one registry with its label and
its parameters, `file.save`, which menus, shortcuts, the palette and agents run alike, by its name;
its shortcut lives in the editor's table of shortcuts. Those that make a step enter the history,
those that change the selection or the window do not. Not a world's *command*, which an operation
may make (`slime_editor::OPERATIONS`, [ADR-0155](adr/0155-the-editors-design.md),
[ADR-0156](adr/0156-the-editors-window-and-its-operations.md)).

**Orthographic projection.** A camera's projection in which what is farther is as large, every
pixel seen along the camera's axis, as a map or a plan is drawn (`slime_render::Projection`): a box
as high as the camera's height, as wide as its aspect makes it, from its near plane to its far one,
beyond which nothing is seen ([ADR-0126](adr/0126-views-apart-from-the-renderer.md)). See
*reversed depth*.

**Outermost placement.** Of an entity, the *placement* that holds it and that no placement holds,
`yard/lamp 1` for `yard/lamp 1/post/light`, or the entity itself when it belongs to none
(`World::placement_root`): what a click in the editor's *viewport* selects first, a click again
going a level deeper ([ADR-0159](adr/0159-picking-and-the-selection.md)).

**Outline (render).** A line drawn around instances over a *view*'s image, as an editor shows what
is selected: from a mask of where they are, drawn at the view's *render scale*, and the scene's
depth *resolved* into one sample, in one color where the scene shows them and another where a
surface in front hides them (`slime_render::Outline`,
[ADR-0159](adr/0159-picking-and-the-selection.md)).

**Overlay (dev).** Panels on the engine's interface, in the editor's look, floating over a
program's view, to show statistics and tooling over a running game: the frame's figures, the
settings of its views, its world panel (`slime_devui::Overlay`,
[ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)). It was drawn with egui until
Stage 6b.

**Override.** A value that a *placement* sets on its *prefab*: a value changed, a component added
or removed, an entity of the prefab removed or one added, written by the scene that places it. It
stays when the prefab later takes the same value, and one whose target the prefab no longer has is
kept and reported ([ADR-0089](adr/0089-scenes-and-prefabs.md)).

**Owner.** The person who validates the project's direction and its architecture decisions.

**Page (meshes).** A vertex buffer and an index buffer that hold the vertices and indices of many
*meshes*, each in a range of its own, so that draws of different meshes bind no other buffer: a
renderer and its loads place their meshes in its pages (`slime_render::Mesh`,
[ADR-0117](adr/0117-frustum-culling-and-instancing.md)).

**Palette of commands.** A layer that the interface opens over a window, a field and a list, which
finds every command that the program names by the letters typed, in their order, and gives the
chosen one to the program as a shortcut gives its command (`Ui::open_palette`,
[ADR-0151](adr/0151-rich-widgets.md)).

**Pane (editor).** A part of the editor's window that holds a *viewport (editor)*: its surface, its
camera and how the user moves it, what its buttons pick, its settings. Two panes show the level
side by side; the View menu changes the one last pressed in
([ADR-0160](adr/0160-two-viewports-and-blenders-way.md)).

**Part (level).** A scene file that a level places and that the editor edits in place, writing it to
its own file, named in a table of the level's file of its own, `[parts.<id>]`; what a level places
otherwise is a *prefab* ([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md),
[ADR-0155](adr/0155-the-editors-design.md)). Decided, not built yet.

**Partial value.** A *value* that leaves some of its fields out, those of its structs and variants
at any depth, its tuples, lists and options whole: what changed of a value, as a save holds it. An
*upgrade* brings it forward, told that it is partial ([ADR-0086](adr/0086-serialization.md)).

**Pass (plan).** Milestones between two stages that fix or decide rather than add a stage's
features, such as the audit pass and the foundations pass before Stage 6
([ADR-0097](adr/0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md)), not a render
pass.

**Path (element).** What names an element of an interface from one frame to the next: the number of
its root, then the keys from its root down to it, written `#0/bar/open`, a number after `#`
(`Ui::path_text`, [ADR-0143](adr/0143-what-agents-see-of-the-interface.md)). Not a *property path*.

**Percentage-closer filtering (PCF).** Reading a *shadow map* by comparing a point's depth with
several stored depths around it and averaging the results, so that the edges of shadows are soft
instead of blocky. A *comparison sampler* with linear filtering does it over 2 × 2 pixels in the
GPU; the renderer makes nine such comparisons over 5 × 5 texels, placed and weighed as Castaño
proposed, so that a shadow's edge fades smoothly over about six texels
([ADR-0063](adr/0063-a-smoother-shadow-filter.md)).

**Persistence (save).** What becomes of what the run changed of a scene's entity when the scene is
*unloaded*, as the component `slime_world::Persistence` says it, on the scene's root for its scene
or on an entity for itself: *persistent*, kept and applied again when the scene is spawned again, a
door broken open; *reset*, forgotten, the scene spawned again giving the entity as its file does, an
enemy that comes back. What is never saved is *Unsaved* or *driven*
([ADR-0112](adr/0112-levels-saves-and-worlds-for-open-worlds.md),
[ADR-0125](adr/0125-saves-per-scene.md)).

**Persistent identity.** How files and saves name an *entity*: the key that a scene was spawned
under, then the path of the IDs local to each file through the *placements* that placed it, such as
`zone/door` or `zone/lamp 3/light`, where an imported glTF node's ID is its name, or its index,
`#12`, when it has no name of its own; for an entity created outside a scene, a key that the game
chose, one part, as a scene's key, `camera` (`World::spawn_keyed`), or `created/<number>`, its rank
in the world's count of what it created so, which loading a save gives, for what the run created,
the numbers that follow the count of the world it is loaded into; and for an entity of a
*placement* made while a world runs, its root's followed by its path in the prefab,
`created/7/light` ([ADR-0079](adr/0079-the-engines-own-world.md),
[ADR-0087](adr/0087-the-worlds-documents.md),
[ADR-0091](adr/0091-scene-documents-and-placements.md),
[ADR-0092](adr/0092-a-world-written-back-as-a-scene.md),
[ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md)). *Commands* name the entities
that they change by it (`slime_world::Command`); an entity's handle being a cache of it in the
editor is decided, not built yet ([ADR-0109](adr/0109-stable-references-and-undo.md)).

**Peter-panning.** A shadow that starts a little away from the object that casts it, because the
*depth bias* pushes the stored depths back, or the *normal offset* moves the point looked up: in
the renderer, by up to 4.5 of the shadow map's texels.

**Phase (frame).** One of the steps of a frame, in their order: start, where the *host* applies the
*commands*, fixed steps and update, where the *simulation* changes the world, transforms,
extraction, where the *presentation* reads it, render and present, where it draws without it
([ADR-0107](adr/0107-the-execution-model.md),
[ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md)).

**Physical key.** The position of a key on the keyboard, whatever the layout
(`slime_platform::KeyCode`), named after the character it produces on a US keyboard: the first
letter key of the top row is `KeyQ` on every layout, whether it is labelled Q (QWERTY) or A
(AZERTY). Suits movement keys. See also *logical key*.

**Physical pixel.** A pixel of the display. See *logical pixel*.

**Picking.** Finding which object is under the mouse: by the GPU, from an image of an area of a
*view* where each pixel holds the number that its client gave the instance drawn nearest there,
read back a frame or more later without holding up a frame (`View::pick`); or by the CPU, a ray
against the boxes of the meshes that a world places, then their triangles, without a GPU
(`slime_world_render::Picker`). The bridge from the world numbers each instance with its entity's
handle ([ADR-0115](adr/0115-what-the-gpu-layer-gives-the-editor.md),
[ADR-0130](adr/0130-picking.md)).

**Play (editor).** The editor running the game's program on the level as the editor holds it,
steps not written included, as private copies of the scenes that the game reads in place of their
cooked files, the user's files left alone; paused, stepped and stopped from a toolbar, its log in a
console ([ADR-0164](adr/0164-play.md)). The steps made while it runs are sent to the game as the
texts of the commands that they applied ([ADR-0165](adr/0165-editing-while-playing.md)). Play
builds the game with cargo as it starts it, and again as its code changes while it runs, the game
then starting again on a save of its run ([ADR-0166](adr/0166-building-the-game-again.md)).

**Pivot (handles).** The point about which the editor's *handles* stand, turn and scale: the middle
of the origins of the outermost entities selected, those without an ancestor in the selection
([ADR-0161](adr/0161-the-open-step-and-the-handles.md)).

**Placement.** A *prefab* placed in a scene, under an ID of that scene, with its *overrides*, or
while a world runs, under `created/<number>`; the entities of a placement are named through it, as
`zone/lantern 3/light`. Not an *instance (render)* ([ADR-0089](adr/0089-scenes-and-prefabs.md)).

**Pose (skeleton).** Where each bone of a skeleton is at a moment: one component of flat arrays,
never saved, rather than an entity per bone
([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md)). Decided, not built yet.

**Pre-exposure.** Multiplying the scene's light by the *exposure* in the scene pass, before the
*HDR* target is written, so that light in physical units fits *half precision*
([ADR-0037](adr/0037-lights-in-physical-units-and-pre-exposure.md)); the target holds that light
divided by 64, so that up to 64 × 65 504 keeps its energy for the *bloom*
([ADR-0040](adr/0040-bloom.md)).

**Prefab.** A scene placed many times, each *placement* keeping values of its own, its *overrides*,
such as a lantern placed along a path: a role that any scene plays, not a kind of file. Stage 5b
brought them ([ADR-0078](adr/0078-stage-5-in-two-halves.md),
[ADR-0089](adr/0089-scenes-and-prefabs.md)).

**Prefiltered levels.** The levels of an *environment*'s *cube texture* past the first: the sky
averaged under the reflection lobe of a growing roughness, so that one lookup gives what a rough
surface reflects ([ADR-0039](adr/0039-image-based-lighting-computed-on-the-cpu.md)).

**Premultiplied (color).** A color whose red, green and blue are already multiplied by its alpha, as
`slime_gfx::Blend::Premultiplied` lays it over a target: a shape that covers a pixel in part
multiplies its color and its alpha alike, and an image whose texels are so before they are filtered
mixes without the dark fringes that a transparent texel's color leaves otherwise. The *canvas*'s
colors are ([ADR-0137](adr/0137-the-canvas.md)).

**Present (an image).** Hand a drawn image to the window, which shows it at one of the display's
next refreshes, as the *present mode* says.

**Present mode.** How a window shows the images presented to it (`slime_gfx::PresentMode`). With
*Mailbox*, the engine's default, each image replaces the one waiting to be shown and presenting
never waits; with *FIFO*, every image is shown in order, one per refresh, and presenting waits
while earlier images are still to be shown ([ADR-0021](adr/0021-frame-pacing.md)).

**Presentation.** The part of a game that shows it: the camera, the drawing, the dev overlay, later
sound (`slime_app::Presentation`). It reads the world, and nothing of the *simulation* but the
world, without writing it, and asks for changes as *commands*
([ADR-0107](adr/0107-the-execution-model.md),
[ADR-0121](adr/0121-a-game-as-a-simulation-and-a-presentation.md)).

**Priority (assets).** How urgently a *request* wants its assets, one of three classes
(`slime_render::Priority`): *now*, what a view draws and lacks; *soon*, what a game fetches ahead;
*later*, what is fetched far ahead and the editor's thumbnails. The loads' tasks and their copies to
the GPU go class by class, then in the order the loads started
([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)).

**Property path.** Where a part of a reflected *value* is, as fields joined by `.` and indices in
brackets: `color.r`, `ids[2]` (`slime_reflect::Path`). A field of an enum is one of the variant
the value holds; an option that holds a value is gone through, to the value.

**Punctual light.** A light without size, from one direction or one point, as glTF's
`KHR_lights_punctual` defines it (`slime_render::Light`): a directional light, whose strength is an
*illuminance*; a point light, whose strength is an intensity in *candelas*, shining in every
direction; and a spot light, a point light in a cone, full within its inner angle and dark beyond
its outer one. A point or spot light's optional range is the distance at which its light reaches
zero, smoothly.

**Query (world).** What a game asks a *world* for: the entities that hold every *component* of
some types, with those components, `&T` to read one and `&mut T` to change it through a *guard*
(`slime_world::Query`, `slime_world::QueryMut`,
[ADR-0082](adr/0082-the-worlds-storage-and-handles.md)).

**Rank (hierarchy).** An entity's place among its parent's children, the first at 0: what a
*command* that creates or moves an entity gives, and what a deletion records, so that undoing it
puts each entity back where it was (`World::set_parent_at`, `World::rank`).

**Raw motion.** The mouse's own motion, before the system's acceleration, which no edge of the
screen stops, reported while the window holds the cursor alone (`Event::RawMotion`, *cursor held*).

**Readback.** A copy of a *render target*'s texels to the CPU. `Gpu::read_render_target` waits
for the GPU to finish what was submitted before; `Gpu::read_render_target_later` starts the copy
and returns at once, its texels given once wgpu has seen the copy done, a frame or more later
([ADR-0130](adr/0130-picking.md)).

**Real time.** The frames' deltas, limited after a pause, whose sum is *application time*: the UI,
the camera, the editor and the dev overlay run on it whatever the *game time*
([ADR-0107](adr/0107-the-execution-model.md)).

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

**Reflection.** What lets the engine describe the values of a type that it did not know when it was
compiled: list their properties, read and change them by name, write them to files. A type gets it
from `#[derive(Reflect)]`, under a *stable name*, and is registered in a *type registry*
([ADR-0080](adr/0080-reflection-and-serialization.md), [ADR-0085](adr/0085-reflection.md)).

**Releasable (asset).** An asset that an *asset cache* holds and no *request handle* holds any
more: kept while its kind's budget has room, so that what leaves the screen for a moment does not
load again, and dropped, least recently held first, once it has none
([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)).

**Render graph.** The renderer's description of a frame as passes, each declaring the targets it
draws into and the ones it samples (`crates/slime-render/src/graph.rs`, private): it orders the
passes by those targets, creates the *transient targets*, records the passes, four to a
*submission*, and gives each one's *GPU time* ([ADR-0041](adr/0041-render-graph.md)). It is built
again when the frame's *shape* changes.

**Render scale.** The size of a *view*'s targets against that of the rectangle it draws into,
from 0.25 to 2 (`slime_render::ViewSettings::render_scale`): below 1 it draws fewer pixels, which
the tone mapping stretches over the rectangle; above 1, more, which it averages
([ADR-0127](adr/0127-the-settings-of-a-view.md)).

**Render target.** An image the GPU draws into (`slime_gfx::RenderTarget`): an offscreen image,
which can be read back to the CPU and sampled by later passes, or the image of a window for one
frame, which can be neither.

**Renaming (commands).** The new identities that pasted *commands* create what they copied under,
one of the level for each entity that a command creates, which the identities that they name
afterwards follow, references included (`slime_world::Renaming`, `Command::read_renamed`,
[ADR-0162](adr/0162-creating-copying-and-arranging.md)).

**Report (development channel).** A named value that a game gives the tools that drive it, described
by *reflection*: its *asset cache*'s requests and memory, its views' statistics
(`slime_app::Presentation::reports`, [ADR-0128](adr/0128-asset-requests.md)).

**Request (assets).** Assets asked of an *asset cache* together, at a *priority*, which arrive
together (`slime_render::Request`); it holds each through a *request handle*
([ADR-0113](adr/0113-asset-requests-residency-and-priorities.md)).

**Request handle.** A hold on one asset of a *request* (`slime_render::AssetHandle`): while it
lives, the cache keeps the asset, or keeps loading it; its last handle dropped stops the asset's
load, or makes the asset *releasable*. It says what became of the asset, and stays valid through new
*cooked data* and a lost *device* ([ADR-0128](adr/0128-asset-requests.md)).

**Resolve (multisampling).** Averaging the *samples (multisampling)* of each pixel of a multisampled
*render target* into an ordinary one, which later passes can read. The renderer weighs each sample
by its brightness, so that the edges of bright surfaces are smooth too
([ADR-0060](adr/0060-resolving-the-samples-weighed-by-their-brightness.md)). Its depth is resolved
for an *outline* by taking the farthest of the samples
([ADR-0159](adr/0159-picking-and-the-selection.md)).

**Resolved scene.** A scene resolved into what it spawns (`slime_world::ResolvedScene`): its
entities, each with its parent, its components and its path from the scene's root, the *prefabs*
that it places resolved with it and their *overrides* applied. A *scene library* resolves scene
files, and `ResolvedScene::from_cooked` a glTF scene alone; a world spawns it under a key, as often
as it needs ([ADR-0091](adr/0091-scene-documents-and-placements.md)).

**Reversed depth (reversed-Z).** Depth that runs from 1 at the near plane to 0 infinitely far,
the far plane being at infinity ([ADR-0047](adr/0047-reversed-depth-with-an-infinite-far-plane.md)),
or, for an *orthographic projection*, linearly to 0 at its far plane.
A perspective projection crowds far distances near one end of the depth range; reversed, that end
is 0, where floating-point depths are finest, so that 32-bit depths keep a precision relative to
the distance, at any distance. Passes clear *depth targets* to 0, a depth test keeps the greater
depth, and a *depth bias* is negative.

**Revision (interface).** A number that the code building a panel of an interface reads and moves
when what the panel shows changes: `Ui::build` runs the builder only when it moved
([ADR-0140](adr/0140-the-tree-of-elements.md)).

**Role (interface).** What an element is to whoever reads the interface without seeing it, named as
AccessKit names roles: `Button`, `Label`, `List`, `ListItem`, `Dialog`; given by its kind, or by its
builder (`slime_ui::tree::Role`, [ADR-0143](adr/0143-what-agents-see-of-the-interface.md)).

**Root (interface).** A surface that an interface lays its *elements* out on, a window or any
target, with its size in *logical pixels*, its scale and the direction of its lines
(`slime_ui::tree::RootSettings`); a root laid out from right to left is mirrored
([ADR-0140](adr/0140-the-tree-of-elements.md)).

**Run of text (screen readers).** A piece of one line of a field's text, of one direction and of a
bounded count of characters, through which a screen reader reads the text and follows its caret:
each character's length, place and width, and where its words start (`slime_ui::tree::TextRun`,
[ADR-0146](adr/0146-screen-readers.md)). Not a run of *glyphs*.

**Run.** Two things. The running of a game by `slime_app::run`, from its `start` methods to its
`stop` methods; and, in a game, a play from a new game on, which R starts again, a *save* holds and
a resume goes on with.

**Sample (multisampling).** One of the points of a pixel at which *multisample anti-aliasing*
tests coverage and depth; not the *sample* of a stage.

**Sample game.** The game in `games/` that every stage grows from Stage 3c on, as its *sample*:
*Nightwatch*, then *Hunna Wood* from Stage 4a ([ADR-0067](adr/0067-hunna-wood-replaces-nightwatch.md)).

**Sample.** What demonstrates the exit criterion of a stage: an example up to Stage 3b, the *sample
game* from Stage 3c. Every stage ends with one.

**Sampler.** How shaders read a texture: nearest or linear filtering, and what lies beyond its
edges (repeat, mirror or clamp) (`slime_gfx::Sampler`).

**Save (game).** What a run changed of a *world* since the game *settled* it, its *base*, written to
a file: the scenes spawned under keys and the *placements*, by their assets, entities removed and
created, components added, removed and changed, parents changed, entities named by their
*persistent identities*, and what the world keeps of the scenes that it *unloaded*; it begins with
what the game says of it, such as what it was made on, which can be read alone. Loading applies it
to a *start world* made from the scenes as they are now, spawning the scenes that it lacks, so that
a game update that changes a scene reaches the saves made before; what the run created takes the
numbers that follow that world's count. It costs what changed
([ADR-0079](adr/0079-the-engines-own-world.md),
[ADR-0087](adr/0087-the-worlds-documents.md), [ADR-0088](adr/0088-saving-and-resuming-a-run.md),
[ADR-0093](adr/0093-saves-and-hot-reload-with-placements.md),
[ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md),
[ADR-0125](adr/0125-saves-per-scene.md)).

**Scene (cooked).** A glTF file's node hierarchy as the cooker makes it
(`slime_assets::CookedScene`): each node's name, parent, transform, meshes, light, `extras` and
*typed properties*, which a *resolved scene* turns into *entities* with *persistent identities*,
spawned under a key or placed in a *scene file* as a *prefab*. A *save* names the scenes that its
world spawned ([ADR-0069](adr/0069-the-asset-pipeline.md),
[ADR-0083](adr/0083-transforms-and-the-hierarchy.md), [ADR-0095](adr/0095-cooked-scene-files.md)).

**Scene (render).** What the renderer draws in one frame: *punctual lights*, an *environment*, an
*exposure*, a *bloom* strength and *instances* (`slime_render::Scene`), which it owns, drawn
through one or more *views*, each with its own camera. Not a scene
graph: the renderer keeps no scene between frames; `slime-world-render` builds one from the *world*
at each frame ([ADR-0084](adr/0084-what-the-world-draws.md)).

**Scene file.** A scene written as TOML, with the extension `.scene`: its entities under IDs of the
file, their parents and components, the *prefabs* that it places with their *overrides*, and the
assets that it uses, by short names; the rustdoc of `slime_world::SceneLibrary` lays out its keys.
Written by hand, or from a world, its placements kept as placements
(`slime_world::World::write_scene`). The cooker checks its layout and cooks it into its *binary
encoding*; a *scene library* reads it and resolves it
([ADR-0089](adr/0089-scenes-and-prefabs.md), [ADR-0091](adr/0091-scene-documents-and-placements.md),
[ADR-0092](adr/0092-a-world-written-back-as-a-scene.md),
[ADR-0095](adr/0095-cooked-scene-files.md)).

**Scene library.** The scenes that a game has, by their asset IDs (`slime_world::SceneLibrary`):
glTF scenes and *scene files*, read from cooked data when first resolved, or given one by one; each
resolved once into a *resolved scene*, the prefabs that it places with it, then shared, and read
again when new cooked data changes it ([ADR-0091](adr/0091-scene-documents-and-placements.md),
[ADR-0095](adr/0095-cooked-scene-files.md)).

**Set aside.** Kept but not applied, and reported: an *override*, a reference or a part of a *save*
whose target the *prefab* or the world no longer has (`slime_world::SetAside`,
[ADR-0089](adr/0089-scenes-and-prefabs.md)). In *Hunna Wood*, a save that is not resumed is set
aside too: renamed beside the save's file, so that the next save does not write over it
([ADR-0088](adr/0088-saving-and-resuming-a-run.md)).

**Settle (world).** Make what a *world* holds the *base* of its *saves*
(`slime_world::World::settle`): a game settles its world once it made the start of a run, and again
once it prepared a scene that it spawned while the run goes on, so that what it adds to the scene as
it spawns it is not saved ([ADR-0125](adr/0125-saves-per-scene.md)).

**Shadow acne.** Speckles of shadow that a surface casts on itself where its depth and the one
stored in the *shadow map* differ only by rounding or by the map's texels; a *depth bias* and a
*normal offset* remove them.

**Shadow atlas.** One *depth target* split into square tiles, each a *shadow map*: of one spot light
([ADR-0058](adr/0058-shadows-for-every-spot-light-in-one-atlas.md)), or of one *cascade* of the
directional light ([ADR-0064](adr/0064-cascaded-shadow-maps.md)), each in an atlas of its own.

**Shadow map.** A *depth target* drawn from a light's point of view by a *depth-only pass*: a
point is in shadow where something nearer to the light is stored at its place in the map. The
renderer draws, for each *view*, one for each *cascade* of the first directional light that casts
shadows, through an orthographic projection that covers the cascade's slice of what that view's
camera sees, its
texels on a grid fixed in the world ([ADR-0038](adr/0038-shadow-mapping.md),
[ADR-0050](adr/0050-a-shadow-region-that-follows-the-camera.md)), and, once per frame, one for each
spot light that casts shadows, in perspective.

**Shape (of a frame).** What decides a *view*'s passes and targets: the size of its targets, its
anti-aliasing, whether the sun's and the spot lights' shadow passes, the sky and the bloom run,
whether its rectangle covers its whole output, and whether anything is drawn after the sky or over
the image. The *render graph* is built again, and its GPU times start over, when it changes
([ADR-0041](adr/0041-render-graph.md)).

**Shaping.** Turning a run of text into the *glyphs* that draw it and their positions: the forms
that a script's letters take beside each other, ligatures, marks placed on their letters. parley
shapes the engine's text ([ADR-0136](adr/0136-the-interfaces-text.md)).

**Sheet of shortcuts.** A layer that the interface opens over a window, which lists the chords of
the tables of shortcuts by scope, each with the name of its command (`Ui::open_shortcut_sheet`,
[ADR-0151](adr/0151-rich-widgets.md)).

**Shortcut.** A chord, modifiers and a key named by its meaning, `Ctrl+Z`, that names a command in
a scope, `global` or the class of an element around the *focus*, read from a table of shortcuts,
a document cooked from a `.keys` file (`slime_ui::shortcuts::Shortcuts`,
[ADR-0142](adr/0142-input-and-focus.md)). A chord may name a button of the mouse in place of a
key, `Shift+MiddleButton`: it runs no command of its own, and the program reads what it names, as
the editor reads what each button does over its viewport (`Ui::button_command`,
[ADR-0160](adr/0160-two-viewports-and-blenders-way.md)).

**Simulated time.** Frame deltas supplied as a constant duration instead of being measured, so
that a run gives the same results on every machine. Headless runs use it.

**Simulation.** The part of a game that holds its rules (`slime_app::Simulation`): it changes the
world on *game time*, from *input* given as data, without a GPU or a window, and asks its *host* to
stop rather than ending the process ([ADR-0107](adr/0107-the-execution-model.md)).

**Skeleton (crate).** At Stage 0, a crate that existed with its documentation and inherited the
workspace policies, but held no code yet; no crate is one today.

**Sky pass.** The render pass that draws the *environment* and the *sun's disc* where no surface
was drawn, after the scene pass.

**Sky.** The procedural daytime sky from which the renderer computes an *environment*
(`slime_render::Sky`): a gradient from the zenith to the horizon, a haze into the ground below,
and an optional sun, whose glow, the light the sky scatters around it, it holds. The sun's disc is
drawn by the *sky pass*, not held.

**Slot (element).** An *element* whose children its own builder gives, by its own *revision*, kept as
they are when its parent is built again (`Children::add_slot`): a panel
([ADR-0140](adr/0140-the-tree-of-elements.md)).

**Snapping (editor).** The editor's *handles* moving what they drag by steps: the *pivot* onto the
world's grid of a quarter of a metre in the world's axes, by quarters of a metre in the selection's,
turns by fifteen degrees, scales by tenths, while *Edit* > *Snap* is ticked; the user's, never the level's, and never undone
([ADR-0161](adr/0161-the-open-step-and-the-handles.md)).

**Source asset.** A file that people make or download and that *cooking* imports: a glTF scene, a
PNG or JPEG image, or a *scene file*. Its *metadata file* is committed beside it; the file itself
is committed too, or fetched from the *mirror* ([ADR-0069](adr/0069-the-asset-pipeline.md),
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
into sRGB targets; `LinearColor::from_srgb8` decodes bytes on the CPU. Seen without its encoding
(`RenderTarget::without_srgb`), an sRGB target stores what is written as it is, so that the
*canvas* mixes encoded values.

**Stable name.** The name a reflected type is known by in a *type registry*, in files and in saves:
bare for the engine's types (`Transform`), `<crate>::<Type>` for a game's (`hunnawood::Collider`),
never a module path, so that moving a type breaks nothing; a former name stays as an alias
([ADR-0080](adr/0080-reflection-and-serialization.md)).

**Stage.** A step of [`docs/ROADMAP.md`](ROADMAP.md), ordered by dependency, with a goal and an exit
criterion. Stages are numbered from 0; a stage may be split (Stage 3a and 3b), and a stage added
later takes a letter (Stage 3c), so that the numbers cited elsewhere stay right.

**Staging buffer.** A buffer in memory that the CPU writes and the GPU reads, from which a copy
brings bytes into a texture or a buffer that the GPU alone reads fast. The *uploader* keeps them
from one upload to the next.

**Start world.** The *world* as a game makes it at the start of a run, from the scenes it spawns and
as it prepares them, then *settles*: the *base* of its *saves*, and the world that a save is loaded
into (`slime_world::World::load`, [ADR-0125](adr/0125-saves-per-scene.md)).

**Step (editor).** What the editor undoes and redoes as one: a label and the *commands* made
together, applied in their order, the whole step made or none of it, with what was selected before
and after it (`slime_editor::Editor`, [ADR-0134](adr/0134-the-editors-core.md)).

**Storage buffer.** A buffer that shaders read as an array they index, `var<storage, read>` in WGSL
(`slime_gfx::StorageBuffer`): the renderer's instances find their values in one by their number
([ADR-0117](adr/0117-frustum-culling-and-instancing.md)). See *uniform buffer*, which holds the
values that a whole draw reads.

**Stretch.** How much an entity's own meshes and shapes are scaled along its own axes, which its
children do not inherit: a wall longer than it is tall. Children inherit a uniform scale only, so
that none is ever sheared (`slime_world::Transform`,
[ADR-0083](adr/0083-transforms-and-the-hierarchy.md)).

**Strict and lenient reading.** How a *document* is read: strictly for a source, which people write,
where a field that its type does not have is an error; leniently for a save, which an earlier
version of a program may have written, where such a field is left out with a warning
([ADR-0080](adr/0080-reflection-and-serialization.md)).

**Structure counter.** The latest *tick* at which a world's entities were spawned or destroyed,
changed parent or name (`World::structure`), so that a list of the entities is made again only
then ([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md)).

**Style sheet.** A document of rules that style an interface's *elements*: each selects elements by
kind, *classes*, *key*, states and ancestor, and sets a part of their style, its fields given or
named by a *theme*; the rules apply in the order of the file, and an element's own style wins over
them (`slime_ui::theme::StyleSheet`, [ADR-0141](adr/0141-style-sheets-and-themes.md)). Cooked from
a `.style` file.

**Sub-asset.** An *asset* inside a source file, such as a mesh, a material or a scene of a glTF
file, whose ID is derived from the file's ID, its kind and its name
([ADR-0069](adr/0069-the-asset-pipeline.md)).

**Submission.** Recorded GPU work handed to the GPU at once (`slime_gfx::Gpu::submit`, or the single
pass of `Gpu::render`): one call into the driver, and a unit of synchronization on the GPU. Each
costs CPU time, so the renderer records its passes sixteen to a submission, a whole frame of the
samples in one ([ADR-0158](adr/0158-the-viewport.md)).

**Subpixel position.** One of the positions between two pixels at which a *glyph*'s origin is
rasterized, across the line, so that glyphs are spaced as their advances say rather than rounded
to whole pixels ([ADR-0136](adr/0136-the-interfaces-text.md)).

**Sun's disc.** The disc of the sun's apparent size, 0.533°, that the *sky pass* draws, whose
radiance gives a surface facing it the sun's illuminance; it lights nothing, the scene's
directional light carrying the sun's light.

**Surface (interface).** An element of an interface that shows an image that the program draws, an
editor's *viewport*, and gives the program the pointer over it (`slime_ui::tree::Kind::Surface`,
[ADR-0158](adr/0158-the-viewport.md)). Not the GPU's *surface*.

**Surface.** The link between the GPU and a window: the images the window shows. Each frame, the
engine *acquires* one, the application draws into it, and the engine *presents* it
(`slime_gfx::Surface`). The graphics API can report it *lost*: it is then created again, and given
up, which stops the run, when that fails or when it is lost again each time
([ADR-0025](adr/0025-giving-up-a-lost-surface.md)).

**Tangent.** A direction in a surface, along which the texture's u coordinate grows. With the
normal and the *handedness* w (±1, −1 where the texture is mirrored on the surface), it gives the
bitangent, cross(normal, tangent) × w, and with it the *tangent space* in which *normal textures*
are stored (`MeshData::tangents`, glTF's convention).

**Task pool.** Threads that run small pieces of work, *tasks*, as they come, each on the first
thread free. The engine has one, `slime-tasks`, the machine's logical cores but one, whose tasks
have one of two priorities: a *frame* task, work that the frame being made waits for, such as the
sky's environment, which a free thread takes first; a *background* task, work that no frame waits
for, such as a load or the cooker's encoding, which never holds every thread
([ADR-0107](adr/0107-the-execution-model.md),
[ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md)).

**Termination signal.** A request from outside the process to stop it: SIGINT (Ctrl+C in a
terminal), SIGTERM (a service manager or `kill`) or SIGHUP (the terminal closing). The first one
stops a run cleanly; a second one, half a second or more later, ends the process at once
([ADR-0012](adr/0012-clean-shutdown-on-termination-signals.md)).

**Texture role.** What a cooked texture is used for: colors, data or normals, which sets how the
cooker averages its levels and compresses it ([ADR-0071](adr/0071-cooked-textures.md)).

**Theme (interface).** A document of named values, colors, numbers, lists, that the rules of a
*style sheet* give as `"$name"`: another theme restyles an interface, its rules unchanged, as a
dark theme and a light one share one style sheet (`slime_ui::theme::Theme`,
[ADR-0141](adr/0141-style-sheets-and-themes.md)). Cooked from a `.theme` file.

**Threat model.** What the files that the engine reads from outside its program may hold, damaged,
edited or made by a buggy tool, and what the engine promises about them: no panic, hang, exhausted
memory or lost progress ([ADR-0096](adr/0096-the-files-the-engine-reads.md)).

**Tick (world).** A number that grows by one at each operation that may change a world
(`slime_world::Tick`). Each value of a component holds the tick at which it was added and the one
at which it last changed, and a reader asks what changed since the tick at which it last read
([ADR-0108](adr/0108-what-the-world-knows-of-its-changes.md),
[ADR-0119](adr/0119-the-worlds-ticks-blocks-and-log.md)).

**Timestamp query.** A GPU feature that writes the GPU's clock at a point of its command stream;
two of them, around a render pass, give its *GPU time* (`slime_gfx::GpuTimer`). Optional: the
device asks for it only when the adapter offers it (`Gpu::can_time`,
[ADR-0031](adr/0031-dev-overlay-with-egui.md)).

**Token (development channel).** The secret, 128 random bits, that whoever starts a game gives it in
its environment, never on its command line, and without which no connection to its *development
channel* is accepted (`slime_channel::Token`,
[ADR-0122](adr/0122-the-development-channel.md)).

**Tone mapping.** Mapping the *HDR* colors of a scene to the range a display shows, 0 to 1, with a
curve that compresses highlights instead of clipping them. The renderer's tone-mapping pass draws a
*fullscreen triangle* that applies to the exposed light Khronos PBR Neutral, the default of the
Khronos glTF Sample Viewer, which keeps hues and lowers dark and mid tones by up to 0.04, an offset
meant to balance the reflection of their surroundings that surfaces get from image-based lighting
([ADR-0034](adr/0034-hdr-rendering-and-tone-mapping.md)).

**Tooling (engine's).** Code that serves development, not what a game is made of: the dev
*overlay*, which a game keeps in its development builds and leaves out of its build for shipping,
as *Hunna Wood* does ([ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)).

**Tooltip.** A short text, under a title where it has one, that shows below an element once the
pointer has rested on it, or once the keys gave it the focus, in a *layer* that the pointer passes through; screen
readers read it as the element's description (`Element::tooltip`,
[ADR-0150](adr/0150-layers.md)).

**Transform in the world (global transform).** Where an entity is in the world: its `Transform`
composed with its ancestors', a translation in `f64`, a rotation and a uniform scale, without its
*stretch* (`slime_world::GlobalTransform`, [ADR-0083](adr/0083-transforms-and-the-hierarchy.md)).

**Transient field.** A field of a reflected type that saves leave out, `#[reflect(transient)]`:
what a game computes again, such as a value that changes at every frame. It has a default, its
own, its struct's or its type's, which a save that leaves it out gives it
([ADR-0087](adr/0087-the-worlds-documents.md)).

**Transient target.** A target of the *render graph* that it creates for a *view*, at the size of
the view's targets divided by a power of two, and whose content lives for one frame: the renderer's
high-dynamic-range and depth targets and the bloom's levels. It is kept from frame to frame, and
reused by the graph of the next *shape* that declares it at the same size
([ADR-0041](adr/0041-render-graph.md)).

**Transition (interface).** A change of an element's style shown over a time rather than at once:
its colors and its opacity going from what they showed to what the new style gives, a section
growing or shrinking to its height, as the style's `transition` says (`slime_ui::style::Transition`,
[ADR-0149](adr/0149-the-looks-foundations.md)).

**Type registry.** The reflected types that a program describes, by *stable name* and alias, with
what reads and sets their *values* (`slime_reflect::Registry`). Each *world* keeps one, which holds
the types of its components ([ADR-0085](adr/0085-reflection.md)).

**Typed property.** A custom property of a glTF node named `slime:` followed by a registered type's
*stable name*, whose value is a text that holds a component's value as a scene file writes it, the
TOML that `slime-serialize` reads, such as `{ height = 2.6 }`, or a boolean or a number: set in
Blender, it gives the node that component when a world resolves the scene, at the version of its
type that the node's `slime:$versions` gives, 1 if it gives none. Other custom properties stay
untyped `extras` ([ADR-0095](adr/0095-cooked-scene-files.md)).

**Uniform buffer.** A buffer of values that a whole draw reads, such as a camera's matrices,
declared `var<uniform>` in WGSL and read through a *bind group* (`slime_gfx::UniformBuffer`).

**Unlit material.** A *material* of the second *kind*, which shows its color, times its texture, as
it is, whatever lights the scene and whatever its exposure, and casts no shadows: an editor's icons
and handles, a stylized game's surfaces (`Renderer::create_unlit_material`,
[ADR-0129](adr/0129-drawing-for-the-editor.md)).

**Unload (scene).** Destroy the entities of a scene spawned under a key and forget it, keeping what
the run changed of it, but what *resets*, until the game spawns the same scene under the key again
and *settles* it (`slime_world::World::unload_scene`); spawning another scene under the key unloads
the one before ([ADR-0125](adr/0125-saves-per-scene.md)).

**Unsaved (entity).** An entity that holds the `Unsaved` component, or whose ancestor does: saves
leave it out, with its descendants, as they do what a game makes again at each run, such as an
effect ([ADR-0087](adr/0087-the-worlds-documents.md)). A level written back as a scene file writes
the component, and leaves out only what the run created that holds it, or is under what does
([ADR-0101](adr/0101-saves-and-identities-that-never-mislead.md)).

**Upgrade.** The function that brings a value of a reflected type from one version of the type to
the next, on the value as a *document* holds it, before the type reads it; a type past version 1
names one, `#[reflect(upgrade = ...)]`, and a reader applies them in turn
([ADR-0086](adr/0086-serialization.md)).

**Uploader.** The one owner of the copies to the GPU of what loads bring
(`slime_gfx::Uploader`): the threads that load write textures and vertices into *staging buffers*,
and the uploader's thread submits their copies after each frame, at most a budget of bytes per
frame while frames are drawn, so that a frame does not wait for all of a load's copies ([ADR-0107](adr/0107-the-execution-model.md),
[ADR-0118](adr/0118-the-task-pool-and-the-copies-to-the-gpu.md)).

**Value (reflection).** A reflected value as plain data, without its Rust type: booleans, integers,
floats of either width, text, structs, enums, options, lists, references to assets and entities
(`slime_reflect::Value`). What passes between reflection, files, saves and editors.

**View (render).** A camera's image drawn into a render target, or into a rectangle of one, which
the game holds from frame to frame (`slime_render::View`): its targets at the size it draws, its
buffers, its part of the render graph and its statistics. A frame draws one or more, each with its
own camera; what they share, the pipelines, the meshes, the spot lights' shadows and the sun's
cascades' atlas, the renderer makes once ([ADR-0111](adr/0111-the-frame-contract.md),
[ADR-0126](adr/0126-views-apart-from-the-renderer.md)). Each draws as its *view settings* say, and
makes its targets at a new size once that size has held still for a moment, its image stretched
until then ([ADR-0127](adr/0127-the-settings-of-a-view.md)).

**View settings.** How a *view* draws, which it keeps from frame to frame
(`slime_render::ViewSettings`): its *render scale*, its *anti-aliasing*, a *mode of debugging*, and
whether it draws the shadows, the sky and the bloom. Described by reflection, through which the dev
overlay and a tool driving a game read and change them
([ADR-0127](adr/0127-the-settings-of-a-view.md)).

**Viewport (editor).** The part of the editor's window that shows a *view* of the level being
edited, drawn into a target of its own, which the interface's *canvas* shows through a *surface
(interface)*, and drawn again only when what it shows changes; the user flies through it and picks
in it. The window shows one or two, each in a *pane (editor)*
([ADR-0155](adr/0155-the-editors-design.md), [ADR-0158](adr/0158-the-viewport.md),
[ADR-0160](adr/0160-two-viewports-and-blenders-way.md)).

**Visual target.** A reference image that a game sets for its look, on its target *hardware tier*
at its frame rate: the gap between the game's image and its targets is the measured problem by
which a rendering technique enters the engine ([ADR-0098](adr/0098-the-games-the-engine-aims-at.md)).

**WARP.** Microsoft's software driver for Direct3D 12, part of Windows. wgpu uses it on Windows
machines without a GPU, such as the CI runners.

**Widget.** An element of the interface that a person uses, a check box, a slider, a section that
folds, a bar of tabs, two panels and their handle, a list to choose from, a button that toggles, a
color well: an element kind of `slime-ui` that keeps its state in the tree, which the program's
value replaces only when it changes, with its own input, meaning and actions
([ADR-0148](adr/0148-widgets.md), [ADR-0150](adr/0150-layers.md),
[ADR-0151](adr/0151-rich-widgets.md), [ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)).

**World panel.** The panel of the dev *overlay* that lists a *world*'s *entities* by name and
edits the *components* of the one selected through *reflection*, returning each change as a
*command*, which a *presentation* sends to its *host* (`slime_devui::WorldPanel`,
[ADR-0081](adr/0081-stage-5a-the-world.md),
[ADR-0152](adr/0152-the-dev-overlay-on-the-engines-interface.md)); tooling, as the rest of the
overlay.

**World.** The *entities* of a game, their *components* and their hierarchy, kept by the engine
(`slime_world::World`); `slime-world-render` draws what it holds, and it saves what changed.
A game, a test or an editor holds as many worlds as it needs
([ADR-0079](adr/0079-the-engines-own-world.md)).
