# canvas-prototype

A prototype thrown away once measured: it lives on the branch `m6d4a-prototype` only
([ADR-0242](../../docs/adr/0242-the-canvas-moved-in-three-steps-tokens-read-when-drawn-later.md),
milestone M6d.4a). It chooses how the canvas hands its items to its shaders on a path that WebGL2
takes, since WebGL2 has no storage buffer.

It draws the two screens of the benchmark `render.canvas` (`benches/src/render.rs`: an editor's
screen, some 19 500 items, and a game's menu, some 4 200) with three layouts of the canvas's
items, compares their images with today's, times their frames, and counts their pipelines against
WebGL2's limits. It does the same for five effects (below): a transform per item, a group drawn
apart, a blur of what lies behind, a path, and one element animated over a still screen that
grows. It depends on wgpu, pollster and png alone: the canvas's items and the two screens
are copied from `slime-render` and the benchmarks, not linked.

## The three layouts

An *item* is what the canvas draws: a box, a shadow, a gradient, or a part of an image, either a
coverage (one channel, drawn in a colour, as glyphs are) or colours (an icon, a picture).

- **S, storage** (`storage.rs`, `storage.wgsl`): today's canvas, the control. Each item is nine
  `vec4`s (144 bytes) in a storage buffer; the vertex shader builds the quad from the vertex's
  index, and the fragment shader reads the item again by its index. `storage.wgsl` is
  `crates/slime-render/src/canvas.wgsl` unchanged. One pipeline; one draw per run of items that
  read one image.
- **I, instances** (`instances.rs`, `instances.wgsl`): one pipeline per kind of item, each with its
  own instance format in a vertex buffer, six vertices an instance; the fragment shader receives
  what its kind needs as flat values from the vertex shader. Items keep their order: one draw per
  run of consecutive items of one kind and one image. `instances.rs` gives each format (glyphs: 48
  bytes).
- **T, texture** (`texture.rs`, `texture.wgsl`): S's items, the same bytes, written each frame into
  an `Rgba32Uint` texture 2048 texels wide, read by `textureLoad` in both shaders. One pipeline,
  one draw per run of one image, as S. The texture grows by powers of two in height, up to 2048
  rows: 466 033 items.

Every layout draws into an `Rgba8Unorm` target, colours multiplied by their alpha and sRGB-encoded
on the CPU, images' texels encoded in the shader, so that colours mix as sRGB values
([ADR-0137](../../docs/adr/0137-the-canvas.md)): GL cannot view a target in another format. S also
draws once into an `Rgba8UnormSrgb` target through an `Rgba8Unorm` view, as `slime-render` does
today, and its bytes must equal those of its `Rgba8Unorm` image.

A third screen, `kinds`, holds a few items of every kind, gradients included, which neither screen
of the benchmark draws, each clipped between two pixels and translucent; it is compared, not timed.

## The effects

`--effect NAME` runs an effect in place of the screens: `transform`, `group`, `blur`, `path`,
`grow`, or `all`; it may be given more than once. The first four draw with the *effects'
pipelines*, one for each layout: `storage_fx`, `instances_fx`, `texture_fx` (`.rs` and `.wgsl`),
whose shaders share `fx_common.wgsl`. They draw as the screens' pipelines do, plus a transform per
item and three kinds of item: GLASS, LAYER and PATH. An item whose transform is the identity
draws exactly as the screens' pipelines draw it: the transform's run checks this on the game's
menu (`fx-game` against S's `game`).

- **`transform`, a transform per item.** The game's menu, each of its four panels and their
  contents turned by a few degrees about the panel's centre, one card scaled by 1.25
  (`scenes.rs`). Each item names a 2D affine transform: S, an index into a storage buffer of
  transforms (32 bytes each); T, an index into the transforms written after the items in its
  texture (2 texels each); I, the transform's six values in the instance. A shape's signed
  distance is computed in the item's own space and multiplied by the transform's scale, the
  square root of the determinant of its linear part, which the vertex shader computes: no byte
  sent, exact for turns and even scales; derivatives were not used. The vertex shader grows a
  transformed quad by one pixel of the target, so that the band where an edge covers a pixel in
  part is drawn whatever the turn, and smooths a transformed image's edges as a box's. Clips stay
  in the target's pixels and are tested against the fragment's position, so the CPU no longer cuts
  a transformed quad to its clip (`Canvas::keep_whole`), and I sends an image's clip whole.
- **`group`, a group drawn apart.** The game menu's right panel, its box, rows and text, drawn
  into a texture of its own (`Rgba8Unorm`, drawn into and read, its colours multiplied by their
  alpha and encoded as the target's), then drawn over the screen as one LAYER item faded to 0.6
  and scaled by 0.95 about its centre. Its shadow stays on the screen. Two frames are timed: one
  that draws the group again (two passes), one that reuses its texture (one pass); their images
  must be identical.
- **`blur`, a blur of what lies behind.** The canvas draws into a texture of its own, since a
  window's image cannot be read on GL, and copies it onto a target that allows nothing but being
  drawn into, as a GL surface, at the end. What lies behind (the 16 pictures across the screen,
  bright boxes, rows of text, and the menu's dimming box) is drawn first; then, for each of the
  menu's four panels, the region behind it is blurred (`blur.rs`); then the rest of the menu, its
  panels made GLASS items: rounded boxes filled with their tint over the blurred region, with their
  border. The blur is a separable Gaussian at a lower size, not dual Kawase: the region is halved
  two or three times, each pixel the average of four read as one sample, then blurred across and
  down there, and the glass reads the result through its sampler. Its radius is 24 pixels at
  1080 lines, scaled with the screen, read as a shadow's is, twice the Gaussian's deviation.
  Dual Kawase reads every size twice, down then up, its last pass at half the screen's size, and
  a radius in pixels does not set its reach.
- **`path`, a path.** Twenty filled shapes of quadratic and cubic Bézier curves, stars, circles,
  rings, outlines like glyphs and flowers (`path.rs`), each flattened on the CPU into at most 64
  segments, shared among its curves by the square root of their length times their turning. The
  segments are kept where each layout keeps variable data: S, a storage buffer; T, its texture,
  after the transforms; I, an `Rgba32Float` texture 2048 texels wide, since an instance holds a
  fixed size. The fragment shader reads the path's segments, counts its winding number (non-zero
  rule) and its distance to the nearest, and covers one pixel across the edge. This counts the
  data a path takes through each layout; it is not the choice of how uwui draws vector shapes,
  which stays open for uwui's first stage.
- **`grow`, one element animated over a still screen that grows.** Screens of 1,000, 10,000 and
  100,000 items, an opaque background then rows of a box and glyphs as small as fill the screen,
  one box in the middle moving each frame, drawn with the screens' pipelines. Two ways per layout:
  `all written`, the screen's items made again and all written; `box alone`, only the box's item
  made again and its bytes written (`write_buffer`, or `write_texture` of its texels), the rest
  kept from the frame before. Their images after five moves must be identical.

## Configurations

- `vulkan`: Vulkan, the adapter's limits. S, I and T run.
- `vulkan-webgl2`: Vulkan, WebGL2's limits (`Limits::downlevel_webgl2_defaults()`), no optional
  feature but timestamp queries. I and T run; S fails with wgpu's validation error, printed.
- `gl`: wgpu's GL backend, without a window (EGL's surfaceless platform), WebGL2's limits. I and T
  run; S fails, printed. wgpu takes desktop GL whenever EGL offers it: on llvmpipe, OpenGL 4.6.
- `gl-es`: the same, Mesa made to refuse desktop GL 3.3 and to offer OpenGL ES 3.0 alone, whose
  shaders, GLSL ES 3.00, are WebGL2's. Its rows say `gl-es`. It has no timestamp queries, as WebGL2.

On the configurations with WebGL2's limits, the size of textures is the adapter's
(`Limits::using_resolution`), as a WebGL2 program asks for it so that its drawing buffer may be
larger than 2048 pixels: the frames of 3840 × 2160 need it. T keeps its own texture within 2048 ×
2048, WebGL2's guaranteed size, whatever the device allows.

## Running

Inside `nix develop`, from the repository's root:

```sh
cargo build --release -p canvas-prototype
crates/canvas-prototype/run-config.sh vulkan          # first: writes S's images
crates/canvas-prototype/run-config.sh vulkan-webgl2
crates/canvas-prototype/run-config.sh gl
crates/canvas-prototype/run-config.sh gl-es
```

Options: `--frames N` (frames measured, 30; 0 compares images only), `--warm-up N` (frames drawn
first, 10), `--effect NAME` (above).

```sh
crates/canvas-prototype/run-config.sh vulkan --effect all     # first: writes S's images
crates/canvas-prototype/run-config.sh gl --effect blur --effect path
```

The transform's run compares the effects' pipelines with S's image of the game's menu, which the
screens' run on `vulkan` writes: run it once first.

`run-config.sh` sets the variables that keep each run on Mesa's drivers that draw on the CPU:

- Vulkan: `env -u VK_LOADER_DRIVERS_SELECT -u VK_LOADER_DRIVERS_DISABLE
  VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD"`, lavapipe alone.
- GL: `LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe`; `LD_LIBRARY_PATH` gains glvnd's `libEGL`
  from the flake's nixpkgs, which the default shell lacks and wgpu opens at run time;
  `__EGL_VENDOR_LIBRARY_FILENAMES` names the EGL of the shell's Mesa, lavapipe's own; `WAYLAND_DISPLAY`
  and `DISPLAY` are unset. `gl-es` adds `MESA_GL_VERSION_OVERRIDE=3.2
  MESA_GLES_VERSION_OVERRIDE=3.0`.

The program refuses an adapter that is not llvmpipe (lavapipe names itself so). On a GPU, run the
binary by hand with `--any-adapter`.

The images are written to `target/canvas-prototype/<config>-<layout>-<screen>.png`. The program
exits with 1 if a layout cannot be built where it must be, if an image of I or T differs from S's
by more than 2/255 on a channel, or if S's two targets differ.

## Reading the output

Each run prints, as tab-separated lines:

- `adapter`: its name, backend, type and driver; `device`: the limits it was made with, and whether
  it has timestamp queries.
- **Limits**, one row per pipeline: its vertex attributes and instance stride in bytes; its values
  between the stages (`@location`s of the vertex shader's output, `@builtin(position)` not
  counted, as WebGL2's 15 count them); its bind groups; its sampled textures in the vertex and
  fragment stages; its storage buffers in each; its uniform bytes; and the most items a frame holds
  under WebGL2's limits: none for S, the 256 MiB of a buffer over the stride for I, 2048 × 2048
  texels over 9 for T. wgpu checks every limit when it builds a pipeline on `vulkan-webgl2` and
  `gl`: a pipeline built there fits.
- **Images**, at 1280 × 800: for each layout and screen, the largest difference of a channel from
  S's image on `vulkan`, in steps of 1/255, and the pixels where one differs by more than 1.
- **Times**, at 1280 × 800, 1920 × 1080 and 3840 × 2160, each screen laid out for 1920 × 1080 and
  scaled: the median (p50) and 90th percentile (p90) of the frames measured, in microseconds; each
  frame is waited for before the next.
  - `items`, `draws`, `pipelines`: the items drawn, the draws, the pipelines set in the pass.
  - `bytes`: the bytes written each frame, the items' and the 16 of the frame's uniform.
  - `write`: CPU, the items turned into the layout's bytes and written (`write_buffer`,
    `write_texture`), the runs of draws planned.
  - `encode`: CPU, the pass recorded, finished and submitted.
  - `GPU`: the pass, from timestamp queries; `none` without them.
  - `wall`: from just before the submission to the end of the wait for it.

An effect's run prints the same three parts, as `# effect NAME`:

- **Limits of the effects' pipelines**, as above, each I kind with the screens' format beside it;
  `variable data` says where items, transforms and segments go. The blur's passes and the copy
  read one texture and a uniform of 160 bytes; they take no vertex data.
- **Images**: as above, each effect's image against S's on `vulkan`, written as
  `<config>-<layout>-<effect>.png`. `group reused` and `grow, the box's bytes alone` compare a
  layout's two frames with each other: their bytes must be identical.
- **Times**: as above, with `effect`, `variant` (`group drawn`, `group reused`; `all written`, `box
  alone`), `passes` (the render passes of a frame), and `build` (the growing screen's items made on
  the CPU; 0 elsewhere). `bytes` counts every write of the frame: items, transforms, segments,
  frame uniforms (32 bytes for the effects' pipelines). `GPU` is the sum of the frame's passes.
- **Steps** (`group`, `blur`): each step's GPU time, `passes` the render passes it takes: for the
  blur, `behind` (what lies behind, into the canvas's texture), `glass N blur` (its halvings and
  its two blur passes), `front` (the rest of the canvas), `copy` (the canvas's texture onto the
  target).

On llvmpipe's GL, the columns of the CPU and of the GPU do not mean what they mean on Vulkan.
wgpu's GL backend replays the recorded commands into GL calls at the submission
(`wgpu-hal`'s `gles/queue.rs`), so that `encode` holds the GL driver's own work on the calling
thread. With timestamps (`gl`), wgpu also asks GL for the queries' results there
(`CopyQueryResults`), and the submission then lasts the whole frame: `encode` equals `wall`. And
llvmpipe's timestamps come out a small fraction of `wall`: they do not span the rasterisation. On
GL, `wall` is the measure, and `gl-es`, without queries, gives the CPU without that wait. On
lavapipe, `GPU` agrees with `wall` and is the pass's time.

lavapipe and llvmpipe draw on the CPU: their times rank the layouts for a CPU driver, which reads
every byte of an item for each fragment, not for a GPU. The milestone times them on the RX 6900 XT
too.
