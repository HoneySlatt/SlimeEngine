# 0038. Shadow mapping

- Status: Accepted; amends [0027](0027-the-render-crate.md),
  [0033](0033-gpu-layer-for-multi-pass-rendering.md) and
  [0037](0037-lights-in-physical-units-and-pre-exposure.md); amended by
  [0039](0039-image-based-lighting-computed-on-the-cpu.md) (the frame's values; group 0 holds the
  environment) and by [0041](0041-render-graph.md) (the shadow pass shares a submission and its
  timer)
- Date: 2026-10-13

## Context

The plan of Stage 3b gives the main light shadows: one shadow map for the first directional light
of `Scene::lights`, an orthographic projection fitted to the scene's bounds, a depth bias, and
percentage-closer filtering (PCF) over 3 × 3 samples through a comparison sampler; no cascades,
and no shadows of point and spot lights (`docs/PLAN.md`).
[ADR-0033](0033-gpu-layer-for-multi-pass-rendering.md) gave `slime-gfx` what this needs:
depth-only pipelines and passes, depth targets that later passes sample, comparison samplers whose
linear filtering blends 2 × 2 comparisons, a constant and a slope-scaled depth bias, and a timer
per pass. It could not describe a fragment stage without a color target, which a shadow pass needs
to cut out what masked materials discard ([ADR-0035](0035-physically-based-materials.md) left
open whether masked casters cast solid or cut-out shadows).

wgpu counts every bind group set in a render pass in that pass's usage, so a pass cannot bind a
group holding its own depth target; its default limits give four bind groups, of which the scene
pipelines use three. In WGSL, `textureSampleCompare` takes derivatives, while
`textureSampleCompareLevel` reads level 0 without them and becomes `SampleCmpLevelZero`, not a
gradient instruction, for FXC ([ADR-0036](0036-direct3d-12-shader-compiler.md)), which cannot
unroll the loop over the lights. The image tests are strict on lavapipe and loose elsewhere
([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md)), and the plan feared that shadow
edges flip whole pixels from one driver to another.

To decide: what a user sees and controls; how the projection fits the instances; the map's size,
lifetime and binding; the pass's pipelines and culling; where the light's matrix lives; masked
casters; the bias and the filter; how this is tested. The owner decided on 2026-10-13 that masked
materials cast cut-out shadows, that every face of an object blocks the light, that a directional
light can be told not to cast shadows, and that the sample keeps its sun.

## Decision

### What a user sees

- `DirectionalLight::casts_shadows`, `true` by default (and for the lights that the examples'
  loader reads): the first directional light of `Scene::lights` that casts shadows casts them, onto
  every instance, from every instance. The others, and point and spot lights, cast none. A scene
  without such a light runs no shadow pass.
- `RenderStats::shadow_draw_calls` (one per instance) and `RenderStats::shadow_gpu_time`; the
  pass is skipped, and they are 0 and `None`, without a light that casts shadows or when every
  instance is flattened to a point.

### The projection

