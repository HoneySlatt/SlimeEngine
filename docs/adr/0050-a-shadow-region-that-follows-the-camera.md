# 0050. A shadow region that follows the camera

- Status: Accepted; amends [0038](0038-shadow-mapping.md) and
  [0049](0049-instances-that-cast-no-shadow.md)
- Date: 2026-10-14

## Context

The shadow map was fitted to the boxes of every instance that casts shadows
([ADR-0038](0038-shadow-mapping.md), [ADR-0049](0049-instances-that-cast-no-shadow.md)): its
2 048 texels spread over the scene's extent across the light, 3 mm for the 6 m sample, 5 cm for a
level of 100 m, 1 m for a terrain of 2 km, and they moved and changed size whenever an instance
at the scene's edge moved, so that every shadow's edge shimmered. Shadows were usable in a room,
not in a level.

An audit of the debt before 0.1 listed this; the owner decided on 2026-10-14 on a map that follows
the camera over a distance a game sets, its texels on a grid fixed in the world so that they do not
shimmer: the first cascade of the cascaded shadow maps that large engines use.

## Decision

- **The distance.** `DirectionalLight::shadow_distance`, in metres, 50 by default: how far from
  the camera the light casts shadows. It is checked as the light's other values are
  ([ADR-0042](0042-lights-beyond-the-most-drawn-and-checked-scene-values.md)). Unreal, Godot and
  Bevy also set it on the light.
- **The region.** The map covers the square, across the light, around the smallest sphere centred
  on the view's axis that holds the corners of the camera's near plane and of the plane at the
  distance. The sphere's radius depends on the field of view, the output's aspect and the two
  distances only: the map keeps its size as the camera turns. At 50 m, a 60° field of view and a
  16:9 output, the radius is 59 m and a texel 6 cm.
- **The grid.** Across the light, the map's center moves to the nearest multiple of a texel from
  the world's origin, computed in `f64`: as the camera moves, the map jumps by whole texels and
  each texel keeps covering the same ground. The texel grid is fixed in the world, not in the
  camera-relative space of [ADR-0048](0048-rendering-relative-to-the-camera.md).
- **The depth range.** Along the light, the map runs from the nearest to the farthest of the boxes
  of the instances that cast shadows and reach across the square, padded by 1/64 of the larger of
  the square's side and that range; instances beside the square are left out, a tower 2 km away
  as much as one in another level. Without such a box, there is no shadow pass.
- **The bias.** A texel of 6 cm, where it was 3 mm, made the slope-scaled bias fail: it is computed
  from the slope of the triangle stored in a texel, and on a curved surface, a sphere of the
  samples, the filter compares a point with texels of neighbouring facets flatter than its own,
  which shadowed specks of the lit side; and at a texel of 6 cm, the 4 to 8 times the slope that it
  needs push a shadow 25 to 50 cm away from what casts it, on a surface at 45°. The slope-scaled
  bias is removed. The scene shader instead looks each point up in the map moved off its surface
  along its normal (normal offset), towards the light, by 3 texels times the sine of the light's
  incidence: there, the point lies 3 tan(t) texels' depth nearer to the light than its own surface,
  while the filter's taps, up to 2√2 texels away, find that surface's plane at most 2.83 tan(t)
  texels' depth nearer than at the point, whatever the angle t; a convex surface curves away from
  the light beyond its plane. A shadow moves by at most 3 texels. The constant bias of 32 units of
  the depth's last bit stays, for rounding; 4 suffice on lavapipe and RADV. The texel's width
  reaches the shader in the frame's values.
- **The samples** set the distance their scenes need: the sample scene to the far side of its
  bounds from the camera, each frame; the image test's shapes to 7 m.

This amends ADR-0038 (the map's fit, the bias, the frame's values) and ADR-0049 (the map is no
longer fitted across the light to the instances that cast shadows, which set its depth range only).

## Alternatives considered

- **Fitting to the camera's view frustum.** Finer texels, but the map changes size and turns with
  the camera, and its edges shimmer at every move; the sphere trades some texels for stability.
- **The tighter of the camera's region and the casters' boxes.** Small scenes would keep their
  fine texels, but the casters' boxes change as instances move, which brings the shimmering back.
- **The distance on the `Scene` or the `Renderer`.** It belongs to the light's shadows, as the
  cascades' other settings will; engines put it there.
- **A receiver-plane depth bias.** It removes acne on flat receivers exactly, from the derivatives
  of the point's position in the map, but is unstable at grazing angles and at silhouettes, and
  needs those derivatives in uniform control flow; the normal offset is simpler and serves curved
  surfaces as well.
- **Fading shadows out near the distance.** Shadows now end at the map's edge; a fade is left to
  the cascades, which hand shadows from one map to the next.

## Consequences

- Shadows work in levels and open worlds, with texels set by the distance a game chooses, and do
  not shimmer as the camera moves. Small scenes get coarser texels than when the map was fitted to
  them, unless the game shortens the distance; the samples' shadows are slightly softer, and their
  reference images were rendered again.
- What lies beyond the region receives no shadow: shadows end where the map ends, at least the
  distance away from the camera, without fading.
- `tests/shadows.rs` computes each expected shadow from the world's texel grid, the filter's
  taps and the normal offset; the image of a sphere lit by the light must be the same with and
  without shadows where it faces the light.
- Cascades, a later stage's work, add maps of growing distances next to this one.
