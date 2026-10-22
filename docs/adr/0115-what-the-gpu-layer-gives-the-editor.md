# 0115. What the GPU layer gives the editor

- Status: Accepted; amends [0026](0026-resource-binding-and-depth.md) (buffers of several declared
  uses, written by ranges), [0041](0041-render-graph.md) (points where a client inserts its draws in
  a view's graph), [0059](0059-multisampling-in-the-gpu-layer.md) (a depth resolved when a pass
  asks for it) and [0111](0111-the-frame-contract.md) (an unlit kind of material at Stage 6);
  amended by [0117](0117-frustum-culling-and-instancing.md) (each buffer's uses are those of its
  type) and [0129](0129-drawing-for-the-editor.md) (lines drawn by the renderer; the first point
  after the sky; the scene's depth resolved and a part of a texture written wait for their first
  user; the milestone in two)
- Date: 2026-10-20

## Context

The foundations pass ([ADR-0097](0097-an-audit-pass-and-a-foundations-pass-before-stage-6.md),
[`docs/PLAN.md`](../PLAN.md), P6) decides what the GPU layer, `slime-gfx`, and the renderer give the
editor and the start of Stage 6 (C2-02, C2-05). `slime-gfx` is the one crate that uses wgpu
([ADR-0016](0016-gpu-layer-with-wgpu.md)), apart from the dev overlay's `slime-devui` through its
`wgpu` feature ([ADR-0031](0031-dev-overlay-with-egui.md)); it grows by pieces, each with its first
user and an ADR.

- **What a 3D editor draws, the GPU layer cannot yet.** A pipeline draws lists of triangles, without
  blending; no render target has an integer format; reading a target back waits until the GPU is
  done; `slime-gfx`'s README says "Not there yet: blending and array textures". The renderer has
  one kind of material, lit ([ADR-0035](0035-physically-based-materials.md)), and its passes are a
  closed list in a private graph ([ADR-0041](0041-render-graph.md)): no client draws into a view.
  The scene's depth exists at four samples per pixel only, and nothing resolves it into one
  ([ADR-0059](0059-multisampling-in-the-gpu-layer.md)), so that a draw after tone mapping cannot be
  hidden behind the scene. An editor draws a grid, icons of lights, *gizmos*, the handles that
  move, turn or scale a selection, an outline around it, and asks which object is under the mouse,
  *picking*.
- **Instancing asks for more too.** A draw draws one instance, and an indexed draw takes its
  vertices from the start of its vertex buffer (`RenderPass::draw`, `draw_indexed`); a buffer has
  "a type per use", vertices, indices or uniform values, and a uniform buffer is written "always
  with contents of the size given at creation", or from its start
  ([ADR-0026](0026-resource-binding-and-depth.md), [ADR-0106](0106-the-measured-quick-wins.md)).
  Frustum culling and instancing, Stage 6's first milestone (ADR-0097), draw many instances per
  draw from a storage buffer, and meshes that share buffers.
- **Without these pieces**, the editor would work around the renderer: gizmos drawn by its UI over
  the image, without the scene's depth, and picking by approximate boxes, to undo later.

The owner took three decisions on 2026-10-20, one at a time, each among the options that
"Alternatives considered" lists.

## Decision

**The GPU layer grows by pieces, each with its first user, now known.** wgpu stays inside
`slime-gfx`, so that the native backends of Stage 9c replace it there. What comes, and its user:

- for frustum culling and instancing: a storage buffer of the instances' values, a buffer that
  declares several uses (storage, copies, and later indirect draws) and that is written by ranges;
  meshes that share buffers, an indexed draw starting from a base vertex; ranges of instances per
  draw;
- for the editor: blending, lines, an integer format of render target, reading a target back without
  waiting, writing a part of a texture;
- with their first users, Stage 9a's many lights and Stage 9b's particles on the GPU: compute
  shaders and indirect draws.

ADR-0026's rules give way as these come: a buffer may declare several uses, and be written by
ranges, as ADR-0106 wrote the start of a uniform buffer.

**A milestone of rendering for the editor, early in Stage 6.** It comes after culling and instancing
and the views of ADR-0111, which give orthographic projections, and before the editor's viewport:

