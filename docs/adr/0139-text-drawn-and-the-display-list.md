# 0139. Text drawn, and the display list

- Status: Accepted; amends [0133](0133-font-files-under-the-open-font-license.md) (the font of the
  editor's and the overlay's text: Geist)
- Date: 2026-10-24

## Context

[ADR-0132](0132-the-engines-ui-framework.md) decided that `slime-ui`, without a GPU, gives what an
interface draws as a *display list*, owned data that a frame sends to the thread that draws, each
item with its clip, transform and layer, the transform the identity and the layer the root's until
Stage 7; that a bridge, `slime-ui-render`, turns a display list into the renderer's canvas and keeps
the glyphs' *atlas*, pages in which etagere places the glyphs' images within a budget, the least
recently used leaving first, rasterized again after a lost device; that the interface's images are
cooked textures that the renderer's asset cache holds, once its requests name images, as the
milestone that draws them decides. [ADR-0136](0136-the-interfaces-text.md) left to the owner,
judging images of the text drawn, whether glyphs are hinted;
[ADR-0133](0133-font-files-under-the-open-font-license.md) made Noto Sans the font of the editor's
and the overlay's text, unless the owner prefers another when the text is first drawn.
[ADR-0137](0137-the-canvas.md) cut Stage 6b's third milestone in two: the canvas, done, then M6b.3b,
which draws the text through it.

Building M6b.3b raised what those ADRs left to it: when the interface's images come through the
asset cache, how the display list names them, what shape its rectangles take, how it holds its
texts, and how the atlas is laid out and filled.

The owner decided on 2026-10-23, among the options that "Alternatives considered" lists, when images
come through the asset cache and how rectangles are given, and validated the rest of the design
below; on 2026-10-24, from images of the text drawn by the engine, the editor's font and whether
glyphs are hinted.

## Decision

**The display list.** `slime_ui::display::DisplayList` holds items in the order they are drawn,
each a primitive, a box, the shadow that a box casts, a text or a part of an image, with its clip,
its transform and its layer. The transform and the layer are values that only the identity and the
root take, `TransformId::IDENTITY` and `LayerId::ROOT`, until Stage 7 gives them others. Its
coordinates are pixels of the screen: an interface lays itself out at the screen's scale. It is
`Send`, `Sync` and `'static`. Its colors are `slime_color::LinearColor`s, as every crate that holds
a color names them: `slime-ui` depends on `slime-color`, its first engine crate.

**A text is shared, not copied.** A text item holds its layout, `Arc<TextLayout>`, which `TextCache`
gives it as: a frame's display list holds the texts that the cache keeps, without copying their
glyphs. A layout that a display list still holds is copied before the cache breaks its lines again
in another box.

**One shape of rectangle.** `slime_ui::Rect` gives a rectangle by its top left corner and its size,
as the canvas's `PixelRect` and the renderer's `ImagePart` do; the text's carets and selections
use it too, where they gave edges before.

**Images are named by keys, which the program gives.** An image item names its image by an
`ImageKey`, a number that the program that builds the list chooses; the program that draws it gives
the image of each key, a texture or a view's target, when it draws. The asset cache comes behind
those keys with the first program that draws cooked images in an interface, the editor or a game's
screens.

**The bridge, and its atlas.** `slime_ui_render::UiRenderer` draws a display list through
`Renderer::draw_canvas` in two steps: first the atlas, which rasterizes and writes what the texts
lack, then the canvas, which reads its pages. The atlas holds pages of 1 024 × 1 024 texels: of one
channel, `R8Unorm`, for the glyphs of letters, and of colors, `Rgba8UnormSrgb`, for emoji; etagere
places each glyph's image in a page, with an empty texel around it, so that filtering at its edge
reads nothing of another. A page is made when a glyph finds no room, within the atlas's budget, 16
MiB by default; past it, the glyphs of its kind that no draw asked for the longest leave first, one
by one, those of the draw under way kept, then the pages of either kind that the draw under way
draws nothing from, so that pages of coverage never keep a page of colors out, nor the reverse. A
glyph that still finds no room is left out of that draw, and one larger than a page of every draw,
each counted; a warning says so the first time. A text's origin is rounded to a whole pixel, so that
its glyphs, placed at whole pixels within their layout, are drawn texel for pixel. After a lost
device, a new bridge rasterizes its glyphs again. `slime-gfx` gains the writing of a part of a
texture, `Gpu::write_texture`, through which the atlas receives each glyph.

**The editor's font is Geist, and glyphs are not hinted.** The owner compared Noto Sans, Fira Sans,
Geist and Open Sans, drawn by the engine at scales 1 and 2, on dark and light backgrounds, and chose
Geist, under the OFL, as nixpkgs packages it: Latin, Vietnamese included, and Cyrillic, not Greek.
The editor falls back on the system's fonts for the other writing systems, as ADR-0133 has it; the
overlay, which ships in games, draws those two alone. The font's file enters the repository with its
license, fetched with the owner's agreement, at the milestone that first draws the editor's or the
overlay's text. Glyphs stay unhinted, as ADR-0136 drew them: hinting sharpened stems a little at
scale 1, in the fonts that carry its instructions alone, and changed their shapes from one size to
the next.

**Checked against the CPU.** The bridge's tests draw a sentence of each writing system of
`slime-ui`'s tests, at scales 1 and 2, and compare each pixel with the same glyphs rasterized and
laid over black by the CPU. The benchmark `ui.draw.menu` draws a game's menu of real text at the
hardware tiers' sizes, within the interface's budget that ADR-0137 sets
([`measurements.md`](../measurements.md), M6b.3b).

## Alternatives considered

Images:

- **Through the asset cache now**: the cache holding textures by their asset IDs, ready for the
  editor; but designed before any program draws a cooked image in an interface, and M6b.3b larger.

Rectangles:

- **By their edges**, left, top, right and bottom, as the text gave them: intersections of clips
  simpler, but the canvas, `ImagePart` and the camera changed.
- **Both shapes**, each crate keeping its own: nothing to change, but two ways to say one thing.

Texts:

- **Glyphs copied into the display list**: no shared layout, but the glyphs of every text copied at
  every frame.

The atlas:

- **etagere's `BucketedAtlasAllocator`**: faster, but room freed only when every glyph of a bucket
  leaves, where an interface's glyphs come and go one by one.
- **Pages of 2 048 texels**: fewer pages, but 4 MiB of coverage, or 16 of colors, made for a few
  glyphs.
- **Whole pages let go**, rather than glyphs: simpler, but the glyphs still in use rasterized again.

The font, from the same images:

- **Noto Sans**, ADR-0133's: the most writing systems in one design, the fallbacks of the editor
  alike, but a look the owner liked less.
- **Fira Sans**: narrower, made for interfaces. **Open Sans**: Noto Sans's look, wider.

Hinting:

- **Hinted glyphs**: stems a little sharper at scale 1, as Windows draws them, but in the fonts
  that carry hinting's instructions alone, and shapes that change with the size.

## Consequences

- `slime-ui` gains `display`, `Rect` at its root, `TextLayout` that can be cloned and shown with
  `Debug`, and a `TextCache` that gives `Arc<TextLayout>`; it depends on `slime-color`.
- A new crate, `slime-ui-render`, depends on `slime-ui`, `slime-render`, `slime-gfx` and etagere,
  which enters the build, under Apache-2.0 or MIT, bringing euclid, which egui already builds.
- The example `interface` draws its interface from a display list, with a sentence of each writing
  system, its fonts and texts built into the program, so that `interface.exe` runs from the
  Windows artifact.
- The editor's and the overlay's font is chosen, not yet in the repository.
