# 0111. The frame contract

- Status: Accepted; amends [0027](0027-the-render-crate.md) (what the renderer keeps from one frame
  to the next), [0035](0035-physically-based-materials.md) (the physically based material is one
  kind of material), [0041](0041-render-graph.md) (views apart from the renderer, a part of the
  graph each) and [0084](0084-what-the-world-draws.md) (a frame that owns what it draws, and a cache
  that loads without the renderer); amended by [0115](0115-what-the-gpu-layer-gives-the-editor.md)
  (an unlit kind of material at Stage 6), [0120](0120-a-frame-that-owns-what-it-draws.md) (the shape
  of the owned frame and of the kinds of materials) and
  [0126](0126-views-apart-from-the-renderer.md) (how views are built)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decides, before Stage 6 writes the editor's viewport against it,
the *frame contract*: what the world gives the renderer at each frame, and how (G17, C2-04, C2-09,
C7-04). [ADR-0098](0098-the-games-the-engine-aims-at.md) adds that what a game hands the renderer
for an object is not limited to the fields of the physically based material, and that a frame may
hold several views, as screens in a cockpit or mirrors do. The audit of 0.4 measured the figures
below on the owner's RX 6900 XT at `v0.4.0`, unless a document of the repository is named; its
report and its tests are kept outside the repository, as the plan says.

- **The frame borrows, and loading borrows the renderer.** `FrameBuilder::build`, in
  `slime-world-render`, the bridge between the world and the renderer
  ([ADR-0084](0084-what-the-world-draws.md)), returns a `Frame<'a>` that borrows the asset cache
  and the builder, which holds the frame's lights and environment; `slime_render::Instance<'a>`
  borrows its mesh and its material, `Scene<'a>` its instances, lights and environment.
  `AssetCache::request` and `FrameBuilder::build` take the `Renderer`. Building and drawing a frame
  run on the main thread, one after the other: no thread can draw one frame while the next is
  built, as tests of the audit that the compiler refuses showed. What rendering costs is its draws,
  whose CPU P4 halved on the RX 6900 XT ([`measurements.md`](../measurements.md)), not too few
  threads; *Hunna Wood*'s zone takes 0.54 ms of render CPU at the median after P4 (the plan, P4's
  outcome). A render thread adds a frame of delay between a key and the image.
- **A renderer draws one output of one size.** Its rustdoc advises one renderer per size, and
  [ADR-0041](0041-render-graph.md) one per output size for its timers. Each holds its pipelines, its
  targets of four samples, its two shadow atlases of 64 MiB each, the sun's cascades and the spot
  lights' tiles, its buffer of instances, its graph and its timers; the scene's `Camera` is a
  perspective one, without settings of its own. Four renderers of 1280 × 720 with the sun hold
  about 508 MiB, about 260 of them in the cascade atlases; a view of 720p, some 54 to 62 MiB of
  targets. A target resized keeps its earlier image while the new one is made: up to 1 GiB more
  while the edge of a 4K panel is dragged. The editor has views of a scene, previews and
  thumbnails; ADR-0098's games, screens in a cockpit and mirrors.
- **One way to light a surface.** A material is glTF's metallic-roughness material
  ([ADR-0035](0035-physically-based-materials.md)), drawn by one scene shader. ADR-0098's
  toon-shaded, painted and hand-drawn games need lighting of their own, which Stage 9a builds, and
  Stage 9b's material graph generates shaders.
- **Each frame is built again from the whole world.** The bridge visits every entity that draws
  something at each frame: 74 µs for *Hunna Wood*'s zone, 429 instances (ADR-0084), 11.7 ms for
  100 000 entities on the RX 6900 XT after P4 (`world.frame.100k`,
  [`measurements.md`](../measurements.md)), whether anything moved or not. The renderer's README
  says that it keeps no scene between frames, and [ADR-0027](0027-the-render-crate.md) rejected a
  scene that the renderer keeps, a second model of the scene beside the world. The bridge already
  keeps one thing from frame to frame, the sky's environment, made again only when the sky or its
  sun changes (ADR-0084); P4 keeps the memory of the instances' values
  ([ADR-0106](0106-the-measured-quick-wins.md)).

The owner took four decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**A frame owns what it draws, and loading needs no renderer.** The frame that the bridge builds is a
value of its own, `Send` and `'static`: its instances hold shared handles to their meshes and
materials, counted references rather than borrows, and it holds its lights and its environment. The
asset cache keeps, from its creation, what it needs of the renderer, so that `AssetCache::request`
and `FrameBuilder::build` no longer take the `Renderer`. The phases of
[ADR-0107](0107-the-execution-model.md) become two calls: extraction builds the frame from the
world, read only, and rendering draws that frame without the world. The editor draws the world
only through `slime-world-render`, never with instances that it makes by hand. A render thread,
which draws one frame while the next is built, is decided at Stage 7, on a measurement of the CPU
that rendering still costs once draws are batched, weighed against the frame of delay that it
adds; this contract makes it a change inside the engine.

