# 0034. HDR rendering and tone mapping

- Status: Accepted; amended by [0035](0035-physically-based-materials.md) (light above 65 504 is written as 65 504), by [0037](0037-lights-in-physical-units-and-pre-exposure.md) (the exposure is applied by the scene pass, before the HDR target is written; the tone-mapping pass no longer multiplies by it) and by [0039](0039-image-based-lighting-computed-on-the-cpu.md) (the sky replaces the clear color)
- Date: 2026-10-13

## Context

The renderer of Stage 3a ([ADR-0027](0027-the-render-crate.md)) draws each frame in one pass,
straight into its output: the window's 8-bit sRGB image, or an offscreen one. Light above 1 clips
there, so a surface lit twice as strongly as white looks like white, and a saturated color that
clips in one channel changes hue. Stage 3b brings light far above 1: physically based highlights
(M3b.3), lights in lux and candela (M3b.4), a sky (M3b.6), and a bloom (M3b.7) that needs the
image before it is mapped to the display. Its plan draws the scene in high dynamic range (HDR)
first, then maps it to the display with a fullscreen pass that multiplies it by an exposure and
applies a tone-mapping operator, so that every later image test renders its references against
that output transform once.

[ADR-0033](0033-gpu-layer-for-multi-pass-rendering.md) gave `slime-gfx` what this needs:
`Rgba16Float` render targets (half-precision floats: 11 significant bits, magnitudes up to
65 504, beyond which a driver stores 65 504 or an infinity) that a later pass samples; pipelines
without a vertex buffer, for a fullscreen triangle; and one `GpuTimer` per pass.

To decide: the tone-mapping operator; what the exposure is and its unit; who owns the HDR target
and when it is created; how the second pass reads it; what the statistics report; and how tests
check values through the curve.

Constraints: no wgpu in `slime-render` (ADR-0027); no new dependency in Stage 3b; image tests
strict on lavapipe and loose on RADV and WARP
([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md)). The glTF specification defines
no display transform, so what makes an asset look as its author intended is what glTF viewers do.
The Khronos glTF Sample Viewer multiplies the light by an exposure of 1 by default, then applies
Khronos PBR Neutral; three.js, Filament, Babylon.js, model-viewer and CesiumJS ship the same
operator, and Blender offers it as a view transform.

Candidate operators, as 8-bit sRGB output at an exposure of 1:

| Operator | 0.18 | 0.5 | 1.0 | 4.0 | (0.8, 0.15, 0.1) | (1, 0, 0) |
|---|---|---|---|---|---|---|
| None (clip) | 118 | 188 | 255 | 255 | 231/108/89 | 255/0/0 |
| Khronos PBR Neutral | 105 | 181 | 240 | 253 | 226/93/69 | 241/33/33 |
| ACES, Narkowicz's fit | 141 | 206 | 232 | 252 | 225/128/99 | 232/0/0 |
| ACES, Hill's fit | 91 | 165 | 206 | 245 | 199/86/67 | 216/0/9 |
| Reinhard, x/(1+x) | 109 | 156 | 188 | 231 | 178/101/85 | 188/0/0 |

## Decision

- **Two passes.** The scene pass draws the instances into an `Rgba16Float` target, the HDR
  target, with its depth target. The tone-mapping pass then draws one fullscreen triangle, without
  a vertex buffer, into the output: each pixel reads the texel under it with `textureLoad`,
  without a sampler, multiplies it by the exposure, applies the operator and writes the result
  with an alpha of 1. The output can have any render-target format: an sRGB one encodes the
  result for display, `Rgba16Float` keeps the tone-mapped linear values. The dev overlay keeps
  drawing after both, in the output, without tone mapping.
- **Targets.** The renderer owns the HDR target, the depth target and the bind group through
  which the tone-mapping pass reads the HDR target and the exposure. It creates them together at
  the size of the output it draws into, and again together when that size changes, since a group
  keeps the target it was created with.
- **Operator: Khronos PBR Neutral**, fixed in the code, written in WGSL from the equations of its
  specification (<https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md>,
  cited in the shader), with F90 = 0.04, K_s = 0.76 and K_d = 0.15. Colors whose channels all lie
  between 0.08 and 0.8 come out lowered by 0.04 with their hue kept; darker ones lose less, and
  black stays black; brighter ones are compressed towards 1, which they never reach, and move
  towards white, still with their hue. Its input must not be negative.
- **Exposure.** `Scene::exposure`, a linear factor: 1 leaves the light as it is, 2 is one stop
  brighter, 0 gives black. The samples and the tests use 1, the operator's calibration point and
  the Sample Viewer's default. The tone-mapping pass applies it, from a uniform buffer of its own,
  to everything the scene pass wrote, the background included.
- **Clear color.** Light, in the units of the lights: it may exceed 1, and is exposed and tone
  mapped like surfaces. Its alpha is not used.
- **Statistics.** `RenderStats` gives the GPU time of each pass in a field of its own,
  `scene_gpu_time` and `tone_mapping_gpu_time`, each from its own `GpuTimer`, and no total.
  `draw_calls` and `triangles` count the scene pass.
