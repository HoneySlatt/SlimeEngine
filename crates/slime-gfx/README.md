# slime-gfx

The GPU layer: the boundary between the engine and wgpu, the library that talks to the graphics
drivers.

## Status

Partial. It opens a GPU device, with or without a window, draws into offscreen images and reads
them back, and draws into a window:

- It draws lists of triangles from vertex and index buffers, or from the vertex index alone (a
  fullscreen triangle); many instances in one draw, each finding its values in a storage buffer
  by its number; and meshes that share a vertex and an index buffer, each from its first vertex.
- Its shaders read uniform and storage buffers, textures, samplers and comparison samplers through
  bind groups, with a depth test, a depth bias and face culling.
- It draws into 8-bit sRGB or linear images, into 16-bit floating-point ones (high dynamic range),
  or into a depth target only.
- It draws into images of four samples per pixel, which a pass resolves into ordinary ones, or
  which later shaders read sample by sample, for multisample anti-aliasing.
- Later passes sample the render and depth targets that earlier ones drew into. Several passes
  can be recorded into one submission.
- It times render passes, each pass of a submission apart, on adapters that have timestamp queries.
- It opens a new device in place of a lost one, with or without a window, for a program that
  creates its GPU objects again.
- It creates textures from the CPU: two-dimensional ones whose levels of detail it computes, or
  whose levels the caller gives, in 8-bit or half-precision float formats, or in blocks of BC7 and
  BC5 compression, which the asset pipeline's cooked textures use, and cube textures.
- It brings what loads read to the GPU through an uploader: textures and ranges of buffers written
  into staging buffers by the threads that load, reused from one upload to the next, and their
  copies submitted by one thread of the uploader's, within a budget of bytes per frame, the lowest
  rank first.

Not there yet: blending and array textures.

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
use slime_color::LinearColor;
use slime_gfx::{
    CullMode, FragmentState, GfxError, Gpu, RenderPassDescriptor,
    RenderPipelineDescriptor, TextureFormat, VertexAttribute, VertexFormat, VertexLayout,
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
    let target = gpu.create_render_target("image", 64, 64, TextureFormat::Rgba8UnormSrgb)?;
    let shader = gpu.create_shader_module("white", SHADER)?;
    let pipeline = gpu.create_render_pipeline(&RenderPipelineDescriptor {
        label: "white triangles",
        shader: &shader,
        vertex_entry_point: "vs_main",
        vertex_layout: Some(VertexLayout {
            stride: 8,
            attributes: &[VertexAttribute {
                location: 0,
                offset: 0,
                format: VertexFormat::Float32x2,
            }],
        }),
        fragment: Some(FragmentState {
            entry_point: "fs_main",
            target_format: Some(target.format()),
        }),
        bind_group_layouts: &[],
        depth: None,
        cull: CullMode::None,
        multisampled: false,
    })?;
    // Three corners as (x, y) pairs of f32, in the GPU's byte order (little-endian).
    let corners: [f32; 6] = [0.0, 0.5, -0.5, -0.5, 0.5, -0.5];
    let bytes: Vec<u8> = corners.iter().flat_map(|value| value.to_le_bytes()).collect();
    let vertices = gpu.create_vertex_buffer("triangle", &bytes)?;

    let black = LinearColor::BLACK;
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(black),
        depth: None,
        timer: None,
        resolve: None,
    };
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

- `Gpu::new()` opens the most capable GPU of the machine, without a window, and
  `Gpu::new_for_window(window, width, height, present_mode)` one that can draw into a window,
  together with the window's `Surface` (ADR-0020). The device has every limit that its adapter
  offers, and an adapter below the engine's floor of limits is refused with `GfxError::BelowFloor`,
  which names the limit (ADR-0114). `Gpu::check()` says whether the device was lost;
  once it is, the methods that return a `Result` return `GfxError::DeviceLost`, which says why
  (`LossKind`; ADR-0022), and so do they when they are the first to find the loss, on any thread,
  waits included: the engine never polls the device, on which wgpu panics (ADR-0103).
  `Gpu::restore` takes a lost device and opens one to replace it,
  and `Surface::restore` does it for a window, whose surface it creates again; nothing else may hold
  the lost device then, since Direct3D 12 would not open the new one on the same adapter;
  `Gpu::lose` loses a device on purpose, for tests and tools (ADR-0075). Clones of a `Gpu` share its
  device, so that threads create GPU objects through it (ADR-0073). `Gpu::can_sample_bc` says
  whether it samples block-compressed textures.
