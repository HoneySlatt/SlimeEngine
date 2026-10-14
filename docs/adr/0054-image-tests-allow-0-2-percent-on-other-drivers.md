# 0054. Image tests allow 0.2 % of the pixels to differ on other drivers

- Status: Accepted; amends [0030](0030-image-tests-strict-on-the-reference-driver.md)
- Date: 2026-10-14

## Context

On drivers other than the one that renders the references, the image tests let 0.1 % of the pixels
differ by more than 8 out of 255 ([ADR-0030](0030-image-tests-strict-on-the-reference-driver.md)):
65 of the 65 536 pixels of a test's image. Drivers pick the level of detail of a minified texture,
and blend two levels, from derivatives they compute with different precision, and the tolerance
exists for those differences.

Before 0.1, a check on Windows found the bricks of the examples' asset turned by 90° on the wall's
large sides, three repeats squeezed into its height: its script gave each side of a box its
texture in the order Blender lists the side's corners. Laid along the wall, the bricks' mortar
joints run across a wall seen at a grazing angle, where the texture is most minified. Measured
against the lavapipe references at 256 × 256, the owner's RX 6900 XT (RADV) then differs on the
glTF scene's front view by more than 8 on 76 pixels (the largest difference is 24, all on the
wall), where it differed on 3 with the squeezed bricks. Bricks twice as large make it 121: the
joints cover more pixels. To the eye, the pictures are the same.

The owner decided on 2026-10-14 to fix the bricks before 0.1 and to widen the tolerance.

## Decision

- On drivers other than the reference driver, 0.2 % of the pixels may differ by more than 8 out of
  255: 131 pixels of a test's image of 256 × 256.
- The rest of ADR-0030 stands: on lavapipe, which renders the references, no pixel differs by
  more than 2; elsewhere, a pixel may differ by up to 8 before it counts.

## Alternatives considered

- **Joints of less contrast in the brick texture,** so that the drivers' differences stay below
  8. The asset would be drawn for the test's tolerance, not for what it shows, and the next asset
  with fine detail seen at a grazing angle meets the same limit.
- **A tolerance per reference image,** 0.2 % for the glTF scene's front view only. More precise,
  but an exception per image to justify and maintain, for one case.
- **Keeping the squeezed bricks until after 0.1.** The sample of 0.1 would show a wall that a
  reviewer noticed at once.

## Consequences

- On RADV and WARP, a regression of up to 131 pixels of a test's image goes unseen, where 65 did;
  on lavapipe, which the Linux CI job runs, any regression is still seen.
- The image tests keep printing how many pixels differ by more than 2, 4, 8 and 16, so that the
  margin is measured again whenever what they draw changes.