**Views apart from the renderer.** What every view shares is made once: the pipelines, the spot
lights' shadows, the environment, the BRDF table. Each view has its own camera, size, settings,
targets, timers and history, the images of earlier frames that Stage 9a's temporal anti-aliasing
reads; the render graph has a part for each view. The sun's cascade atlases, which each view fits
to its own camera, come from a pool, so that views drawn one after the other use the same memory.
A view sets:

- a perspective or an orthographic projection, and an offset of its projection, which draws a part
  of a larger image, for a split screen or a magnifier;
- the scale it renders at;
- its anti-aliasing;
- a mode of debugging, which shows normals or shadows;
- which passes it runs.

A frame may hold several views. A view's targets are made again once its size holds still, and the
earlier ones are freed before the new ones are made. Cameras placed in the world as components,
from which the bridge makes views, come when the editor or a game asks for them, on top of this.

**A material has a kind, which the renderer knows by registration.** An instance names its material
by a handle; a material is of a *kind*, a way of lighting a surface, that gives the renderer its
shaders, its parameters, described by reflection so that the editor shows and edits those of any
kind with no code of its own, and the passes it draws in. Stage 6 builds the indirection, with one
kind registered: the physically based material of today, which draws as it does. At Stage 9a a game
registers kinds of its own, toon or painted, and Stage 9a's method for many lights sets what a
kind gives the engine; Stage 9b's material graph makes kinds.

**Each frame is built again, until a measurement asks to update what changed.**
`FrameBuilder::build` keeps its shape. Once instancing has batched the draws, when building the
frame (`world.frame.100k`) costs more than drawing it (`render.instances.100k`), and before Stage
10a's large worlds at the latest, the bridge keeps what it draws from one frame to the next and
updates only what changed since it last built, by the world's ticks
([ADR-0108](0108-what-the-world-knows-of-its-changes.md)). What it keeps is a cache: the world stays
the one truth, and the cache can be made again from it at any time. The renderer and the bridge may
keep data from one frame to the next, as the bridge keeps the sky's environment; the renderer's
README says so, and its rule becomes that what it keeps is memory it reuses or a cache of what it
is given, never a second model of the scene.

**When it is built.** At Stage 6, before the editor's viewport: the frame that owns what it draws
and the cache that loads without the renderer, about one milestone; views apart from the renderer,
about one milestone, which ends the growth of memory while a target is resized; materials of a
kind, with the frame. The render thread is decided at Stage 7; updating what changed, on the
measurement above.

## Alternatives considered

The frame:

- **The frame as it is**, borrowed, on one thread: nothing to do, but the editor's asset browser,
  thumbnails and previews would multiply the calls that borrow the renderer, all to write again
  once a render thread is wanted.
- **A render thread now**: the simulation and drawing in parallel, but nothing measured asks for
  it, the zone drawing in under a millisecond, it adds a frame of delay, and a lost device and hot
  reload would be synchronized between two threads.

The views:

- **One renderer per view**, as today: no change, but memory and work multiplied by the views,
  nothing shared, and the editor's code of views written against that shape.
- **Cameras as components of the world now**: views declared as data, but it needs views apart from
  the renderer all the same, and touches the world before anything asks for it.

What a game hands for an object:

- **The physically based material alone until Stage 9a**: nothing more now, but the frame, the
  editor and its inspector of materials, written for one kind, would be taken apart at 9a.
- **Materials of a game's own now**: Stage 9a's method for many lights sets what a kind must give,
  and the shader language beyond WGSL is Stage 9b's: designed now, it would be designed again.

Building the frame:

- **Always built again from the whole world**: simple, nothing to fall out of step, but a cost that
  follows the world's size, 11.7 ms for 100 000 entities, which closes large worlds.
- **Updating what changed from Stage 6**: the gain at once for large worlds; but before instancing,
  building the frame is not yet the largest cost, and the risk that the bridge misses a change
  would be taken before a measurement asks for it.

## Consequences

- `Frame`, `Scene` and `Instance` lose their lifetimes; building an instance counts a reference
  to its mesh and its material, a cost measured when it is built.
- `AssetCache::request` and `FrameBuilder::build` take no `Renderer`; the editor and games load and
  build frames without one.
- A program that drew several outputs with several renderers draws them as views of one; each
  view's memory is its targets, and the sun's atlases are shared in turn.
- The renderer draws materials of any kind registered; the physically based one stays the only
  kind until Stage 9a.
- The renderer's README and [ADR-0027](0027-the-render-crate.md) allow memory reused and caches
  kept between frames, of what the renderer is given, never a second model of the scene.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6 builds the frame contract and views before the
  editor's viewport; Stage 7 decides a render thread; Stage 9a registers the kinds of materials of
  games.
