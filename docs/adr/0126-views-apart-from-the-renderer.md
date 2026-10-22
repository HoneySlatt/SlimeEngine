# 0126. Views apart from the renderer

- Status: Accepted; amends [0038](0038-shadow-mapping.md) and [0064](0064-cascaded-shadow-maps.md)
  (the cascades fitted to each view's camera, drawn in turn into one atlas),
  [0041](0041-render-graph.md) (a graph for each view),
  [0047](0047-reversed-depth-with-an-infinite-far-plane.md) (an orthographic camera's far plane),
  [0048](0048-rendering-relative-to-the-camera.md) (positions relative to each view's camera),
  [0058](0058-shadows-for-every-spot-light-in-one-atlas.md) (the spot lights' tiles drawn once a
  frame), [0084](0084-what-the-world-draws.md) (a frame's scene given a point rather than a camera),
  [0106](0106-the-measured-quick-wins.md) (the lights nearest a point that the game gives first),
  [0111](0111-the-frame-contract.md) (how views are built), [0116](0116-stage-6-in-two-halves.md)
  (M6a.8 in two milestones) and [0122](0122-the-development-channel.md) (version 3 of the protocol:
  a view captured); amended by [0127](0127-the-settings-of-a-view.md) (a view's targets made again
  once its size holds still)
- Date: 2026-10-22

## Context

[ADR-0111](0111-the-frame-contract.md) decided views apart from the renderer: what every view
shares made once, each view with its own camera, size, settings, targets, timers and part of the
render graph, a perspective or orthographic projection and an offset of it, several views a frame,
and a view's old targets freed before new ones are made. It left to Stage 6a how.

Until now a `Renderer` drew one output of one size: it held its pipelines, its meshes' pages, its
uploader, the two shadow atlases of 64 MiB each, the buffers of its frame's values and of its
instances, and its frame's graph with the targets at the output's size; the `Scene` held the
camera, a perspective one. Its rustdoc advised one renderer per output size, and ADR-0041 one per
size for its timers: four renderers of 1280 × 720 with the sun held about 508 MiB on an RX 6900 XT,
about 260 of them in the cascades' atlases (ADR-0111). The audit before Stage 6 found three defects
there (G18, G37, G47 in `docs/PLAN.md`): a resize built the targets of the new size while those of
the old one lived, and kept them after `GfxError::OutOfMemory`; when creating the spot lights'
atlas ran out of memory after the cascades' atlas was created, the group that binds both was never
made again, and the sun lost its shadows without a word; a multisampled output made `render` panic
inside wgpu.

The owner took two decisions on 2026-10-21, one at a time:

- **Q1, the milestone**: M6a.8 split in two at its start, as the plan allowed: M6a.8a builds the
  views and what they share, M6a.8b the settings of a view (its render scale, anti-aliasing, mode of
  debugging, passes, and targets made again once its size holds still).
- **Q2, who holds a view**: the caller, as a value that `Renderer::create_view` makes and that
  dropping frees, rather than the renderer, which would name its views by handles.

## Decision

**A view is a value that the caller holds.** `Renderer::create_view(gpu, label)` makes a `View`:
the buffers of its frame's values, of the shadow passes' views and of its instances, its tone
mapping's settings, its frame's graph with the high-dynamic-range and depth targets and the bloom's
levels at the size it draws, their timers, and the statistics of its last frame
(`View::stats`). `Renderer::render(gpu, &scene, &mut [ViewDraw])` draws a scene through each view,
in their order; a `ViewDraw` gives a view its camera, its target and the rectangle of the target
that it draws into (`Viewport`), the whole target by default. A game holds one view for each image
that it draws, the editor one per viewport, and drops it to free its memory. The renderer keeps
what every view shares: its pipelines, its layouts, the BRDF table, its meshes' pages, its uploader,
whose budget each call renews once, and the shadow atlases.

**The scene has no camera.** `Scene::new(instances)`: its instances, lights, environment, exposure
and bloom are the same in every view. The renderer draws the lights that it can draw from every
view's camera, the first `MAX_LIGHTS` of them; a game whose scene holds more orders them by what
matters to its views, and `slime-world-render`'s `Frame::scene(near)` orders the point and spot
lights by their distance to a point that the game gives, the eye of its main view's camera or where
its player stands, rather than to the camera of the scene.

**What views share is drawn once or in turn.** The spot lights' tiles do not depend on the camera:
the first view of a frame draws them, and every view reads them, each through matrices relative to
its own eye. The sun's cascades are fitted to each view's camera: one atlas holds one view's
cascades at a time, each view drawing its own before its scene pass reads them, as the GPU runs the
submissions in their order. The group that binds the atlases is made again whenever they differ
from those it was made with (G37). Each view's values are relative to its own camera's eye
(ADR-0048).

**A camera projects in perspective or orthographic, and draws its whole image or a part of it.**
`Camera::projection` is a `Projection`: `Perspective { fov_y }`, as before, or
`Orthographic { height, far }`, whose depth is reversed and linear, from 1 at the near plane to 0 at
the far one, beyond which nothing is seen, frustum culling included. An orthographic camera sees
every pixel along its axis: the sky shows as it lies along it, and surfaces reflect it seen from
there. `Camera::part`, an `ImagePart`, moves and scales the whole image's projection so that the
part fills the view: parts drawn side by side make the whole image, for a split screen of one
camera or a magnifier, and fit the cascades to the whole image, so that its parts show the same
shadows.

**A view draws into a rectangle of its target.** Its targets are at the rectangle's size; its tone
mapping writes the rectangle alone, keeping the rest of the target, which other views draw, and
clears a target that the view covers. A map in a corner is a second view drawn into a rectangle of
the window's image, after the first.

**A view resized frees its old targets first.** When a view's rectangle changes size, it drops its
frame and waits for the GPU to finish the frames that use its targets, which gives their memory
back, then makes the new ones: the GPU never holds both sizes (G18), and a size that does not fit
returns the error and leaves the view without targets. The wait costs the frame that changes size
the GPU's time of the frames in flight; M6a.8b waits for a size to hold still before it makes
targets again. Every view of a call is prepared, its values written and its targets made, before
any is recorded: an error draws nothing, in any view.

**A multisampled output is a bug of the caller** (G47): the renderer resolves its own samples, and
`render` panics, with a message that says so, on an output of several samples per pixel, as on a
rectangle that does not lie inside its target.

**A tool captures a view.** `slime-app`'s `Presentation::views` names the views that its last frame
drew into the window's image, with their rectangles (`ShownView`). Version 3 of the development
channel's protocol lists their names in the game's state, and a capture names one, or none for the
whole image (`Request::Capture { view }`); the host crops the image it read to the view's
rectangle. The MCP server's `capture` takes the view's name. The `world` example draws a map from
above, orthographic, in its window's top right corner, and a test captures each of its two views
through the server.

**The memory of views is measured.** `Gpu::memory_in_use` gives what wgpu's allocator counts, on
Vulkan and Direct3D 12, without what the driver keeps for itself. Four views of 1280 × 720 with the
sun take about 60 % of what four renderers take, one cascades' atlas in place of four
([`docs/measurements.md`](../measurements.md)).

## Alternatives considered

Who holds a view:

- **The renderer, which names its views by handles**: the renderer knows every view and could
  share their memory, but a view's lifetime would be the renderer's to manage, its handles could
  outlive it, and the editor's viewports and a game's split screen already own their state.
- **One renderer per view**, as before: no new type, but each view would hold the pipelines, the
  meshes' pages and both atlases, 128 MiB more per view, and meshes made by one renderer drawn by
  another.

What views share:

- **A cascades' atlas for each view**, from a pool: views could be recorded in parallel, but each
  holds 64 MiB more, and views are recorded one after the other on one thread until Stage 7 decides
  a render thread (ADR-0111); a pool of one, used in turn, is what that leaves.
- **The spot lights' tiles drawn by each view**: no first view that draws for the others, but the
  same depths drawn again for every view.
- **Lights for each view**, a scene per view: each view could draw the lamps nearest its own camera,
  but a scene would be built and checked once per view, and the views of one frame would light the
  same surface differently.

A view in a rectangle:

- **A target of the view's size, copied into the output**: no change to the tone mapping, but a
  copy and a target more for each view.

Freeing the old targets:

- **Drop them without waiting for the GPU**: no wait at a resize, but wgpu frees their memory only
  once the frames that use them are done, after the new targets are made: the GPU would hold both.

Capturing a view:

- **The tool crops the whole image**: no change to the protocol, but the tool would have to know
  where each view is drawn, which only the game knows.

## Consequences

- A game creates a view for each image that it draws, keeps it from frame to frame, and gives
  `render` its cameras; `Scene::new` takes no camera; `Frame::scene` takes a point.
- The memory of a renderer no longer grows with the outputs that it draws: four views of 1280 × 720
  take about 60 % of what four renderers took.
- A view that changes size waits for the GPU once; one whose size changes at every frame, as while
  a window's edge is dragged, waits at every frame until M6a.8b.
- A view drawn into a rectangle of the window's image keeps the rest of the image: the first view
  drawn into it covers it, or the game clears it.
- The protocol of the development channel is at version 3: tools and games of version 2 refuse
  each other.
- The renderer's known debt of one renderer per output size, and its frame's lost tone-mapping and
  bloom times when its shape changes at every call, is a view's now, with one view per size.
