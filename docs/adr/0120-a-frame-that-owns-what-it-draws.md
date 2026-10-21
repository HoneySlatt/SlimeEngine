# 0120. A frame that owns what it draws

- Status: Accepted; amends [0084](0084-what-the-world-draws.md) (no `AssetCache::clear`: a cache
  is made again with the renderer that replaces a lost one), [0111](0111-the-frame-contract.md)
  (the shape of the owned frame and of the kinds of materials),
  [0113](0113-asset-requests-residency-and-priorities.md) (after a lost device, the cache is made
  again rather than cleared) and [0114](0114-hardware-tiers-and-the-gpus-limits.md) (the floor's
  figures); amended by [0121](0121-a-game-as-a-simulation-and-a-presentation.md) (the frame's
  lights name their entities)
- Date: 2026-10-21

## Context

[ADR-0111](0111-the-frame-contract.md) decided the frame contract: a frame that owns what it
draws, `Send` and `'static`, an asset cache that keeps what it needs of the renderer, so that
loading takes no renderer, and materials of a kind that the renderer knows by registration, the
physically based kind alone until Stage 9a. [ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)
decided that the device asks for every limit of its adapter, above a floor checked when it opens,
its figures set when it is built. [`docs/PLAN.md`](../PLAN.md) gathers both in M6a.4, which builds
them. The owner chose on 2026-10-21 that a kind is a Rust trait: the kind gives its shaders, its
passes and its parameters described by reflection, and converts its parameters into what its
shader reads.

The figures below were measured on the owner's machine, 16 logical CPUs and an RX 6900 XT, in
release builds, while other programs ran; the sections "M6a.4, before" and "M6a.4" of
[`measurements.md`](../measurements.md) hold those of a quiet machine.

## Decision

**The floor is WebGPU's default limits.** `Gpu::new` opens the device with every limit that its
adapter offers, and refuses, with `GfxError::BelowFloor`, which names the limit, an adapter below
wgpu's `Limits::default`: the engine draws everything it draws within them; the RX 6900 XT and
lavapipe offer more, and WARP opened a device with them in CI. Nothing the engine draws asks for
more yet; each technique that does raises the floor, or enters a tier, as ADR-0114 says. What the
adapter offers above the floor is used where the engine sizes itself by the device: a frame draws as
many instances as a storage
binding holds, 16 777 215 on the RX 6900 XT, 1 048 576 on lavapipe. Textures written at once and
images read back still go through buffers of at most 256 MiB, the floor's largest, so that they
cost as much memory on every GPU. CI's Windows job prints WARP's limits.

**A frame owns clones.** `Instance` holds its `Mesh` and its `Material`, which are handles: a mesh
is a counted reference to its ranges in its page, and a material one counted reference to what its
clones share, its group, its values and its kind. `Scene` holds its instances, lights and
environment in vectors and an option, and `slime_world_render::Frame` the same, which
`Frame::scene` hands over; neither has a lifetime. `Scene::new` takes the instances, a slice of
them cloned. Two meshes, or two materials, are equal when one is a clone of the other.

**The cache keeps a loader.** `AssetCache::new(&renderer, assets)` keeps what the loads of a
renderer take: the parts of its materials, the pages of its meshes and its uploader, which the
renderer shares with it. `AssetCache::request`, `AssetCache::reload` and `FrameBuilder::build` take
no renderer, and `Environment::new` makes an environment without one. A cache loads into the pages
of the renderer it was made with, for its device: after a lost device, a game makes a new cache
with the renderer that replaces the lost one, and `AssetCache::clear` is gone.

**A kind is a trait of the renderer's own.** `MaterialKind` gives its name, the shader file of its
passes, the bindings of its materials' group, whether its materials cast shadows, and registers
the reflected type of its parameters; and it converts a material's parameters, a reflected value,
into the bytes that its shader reads, or refuses them. The renderer registers its kinds when it is
made, and makes for each the pipelines of the scene and shadow passes from its shader; a material
holds its kind, its index among the renderer's, and its parameters as a reflected value. The draws
are sorted, and their pipelines set, by kind then by pipeline, and a material stays bound only for
the pipelines of its kind, whose layout reads its group. The physically based kind's parameters are
`MaterialValues`, now reflected, its ranges and its check those that creating a material applies.
`Material::kind`, `Material::parameters`, `Material::set_parameters` and
`Renderer::describe_materials` give an editor what it needs to show and change the parameters of
any kind. The trait stays inside `slime-render`: Stage 9a sets what a kind gives the engine, with
its method for many lights, and makes registration public for games.

## Alternatives considered

- **A floor above WebGPU's defaults**, the larger buffers and storage bindings of today's desktop
  GPUs: nothing the engine draws needs them, and WARP's limits were not read yet.
- **Staging buffers as large as the device allows**: one copy for any texture, but 1.3 GiB of
  memory at once for a texture of 16 384 texels a side on the RX 6900 XT.
- **A material as the struct of its parts, cloned field by field**: no indirection, but five
  counted references per clone; built so, a frame of 100 000 instances took 24 to 27 ms of
  render CPU where it took 19 to 22 ms, and `world.frame.100k` 15.7 to 16.0 ms where it took 11.0
  to 12.2 ms. One counted reference per material brought both back within the noise.
- **A scene that borrows its instances, a frame that owns them**: the renderer's API unchanged, but
  two shapes of the same list, and a scene that cannot cross a thread.
- **`AssetCache::clear` that takes the new renderer**: the cache kept, but one more way to reach the
  renderer's parts, and a cache whose loads, made before, would still write into the lost pages.
- **The trait public now**: games could register kinds, but the contract between a kind and the
  engine, which groups it reads and what a light gives it, is Stage 9a's to set.
- **The physically based values stored typed in each material, converted by the material**: no
  conversion at each read of `Material::values`, but the kind would not convert its parameters, as
  the owner chose, and a second kind would change how materials store them.

## Consequences

- A frame built on one thread is drawn on another; a cache loads without a renderer; the editor
  builds its previews and thumbnails without borrowing one.
- Building a frame counts two references per instance, and dropping it gives them back: 1.3 to
  2.7 ms more for 100 000 instances on `world.frame.100k`, 11 to 23 %; drawing it costs the
  same ([`measurements.md`](../measurements.md), "M6a.4"). ADR-0111's condition for building the
  frame from what changed, a frame that costs more to build than to draw, still does not hold:
  13.3 to 14.4 ms against 19.8 to 21.8 ms.
- A program below the floor stops at start with the limit it lacks named; above it, the renderer
  draws as many instances as the GPU's storage bindings hold.
- A program that kept its cache across a lost device makes a new one.
- The unlit kind of [ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md), at M6a.10, is the
  trait's second implementation. Reading a physically based material's values converts its
  reflected parameters, at each `Material::values`.
- The request handles of ADR-0113, at M6a.9, are given by a cache that a lost device makes a game
  make again: how what they hold carries over to the new cache is that milestone's to build.
