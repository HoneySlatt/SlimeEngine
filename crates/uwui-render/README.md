# uwui-render

uwui's renderer: its *canvas*, which draws boxes, shadows, gradients and images over a target, and
its display lists drawn through it, the glyphs of their texts kept in an atlas
([ADR-0137](../../docs/adr/0137-the-canvas.md),
[ADR-0139](../../docs/adr/0139-text-drawn-and-the-display-list.md)). It draws with `slime-gfx`'s
handles until it draws on wgpu, whose device the engine lends it then
([ADR-0244](../../docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)). The
engine draws through it from `slime-app`'s bridge, `InterfaceRenderer`.

## Role

- Holds the canvas (`canvas`): what an interface draws in a frame, as data alone, boxes with
  rounded corners and borders, their shadows, boxes filled with a gradient between their corners'
  colors, and parts of images, each clipped, in their order; and what draws it over a target of
  one format, after the views, in a pass of its own, its items read by their index from a storage
  buffer by one pipeline and its shader, `canvas.wgsl`. Into an sRGB target, its colors mix as
  their sRGB values do, as design tools mix them.
- Turns a `uwui::display::DisplayList`, data that knows no GPU, into a canvas, and draws it:
  boxes, shadows, gradients and images as they are, each text's glyphs as images of the atlas.
- Keeps the glyphs' atlas: textures, pages of coverage for the glyphs of letters and pages of
  colors for emoji, in which etagere places each glyph's image, rasterized by
  `uwui::text::GlyphRasterizer` the first time a draw asks for it, then kept within a budget of
  memory, the glyphs that no draw asked for the longest leaving first.
- Gives a display list's images, which it names by keys, as the program that draws it resolves
  them: textures, or targets that a view drew, which the program holds.

## Entry points

- `canvas::Canvas::new()` starts what an interface draws in a frame: `draw_box` adds a
  `CanvasBox`, its corners rounded and its border inside its edges, `draw_shadow` the
  `CanvasShadow` that a box casts, `draw_gradient` a `CanvasGradient`, a box without a border
  whose corners' colors mix across it
  ([ADR-0152](../../docs/adr/0152-the-dev-overlay-on-the-engines-interface.md)), and `draw_image`
  an `ImageQuad`, a part (`ImagePart`) of the image that its `ImageId` names: the index of the
  image in the list given when the canvas is drawn. Each item has its clip, and coordinates are the
  target's pixels (`PixelRect`).
- `canvas::CanvasRenderer::new(gpu, format)` makes what draws canvases into targets of `format`,
  its shader, `canvas::SHADER`, compiled; `draw(gpu, &canvas, &images, &target)` draws a canvas
  over a target with its images, each a `CanvasImage`, a texture of colors or of coverage that a
  color tints (`ImageKind`), or a render target that a view drew, and returns its `CanvasStats`,
  its GPU time among them; `set_shader(gpu, source)` replaces its shader while a program that
  holds it runs, keeping the one it had when the new one is refused; `UiRenderer` and the engine's
  bridge do not offer it. A canvas holds at most `MAX_CANVAS_ITEMS` items.
- `UiRenderer::new(gpu, format)`, or `UiRenderer::with_budget(gpu, format, budget)`, makes a
  renderer of display lists into targets of `format`, with its own canvas's renderer, whose atlas
  takes at most `budget` bytes, `GLYPH_ATLAS_BUDGET` by default.
- `UiRenderer::draw(gpu, &fonts, &list, images, &target)` draws a list over a target of the
  renderer's format, `images` giving the image of each key, and returns its `UiStats`: what the
  canvas drew, the glyphs drawn, rasterized and left out, the images left out and the atlas's
  memory. `UiRenderer::draw_canvas(gpu, &canvas, &images, &target)` draws a canvas that a program
  fills itself, with the same canvas's renderer.

The rustdoc holds the contracts: the canvas's and the atlas's figures, what is left out, the errors
and panics.

## Dependencies

`uwui`, whose display list and glyphs it draws; `slime-gfx`, whose handles its canvas draws with
and whose textures hold the atlas; `slime-color`, the colors of the canvas's items, encoded into
sRGB as an sRGB target mixes them; etagere, which places the glyphs in the atlas; tracing.

## Must not

- Lay text out, or hold an interface's tree: `uwui` does, without a GPU.
- Depend on the renderer, `slime-render`, or on an engine crate besides `slime-gfx` and
  `slime-color`; `slime-gfx` leaves it once it draws on wgpu
  ([ADR-0244](../../docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)).
- Reach wgpu until then: it draws through `slime-gfx`'s handles, and the engine's programs draw it
  through `slime-app`'s bridge.
