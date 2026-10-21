# 0084. What the world draws

- Status: Accepted; amended by [0106](0106-the-measured-quick-wins.md) (the lights nearest the
  camera first, beyond those the renderer draws), [0111](0111-the-frame-contract.md) (a frame that
  owns what it draws, and a cache that loads without the renderer) and
  [0113](0113-asset-requests-residency-and-priorities.md) (assets requested through handles, by the
  bridge, the editor and games, and dropped when nothing holds them),
  [0120](0120-a-frame-that-owns-what-it-draws.md) (no `AssetCache::clear`: a cache is made again
  with the renderer that replaces a lost one) and [0126](0126-views-apart-from-the-renderer.md) (a
  frame's scene given a point rather than a camera)
- Date: 2026-10-16

## Context

[ADR-0079](0079-the-engines-own-world.md) decides that the world keeps what is drawn and holds no
GPU object: components name meshes, materials and textures by their asset IDs; the GPU objects live
in a cache outside the world, which loads them in the background, shares those that several scenes
use, makes them again after a lost device and loads them again when their cooked data changes; at
each frame a bridge builds the renderer's instances and lights from the world, and `slime-render`
does not depend on the world. It also decides that positions are `f64` in the renderer's API, that
the color type moves below `slime-gfx`, and that `slime-app` owns the game's world and calls it
around each fixed step, what moved during a step being drawn between its last two transforms.
[ADR-0081](0081-stage-5a-the-world.md) asks that the sky's sun, which a game describes twice, be
described once when the world draws. They leave how to M5a.3.

Where the engine stands:

- The renderer loads a whole cooked scene at once, on threads of its own, and gives it whole or not
  at all (`LoadedScene`, [ADR-0073](0073-loading-scenes-in-the-background.md)); a reload keeps the
  meshes and textures whose cooked file did not change
  ([ADR-0076](0076-hot-reload-of-cooked-data.md)). A scene's instances borrow its GPU objects.
- *Hunna Wood* rebuilds about 430 instances at every frame from its loaded scenes, keeps its hero's
  previous position to draw it between fixed steps, makes its whole view again after a lost device,
  and gives the same light to the sky and to the lights.
- The world keeps where each entity is, always current; `World::update_transforms` does not report
  what it computed again, and `GlobalTransform` holds no stretch
  ([ADR-0083](0083-transforms-and-the-hierarchy.md)). A cooked scene's nodes become entities with
  `Meshes`, a `Light` that wraps the cooked light, and `Extras`. Each cooked mesh names the material
  it is drawn with.
- `LinearColor` lives in `slime-gfx`, which the world cannot depend on.

## Decision

The owner took these decisions on 2026-10-16, one by one, when M5a.3 started.

**The cache lives in `slime-render`.** `AssetCache` holds meshes, the materials they are drawn with
and the textures those read, by asset ID, and knows nothing of a world. It loads with the scene
loader's code: a load of a batch of meshes, each once, with their materials and textures, on the
renderer's threads, keeping what it holds already whose cooked file did not change. A scene load is
the load of its meshes' batch, then the placing of its nodes: one loader, not two.

**It loads what is drawn, by itself.** At each frame the bridge asks the cache for the meshes of
what it draws; those the cache neither holds nor loads start loading, as one batch. A mesh is drawn
once its batch is: a batch arrives whole. `AssetCache::loading` says how many meshes are on their
way, and a game that shows a scene whole waits for none. A batch that fails is reported, and its
meshes are not asked for again until the cooked data changes. The cache keeps what it loaded: no
asset is dropped when nothing draws it any more. Loading ahead of what is drawn, and dropping what
is no longer drawn, come with the streaming of Stage 10 or with a loading screen, whichever needs
them first.

**Hot reload and lost devices are the cache's.** `AssetCache::reload` takes new cooked data and
loads again, in the background, every mesh it holds, keeping each mesh and texture whose cooked file
the new manifest still names, and making every material again, as ADR-0076 decided for scenes; it
gives what it held until the reload is done, then takes the result at once. `AssetCache::clear`
drops every GPU object and cancels the loads: after a lost device, what the world names loads again
by itself. The world is untouched by either.

**Positions are `f64` in the renderer's API.** `Camera::eye` and `Camera::target`, the positions of
point and spot lights and a loaded scene's bounds are `DVec3`s; `Instance::transform` is a
`DAffine3`, the matrix that the world's transforms give; `Camera::view` and
`Camera::view_projection` are `DMat4`s. The renderer subtracts the eye in `f64` before positions
reach the GPU as `f32`s, as [ADR-0048](0048-rendering-relative-to-the-camera.md) decided.

**The color type has a crate of its own.** `slime-color` holds `LinearColor` and the sRGB transfer
function, and depends on nothing; `slime-gfx`, `slime-render`, `slime-cook` and `slime-world` use
it. `slime-gfx` keeps the conversion to wgpu's clear color to itself.

**The bridge is `slime-world-render`.** At each frame, `FrameBuilder::build` takes the batches the
cache finished, then builds, from the world and the cache, the instances to draw, the lights, and
the sky's environment, which it keeps; it skips the entities whose meshes are not yet loaded, and
asks the cache for them. Its `Frame` says how many meshes are still on their way (`loading`) and why
the batches that failed did; `Frame::scene(camera)` is the renderer's `Scene`. The camera, the
exposure and the bloom stay the game's.

**The sun is given once.** A directional light of the world has a field that says it shows in the
sky, with its disc and its glow. The bridge gives that light to the sky as its sun and to the
lights, first among them, so that its shadows are the ones the renderer draws when it casts any; if
several have it, the first one does, and the log says so. The sky's environment is made again when
the sun or the sky changes, which costs about 23 ms in a release build; a sun whose shadows alone
change keeps it. The rest of the sky, its colors, is given by the game to the bridge.

**What the world draws.** Components of `slime-world`, without GPU objects:

- `Meshes`: the cooked meshes an entity places, by asset ID, each drawn with the material that its
  cooked mesh names, and whether they cast shadows. Another material for one entity comes when a
  game needs it.
- `Hidden`: an entity that holds it is not drawn, nor are its descendants, nor their lights.
- `Light`: the world's own light, with a `LinearColor`, its strength, its range, its cones, whether
  it casts shadows, and, for a directional light, whether it shows in the sky; at its entity's
  origin, along its entity's −Z axis.

**What moved is drawn between fixed steps.** The world keeps, for each entity whose transform in
the world changed during the last fixed step, its transform before that step, and gives the
transform between the two at a fraction of the step (`World::interpolated_transform`): positions in
`f64`, rotations along the shortest arc, scales in between. A frame without a step keeps the last
step's two; a frame with several takes the last one's. A change made outside a step, an entity
placed during one, and `World::teleport`, make an entity, with its descendants for a teleport, be
drawn where it is now; but one whose ancestor moved during the step is carried by the nearest such
ancestor, where it is now relative to it, so that what a moving entity holds stays on it. An entity
given another parent during a step is drawn from where it was to where it goes. A stretch is not
interpolated: it jumps.

The owner decided, on 2026-10-16 after the milestone's review, that the sun comes first among the
lights and that what a moving entity holds stays on it: the shadows could come from another
directional light than the sun, and a child changed outside a step, or placed during one, came
loose from a parent that moved.

**`slime-app` owns the world.** `Context::world` and `Context::world_mut` give it. `slime-app` tells
the world before and after each `fixed_update`; a change made outside a step is drawn at once. It
computes the world's transforms again after `update`, so that `render` reads them at the cost of a
lookup.

**An example, `world`**, draws the examples' cooked scene from a world, its glowing ring floating at
the fixed step with its light, headless too, and goes on after a lost device.

**Measured** in a release build on the owner's machine (Ryzen 7 5800X, RX 6900 XT): *Hunna Wood*'s
zone spawned in a world, its 430 meshes loaded by the cache in one batch in 170 ms, a frame of its
429 instances and 9 lights is built in 74 µs on the CPU (`hunnawood`'s ignored test
`what_building_a_frame_of_the_zone_s_world_costs`).

## Alternatives considered

- **The cache in the bridge crate**: `slime-render` would make public the pieces of its loader, its
  pool, the making of a mesh or a material from cooked data, and loading would be split across two
  crates.
- **Loads that the game asks for**: a scene arrives whole, and a game can load ahead of time; but an
  entity that no one asked the assets of is never drawn, and every new case (an object created
  while the game runs, a prefab of Stage 5b) must remember to ask. **Both**: two ways of loading,
  one of them without a user yet.
- **A batch per mesh**: a mesh that fails would not take its batch with it, but a load has a thread
  of its own, and a zone would start hundreds at once.
- **Two loaders**, the scene loader and the cache's: the same work written twice.
- **An `f64` translation beside an `f32` matrix** in an instance, which holds exactly the precision
  needed, but is a pair of our own to explain and to split from every cooked matrix; **a `DMat4`**,
  whose last row an affine transform does not need.
- **The color type in `slime-assets`**: the GPU layer would depend on the formats of cooked data.
  **A crate of shared basics** (`slime-math`, `slime-core`): it would hold the color type alone.
- **`slime-draw`**, which the renderer could be taken for; **`slime-view`**, which the game's `View`
  and the camera's view matrix already name; **`slime-scene`**, which cooked scenes and Stage 5b's
  scene files already name.
- **A marker component for the sun**: the dev overlay's panel, which adds and removes no component
  ([ADR-0081](0081-stage-5a-the-world.md)), could not turn it on or off. **The first directional
  light as the sun**: *Hunna Wood*'s overcast day, lit by a sun that shows no disc, would need a
  flag to say the opposite, and the order of entities would decide.

## Consequences

- Every game and example that places a camera, a light or an instance changes: `Vec3` positions
  become `DVec3`s, `Mat4` instance transforms `DAffine3`s.
- `LoadedScene` stays: the `scene` example draws with the renderer's own API.
- A batch that fails draws none of its meshes, as a scene that fails draws nothing.
- A reload makes every material again: values a game set on one are those of the cooked data
  afterwards, and a game that changes them sets them at each frame, as before.
- The cache's memory only grows until it is cleared.
- `slime-app` depends on `slime-world`, and headless runs hold a world.
- *Hunna Wood* moves onto the world in M5a.4, which removes its previous position, its instances
  rebuilt by hand and its nodes left out by index.
