# 0051. Dithering 8-bit outputs

- Status: Accepted; amends [0034](0034-hdr-rendering-and-tone-mapping.md)
- Date: 2026-10-14

## Context

The tone-mapping pass ([ADR-0034](0034-hdr-rendering-and-tone-mapping.md)) writes the frame into
the output, which for a window is 8 bits per channel, sRGB-encoded: 256 values. A slow gradient,
as the sky's from the horizon to the zenith, then shows bands one value apart, 20 to 40 pixels
wide in the sample, and the sun's halo steps near the sun. The scene is computed in floating point:
the bands come from rounding alone, the same value given to every pixel between two steps.

An audit of the debt before 0.1 listed this; the owner decided on 2026-10-14 to dither 8-bit
outputs now.

## Decision

- For an 8-bit output, the tone-mapping pass adds noise to each pixel before the output rounds it:
  up to one step, 1/255, in encoded values for an sRGB output (whose steps are even there, not in
  light), the same in every channel, so that a grey stays grey.
- The noise's density is triangular, the sum of two uniform values from −1/2 to 1/2 steps: the
  rounding's average then equals the value before it, wherever it falls between two steps, and the
  error no longer depends on the value, which removes the bands and leaves an even grain of one
  step.
- The values come from a hash of the pixel's position (Jarzynski and Olano's `pcg2d`), the same at
  every frame: images are reproducible, as the image tests need, and a still image does not
  flicker.
- A channel within half a step of 0 or 1, which rounds there without noise, gets none: black and
  white stay black and white, where no band shows.
- A floating-point output, `Rgba16Float`, is not dithered: it holds the values themselves.
- The tone-mapping pass's uniform gains the step and whether the output encodes sRGB, which the
  renderer sets from its output format.

This amends ADR-0034: the tone-mapping pass dithers 8-bit outputs.

## Alternatives considered

- **Noise that changes at every frame.** Averaged by the eye over frames, it is less visible, but a
  still image flickers and image tests need a reproducible frame; temporal anti-aliasing, a later
  stage's work, is where frames would share such noise.
- **A blue-noise texture.** Its grain is finer to the eye, but it needs a texture made and shipped
  with the engine, for a grain of one step that is already hard to see.
- **Uniform noise of one step.** It removes the bands too, but leaves an error that depends on the
  value, which shows as noise that brightens and fades along a gradient.
- **Noise per channel.** It turns greys into coloured grain.
- **A 10-bit output.** Not every display or window system offers one, and 10 bits band too, less.

## Consequences

- Gradients no longer band on 8-bit outputs; every pixel that is not black or white carries a grain
  of at most one step.
- Pixels of 8-bit outputs differ from the undithered value by up to one step: tests compare them
  with a tolerance of one step or more, as they did for the drivers' rounding.
- The image tests' references were rendered again.
