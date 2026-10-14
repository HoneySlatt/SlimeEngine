# Plan

This file says what is being worked on now. It lists the milestones of the current stage, their
status, how each one is checked, and the known debt. Direction beyond this stage is in
[`ROADMAP.md`](ROADMAP.md); decisions are in [`adr/`](adr/README.md). A *stage* is a step of the
roadmap; a *milestone* is a small, verifiable step of a stage.

Every milestone ends with the definition of done in [`AGENTS.md`](../AGENTS.md#definition-of-done):
build, tests, clippy, rustfmt and rustdoc pass, the program was actually run, and the affected
documentation was updated and reread. From Stage 2 on, every milestone is also checked on Windows
([ADR-0014](adr/0014-amend-roadmap-for-stage-2-and-windows.md)).

## Current

- Stage 3b — Physically based renderer → 0.1: in progress. Planned on 2026-10-13 and validated by
  the owner. M3b.1 to M3b.8 are done. The owner's decisions of 2026-10-13 on earlier proposals are
  applied, and the findings of rounds 4 and 5 of the audit of 0.0.3 (issue
  [#4](https://github.com/HoneySlatt/SlimeEngine/issues/4)) are fixed or recorded below; next: a
  pass over the known debt that users of 0.1 would meet, then M3b.9.

## Status

| Milestone | Goal | Status |
|---|---|---|
| [M3b.1](#m3b1--gpu-layer-for-multi-pass-rendering) | GPU layer for multi-pass rendering | Done |
| [M3b.2](#m3b2--hdr-rendering-and-tone-mapping) | HDR rendering and tone mapping | Done |
| [M3b.3](#m3b3--physically-based-materials) | Physically based materials | Done |
| [M3b.4](#m3b4--light-types) | Light types | Done |
| [M3b.5](#m3b5--shadows) | Shadows | Done |
| [M3b.6](#m3b6--image-based-lighting) | Image-based lighting | Done |
| [M3b.7](#m3b7--bloom) | Bloom | Done |
| [M3b.8](#m3b8--render-graph) | Render graph | Done |
| [M3b.9](#m3b9--windows-check-documentation-release-01) | Windows check, documentation, release 0.1 | Planned |

Stages 0, 1, 2 and 3a are done; see [Completed stages](#completed-stages).

## Stage 3b — Physically based renderer → 0.1

Exit criterion (see [`ROADMAP.md`](ROADMAP.md#stage-3b--physically-based-renderer--01)): a sample
scene with a camera, lights, physically based materials and shadows, covered by image tests.

The renderer of Stage 3a becomes physically based (PBR): surfaces say how much light they reflect
and how, with glTF's metallic-roughness materials; lights have physical units; the scene is drawn
in high dynamic range (HDR), with colors above 1, then mapped to the display by a tone-mapping
pass; a shadow map gives the main light its shadows; image-based lighting (IBL) lights surfaces
with their surroundings; and a minimal render graph, extracted from these passes at the end of the
stage, orders the passes of a frame.

Out of scope, left to the stages that need them: blended transparency (glTF's `BLEND` alpha mode;
`MASK` is in); cascaded shadow maps, and shadows of point and spot lights; environments read from
HDR image files (a decoder or a new dependency, and an asset far above the sample's budget: the sky
is procedural); IBL precomputed on the GPU, compute passes and storage buffers; screen-space
effects (ambient occlusion, reflections) and temporal anti-aliasing; multisampling and FXAA, unless
the image tests or the sample show aliasing that the tolerance cannot absorb, the rule that
admitted mipmaps in Stage 3a; tangents computed with MikkTSpace, as glTF prescribes for files
without tangents (the loader computes simpler ones; the importer of Stage 4 will do it right);
animation and skinning; glTF extensions other than `KHR_lights_punctual` and
`KHR_materials_emissive_strength`; and, as in Stage 3a, a scene model (Stage 5) and the asset
pipeline (Stage 4): the renderer still draws flat lists of instances, and the glTF loader stays
temporary.

Decisions taken when this stage was planned, each recorded as an ADR in the milestone that applies
it:

- HDR first. The scene is drawn into a target of 16-bit floating-point colors (`Rgba16Float`),
  then a fullscreen pass multiplies it by an exposure, maps it to the display's range with a
  tone-mapping operator, and writes the sRGB image of the window. It comes first because every
  later image test depends on this output transform: their references are rendered once against
  it.
- Materials. glTF's metallic-roughness model, with the formulas of the Khronos glTF sample viewer
  (the GGX distribution, Smith's visibility term, Schlick's Fresnel approximation): base color,
  metallic-roughness, normal, occlusion and emissive textures, each created in the format of its
  slot (sRGB for colors, linear for data); the `MASK` alpha mode by discarding fragments;
  double-sided materials drawn without culling.
- Lights. Directional, point and spot lights in glTF's units (lux for directional lights, candela
  for the others), read from the `KHR_lights_punctual` extension through the `gltf` crate's
  feature of that name, a feature of an admitted dependency recorded in `dependencies.md`; a fixed
  maximum number per scene, in a uniform buffer; storage buffers only if a measurement asks for
  them.
- Shadows. One shadow map, for the first directional light: an orthographic projection fitted to
  the scene's bounds, a depth bias, and percentage-closer filtering (PCF) over 3 × 3 samples
  through a comparison sampler.
- IBL. A procedural sky drawn into an environment cube map, its irradiance, its prefiltered
  specular levels and the table of the BRDF's scale and bias, all computed on the CPU at startup:
  no compute pass and no image file. The asset pipeline of Stage 4 may cook them instead. (Done at
  M3b.6, ADR-0039: the roughest level is the irradiance; the sun's disc is drawn by the sky pass,
  its light carried by the scene's directional light.)
- Post-processing. The tone-mapping pass with its exposure, and a bloom: a chain of downsampled HDR
  targets, upsampled and added in the tone-mapping pass, so that no blending is needed. (Done at
  M3b.7, ADR-0040: energy-conserving, without a threshold, the upsampled chain a running average
  in a second chain of targets.)
- Render graph. Extracted at the end of the stage from the passes that exist then (shadow, scene,
  sky, bloom, tone mapping), as the roadmap asks: passes declare the targets they read and write,
  the graph owns the targets that live for one frame and orders the passes; no memory aliasing, no
  asynchronous compute. (Done at M3b.8, ADR-0041: a private module of `slime-render`, built again
  when the frame's shape changes, recording the passes four to a submission.)
- GPU layer. `slime-gfx` gains what the passes need, when they need it, without wgpu outside it
  (ADR-0016): float render targets, targets and depth targets that shaders sample, pipelines
  without vertex buffers, passes without color targets, comparison samplers and depth bias in
  M3b.1, for the HDR and shadow passes; cube textures and mip levels given by the caller in M3b.6,
  with IBL, their first user (amending ADR-0032).
- Loader and asset. The temporary glTF loader reads the PBR textures, tangents (computed per
  triangle when the file has none) and lights; `make_scene.py` draws the new textures in code and
  exports tangents and lights; the asset stays under CC0 and under 500 kB.
- No new dependency. Half-precision floats are converted by hand, as bytes are packed by hand
  (ADR-0026); the procedural sky needs no image decoder.
- Windows. The Windows CI job runs the tests and the headless samples at every milestone; the owner
  checks windowed behaviour in the virtual machine at M3b.2, M3b.5 and M3b.9.

Risks, to measure as the stage goes: shadows flip whole pixels where drivers rasterize an edge
differently, which the per-channel tolerance of the image tests cannot absorb (ADR-0028), so the
tolerance may need a measured amendment at M3b.5 (measured at M3b.5: they do not, and ADR-0030's
tolerance stands, ADR-0038); every image test renders more passes on lavapipe and WARP, which
lengthens CI; the CPU precomputation of IBL runs in every process that draws the sample, tests
included, and must stay small (measured at M3b.6: 38 ms for the sample's sky and the BRDF table in
a release build on the owner's 16 threads, about 180 ms for the sky in a debug build, ADR-0039);
upgrading wgpu still waits for egui.

### M3b.1 — GPU layer for multi-pass rendering

- **Goal.** What the passes of the stage need from `slime-gfx` to draw into targets that later
  passes read, proven offscreen on lavapipe and WARP.
- **Crates.** `slime-gfx`; the callers of what it changes (`slime-render`, `slime-devui`, the
  examples and the tests).
- **Delivers.** The `Rgba16Float` format for render targets, read back to the CPU as floating-point
  values; render targets and depth targets that shaders sample through bind groups once a pass has
  drawn into them, while a window's image cannot be sampled; pipelines without a vertex buffer, for
  a fullscreen triangle built from the vertex index; passes and pipelines without a color target,
  which write depth only; comparison samplers, and a depth bias on pipelines; several passes timed
  in one frame, each time read back on its own. ADR: the GPU layer for multi-pass rendering
  (amends 0017 and 0026).
- **Done when.** `slime-gfx` tests pass on lavapipe and on WARP: a float target keeps values above
  1 and below 0 and reads them back as half-precision floats represent them; a fullscreen triangle
  drawn without a vertex buffer covers every pixel; a second pass samples the target a first pass
  drew into; a depth-only pass writes depths that a later pass reads through a comparison sampler,
  which gives 0 on one side of the reference depth and 1 on the other; a depth bias moves a
  surface's depth by the expected amount; three passes timed in one frame each get a time.
- **How to test.** `VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" cargo test -p slime-gfx`.
- **Checked** on 2026-10-13: ADR-0033 records the API and the alternatives rejected.
  `crates/slime-gfx/tests/passes.rs` (a fullscreen triangle without a vertex buffer covering a
  67 × 5 float target; a pass sampling a float target an earlier pass drew, mirrored, then copied
  into an sRGB target; a depth-only pass into a shadow map twice the size of the target that reads
  it through comparison samplers, with the value at a shadow's edge and a reference equal to the
  stored depth; the constant bias at a depth whose exponent matters, and the slope-scaled bias;
  the misuses that wgpu refuses: a pass sampling its own target or depth target, a pipeline in a
  pass of the other kind, a resource bound at a binding of another kind), `readback.rs` (float
  targets of six sizes cleared to values beyond 0 to 1), a unit test of `image.rs` (rows read back
  in bands, as targets larger than the largest buffer are), `timing.rs` (three passes of a frame,
  one of them depth-only, timed each) and `errors.rs` (pipelines without vertices or without a
  target, bindings of another kind than their layout in both directions, the read and texture
  asserts) pass on lavapipe, on the owner's RX 6900 XT (RADV) and, in the Windows CI job, on
  WARP. On all three, the biases are exactly the amounts the formula gives, and a comparison
  halfway between a shadowed and a lit texel gives 0.5. The passes of the timing test took 241,
  165 and 133 µs on lavapipe, 2.4, 1.2 and 1.1 µs on RADV, 36, 39 and 17 µs on WARP. No example
  uses these features yet: the `scene` example does from M3b.2 on. The Windows job took 8 minutes
  40 seconds, the Linux one 3 minutes 23 seconds.

### M3b.2 — HDR rendering and tone mapping

- **Goal.** The scene is drawn in high dynamic range and mapped to the display by a tone-mapping
  pass with an exposure.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** The renderer draws the scene into an `Rgba16Float` target with its depth target,
  both following the output's size, then a fullscreen pass samples it, multiplies it by the
  exposure, applies the tone-mapping operator chosen in the ADR, and writes the output target (the
  window's image, or an offscreen sRGB one); `Scene` gains the exposure; `RenderStats` gives the
  GPU time of each pass; the `scene` example shows them in the overlay and lets the user change the
  exposure; the references of the image tests are rendered again on lavapipe and looked at; the
  lighting and material tests of `slime-render` check their values through the tone-mapping
  curve. ADR: HDR rendering and tone mapping (amends 0027: several passes, the scene in HDR).
- **Done when.** The image tests pass on the three drivers; the sample looks right by eye on niri
  and in a screenshot of the headless Weston; the owner ran `scene.exe` in the Windows virtual
  machine.
- **How to test.** `cargo run --example scene`; `cargo test -p slime-examples`.
- **Checked** on 2026-10-13: ADR-0034 records the operator (Khronos PBR Neutral), the place of the
  exposure and the alternatives rejected. `crates/slime-render/tests/tone_mapping.rs` (the tests'
  copy of the curve against nine values derived by hand from the specification; the pass itself,
  read back from an `Rgba16Float` output within 0.001; the exposure applied before the curve; the
  targets following the output through four sizes; an sRGB output encoding the tone-mapped
  light), the lighting and material tests, whose expected values now go through the curve, and
  the new lighting tests (the exposure scaling the light that surfaces reflect, the projection
  following a 64 × 32 output, the GPU time of each pass) pass on lavapipe and on the owner's
  RX 6900 XT (RADV). The references of the image tests were rendered again on lavapipe and looked
  at: lavapipe matches them with no channel off by more than 2. On RADV, `gltf-front` has 121
  pixels off by more than 2 in a channel, 8 by more than 4, and differs by at most 7;
  `gltf-back` 26, 1, at most 5; `shapes` 612, 302, 50 off by more than 8 (65 allowed, 14 at
  0.0.3) and one by more than 16, at most 19: the curve steepens the differences in edge pixels
  that 0.0.3 already had. In the lighting test, the scene pass took 7.2 µs and the tone-mapping
  pass 2.2 µs on RADV. The `scene` example ran 400 frames in a headless Weston at 1280 × 720 on
  lavapipe, each pass taking 0.8 to 0.9 ms, and a screenshot of it was looked at. In the Windows
  CI job, on WARP, every test passes and the references differ by at most 5 (`gltf-front`: 42
  pixels off by more than 2, 3 by more than 4; `gltf-back`: 20 and 0; `shapes`: 82 and 0); the
  job took 8 minutes 40 seconds, the Linux one 3 minutes 51 seconds. In the owner's Windows 11
  virtual machine (RX 6900 XT, Direct3D 12, display scale 150 % and 125 %), on 2026-10-13, after
  M3b.3: the image, the two GPU times, the exposure slider (Escape and F1 still work while it is
  held) and the overlay at both scales are as expected. The owner looked at the sample on
  Hyprland, the other compositor of the owner's desktop, on 2026-10-13, after M3b.3: it looks
  right.

### M3b.3 — Physically based materials

- **Goal.** Surfaces shaded with glTF's metallic-roughness model.
- **Crates.** `slime-render`; `examples` (the loader, the asset and its Blender script, the image
  tests).
- **Delivers.** `MaterialData` gains the metallic and roughness factors and their texture, a normal
  texture and its scale, an occlusion texture and its strength, the emissive factor, strength and
  texture, the alpha mode (opaque, or masked with a cutoff) and double-sidedness; `MeshData` gains
  tangents; the shader evaluates the BRDF for the directional light, with the camera's position in
  the frame's values; default textures stand in for the slots a material leaves empty; the loader
  reads all of it, creates each texture in its slot's format, and reads tangents or computes them;
  `make_scene.py` draws the new textures and exports tangents; the shapes of the image test gain
  spheres of increasing metallic and roughness. ADR: physically based materials (amends 0027 and
  0029). The pipeline is chosen per draw from the material's double-sidedness and from the sign of
  the determinant of the instance's transform (no culling, or culling the back or the front
  faces), so that mirrored instances keep their front faces, as glTF does; the examples' loader
  then stops reversing the winding of mirrored nodes. The lists in ADR-0029
  of what the loader reads, refuses and warns about predate several of its current checks: images
  larger than the GPU accepts, primitives too large for its buffers, malformed image data and
  accessors, node hierarchies that are not trees, the vertex-color warning, the reversed winding
  of mirrored nodes, and the minification filters it approximates. Until this milestone's ADR
  brings them up to date, the module comment of `examples/common/gltf.rs` is authoritative.
  Tangents make the renderer's vertex larger than 32 bytes, so the `# Errors` section of
  `Renderer::create_mesh`, which gives the largest mesh in vertices, and the loader's
  `check_buffer_sizes`, which repeats that size, change with them.
- **Done when.** Tests that read pixels back: a white dielectric lit at normal incidence gives the
  value the BRDF predicts, computed in the test; a rougher surface spreads its highlight; a metal
  reflects in its base color; a normal map tilts the shading as a tilted surface would; a masked
  material discards fragments below its cutoff. The image tests pass on the three drivers; the
  sample looks right by eye.
- **How to test.** As M3b.2.
- **Checked** on 2026-10-13: ADR-0035 records the material model, the light's unit, the pipelines
  and the alternatives rejected; the owner took the choices it lists.
  `crates/slime-render/tests/shading.rs` reads `Rgba16Float` outputs back and compares them within 2
  × 10⁻³ with what a CPU copy of the BRDF (`tests/common/brdf.rs`, checked against closed forms)
  predicts: a white dielectric at normal incidence, a highlight spreading with roughness, a metal
  reflecting its base color, the channels of the metallic-roughness texture, a normal texture
  tilting the shading as a turned square does (green up the image), masks by factor and by texture,
  double-sided back faces, mirrored instances with their normal textures and a pipeline switch in
  one frame, occlusion and emission, the clamp at 65 504, and the panics on a texture in the wrong
  format; mutating the shader's handedness or facing makes it fail. The unit tests check the
  material's packing, the tangents of a mesh, `cull_mode` and `is_mirrored` (3600 meshes flattened
  between two rotations, none mirrored); the loader's tests check computed and read tangents, masks,
  blending, double sides, required extensions, a texture in two slots and coordinate sets in every
  slot. They pass on lavapipe, on the owner's RX 6900 XT (RADV), on WARP in the Windows CI job, and
  on lavapipe with glam's assertions on. The references were rendered again on lavapipe and looked
  at. On RADV, `gltf-front` has 222 pixels off by more than 2, 78 by more than 4, 7 by more than 8
  (65 allowed), largest 26; `gltf-back` 25, 0, 0, largest 4; `shapes` 588, 278, 36 (50 at M3b.2),
  largest 18. On WARP, the references differ by at most 5 (`gltf-front`: 52 pixels off by more than
  2, 1 by more than 4; `gltf-back`: 15 and 0; `shapes`: 48 and 0); the Windows job took 8 minutes 28
  seconds, the Linux one 3 minutes 51 seconds. The scene pass took 5.8 µs on RADV in the lighting
  test. The asset, regenerated with Blender 5.2 from `make_scene.py`, is 127 296 bytes, and every
  item that the design checks in it was found. The `scene` example ran 400 frames in a headless
  Weston on lavapipe, and a screenshot was looked at: the glowing ring, the iron frames of the
  crates, the metal pillar, the bricks' relief. In the owner's Windows 11 virtual machine (RX 6900
  XT, Direct3D 12, shaders compiled with FXC), on 2026-10-13, every element shows, without a stray
  pixel, the floor stays visible from below, and the passes take 0.090 ms (scene) and 0.102 ms
  (tone mapping); from below, the bottoms of the wall and the crates flicker where they touch the
  floor (known debt). The owner looked at the sample on Hyprland on 2026-10-13: it looks right;
  the scene pass took 11.5 µs and the tone-mapping pass 6.8 µs there (RADV, 637 × 696).

### M3b.4 — Light types

- **Goal.** Several directional, point and spot lights per scene, in physical units.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** `Scene` holds a list of lights: directional (illuminance in lux), point (intensity
  in candela, range) and spot (intensity, range, inner and outer cone angles), up to a fixed
  maximum, in a uniform buffer; the falloff that `KHR_lights_punctual` recommends (the inverse
  square of the distance, brought smoothly to zero at the range) and its cone attenuation; the
  constant ambient light stays until IBL replaces it; the loader reads `KHR_lights_punctual`
  through the `gltf` feature of that name (`dependencies.md` updated, and the lists of ADR-0035
  amended), adds it to `READ_EXTENSIONS`, and a test loads a file that requires it; the asset
  exports its sun and a point or spot light; the sample's exposure suits physical units. The
  exposure is applied in the scene pass, before the HDR target is written (pre-exposure), so that
  light in physical units stays within half precision; the tone-mapping pass no longer multiplies
  by it (the owner's decision of 2026-10-13, amending ADR-0034). ADR: lights and their units, and
  pre-exposure.
- **Done when.** Tests that read pixels back: a point light gives a quarter of the light at twice
  the distance, and none beyond its range; a spot light lights fully inside its inner cone, not at
  all outside its outer cone, and in between smoothly; a scene with the maximum number of lights
  draws, and one with more is refused as documented. The image tests pass on the three drivers;
  the sample looks right by eye.
- **How to test.** As M3b.2.
- **Checked** on 2026-10-13: ADR-0037 records the lights' API and units, the values the shader
  reads, the falloff and the cone, pre-exposure, what the loader reads and refuses, and the sample's
  values; the owner chose how a light's strength is written (a color apart from it), the maximum
  (16, a documented panic beyond), the sample's sun (100 000 lux) and its two other lights (a point
  light in the ring and a spot light on the wall). `crates/slime-render/tests/lights.rs` reads
  `Rgba16Float` outputs back and compares them within 2 × 10⁻³ with what the CPU copy of the
  BRDF predicts under the extension's formulas, written in the test: a point light gives a quarter
  of its light at twice the distance, and exactly nothing at and beyond its range; a spot light is
  full within its inner cone, exactly dark beyond its outer one, and decreasing between; 16 lights
  of every kind, each of which adds at least 0.01, draw; 17 panic; a light on the surface it lights
  gives no infinity, and one 1 mm above it what it gives at 1 cm; a point light and an emission far
  above 65 504 are exposed before the target. `lighting.rs` checks the same for a directional light,
  `tone_mapping.rs` for the background, which the renderer also bounds by 65 504 (a unit test). The
  loader's tests load a file that requires `KHR_lights_punctual` and is lit by its own light only,
  place lights in world space along their node's −Z axis whatever its scale, and refuse the lights
  glTF forbids, the files the `gltf` crate would panic on, and 17 lights. A prototype of the same
  design was checked against 20 mutations of the shading and 7 of the loader, all caught but two
  that lavapipe and RADV cannot show: the background's bound, which the unit test catches, and the
  guard of a fragment on a light, kept because WGSL leaves NaN undefined. Every test passes on
  lavapipe, with glam's assertions on, and on the owner's RX 6900 XT (RADV). The shapes' and the
  renderer's tests kept their values: a directional light of π lux at an exposure of 1 draws the
  shapes' reference exactly. The glTF references were rendered again on lavapipe and looked at; on
  RADV, `gltf-front` has 218 pixels off by more than 2, 83 by more than 4, 3 by more than 8 (65
  allowed), largest 26; `gltf-back` 25, 0, 0, largest 4; `shapes` 588, 278, 36, largest 18, as at
  M3b.3. The scene pass took 18.5 µs without a light, 36.8 µs with 1, 97.2 µs with 4, 177.3 µs
  with 8 and 337.6 µs with 16, for a square covering a 1920 × 1080 output on RADV (release build,
  median of 40 frames): about 20 µs per light. The asset, regenerated with Blender 5.2 from
  `make_scene.py`, is 128 400 bytes and holds the three lights as designed. The `scene` example ran
  400 frames in a headless Weston at 1280 × 720 on lavapipe (scene pass 1.7 ms, tone mapping 1.0
  ms), and a screenshot was looked at: the ring's teal light on the floor, the crate and the pillar,
  the spot on the wall, the slider at EV100 14.7. In the Windows CI job, on WARP, every test passes,
  so FXC compiles the loop over the lights, and the references differ by at most 4 (`gltf-front`: 48
  pixels off by more than 2, none by more than 4; `gltf-back`: 15 and 0; `shapes`: 48 and 0); the
  job took 7 minutes 12 seconds, the Linux one 3 minutes 22 seconds. The owner looked at the sample
  on the desktop on 2026-10-13: it looks right; the scene pass took 0.090 ms and the tone-mapping
  pass 0.036 ms there (RADV, 3816 × 2082).

### M3b.5 — Shadows

- **Goal.** The first directional light casts shadows.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** A depth-only shadow pass into a sampleable depth target, from an orthographic
  projection fitted to the bounds of the instances; the scene pass reads it with 3 × 3 PCF through
  a comparison sampler, with a constant and a slope-scaled depth bias; `RenderStats` counts the
  shadow pass's draw calls and gives its GPU time. ADR: shadow mapping. Masked materials
  (ADR-0035) discard in a fragment stage, which depth-only pipelines lack (ADR-0033): this
  milestone decides whether masked casters cast solid shadows or depth-only pipelines gain a
  fragment stage.
- **Done when.** Tests that read pixels back: a square above a floor darkens a square of the floor
  of the expected size, with edges blended over the width of the filter; a lit floor shows no
  self-shadowing (shadow acne). The image tests pass on the three drivers, with a tolerance amended
  by a measured ADR if shadow edges require it; the sample looks right by eye; the owner ran
  `scene.exe` in the Windows virtual machine.
- **How to test.** As M3b.2.
- **Checked** on 2026-10-13: ADR-0038 records the projection, the map, the pass, the bias, the
  filter and the alternatives rejected; the owner chose cut-out shadows for masked materials, every
  face blocking the light, a `casts_shadows` switch on directional lights, and to keep the
  sample's sun. `crates/slime-render/tests/shadows.rs` compares every pixel of an `Rgba16Float`
  output with values computed from the geometry: a square 1.5 m above a floor, lit 45° from the
  normal, shadows the expected square, its edges ramping linearly over three texels, whether it
  faces the light or not, within 5.3 × 10⁻⁴ on lavapipe and 5.6 × 10⁻⁴ on RADV; a floor lit up to
  85° from its normal, along and across the map's axes, turned and 46 m from the origin, does not
  shadow itself; the first directional light that casts shadows casts them, and no other, and
  without one no shadow pass runs; a masked square casts no shadow where it discards. Unit tests
  check the projection's fit and its degenerate cases, and a test of `slime-gfx` draws a depth-only
  pass whose fragment stage discards. The bias was chosen by measurement: a slope scale of 2 left
  floors darkened by up to 0.054 where the light comes diagonally to the map's axes, 4 leaves
  half-precision rounding (5.3 × 10⁻⁴), the same on lavapipe and RADV; with the light-space
  position computed per vertex, as the shadow pass computes it, a constant of 4 already removes
  every speck, and 16 is kept. A prototype of the design was checked against 21 mutations (bias,
  filter, map orientation, light chosen, culling, projection), each caught. Every test passes on
  lavapipe, with glam's assertions on, and on the owner's RX 6900 XT (RADV). The references were
  rendered again on lavapipe and looked at (the shadows of the crates, the sphere, the pillar and
  the wall); on RADV, `gltf-front` has 219 pixels off by more than 2, 83 by more than 4, 3 by more
  than 8 (65 allowed), largest 26; `gltf-back` 25, 0, 0, largest 4; `shapes` 589, 278, 36, largest
  18: one pixel more than without shadows, so ADR-0030's tolerance stands. On RADV, in a prototype,
  the shadow pass took 8.9 µs for the sample at 1280 × 720, the scene pass's lookups 11.4 µs over a
  1920 × 1080 output (about half a light), and masked materials' shadow pipeline 28.5 µs instead of
  6.5 µs for a square covering the map; the sampling usage of depth targets cost nothing measurable
  (ADR-0033's correction). The `scene` example ran 400 frames in a headless Weston at 1280 × 720
  on lavapipe (shadow pass 2.6 ms, scene pass 5.4 ms, tone mapping 2.0 ms), and a screenshot was
  looked at: the shadows of the crates, the sphere and the wall, and the overlay's shadow time and
  draw calls. In the Windows CI job, on WARP, every test passes, so FXC compiles the shadow lookup,
  and the references differ by at most 4 (`gltf-front`: 49 pixels off by more than 2, none by
  more than 4; `gltf-back`: 15 and 0; `shapes`: 48 and 0); the shadow, scene and tone-mapping
  passes of the statistics test took 29.9, 34.8 and 20.6 µs; the job took 10 minutes 3 seconds,
  the Linux one 4 minutes 24 seconds. In the owner's Windows 11 virtual machine (RX 6900 XT,
  Direct3D 12, FXC, display scale 150 % and 125 %), on 2026-10-13: the shadows of the crates, the
  sphere, the pillar and the wall touch their objects, show no acne from far, near or grazing
  views, and their softened edges do not flicker while the camera turns; the ring's teal light
  and the warm spot show; the overlay gives the three GPU times (shadow 0.063 to 0.066 ms, scene
  0.32 ms, tone mapping 0.10 ms) and 7 draw calls for shadows, and is sharp at both scales; the
  exposure slider, F1 and Escape work, and `--frames 300` ends the process with code 0. Through
  Vulkan on the same machine, the shadows are the same (shadow pass 53 µs).

### M3b.6 — Image-based lighting

- **Goal.** Surfaces lit by their surroundings: a procedural sky and its reflections.
- **Crates.** `slime-gfx` (cube textures, mip levels given by the caller, `Rgba16Float` textures
  uploaded from the CPU); `slime-render`; `examples`.
- **Delivers.** `slime-gfx`: cube textures, and textures whose mip levels the caller gives, in
  `Rgba16Float` among other formats, with half-precision floats converted by hand (amends ADR-0032).
  `slime-render`: a procedural sky (a gradient and a sun) drawn on the CPU into a small cube map,
  its irradiance and its prefiltered specular levels, and the table of the BRDF's scale and bias,
  computed at startup; a sky pass draws the environment behind the scene instead of the clear color,
  its light exposed as the scene pass exposes surfaces (ADR-0037); IBL replaces the constant ambient
  light. Bind group 3 holds the shadow map, the last of wgpu's default four (ADR-0038): the
  environment's textures join group 3 or group 0. ADR: image-based lighting computed on the CPU.
- **Done when.** Tests that read pixels back: a white rough sphere inside a uniform environment
  reflects that environment's radiance (the furnace test); a mirror sphere reflects the sky; the
  CPU precomputation of the sample stays under a budget measured and recorded here. The image
  tests pass on the three drivers; the sample looks right by eye.
- **How to test.** As M3b.2.
- **Checked** on 2026-10-13: ADR-0039 records the sky, the precomputation, the binding, the sky
  pass, the shading and the alternatives rejected, from three prototyped designs; the owner chose a
  sun disc and its glow described in the sky, a clear blue sky over dark soil for the sample, and
  faces of 128 texels. The budget: the sample's environment and the BRDF table take at most 50 ms on
  every core in a release build on the owner's machine (Ryzen 7 5800X, 16 threads); measured with
  its desktop busy, 23 ms (20 to 28) and 15 ms, 162 ms and 135 ms on one core, with `cargo test
  --release -p slime-render --lib -- --ignored --nocapture`. Unit tests compare the sky's formula
  with values computed by hand, the glow's integral and the disc's radiance with their closed forms,
  every prefiltered level with an integral over every texel of the first (2.2, 1.4, 1.1 and 0.31 %
  at most, the largest in the sun's glow), the roughest level with the irradiance of a linear sky
  and of the sample's (0.43 %), the BRDF table with a Gauss-Legendre quadrature, the uniform sky's
  shortcut with the computation, and the texels' directions with the table of
  `Gpu::create_cube_texture`. `crates/slime-render/tests/environment.rs` reads every pixel back from
  an `Rgba16Float` output: white spheres of roughness 0 to 1, metal and dielectric, vanish into a
  uniform sky (the furnace test, within 2.8 × 10⁻⁴); a black dielectric and a grey metal
  reflect what the split sum predicts from an independent integration (2.8 × 10⁻³); a mirror
  sphere reflects the sky's formula (8.7 × 10⁻⁴); a matte sphere shows the sky's irradiance
  (1.9 × 10⁻³); rough metals show the sky under their lobe (3.9 × 10⁻³); the sky pass shows
  the sky and the sun's glow along every view ray (5.1 × 10⁻⁴), the same on lavapipe and RADV;
  an environment draws the same with another renderer, and a scene without one is black.
  `tone_mapping.rs` checks that the sky is exposed before the target, its value beyond 65 504, and
  that the sun's disc is drawn once at its radiance and hidden below the horizon. `slime-gfx` checks
  every half and every midpoint between halves through `f32_to_f16`, half-precision levels read back
  exactly, and every texel of a cube in its documented direction. A prototype of the design, with
  another sky and not kept, was checked against 43 mutations: 42 were caught, and the sky pass's
  clamp at 65 504 cannot be shown on lavapipe and RADV, which clamp themselves. Every test passes on
  lavapipe, with glam's assertions on, and on RADV. The references were rendered again on lavapipe
  and looked at (the sky, blue shadows, metals reflecting it); on RADV, `gltf-front` has 223 pixels
  off by more than 2, 82 by more than 4, 4 by more than 8 (65 allowed), largest 25; `gltf-back` 25,
  1, 0, largest 5; `shapes` 554, 284, 49, largest 19: ADR-0030's tolerance stands, `shapes` nearer
  its limit (49 of 65, 36 at M3b.5), where its metal spheres reflect the sky. On RADV at 1920 ×
  1080, the sample's passes take 9.0 µs (shadow), 21.6 µs (scene), 14.6 µs (sky) and 10.2 µs
  (tone mapping) from the start pose, 9.0, 14.6, 15.2 and 10.0 µs from a low pose. The `scene`
  example ran 400 frames in a headless Weston at 1280 × 720 on lavapipe (sky computed in 179 ms in
  a debug build; shadow pass 2.0 ms, scene 3.0 ms, sky 1.1 ms, tone mapping 0.9 ms), and a
  screenshot was looked at: the sky, the blue shadows, the overlay's sky lines. A reviewing agent
  read the milestone against `AGENTS.md`: the sky's directions were divided by a w that rounding
  takes to zero for a far plane 10⁷ times the near one, and the sun's disc showed below the
  horizon; both were fixed, with the tests and documents it found missing or wrong. In the
  Windows CI job, on WARP, every test passes, so FXC compiles the environment's lookups and the
  sky pass, and the references differ by at most 5 (`gltf-front`: 41 pixels off by more than 2,
  1 by more than 4; `gltf-back`: 17 and 0; `shapes`: 53 and 0); the sky took 0.93 s to compute in
  that job's debug build; the job took 11 minutes 56 seconds (10 minutes 3 seconds at M3b.5), the
  Linux one 5 minutes 17 seconds (4 minutes 24 seconds). The owner looked at the sample on the
  desktop on 2026-10-14: the sky and the sun look right.

### M3b.7 — Bloom

- **Goal.** Bright parts of the image glow.
- **Crates.** `slime-render`; `examples`.
- **Delivers.** A chain of HDR targets, each half the size of the one before, filled by
  downsampling the scene, then upsampled back and added to the scene in the tone-mapping pass, with
  a strength; the sample's asset has an emissive object bright enough to bloom (its ring already
  emits at a strength of 4, M3b.3); the overlay shows
  the chain's GPU time. ADR: bloom.
- **Done when.** Tests that read pixels back: a single bright pixel spreads into a symmetric halo,
  and a dark image is unchanged. The image tests pass on the three drivers; the sample looks right
  by eye.
- **How to test.** As M3b.2.
- **Checked** on 2026-10-14: ADR-0040 records the strength, the chain, the filters, the edges, the
  headroom of the high-dynamic-range target and the alternatives rejected, from three prototyped
  designs and a model of the chain that compared their treatments of the image's edges; the owner
  chose a strength of 0.04 for the samples and to keep light beyond 65 504 for the bloom. The asset
  does not change: its ring glows faintly at 0.04, and a brighter ring whitens and floods the floor
  rather than glowing. `crates/slime-render/tests/bloom.rs` reads `Rgba16Float` outputs back: a
  single bright pixel's halo is symmetric about the diagonal through it, and two neighbouring
  pixels' halos mirror each other within 10⁻³; on a 257 × 131 output the halo falls with the
  distance and keeps its shape, within 3 %, as the pixel moves; it does not wrap around; uniform
  images of light 0 to 4 are unchanged at a strength of 1 from 1 × 1 to 64 × 64; a strength of 0
  skips the passes, and outputs below 15 × 15 show no bloom at any strength; the chain follows the
  output's size; every pixel of a halo, inside and at corners of 256 × 256, 257 × 131 and 64 × 64
  outputs, matches a CPU model of the chain within 3.9 × 10⁻³ relative, the same on lavapipe and
  RADV; the model keeps a pixel's light at every edge of a 64 × 64 output within 3 × 10⁻³; a
  pixel of 4 × 65 536 keeps its halo. Clamping the sampler to the edge, and removing the headroom,
  each fail a test. Prototypes of the designs were checked against 16 and 30 mutations (all but a
  threshold of 0.01, invisible, caught). Every test passes on lavapipe, with glam's assertions on,
  and on RADV. The references were rendered again on lavapipe and looked at (a soft glow around the
  ring and bright edges); on RADV, `gltf-front` has 200 pixels off by more than 2, 72 by more than
  4, 3 by more than 8 (65 allowed), largest 24; `gltf-back` 19, 0, 0, largest 4; `shapes` 540, 259,
  33, largest 18: the bloom averages the drivers' edge differences, and `shapes` is back from 49 to
  33. On RADV, the sample's bloom takes 34 µs at 1280 × 720 and 51 µs at 1920 × 1080, and the
  tone-mapping pass 9 and 20 µs more; on lavapipe 5.3 and 10.1 ms, and 3.4 and 7.4 ms; the CPU time
  of `render` grows by 0.51 and 0.63 ms, the passes' submissions and timers. The workspace's tests
  take 26.2 s on lavapipe (23.9 s at M3b.6). The `scene` example ran 400 frames in a headless Weston
  at 1280 × 720 on lavapipe, and a screenshot was looked at: the ring's glow, the overlay's bloom
  time and slider. A reviewing agent read the milestone against `AGENTS.md`: the edges' figures
  were too optimistic and a frame too small for a chain was untested; both were corrected, with
  the pipelines' count. In the Windows CI job, on WARP, every test passes, so FXC compiles the
  bloom's shaders, and the references differ by at most 4 (`gltf-front`: 40 pixels off by more than
  2, none by more than 4; `gltf-back`: 13 and 0; `shapes`: 37 and 0); the job took 11 minutes 48
  seconds, the Linux one 4 minutes 41 seconds. The owner looked at the sample on the desktop on
  2026-10-14: the glow and the sun look right.

### M3b.8 — Render graph

- **Goal.** The passes of a frame are declared and ordered by a minimal render graph, extracted
  from the passes that exist.
- **Crates.** `slime-render`, unless the ADR shows that the graph belongs in `slime-gfx`.
- **Delivers.** Passes declare the targets they read and write; the graph creates the targets that
  live for one frame at the output's size or a fraction of it, orders the passes by what they read
  and write, records them, and reports each pass's statistics; the renderer's frame (shadow,
  scene, sky, bloom, tone mapping) is built as a graph. ADR: the render graph.
- **Done when.** Tests of the graph without a GPU: passes are ordered by their dependencies, a read
  of a target that no pass writes and a cycle are refused; the image tests give the same pixels on
  lavapipe as before the extraction; the CPU time of a frame is not worse, measured before and
  after.
- **How to test.** `cargo test -p slime-render`; the image tests.
- **Checked** on 2026-10-14: ADR-0041 records where the graph lives, what passes and targets
  declare, the order and the refusals, how a frame is submitted and timed, and the alternatives
  rejected, from three prototyped designs. The owner chose four passes per submission rather than
  one per frame, once one submission of a whole bloomed frame was measured stalling in the kernel
  when the RX 6900 XT's 256 MiB window of CPU-visible memory was filled to 241 MiB (a mean 2.4 ms,
  8 ms at the 90th percentile, at 1920 × 1080; submissions of 2 to 6 passes stable, 8 not); to
  keep the five GPU times of `RenderStats`; and to free the bloom's levels while it is off.
  Without a GPU, `graph.rs`'s tests order a frame declared backwards, keep the order of passes
  free to run in any order, chain the passes that draw into one target before its readers, let an
  imported target be read and kept without a writer, refuse each `GraphError` by value (two
  transient targets with one label, a read or a keep of an unwritten transient target, a clear of
  a drawn target, a pass sampling its own target, a cycle with the passes in it, an unused target)
  and size transient targets within half a texel; with a GPU, a frame of six passes in two
  submissions is recorded in its order into targets of its size, each pass timed, and the next
  shape reuses the targets of the same label, format and size only. Every shape of the renderer's
  frame (five sizes, shadows and bloom on and off) compiles into the order it had; each pass's GPU
  time folds into its field; a renderer whose bloom stops and resumes and whose output grows and
  shrinks back draws what a new renderer draws, byte for byte, and gets its times back. In
  `slime-gfx`, the passes of one submission run in the order recorded, a target counts as drawn
  once submitted, one timer times three passes of a submission apart, and a submission of two
  passes on a lost device does not panic. Mutations of the timer (every pass writing the first
  pair, a pass fewer resolved, a pass more timed), of the reuse of targets (the format ignored, no
  reuse) and of the timers' creation each fail a test, as did the prototypes' 17 and 30 of 32
  mutations. Every readback of every test of the workspace (317) and 300 frames (shadows on and
  off, an environment or none, bloom strengths of 0, 0.04 and 1, ten output sizes, sRGB and float
  outputs) hash identically, byte for byte, before (cc4d35d) and after (b81fdd5), on lavapipe and
  RADV. The CPU time of `render`, on the RX 6900 XT (Ryzen 7 5800X, RADV of Mesa 26.2.4), drawing
  the sample offscreen in a release build, three alternated rounds of 300 frames, with the
  CPU-visible window as the desktop left it (201 MiB) and filled to 241 MiB: 0.71 → 0.33 ms at
  1280 × 720 with the bloom, 0.81 → 0.38 ms at 1920 × 1080, 0.22 → 0.11 ms without it, the 90th
  percentiles below 0.5 ms; a frame that changes shape costs 0.26 ms more when the bloom starts,
  0.12 ms when it stops and 0.28 ms for a resize (0.10 ms before). GPU times do not change. A
  capture of the `scene` example with RenderDoc 1.46, in a headless Weston through Xwayland, shows
  a 1280 × 720 frame as four submissions of 4, 4, 4 and 3 passes, each ending with its timer's
  copy, then the overlay's. The workspace's tests take 25.5 s on lavapipe (26.2 s at M3b.7). A
  reviewing agent read the milestone against `AGENTS.md`: a renderer whose shape changes at every
  call lost its tone-mapping and bloom times without a word (now documented, with the known debt),
  ADR-0034, 0038 and 0039 were not marked amended, the GPU times' return after a change of shape
  was misstated, passes' and submissions' counts held only at common sizes, two transient targets
  with one label could have shared a texture (now refused), and the shape changes and the reuse of
  targets were untested; all were corrected. In the Windows CI job, on WARP, every test passes,
  one timer times a submission's passes apart, and the references differ as at M3b.7
  (`gltf-front`: 40 pixels off by more than 2, none by more than 4; `gltf-back`: 13 and 0;
  `shapes`: 37 and 0); the job took 11 minutes 56 seconds, the Linux one 5 minutes 35 seconds. The
  owner ran the `scene` example on RADV on 2026-10-14: every pass's GPU time shows in the overlay.

### M3b.9 — Windows check, documentation, release 0.1

- **Goal.** 0.1 released.
- **Crates.** All; CI.
- **Delivers.** The Windows checklist run by the owner in the virtual machine; the documentation
  reread by reviewing agents; `CHANGELOG.md`; the duration of CI measured; the `v0.1.0` tag on a
  commit whose CI is green on both platforms.
- **Done when.** The checks are green on both platforms; the tag is pushed.
- **How to test.** Push and read the CI run; the Windows artifact in the virtual machine.

## Releasing

A release is an annotated git tag on a commit whose CI run is green on Linux and Windows
([ADR-0004](adr/0004-workspace-layout-and-crate-naming.md)):

1. In one commit, set the version in the root `Cargo.toml`, rename the `[Unreleased]` section of
   `CHANGELOG.md` to `[<version>] - <date>`, and update this plan.
2. Push that commit and wait for its CI run to pass.
3. Tag that commit, not a later one, then push the tag:
   `git tag -a v<version> -m "SlimeEngine <version>" <commit>`.

0.0.1 was released on 2026-10-12, before Windows joined CI: the tag `v0.0.1` is on the commit
"Release 0.0.1". 0.0.2 was released the same way on 2026-10-12: the tag `v0.0.2` is on the
commit "Release 0.0.2", and 0.0.3 on 2026-10-13: the tag `v0.0.3` is on the commit
"Release 0.0.3".

## Completed stages

| Stage | Milestones | Result |
|---|---|---|
| 0 — Foundations | M0.1 repository and ADR process; M0.2 Nix environment; M0.3 workspace, lints, crate skeletons; M0.4 dependency policy and CI | Done. CI green on the first push to GitHub (2026-10-12). |
| 1 — Core and platform → 0.0.1 | M1.1 logging and diagnostics; M1.2 frame timing; M1.3 lifecycle and headless mode; M1.4 window; M1.5 input; M1.6 clean shutdown | Done; released as 0.0.1 (tag `v0.0.1`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 2 — GPU layer → 0.0.2 | M2.0 Windows in CI; M2.1 GPU device without a window; M2.2 offscreen triangle and reference image; M2.3 the triangle in the window; M2.4 frame pacing; M2.5 debug markers, robustness, release | Done; released as 0.0.2 (tag `v0.0.2`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |
| 3a — First renderer → 0.0.3 | M3.1 GPU resources for a renderer; M3.2 the render crate; M3.3 directional light and material; M3.4 textures and samplers; M3.5 a glTF scene and camera control; M3.6 statistics, GPU timings and the dev overlay; M3.7 Windows check, documentation, release | Done; released as 0.0.3 (tag `v0.0.3`). Content listed in [`CHANGELOG.md`](../CHANGELOG.md). |

The details of each completed milestone (what it delivered, how it was checked, what was measured)
are in the history of this file and in the commits that implemented it; the measurements that
justify a decision are also in its ADR.

## Known debt

Limitations known and accepted for now. The ones that a contributor could take on alone are also
GitHub issues, linked from their entry; this list stays the reference.

- **Frames are not synchronized with the display's refresh** (ADR-0021; issue
  [#1](https://github.com/HoneySlatt/SlimeEngine/issues/1)). With Mailbox, the default, the
  schedule alone paces frames: on a 60 Hz display, an image is now and then shown twice while the
  next one is dropped. Aligning frames on the compositor's frame callbacks is to reconsider when a
  renderer draws real scenes.
- **Without Mailbox, presenting can still stall the loop.** Windows that do not offer it, and
  applications that choose `PresentMode::Fifo`, wait for the display: one frame per second for a
  hidden window on niri. In a headless Weston with lavapipe, `acquire` waits for Weston's frame
  callbacks for a time that depends on the machine's load: 120 frames of the `window` example took
  15 s on 2026-10-12 (release build, waits of 130 ms on average), 9 s and 25 s in other
  measurements the same day, against 2 s with Mailbox.
- **A hidden window keeps drawing** at the scheduled rate (2 to 3 % of one CPU core on the
  development machine): the engine cannot tell on Wayland whether its window is visible. winit
  reports occlusion on X11, which the engine ignores: under a compositing manager, Xwayland
  included, the X server reports windows as visible anyway.
- **wgpu's errors panic, in release builds too** (ADR-0016): a use of wgpu that it rejects is
  treated as a bug. Before games ship, logging the error and dropping the frame is to be
  considered. Running out of GPU memory panics as well on the paths where wgpu does not treat it as
  the loss of the device (ADR-0022). The engine avoids the rejections that the environment can
  cause: a window larger than the device's largest image is drawn at that size (`Surface::resize`),
  and a shader or pipeline that the driver rejects is returned as an error (`GfxError::Shader` or
  `GfxError::Pipeline` on Direct3D 12; on Vulkan, wgpu treats the driver's failure as a lost
  device, `GfxError::DeviceLost`); a window's surface that the graphics API refuses to configure
  counts as lost (ADR-0025). The refusal of a configuration is not tested: nothing the tests
  control makes a conformant driver refuse one.
- **A lost GPU device is not replaced** (ADR-0022; issue
  [#2](https://github.com/HoneySlatt/SlimeEngine/issues/2)): the run stops with an error, and
  since winit allows one event loop per process, carrying on means starting the process again.
- **Captures in RenderDoc were not tried on Windows** (Direct3D 12).
- **Frames pause on Windows while the user moves the window, holds its title bar or a border, or
  opens the system menu**: the system then runs a loop of its own, in which no redraw arrives and
  the engine's loop does not wake, so frames and the check for termination signals pause until it
  ends (ADR-0020; the move was measured in M2.4). Keeping frames going would mean asking for
  redraws from inside each frame and waiting there for the schedule.
- **wgpu-hal logs an error when an adapter cannot present to the window**, even though the engine
  then uses another one: seen with RADV in a headless Weston, where only lavapipe can present.
- **No title text in client-side decorations.** On compositors that do not draw decorations
  (GNOME), the title bar has no text, because winit's text renderer depends on an unmaintained
  crate (ADR-0011).
- **Input not reported yet** (issue [#3](https://github.com/HoneySlatt/SlimeEngine/issues/3)):
  input-method text, touch, touchpad gestures, gamepads, dropped files.
- **A compositor that stops answering blocks the engine.** Before the first frame, winit waits for
  the window's first configuration without a timeout, so a termination signal is noticed only when
  a second one ends the process; after `shutdown`, releasing the window or the surface can block
  too (seen with a Weston stopped by SIGSTOP).
- **On Direct3D 12, a covered window or a locked session was not measured.** In `acquire`, wgpu
  waits on the swap chain's frame-latency object, up to one second, if the compositor stops taking
  images; whether Windows does so for such windows is unknown. After that wait wgpu hands out the
  image anyway rather than reporting a timeout, so the frame is drawn late instead of skipped, and
  no debug line marks it: the stall shows only as a long `acquire` span
  (`SLIME_LOG=slime_app=trace`).
- **WARP's shader compiler crashes the process, now and then, when several threads compile
  shaders at once**, even on separate devices: an access violation (`STATUS_ACCESS_VIOLATION`)
  that other projects trace to `d3d10warp.dll`, and that no test can catch. Seen once in the
  Windows CI job, on 2026-10-13, in the texture tests of
  `slime-gfx`, after about ten green runs; other projects that test on WARP report the same crash,
  and that running the tests one at a time avoids it, which the Windows job does since, and which
  `CONTRIBUTING.md` asks of contributors on WARP. The engine's own code is not involved.
- **Client-side decorations run `dbus-send` twice when the window is created** (winit's
  sctk-adwaita, on compositors that do not draw decorations), each waiting up to 100 ms when the
  settings portal does not answer; the error they log when no session bus or portal answers is
  hidden by the default log filter (ADR-0019).
- **Upgrading wgpu waits for egui** while `slime-devui` exists: `egui-wgpu` is built against one
  wgpu version (egui 0.36 against wgpu 30 today), so wgpu moves only once egui follows
  (ADR-0031). Ends when the overlay is deleted.
- **The dev overlay blends in an sRGB framebuffer.** egui prefers a linear one for its blending,
  warns once at startup, and its panels look slightly lighter than egui's own demos (ADR-0031).
  Harmless for tooling.
- **Mipmaps are computed on the CPU when a texture is created** (`Gpu::create_texture`), on the
  calling thread, in work proportional to the texture's pixels (ADR-0032); a caller with levels of
  its own gives them to `Gpu::create_texture_with_levels` (ADR-0039). To move to the GPU, or to
  the asset pipeline of Stage 4, if textures grow.
- **Every offscreen render target and depth target can be sampled** (ADR-0033): they all carry the
  usage that allows it, which may cost some compression on some GPUs. Measured at M3b.5 on an RX
  6900 XT (RADV), with two builds that differ only by it: no cost (the scene pass took 43.9 µs
  with it and 44.0 µs without at 1920 × 1080, ADR-0033's correction). Other GPUs are not measured.
- **A pipeline that does not fit its pass, or a target sampled by the pass that draws into it,
  panics only when the commands of its submission are finished** inside `Gpu::submit` (or
  `Gpu::render`, `Gpu::render_depth`), with wgpu's message naming the pass, rather than at the
  faulty call (ADR-0033, ADR-0041). For the renderer's frame, the render graph refuses a pass that
  samples what it draws into before wgpu sees it.
- **On X11, losing the connection to the X server ends the process** with exit status 1, inside
  the X11 library (Xlib). `EventHandler::exiting`, `App::shutdown` and the return of `run` do not
  happen, although ADR-0010 promises that `shutdown` is called once `init` has been. winit
  installs no IO error handler. Installing one (`XSetIOErrorExitHandler`, libX11 1.7 or later)
  would need a new direct dependency such as `x11-dl`. On Wayland, a compositor that dies ends the
  loop cleanly.
- **Without libxkbcommon, the windowing library panics** rather than returning
  `PlatformError::Connect`. On X11 it panics at once, also without libxkbcommon-x11 or without the
  X server's XKB extension. On Wayland it panics when the compositor first reports a keyboard,
  which can be in the middle of a run. Turning it into an error would mean probing the libraries
  before winit does.
- **Hiding the window when the event loop ends was not seen on Windows** (2026-10-13). winit
  destroys windows only from inside the loop, so the engine hides the window when the loop ends.
  In the owner's Windows 11 virtual machine, closing an example ends the process in about 150 ms,
  too soon to tell the hiding from the end of the process. To see it: add a sleep after `run`
  returns in an example and check that the window disappears as soon as it is closed.
- **A termination signal received after `run` has returned no longer ends the process by itself**
  (ADR-0012). `ctrlc` cannot remove the handler that `run` installs. A first signal received then
  only stops later runs, and logs that the run stops after the current frame; only a second one
  ends the process, with exit code 130. This matters for a program that keeps working after
  `run`, for example under a service manager, which waits out its stop timeout. Ending the process
  at once when no run is active would amend ADR-0012.
- **A GPU device lost by a frame's own submission is checked by hand only** (ADR-0022). The
  `device-lost` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` destroys the device in
  the middle of a frame and checks that the run stops: no frame drawn after it, `shutdown` called
  once, and `run` returning `RunError::Gfx(GfxError::DeviceLost)`. The most likely loss in a
  windowed frame is one that a submission of `App::render` finds, between `Surface::acquire` and
  `SurfaceFrame::present`: wgpu 30 then refuses to present the image and keeps it, and on Vulkan,
  dropping the surface at the end of the run panicked in wgpu-hal. `present` now destroys the
  image's texture instead, which releases what wgpu-hal holds for it (it relies on wgpu-core 30's
  `Texture::destroy`, to check again when wgpu is upgraded). This was checked by hand on 2026-10-13
  in a headless Weston with lavapipe, with a temporary hook that destroys the device from
  `App::render`: before the fix the run panicked, after it `run` returns `DeviceLost`. A test cannot
  keep this check: a destroyed device records its loss only in a poll, and that poll releases the
  image itself.
- **A window frame dropped without being presented keeps one of the window's images on Vulkan.**
  wgpu 30 discards such a frame with wgpu-hal's `discard_texture`, which does nothing on Vulkan.
  The image stays acquired until the surface is configured again, and once none is left,
  `Surface::acquire` waits one second and returns `None`. `SurfaceFrame` documents this as a
  misuse, and nothing reports it; the engine always presents. A `Drop` that presents was rejected:
  it would show a black or half-drawn frame instead of skipping it, and present during unwinding.
- **An oversize window's surface is configured again at every frame on X11.** A window wider or
  taller than the device's largest image (8192 pixels with the default limits) gets images of
  that size, smaller than the window. On X11, Mesa's Vulkan drivers mark such a swapchain
  suboptimal, and every image of it too. `Surface::acquire` treats each suboptimal image as a
  reason to configure the surface again, at the same clamped size, so every frame waits for the
  GPU to finish and rebuilds the window's images. The frames are correct, and only `debug` logs
  show it; Wayland and Direct3D 12 are not affected. The fix is one line in the `Suboptimal` arm
  of `Surface::next_image`: do not set `stale` while `oversize` is set. It was written, then
  reverted for lack of a measurement. To measure, before and after: start Xvfb with a screen wider
  than 9000 pixels; with `WAYLAND_DISPLAY` unset, `DISPLAY` set to it and lavapipe, run the child
  of the `oversize` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` with
  `SLIME_LOG=wgpu_core=debug`; count wgpu-core's `configuring surface with` records (about one per
  frame without the fix, one with it) and note the frame times.
- **Termination signals are ignored for the rest of the process when `ctrlc` cannot start its
  thread.** `ctrlc` installs its handlers, then starts the thread that waits for what they catch.
  When that thread cannot start (a limit on threads reached when `run` installs the handler), the
  handlers stay installed and SIGINT, SIGTERM and SIGHUP (Ctrl+C and Ctrl+Break on Windows) are
  ignored: only SIGKILL, or closing the console on Windows, ends the process. The engine logs a
  warning that says so and gives the system's error. Restoring the default dispositions would need
  a direct dependency on libc or nix.
- **Any `trace` directive in `SLIME_LOG` makes wgpu build its per-command trace records.**
  tracing-subscriber sets the `log` crate's maximum level from the most verbose directive. With
  `slime_app=trace`, wgpu-core then formats and filters out about five records per draw while it
  encodes a pass, which counts in the caller's span (`render` in a windowed run). This is
  negligible for the samples. To measure it: read `RenderStats::cpu_time` for a scene of about
  1000 instances with `SLIME_LOG` unset, then with `slime_app=trace`. Alternatively, profile with
  `perf record -g` and look for `tracing_log::LogTracer::log`, `EnvFilter::enabled` and
  `wgpu_core::resource::Labeled::error_ident`. Capping the `log` level needs `log` or
  `tracing-log` as a direct dependency (ADR-0006). Span close lines are also written while the
  enclosing span is entered, so `frame`, `render` and, with `slime_gfx=trace`,
  `RenderStats::cpu_time` include them: take timing baselines with `SLIME_LOG` unset.
- **On Windows with Vulkan (`WGPU_BACKEND=vulkan`), an acquire that waits more than one second
  loses the device.** wgpu-hal 30.0.1 waits on a fence after `vkAcquireNextImageKHR` hands out an
  image, on Windows only, and maps a timeout of that wait to `DeviceError::Unexpected`, which
  wgpu-core treats as the loss of the device. A GPU frame longer than about one second, or a
  compositor that holds the window's images that long (a covered window, a locked session), then
  stops the run with `GfxError::DeviceLost`, with a reason that blames the driver; on Linux the
  frame is skipped instead. Not reproduced: in the owner's Windows 11 virtual machine with an AMD
  driver, the `scene` example kept running with its window covered for six seconds and with the
  session locked (2026-10-13). To report to wgpu: that wait's timeout should give
  `SurfaceError::Timeout`, as its other waits do.
- **Seen from below, the sample scene's walls and crates flicker where they touch the floor.** Their
  bottom faces lie in the floor's plane, and since M3b.3 the floor is double-sided, so from below
  both are drawn at the same depth (z-fighting). The `scene` example keeps its camera above the
  floor, but another viewer of the asset sees it. Lifting them by a few millimetres in
  `examples/assets/make_scene.py` fixes it, at the cost of rendering the glTF references again.
- **A mouse button can stay down on Wayland when the compositor takes the pointer away during a
  press without taking the keyboard focus.** winit 0.30 reports only `CursorLeft` then, and
  `Translator` (`crates/slime-platform/src/event.rs`) releases held buttons only when the focus is
  lost. Releasing them on `CursorLeft` would be right on Wayland only: on X11 and Windows the
  cursor leaves the window during an ordinary drag.
- **On Windows, the first of two held Shift keys let go of is released late.** Windows sends a
  single key-up for both Shift keys, so the engine releases the first one only when the second
  goes up. Windows 11 marks the second Shift's press as a repeat, so neither its press nor its
  release is reported (seen in the owner's virtual machine on 2026-10-13). Polling `GetKeyState`,
  as GLFW and SDL do, would remove the delay and report both keys, but needs `windows-sys` as a
  direct dependency.
- **After a dead key, a press and its release report different logical keys.** On Wayland and
  X11, a character key that does not combine with a pending dead key is reported as
  `Key::Unidentified` with no text; when the keys combine, the press is the composed character and
  the release the plain one. winit exposes no better value; rebuilding one would need libxkbcommon
  called directly. Documented in `KeyEvent::logical`.
- **A key pressed just after the window gains the focus can be swallowed on X11 and Windows.**
  winit reads which keys are down when it handles the focus event, not when the focus changed. A
  key pressed in between counts as held before the focus and is not reported until it is pressed
  again; on X11 the gap covers `App::init` when the window manager focuses the window while it
  runs. The translator cannot tell such a press from X11's unmarked first auto-repeat. On Windows,
  trusting `KeyEvent::repeat` would close the gap, at the cost of a rule that differs between
  platforms; left as documented.
- **The examples' glTF loader sets no limit on a file's total memory** (left as it is by the owner's
  decision of 2026-10-13: the loader is temporary). It checks each primitive alone against the GPU's
  largest buffer, reads every primitive of every mesh before placing any, and gives primitives that
  name the same accessors their own copy, three vertices per triangle when they have no normals. A
  valid GLB of a few MiB that lists one primitive many times can make the process run out of memory
  rather than be refused. Budgets for imported assets belong to the importer of Stage 4.
- **No test covers the empty submission of frames without a window image.** When
  `Surface::acquire` gives no image, it submits an empty batch, so that wgpu releases the writes
  an application makes outside `App::render`. Without it, 2000 frames of a surface resized to
  zero, each writing 64 KiB to a uniform buffer, grew the process by about 120 MiB (lavapipe in a
  headless Weston, 2026-10-13), measured by a program kept out of the repository. The
  `draws-nothing` scenario of `crates/slime-app/tests/windowed_lifecycle.rs` checks the same for
  frames whose image nothing draws, not for frames without an image.
- **A GPU time can read 0 once when the timestamp counter wraps.** On adapters whose timestamps
  have 36 valid bits (Intel's Vulkan driver, about once every one to one and a half hours), a
  timed pass that runs across the wrap ends before it starts. `GpuTimer::latest` then gives 0 for
  that pass until the next time replaces it. wgpu does not tell how many bits are valid, so the
  wrapped difference cannot be recovered. Dropping such a sample (`end.checked_sub(start)` in
  `crates/slime-gfx/src/timer.rs`) is a one-line change if it ever matters.
- **The instance buffer is uploaded whole every frame.** `Renderer::render` uploads
  `capacity × stride` bytes (256-byte slots with the default limits), where the capacity is a
  power of two from 64 that fits the frame's instances, up to four times as many: it shrinks back
  once a frame uses a quarter of it or less, so that a 10-instance frame after a 100 000-instance
  one costs what it costs on a new renderer (0.054 ms against 21.4 ms before, on lavapipe).
  Uploading only the live slots would amend ADR-0026's rule that a uniform buffer is always
  written at its creation size.
- **The dev overlay can still panic on a lost device** (ADR-0022). `Overlay::render` skips the
  frame when the device was lost earlier in it, but two cases panic inside egui-wgpu or wgpu: a
  loss that egui-wgpu's own buffer upload is the first to find, and `Overlay::new` on a device
  already lost (egui-wgpu creates its buffers with `create_buffer_init`). Both are documented
  under `# Panics`; closing them needs a change in egui-wgpu or wgpu.
- **The glTF loader copies shared vertex attributes into every primitive that uses them**
  (`examples/common/gltf.rs`). Each primitive that has normals gets every vertex of its position,
  normal and texture-coordinate accessors, including those its indices do not name. K primitives
  that share one set of attributes over V vertices, each with its own indices (three.js writes a
  mesh of several materials this way), therefore upload K × V vertices instead of about V, decode
  the attributes K times and grow the bounds over K × V points. The sample asset does not share
  attributes, and the importer of Stage 4 replaces the loader. The fix is to compact each
  primitive to the vertices its indices name. To measure: load a three.js export of a mesh with
  several materials, and compare the sum of `MeshData::positions.len()` over the meshes created,
  and the load time, before and after.
- **Exposed light beyond 64 × 65 504 is clamped** (ADR-0035, ADR-0037, ADR-0039, ADR-0040). The
  scene and sky shaders write exposed light divided by 64 into the `Rgba16Float` target, and light
  above 64 × 65 504 as 65 504, the largest half, rather than leave it to the driver, which may store
  an infinity that the tone curve turns into NaN; lavapipe and RADV clamp it themselves, so that no
  test can show either clamp. The sun's disc, 1.47 × 10⁹ nits for the sample's sun, is clamped from
  an exposure of 2.85 × 10⁻³, six and a half stops above the sample's; emission above
  64 × 65 504 / exposure, and any light once the exposure is raised about 13.5 stops above the
  sample's, are clamped too, and the bloom spreads them with the energy they keep. A sky's radiance
  is at most 65 504 nits before the exposure: brighter zenith, horizon or ground is refused, and a
  brighter glow kept within it. Divided by 64, exposed light below 64 × 2⁻¹⁴, about 0.004, is
  stored in half precision's subnormals, in steps of 64 × 2⁻²⁴: black on an 8-bit display, whose
  darkest step is about 0.007 exposed; an output of higher precision would show its steps.
- **Every fragment evaluates every light of the scene** (ADR-0037). The scene pass has no light
  culling: its cost grows with the number of lights, about 20 µs per light for a screen of 1920 ×
  1080 fragments on an RX 6900 XT (2026-10-13), 338 µs for 16, the most a scene holds, and 10 to 20
  times more on an integrated GPU; a light's range saves no work. More lights would need clustered
  or tiled culling and a storage buffer, a measured decision of a later stage.
- **Point and spot lights cast no shadows, and light what objects hide** (ADR-0037, ADR-0038: only
  the first directional light that casts shadows casts them). The sample's spot light is aimed up,
  so that the light the wall would stop goes into the sky; its ring light reaches 4 m, and gives
  little beyond a metre.
- **One shadow map covers every instance that casts shadows** (ADR-0038, ADR-0049). Its texels are
  their extent across the light divided by 2048, 3 mm for the sample: a terrain of a few kilometres
  would get texels of a metre, and objects smaller than a texel cast nothing, unless the terrain
  casts no shadow (`Instance::casts_shadows`). Cascades, or a region that the game chooses, are
  left to a later stage. The map takes 16 MiB per renderer, even for scenes without a directional
  light.
- **Shadow edges crawl when the instances' extent changes** (ADR-0038). The map is fitted to the
  instances again every frame, without texel snapping: an instance that moves at the edge of the
  scene shifts or rescales its texels. A static floor larger than the moving objects, as in the
  sample, avoids it.
- **Thin objects close to a surface lose their shadow (peter-panning)** (ADR-0038). The
  slope-scaled bias pushes stored depths back by about four texels' slope: 10 mm on the sample's
  floor, more under grazing light. A sheet lying a centimetre above a floor casts nothing.
- **Every draw of the scene pass runs a shader that can discard fragments** (ADR-0035). The `MASK`
  alpha mode discards in the one fragment shader that every material uses, opaque materials with a
  cutoff of 0. Some GPUs then test depth after the shader rather than before, for opaque draws
  too, which costs overdraw. Not measured: compare the scene pass's GPU time on a scene with much
  overdraw against a build whose opaque materials use a fragment entry point without `discard`
  (six pipelines instead of three).
- **Tangents that a glTF file lacks are computed per triangle, not with MikkTSpace** (ADR-0035),
  against which normal textures are usually baked. Such files shade slightly differently from
  other viewers, most at seams in the texture coordinates. The importer of Stage 4 computes
  MikkTSpace tangents.
- **The sky's light is neither shadowed nor occluded** (ADR-0039). A surface under an object, or
  inside a box, receives the whole sky, but for its material's occlusion texture: no ambient
  occlusion (screen-space effects are outside the stage), no reflection probes, and mirrors reflect
  the sky, never the objects around them.
- **The environment is computed on the CPU, in every process that draws a sky** (ADR-0039): 23 ms
  in a release build on the owner's 16 threads, 162 ms on one core, and the BRDF table once (15
  ms); about eight times longer in a debug build, which every test that draws a sky with a gradient
  or a sun pays: the workspace's tests took 23.9 s instead of 9.7 s on lavapipe on the owner's
  machine, most in `slime-render`'s `tests/environment.rs` (6.2 s) and the examples'
  `tests/gltf.rs` (5.7 s), whose every load of a file computes the sample's sky. A sky that changes
  needs a new environment: no time of day. To measure: `cargo test --release -p slime-render --lib
  -- --ignored --nocapture`, against the budget of M3b.6. To move to the asset pipeline of Stage 4,
  or to compute passes if skies change at run time; if the tests' time matters first, building
  `slime-render` optimized in debug builds would shorten it, at the cost of stepping through it in
  a debugger.
- **The sky's sun is described twice** (ADR-0039). A game gives the same `DirectionalLight` to
  `Sky::sun`, for the disc and the glow, and to `Scene::lights`, for its light and its shadows;
  two different lights show a disc where no light comes from. The sample takes both from the
  file's first directional light.
- **Rough metals reflect the sky with multiple scattering compensated, the lights without**
  (ADR-0039, as the Khronos glTF Sample Viewer): a white metal of roughness 1 reflects all of a
  uniform sky, but its lights keep glTF's single-scattering BRDF, under which it would reflect
  31 % of that sky; rough metals are darker in the sun than in the shade, relatively.
- **A roughness between two prefiltered levels reads a blend of their lobes** (ADR-0039): the four
  levels are 0.25 apart, and a lobe in between is approximated by mixing the nearest two, the split
  sum's usual approximation; each level is within 2.2 % of its own lobe's average.
- **Smooth gradients band on 8-bit outputs.** The sky's gradient shows steps one sRGB level apart,
  20 to 40 pixels wide, as an 8-bit output does without dithering, and the sun's halo steps 2 to 9
  pixels wide near the sun; not visible in the sample at normal contrast.
- **The `shapes` image test uses 33 of the 65 pixels that RADV may have off by more than 8**
  (ADR-0030): edges and the metal spheres' reflections of the sky (49 at M3b.6, before the bloom).
  A change that adds reflective detail may exceed it; measure it again then.
- **Four passes per submission is a number measured on one machine** (ADR-0041). On the owner's
  RX 6900 XT (RADV), whose memory that the CPU can map is a 256 MiB window, one submission of a
  whole bloomed frame stalled in the kernel once the desktop's programs and a test program filled
  that window to 241 MiB (a mean 2.4 ms, 8 ms in one frame of ten, at 1920 × 1080); submissions of
  2 to 6 passes did not, 8 did. The cause in the kernel is not established; Direct3D 12 and other
  GPUs are not measured, and a GPU with resizable BAR has no such window. To measure: the CPU time
  of `render` at 1920 × 1080 with the sample's bloom, for several values of
  `PASSES_PER_SUBMISSION` in `crates/slime-render/src/graph.rs`, with the window filled to
  several levels, by the desktop's programs or by a program that allocates device-local,
  host-visible memory (on Linux, `mem_info_vis_vram_used` in the GPU's sysfs directory tells how
  full it is).
- **The render graph trusts what a pass says it samples from imported targets** (ADR-0041). The
  groups that sample transient targets are built from the declarations, but the scene pass's group
  holding the shadow map is built with the renderer: a pass that binds an imported target must
  declare it, or the graph orders it without knowing.
- **A renderer whose frame changes shape at every call reports no tone-mapping or bloom time**
  (ADR-0041). Each change of shape (the output's size, the shadow pass or the bloom starting or
  stopping) builds the frame again with new timers, and the last submission's times come back
  during the next call, by then read from newer timers; such a renderer also creates its targets
  again at every call, as it did before. Use one renderer per output size.
- **Tiny, very bright sources blink in the bloom** (ADR-0040). Without a firefly filter (Karis's
  average dims the sun's halo and makes it flicker, measured), a highlight smaller than a pixel
  blooms as its coverage of the pixel changes from frame to frame. Temporal anti-aliasing, or a cap
  on the light a pixel brings to the chain, belong to a later stage.
- **The bloom has one shape** (ADR-0040): every level the same weight, down to a side of 8 texels;
  no radius, threshold or tint for a game to set. A window resized across a power of two adds or
  removes a level: the glow's reach changes a little while the edge is dragged.
- **Light near the right and bottom edges blooms a little more or less** (ADR-0040). Where an
  output's side is not a multiple of 2^levels, the coarse levels' last texels straddle the edge: a
  pixel's halo there holds 0.83 to 1.37 of its light at ten sizes measured, up to 1.9 at the far
  corner (1023 × 1023, 2047 × 1151), within 1.002 at 1280 × 720 and 1.10 at 1920 × 1080; inside the
  image and at the top and left edges, its light exactly. To measure: the CPU model of
  `tests/bloom.rs` at the size in question. A fix would pad the chain beyond those edges.
- **glTF minification filters are approximated.** `slime-gfx` ties mip blending to the
  minification filter and gives every texture mipmaps, so only `NEAREST_MIPMAP_NEAREST` and
  `LINEAR_MIPMAP_LINEAR` are exact (documented in the loader). Revisit when an engine-side user
  needs a separate mipmap filter or a base-level-only read.
## Notes

- CI build cache: not added yet. With wgpu, the Linux job takes about 2.5 minutes and the Windows
  job about 5 (measured on the runs of 2026-10-12; it compiles the dependencies three times: for
  clippy, for the tests, and in release for the artifact), which is acceptable; a cache is to add if
  it grows much further. The dependencies of Stage 3a (glam, gltf, egui) were expected to add to
  it, and each milestone that added one recorded the new duration here. With glam and the render
  crate (M3.2, 2026-10-13): Linux 2 minutes 49 seconds, Windows 4 minutes 59 seconds. With gltf and
  egui (M3.6, 2026-10-13): Linux 3 minutes 22 seconds, Windows 5 minutes 42 seconds, and 6 minutes
  2 seconds once its tests ran one at a time. At the release (M3.7, 2026-10-13), over the ten
  green runs before it: Linux from 2 minutes 35 seconds to 3 minutes 38 seconds, Windows from
  3 minutes 12 seconds to 7 minutes 10 seconds; the Windows runner's speed varies from one run to
  the next more than the code changes between them.
