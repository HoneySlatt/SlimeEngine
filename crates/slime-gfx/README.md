# slime-gfx

The GPU layer: the boundary between the engine and wgpu, the library that talks to the graphics
drivers.

## Status

Partial. It opens a GPU device, with or without a window, draws into offscreen images and reads
them back, and draws into a window:

- It draws lists of triangles from vertex and index buffers, or from the vertex index alone (a
  fullscreen triangle).
- Its shaders read uniform buffers, textures, samplers and comparison samplers through bind
  groups, with a depth test, a depth bias and face culling.
- It draws into 8-bit sRGB or linear images, into 16-bit floating-point ones (high dynamic range),
  or into a depth target only.
- Later passes sample the render and depth targets that earlier ones drew into. Several passes
  can be recorded into one submission.
- It times render passes, each pass of a submission apart, on adapters that have timestamp queries.
- It creates textures from the CPU: two-dimensional ones whose levels of detail it computes, or
  whose levels the caller gives, in 8-bit or half-precision float formats, and cube textures.

Not there yet: blending, multisampling, and array textures.

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
    CullMode, FragmentState, GfxError, Gpu, LinearColor, RenderPassDescriptor,
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
    })?;
    // Three corners as (x, y) pairs of f32, in the GPU's byte order (little-endian).
    let corners: [f32; 6] = [0.0, 0.5, -0.5, -0.5, 0.5, -0.5];
    let bytes: Vec<u8> = corners.iter().flat_map(|value| value.to_le_bytes()).collect();
    let vertices = gpu.create_vertex_buffer("triangle", &bytes)?;

    let black = LinearColor::BLACK;
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

- `Gpu::new()` opens the most capable GPU of the machine, without a window, and
  `Gpu::new_for_window(window, width, height, present_mode)` one that can draw into a window,
  together with the window's `Surface` (ADR-0020). `Gpu::check()` says whether the device was lost;
  once it is, the methods that return a `Result` return `GfxError::DeviceLost` (ADR-0022).
- Every method that creates a GPU object returns a `Result`: a buffer, texture or timer that the
  GPU's memory cannot hold is `GfxError::OutOfMemory`, so that a game can free memory or ask for
  less (ADR-0044).
- The GPU objects, each created by a method of `Gpu` with a label that names it in error messages
  and capture tools, are handles that their clones share (ADR-0017): `RenderTarget` and
  `DepthTarget`, which passes draw into and later passes sample; `Texture`, `Sampler` and
  `ComparisonSampler`; `VertexBuffer`, `IndexBuffer` and `UniformBuffer`; `BindGroupLayout` and
  `BindGroup`, the resources a pipeline's shaders read (ADR-0026); `ShaderModule` and
  `RenderPipeline`; and `GpuTimer`, which times passes on the GPU.
- `Gpu::render` and `Gpu::render_depth` run one pass, through a `RenderPass`, and `Gpu::submit`
  records several into one submission (ADR-0033). `Gpu::read_render_target` and
  `Gpu::read_float_render_target` copy a target back to the CPU.
- `Surface::acquire` gives the `SurfaceFrame` to draw a window's next image into, and
  `SurfaceFrame::present` shows it, as its `PresentMode` says (ADR-0021). Applications built on
  `slime-app` do not call these: the engine does.
- `LinearColor`: the color type of every API of the engine (ADR-0045).
- `GfxError`: why the GPU could not be opened or used.

The documentation of each item holds its contracts: its formats, limits, errors and panics, and
how it behaves on each graphics API.

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
  tooling and the engine's own tests.
