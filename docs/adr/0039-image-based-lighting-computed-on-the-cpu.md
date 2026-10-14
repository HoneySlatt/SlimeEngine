# 0039. Image-based lighting computed on the CPU

- Status: Accepted; amends [0027](0027-the-render-crate.md),
  [0032](0032-mipmaps-computed-at-texture-creation.md),
  [0033](0033-gpu-layer-for-multi-pass-rendering.md),
  [0034](0034-hdr-rendering-and-tone-mapping.md), [0035](0035-physically-based-materials.md),
  [0037](0037-lights-in-physical-units-and-pre-exposure.md) and [0038](0038-shadow-mapping.md);
  amended by [0041](0041-render-graph.md) (the sky pass shares a submission and its timer)
- Date: 2026-10-13

## Context

Surfaces reflected their lights and a constant ambient light, the same from every direction, times
their base color whatever their metallic and roughness, and the scene pass cleared its target to a
background color exposed on the CPU ([ADR-0035](0035-physically-based-materials.md),
[ADR-0037](0037-lights-in-physical-units-and-pre-exposure.md)): smooth and metallic surfaces
reflected nothing of their surroundings, and shadowed surfaces were lit evenly. The plan of
Stage 3b replaces both with image-based lighting (IBL): a procedural sky, a gradient and a sun,
drawn on the CPU into a small cube map, its irradiance, its prefiltered specular levels and the
table of the BRDF's scale and bias, computed at startup, without compute passes or image files; a
sky pass that draws the environment behind the scene, exposed as surfaces are; in `slime-gfx`,
cube textures and textures whose levels the caller gives, in `Rgba16Float` among other formats,
with half-precision floats converted by hand (`docs/PLAN.md`, M3b.6).

Known: the Khronos glTF Sample Viewer, the reference of ADR-0035, reflects an environment with
Karis's split sum (a prefiltered level times a Fresnel term from a table of the BRDF's scale and
bias), Fdez-Agüera's roughness-dependent Fresnel and multiple-scattering compensation, the level
chosen linearly in roughness, and glTF's occlusion on the whole term. The sun's mean apparent
diameter is 0.533°: a sun of 100 000 lux is a disc of about 1.47 × 10⁹ nits, beyond the 65 504 of
half precision. wgpu reads a cube's faces in the order +X, −X, +Y, −Y, +Z, −Z, with Vulkan's and
Direct3D's orientation, and filters across their edges. Its default limits give four bind groups;
the fourth holds the shadow map ([ADR-0038](0038-shadow-mapping.md)), and every group of a
pipeline's layout must be bound at each draw. FXC, the Direct3D 12 shader compiler, refuses
gradient instructions in loops it cannot unroll
([ADR-0036](0036-direct3d-12-shader-compiler.md)). The image tests are strict on lavapipe
([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md)). No new crate may be added: no
`half`, no image decoder.

To decide: what replaces the ambient light and the clear color; what a sky is and how its sun
relates to the scene's directional light; what is precomputed, how precisely and at what cost; how
`slime-gfx` takes the levels; where the environment is bound; how the sky is drawn; how surfaces
reflect it; how this is tested. Three designs were prototyped (the least code, the most rigorous
tests, the best image); the owner decided on 2026-10-13 that the sky shows a sun disc and its glow,
described in the sky; that the sample's sky is a clear blue over dark soil; and that its faces are
128 texels wide, for a startup cost of about 20 ms. The rest was common to the designs or measured.

## Decision

### What a user sees

- `Sky { zenith, horizon, ground, sun }`: three radiances in nits, linear, and an optional sun, a
  `DirectionalLight`. Above the horizon, a direction of elevation sine y sees
  `zenith + (horizon − zenith) t`, t = (g(y) − g(1)) / (1 − g(1)), g(y) = 1 − exp(−0.32 / y): the
  CIE standard clear sky's luminance gradation, which brightens and pales mostly within 20° of the
  horizon.
  Below it, `ground + (horizon − ground) exp(y / 0.03)`: the horizon fades into the ground within
  about 1.7°, a haze. With a sun, the sky holds its glow: 4 % of its illuminance, spread around its
  direction by a Henyey-Greenstein lobe (g = 0.8) and hidden below the horizon through the same
  haze. Each channel of the result is at most 65 504.
