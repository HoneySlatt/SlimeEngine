# slime-ui

SlimeEngine's UI framework, for the editor and for the interface of games
(`docs/adr/0132-the-engines-ui-framework.md`). It runs without a GPU or a window: what it draws
leaves it as data. Today it holds its text (`docs/adr/0136-the-interfaces-text.md`) and its display
list (`docs/adr/0139-text-drawn-and-the-display-list.md`); the tree of elements, their styles and
their layout come with the next milestones of Stage 6b (`docs/PLAN.md`).

## Role

- `text::Fonts` holds the fonts that text is laid out with: those registered from their bytes,
  named by their families in a style's stack, and the system's where a program asks for them, as
  the editor will, the font found for a writing system named for it from then on. A game draws
  with the fonts it ships, never the system's.
- `Fonts::lay_out` lays a text out in a `text::TextStyle` and a box, `text::TextBounds`: shaped,
  put in the order of its directions, broken into lines, aligned, and cut with an ellipsis when
  the box holds fewer lines than the text. The `text::TextLayout` gives its lines and its glyph
  runs, the cluster under a point, the position nearest it, carets and the rectangles of a
  selection.
- `text::GlyphRasterizer` draws a glyph's image, coverage or color, from the `text::GlyphKey` that
  a placed glyph carries: its face, size and position between two pixels.
- `text::TextCache` keeps laid-out text from one frame to the next within a budget of memory, and
  gives it shared, `Arc<TextLayout>`, so that a display list holds it without copying it.
- `display::DisplayList` is what an interface draws in a frame, as owned data: boxes, the shadows
  that boxes cast, texts laid out and parts of images, which it names by keys
  (`display::ImageKey`) that the program drawing it resolves, each with its clip, its transform
  and its layer, the identity and the root's until Stage 7. `slime-ui-render` draws it through the
  renderer's canvas.
- `Rect` is the rectangle the framework speaks in, by its top left corner and its size, as the
  renderer's canvas gives its own.

## Depends on

parley, fontique and swash, whose types stay inside the crate
(`docs/adr/0132-the-engines-ui-framework.md`); icu_properties, for the writing system of a
character; `slime-color`, the display list's colors; thiserror and tracing. No other engine crate:
text is laid out and rasterized on the CPU, and `slime-ui-render` draws the display list.

## Must not

- Use a GPU, a window or the platform's events: the framework runs headless, in tests and on
  servers, and what it draws leaves it as data.
- Find a font by fallback where a stack can name it: the system's fallback costs up to a hundred
  times more than a font named (`docs/adr/0133-font-files-under-the-open-font-license.md`).
- Show parley's, fontique's or swash's types in its API.