- blending and lines in `slime-gfx`;
- an unlit material, for gizmos and icons: a second kind of material, which ADR-0111 left to Stage
  9a;
- picking by the GPU: an image of integers, at one sample per pixel, the numbers not being averaged,
  where each pixel holds the number of the instance drawn there, which `slime-world-render`, the
  bridge between the world and the renderer, maps to its entity's persistent identity
  ([ADR-0109](0109-stable-references-and-undo.md)); it is read back without holding up a frame, its
  answer coming a frame or two later, and given up without a panic when the device is lost; a click
  between the leaves of a texture that cuts them out goes through to what is behind;
- picking by the CPU beside it, a ray against the instances' bounds, then the triangles of the
  meshes that it meets, which it keeps in the CPU's memory for that, for what runs without a GPU:
  tests, the agents of [ADR-0099](0099-ai-agents-drive-the-engine.md) and a game's questions,
  before Stage 7's physics brings its own rays;
- a view's image shown in a panel of the editor's UI, which samples the view's target through the
  GPU layer, so that the editor's code never names wgpu.

These pieces serve 2D and stylized games too ([ADR-0098](0098-the-games-the-engine-aims-at.md)).
Transparency sorted by depth, as glass and smoke need, stays at Stage 9a: blending here is what
gizmos and icons need.

**Two points where a client inserts its draws.** Each view's graph declares them:

- *in the scene pass, after the opaque instances*: the draws share the scene's depth, at four
  samples, so that they hide behind objects and are anti-aliased, as a grid, icons of lights and
  gizmos placed in the world are;
- *after tone mapping*, on the final image, in the display's colors: an outline, gizmos always on
  top, drawings in 2D; the scene's depth is resolved into one sample when a pass at this point asks
  for it.

A client, the editor or a tool, and a game from Stage 9a, registers its draws at these points, view
by view; the graph's inside stays private. Stage 9a adds points, with the passes of a game's own
that ADR-0098 places there.

**When it is built.** The pieces of culling and instancing with them, Stage 6's first milestone;
the milestone of rendering for the editor next, about 1 500 lines with their image tests by the
audit's estimate, not checked; compute shaders and indirect draws with their first users.

## Alternatives considered

How the GPU layer grows:

- **A second GPU layer designed now, whole**: shapes consistent at once, but compute and indirect
  draws designed without a user, against the rule of a second concrete use, and the code would come
  by pieces all the same.
- **wgpu used by the renderer directly**: all of wgpu at once, but ADR-0016's boundary given up,
  which lets Stage 9c's native backends replace wgpu in one crate.

The editor's pieces:

- **Each piece built when a panel of the editor asks for it**: each comes with its use, but under
  pressure the editor works around the renderer, gizmos over the image without depth and picking by
  boxes, to undo later.
- **Picking by the CPU alone**: without a GPU, testable anywhere, exact to the triangle; but leaves
  that a texture cuts out are picked where they are not, and nothing for lines, blending and the
  rest.

Passes that a client inserts:

- **The whole graph opened now**, any pass anywhere, declaring what it reads and writes: the most
  flexible, but it designs Stage 9a's passes of a game's own without their users, and freezes the
  graph's inside before many lights and temporal anti-aliasing change it.
- **The editor's overlays drawn apart and laid over the image by the UI**: nothing changes in the
  renderer, but no depth test against the scene, a grid and gizmos that never pass behind an object,
  and two images to compose per view.

## Consequences

- `slime-gfx` gains buffers of several declared uses written by ranges, instanced draws from a base
  vertex, blending, lines, integer targets, reading back without waiting, writing part of a texture,
  and a pass that resolves a depth of four samples into one, which reads a multisampled depth that
  no binding reads today.
- The renderer draws an unlit kind of material and an image of instances' numbers for picking; the
  editor picks at the pixel with the GPU, and anywhere without one with the CPU.
- The editor draws its grid, icons, gizmos and outline through two points of each view's graph,
  never around the renderer.
- [`docs/ROADMAP.md`](../ROADMAP.md) says it: Stage 6 builds the pieces of culling and instancing,
  then a milestone of rendering for the editor before its viewport; Stage 9a adds compute, indirect
  draws and more points for a game's own passes.
