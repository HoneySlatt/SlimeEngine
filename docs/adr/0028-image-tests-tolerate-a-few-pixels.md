# 0028. Image tests tolerate a few differing pixels

- Status: Accepted
- Date: 2026-10-13

## Context

[ADR-0018](0018-reference-image-tests.md) compares a rendered image with its reference within a
tolerance of 2 out of 255 per channel, which no pixel may exceed, and says that the tolerance is
raised, or a driver gets its own reference, with the measurement that justifies it. The samples'
triangle and the flat-colored cubes met it on lavapipe, RADV and WARP, and the sphere's smooth
shading differed by at most 1 out of 255. With a checker texture read with linear filtering
(M3.4), the owner's RX 6900 XT (RADV) differs from the lavapipe reference by 3 out of 255 on one
pixel of 65 536: where two texels of different colors blend, drivers do not interpolate texture
coordinates and blend weights with the same precision. Such differences grow with the detail of
a scene, and the edge of a triangle can also fall on either side of a pixel's center on different
drivers, where the pixel then differs by a whole color: no per-channel tolerance covers that.

## Decision

- A rendered image matches its reference when at most 0.1 % of its pixels (65 of a 256 × 256
  image) differ by more than 2 out of 255 in a channel. The per-channel tolerance of ADR-0018
  stays.
- The image tests print the difference they measure, visible with `--nocapture`, and the Windows
  CI job runs them that way, so that WARP's figures are known; `docs/PLAN.md` records the
  measurements of each milestone that changes what the tests draw.
- Everything else in ADR-0018 stands: references rendered with lavapipe, one per test, rendered
  again only on purpose.

This amends ADR-0018: how many pixels may exceed the tolerance.

## Alternatives considered

- **A per-channel tolerance of 4, still exceeded by no pixel.** Covers this measurement but not
  the next: a texel boundary on a smaller triangle, or an edge pixel that flips, differ by far
  more than any such tolerance.
- **A reference per driver.** Three images to keep in step for each test, and the owner's
  machine is the only one with RADV.
- **A perceptual comparison.** Meant for photographs; a few pixels at texel boundaries are not a
  perceptual question.

## Consequences

- A regression confined to a few pixels, such as a seam one texel off, can pass; one that changes
  a shape, a color or the lighting still fails by thousands of pixels.
- The threshold is a fraction, so the same rule serves images of every size.
