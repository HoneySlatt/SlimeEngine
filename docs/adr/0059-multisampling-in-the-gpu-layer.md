# 0059. Multisampling in the GPU layer

- Status: Accepted; amends [0033](0033-gpu-layer-for-multi-pass-rendering.md) (multisampled
  targets, pipelines and resolves)
- Date: 2026-10-15

## Context

Stage 3d smooths the edges of the scene with multisample anti-aliasing (MSAA) at four samples
([ADR-0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)): the GPU tests whether a
triangle covers each of four points of a pixel, its samples, shades the pixel once for those it
covers, and a *resolve* averages them into one pixel. `slime-gfx` knew one sample per pixel only:
its render and depth targets, its pipelines and its passes.

wgpu 30 shapes the choices:

- Four samples, with a resolve, are guaranteed without any optional feature for every format of
  the engine's render targets and for `Depth32Float`, the format of its depth targets. Eight are
  not.
- A pipeline is built for one number of samples, which the targets of every pass it draws in must
  have, the depth target's included.
- A pass resolves a multisampled color target into a target of one sample, of its size and format,
  when it ends, if its color attachment names one. Nothing resolves a depth target.
- A multisampled texture cannot be copied, so it cannot be read back. A shader reads one only
  through a binding declared multisampled (`texture_multisampled_2d`), sample by sample.
- Drivers place four samples at the points of the standard pattern of Vulkan and Direct3D, one in
  each quarter of the pixel's width and of its height.

Constraints: no wgpu type outside `slime-gfx` ([ADR-0016](0016-gpu-layer-with-wgpu.md)); a type per
use of a resource ([ADR-0026](0026-resource-binding-and-depth.md)), which ADR-0033 interpreted when
it gave the image of a window the type of the other render targets; and 37
`RenderPassDescriptor` and 19 `RenderPipelineDescriptor` literals in the workspace, three of them
in READMEs that are doc tests.

## Decision

- **Four samples, or one.** `MULTISAMPLE_COUNT`, 4, is the only number of samples besides one.
- **Targets.** `Gpu::create_multisampled_render_target(label, width, height, format)` and
  `Gpu::create_multisampled_depth_target(label, width, height)` return the existing `RenderTarget`
  and `DepthTarget` types, whose textures have the render-attachment usage only: passes draw into
  them, and nothing reads them back or samples them. Reading one back or binding one is refused by
  wgpu and panics, as for the image of a window (ADR-0033).
- **Pipelines.** `RenderPipelineDescriptor::multisampled: bool` builds a pipeline for targets of
  four samples.
- **Resolves.** `RenderPassDescriptor::resolve: Option<&RenderTarget>` names the target of one
  sample that the pass resolves its multisampled target into when it ends. The samples stay in the
  multisampled target: a later pass can draw over them and resolve, and a pass that draws nothing
  resolves too.
- **What is checked.** As ADR-0033 decided, `slime-gfx` adds no check of its own: wgpu's messages
  name a pipeline of the wrong number of samples, a depth target of another number than the
  target, and a resolve into a target that does not fit, when the submission's commands are
  finished.
- **Not now.** No binding reads samples, so no shader resolves on its own; and no depth target is
  resolved. Each waits for a use.

The tests check the standard pattern: an edge at a quarter of a pixel covers one sample or three,
at half of it two, and the depth test holds for each sample.

## Alternatives considered

- **A number of samples as a parameter**, of `create_render_target` or of the descriptors: a value
  that is 1 or 4 at every call. ADR-0033 rejected a parameter at every call for the same reason.
- **Types of their own** for multisampled targets: every API that takes a target would take two
  types, where the image of a window already shares the type of the others.
- **A resolve command** between passes: wgpu has none, so it would be a pass that draws nothing, as
  a `resolve` with no draw is.
- **Discarding the samples once resolved**, which saves memory bandwidth on some GPUs: the renderer
  needs them across two passes, the scene's and the sky's. To measure in Stage 3d before deciding.
- **Eight samples**: not guaranteed for every format, and nothing asks for them.

## Consequences

- Every literal of the two descriptors names the new field, `resolve: None` and
  `multisampled: false` where nothing changes.
- A multisampled target takes four times the memory of a target of one sample.
- A game that draws with `slime-gfx` can resolve into the image of a window, whose usage is a
  render attachment's.
- Smoothing the edges that a masked material cuts (alpha to coverage) is not offered: known debt of
  Stage 3d.
