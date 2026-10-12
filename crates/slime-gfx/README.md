# slime-gfx

The GPU layer: the boundary between the engine and wgpu, the library that talks to the graphics
drivers.

## Status

Partial. It opens a GPU device without a window, draws into offscreen images and reads them back.
Drawing into a window arrives in the next milestones of Stage 2 (see `docs/PLAN.md`).

## Role

- Owns every use of wgpu. Its public API uses engine-owned types only; no wgpu type leaves this
  crate (`docs/adr/0016-gpu-layer-with-wgpu.md`).
- Opens the GPU with the graphics API of each platform: Vulkan on Linux, Direct3D 12 on Windows.
- Works without a window: offscreen rendering and its tests need no windowing system.

## Entry points

Drawing a triangle into an offscreen image and reading it back:

```rust
use slime_gfx::{
    Color, GfxError, Gpu, RenderPipelineDescriptor, TextureFormat, VertexAttribute, VertexFormat,
    VertexLayout,
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
        target_format: target.format(),
    })?;
    // Three corners as (x, y) pairs of f32, in the GPU's byte order (little-endian).
    let corners: [f32; 6] = [0.0, 0.5, -0.5, -0.5, 0.5, -0.5];
    let bytes: Vec<u8> = corners.iter().flat_map(|value| value.to_le_bytes()).collect();
    let vertices = gpu.create_vertex_buffer("triangle", &bytes);

    let black = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    gpu.render(&target, black, |pass| {
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
  finished its work.
- GPU objects, each created by a method of `Gpu` with a label that names it in error messages and
  GPU capture tools (`docs/adr/0017-gpu-resource-types.md`):
  - `RenderTarget`: an image the GPU draws into, which can be read back
    (`create_render_target`). Its only format for now is `TextureFormat::Rgba8UnormSrgb`.
  - `VertexBuffer`: vertex data, given as bytes (`create_vertex_buffer`).
  - `ShaderModule`: WGSL source compiled by wgpu (`create_shader_module`).
  - `RenderPipeline`: a shader's entry points, the layout of its vertices and the format of the
    target it draws into (`create_render_pipeline`, from a `RenderPipelineDescriptor`). It draws
    lists of triangles, without depth, culling or blending.

  They are cheap to clone, clones share the GPU object, and it is freed when the last one is
  dropped.
- `Gpu::render(target, clear, record)` clears a target to a color, lets `record` draw into it
  through a `RenderPass` (`set_pipeline`, `set_vertex_buffer`, `draw`), and sends the work to the
  GPU.
- `Gpu::read_render_target(target)` waits for the GPU and copies a target into an `Image`: RGBA
  bytes, rows from top to bottom.
- `GfxError` says why the GPU could not be opened or used. A rejected shader or pipeline is
  returned as `GfxError::Shader` or `GfxError::Pipeline`, with wgpu's message, which for WGSL
  gives the line and column of the error. Other misuses of wgpu are bugs and panic.

## Dependencies

- `wgpu`: access to the GPU through Vulkan and Direct3D 12, and the WGSL shader compiler.
- `pollster`: waits for wgpu's requests, which are futures.
- `thiserror`: derives `GfxError`'s `Display` and `Error` implementations.
- `tracing`: logging.

## Must not

- Depend on `slime-platform` or any windowing code: windows reach it as `raw-window-handle`
  traits, so that offscreen rendering works without a windowing system.
- Expose wgpu's types in its public API.
