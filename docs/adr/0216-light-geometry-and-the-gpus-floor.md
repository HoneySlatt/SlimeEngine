# 0216. Light, geometry and the GPU's floor

- Status: Accepted; amends [0034](0034-hdr-rendering-and-tone-mapping.md) (the curve of tones in
  a game's look file), [0035](0035-physically-based-materials.md) (the model of light apart from
  the surface), [0098](0098-the-games-the-engine-aims-at.md) (virtual geometry at Stage 9a),
  [0114](0114-hardware-tiers-and-the-gpus-limits.md) (one raised floor; the enthusiast tier's
  path tracing), [0115](0115-what-the-gpu-layer-gives-the-editor.md) (a render graph open to games, with
  compute) and [0208](0208-the-engines-promises-and-systems.md) (the ladder of light; learned models
  at Stage 9a)
- Date: 2026-11-07

## Context

[ADR-0208](0208-the-engines-promises-and-systems.md) gave light without long bakes a ladder, one
technique per tier: rays traced by the hardware on the high tier, tracing in software on the modest
one, probes on the Deck and cooked probes on the minimal tier; ray tracing never required; learned
models decided at Stage 9c; a reference path tracer at Stage 9a. Virtual geometry waited for Stage
10a, admitted against a measured problem.

On 2026-11-07 the owner asked whether ray tracing is the way: games of the market lit beautifully
before it, and it often costs much for an image that is not exceptional. What stands:

- With enough rays, path tracing is the truth of light; at the rays a frame affords, it is noisy,
  and the denoiser leaves blur and trails. On the largest GPUs of 2026, path tracing in real time
  gives the market's best images; on most machines, an approximate method gives an image closer to
  the truth at the same cost.
- Games of the market that lead in image use little or no ray tracing; they bake light and touch
  scenes up by hand, which a small team helped by agents cannot afford.
- wgpu offers ray queries as an experimental feature on Vulkan, Direct3D 12 and Metal, and pipelines
  of ray tracing on Vulkan alone.
- The renderer's code ([`slime-render`](../../crates/slime-render/README.md)) holds the light inside
  the material's shader: one entry point gathers 16 lights, shadows, the light of the environment
  and the physically based formulas; a kind of material supplies a whole shader file, so that a toon
  kind would copy that gathering; shader files share no code. Each material has a group of
  bindings of its own; the floor is WebGPU's default limits; there is no compute; the render graph's
  passes are a closed list, each reading two targets at most, all sized alike; the sky's light is
  computed on the CPU; upscaling is a stretch inside the pass of tones, whose curve is fixed.
- The world aims at no visible limit to the distance seen, dense cities and foliage, and assets
  imported without hand-made levels of detail ([ADR-0215](0215-the-engine-measured-against-the-market.md),
  [ADR-0217](0217-worlds-by-density-generated-and-destroyed.md)).
- Unreal 5 requires Direct3D 12 or Vulkan; Godot 4 keeps a renderer of compatibility on OpenGL.

## Decision

The owner's decisions of 2026-11-07.

**One architecture of dynamic light, for every tier.** Only the way of tracing changes: rays traced
by the hardware, tracing in software, probes updated over frames, probes cooked. On each tier, the
method whose image is closest to the reference path tracer at equal GPU time is the one used; ray
tracing is never required. Artists light once, whatever the tier.

**Path tracing in real time, at the top.** The enthusiast tier's top rung is path tracing in real
time, which shares its code with the reference path tracer: the surface and the light that a ray
meets are computed by the same code as the rasterizer's.

**When rays start** is Stage 9a's question: wgpu's experimental ray queries, measured on RADV, the
Deck and Direct3D 12, against a native Vulkan path. The native backends of Stage 9c come sooner if
the high and enthusiast tiers need them.

**Virtual geometry,** decided at Stage 9a's start with the architecture of light, as the base of all
geometry: clusters of triangles in a hierarchy of detail, culling on the GPU, small triangles
rasterized in software, and the way of applying materials that goes with it, a visibility buffer if
measured better. Meshes cooked in clusters come with the redesign of cooked archives at Stage 9b;
foliage is measured apart, against Unreal's.

**Learned models in play,** decided at Stage 9a's start: inference on the engine's own compute
shaders, on every vendor's GPU, without a vendor's kit; first for the upscaler and the denoiser,
then faces. Inference in tools, capture and voice, is decided at Stage 9b's start. Stage 9c adds the
vendors' accelerations. A model and its training data have licences that fit the engine's, checked
at each choice.

**A shader in three layers,** recorded in an ADR before Stage 7: the surface, a material's; the
model of light, physically based or a ramp of toon, which a game may write; and the gathering of
light and its visibility, the engine's, chosen per tier. A system of shader modules joins them, a
dependency put to the owner then.

**A floor raised, and only one.** Vulkan, Direct3D 12 or Metal is required. At Stage 9a's start, the
floor rises for every tier: materials in a table read by index, textures in arrays, meshes readable
as storage, shaders in 16-bit floats and subgroups, measured on the Deck, RADV, WARP and lavapipe.
The minimal tier is named on hardware of about 2015 that has Vulkan, of the class of an Intel HD
520, as the owner's laptop. No renderer on OpenGL. Applications on uwui keep their renderer on the
CPU.

**A render graph open to games,** in Stage 9a's first milestone: passes that a game registers;
buffers and compute in the graph; a size drawn apart from the size shown; targets kept from one
frame to the next; its model of resources ready to drive explicit barriers and the aliasing of
memory on the native backends. The sky is computed on the GPU, and a game may replace it. The
upscaler runs before the effects after the frame. The curve of tones is the look file's.

## Alternatives considered

- **ADR-0208's ladder kept,** a fixed technique per tier: four methods of light to keep alike, and
  nothing to beat path tracing in real time on the largest GPUs.
- **Ray tracing first,** required from the high tier as some engines do: one path to tend there, but
  the owner's RX 6900 XT, the machine that measures, is weak at it, and a cheaper method may give a
  better image for the same time on part of that tier.
- **Path tracing in real time put aside:** cheaper, and the market's best image left unmatched.
- **Virtual geometry kept at Stage 10a:** Stage 9a's light, materials and cooked meshes built on a
  shape that it would then remake.
- **A copy of Nanite's design decided now:** chosen without measure, years before Stage 9a, against
  principle 9.
- **Learned models kept at Stage 9c:** no learned upscaler or denoiser at Stage 9a, faces below the
  market's best at Stage 9b.
- **A second renderer on OpenGL ES** for the minimal tier: the widest reach, as Godot keeps, and two
  paths to keep alike for hardware that is going.
- **The renderer left as it is until Stage 9a:** Stage 7's interfaces drawn with games' materials
  would be built on a shader that the layers then replace.

## Consequences

- Stage 9a starts with the foundations: compute, the raised floor, shader layers, the open graph,
  the sky on the GPU, the reference path tracer and the measure of closeness to it, before any
  technique is chosen ([ADR-0220](0220-the-roadmap-reshaped-by-the-market.md)).
- Tracing in software is built first: it serves the reference path tracer, which CI runs on CPU
  drivers, the modest tier and the probes; rays traced by the hardware accelerate the same tracing.
- GPUs without Vulkan, Direct3D 12 or Metal, mostly older than about 2015, do not run games made with
  the engine.
- The editor's views and the interfaces of games drawn with materials follow the shader's layers.
