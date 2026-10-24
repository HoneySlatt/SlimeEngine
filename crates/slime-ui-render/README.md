# slime-ui-render

The bridge between the engine's interface and its renderer: it draws a display list of `slime-ui`
through `slime-render`'s canvas, the glyphs of its texts kept in an atlas
([ADR-0132](../../docs/adr/0132-the-engines-ui-framework.md),
[ADR-0139](../../docs/adr/0139-text-drawn-and-the-display-list.md)). It is to the interface what
`slime-world-render` is to the world.

## Role

- Turns a `slime_ui::display::DisplayList`, data that knows no GPU, into a `slime_render::Canvas`,
  and draws it over a target with `Renderer::draw_canvas`: boxes, shadows and images as they are,
  each text's glyphs as images of the atlas.
- Keeps the glyphs' atlas: textures, pages of coverage for the glyphs of letters and pages of
  colors for emoji, in which etagere places each glyph's image, rasterized by
  `slime_ui::text::GlyphRasterizer` the first time a draw asks for it, then kept within a budget of
  memory, the glyphs that no draw asked for the longest leaving first.
- Gives a display list's images, which it names by keys, as the program that draws it resolves
  them: textures, or targets that a view drew, which the program holds.

## Entry points

- `UiRenderer::new(budget)`, or `UiRenderer::default()`, makes a renderer of display lists whose
  atlas takes at most `budget` bytes, `GLYPH_ATLAS_BUDGET` by default.
- `UiRenderer::draw(gpu, &mut renderer, &fonts, &list, images, &target)` draws a list over a target
  of the renderer's format, `images` giving the image of each key, and returns its `UiStats`: what
  the canvas drew, the glyphs drawn, rasterized and left out, the images left out and the atlas's
  memory.

The rustdoc holds the contracts: the atlas's figures, what is left out, the errors and panics.

## Dependencies

`slime-ui`, whose display list and glyphs it draws; `slime-render`, whose canvas draws them;
`slime-gfx`, whose textures hold the atlas; etagere, which places the glyphs in the
atlas; tracing.

## Must not

- Lay text out, or hold an interface's tree: `slime-ui` does, without a GPU.
- Reach wgpu, or draw otherwise than through the canvas: it is a client of the renderer's public
  API, as `slime-world-render` is.
