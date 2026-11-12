# 0243. The canvas reads its items by their index, from a storage buffer or a texture, and draws in passes

- Status: Accepted; amends [0137](0137-the-canvas.md) (the items read from a storage buffer or a
  texture, their fields one by one; a target drawn without a second view where the backend has
  none; the canvas's passes) and [0181](0181-uwuis-scope.md) (one shader with two ways of reading
  an item, not one path of instances)
- Date: 2026-11-12

## Context

The canvas reads each item, nine `vec4`s, from a storage buffer in both of its shaders, and draws
into an sRGB target through a view without its encoding ([ADR-0137](0137-the-canvas.md)). WebGL2
has neither: wgpu's GL backend gives no storage buffer, no view of a texture in another format, and
a window's image that can only be drawn into. [ADR-0181](0181-uwuis-scope.md) planned one path,
the items handed on as instances of vertex buffers; [ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md)
named the risk: the items handed to the fragment shader as values of the vertices were measured
slower, 101.2 µs against 76.3 µs. [ADR-0242](0242-the-canvas-moved-in-three-steps-tokens-read-when-drawn-later.md)
gave M6d.4a a prototype thrown away, to choose the layout by measure.

The prototype, on the branch `m6d4a-prototype`
([`crates/canvas-prototype`](https://github.com/HoneySlatt/SlimeEngine/tree/744dce3373481e0ee054e665165a0fc5b4fc4faf/crates/canvas-prototype)),
draws `render.canvas`'s two screens and five effects with four layouts, on Vulkan, on Vulkan with
WebGL2's limits and on OpenGL ES 3.0, whose shaders are WebGL2's. Its figures are in
[`measurements.md`](../measurements.md#m6d4a-the-canvass-prototype); on the RX 6900 XT:

- **S**, today's storage buffer, each fragment reading its whole item; **S2**, the same buffer,
  each field read when its kind needs it; **T**, the items in a texture read by their index, fields
  one by one; **I**, small instances, one format for each kind of item, one draw for each run of
  one kind.
- Every layout draws the same image, within 2/255 of S's, on every backend that builds it. S and
  S2 build on no device with WebGL2's limits; T and I build on all.
- I draws as many times as the kinds change in a row: 287 draws on the game's menu, where boxes and
  texts alternate, against 3. It then takes 3 times T's GPU time at 1280 × 800, 9 times its CPU to
  record, and 3.5 times its CPU to submit on OpenGL ES. It is as fast as the others on the editor,
  whose items come in long runs of one kind.
- T takes 13 to 20 % more GPU time than S2 on every screen and size, 45 to 49 µs more at
  3840 × 2160. Their shaders differ in the function that reads a field, some ten lines.
- Every layout keeps the interface's budget of 0.5 ms at 3840 × 2160 on the two screens; with a
  group drawn apart, T passes the handheld tier's 50 µs at 1280 × 800, 55.8 µs with its copies,
  where S keeps it, 47.4 µs.

## Decision

**One shader, two ways of reading an item.** The canvas's shader reads an item by its index in both
stages, each field when its kind needs it, through one function. That function reads a storage
buffer where both stages of the device can read one: Vulkan, Direct3D 12, Metal and WebGPU. It
reads a texture of `Rgba32Uint`, floats stored as their bits, at most 2048 texels wide, elsewhere:
WebGL2, OpenGL ES and WebGPU's mode of compatibility. The canvas chooses at its start, from the
device's limits and downlevel flags, not from its backend. The items, their bytes, the draws and
the CPU's work are the same on both; only the binding and the write differ.

**One store for what varies in size.** Transforms, a path's segments and, later, the stops of a
gradient live in the same buffer or texture as the items, which name them by their index: no
binding, attribute or value between the stages grows with them.

**One pipeline, one draw for each run of one image,** as today. The items are drawn in their order;
nothing reorders them.

**The canvas's passes**, in a frame's order:

1. Each group drawn apart, into a texture of its own, the canvas's format; a group that has not
   changed keeps its texture and costs nothing.
2. The canvas, into the target, or into a texture of its own when the frame holds an element that
   blurs what lies behind it: a window's image cannot be read on every backend.
3. At each such element, the pass ends; the region behind it is halved twice, three times at
   3840 × 2160, blurred by a Gaussian separated across and down, and drawn back as the element's
   fill; then the pass resumes.
4. The canvas's texture, if it drew into one, copied onto the target.

**What a target must allow**: being drawn into, one sample per pixel; a format of 8 bits per
channel without encoding, `Rgba8Unorm` or `Bgra8Unorm`, into which the canvas writes colours
already encoded; or an sRGB format, which the canvas draws into through its format without
encoding where the device allows such a view, Vulkan, Direct3D 12 and Metal; or `Rgba16Float`,
the encoding continued past 1 ([ADR-0234](0234-uwuis-colour-model-before-its-api-is-stable.md)).
The canvas never reads its target.

**A transform for each item**: two rows of an affine map, the shape's distance computed in the
item's own space, its edge kept one pixel wide by the square root of the map's determinant; a clip
stays in the target's pixels. How a path is drawn stays open for uwui's first stage: the
prototype's paths only counted where their data lives.

## Alternatives considered

- **Instances, one format for each kind** (I), as ADR-0181 planned: fewer bytes written, a third
  of S's, and the fastest where kinds come in long runs; but a draw at every change of kind, which
  an interface makes at every label on a box.
- **Instances ordered by kind where they do not overlap**, as GPUI orders them: fewer of I's draws,
  not tried; the CPU's work to find overlaps, and an order that is no longer the list's.
- **The texture alone, everywhere** (T): one way to read, but 13 to 20 % more GPU on every device
  that has storage buffers, the margin that effects need.
- **Storage buffers alone** (S2): the fastest, but nothing on WebGL2, OpenGL ES or WebGPU's mode
  of compatibility.
- **The item handed on as values of the vertices**: measured slower at ADR-0137.
- **A uniform buffer of items for each draw**, 16 KiB: at least 37 draws on the game's menu.
- **A blur by dual Kawase**: each level read twice, its last pass at half the screen, and a reach
  that its radius does not set.

## Consequences

- M6d.4b1 moves the canvas as it is, its storage buffer and its images unchanged; M6d.4b2 gives it
  the two ways of reading, each item's fields read when its kind needs them, its images on both
  compared in CI, on lavapipe and on Mesa's GL.
- A canvas holds at most as many items as its store: 466,033 in a texture of 2048 × 2048 texels,
  fewer than today's `MAX_CANVAS_ITEMS`; more where the device's textures are larger.
- On radeonsi's OpenGL ES, images of colours drawn under a rotation differ from Vulkan's by up to
  10/255 on 216 pixels of 1,024,000; Mesa's GL on the CPU does not show it. M6d.4b2's image tests
  on GL keep their own references where a transform turns an image.
- Not measured: a GPU of WebGL2's class, a Mali or the owner's laptop, Windows, a browser; no GPU
  time on OpenGL ES, which has no timestamps. The web's trial measures them.
- The prototype's glass, four panels at 3840 × 2160, takes 546 to 586 µs with its copies,
  over the interface's 0.5 ms on every layout; uwui's first stage, which builds glass, measures it
  again.
- Writing only what changed: one item rewritten costs its own bytes, 144, against 14.4 MB for
  100,000 items written whole; the canvas writes every item each frame until uwui's drawing keeps
  its items across frames, at its first stage.
