# 0056. Stage 3d: anti-aliasing and the shadows of spot lights

- Status: Accepted; amends [0052](0052-amend-roadmap-before-0-1.md) (Stage 9a's content); amended by
  [0058](0058-shadows-for-every-spot-light-in-one-atlas.md) (how many spot lights cast shadows)
- Date: 2026-10-15

## Context

Stage 3c's game, *Nightwatch*, is a stealth game of light and shadow: guards walk with flashlights
and floodlights light the courtyard, both spot lights, and the player hides in the dark. The owner
played it on 2026-10-14 and found two defects, both of the renderer:

- **Light goes through walls.** The engine's spot lights cast no shadows
  ([ADR-0037](0037-lights-in-physical-units-and-pre-exposure.md),
  [ADR-0038](0038-shadow-mapping.md): only the first directional light does), so a flashlight
  lights the far side of a wall or a crate. The game's rules know better, and do not let it see
  there, but the picture contradicts them.
- **Jagged edges.** The renderer has no anti-aliasing: a nearly horizontal edge, the top of a wall
  for instance, steps by one pixel at long intervals, which reads as teeth along it.

[ADR-0052](0052-amend-roadmap-before-0-1.md) puts both in Stage 9a, after the editor, the runtime
systems and the plugins: every game until then would show them.

The owner decided on 2026-10-15 to make them a short stage of their own, right after Stage 3c.

## Decision

The roadmap gains **Stage 3d — Anti-aliasing and the shadows of spot lights**, between Stage 3c and
Stage 4:

- Multisample anti-aliasing (MSAA), four samples, on the scene pass, resolved before the passes
  that read its target.
- Shadow maps for spot lights: a few of the scene's spot lights, at most four, cast shadows, each
  through a shadow map of its own seen in perspective from the light, filtered as the directional
  light's is.
- Nightwatch shows both: walls and crates stop the flashlights' and floodlights' light, and its
  edges are smooth.

The stage takes a letter, as ADR-0052 asks of stages added, and has no release of its own, as
Stage 3c: what it delivers is released with 0.2. Stage 9a keeps the rest of its renderer work:
cascaded shadow maps for the directional light, the shadows of point lights, more spot lights that
cast shadows if a game needs them, and temporal anti-aliasing.

This amends ADR-0052, whose Stage 9a held the shadows of spot lights and the first anti-aliasing.

## Alternatives considered

- **Anti-aliasing only, now**, the shadows of spot lights at Stage 9a: one milestone instead of
  about three, but the light that goes through walls, the defect a game of light and shadow shows
  most, would stay.
- **Both at Stage 9a**, as planned: the asset pipeline of Stage 4 comes sooner, and every game until
  Stage 9a shows both defects.
- **Post-process anti-aliasing (FXAA, SMAA)** instead of MSAA: cheaper on the GPU, and it smooths
  edges inside textures too, but it blurs detail and guesses edges from the image; MSAA samples the
  geometry itself. To weigh when the stage is planned, with measurements.

## Consequences

- Stage 4 starts about three milestones later.
- The image tests' references are rendered again for both changes.
- The roadmap's Stage 9a no longer lists the shadows of spot lights or the first anti-aliasing.
