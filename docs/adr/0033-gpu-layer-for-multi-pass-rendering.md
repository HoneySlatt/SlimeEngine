# 0033. The GPU layer for multi-pass rendering

- Status: Accepted; amended by [0038](0038-shadow-mapping.md) (a fragment stage may have no target format), by [0039](0039-image-based-lighting-computed-on-the-cpu.md) (cube textures, `Rgba16Float` textures from the CPU, levels given by the caller) and by [0041](0041-render-graph.md) (several passes in one submission; a timer times the passes of a submission)
- Date: 2026-10-13

## Context

Stage 3b draws each frame in several passes (`docs/PLAN.md`). M3b.2 draws the scene into a target
of 16-bit floating-point colors with a depth target, then a fullscreen pass samples it, maps it to
the display and writes the window's sRGB image. M3b.5 draws a depth-only shadow pass into a depth
target, with a depth bias, which the scene pass then reads through a comparison sampler with
percentage-closer filtering (PCF). The GPU time of each pass is reported.

[ADR-0017](0017-gpu-resource-types.md) and [ADR-0026](0026-resource-binding-and-depth.md) gave
`slime-gfx` 8-bit render targets read back as RGBA bytes; depth targets that only the depth test
reads; bindings of uniform buffers, textures and samplers; pipelines that always read one vertex
buffer and always have a fragment shader and a color target; passes that always have a color
target; and depth tests without bias. [ADR-0031](0031-dev-overlay-with-egui.md) gave a `GpuTimer`
that times a pass.

wgpu 30 shapes the choices:

- `Rgba16Float` renders, filters, is sampled and is copied without any optional feature;
  `Rgba32Float` filters only with `FLOAT32_FILTERABLE`, and `Rg11b10Ufloat` renders only with
  `RG11B10UFLOAT_RENDERABLE`.
- `Depth32Float` is sampled as a depth texture (`texture_depth_2d`), read with `textureLoad` or
  compared through a `sampler_comparison`, a binding type of its own that accepts linear
  filtering.
- A pipeline may declare no vertex buffer, and no fragment stage when it tests depth; a declared
  vertex buffer must be bound at every draw.
- A pass's list of color attachments must equal its pipelines' list of targets: a pass without a
  color target has an empty list, not one empty slot.
- A depth-bias clamp needs a downlevel flag. Clear colors reach the driver unclamped.
- Sampling a texture in the pass that draws into it is a usage conflict, which wgpu reports when
  the pass's commands are finished.
- A window's surface guarantees only the render-attachment usage, the one the engine configures.

Constraints: no wgpu type outside `slime-gfx` ([ADR-0016](0016-gpu-layer-with-wgpu.md)); a type
per use of a resource (ADR-0026); no new dependency in Stage 3b, while Rust's `f16` type is still
unstable; and 16 `RenderPassDescriptor` and 7 `RenderPipelineDescriptor` literals in the
workspace, three of them in READMEs that are doc tests.

## Decision

- **Float render targets.** `TextureFormat::Rgba16Float` is a format of render targets only:
  `Gpu::create_texture` refuses it until a texture needs float data.
  `Gpu::read_float_render_target` reads such a target into a `FloatImage` of `f32` values,
  converted exactly from half precision by a hand-written function. `Image` and
  `Gpu::read_render_target` keep the 8-bit contract; each read method panics on the other's
  formats.
- **Targets that shaders sample.** Every render target that `Gpu::create_render_target` creates,
  and every depth target, can be sampled. A later pass binds them with
  `BindingResource::RenderTarget` at a `BindingType::Texture` binding (`texture_2d<f32>`), and with
  `BindingResource::DepthTarget` at a new `BindingType::DepthTexture` binding
  (`texture_depth_2d`). A window's image keeps the render-attachment usage only. Binding it, like
  sampling a target in the pass that draws into it, is refused by wgpu and panics; `slime-gfx` adds
  no check of its own.
- **Pipelines.** `RenderPipelineDescriptor::vertex_layout` is an `Option`: `None` declares no
  vertex buffer, and the vertex shader builds its vertices from `@builtin(vertex_index)`.
  `fragment_entry_point` and `target_format` become `fragment: Option<FragmentState>`, holding the
  entry point and the target format: `None` is a pipeline without fragment shader or color target,
  which writes depth only. A pipeline with neither a fragment stage nor a depth test is rejected as
  `GfxError::Pipeline`.
- **Depth-only passes.** `Gpu::render_depth(&DepthPassDescriptor { depth, timer }, record)` runs a
  pass without a color attachment. It always clears the depth target to the farthest depth, and
  records into the same `RenderPass` as `Gpu::render`. `RenderPassDescriptor` and `Gpu::render` do
  not change; both methods share one implementation.
- **Comparison samplers.** `ComparisonSampler` is a type of its own, created by
  `Gpu::create_comparison_sampler(label, compare, filter)` from the depth test's `DepthCompare` and
  a `Filter`, always clamped to the edge. It is bound with `BindingResource::ComparisonSampler` at a
  new `BindingType::ComparisonSampler` binding (`sampler_comparison`). Linear filtering gives PCF
  over 2 × 2 pixels, done by the GPU.
- **Depth bias.** `DepthState::bias` is a `DepthBias { constant, slope_scale }`, in the units that
  Vulkan and Direct3D 12 define for floating-point depth. Its default, zero, is no bias. It has no
  clamp.
