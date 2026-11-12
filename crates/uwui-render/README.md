# uwui-render

uwui's renderer: its *canvas*, which draws boxes, shadows, gradients and images over a target, and
its display lists drawn through it, the glyphs of their texts kept in an atlas
([ADR-0137](../../docs/adr/0137-the-canvas.md),
[ADR-0139](../../docs/adr/0139-text-drawn-and-the-display-list.md)). It draws with wgpu: its host
gives it a device, its queue and an encoder that it records its passes into and that the host
submits, as egui-wgpu takes them
([ADR-0244](../../docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)). The
engine lends it its device from `slime-app`'s bridge, `InterfaceRenderer`, through `slime-gfx`'s
guarded door.

## Role

- Holds the canvas (`canvas`): what an interface draws in a frame, as data alone, boxes with
  rounded corners and borders, their shadows, boxes filled with a gradient between their corners'
  colors, and parts of images, each clipped, in their order; and what draws it over a target of
  one format, after the views, in a pass of its own recorded into its host's encoder, its items
  read by their index from a storage buffer by one pipeline and its shader, `canvas.wgsl`, the
  pass timed where the device has timestamps. Into an sRGB target, its colors mix as their sRGB
  values do, as design tools mix them, through a view of the target without its encoding, which
  the target must allow.
- Turns a `uwui::display::DisplayList`, data that knows no GPU, into a canvas, and draws it:
  boxes, shadows, gradients and images as they are, each text's glyphs as images of the atlas.
- Keeps the glyphs' atlas: textures, pages of coverage for the glyphs of letters and pages of
  colors for emoji, in which etagere places each glyph's image, rasterized by
  `uwui::text::GlyphRasterizer` the first time a draw asks for it, then kept within a budget of
  memory, the glyphs that no draw asked for the longest leaving first.
- Gives a display list's images, which it names by keys, as the program that draws it resolves
  them: views of textures, or of targets that a view drew, which the program holds.
- Checks what it creates in error scopes of its own: memory that runs out for its buffers, textures
  and queries is its `Error`, and so is a shader or a pipeline refused, the pipeline it had kept;
  wgpu gives the device up when memory runs out for its other objects, which its host reports. It sets no handler nor callback on
  the device, polls nothing, and keeps neither the device, its queue nor a view of a target from
  one draw to the next.

## Entry points

- `canvas::Canvas::new()` starts what an interface draws in a frame: `draw_box` adds a
  `CanvasBox`, its corners rounded and its border inside its edges, `draw_shadow` the
  `CanvasShadow` that a box casts, `draw_gradient` a `CanvasGradient`, a box without a border
  whose corners' colors mix across it
  ([ADR-0152](../../docs/adr/0152-the-dev-overlay-on-the-engines-interface.md)), and `draw_image`
  an `ImageQuad`, a part (`ImagePart`) of the image that its `ImageId` names: the index of the
  image in the list given when the canvas is drawn. Each item has its clip, and coordinates are the
  target's pixels (`PixelRect`).
- `DrawContext { device, queue, encoder }` is what a draw records with; the host submits the
  encoder before the same renderer draws again. `Error` is what goes wrong: memory that runs out,
  a shader or a pipeline refused.
- `canvas::CanvasRenderer::new(&device, &queue, format)` makes what draws canvases into targets of
  a `wgpu::TextureFormat`, its shader, `canvas::SHADER`, compiled; `draw(&mut context, &canvas,
  &images, &target)` records a canvas over a `wgpu::Texture` with its images, each a
  `wgpu::TextureView` of colors or of coverage that a color tints (`ImageKind`), a texture's or a
  target's that a view drew, and returns its `CanvasStats`, its GPU time among them;
  `set_shader(&device, source)` replaces its shader while a program that holds it runs, keeping
  the one it had when the new one is refused; `UiRenderer` and the engine's bridge do not offer
  it. A canvas holds at most `MAX_CANVAS_ITEMS` items.
- `UiRenderer::new(&device, &queue, format)`, or `UiRenderer::with_budget(&device, &queue,
  format, budget)`, makes a renderer of display lists into targets of `format`, with its own
  canvas's renderer, whose atlas takes at most `budget` bytes, `GLYPH_ATLAS_BUDGET` by default.
- `UiRenderer::draw(&mut context, &fonts, &list, images, &target)` records a list over a target
  of the renderer's format, `images` giving the view of each key, and returns its `UiStats`: what
  the canvas drew, the glyphs drawn, rasterized and left out, the images left out and the atlas's
  memory. `UiRenderer::draw_canvas(&mut context, &canvas, &images, &target)` records a canvas that
  a program fills itself, with the same canvas's renderer.

## A lost device

wgpu cannot be asked whether a device is lost, and `uwui-render` sets no callback to learn it: its
host does. On a lost device, wgpu makes invalid objects without a word, and drops the errors of the
loss. `uwui-render` uses none of wgpu's helpers that panic then (`create_buffer_init`, `poll`), and
fills the buffer it grows through its mapping, which an invalid buffer refuses: a renderer made
before the loss draws nothing and raises no error. wgpu still raises errors of validation for what
a host makes after the loss: a host that opens its own device, rather than the engine's door,
ignores them once the device is lost, as `tests/common/mod.rs` does.

## Tests

`tests/canvas.rs` and `tests/drawing.rs` draw on a bare device that `tests/common/mod.rs` opens, as
a host that is not the engine does, its requests read with a waker that does nothing: the
canvas's shapes, a display list's texts in every writing system, a tree of elements, and a lost
device on which nothing is drawn. The engine's bridge draws through the door in `slime-app`'s
tests.

The rustdoc holds the contracts: the canvas's and the atlas's figures, what is left out, the errors
and panics.

## Dependencies

`uwui`, whose display list and glyphs it draws; wgpu, the workspace's version and features, the
GPU it draws with and whose textures hold the atlas; `slime-color`, the colors of the canvas's
items, encoded into sRGB as an sRGB target mixes them; etagere, which places the glyphs in the
atlas; thiserror, which derives its errors; tracing.

## Must not

- Lay text out, or hold an interface's tree: `uwui` does, without a GPU.
- Depend on an engine crate besides `slime-color`, which leaves with it; `scripts/check.sh`
  checks it ([ADR-0244](../../docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md)).
- Set a callback or a handler on the device it is given, poll it, submit work, or keep the device,
  its queue or a view of a target from one draw to the next.
