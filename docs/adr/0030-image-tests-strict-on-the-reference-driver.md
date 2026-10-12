# 0030. Image tests: strict on the reference driver, loose on the others

- Status: Accepted
- Date: 2026-10-13

## Context

[ADR-0018](0018-reference-image-tests.md) set one tolerance for every driver, 2 out of 255 per
channel, and [ADR-0028](0028-image-tests-tolerate-a-few-pixels.md) let 0.1 % of the pixels exceed
it. In M3.5, textures gained mipmaps, and the image tests started to draw textures minified on a
sphere and on a floor seen at a grazing angle. Which level of detail is read, and how two levels
are blended, depends on derivatives that drivers compute with different precision, and the
levels of a checker differ a lot from one another. Measured against the lavapipe references, the
owner's RX 6900 XT (RADV) differs on the glTF scene by more than 4 on 4 pixels of 65 536 (the
largest difference is 5), and on the checkered sphere by more than 4 on 233 pixels, by more than
8 on 14, by more than 16 on 1 (the largest is 18). These are the same pictures to the eye, and
no single tolerance both admits them and still catches a regression.

The references are rendered with lavapipe, whose version the Nix flake pins: on that driver, a
difference means that the rendering changed, not the driver. The Linux CI job runs the tests on
it; the Windows job runs them on WARP, and developers on their GPUs.

## Decision

- On the driver that rendered the references (lavapipe, recognized by the adapter's name),
  the rule of ADR-0018 stands: no pixel differs by more than 2 out of 255. A change seen there
  is a change of the rendering, to review, or a change of lavapipe's version, after which the
  references are rendered again on purpose.
- On every other driver, a pixel may differ by up to 8 out of 255, and 0.1 % of the pixels may
  differ by more: the test checks that the driver draws the same picture, not that it rounds the
  same way.
- The image tests print how many pixels differ by more than 2, 4, 8 and 16, so that the next
  change of what they draw is measured the same way, on RADV by hand and on WARP by the Windows
  CI job.

This amends ADR-0028, and through it ADR-0018: the tolerance depends on the driver.

## Alternatives considered

- **One tolerance of 8 for every driver.** The CI job on lavapipe would then let a regression of
  up to 8 out of 255 through, where it could catch any.
- **A larger share of differing pixels** (0.5 %) with the tolerance of 2. It passes this
  measurement by counting pixels that differ for the reason a per-channel tolerance exists:
  small, invisible differences in filtering.
- **A reference per driver.** Three sets of images to keep in step, and the owner's machine is
  the only one with RADV.
- **Nearest filtering between levels of detail in the tests.** Fewer differences, but the tests
  would not draw what the samples draw.

## Consequences

- On lavapipe, the tests stay as sensitive as before; on other drivers they only catch a wrong
  picture, which is what a developer's GPU can tell.
- An update of lavapipe through the flake may change the references: the references are then
  rendered again in the same change, and looked at.