- **Several timed passes.** One `GpuTimer` per timed pass, each read with its own `latest`. The
  timer's API does not change.
- **What is checked where.** Unchanged in principle: shader and pipeline rejections come back as
  `GfxError::Shader` or `GfxError::Pipeline`; reading a target with the wrong method, and creating
  a texture in `Rgba16Float`, are `slime-gfx` asserts; binding a resource of the wrong kind, a
  window's image, or a target the pass draws into, and using a pipeline that does not fit its
  pass, are wgpu validation errors that panic. They are documented under `# Panics`.

This amends ADR-0017 (formats, readback, how passes run, what pipelines draw) and ADR-0026
(binding kinds, sampled depth targets, the pipeline descriptor, the depth state).

## Alternatives considered

- **`RenderPassDescriptor::target: Option<&RenderTarget>`.** One pass method, but all 16 pass
  literals change, a depth-only pass carries a clear color it does not use, and a pass without any
  attachment compiles and panics at run time.
- **Two independent options for the fragment entry point and the target format.** They admit a
  target format without a fragment shader, which wgpu cannot express.
- **A required fragment entry point with an optional target format.** Every depth-only pipeline
  then carries an empty function. Its one use, alpha-masked shadow casters, is not asked for by any
  milestone.
- **A separate `DepthPipelineDescriptor` and `create_depth_pipeline`.** They repeat six fields of
  the descriptor and are a second way to build a pipeline.
- **A type for offscreen targets, dereferencing to `RenderTarget`, so that a window's image cannot
  be bound or read back at compile time.** It changes the return type of `create_render_target`,
  uses `Deref` on a type that is not a smart pointer, and duplicates a refusal that wgpu already
  makes.
- **A `slime-gfx` assert that recognizes window images by their missing usage.** It is a second
  check in front of wgpu's, tied to how the surface is configured.
- **A comparison field in `SamplerDescriptor`.** One fewer type, but one `Sampler` type for two
  WGSL types; repeating address modes, the default, would make shadow lookups wrap; and three
  literals would change.
- **`BindingType::Sampler { comparison: bool }`.** It changes the existing variant. The rule kept is
  one variant per WGSL binding type.
- **The bias as two fields of `DepthState`, as a pipeline field, or as `Option<DepthBias>`.** Two
  fields lose the grouping that documents the formula; a bias without a depth test means nothing;
  `Some(zero)` would be a second way to say "no bias".
- **A bias clamp.** It needs a downlevel flag, and nothing uses it.
- **A clear flag on depth-only passes.** No pass keeps a depth target between passes yet.
- **`Image` holding float pixels, or an image tagged with its format.** Either breaks `Image`'s
  four-bytes-per-pixel invariant, on which the PNG writer and the image tests rely.
- **The `half` crate.** A dependency for a dozen lines.
- **An opt-in flag making a target sampleable.** A parameter at every call, for a cost that has not
  been measured.
- **An attribute-less `VertexLayout` meaning "no buffer".** wgpu would still require a buffer in
  slot 0.
- **A timer with named scopes, or all passes of a frame in one encoder.** Neither is needed for 3
  to 5 passes. The render graph of M3b.8 may revisit submission.
- **`Rgba32Float` or `Rg11b10Ufloat` targets.** Each needs an optional feature.

## Consequences

- Callers change mechanically: 7 pipeline literals (`vertex_layout: Some(..)`, and
  `fragment: Some(FragmentState { .. })` instead of two fields) and 2 `DepthState` literals
  (`bias: DepthBias::default()`). No pass or sampler literal changes.
- `DepthState` is no longer `Eq` or `Hash`, since it holds an `f32`.
- There are two pass methods and two read methods, each for one kind of target.
- Every offscreen render target and depth target carries the sampling usage, the renderer's own
  depth target included. On some GPUs such textures may be compressed less; this is not measured.
- A mismatch between a pipeline and its pass, or a target bound by the pass that draws into it,
  panics with wgpu's message when the pass is finished inside `render` or `render_depth`, not at
  the faulty call.
- A pipeline that runs a fragment shader without a color target (alpha-masked shadow casters)
  cannot be described; supporting it would make `FragmentState::target_format` optional.
- Each pass is still its own submission. Times of different timers can come from different frames,
  and the sum of a frame's pass times is not a frame time.
- The depth-bias amount and linear comparison filtering depend on the driver. The tests check the
  expected values and print what they measure: on lavapipe, on the owner's RX 6900 XT (RADV) and
  on WARP, the constant and slope-scaled biases are exactly the amounts the formula gives, and a
  comparison halfway between a shadowed and a lit texel gives 0.5.
- `Rgba16Float` textures uploaded from the CPU, cube and array textures, mip levels given by the
  caller, multisampling, blending and compute passes arrive with their first users.

## Corrections

- 2026-10-13: "The timer's API does not change" held when M3b.1 was done. `GpuTimer::latest` has
  since lost its parameter: it no longer polls the device, and returns the latest time that came
  back.

## Corrections

- 2026-10-13: the consequence "this is not measured" about the sampling usage was measured at
  M3b.5, on an RX 6900 XT (RADV), with two builds that differ only by it: the scene pass took
  43.9 µs with the usage and 44.0 µs without at 1920 × 1080, 306.7 and 306.8 µs with 8 layers drawn
  back to front, 51.3 µs either way front to back. Other GPUs are not measured.
