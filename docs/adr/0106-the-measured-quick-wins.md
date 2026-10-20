# 0106. The measured quick wins

- Status: Accepted; amends [0042](0042-lights-beyond-the-most-drawn-and-checked-scene-values.md)
  and [0084](0084-what-the-world-draws.md) (which lights a world gives the renderer first),
  [0071](0071-cooked-textures.md) (textures whose sides are not multiples of 4) and
  [0083](0083-transforms-and-the-hierarchy.md) (how an entity's children are read)
- Date: 2026-10-20

## Context

The audit of 0.4 ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md))
measured quick wins, each a change of a few dozen lines that leaves the architecture as it is
([`docs/PLAN.md`](../PLAN.md), P4): defects of complexity, a child that leaves its parent walking
all its siblings (G27) and a scene file's override looking its placement up among all its entries
(G26); work done again for nothing, the sky made again for a sun that did not move (G13), the
whole manifest copied at each batch of loading (G25), every component storage looked up in a map
hashed again several times per entity of a frame (B2-F06); textures left uncompressed (B1-F06);
lamps near the player left dark beyond the 16 lights drawn (G12); and, in the renderer, draws that
bind what is bound already in the order they come (B3-F02), and values of every slot of the
instances' buffer written and sent at each frame (B3-F05).

The benchmarks of [ADR-0104](0104-the-build-ci-and-measurements.md) measure each change before and
after on lavapipe, on a machine of 4 logical CPUs; the gains of the GPU's own work are measured on
a GPU, the owner's RX 6900 XT, which the plan leaves to the owner.

## Decision

**A child leaves its parent in constant time.** Each node of the world links to its first and last
children and to the siblings before and after it, and counts its children: a child that gets
another parent, or none, or is destroyed, leaves without a walk among its siblings, and the others
keep their order, which saves and scene files written back rely on. `World::children` gives them as
an iterator, `Children`, that reads from either end and knows how many are left, rather than a
slice: the shape that the editor's hierarchy will read, where moving a child or putting it before
another changes a few links. The walks of a subtree, when transforms change or are computed, read
each node once, its first child and its next sibling.

**A frame looks each storage up once.** The map from a component type to its storage keeps the hash
that a `TypeId` already is; `World::is_hidden` looks the storage of `Hidden` up once rather than at
each ancestor; a transform in the world that is current needs no lookup; an entity is drawn where
it is without a walk up its ancestors when nothing moved during the last step.

**A scene file's entries are indexed by their IDs**, once, where each override looked its
placement up among all of them.

**Interpolating a transform keeps exactly what does not change.** `GlobalTransform::lerp` gives back
a translation or a rotation that is the same at both ends, which interpolating them in floating
point does not always do: an entity that did not move is drawn exactly where it is, and the frame
builder, which compares the sun exactly, keeps the sky's environment.

**Beyond the lights that the renderer draws, the nearest first.** When a frame of a world holds
more lights than the renderer draws, `MAX_LIGHTS`, `Frame::scene`, which knows the camera, orders
them: the sky's sun stays first, then come the other directional lights, which reach everywhere,
then the point and spot lights nearest the camera's eye, each group in the world's order for
lights as near. The renderer still draws the first `MAX_LIGHTS` lights of a scene in the order it
is given, as ADR-0042 decided for its own API; a game that draws through the world no longer
orders its lights itself, which amends ADR-0042's "the game orders its lights" and ADR-0084.

**Cooked data shares its manifest.** `CookedAssets` holds its manifest in an `Arc`, which its clones
share: each batch of loading takes a clone without copying the manifest.

**Textures are stretched to whole blocks.** An image whose sides are at least 4 texels but not
multiples of 4 is stretched to the next multiples before it is cooked, each texel the average of
those it covers, in the way its role asks, then compressed in BC7 or BC5 as any other: its
texture coordinates still span it whole. An image smaller than a block, along either side, keeps
its texels, as 8-bit texels. `TEXTURE_COOKER_VERSION` is 2, so that every texture is cooked again.