- `Renderer::create_environment(&gpu, &sky) -> Environment` computes and uploads the environment,
  on every core; it panics on a zenith, horizon or ground that is negative, not finite or above
  65 504 nits. An `Environment` is shared by its clones and not tied to the renderer that made it.
- `Scene::environment: Option<&Environment>` replaces `Scene::ambient` and `Scene::clear`. `None`
  is a black sky: only the lights and emission light surfaces, and the background is black.
- The sun's disc is in no level of the environment. The sky pass draws it: a disc of angular radius
  0.2665°, of radiance `color × illuminance / Ω`, Ω = 4π sin²(r / 2) = 6.7967 × 10⁻⁵ sr,
  which gives a surface facing it the sun's illuminance; its edge is blended over a pixel, and below
  the horizon the ground's haze hides it. The sun's light reaches surfaces only through the same
  light in `Scene::lights`, which casts shadows: nothing is counted twice, and a shadow removes the
  sun's light and keeps the sky's, as outdoors. A game gives the sky and the scene the same light;
  the sample takes it from the file's first directional light.
- `RenderStats::sky_gpu_time`, under the rules of the other GPU times.

### The precomputation

- Level 0: the sky in a cube of 128 × 128 texels per face (0.90° per texel at a face's centre,
  0.45° at its edges), each texel the average of 2 × 2 points of it.
- Levels 1 to 4, faces of 64, 32, 16 and 8 texels: Karis's prefilter for roughness 0.25, 0.5, 0.75
  and 1, Σ L(l)(n·l) / Σ (n·l) with n = v = r, from 64, 128, 256 and 2 048 GGX directions of a
  Hammersley set, the same around every texel, each read from a box-filtered chain of level 0 at
  the level whose texels cover the solid angle it stands for (Křivánek and Colbert's filtered
  importance sampling), without the customary bias of one level, which blurs the levels beyond the
  tests' tolerances.
- At roughness 1, GGX's distribution of normals is uniform, and the prefilter's directions are
  uniform over the sphere: level 4 is the irradiance divided by π. There is no irradiance map.
- The BRDF table: 32 × 32 texels, n·v across and roughness down, their centres on 0 and 1 of each
  (n·v = 0 computed at 10⁻⁴), A in red and B in green, each from 1 024 directions by Heitz's
  sampling of the normals the view sees, which converges much faster than sampling GGX's
  distribution near grazing views; computed once per process.
- `std::thread::scope` on every core, each value computed alone, so that the result does not
  depend on the number of threads. A sky of one radiance in every direction, without a sun, has that
  radiance in every texel of every level: the integrals are skipped.
- Measured on the owner's machine (Ryzen 7 5800X, 16 threads, its desktop busy), release build: the
  sample's environment in 23 ms (20 to 28), 162 ms on one core; the table in 15 ms, 135 ms on one
  core, once per process. The budget: the sample's environment and table under 50 ms on every core
  (`cargo test --release -p slime-render --lib -- --ignored --nocapture`). Every level is within
  2.2 % of the integral it estimates on the sample's sky, the largest errors in the sun's glow at
  roughness 0.25; the irradiance within 0.43 %; the table within the 2.5 × 10⁻³ of an independent
  quadrature that its test allows.

### Binding, passes, shading

- Group 0 holds the frame's values (binding 0), the environment (1, `texture_cube<f32>`), the BRDF
  table (2) and one linear sampler, clamped to the edge, linear between levels (3). The renderer
  builds the group for the scene's environment and keeps it until another one is named, and holds
  a one-texel black environment for scenes without one. Group 3 keeps the shadow map: ADR-0038's
  binding does not change.
- The frame's values:

| Uniform | Bytes | Members (offset) |
|---|---|---|
| Frame, group 0 | 1 280 | view_proj (0), sky_from_clip (64), camera_position (128), exposure (140), light_view_proj (144), light_count (208, `u32`), shadowed_light (212, `u32`), to_sun (224), sun_radiance (240), lights (256, 16 × 64 bytes) |

  `sky_from_clip` is the inverse of the projection times the view's rotation, without the camera's
  position, so that the far plane's points give directions.
