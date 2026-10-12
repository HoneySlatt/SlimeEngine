# 0018. Reference image tests

- Status: Accepted; amended by [0028](0028-image-tests-tolerate-a-few-pixels.md) (how many pixels may differ)
- Date: 2026-10-12

## Context

The exit criterion of Stage 2 compares an offscreen render with a reference image in CI. CI has no
GPU: Linux CI renders with lavapipe, Mesa's Vulkan driver that runs on the CPU, and Windows CI with
WARP, Microsoft's software Direct3D 12 driver. Developers also run the tests on their own GPUs.
Different drivers round colors differently, so exact comparison would fail outside the machine
that produced the reference.

Measured on the samples' triangle (256 × 256 pixels, colors interpolated across it): AMD's RADV
driver differs from lavapipe on 2139 pixels, each time by exactly 1 out of 255 in one or more
channels, and covers exactly the same pixels.

## Decision

- Reference images are PNG files (8-bit RGBA, sRGB) stored in the repository next to the test that
  uses them: `examples/tests/reference/`. They are small (7 KB for the triangle).
- They are rendered with lavapipe, the driver of the checks: the reference and the most frequent
  comparison come from the same driver.
- A rendered image matches its reference when no pixel differs by more than 2 out of 255 in any
  channel (at most 0 pixels may exceed it). If WARP or a hardware driver ever needs more, the
  tolerance is raised with the measurement that justifies it, or that driver gets its own
  reference; this ADR is then amended.
- `SLIME_UPDATE_REFERENCE_IMAGES=1`, with `VK_DRIVER_FILES` pointing at lavapipe, renders the
  references again; this is done only after a deliberate change of what the test draws.
- When a comparison fails, the test writes the image it rendered in the build directory and prints
  its path and the size of the difference.
- PNG files are read and written with the `png` crate, used only by the examples and their tests.
  The comparison itself is a few lines of test code.

## Alternatives considered

- **Exact comparison, or comparing hashes.** Fails on every driver but the one that made the
  reference.
- **One reference per driver from the start.** More files to keep in sync, while one tolerance
  covers the drivers measured so far.
- **The `image` and `image-compare` crates.** Larger dependencies for one PNG format and a
  per-pixel comparison.
- **Perceptual comparison.** Meant for photographs; a triangle needs no model of human vision.

## Consequences

- The same test runs on lavapipe, WARP and hardware GPUs.
- A real regression smaller than the tolerance would pass: with a tolerance of 2 out of 255, a
  shift of one or two levels goes unnoticed. A change of a vertex's red component from 1.0 to 0.9
  makes 8502 pixels differ by up to 12, and fails.
- Reference images are binary files in git; updating one must be a deliberate, reviewed change.
