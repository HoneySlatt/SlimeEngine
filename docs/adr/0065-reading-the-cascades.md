# 0065. Reading the cascades

- Status: Accepted; amends [0050](0050-a-shadow-region-that-follows-the-camera.md) (the depth
  range's padding; the lookup off the smooth normal, in the vertex shader),
  [0062](0062-looking-spot-shadows-up-off-the-triangle.md) (the directional light, which kept the
  smooth normal) and [0064](0064-cascaded-shadow-maps.md) (how a cascade is chosen and its band
  read)
- Date: 2026-10-15

## Context

[ADR-0064](0064-cascaded-shadow-maps.md) gave the directional light four cascades, each fitted to a
slice of the view as the single map was, and a band at each cascade's edge across which its shadows
fade into the next cascade's. Until they were read, the scene shader looked every point up in the
last cascade, where the single map's lookup left it: the vertex shader moved each vertex off its
surface along the mesh's interpolated normal
([ADR-0050](0050-a-shadow-region-that-follows-the-camera.md)) and projected it into the map; the
fragment shader compared what it interpolated.

Reading the four cascades raised three questions, and their tests found two problems.

- **Which cascade shades a point.** Cascades are slices of the view along its axis, but each covers
  a square across the light, which holds every point whose shadow it can tell, at any distance from
  the camera: a cascade's square reaches beyond its slice, and holds points of the next slices too.
- **How its band is read**, and what lies beyond the last cascade.
- **Where a point is looked up.** Each cascade's texels have their own width: the normal offset,
  counted in texels, differs from one cascade to the next.
- **The smooth normal falls short under fine texels.** The first cascade's texels are a few
  millimetres wide, much finer than the facets of a coarse mesh: a sphere of 5 cm facets would
  shadow its own facets near the edges they face the light by, as spot lights' tiles did
  ([ADR-0062](0062-looking-spot-shadows-up-off-the-triangle.md)).
- **The constant bias falls short over a shallow range.** Each cascade's depths run from the
  nearest to the farthest box that reaches across its square, padded on each side by 1/64 of the
  larger of the square's side and that range (ADR-0050). Under a first cascade 5.5 m wide, a
  floor alone, lit head-on, gets a range of 17 cm: there, the constant bias of 32 units of the
  depths' last bit is a third of a micrometre, less than what rounding leaves of positions a few
  metres from the camera. A floor turned and lit head-on shadowed itself in the tests; under the
  single map, 83 m wide, the range was at least 2.6 m.

## Decision

- **The finest cascade that holds a point shades it.** The cascades are tried finest first. A
  cascade holds a point, looked up in it, when the point lies inside its square by more than
  4 texels, the filter's reach of 3 and a margin for rounding, so that the filter reads none of
  another tile's texels. Holding is told by the square, not by the slice: a point beyond a
  cascade's slice that its square holds is shaded with its finer texels.
- **Each cascade gives the share it holds of what the finer ones left.** Deeper inside its square
  than its band, a tenth of its width, a cascade holds all of the point; across the band, its hold
  falls to none at the margin. The light that reaches the point is the sum, over the cascades that
  hold it, of each one's comparison times its hold times the share the cascades before it left; the
  share that none holds is light. Shadows fade from one cascade into the next across its band, and
  out across the last one's band, beyond which nothing is shadowed. The loop stops at the first
  cascade that holds all of the point: one filtered comparison for most points, two in a band.
- **Looked up in the fragment shader, off the triangle.** For each cascade it tries, the fragment
  shader moves the point off its triangle along the triangle's own normal, from the derivatives of
  the position (ADR-0062), towards the light, by 4.5 of that cascade's texels times the sine of the
  light's incidence, and projects it into the cascade. The vertex shader no longer looks anything
  up. A pixel that sees its triangle edge on has no normal, and the point is not moved.
- **A range at least as deep as the square is wide.** Each side of a cascade's depth range is
  padded by half the square's side, or by 1/64 of the range when that is more. Rounding grows with
  the size of the square, as the points a cascade holds lie within about its side of the camera, and
  32 units of the depths' last bit then outweigh it, however close together the boxes lie along the
  light. A cascade that no box reaches keeps its square, around an empty range: the points it holds
  are lit.

## Alternatives considered

- **The cascade of the slice that holds the point's distance along the view**: the simpler rule of
  many engines, which leaves the finer texels of a square that reaches beyond its slice unused.
- **The first cascade that holds the point and the next one only**: the next cascade does not
  always hold a point near the first one's edge; summing the shares of every cascade that holds the
  point needs no exception, and as few comparisons.
- **The smooth normal**: the facets of a coarse mesh shadow themselves under the first cascade's
  texels (ADR-0062).
- **A larger constant bias**: 512 units of the last bit removed the acne over the shallow range, and
  would push shadows off their casters over a deep one.
- **A bias of a fraction of a texel along the light**, beside the constant bias: two biases to tune
  where one, over a range of a sound depth, suffices.
- **Looking up each cascade in the vertex shader**: four positions to interpolate, and the
  triangle's normal is known in the fragment shader only.

## Consequences

- Each fragment lit by the shadowed light computes up to four products by a matrix to choose its
  cascade, and one filtered comparison, two in a band.
- The tests model the cascades apart from the renderer, their squares, their texels and their
  bands, and check every pixel of a floor seen from near to the horizon against the model.
- Depths are coarser in a cascade whose boxes lie close together along the light, about a
  ten-millionth of the square's side, far below a texel.
