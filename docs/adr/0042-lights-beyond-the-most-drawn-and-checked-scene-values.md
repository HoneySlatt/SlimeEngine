# 0042. Lights beyond the most drawn, and checked scene values

- Status: Accepted; amends [0037](0037-lights-in-physical-units-and-pre-exposure.md); amended by
  [0102](0102-a-damaged-file-gives-an-error.md) (a light that cannot be drawn is left out)
- Date: 2026-10-14

## Context

`Renderer::render` packs at most `MAX_LIGHTS` (16) lights into the frame's values, and panicked on
a scene with more, in release builds too:
[ADR-0037](0037-lights-in-physical-units-and-pre-exposure.md) treated it as a bug in the caller, as
a target of another format is. A game's lights come from its data: torches a level places, lamps
spawned at run time. A level with one light too many ended the game for its players, the only way
valid game data could make the renderer panic.

The other values of a scene document their ranges but were not checked: an exposure that is not
a number turned every pixel white, a negative one gave wrong colors, a bloom of 2 took more light
from a pixel than it held and wrote negative light, and a light with a zero direction lit the
scene from a direction that is not a number, panicking only with glam's assertions on.
[ADR-0008](0008-errors-logging-and-assertions.md) has a violated invariant, which can only come
from a bug, panic, and keeps cheap checks in release builds.

An audit of the debt before 0.1 measured these cases; the owner decided on 2026-10-14 that the
renderer draws the first 16 lights of a scene with more.

## Decision

- A scene may hold more than `MAX_LIGHTS` lights. `render` draws its first `MAX_LIGHTS`, in the
  scene's order, and leaves the others out, logging a warning the first time a renderer meets such
  a scene, not at every frame. The light that casts the shadows is the first directional light
  that casts them among those drawn. A game that needs more chooses which ones matter, nearest or
  brightest, and puts them first.
- `render` checks, every frame, in release builds too, that the scene's exposure is finite and not
  negative, that its bloom is from 0 to 1, and that each drawn light's values are in the ranges
  their documentation gives: a color and a strength finite and not negative, a position finite, a
  direction finite and not zero, a range positive and finite, a spot's cones such that
  0 ≤ inner < outer ≤ π / 2. A value outside its range panics with a message that names the value
  and, for a light, its index: these values come from the game's code, and a bug there is better
  found at once than drawn as a white or black image.

This amends ADR-0037: `render` no longer panics on a scene with more than `MAX_LIGHTS` lights.

## Alternatives considered

- **Keep the panic on a 17th light**: a mistake cannot go unnoticed in development, but a player
  can meet it, in a release build, from data rather than code.
- **Return an error from `render`**: every frame becomes a `Result` to handle, for a case that
  does not need to stop the frame.
- **Pick the 16 lights that matter most** (nearest the camera, brightest): a policy the renderer
  cannot know for every game; the game orders its lights.
- **Clamp values outside their ranges, or ignore such lights**: a bug in the game's code would be
  drawn as a plausible image and never found.
- **Check the values in debug builds only**: a few comparisons per light per frame, nothing to
  save.

## Consequences

- A level that places too many lights shows its first 16, and the log says so once per renderer.
- A game whose exposure, bloom or lights carry values outside their ranges panics where it drew a
  wrong image before; the message names the value.
- When the renderer draws many lights (light culling, a later stage), the cap goes, and with it
  this rule.
