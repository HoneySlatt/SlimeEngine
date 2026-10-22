# 0130. Picking

- Status: Accepted; amends [0115](0115-what-the-gpu-layer-gives-the-editor.md) (how picking by the
  GPU and by the CPU are built) and [0122](0122-the-development-channel.md) (version 6 of the
  protocol: a picking)
- Date: 2026-10-22

## Context

[ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md) decided that the engine says which object
is under the cursor two ways: by the GPU, an image of the instances' numbers at one sample per
pixel, read back a frame or two later without holding up a frame, given up without a panic on a
lost device, each number mapped by the bridge between the world and the renderer to the entity
drawn; and by the CPU, a ray against the instances' bounds, then the triangles of the meshes it
meets, kept in the CPU's memory for it, for what runs without a GPU. It asked the GPU layer for a
target of integers and for reading a target back without waiting. M6a.10 was cut in two
([ADR-0129](0129-drawing-for-the-editor.md)); M6a.10b, in [`docs/PLAN.md`](../PLAN.md), builds
picking, and names through the MCP server the entity under a pixel of a view.

Before it, the GPU layer read a target back by waiting for the GPU (`Gpu::read_render_target`), its
targets held colors alone, and an instance carried nothing that said what it was drawn for.

The owner took four decisions on 2026-10-22, one at a time:

- **On request**: a picking is asked for an area of a view, which the next frame draws alone, its
  answer a frame or two later; rather than at every frame, which would cost every frame a pass over
  what the view sees, whether anything is picked or not.
- **A number that the client gives**: each instance carries 64 bits chosen by whoever builds the
  frame, which the bridge makes its entity's handle, its index and its generation; the answer is the
  entity itself, without a table to keep until it comes, and an entity destroyed meanwhile is told
  by its generation; rather than the instance's place in the frame, which would keep the frame's
  table for each picking, and a renderer that does not know where its instances come from.
- **Triangles read when needed**: picking by the CPU reads a mesh from the cooked data when a ray
  first needs it, and keeps it while the world names it; rather than everything ever read, kept
  without a bound, or a copy that the asset cache keeps of every mesh, which doubles their memory
  and needs a GPU.
- **The MCP server picks with the GPU**: exactly what the agent sees in its captures, cut-out leaves
  included, a frame or more later; rather than the CPU, which ignores what masks cut out, or both,
  two answers to one question that may differ. A game without a window has no pixels to pick at.

## Decision

**The GPU layer.** `TextureFormat::Rg32Uint` holds two 32-bit integers per texel, which shaders
write as `vec2<u32>`: render targets only, of one sample, which no pipeline blends.
`Gpu::read_render_target_later` copies a target after the work submitted before, and returns a
`Readback` at once, whose `try_read` gives the texels once wgpu has seen the copy done, which it
checks when later work is submitted: it never waits, nor polls the device, which wgpu treats as
fatal on a lost Vulkan device (ADR-0022). On a lost device, it gives the loss.

**Picking by the GPU.** `Instance::pick` is the number that a client gives an instance, 0, the
default, being nothing; it travels in the padding of the instance's values. `View::pick(area)`
returns a `Pick`: the next frame that draws the view draws, after its passes, the number of each
instance that the view sees where it is the nearest, in the area alone, as its camera sees it, into
an `Rg32Uint` target of the area's size, with a depth test, at one sample, then starts its readback.
`Pick::take` gives `Picked`, the numbers of the area, once it has come back, or the loss of the
device, whether the picking was drawn or not. Each kind of material draws it with its entry points
`vs_pick` and `fs_pick`, through the view of the area, which the slot of the shadow passes' views
after theirs holds: a masked material's cut-out parts let it through to what is behind, a blended
one is picked where its alpha reaches one half, and lines are not picked. A view draws one picking
per frame, the oldest asked for first; an area is cut to the view's rectangle. The pass draws what
the view's scene pass draws, not what the area alone holds.

**The bridge's numbers.** `FrameBuilder` gives each instance its entity's handle,
`Entity::to_bits`, which `Entity::from_bits` gives back; the world says whether that entity still
lives.

**Picking by the CPU.** `Picker::cast` gives the entity of a world whose mesh a ray meets first,
`Hit`, with its distance, as the frame builder places its meshes, without a GPU. It reads each mesh
from the cooked data it was made with, whole, the first time a ray is cast in a world that names it,
which costs that cast the reading of every mesh named; it keeps the mesh's box, and its triangles
if a ray met the box, or reads it again when one first does, and keeps them while the world names
the mesh.
A ray meets both faces of a triangle, nothing hidden, and passes through nothing that a mask cuts
out. `Camera::ray` gives the ray through a point of a camera's image.

**For tools.** `Presentation::pick(view, (x, y), answer)` asks a presentation to pick at a pixel of
one of its views, which gives the entity to `PickAnswer::give` once it knows it; dropped without an
answer, it says that none came. Version 6 of the development channel's protocol asks it
(`Request::Pick`), at a pixel of a view or of the whole image, where the last view drawn there is
picked, and answers with the entity's persistent identity and name, or nothing (`Reply::Picked`);
the MCP server's `pick` tool gives it. The `world` example picks for tools in its two views, and
selects what a click picks, whose box both views draw.

## Alternatives considered

Those of the four decisions are above. Besides:

- **The picking pass culled to the area**: fewer instances drawn for a click, but another culling
  and another order of draws, where the view's own batches serve as they are; a measure decides it.
- **A readback that polls the device**: answers sooner, but wgpu panics when a poll finds a lost
  Vulkan device, which a submission only reports.
- **Lines picked**: an editor's handles are meshes, at Stage 6b; a line's number would have to
  travel with each segment.

## Consequences

- The editor picks at the pixel by the GPU, and anywhere without one by the CPU; an agent names
  what is under a pixel of a game's view through the MCP server.
- Every instance carries a number, which the instances' values held already, in their padding.
- Tools and games of version 5 of the channel refuse each other.