- Every method that creates a GPU object returns a `Result`: a buffer, texture or timer that the
  GPU's memory cannot hold is `GfxError::OutOfMemory`, so that a game can free memory or ask for
  less (ADR-0044); the `SLIME_GPU_MEMORY_BUDGET` environment variable (`MEMORY_BUDGET_ENV_VAR`)
  lowers that memory, to a percentage of what the driver allows. `Gpu::memory_in_use` gives what
  the device's objects take, as wgpu counts it on Vulkan and Direct3D 12.
- The GPU objects, each created by a method of `Gpu` with a label that names it in error messages
  and capture tools, are handles that their clones share (ADR-0017): `RenderTarget` and
  `DepthTarget`, which passes draw into and later passes sample; `Texture`, `Sampler` and
  `ComparisonSampler`; `VertexBuffer`, `IndexBuffer`, `UniformBuffer` and `StorageBuffer`, each of
  one use and copies into it, so that the CPU writes them by ranges (`Gpu::write_vertex_buffer`,
  `write_index_buffer`, `write_storage_buffer`; ADR-0117); `BindGroupLayout` and
  `BindGroup`, the resources a pipeline's shaders read (ADR-0026), whose uniform bindings declare
  the size of their values, so that a pipeline whose shader reads more is refused when it is made
  (ADR-0077); `ShaderModule` and `RenderPipeline`; and `GpuTimer`, which times passes on the GPU.
- `Uploader::new(gpu, bytes_per_frame)` makes the one owner of the copies of what loads bring
  (ADR-0107, ADR-0118): `Uploader::texture` and `Uploader::buffers` write a texture's levels or
  ranges of vertex and index buffers into staging buffers on the calling thread, and call back once
  the uploader's thread has submitted their copies, those of the lowest `UploadRank` first, a rank
  that can change while they wait (`docs/adr/0128-asset-requests.md`); `Uploader::frame` renews
  the budget, as a renderer does after each frame. `Texture::bytes` gives what a texture's levels
  take of the GPU's memory.
- `Gpu::render` and `Gpu::render_depth` run one pass, through a `RenderPass`, and `Gpu::submit`
  records several into one submission (ADR-0033). `RenderPass::draw_indexed` draws a range of
  instances, from a base vertex (ADR-0117). `RenderPass::set_viewport` draws into part of a
  pass's targets, so that one pass can draw several views side by side, as a shadow atlas holds
  them. `Gpu::read_render_target` and `Gpu::read_float_render_target` copy a target back to the
  CPU, a window's image too where the window's system allows it (`GfxError::NotReadable` otherwise,
  ADR-0122): a tool captures what the player sees so.
- `Gpu::create_multisampled_render_target` and `Gpu::create_multisampled_depth_target` create
  targets of `MULTISAMPLE_COUNT` samples per pixel, which pipelines built `multisampled` draw into;
  `RenderPassDescriptor::resolve` names the target of one sample that the pass averages them into
  as it ends (ADR-0059); a later pass's shader can also read them sample by sample, at a
  `BindingType::MultisampledTexture` binding (ADR-0060). `RenderTarget::is_multisampled` tells them
  from targets of one sample.
- `Surface::acquire` gives the `SurfaceFrame` to draw a window's next image into, and
  `SurfaceFrame::present` shows it, as its `PresentMode` says (ADR-0021). The surface releases the
  last image when it is dropped or restored, so that an image that a panic or a lost device left
  held does not crash the program (ADR-0103). Applications built on `slime-app` do not call these:
  the engine does.
- `GfxError`: why the GPU could not be opened or used.

The documentation of each item holds its contracts: its formats, limits, errors and panics, and
how it behaves on each graphics API.

## Dependencies

- `wgpu`: access to the GPU through Vulkan and Direct3D 12, and the WGSL shader compiler.
- `pollster`: waits for wgpu's requests, which are futures.
- `slime-color`: `LinearColor`, the color a render pass clears its target to, and the sRGB transfer
  function of the mipmaps it computes.
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
  tooling and the engine's own tests.
