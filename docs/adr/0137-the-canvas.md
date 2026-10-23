# 0137. The canvas

- Status: Accepted; amends [0131](0131-stage-6b-and-stage-6c.md) (M6b.3 cut in two: the canvas,
  then the text and the display list) and [0132](0132-the-engines-ui-framework.md) (no scissor
  apart from the viewport; the interface blends in sRGB's encoding)
- Date: 2026-10-23

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that `slime-render` gains a *canvas*: rounded
rectangles with their borders and shadows, glyphs and images, each clipped, drawn in a few batches
after the views, into any target, sharing the renderer's shader reloading, GPU timers and recovery
from a lost device. It gave the canvas's items the indices of a transform and of a layer, which stay
the identity and the root's until Stage 7. It left to the milestone that draws the interface whether
it blends in linear space or in sRGB's encoding, chosen by the owner from images, and the
interface's share of a frame on each hardware tier of
[ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md).

Stage 6b's third milestone, M6b.3, was to build the canvas, the display list, the bridge between
them and the glyphs' atlas at once. At its start, the owner cut it in two on 2026-10-23: M6b.3a, the
canvas, and M6b.3b, the text, the display list and `slime-ui-render`. Building the canvas raised
what ADR-0132 left open, and how its items are drawn, how its shadows are computed, how it clips,
and what its budget is checked against.

The owner decided on 2026-10-23 the canvas's budget, a tighter one than the plan's 1 ms; then, the
first measurements made, what the handheld tier's benchmark draws; then, from pairs of images, how
the interface blends. The design below was validated the same day.

## Decision

**What a frame draws.** A client fills a `Canvas` each frame: boxes, a rectangle with its four
corners' radii, its fill and a border inside its edges; the shadows that boxes cast; and parts of
images, each item with its clip, an axis-aligned rectangle, in the target's pixels. Images are
textures, of colors or of one channel, `R8Unorm`, how much of each texel a shape covers, drawn in a
color, as glyphs are; or a render target that a view drew, which a panel shows. The canvas borrows
them for the frame. `Renderer::draw_canvas` draws it over a target of the renderer's format, in a
pass of its own, after the views: the window's image, or a panel's target. It reports the items
drawn, those left out past its bound, the draws, and its time on the GPU, and is drawn again by a
renderer made again after a lost device.

**One pipeline, items in a buffer.** Each item is nine `vec4`s in a storage buffer, its kind among
them; the vertex shader builds two triangles from the vertex's index alone, and the fragment shader
reads the item again by its index and draws it by its kind. Items are drawn in their order, each
over those before it, and one draw takes every run of items that read one image, the items that read
none joining any run. Handing the item to the fragment shader as values of the vertices was measured
slower, for the many small quads of glyphs.

**Shapes are signed distances.** A box's coverage of a pixel is its signed distance at the pixel's
centre, rounded corners included, made a coverage over one pixel; its border is the box less the box
inset by the border's width. A box's quad is its rectangle grown to whole pixels, so that a pixel
that its edge crosses is drawn, whatever fraction of a pixel the edge lies at.

**Shadows are Raph Levien's approximation.** A box blurred by a Gaussian is computed in closed form:
the distance to the box, its corners measured as superellipses that the blur widens, and across it
the integral of the Gaussian over the box's shorter side, as vello computes it. Its constants, which
depend on the item only, are computed once per item on the CPU. Its largest error against the box
blurred exactly is a few hundredths of the shadow's alpha, the largest for a circle whose blur nears
its radius; `Canvas::draw_shadow`'s rustdoc gives the figures, which a test checks against an exact
integral. A shadow is drawn under the box, as egui and Flutter draw it, not cut out of it as CSS
does: the box that casts it covers it. The blur counts a pixel's own width, so that a blur of less
than a pixel is drawn smooth.

**Clips are cut in the shader, and on the CPU.** Each pixel's share inside its item's clip is
computed in the fragment shader, so that a clip that ends between two pixels covers one in part. The
canvas cuts each item's quad to its clip, grown to whole pixels, when the item is added, and leaves
out an item that its clip leaves no pixel of. `slime-gfx` gains no scissor apart from the viewport,
which ADR-0132 planned: clips need none.

**The interface blends in sRGB's encoding.** Into a target of an sRGB format, colors mix as their
sRGB values do, as design tools, browsers and Windows mix them, rather than as light does: text has
the same weight on a light and on a dark background, and an opacity looks as it does in the
designer's tool. The canvas draws into the target through a view without its encoding, which
`slime-gfx` gives (`RenderTarget::without_srgb`), its colors multiplied by their alpha and encoded
before they mix: an item's colors once, on the CPU, as the canvas's bytes are written, consecutive
items of one color encoded once; an image's texels in the shader. Into a target of another format,
which holds linear values, `Rgba16Float` as the tests use, colors mix linearly. A window's images
are made so that a view without the encoding is allowed: Direct3D 12 always allows it, and Vulkan
with `VK_KHR_swapchain_mutable_format`, which the desktops' drivers have; `slime-gfx` refuses to
make a window's surface without it, rather than draw it otherwise.

**Colors come multiplied by their alpha.** `slime-gfx` gains the blending of such colors,
`Blend::Premultiplied`, with which a shape that covers a pixel in part multiplies its color and its
alpha alike by that share. An image of colors is filtered before its texels are multiplied by their
alpha: a transparent texel lends its color to its opaque neighbour, so that its color should be its
neighbours', or their edge darkens, as `CanvasImage`'s rustdoc says.

**The budget: half a millisecond on each tier.** The interface takes at most 0.5 ms of GPU on each
tier's reference machine: on the RX 6900 XT, with ADR-0114's ratios, 50 µs at the handheld tier's
1280 × 800, 250 µs at the modest tier's 1920 × 1080, and 500 µs at the high tier's screen
([ADR-0138](0138-the-high-tier-at-3840-2160.md)). The benchmark `render.canvas` draws two screens,
each laid out for 1920 × 1080: a game's menu over its image, which the budget bounds at every tier;
and an editor's screen, which it bounds at the modest and high tiers, measured at 1280 × 800 too,
where no editor runs. Both hold their budget ([`measurements.md`](../measurements.md), M6b.3a).

**What the canvas does not do yet.** Items are not rotated or scaled until Stage 7 gives them
transforms; an image is not rounded; a canvas holds at most `MAX_CANVAS_ITEMS` items, past which the
last are left out, with a warning. M6b.3b draws text through it, from the display list that it adds
to `slime-ui`, and writes parts of the glyphs' atlas textures.

## Alternatives considered

The milestone:

- **M6b.3 whole**: one milestone as planned, but the canvas, the display list, the bridge, the
  atlas, the text's choices and the canvas's budget in one review.

How shadows are computed:

- **The blur integrated over four rows**, as Evan Wallace's shadows do: an error of up to three
  hundredths, of the same order, for twice the canvas's cost on the GPU in the editor's screen,
  which its shadows took the most of.
- **A blurred image of the box**, made by a pass of blur: exact, but a pass and a target for each
  shadow's size.

Clips:

- **A scissor per item**, as ADR-0132 planned: no work in the shader, but a draw for each clip, and
  no edge between two pixels.

How the interface blends, from images that the owner compared:

- **Linear**: exact light and nothing to add, but text darker than its background thin and text
  lighter than it bold, and opacities that look lighter than the designers' tools show them.

The budget, and what checks it:

- **1 ms on each tier**, the plan's: margin for the canvas as first built, less for the game's
  frame, at 144 frames per second most of all.
- **The editor's screen at every tier**: one screen, but an editor of letters 6 pixels wide on a
  handheld that runs no editor, and the handheld's budget spent on it.
- **The handheld tier's budget raised to 0.75 ms**: the screen kept, less time for the game on the
  weakest machine.
- **Drawing in a compute shader, every pixel written once, as vello does**: the overdraw of an
  interface's layers gone, but compute shaders come to `slime-gfx` at Stage 9a
  ([ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md)).

## Consequences

- `slime-gfx` gains `TextureFormat::R8Unorm`, a texture's format only; `Blend::Premultiplied`;
  `TextureFormat::Bgra8Unorm` and `TextureFormat::without_srgb`, and `RenderTarget::without_srgb`,
  which every target of an sRGB format allows; and refuses a window whose images do not allow it
  (`GfxError::SurfaceWithoutEncoding`). `slime-color` gains `linear_to_srgb`.
- `slime-render` gains the canvas, its shader `canvas.wgsl`, reloaded with the others, and
  `Renderer::draw_canvas`.
- The example `interface` draws a canvas over a window or into a PNG file; CI runs it on Linux and
  on Windows, and builds it for the Windows artifact.
- The interface's budget is checked by hand on the owner's GPU, as ADR-0114's budgets are; the
  handheld tier's ratio, a tenth, may be generous for an interface, whose cost lies more in writing
  pixels than in computing them, until a handheld measures it.
