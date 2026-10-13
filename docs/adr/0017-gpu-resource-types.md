# 0017. GPU resource types

- Status: Accepted; amended by [0026](0026-resource-binding-and-depth.md) (buffers, bind groups, depth) and [0033](0033-gpu-layer-for-multi-pass-rendering.md) (float targets, sampled targets, depth-only passes)
- Date: 2026-10-12

## Context

To draw anything, the code above `slime-gfx` needs to create GPU objects (a render target, a
vertex buffer, a shader, a pipeline) and record drawing commands, without seeing wgpu's types
([ADR-0016](0016-gpu-layer-with-wgpu.md)). The roadmap first spoke of "engine-owned handles",
which suggests identifiers into tables owned by the GPU layer;
[ADR-0014](0014-amend-roadmap-for-stage-2-and-windows.md) left the representation to be decided
when the first users exist. In Stage 2, those users are the triangle of the samples and the
offscreen render of the reference image test.

wgpu's objects are reference-counted: a clone is cheap, and wgpu keeps an object alive until the
GPU no longer uses it, even after the program drops it.

## Decision

- **Owning types.** Each kind of GPU object is an engine-owned type that holds its wgpu object
  privately: `RenderTarget`, `VertexBuffer`, `ShaderModule`, `RenderPipeline`. They are created by
  methods of `Gpu`, are `Clone` (clones share the object), and free it when the last clone is
  dropped. There is no table of identifiers and no explicit destruction.
- **Only what is used.** Each type covers what the triangle and the offscreen render need: render
  targets of one format (`Rgba8UnormSrgb`) that can be read back, vertex buffers filled at creation,
  pipelines that draw triangle lists from one vertex buffer without depth, culling or blending.
  Each grows when a renderer needs more. (When this was decided, `Rgba8UnormSrgb` was the only
  format; the window milestone added `Bgra8UnormSrgb`, the usual format of windows, whose images
  cannot be read back: ADR-0020.)
- **Labels everywhere.** Every creation method takes a label. It names the object in error
  messages and in GPU capture tools.
- **Errors at creation.** Shader modules and pipelines can be rejected for reasons the caller
  controls (a WGSL error, a missing entry point). They are created inside wgpu error scopes, and a
  rejection, by wgpu's validation or by the graphics driver's compiler, is returned as
  `GfxError::Shader` or `GfxError::Pipeline`, with wgpu's message, which for WGSL shows the line
  and column of the error. Other wgpu errors are bugs and panic (ADR-0016).
- **Drawing through a closure.** `Gpu::render(target, clear_color, |pass| ...)` creates the
  command encoder and the render pass, lets the closure record commands into an engine-owned
  `RenderPass`, then submits the work. wgpu's lifetimes stay inside `slime-gfx`.
- **Reading back.** `Gpu::read_render_target` copies a target into CPU memory as an `Image` of
  RGBA bytes, rows from top to bottom, handling the row alignment that GPU copies require.
- **Bytes for vertex data.** Vertex buffers take a byte slice; how the caller produces it is its
  choice. No dependency on a casting crate (such as `bytemuck`) until vertex structures need one.

## Alternatives considered

- **Generational identifiers into tables owned by `slime-gfx`.** Copyable and decoupled from
  wgpu's lifetimes, which a render graph or asset streaming may need; today it would only add
  bookkeeping, explicit destruction and use-after-free checks with no user.
- **Exposing wgpu's types from `slime-gfx`.** No wrapping at all, but the samples and games, which
  sit above `slime-gfx`, would see wgpu's types, against a fixed decision of the roadmap.
- **One generic `Buffer` type.** Only vertex buffers exist for now; a general type is to introduce
  with the second kind of buffer.
- **Command encoders and passes exposed as objects.** More flexible, but their lifetimes are
  wgpu's and would leak into the engine's API; the closure covers every use so far.

## Consequences

- Rust's ownership manages GPU memory: nothing to destroy by hand.
- Every new GPU feature needs its own engine-owned type or method, which keeps the boundary
  explicit but costs some code at each step.
- Moving to identifiers later, if a render graph needs them, changes `slime-gfx` and its callers
  (the samples now, the renderer later); before a renderer exists, the change is small.
