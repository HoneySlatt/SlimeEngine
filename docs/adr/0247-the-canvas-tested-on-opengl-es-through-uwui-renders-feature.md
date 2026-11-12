# 0247. The canvas tested on OpenGL ES through a feature of `uwui-render` alone

- Status: Accepted; amends [0016](0016-gpu-layer-with-wgpu.md) (wgpu's GL backend, for
  `uwui-render`'s tests alone) and
  [0243](0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md) (the store chosen
  from the device's limits and a layout it accepts)
- Date: 2026-11-12

## Context

[ADR-0243](0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md) gives the
canvas two ways of reading its items: a storage buffer, where both stages of a pipeline read one,
and a texture of `Rgba32Uint` elsewhere, as on WebGL2 and OpenGL ES. A device opened on Vulkan with
WebGL2's limits takes the texture, and draws the same bytes as the buffer. It does not run what
only a GL device runs: naga's shaders in GLSL ES 3.00, `texelFetch` of an integer texture, wgpu's
uploads of textures through GL, and buffers mapped on the CPU, as WebGL2 maps them.

[ADR-0016](0016-gpu-layer-with-wgpu.md) leaves OpenGL out of the workspace's wgpu, whose features
are `std`, `parking_lot`, `vulkan`, `dx12` and `wgsl`. M6d.4a's prototype turned on wgpu's `gles`
feature on its own crate, thrown away since
([ADR-0242](0242-the-canvas-moved-in-three-steps-tokens-read-when-drawn-later.md)).

Two facts shape where the store is chosen. A `wgpu::Device` gives its limits and features, but no
downlevel flags; those are the adapter's, which `uwui-render` is not given
([ADR-0244](0244-the-engine-lends-its-device-through-a-guarded-door.md)). And on OpenGL ES, a
device may count storage buffers in the fragment stage alone: its limit is above zero, and wgpu
refuses a storage buffer that the vertex stage reads, a validation error, when the layout is made.

wgpu's GL backend asks EGL for desktop GL 3.3 before OpenGL ES, and takes it wherever it is offered.
Told to offer desktop GL up to 3.2 alone (`MESA_GL_VERSION_OVERRIDE=3.2`), Mesa refuses that 3.3,
and wgpu falls back to OpenGL ES, 3.0 there (`MESA_GLES_VERSION_OVERRIDE=3.0`), the API that WebGL2
is built on. The GL backend reports no view of a texture in another
format (no `VIEW_FORMATS` among its downlevel flags), so it draws into no sRGB target through a view
without its encoding.

## Decision

**A feature of `uwui-render`, `gles`, off by default, turns on wgpu's `gles`.** Nothing else in
the workspace turns it on: the engine, the game, the editor and their tests never build GL, and
`scripts/check.sh` fails if the workspace's tree holds `glow`, the bindings that GL alone brings.
The feature leaves with `uwui-render` at the move.

**A part of `scripts/check.sh`, `gles`, and a job of CI, `check (gles)`,** build `uwui-render`
with the feature, run clippy on it for Linux and Windows, and run its tests at WebGL2's limits, the
canvas reading from a texture, on lavapipe and on Mesa's GL on the CPU (llvmpipe), OpenGL ES 3.0
alone. The tests
refuse any GL context that is not Mesa's on the CPU in OpenGL ES 3.0, so that no run passes on
desktop GL or on a GPU's driver. A test compares GL's images with those that Vulkan or Direct3D 12
draws from a storage buffer, within 1/255 a channel. The development shell names glvnd's loader of
EGL and Mesa's driver of it (`SLIME_GLVND`, `SLIME_MESA_EGL`), which the part hands to wgpu.

**The store is chosen from the device alone.** The canvas takes a storage buffer where its device's
limit of storage buffers per stage is above zero and the device accepts a layout in which both
stages read one, tried in an error scope of its own; a texture otherwise. Its tests on a device
that refuses such a layout while its limit allows it are left to a device that has one.

## Alternatives considered

- **`gles` in the workspace's wgpu line:** one line, but every program built with GL for a need of
  tests, and the engine's choice of adapter
  ([ADR-0016](0016-gpu-layer-with-wgpu.md)) open to a GL adapter.
- **`gles` through a dev-dependency of `uwui-render`:** on in every `cargo test --workspace` and
  `cargo clippy --all-targets`, so that wgpu and naga are built twice in every check.
- **A crate of tests of its own:** a member of the workspace, which turns the feature on for every
  crate in `cargo test --workspace`.
- **No GL:** no crate added, but neither GLSL ES, nor GL's uploads, nor its mapped buffers run
  before the web's trial, and M6d.4b2's sheet asks for images on GL.
- **The shaders checked by naga's GLSL writer alone:** no driver runs them.
- **The store chosen from the adapter's downlevel flags,** as ADR-0243 wrote: the device that
  `uwui-render` is given carries none.

## Consequences

- `Cargo.lock` holds seven crates more, which only the feature builds: `glow`, `khronos-egl`,
  `glutin_wgl_sys`, `gl_generator`, `khronos_api`, `xml-rs` and `wgpu-core-deps-emscripten`, under
  MIT, Apache-2.0 or Zlib ([`dependencies.md`](../dependencies.md)).
- CI runs one job more; its time is in [`measurements.md`](../measurements.md). A local
  `scripts/check.sh` builds `uwui-render`'s tree a second time, with GL.
- Mesa's OpenGL ES on the CPU is not WebGL2: a browser, its limits as a browser enforces them, and
  a GPU's GL driver are the web's trial's. GL on Windows is type-checked by clippy, neither built
  nor run. GL's own storage buffers, which desktop GL and OpenGL ES 3.1 give, run nowhere: the GL
  tests take WebGL2's limits.
- On GL, two tests do not apply and say so: the time of the canvas's pass, which OpenGL ES does
  not time, and the bytes of an sRGB target, which it cannot draw into.
- On GL, the canvas draws into `Rgba8Unorm` and `Bgra8Unorm` targets, and `Rgba16Float` where the
  device draws into floats, not into sRGB ones; its tests take `Rgba8Unorm` there, which holds the bytes of an sRGB target.
