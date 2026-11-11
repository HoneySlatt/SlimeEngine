# canvas-prototype

A prototype thrown away once measured: it lives on the branch `m6d4a-prototype` only
([ADR-0242](../../docs/adr/0242-the-canvas-moved-in-three-steps-tokens-read-when-drawn-later.md),
milestone M6d.4a). It chooses how the canvas hands its items to its shaders on a path that WebGL2
takes, since WebGL2 has no storage buffer.

It draws the two screens of the benchmark `render.canvas` (`benches/src/render.rs`: an editor's
screen, some 19 500 items, and a game's menu, some 4 200) with three layouts of the canvas's
items, compares their images with today's, times their frames, and counts their pipelines against
WebGL2's limits. It depends on wgpu, pollster and png alone: the canvas's items and the two screens
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
first, 10).

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
