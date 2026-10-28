# 0159. Picking and the selection

- Status: Accepted; amends [0155](0155-the-editors-design.md) (Alt and a click told from an
  orbit by the mouse's stillness; the second viewport and the `.keys` preset in Blender's way in a
  milestone of their own), [0158](0158-the-viewport.md) (Blender's way waits for that milestone),
  [0115](0115-what-the-gpu-layer-gives-the-editor.md) (the scene's depth resolved into its
  farthest sample, by the outline's frames alone), [0129](0129-drawing-for-the-editor.md) (what a
  client draws gains an outline) and [0127](0127-the-settings-of-a-view.md) (two modes of debugging
  more); amended by [0160](0160-two-viewports-and-blenders-way.md) (the buttons that select are
  the table of shortcuts')
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided how the user picks in the viewport and sees what is
selected: a click picks by the GPU and selects the outermost placement, a second click a level
deeper; Alt and a click go through what lies behind, by the CPU's ray; a marquee selects what its
area holds; the selection is outlined from a mask of what is selected, the scene's depth resolved,
its hidden part dimmed; an entity without a mesh is drawn as its transform's unit box; modes of
debugging show the base color and the light alone, and a wireframe of the selection. Milestone
M6c.2b, which was to bring them with a second viewport and the `.keys` preset in Blender's way, was
cut in two at its start, on 2026-10-28, as ADR-0155 allows: M6c.2b, picking and the selection,
which this ADR records; M6c.2c, the second viewport and Blender's way.

What the code held before this milestone:

- `slime-render` picked by the GPU (`View::pick`, [ADR-0130](0130-picking.md)): a view asked for
  an area draws, at its next frame, the picking number of each instance where it is nearest, and
  answers a frame or two later. `slime-world-render`'s `Picker` cast a ray by the CPU and gave the
  entity that it met first.
- The scene's depth existed at four samples per pixel, which no shader could read: `slime-gfx`
  bound no multisampled depth target. ADR-0115 had decided that a pass resolves it into one sample
  when a pass after tone mapping asks for it, and [ADR-0129](0129-drawing-for-the-editor.md) left
  it for its first user, the outline.
- What a client draws into a view (`Drawings`) was lines and instances in the scene and lines over
  the image.
- A view showed the scene, its normals or its shadows (`DebugMode`).
- The editor's left button orbited with Alt and did nothing without it; the icons of the lights
  were picked as nothing.

## Decision

**What the left button does.** Over the viewport, the left button pressed and let go within 4
logical pixels is a click; moved farther, it draws a marquee, a rectangle over the viewport drawn
by the window's interface, which the image under it does not draw again. A click asks the view what
is drawn at its pixel, a marquee what is drawn in its area, both through `View::pick`, the answer
taken a frame or two later. Alt and the left button orbit, as ADR-0158 decided; pressed and let go
while the mouse moved less than 4 units of its raw motion, taken for logical pixels, as a click
may, they are Alt and a click, which casts a ray through the point by the CPU
(`Picker::cast_through`, every entity whose mesh the ray meets, the nearest first), answered at
once. Shift adds what a click or a marquee finds to what is selected, a click taking out
what was selected already.

**What it selects.** A click selects the outermost placement of what is drawn at its pixel
(`World::placement_root`), or the entity itself, which belongs to none; a click again on what lies
below the one entity selected selects the next entity down from it towards what was clicked, a
level deeper at each click, and the outermost placement again past it. A marquee selects the
outermost placements of what it holds, as the GPU draws them, what is hidden left out. Alt and a
click select, among the outermost placements that the ray meets, the one behind what is selected,
or the nearest when none of them is: a click after another goes through them. Where nothing is
found, nothing is selected. The selection reaches the core as an agent's does, a request that is
not a step. The icons of the lights are picked as their lights, and a click selects them as
theirs; the CPU's ray meets meshes alone, so that Alt and a click go through no icon.

**The outline.** `Drawings` gains `outline`, the picking numbers of the instances outlined, the
scene's and those that a client draws in the scene, its color where the scene shows them and where
it hides them, and its width in the output's pixels: a frame builder gives each instance its
entity's number, so that the editor names an entity's whole without telling its instances apart.
A view that outlines something runs three passes more: after the pass after the sky, the scene's
depth is resolved into one sample, the farthest of the four, so that a surface seen in part of a
pixel counts as seen there, as ADR-0115 decided, in the frames that outline alone, and without
anti-aliasing the scene's own depth is read; then a mask is drawn at the view's render scale, the
outlined instances nearest first, culled as the scene pass culls them, each texel covered marking
whether its depth is not farther than the scene's, within the change of its depth across the
pixel; after the tone-mapping pass, each pixel outside the mask, less than the outline's width and
a pixel from the centre of one that it covers, takes the outline's color where the scene shows
what it outlines nearby, and its hidden part's otherwise, fading over the last pixel: an outline
two pixels wide covers the two pixels beside what it outlines along a row or a column. What a
masked material cuts out is not outlined; a blended surface is where its alpha reaches one half, as
picking finds it. `slime-gfx` lets a shader read a multisampled depth target sample by sample
(`BindingType::MultisampledDepthTexture`). The editor outlines what is selected and what lies below
it, in a light orange, half as opaque where hidden, two logical pixels wide; a selected entity
that draws nothing, nor any entity below it, not even a light's icon, is drawn as the box of a
metre that its transform places, over the image.

**Modes of debugging.** `DebugMode` gains `BaseColor`, each surface's color, its texture applied,
written as the other modes are, without light, shadows or the tone curve; and `Light`, the light
alone: the scene drawn as it is, exposed and tone mapped, under its sky, every surface of the
physically based kind white, not metallic and without emission, its roughness, normal and occlusion
kept. Both are branches of the scene's shaders; the unlit kind shows its color in both. The editor's
View menu shows the scene lit, its base color, its light alone, its normals or its shadows, the one
on ticked, by operations that agents run by name too (`view.shade_*`), and the wireframe of what is
selected (`view.wireframe`): the edges of the meshes of what is selected and below it, each once,
read from the cooked data, as lines over the image.

**Agents.** A tool picks in the editor's viewport by its name, `viewport`, as in a game's view: the
entity drawn at a pixel, answered once the viewport has drawn the picking. It selects through the
channel, as before, not by a click.

## Alternatives considered

- **The outline drawn from the scene's own picking target**, the numbers of every instance, then
  compared with the selection's: a target of 64 bits per pixel drawn at every frame that outlines,
  and nothing to tell the hidden part, which the scene's numbers do not hold.
- **The selection's instances marked in the scene** (`Instance::outlined`): the scene's batches
  split by the mark, and a frame builder that knows what an editor selects; the numbers that
  picking gives already name what a client selects.
- **The mask's depth tested against the scene's, read-only, in the pass itself**: a pass that
  clears its color and keeps its depth, which neither the GPU layer nor the graph declare, and two
  pipelines per kind, the hidden part and the whole.
- **The scene's samples read by the mask's shader**, without a pass that resolves them: two
  pipelines per kind, one for each number of samples, where ADR-0115 decided a pass that later
  users share.
- **The nearest of the four samples as the resolved depth**: an edge where something in front
  covers part of the pixel would count as hidden, and the outline would flicker along it.
- **Alt and a click told by a key held apart from Alt**, or the middle button orbiting as Blender's:
  ADR-0155 keeps one default mapping, Alt and the left button orbiting as most editors have it, and
  a click that does not move does not orbit.
- **The marquee drawn by the viewport's image, over its image**: the view drawn again at each frame
  of the drag, where the interface's box costs nothing.
- **A marquee that selects what it holds hidden too**, by the CPU: the GPU's area answers what the
  user sees, as most editors do without an X-ray mode.
- **The wireframe of the whole scene**: ADR-0155 waits for `PolygonMode::Line` measured against
  lines along the edges.
- **One milestone for the second viewport and Blender's way with picking**: about as large again as
  the viewport's, and of one long review; cut, as ADR-0155 allows.

## Consequences

- The scene's depth resolved and the outline, which ADR-0129 left for this milestone, are settled;
  a view that outlines nothing runs none of their passes.
- A client names what it outlines by the numbers it picks by; an instance picked as 0 is never
  outlined.
- The outline costs, at each frame that outlines, a pass over the scene's depth, the outlined
  instances drawn once more, and up to 289 reads of the mask for each pixel of the view, at its
  widest, which nothing has measured at 3840 × 2160 yet.
- The editor's left button selects; the wireframe of a large mesh, as a terrain's, draws one line
  for each of its edges, which the window builds again at each frame that draws the viewport.
- Every multisampled depth target is made readable by shaders, a view's whether it outlines or
  not, which some GPUs may compress less; nothing has measured it.
- M6c.2c brings the second viewport, the one navigated drawn at the full rate, and the `.keys`
  preset in Blender's way with its mouse's buttons.
