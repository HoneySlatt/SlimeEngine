# 0063. A smoother shadow filter

- Status: Accepted; amends [0038](0038-shadow-mapping.md) (the filter),
  [0050](0050-a-shadow-region-that-follows-the-camera.md) and
  [0062](0062-looking-spot-shadows-up-off-the-triangle.md) (the normal offset's texels) and
  [0061](0061-projecting-the-shadows-of-spot-lights.md) (the tiles' margin); amended by
  [0064](0064-cascaded-shadow-maps.md) (cascades)
- Date: 2026-10-15

## Context

The owner played *Nightwatch* at 3840 × 2160 once Stage 3d had smoothed the edges of triangles
([ADR-0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)), and saw no difference on the
edges they had found ugly: those of the shadows. A shadow map stores one depth per texel, about 3 cm
of ground for the game's moon, which casts shadows 30 m around the camera. Where a shadow's edge
runs nearly along the map's texels, each texel makes a long step. The filter
([ADR-0038](0038-shadow-mapping.md)) averaged 3 × 3 comparisons one texel apart, each of the 2 × 2
texels nearest to it: an edge fades over three texels, but the steps keep their shape, and four
times as many pixels show each texel at that resolution. Multisample anti-aliasing does not reach
them: it smooths where a triangle covers part of a pixel, not what a shader computes inside it.

Cascaded shadow maps, several maps of finer texels near the camera, are planned for Stage 9a; the
owner decided on 2026-10-15 that they will be built whatever happens, and left their timing open.

## Decision

- **Castaño's filter**, from *The Witness* (Ignacio Castaño, "Shadow Mapping Summary", 2013), for
  the directional light's map and the spot lights' tiles: nine comparisons, each of the 2 × 2
  texels nearest to it, placed and weighed along each axis by 4 − 3s, 7 and 1 + 3s, out of 12, s
  being where the point lies between two texel centres. Together they weigh the texels over 5 × 5
  by a smooth bump around the point, three texels wide on each side, so that a shadow's edge fades
  over about six texels, its texels' corners rounded. The number of comparisons stays nine.
- **A normal offset of 4.5 texels**, where it was 3: the filter now reads texels up to three away
  along each axis, 4.24 diagonally, and a point moved off its surface by more than that times the
  sine of its slope does not shadow itself (ADR-0050, ADR-0062).
- **Tiles that see 8 texels beyond their cone**, where they saw 6, for the filter's reach and the
  offset (ADR-0061).
- **Cascades are decided apart.** Measured on the game, this filter softens the steps but does
  not remove those of an edge that runs nearly along the texels, whose steps are longer than its
  reach: only finer texels near the camera do, which is what cascades give. They will filter each
  cascade with this filter.

## Alternatives considered

- **A larger map**, 4096 × 4096: texels half as wide everywhere, for 48 MiB more, and the steps
  still show, half as long.
- **More comparisons, on a rotated Poisson disc**: smooth too, but noisy unless filtered over
  frames, and costlier.
- **Shadows that soften with the distance to their caster** (percentage-closer soft shadows): a
  search before the filter, later, with the cascades.

## Consequences

- Shadows are softer: their edges fade over about six texels where they faded over three, and a
  shadow moves off its caster by up to 4.5 texels. On *Nightwatch* at 3840 × 2160, the long steps
  of the moon's shadows along the crates' feet remain, softer; half as long with the moon's shadows
  over 15 m instead of 30.
- The shadow tests model the new filter, apart from the shader; the reference images are rendered
  again.
- The scene shader's cost hardly changes: the same nine comparisons, a few more instructions.
