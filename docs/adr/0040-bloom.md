# 0040. Bloom

- Status: Accepted; amends [0027](0027-the-render-crate.md),
  [0034](0034-hdr-rendering-and-tone-mapping.md) and
  [0037](0037-lights-in-physical-units-and-pre-exposure.md); amended by
  [0041](0041-render-graph.md) (the passes recorded four to a submission with the others, timed by
  their submissions' timers; the levels freed while the bloom is off)
- Date: 2026-10-14

## Context

A lens and the eye scatter a small share of the light that enters them around where it lands: a
bright source shows a halo, the sun a glare, whose light falls about as the square of the angle
from the source. The plan of Stage 3b asks for a bloom: a chain of high-dynamic-range targets, each
half the size of the one before, filled by downsampling the scene, then upsampled back and added to
the scene in the tone-mapping pass, with a strength, so that no blending is needed; the overlay
shows the chain's GPU time (`docs/PLAN.md`, M3b.7).

Known: the scene and sky passes write exposed light into an `Rgba16Float` target, clipped at
65 504, the largest half-precision value (ADR-0037); the sample's sun's disc is exposed to 46 188,
half a stop below. `slime-gfx` has no blending and submits each pass on its own; a pass cannot
sample the target it draws into, and a `GpuTimer` times one pass
([ADR-0033](0033-gpu-layer-for-multi-pass-rendering.md)). FXC refuses gradient instructions in
loops it cannot unroll ([ADR-0036](0036-direct3d-12-shader-compiler.md)). The image tests are strict
on lavapipe ([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md)).

To decide: what a user controls; how the bloom relates to the light; the chain's levels, sizes and
edges; the filters; how levels combine without blending; where the result is mixed; what the
statistics report; what happens to light beyond 65 504; how this is tested. Three designs were
prototyped (the least code, the most rigorous tests, the best image), and a model of the chain
compared their treatments of the image's edges. The owner decided on 2026-10-14 that the samples
bloom at a strength of 0.04, and that the high-dynamic-range target keeps light beyond 65 504 for
the bloom. The rest was common to the designs or measured.

## Decision

### What a user sees

- `Scene::bloom: f32`, from 0 to 1: the share of each pixel's exposed light spread around where it
  lands. The tone-mapping pass shows `(1 − bloom) L + bloom B` before its curve, L the exposed light
  and B its blurred copy: energy-conserving, without a threshold. A uniform image is unchanged, and
  every pixel blooms, whatever its brightness: only contrast makes a halo visible. 0 skips the
  bloom's passes, and the image is the one drawn without bloom; outputs smaller than 15 × 15 have no
  bloom. The samples use 0.04: the sun's disc glows over about a third of the screen, the ring
  faintly, and edges between light and shade soften by about 4 % of their contrast.
- `RenderStats::bloom_gpu_time`: the sum of the latest times of the bloom's passes, one `GpuTimer`
  per pass, under the rules of the other GPU times; `None` when the passes were skipped.

### The chain

- Levels of `Rgba16Float`: level i (from 1) is the output divided by 2^i, rounded to the nearest,
  while both sides keep at least 8 texels: 6 levels at 1280 × 720, 7 at 1920 × 1080, 8 at
  3840 × 2160. The coarsest texel spans an eighth to a sixteenth of the screen's shorter side, so
  that the halo reaches the same share of the screen at any resolution. A second chain of the same
  sizes but the coarsest holds the averages on the way up. Both are created with the
  high-dynamic-range target, at the output's size, and again when it changes: 4.7 MiB at
  1280 × 720, 10.5 MiB at 1920 × 1080.
- Down: each level from the one above (the scene's light for the first) through Jimenez's 13-tap
  filter (five overlapping 2 × 2 boxes read through a linear sampler, the central one weighted 1/2),
  without Karis's average.
- Up: each average from the coarser one through a 3 × 3 tent one coarser texel wide, mixed with its
  own level by 1 / (the number of levels from it to the coarsest), from a 16-byte uniform buffer per
  level: every level ends with the same weight in B, so that a pixel's halo falls with the square of
  the distance, and a running average never exceeds the light it averages. The tone-mapping pass
  reads the first average through the same tent.
- Every pass draws one triangle over its target and reads its source at exactly twice (down) or half
  (up) its pixel's position, through one linear sampler mirrored at the edges. Every level then
  shares one grid anchored at the image's top left corner, and light beyond an edge is the image's
  own light reflected: no halo wraps around, and a pixel at an edge whose position is on every
  level's grid (the top and left edges always, the others when the side is a multiple of 2^levels)
  keeps its light exactly. Elsewhere the rounding to the nearest keeps each level within half a
  texel of the image's edge: measured with the tests' model at ten sizes, a pixel's halo at the
  right and bottom edges holds 0.83 to 1.37 of its light, up to 1.9 at the far corner (1023 × 1023,
  2047 × 1151); within 1.002 at 1280 × 720 and 1.10 at 1920 × 1080. No pass reads what it writes.
- Every read is `textureSampleLevel` at level 0 or `textureLoad`: no derivatives, no loops, no
  discards. The tone-mapping pass branches on its strength, a uniform.

### Light beyond 65 504

The scene and sky passes write exposed light divided by 64, which the tone-mapping pass multiplies
back before mixing and mapping: the high-dynamic-range target and the chain hold exposed light up to
64 × 65 504 with its energy, six and a half stops above the sample's sun. Half precision divides by
a power of two exactly, but below 64 × 2^-14 (about 0.004 exposed, black on an 8-bit display), where
light is stored in steps of 64 × 2^-24 instead of relative ones. Brighter light is still clipped.

### Tests

- `crates/slime-render/tests/bloom.rs`, reading back `Rgba16Float` outputs: a single bright pixel's
  halo is symmetric about the diagonal through it, and two neighbouring pixels' halos mirror each
  other; on an odd output, the halo falls with the distance and does not change as the pixel moves;
  it does not wrap around to the other edge; a uniform image, dark or bright, is unchanged by any
  strength, from 1 × 1 to 64 × 64; a strength of 0 skips the passes, and an output below 15 × 15
  shows no bloom at any strength; the chain follows the output's size; every pixel of a halo, inside
  the image and at its corners, matches a CPU model of the chain written from this ADR within
  3.9 × 10⁻³ relative (6 × 10⁻³ allowed), on lavapipe and RADV; the model keeps a pixel's light at
  every edge of a 64 × 64 output; a pixel of 4 × 65 536 keeps its halo; the statistics give the
  bloom's time. A unit test checks the levels' sizes, another that both shaders scale by the same
  power of two.
- The other renderer tests draw at `bloom: 0.0`: they compare pixels with values computed from
  geometry, which a halo would move.

This amends:
- **ADR-0027:** a frame takes the bloom's passes between the sky and tone mapping; `Scene` gains
  `bloom`.
- **ADR-0034:** the tone-mapping pass mixes the bloom into the scene's light before the curve,
  through a linear sampler; it multiplies the light it reads by 64.
- **ADR-0037:** the high-dynamic-range target holds exposed light divided by 64, clipped at
  64 × 65 504; the tone-mapping pass has a uniform again, the bloom's strength.

## Alternatives considered

- **Additive bloom**: the image brightens by the strength, and the exposure no longer means what it
  says. **A threshold**: not physical, one more number in exposed units for every game to tune, and
  at the samples' strength the image looks the same.
- **Karis's average on the first level**: it keeps 56 to 72 % of the sun's halo, depending on the
  resolution, and makes it flicker by ±3.8 % as the sun moves; the samples have no fireflies to
  filter.
- **A 2 × 2 or 4 × 4 box down, or a 1-3-3-1 kernel**: halos snap to the coarse grid (up to 5 pixels
  off) or small glows are lopsided (41 % between mirrored pixels). **One bilinear tap up**: boxy,
  lopsided glows of small sources (31 % against 12 %), for 18 µs at 1080p on an RX 6900 XT.
- **Geometric weights**, as Unity's scatter: a glow that shrinks on larger screens. **A fixed number
  of levels**: a halo half as wide on a screen twice as large. **Stopping at 16 or 32 texels**: two
  passes fewer, no outer glow. **Down to one texel**: six more passes at 1920 × 1080 for the
  faintest glow.
- **Clamping at the edges**: a corner pixel's halo holds twice its light (1.99 at 256 × 256, 2.2 at
  1280 × 720), an edge pixel's 1.4 to 1.5. **Levels rounded down**: 0.21 to 0.64 at far corners of
  odd outputs. **Rounded up**: up to 10 times at a far corner of 1921 × 1081. **Normalized
  coordinates**: on odd outputs a pixel's halo is doubled or lost depending on its position.
- **A plain sum of the levels**: it overflows half precision near the sun.
- **Mixing after the tone curve**: neither energy-conserving nor brighter around brighter light.
- **Keeping the clip at 65 504**: the sun's glare fades into the sky as the exposure rises, 71 % of
  its light at one stop above the sample's, 18 % at three. **A wider format** (`Rgba32Float` needs a
  feature for linear filtering); **the sun's excess added analytically**: a coupling for one light.
