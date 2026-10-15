# 0062. Looking spot lights' shadows up off the triangle

- Status: Accepted; amends [0061](0061-projecting-the-shadows-of-spot-lights.md) (the normal
  offset of spot lights); amended by [0063](0063-a-smoother-shadow-filter.md) (the offset's texels)
- Date: 2026-10-15

## Context

[ADR-0061](0061-projecting-the-shadows-of-spot-lights.md) planned to look a point up in its spot
light's tile moved off its surface along its normal, towards the light, by 3 texels times the sine
of the light's incidence, as the directional light's shadow map does
([ADR-0050](0050-a-shadow-region-that-follows-the-camera.md)), the normal being the mesh's
interpolated one. Its tests, when the lookup was written, found two ways in which a surface then
shadows itself.

- **A tile's depths run along its light's axis, not along each ray.** For the directional light
  the two are one. Under a spot light, a floor that a ray meets head-on, but off the axis, still
  slopes in the tile's depths: its depth changes from one texel to the next by the sine of the
  angle between its normal and the axis, which the incidence, 0 there, does not see. A square lit
  by a spot light 2 m above it and leaning 10° showed half its light.
- **A tile's texels are much smaller than a mesh's facets.** On a sphere of facets about 5 cm wide,
  under texels of 2 mm, a facet is a plane that leans away from the smooth interpolated normal by
  up to half a facet's angle; the offset, computed from the smooth normal, fell short of the
  facet's own slope near the edge it faces the light by. The directional light's tests set its
  texels to about the facets' width, where this does not show.

## Decision

- **By the tilt from the axis.** The point is moved off its surface by 3 texels times the sine of
  the angle between the surface's normal and the light's axis, a texel being as wide as at the
  point's distance along the axis. Moved so, it lies 3 sin(a) / cos(i) texels' depth nearer than
  its own surface along the ray, where the filter's farthest points find the surface at most
  2.83 sin(a) / cos(i) nearer, a being that tilt and i the incidence.
- **Off the triangle.** The normal is the triangle's own, flat, computed in the fragment shader
  from the derivatives of the position across the pixel, before any fragment may be discarded; it
  is the surface the tile holds. A pixel that sees its triangle edge on has none, and the point is
  not moved.

The directional light keeps its offset: its axis and its rays are one, and its tests hold.

## Alternatives considered

- **The incidence, as ADR-0061 planned**: a surface off the axis that faces the light shadows
  itself.
- **The smooth normal**: the facets of a coarse mesh shadow themselves under fine texels.
- **A larger offset, or a slope-scaled depth bias**: either moves every shadow farther from its
  caster, where the cause is a wrong measure of the slope, not too small a margin.

## Consequences

- The tests check a lit floor under spot lights leaning up to 60° from it, and a lit sphere of
  coarse facets, against the same scenes drawn without shadows.
- A shadow moves off its caster by at most 3 texels, as the directional light's does.
- The scene shader takes the position's derivatives in every fragment, a few instructions.
