# 0117. Frustum culling and instancing

- Status: Accepted; amends [0026](0026-resource-binding-and-depth.md) (a storage buffer, and
  vertex and index buffers written by ranges), [0027](0027-the-render-crate.md) (instances drawn
  together, their values in a storage buffer), [0106](0106-the-measured-quick-wins.md) (the shadow
  passes group what they draw by mesh, the material only for masked materials) and
  [0115](0115-what-the-gpu-layer-gives-the-editor.md) (each buffer's uses are those of its type);
  amended by [0128](0128-asset-requests.md) (an asset cache's meshes in pages of its own)
- Date: 2026-10-20

## Context

Stage 6a begins with frustum culling and instancing
([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[ADR-0116](0116-stage-6-in-two-halves.md), [`docs/PLAN.md`](../PLAN.md), M6a.1). Until then, the
renderer drew every instance of a scene in the scene pass, whether the camera saw it or not, each
with a draw call of its own, its values in a slot of 256 bytes of a uniform buffer bound at a
dynamic offset, half of the slot padding
([ADR-0027](0027-the-render-crate.md), [ADR-0106](0106-the-measured-quick-wins.md)); the shadow
passes left out what lay outside each cascade's square and each spot light's cone, and drew the
rest the same way. A frame's CPU time grew with every instance, about 1 to 2 µs each on the
owner's RX 6900 XT: 10 000 cubes took 9 to 20 ms of CPU, 100 000 from 73 to 131 ms
([`measurements.md`](../measurements.md)). Beyond 1 048 576 instances, the instance buffer passed
the 256 MiB that the device allows, and `render` panicked inside wgpu (G29).

[ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md) decided what the GPU layer gains for this:
a storage buffer of the instances' values, buffers that declare several uses and are written by
ranges, meshes that share buffers, an indexed draw from a base vertex, ranges of instances per
draw. ADR-0097 left GPU-driven rendering, culling and drawing decided by the GPU, to later, and
[ADR-0111](0111-the-frame-contract.md) keeps a frame built again at each frame: the renderer is
given the scene's instances anew each frame, and keeps nothing of them but memory it reuses.

## Decision

**Culling on the CPU, every instance at each frame.** The scene pass draws an instance only when its
box, the mesh's bounds placed by its transform, may be seen: no plane of the camera's view, its four
sides and its near plane, leaves the box wholly outside, a test of a box against planes that keeps
some boxes near the corners of the view, never one in it. The shadow passes keep their own tests,
the cascades' squares and the spot lights' cones: an instance that the camera does not see still
casts its shadow into the view.

**One draw call for the instances of one pipeline, material and mesh.** Each pass sorts the
instances it draws as ADR-0106 sorts them, by pipeline, then material, then mesh, each as it first
appears among all the scene's instances, then in the scene's order, and draws each run of one
pipeline, material and mesh with one draw call, its instances in that order. The shadow passes'
pipeline for materials that are not masked reads no material: it groups their instances by mesh
alone.

**The instances' values in a storage buffer, and a list per draw.** The values of each instance
that some pass draws, 128 bytes, its transform, its normal matrix and its handedness, are written
once per frame, in the scene's order, into a storage buffer; a second storage buffer holds, for each
draw of each pass, where the values of the instances it draws are, 4 bytes each. The vertex shader
reads the values of the instance of number n at `instances[drawn[n]]`. Both buffers grow and shrink
as ADR-0106's instance buffer did.

**Meshes in shared pages.** A renderer's meshes share pages: a vertex buffer and an index buffer,
each mesh in a range of each, which a draw reaches through its first index and its base vertex,
so that a page is bound once for all its meshes. The first page holds 3 MiB of vertices and 1 MiB of
indices, each new page twice the largest, up to 48 MiB and 16 MiB, and a mesh larger than that has
a page of its own; a mesh takes the first free ranges that hold it, a dropped mesh gives its ranges
back, and a page is freed with its last mesh. Pages never move or grow: a mesh's buffers stay the
same for its life. The renderer's loads place their meshes in its pages, from the threads of the
loads.

**The GPU layer's pieces, typed.** `slime-gfx` gains `StorageBuffer`, read by shaders at a
`BindingType::StorageBuffer` binding, and writes vertex, index and storage buffers by ranges, from
buffers created zeroed; `RenderPass::draw_indexed` takes a base vertex and a range of instances.
ADR-0115's buffers of several declared uses take the shape of
[ADR-0026](0026-resource-binding-and-depth.md)'s typed buffers: each type declares its uses, its
own and copies into it, so that a pipeline still cannot bind vertices as uniform values. A generic
buffer whose uses a caller declares waits for a buffer that needs two kinds of binding, the storage
and indirect draws of Stage 9a. The owner chose it on 2026-10-20, rather than the generic buffer
that "Alternatives considered" lists.

**At most what a storage buffer holds (G29).** A frame draws at most as many instances as the
device's largest storage buffer holds the values of, 1 048 576 with the default limits: the
instances that some pass draws beyond it, the last in the scene's order, are left out of every
pass, with a warning the first time a renderer meets such a frame, as lights beyond the most drawn
are ([ADR-0042](0042-lights-beyond-the-most-drawn-and-checked-scene-values.md)). Every pass draws an
instance at most once, so that the list is never the larger buffer.

**What a frame reports.** `RenderStats::draw_calls` counts the scene pass's draws, one per group;
`RenderStats::instances`, new, the instances it drew; `triangles` those of the instances drawn. The
shadow passes' draw calls count groups too.

**Measured at the hardware tiers' resolutions.** The render benchmarks draw at 1280 × 800, 1920 ×
1080 and 2560 × 1440, the screens of [ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)'s
tiers, and give the sum of the GPU times of a frame's passes, which a tier's budget of GPU time is
compared with; [`measurements.md`](../measurements.md) holds the figures before and after.

## Alternatives considered

- **Culling and drawing decided by the GPU**, compute shaders filling the draws' arguments, with
  indirect draws: the CPU's cost per instance gone, but neither compute nor indirect draws is in
  the GPU layer, which gains them with Stage 9a's first users (ADR-0115), and without a render scene
  kept from one frame to the next, the instances' values would still be written by the CPU at each
  frame (ADR-0097).
- **Spheres rather than boxes**: one dot product per plane rather than four, but a long, thin mesh,
  a log or a wall, is kept far beyond the view's edges. The box is what the shadows test already.
- **The values written once per pass, in the order of its draws, without a list**: no second
  buffer, but an instance's 128 bytes written up to twenty-one times, once for the scene and for
  each cascade and spot light that draws it.
- **The list given as a vertex buffer of one entry per instance**: the GPU layer's vertex layouts
  would gain a rate per instance and a second vertex buffer per draw, where a storage buffer is
  what the values need anyway.
- **One generic buffer, its uses declared by flags**: closer to the words of ADR-0115, but a buffer
  bound against its use would panic inside wgpu at a draw rather than fail to compile, which
  ADR-0026 chose against, for no buffer that needs it yet.
- **Each mesh in buffers of its own, as before**: no pages to manage, but every change of mesh binds
  two buffers, and the draws of many meshes in one call that Stage 9a's indirect draws make need
  meshes in shared buffers; M6a.2's copies to the GPU within a budget write into them.
- **One buffer for every mesh, grown by copying it into a larger one**: one buffer to bind, but
  every growth copies all the meshes, and changes the buffer that each mesh is drawn from.
- **The materials and meshes ranked among the instances drawn only**: cheaper when few are seen,
  but the order of two surfaces at the same depth would change as others come into view, and which
  one shows with it.
- **The shadow passes grouping by material too, as before**: a draw for each material of one mesh,
  for a pipeline that reads none.

## Consequences

- A frame's GPU work and draw calls follow what each pass sees; on the RX 6900 XT, the CPU time of
  10 000 cubes falls four to seven times, that of 100 000 about four times, and that of 10 000 out
  of view ten times or more ([`measurements.md`](../measurements.md)).
- Every instance of the scene still costs CPU time at each frame, to place its box, test it against
  the view and the shadows, rank its material and its mesh, and pack and send the values of those
  drawn: what a render scene kept from one frame to the next and GPU-driven rendering remove, at
  Stage 9a. [`docs/PLAN.md`](../PLAN.md)'s known debt says it, with the shadows' tests, each
  instance against every cascade and spot light on one thread, which ADR-0097 left to Stage 9a.
- A page keeps the ranges of dropped meshes until it fills them again or empties: dropping a mesh
  frees no GPU memory until then. [ADR-0113](0113-asset-requests-residency-and-priorities.md)'s
  budgets count the sizes of what the asset cache creates and drop what it may, least recently
  used first; how they count meshes in pages is left to M6a.9, which builds them. Capture tools
  name a page's buffers, not its meshes.
- The renderer's frame no longer panics past a million instances; a frame that draws more leaves
  the last out.
- Tests that counted a draw per instance count groups; `RenderStats` has one more field.
- ADR-0111's condition for building the frame from what changed, a frame that costs more to build
  than to draw, is measured again with these figures.