- **Blending in `slime-gfx`, compute passes**: outside the stage. **One timer around the chain, or
  one submission for it**: changes to `slime-gfx` that M3b.8's graph will make.
- **A struct of settings, a renderer setting, `Option<f32>`**: one field suffices; state between
  frames; two ways to say off.

## Consequences

- Every image of the samples softens slightly; the references are rendered again. Light that the
  clip at 65 504 cut before is kept, without changing the samples' images.
- At 1280 × 720 and 1920 × 1080, 11 and 13 more passes per frame: on an RX 6900 XT, 34 and 51 µs,
  and 9 and 20 µs more in the tone-mapping pass; on lavapipe, 5.3 and 10.1 ms, and 3.4 and 7.4 ms;
  0.51 and 0.63 ms more of CPU per frame for the passes' submissions and timers, until M3b.8 records
  a frame in one submission.
- Light near the right and bottom edges of outputs whose sides are not multiples of 2^levels blooms
  up to about 40 % more or 20 % less, up to nearly twice at a far corner.
- A texel that is not a number in the high-dynamic-range target spreads into a whole halo.
- Tiny, very bright sources' halos blink as their coverage of a pixel changes, without temporal
  anti-aliasing. A window resized across a power of two adds or removes a level, and the glow's
  reach changes a little.
- M3b.8's graph will see 2N − 1 passes, each reading one or two targets and writing one, all at
  the output's size divided by a power of two, living for one frame.