- **Tests.** Tests that read pixels back compute their expected values with a copy of the curve
  kept in the tests, which a test checks against values derived by hand from the specification.
  The tone-mapping pass itself is checked by reading back an `Rgba16Float` output whose HDR input
  is the clear color of a scene without instances. The references of the image tests are rendered
  again on lavapipe and looked at; the tolerances of ADR-0030 stand unless a measurement on RADV
  or WARP exceeds them.

This amends ADR-0027: "one pipeline" becomes two pipelines and two passes; the renderer owns the
HDR target next to the depth target; `Renderer::new` takes the output's format, and the scene is
drawn in `Rgba16Float`; the convention "the colors of materials and clear colors are linear, and
the sRGB render target encodes them" becomes "colors are linear light; the scene is drawn in high
dynamic range, multiplied by the exposure, tone mapped, then encoded by the output's format".

## Alternatives considered

- **ACES, Narkowicz's fit.** One line, but per channel: it shifts hues, keeps saturated highlights
  saturated until they clip, and brightens mid-tones (0.5 shows as 206).
- **ACES, Hill's fit.** Closer to ACES, with its opinionated contrast: darker mid-tones (0.5 shows
  as 165), two matrices.
- **AgX.** Good highlights, but it desaturates base colors by design, and needs log2, pow, a
  polynomial and two matrices, whose precision depends more on the driver.
- **Reinhard** (per channel, extended, or on luminance). It dims every color (0.5 shows as 156,
  white as 188); the per-channel form shifts hues, the luminance form leaves saturated highlights
  out of range.
- **No tone mapping, clipping at 1.** Leaves no reason to draw in HDR.
- **A choice of operators.** No second use; it is added when a game asks for another look.
- **Tone mapping in the scene shader, as the Sample Viewer does.** No HDR image is left for the
  sky and the bloom, and every overdrawn fragment is tone mapped.
- **The exposure applied by the scene pass, before the HDR target is written (pre-exposure).** It
  keeps the HDR target within half precision whatever the units of the lights, which the plan's
  wording does not: with lights in lux and candela (M3b.4), a white surface under a 100 000-lux sun
  reflects about 31 800 cd/m², a highlight of the sun far more, and beyond 65 504 a driver stores
  65 504, which clips the highlight, or an infinity, which the curve turns into NaN. It departs from
  the plan as validated, so it is proposed to the owner for M3b.4, where it becomes necessary.
- **The lights multiplied by the exposure on the CPU, as Filament does.** The same headroom, but
  emission and the sky also need it in shaders: two ways to apply it.
- **The exposure in `Camera`.** The camera gives matrices; the exposure goes with the lighting,
  and the plan put it in `Scene`.
- **The exposure in EV100, or in stops.** No light has a photometric unit yet; stops are an
  interface concern.
- **`textureSample` with a sampler.** The HDR target has the output's size, so there is nothing to
  filter; a sampler adds a binding, coordinates, and filtering that drivers round differently.
- **The HDR target given by the caller, or a `resize` method.** Every application would track the
  size, which the renderer already follows for its depth target.
- **`Rg11b10Ufloat` or `Rgba32Float` targets.** Each needs an optional feature (ADR-0033).
- **A public tone-mapping function.** The tests would be its only user, and a public copy of the
  formula would have to be kept in step with the shader.
- **`RenderStats` as a list of passes, or a total GPU time.** A list loses `Copy` and allocates
  every frame; the render graph (M3b.8) is what will name passes. A total is not a frame time.
- **Counting the tone-mapping draw in `draw_calls`.** It changes every assertion for a constant.
- **The alpha of the HDR target passed to the output.** It would let a material make the window
  translucent, a meaning that the sky and the bloom will not keep.
- **A clamp of negative input in the shader, or an assert on the exposure.** Code for inputs the
  API documents as out of domain; the renderer documents its other constraints without asserting
  them.
- **Encoding with pow(1/2.2), as the Sample Viewer does.** Not the sRGB standard, and it would
  need a non-sRGB output.
- **Raising the samples' background color to keep its pixels.** The sky replaces it in M3b.6.

## Consequences

- Every image changes: a white surface lit at normal incidence by a light of 1 shows as 240 out
  of 255 instead of 255, and full white needs about 13.5 times that light; mid-tones lose 0.04
  (0.5 shows as 181 instead of 188); highlights no longer clip and turn towards white; dark colors
  get darker and more saturated, since the curve brings them towards 0. The offset assumes that
  surfaces reflect about 4 % of their surroundings, which image-based lighting (M3b.6) adds: until
  then, shadowed surfaces and the background look darker than authored (the samples' background
  shows as (34, 44, 60) instead of (63, 69, 80)).
- The references of the image tests are rendered again now, and by every milestone that changes
  shading. Dark, saturated channels are more sensitive to how drivers filter: the checkered sphere
  of the shapes test differs more on other drivers than in Stage 3a (measured in `docs/PLAN.md`).
- Each frame takes one more pass, submission and draw call, and 8 more bytes of GPU memory per
  pixel. The times of the two passes can come from different frames.
- The output is opaque.
- Light above 65 504 overflows the HDR target: it clips, or becomes NaN through the curve on a
  driver that stores an infinity. The scene must stay within that range before the exposure,
  which lights in physical units will not.
- The darkest tones differ slightly from the Sample Viewer, which encodes with pow(1/2.2).
- `RenderStats::gpu_time` is gone: callers read the time of each pass.
