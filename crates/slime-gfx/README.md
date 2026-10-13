# slime-gfx

The GPU layer: the boundary between the engine and wgpu, the library that talks to the graphics
drivers.

## Status

Partial. It opens a GPU device, with or without a window, draws into offscreen images and reads
them back, and draws into a window. It draws lists of triangles from vertex and index buffers,
reading uniform buffers, textures and samplers through bind groups, with a depth test and face
culling, into 8-bit sRGB or linear images, and times its render passes on adapters that have
timestamp queries. Blending is not there yet.

## Role

- Owns every use of wgpu. Its public API uses engine-owned types only; no wgpu type leaves this
  crate (`docs/adr/0016-gpu-layer-with-wgpu.md`).
- Opens the GPU with the graphics API of each platform: Vulkan on Linux, Direct3D 12 on Windows.
- Works without a window: offscreen rendering and its tests need no windowing system.
- Draws into a window given as `raw-window-handle` traits, through the window's *surface*: the
  images the window shows, which the GPU draws into and then *presents*.

## Entry points

Drawing a triangle into an offscreen image and reading it back:

```rust
use slime_gfx::{
    Color, CullMode, GfxError, Gpu, RenderPassDescriptor, RenderPipelineDescriptor, TextureFormat,
    VertexAttribute, VertexFormat, VertexLayout,
};

const SHADER: &str = "
@vertex
fn vs_main(@location(0) position: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(position, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
";

fn main() -> Result<(), GfxError> {
    let gpu = Gpu::new()?;
    let target = gpu.create_render_target("image", 64, 64, TextureFormat::Rgba8UnormSrgb);
    let shader = gpu.create_shader_module("white", SHADER)?;
    let pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
        label: "white triangles",
        shader: &shader,
        vertex_entry_point: "vs_main",
        fragment_entry_point: "fs_main",
        vertex_layout: VertexLayout {
            stride: 8,
            attributes: &[VertexAttribute {
                location: 0,
                offset: 0,
                format: VertexFormat::Float32x2,
            }],
        },
        bind_group_layouts: &[],
        depth: None,
        cull: CullMode::None,
        target_format: target.format(),
    })?;
    // Three corners as (x, y) pairs of f32, in the GPU's byte order (little-endian).
    let corners: [f32; 6] = [0.0, 0.5, -0.5, -0.5, 0.5, -0.5];
    let bytes: Vec<u8> = corners.iter().flat_map(|value| value.to_le_bytes()).collect();
    let vertices = gpu.create_vertex_buffer("triangle", &bytes);

    let black = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    let pass = RenderPassDescriptor { target: &target, clear: Some(black), depth: None, timer: None };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, &vertices);
        pass.draw(0..3);
    });
    let image = gpu.read_render_target(&target)?;
    assert_eq!(image.pixels.len(), 64 * 64 * 4);
    Ok(())
}
```

- `Gpu::new()` opens the most capable adapter of the machine and its device, and logs the adapter.
  The `WGPU_BACKEND` environment variable can choose another graphics API, and `VK_DRIVER_FILES`
  can restrict Vulkan to one driver, such as lavapipe.
- `Gpu::adapter_name()` names the adapter in use; `Gpu::wait_idle()` blocks until the GPU has
  finished its work; `Gpu::check()` returns `GfxError::DeviceLost` once the device is lost (the
  driver crashed or was reset, the GPU was removed, or it ran out of memory): the methods that
  return a `Result` then return that error, and the others do nothing
  (`docs/adr/0022-losing-the-gpu-device.md`). A loss that a wait for the GPU is the first to find
  panics inside wgpu, which treats it as fatal.
- GPU objects, each created by a method of `Gpu` with a label that names it in error messages and
  GPU capture tools (`docs/adr/0017-gpu-resource-types.md`):
  - `RenderTarget`: an image the GPU draws into (`create_render_target`), in one of the
    `TextureFormat`s: `Rgba8UnormSrgb` or `Bgra8UnormSrgb`, the same 8-bit sRGB colors in two byte
    orders, or `Rgba8Unorm`, linear.
  - `Texture`: a two-dimensional image that shaders sample (`create_texture`, from its pixels in
    the byte order of its `TextureFormat`; sRGB formats are decoded to linear values when read,
    `Rgba8Unorm` is read as it is), with mipmaps computed from its pixels when it is created, in
    linear light for the sRGB formats; and `Sampler` (`create_sampler`, from a
    `SamplerDescriptor`): how it is read, with nearest or linear `Filter`s, between the pixels of
    the level of detail that fits and between two levels, and the `AddressMode` beyond its edges
    (repeat, mirror, clamp). Both are bound through bind groups.
  - `VertexBuffer`: vertex data, given as bytes (`create_vertex_buffer`): at least one byte, at
    most 256 MiB with the default limits.
  - `IndexBuffer`: 32-bit indices into a vertex buffer (`create_index_buffer`), for draws whose
    triangles share vertices; it knows its `count`.
  - `UniformBuffer`: values that a whole draw reads, given as bytes laid out as the shader's
    uniform declares (`create_uniform_buffer`: at least 4 bytes, in multiples of 4), and written
    again with `Gpu::write_uniform_buffer`, always with the same size.
  - `BindGroupLayout` and `BindGroup`: the resources a pipeline's shaders read, bound together at
    one `@group` index (`docs/adr/0026-resource-binding-and-depth.md`). A layout
    (`create_bind_group_layout`) lists its bindings as `BindGroupLayoutEntry`s: a binding number,
    the `ShaderStages` that read it and a `BindingType` (a uniform buffer, a texture or a
    sampler); a group
    (`create_bind_group`) gives the `BindingResource` of each binding of a layout: a whole uniform
    buffer, or a range of one whose start is given each time the group is bound (a *dynamic
    offset*, a multiple of `Gpu::uniform_offset_alignment`), so that one buffer holds the values
    of many draws; a texture; or a sampler.
  - `DepthTarget`: a texture holding the depth of what was drawn (`create_depth_target`), at the
    size of the render targets it is used with, for depth testing. It cannot be read back.
  - `GpuTimer`: measures how long render passes take on the GPU (`create_timer`), on adapters
    that support timestamp queries (`Gpu::can_time`; `None` otherwise). A pass that names it in
    its descriptor is timed, and `GpuTimer::latest` gives the time of the last timed pass that
    has come back, a few frames later, without waiting for the GPU. At most four timed passes can
    be in flight: while all four wait for their times, further passes go untimed.
  - `ShaderModule`: WGSL source compiled by wgpu (`create_shader_module`).
  - `RenderPipeline`: a shader's entry points, the layout of its vertices, the layouts of the bind
    groups its shaders read, its depth test (`DepthState`), its culling (`CullMode`) and the format
    of the target it draws into (`create_render_pipeline`, from a `RenderPipelineDescriptor`). It
    draws lists of triangles, without blending. A binding that the shaders declare and that no
    layout holds is rejected.

  Except the timer, they are cheap to clone, clones share the GPU object, and it is freed when
  the last one is dropped. Each belongs to the `Gpu` that created it: using it with another `Gpu`
  is a bug, which panics inside wgpu.
