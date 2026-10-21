# 0064. Cascaded shadow maps

- Status: Accepted; amends [0038](0038-shadow-mapping.md) (one map for the directional light),
  [0050](0050-a-shadow-region-that-follows-the-camera.md) (the region, now one per cascade),
  [0052](0052-amend-roadmap-before-0-1.md) and
  [0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) (cascades move from Stage 9a to
  Stage 3d), and [0063](0063-a-smoother-shadow-filter.md) (cascades, which it left apart); amended
  by [0065](0065-reading-the-cascades.md) (how a cascade is chosen and its band read) and
  [0126](0126-views-apart-from-the-renderer.md) (the cascades fitted to each view's camera, drawn in
  turn into one atlas)
- Date: 2026-10-15

## Context

The directional light cast its shadows through one map of 2048 × 2048 texels, which covered the
sphere around everything the camera sees up to the light's shadow distance
([ADR-0050](0050-a-shadow-region-that-follows-the-camera.md)): the ground near the camera got texels
as wide as the ground at the far end. In *Nightwatch*, whose moon casts shadows up to 30 m, a texel
covers about 3.4 cm, 6 to 7 pixels of a 3840 × 2160 screen near the player. A smoother filter
([ADR-0063](0063-a-smoother-shadow-filter.md)) softened the steps that a shadow's edge makes across
those texels but did not remove them where the edge runs nearly along the texels.

*Cascaded shadow maps* split what the camera sees, along its axis, into slices, and give each slice
a map of its own: near the camera, a map covers less ground with as many texels. Unity, Unreal and
Godot all shadow their sun this way. Stage 9a held them
([ADR-0052](0052-amend-roadmap-before-0-1.md),
[ADR-0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md)); the owner decided on
2026-10-15 to build them now, in Stage 3d.

A study of the game's camera, before the decision, showed their limit: the camera's wide view
covers about 29 m across at 14 m from it, and the cascade that holds those points must hold all of
that width. Near the player, a texel would cover about 1.6 cm, half its width, not a quarter, about
3 pixels at 3840 × 2160. Going further, by fitting the cascades every frame to what the camera
actually sees, or with virtual shadow maps, builds on cascades and is left to Stage 9a. The owner
took the three decisions below on 2026-10-15, knowing this.

## Decision

- **Four cascades, each twice as deep as the one before.** The slices end at 1, 3, 7 and 15
  fifteenths of the light's shadow distance (for 30 m: 2, 6, 14 and 30 m), the first starting at
  the camera's near plane: Unity's default split of four cascades. Where the slices end depends on
  the shadow distance alone, not on the near plane, and the texels' width grows steadily with the
  distance.
- **2048 × 2048 texels each, in one atlas of 4096 × 4096** (64 MiB), the cascades' tiles in two
  rows of two, apart from the spot lights' atlas
  ([ADR-0058](0058-shadows-for-every-spot-light-in-one-atlas.md)). It is created by the first frame
  in which a directional light casts shadows, where the single map was created with the renderer.
  The last cascade's texels are as wide as the single map's were; the nearer ones' are finer.
- **Each cascade is fitted as ADR-0050 fitted the single map**, to its own slice: the square,
  across the light, around the smallest sphere centred on the view's axis that holds the corners of
  the slice's two planes, which keeps its size as the camera turns; its center moved to a texel grid
  fixed in the world, so that its shadows do not shimmer as the camera moves; along the light, the
  depths of the boxes of the instances that cast shadows and reach across that square. An instance
  is drawn in every cascade whose square its box reaches. The normal offset, the bias and the
  filter of ADR-0050 and ADR-0063 apply to each cascade, in its own texels.
- **A blend band between cascades.** A point is shadowed by the first cascade whose square holds
  it; within the band at the edge of a cascade, a tenth of its width, its shadow fades into the next
  cascade's, so that no seam shows where the texels' width changes as the camera moves. Beyond the
  last cascade, the shadow fades out over the same band.

## Alternatives considered

- **Three cascades**: fewer draws, and the shadows near the camera twice as coarse.
- **1024 × 1024 per cascade** (16 MiB, the single map's memory): the last cascade, which covers
  as much ground as the single map did, twice as coarse as it.
- **4096 × 4096 per cascade** (256 MiB): twice as fine everywhere, for too much memory.
- **A hard boundary between cascades**: the cheapest, one comparison per point, and a line where the
  sharpness of the shadows changes at once, moving with the camera.
- **Slices split by a blend of logarithmic and even splits**, which depend on the camera's near
  plane: a near plane of 1 cm or of 10 cm would move every slice.
- **Squares fitted tightly to each slice**, instead of to a sphere around it: finer texels, but a
  square that changes size as the camera turns, whose shadows shimmer.
- **A single larger map** (ADR-0063): texels half as wide everywhere, for 48 MiB more, and its
  steps still show.

## Consequences

- The shadow pass draws an instance once in each cascade its box reaches: up to four times the
  draws of the single map.
- The cascades' atlas takes 64 MiB of GPU memory, where the single map took 16; a scene whose
  directional lights cast no shadows creates none.
- At the default 50 m, with a 60° field of view and a 16:9 output, a texel covers about 4 mm in the
  first cascade, 1.2 cm in the second, 2.7 cm in the third and 6 cm in the last.
- Within the band, the scene shader compares a point with two cascades: twice the comparisons, on
  the pixels that lie there.