**Each pass draws by pipeline, material and mesh, and binds only what changed.** The scene pass and
the shadow passes draw their instances by pipeline, then by material, then by mesh, each material
and mesh in the order it first appears among the scene's instances, then in the scene's order; a
draw binds its material and its mesh only when they differ from the draw before. The order does
not depend on where the materials and meshes lie in memory, so that two surfaces at the same depth
are drawn the same way at every frame and every run.

**Only the instances' slots used are sent.** The renderer keeps the vector of the instances' values
from one frame to the next, and sends only the slots of the frame's instances, through
`Gpu::write_uniform_buffer_start`, which writes the start of a uniform buffer.

**Measured, and left as they are.** The plan asked to measure five more possible gains:

- *The order of the bind groups*: on lavapipe, what a draw binds costs nothing measurable, as the
  draws' order shows above; a GPU's driver may differ, which nothing measured.
- *An opaque pipeline without `discard`*: without the `discard` of masked materials, the scene pass
  of `render.instances.10k` took 159 to 161 ms on lavapipe, where it took 128 to 140 ms with it; a
  GPU that tests depths before shading may gain instead, which nothing measured.
- *The number of passes per submission*: [ADR-0041](0041-render-graph.md) measured it,
  about 35 µs per submission on the RX 6900 XT against stalls of up to 8 ms in the kernel with
  submissions of a whole frame.
- *Foliage left out of the farther cascades*: in *Hunna Wood*'s zone by day, seen from where the
  hero starts, the farthest cascade draws 409 of the 653 shadows' draws, but no caster covers less
  than 4 of its texels: leaving out the smallest casters would leave out none.
- *The CPU mipmaps of `Gpu::create_texture`*: about 70 ms for 2048 × 2048 texels on the
  development machine, as its rustdoc says, paid only by textures made in code, of which the
  engine makes one, of a texel; cooked textures bring their levels.

## Alternatives considered

- **The children in a vector**, as before, an entity's removal walking its siblings: quadratic in
  the number of children that leave. **The children in a vector with holes**, each child knowing
  its place, the holes skipped by an iterator and closed up when they are half: constant time too,
  but two representations to keep consistent, where links are one.
- **The sun compared within a tolerance** in the frame builder: it would hide a sun that turns
  slowly, and the interpolated transforms that every other comparison reads would stay inexact.
- **The camera given to `FrameBuilder::build`** to order the lights: an argument that a frame
  without a scene does not need, where `Frame::scene` takes the camera already. **The brightest
  lights first**, or weighed by their distance: a light's reach depends on its range and the
  exposure, which the bridge does not know; the nearest is what the player sees, and is
  predictable.
- **An odd texture padded with its last row and column** to whole blocks: its texture coordinates
  would shift by up to three texels' share. **Shrunk to the previous multiples**: up to three rows
  and columns of detail lost. **Left uncompressed with a warning**: four times the memory, which
  the coat of the zone's hero showed.
- **The draws ordered by the addresses of their materials and meshes**: cheaper, but the order, and
  with it two surfaces at the same depth, would change from one run to the next. **Front to back**:
  a sort by depth at each frame, for overdraw that nothing measured.
- **The instances' values in a storage buffer** indexed by the instance, without the padding of a
  uniform slot: a change of the shaders and of the bindings, which instancing at Stage 6 decides
  (ADR-0097).

## Consequences

- `World::children` is an iterator: code that indexed the children or kept them as a slice collects
  them.
- `Frame::scene` takes the frame mutably, and `Frame::lights` is a mutable slice.
- Every texture is cooked again once; an image of 2 048 × 2 043 texels is a texture of 2 048 × 2 044
  texels, a quarter of its memory, and the zone's cooked bytes went from 259.5 MB to 209.4 MB.
- Ordering the draws of 10 000 instances takes about 0.8 ms on lavapipe's machine, where it saves
  nothing measurable: whether it saves more on a GPU than it costs is the owner's to measure, as
  the audit measured it there.