- `Gpu::render(&RenderPassDescriptor { target, clear, depth, timer }, record)` runs one render
  pass: it clears the target to a color, and the depth target to the farthest depth, or keeps
  what both hold when `clear` is `None`; lets `record` draw through a `RenderPass`
  (`set_pipeline`, `set_bind_group` with the dynamic offsets of the group, `set_vertex_buffer`,
  `set_index_buffer`, `draw`, `draw_indexed`); and sends the work to the GPU, timed by `timer`
  when there is one. A pass with a depth target takes pipelines with a depth test, and one without
  takes pipelines without. The pass is named after the target's label in capture tools, and
  `RenderPass::debug_group(label, record)` names a group of commands inside it. Labels and groups
  reach the driver in debug builds, or with `WGPU_DEBUG=1`.
- `Gpu::read_render_target(target)` waits for the GPU and copies a target into an `Image`: RGBA
  bytes, rows from top to bottom, whatever the byte order of the target's format.
- `Gpu::new_for_window(window, width, height, present_mode)` opens the most capable GPU that can
  draw into a window, and returns it with the window's `Surface`
  (`docs/adr/0020-rendering-in-the-main-loop.md`):
  - `Surface::format()` is the format of the window's images, the first 8-bit sRGB format the
    window offers; pipelines that draw into the window are created for it.
  - `Surface::resize(width, height)` follows the window's size, limited to what the device accepts
    (8192 pixels in each dimension with the default limits), with a warning when the window is
    larger.
  - `Surface::acquire(label)` returns the `SurfaceFrame` to draw the next frame into, its target
    named `label`, or `None` when the window cannot show a new image (a size of zero, an image not
    ready in time, an occluded surface, a lost surface, which is created again). Draw into
    `SurfaceFrame::target()` with `Gpu::render`, then call `SurfaceFrame::present()`. Window images
    cannot be read back.
  - `Surface::check()` returns `GfxError::SurfaceLost` once the surface is lost for good: it could
    not be created again, or was lost again each time it was; `acquire` then returns `None` without
    logging, and a run built on `slime-app` stops (`docs/adr/0025-giving-up-a-lost-surface.md`).

  `PresentMode` says how the window shows the images (`docs/adr/0021-frame-pacing.md`): with
  `Mailbox`, the default, each image replaces the one waiting to be shown and presenting never
  waits; windows that do not offer it use `Fifo`, which shows every image, one per refresh, and
  makes `acquire` or `present` wait, up to one second per frame on compositors that rarely show a
  hidden window. Applications built on `slime-app` do not call these: the engine does.
- `GfxError` says why the GPU could not be opened or used. A rejected shader or pipeline is
  returned as `GfxError::Shader` or `GfxError::Pipeline`, with wgpu's message, which for WGSL
  gives the line and column of the error. Other misuses of wgpu are bugs and panic, except once
  the device is lost: errors then only repeat that it is gone, and are logged at the `debug`
  level.

## Dependencies

- `wgpu`: access to the GPU through Vulkan and Direct3D 12, and the WGSL shader compiler.
- `pollster`: waits for wgpu's requests, which are futures.
- `raw-window-handle`: the standard traits through which a window, created by another crate, is
  drawn into; with its `alloc` feature, for the implementations on `Arc`, in which the surface
  keeps the window to create itself again.
- `thiserror`: derives `GfxError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## The `wgpu` feature

For the engine's tooling only (`docs/adr/0031-dev-overlay-with-egui.md`): it re-exports wgpu
and gives the wgpu objects behind the engine's types (`Gpu::wgpu_device`, `Gpu::wgpu_queue`,
`RenderPass::wgpu_pass`, `TextureFormat::wgpu_format`), so that `slime-devui` draws egui with
egui's own wgpu renderer. Games must not enable it.

## Must not

- Depend on `slime-platform` or any windowing code: windows reach it as `raw-window-handle`
  traits, so that offscreen rendering works without a windowing system.
- Expose wgpu's types in its public API, except through the `wgpu` feature, to the engine's
  tooling.
