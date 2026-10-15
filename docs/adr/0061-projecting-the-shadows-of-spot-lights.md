# 0061. Projecting the shadows of spot lights

- Status: Accepted; amends [0038](0038-shadow-mapping.md) (the shadow pass reads its view from a
  buffer of views); amended by [0062](0062-looking-spot-shadows-up-off-the-triangle.md) (the
  normal offset of spot lights)
- Date: 2026-10-15

## Context

Every spot light drawn may cast shadows, each through a tile of one atlas, a depth texture of
4096 × 4096 texels, sized by how many cast them
([ADR-0058](0058-shadows-for-every-spot-light-in-one-atlas.md)). This record decides how a tile is
drawn: what it sees, which instances it draws, and how its depths are kept from shadowing the
surfaces they come from.

The directional light's shadow map ([ADR-0038](0038-shadow-mapping.md),
[ADR-0050](0050-a-shadow-region-that-follows-the-camera.md)) is orthographic, fitted to the region
around the camera, with reversed depth ([ADR-0047](0047-reversed-depth-with-an-infinite-far-plane.md)),
a constant depth bias, and a lookup moved off the surface along its normal by up to 3 texels, which
its 3 × 3 filter needs. Its shadow pass reads the light's matrix from the frame's values.

A spot light's cone may be up to π / 2 wide, which a perspective projection cannot hold; a light
without range reaches every distance; and a light often sits inside the mesh of its lamp.

## Decision

- **A perspective projection from the light along its axis**, square, reversed and infinite as the
  camera's: depth is 0.05 m / d at a distance d along the axis. What lies nearer the light than
  0.05 m casts no shadow, so that a lamp the light sits in, if no larger than that, does not hide
  it; a game turns off the shadows of a larger lamp. A light without range needs no far plane.
- **Wide cones are cut at 80° from the axis.** A cone wider than that is seen up to 80°, and its
  light casts no shadow beyond.
- **A margin around the cone.** The tile sees its cone and 6 texels more on each side, so that the
  filter of a point on the cone's edge, looked up off its surface, still reads the tile's own
  texels: the filter reaches a texel and a half, and the normal offset moves a point by up to
  three.
- **The instances each tile draws.** An instance that casts shadows is drawn into a tile when the
  sphere around its box meets the cone through the tile's corners, in front of the light and
  within its range. Without this, every tile would draw every instance.
- **One pass, one view per tile.** The spot lights' shadow pass draws every tile, each into its
  rectangle of the atlas (`RenderPass::set_viewport`). The shadow passes read their view from a
  buffer of views, one slot per map or tile, bound at its offset for each: the directional light's
  first, then the tiles'. The directional light's shadow pass moves onto it too, so that both
  passes share their pipelines.
- **The atlas is created by the first frame in which a spot light casts shadows**, then kept: a
  renderer that draws no such light does not hold its 64 MiB.
- **Bias, as the directional light's.** The tiles are drawn with the directional light's constant
  depth bias, and the scene shader looks a point up off its surface along its normal, towards the
  light, by 3 texels times the sine of the light's incidence, where a texel of a tile is
  2 d tan(h) / N wide at a distance d along its axis, h the angle from the axis to the tile's sides
  and N its side in texels. The shader's lookup comes with the next milestone, which checks this
  bias on three drivers.

## Alternatives considered

- **Six faces of a cube, as for a point light**, for any cone: six times the draws, for cones wider
  than 80°, which a torch or a stage light is not.
- **A fixed far plane**, the light's range: a light without range would need one picked, and
  reversed infinite depth keeps its precision without it.
- **No culling**: every tile draws every instance; 155 draws in *Nightwatch* instead of 46.
- **Each view in the frame's values, and a pipeline per view**: a pipeline cannot change the
  matrix it reads between two draws of one pass without a dynamic offset, which the buffer of views
  gives.
- **Creating the atlas with the renderer**, as the directional light's map is: 64 MiB held by every
  renderer, the editor's thumbnails to come included.

## Consequences

- A lamp that holds its light within 5 cm, or a light without a lamp, needs nothing of a game; a
  larger lamp must not cast shadows, or the light must stand outside it.
- The first frame with a spot light that casts shadows can fail with `GfxError::OutOfMemory`, as
  any frame that creates a target can.
- In *Nightwatch*, on an RX 6900 XT, the five tiles cost 46 draws and about 10 µs of GPU time.