- The sky pass runs after the scene pass, its own pass with its own timer: it keeps the scene's
  light and depths, and draws one triangle at the far plane with the depth test `LessEqual` and no
  depth write, so that only pixels no surface covered are shaded. It reads level 0 along each
  pixel's direction, adds the sun's disc, multiplies by the exposure and writes at most 65 504. The
  scene pass clears to black; the background is no longer exposed on the CPU.
- The scene shader's ambient term becomes the environment's light, as the Sample Viewer computes
  it: the diffuse light read from level 4 along the normal, the specular light from level
  4 × roughness along the reflection (the material's roughness, not the lights' lower bound of
  0.045, so that a mirror reads level 0), each weighted by the Sample Viewer's `getIBLGGXFresnel`
  (A and B read at n·v and roughness; multiple scattering compensated, so that a white metal loses
  no light), mixed as glTF mixes the direct light's layers, times glTF's occlusion and nothing else:
  shadows do not touch it. Every lookup is `textureSampleLevel`, outside the loop over the lights:
  no derivatives, `SampleLevel` for FXC. The sky pass's disc uses `fwidth`, in a shader without
  loops or discards.

### `slime-gfx`

- `Gpu::create_texture_with_levels(label, width, height, format, levels)` and
  `Gpu::create_cube_texture(label, size, format, levels)` take every level from the caller, from
  the full size down, as bytes of any `TextureFormat`; a cube's level holds its six faces in wgpu's
  order, whose orientation `create_cube_texture` documents and a test checks texel by texel. One
  queue write per level, which needs no row alignment. `create_texture` keeps computing the levels
  of 8-bit textures and refusing `Rgba16Float`. `BindingType::CubeTexture` declares
  `texture_cube<f32>`.
- `f32_to_f16`, public, beside `f16_to_f32`: on bits, round to nearest, ties to even, subnormals,
  infinities from 65 520, NaN kept quiet.

### Tests

- `crates/slime-render/tests/environment.rs`, every pixel read back from an `Rgba16Float` output:
  white spheres, metal and dielectric, of roughness 0 to 1, in a uniform sky show its radiance (the
  furnace test); a black dielectric and a grey metal reflect what the split sum predicts from an
  independent integration of the BRDF; a mirror sphere reflects the sky's formula; a white matte
  sphere shows the sky's irradiance; rough metals show the sky averaged under their lobe, and
  between two levels; the sky pass shows the sky, with its glow, along every view ray; an
  environment draws the same with a renderer that did not make it, and a scene without one, drawn
  after it, is black. Tolerances are about 1.5 times the largest differences measured on lavapipe
  and RADV.
- Unit tests: the sky's formula at elevations computed by hand; the glow's integral and peak; the
  disc's radiance; every level against an integral over every texel of level 0; level 4 against
  the irradiance of a linear sky and of the sample's; the table against a Gauss-Legendre
  quadrature; the uniform sky's shortcut against the computation; the texels' directions against
  the table of `Gpu::create_cube_texture`; the WGSL constants against Rust's. `tone_mapping.rs`
  checks the sky's exposure and its value beyond 65 504, that the sun's disc is drawn once, at its
  radiance, and that the ground hides it below the horizon.
- `slime-gfx`: every half and every midpoint between halves through `f32_to_f16`; levels given in
  `Rgba16Float` read back exactly; every texel of a cube in the documented direction.
- A prototype of the design, with another sky and not kept in the repository, was checked against
  43 mutations (conversion, upload, orientation, sky, prefilter, table, shader, sky pass): 42 were
  caught. The sky pass's clamp at 65 504 cannot be shown on lavapipe and RADV, which clamp
  themselves, as for the scene shader's (ADR-0037).

This amends:
- **ADR-0027:** a scene is a camera, lights, an environment, an exposure and instances; a frame
  takes four passes.
- **ADR-0032:** callers may give a texture's levels (`create_texture_with_levels`,
  `create_cube_texture`); `create_texture` still computes them.