- `Renderer::create_mesh` keeps the box around the mesh's positions. Each frame, `render` carries
  every instance's box into the light's view (its center transformed, its half size multiplied by
  the absolute values of the transform's 3 × 3 part) and takes the box around them all.
- The light's view looks along the light, its up vector any vector perpendicular to it. The
  orthographic projection covers that box exactly across the light, and along it from 1/64 of its
  largest extent before the nearest instance to as much after the farthest; every extent is at
  least twice that margin. It depends on the instances and the light only, never on the camera,
  and has no texel snapping.

### The map and the pass

- One 2048 × 2048 depth target of 32-bit floats (16 MiB), created by `Renderer::new`, cleared by
  each shadow pass.
- The shadow pass runs before the scene pass, in its own submission, with its own `GpuTimer`, one
  draw per instance in the scene's order, through two pipelines that share the vertex stage
  `vs_shadow`, the scene's vertex layout and bind group layouts (frame, material, instance), the
  depth test `Less` with writes, the bias below, and no face culled: every face blocks the light,
  whichever way it faces it, so that a single-sided surface stops light from behind too, and
  double-sided materials and mirrored instances need nothing more. The first pipeline has no
  fragment stage. The second, for masked materials, runs `fs_shadow`, which reads the base color's
  alpha and discards below the cutoff, as the scene pass does; its fragment stage has no target
  format, which `slime-gfx` now allows (`FragmentState::target_format: Option<TextureFormat>`).
- The scene pipelines read the map and a comparison sampler (`LessEqual`, linear) at bind group
  3, apart from the frame's group, which the shadow pass binds.
- The light's view-projection and the index of the shadowed light are in the frame's values:

| Uniform | Bytes | Members (offset) |
|---|---|---|
| Frame, group 0 | 1 200 | view_proj (0), camera_position (64), exposure (76), ambient (80), light_count (92, `u32`), light_view_proj (96), shadowed_light (160, `u32`, `u32::MAX` when none), lights (176, 16 × 64 bytes) |

### The lookup

- The scene's vertex shader computes each vertex's position in the map's clip space with the shadow
  pass's expression, and interpolates it.
- The fragment shader, once per fragment and before the loop over the lights, when a light is
  shadowed, compares that depth with the map at 3 × 3 points one texel apart with
  `textureSampleCompareLevel`, each comparison blending 2 × 2 texels, and averages them: across a
  shadow's edge the light ramps linearly over three texels. The loop multiplies the shadowed light,
  and it alone, by that share.
- `DepthBias { constant: 16, slope_scale: 4.0 }`. The comparisons reach texels up to two texels
  away along each axis: on a surface sloping diagonally to the map's axes, their depths differ by
  four times its steeper slope, which the slope-scaled bias multiplies. The constant covers
  rounding where nothing slopes, four times what lavapipe and RADV need once both passes compute
  the vertex's position alike.

### Tests

- `crates/slime-render/tests/shadows.rs` compares every pixel with values computed from the
  geometry: a square above a floor shadows the expected square, its edges ramping over three
  texels, whether it faces the light or not; a floor lit up to 85° from its normal, along and
  across the map's axes, turned and 46 m from the origin, does not shadow itself; the first
  directional light that casts shadows casts them, and no other; a masked square casts no shadow
  where it discards. Unit tests check the projection's fit, its degenerate cases and the mesh's
  box; a test of `slime-gfx` draws a depth-only pass whose fragment stage discards.
- The image tests keep ADR-0030's tolerances: with shadows, RADV differs from the lavapipe
  references as it did without.

This amends:
- **ADR-0027:** a frame takes three passes; the scene pass reads a fourth bind group, the shadow
  map's; meshes keep the box around their positions.
- **ADR-0033:** a fragment stage may have no target format, in a pass without a render target; the
  consequence that such a pipeline cannot be described no longer holds.
- **ADR-0037:** the frame's values are those above; `DirectionalLight` gains `casts_shadows`.

## Alternatives considered

- **Solid shadows for masked materials**, the shadow pass without a fragment stage: no change to
  `slime-gfx` and no cost, but leaves and fences would cast the shadow of their whole cards; the
  owner chose cut-out shadows. Their pipeline costs four to seven times as much as the other for
  the draws that use it (28 µs instead of 6.5 µs for a square covering the map on an RX 6900 XT),
  and only masked materials use it.
- **No switch, the first directional light always casting:** one field fewer to write, but no way
  to turn shadows off or to choose the light; the owner chose the switch. **A choice of light in
  `Scene`, flags per instance, a resolution setting:** no caller needs them yet.
- **Fitting to the camera's view.** Finer texels near the camera, but the map moves with every
  camera move, shimmers without snapping, and needs casters outside the view.
- **Bounding spheres, exact vertex bounds per frame, bounds given by the caller.** Looser; a cost
  proportional to vertices; API for what the renderer computes.
- **Texel snapping.** It steadies a box that moves without changing size, which no scene of the
  stage has.
- **1024 or 4096 texels.** 1024's texels are larger than a pixel of the owner's display at the
  sample's start view; 4096 takes 64 MiB and about 15 ms per frame on lavapipe.
- **A map created lazily.** The scene pipelines need a depth texture bound at every draw: it would
  need a placeholder.
- **The map in the frame's group, with a second frame layout for the shadow pass.** It leaves group
  3 free at the cost of two frame groups; kept in reserve.
- **The light's matrix in a uniform of its own.** A buffer, a layout, a group and a write more; the
  shadow pass binds the frame's group anyway.
- **Culling like the scene pass,** the faces the camera would draw: three or six shadow pipelines,
  and single-sided surfaces facing away from the light let it through; not culling costs about
  1 µs on an RX 6900 XT. **Culling front faces:** open meshes facing the light cast nothing.
- **A normal offset, or a receiver-plane bias.** More shader code; the hardware bias suffices, as
  measured.
- **The light-space position computed per fragment.** It rounds differently from the shadow pass,
  so that the constant must grow with the distance from the origin (1024 at 46 m).
- **`textureSampleCompare`, or the lookup inside the loop.** Derivatives require uniform control
  flow, and FXC cannot place a gradient instruction in a loop it cannot unroll.
- **A tolerance amended for shadow edges.** Measured unnecessary.
- **Moving the sample's sun,** so that the shadows face the starting camera: the asset and its
  references would change, and the lighting of M3b.4 with them; the owner kept the sun.

## Consequences

- Every image of a scene with a directional light that casts shadows changes; the references are
  rendered again.
- `Renderer::new` builds six pipelines and a 16 MiB depth target, even for scenes without a
  directional light. Each `render` with one runs a third pass: about 9 µs on the GPU for the sample
  at 1280 × 720 and 150 µs on the CPU on an RX 6900 XT; the scene pass's lookups cost about half of
  a light.
- Every `DirectionalLight` written in full names `casts_shadows`.
- Bind group 3 is the last of wgpu's default four.
- The map's texels are the instances' extent across the light divided by 2048: a scene that spreads
  wide gets coarse shadows, and an object smaller than a texel may cast none. A moving instance
  that changes that extent makes shadow edges crawl.
- The slope-scaled bias pushes stored depths back by about four texels' slope: thin objects within
  a centimetre or so of a receiver cast nothing at the sample's scale, more under grazing light.
- Measured: with shadows, RADV differs from lavapipe as without (image tests: one pixel more), and
  the sampling usage of the scene's depth target costs nothing measurable on RADV (ADR-0033's open
  question).