- **ADR-0033:** cube textures, `Rgba16Float` textures from the CPU and levels given by the caller
  exist, with `BindingType::CubeTexture`; its consequence that they arrive with their first users is
  met (array textures still wait).
- **ADR-0034:** the sky replaces the clear color, exposed by the sky pass.
- **ADR-0035:** the environment's light replaces the ambient term; occlusion darkens it alone.
- **ADR-0037:** `Scene::environment` replaces `Scene::ambient` and `Scene::clear`; the background is
  no longer exposed on the CPU; the frame's values are those above.
- **ADR-0038:** the frame's values are those above; group 0 holds the environment.

## Alternatives considered

- **No sun disc** (the least code): a daytime sky without a sun, and nothing for bloom to spread.
  **A disc for every directional light**, without a field in `Sky`: a fill light would show a
  second sun, and the sky could hold no glow, which depends on the sun's direction. **The sun in
  the cube map**: beyond half precision, counted twice beside the directional light, and that
  second half not shadowed; fireflies in the prefiltered levels.
- **A gradient in a power of the elevation, or a sharp horizon**: a sharp step shows a double line
  around roughness 0.1, where two levels are blended; the CIE gradation needs no tuned exponent.
- **Faces of 64 texels** (5 ms): a horizon blurred over about 17 pixels at 1280 × 720, lumpy in
  mirrors. **256** (57 ms): no visible gain behind the haze. The owner chose 128.
- **Spherical harmonics, or a separate irradiance cube**: a second representation of what level 4
  holds, a binding or 144 bytes of uniform more.
- **Integrating the sky's formula instead of a chain of level 0**: exact, but each sky would need
  the integrals rewritten; the chain serves any cube.
- **Sampling GGX's distribution for the table**: 1.35 × 10⁻² of error at 1 024 samples, against
  1.1 × 10⁻³.
- **An environment set on the renderer**, or **a `Sky` in `Scene`, computed when it changes**: an
  image depending on an earlier call, or tens of milliseconds hidden in a frame. **A required
  environment**: a second way to say black. **An environment tied to its renderer** (its group
  built once): a panic, or a wrong image, when another renderer draws with it.
- **The environment in group 3 beside the shadow map**: the sky pass would bind four groups;
  **the shadow map moved into group 0** (ADR-0038's reserve): two frame layouts.
- **The sky drawn inside the scene pass**: one pass fewer, but no time of its own, and the render
  graph of M3b.8 names a sky pass. **Before the scene, without depth test**: every covered pixel
  shaded twice.
- **No multiple-scattering compensation**: a white metal of roughness 1 would reflect 31 % of a
  uniform sky. **The level from the lights' clamped roughness**: mirrors blurred.
- **Levels taken as `f32` and converted in `slime-gfx`**: float formats only, against the plan's
  "among other formats". **The `half` crate**: no new crate.

## Consequences

- Every image changes: the sample shows a sky, shadows turn blue, metals reflect their
  surroundings; the references are rendered again.
- API changes: `Scene::ambient` and `Scene::clear` are gone; `Sky`, `Environment`,
  `Renderer::create_environment`, `RenderStats::sky_gpu_time`, `create_texture_with_levels`,
  `create_cube_texture`, `BindingType::CubeTexture` and `f32_to_f16` are new.
- Every process that draws a sky computes it: about 23 ms in release on 16 threads, and the table
  once; a debug build takes about eight times longer (180 ms for the sample's sky while the other
  tests run), which every test that creates a sky with a sun or a gradient pays. A sky that changes
  needs a new environment.
- An environment takes 1.0 MiB on the GPU, the table 8 KiB per renderer.
- The sky pass and the environment's lookups cost a few microseconds on an RX 6900 XT, a few
  milliseconds on lavapipe (`docs/PLAN.md`, M3b.6).
- The sky's light is not shadowed nor occluded but by occlusion textures: the inside of a box is
  lit by the whole sky. Rough metals reflect the environment with multiple scattering compensated,
  the lights without, as in the Sample Viewer.
- M3b.7's bloom will spread the sun's disc; M3b.8's graph has a sky pass that reads the scene's
  depth target and draws into its high-dynamic-range target.
