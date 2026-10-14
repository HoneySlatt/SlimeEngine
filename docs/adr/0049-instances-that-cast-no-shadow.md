# 0049. Instances that cast no shadow

- Status: Accepted; amends [0038](0038-shadow-mapping.md); amended by
  [0050](0050-a-shadow-region-that-follows-the-camera.md) (the casters set the map's depths only)
- Date: 2026-10-14

## Context

The shadow map covers the boxes of every instance ([ADR-0038](0038-shadow-mapping.md)), 2 048
texels across. One large instance spreads them over its extent: a floor 2 km wide makes them 1 m
wide, and every shadow of the scene a blur, though the floor itself casts no shadow anyone sees. A
sky dome or a distant mountain does the same, and each costs a draw in the shadow pass.

An audit of the debt before 0.1 listed this; the owner decided on 2026-10-14 that a game can say
that an instance casts no shadow, alongside the shadow region that follows the camera.

## Decision

- `Instance::casts_shadows`, `true` by `Instance::new`. An instance that casts no shadow is left
  out of the shadow pass and of the boxes the map is fitted to; it still receives shadows.
- The pass is skipped when no instance casts shadows, as when no light does.
- A point beyond the map's sides is lit: every instance that casts shadows lies within the map,
  so nothing there can shadow it. A point beyond the map's far end, farther from the light than
  every instance that casts, is compared as if it were at that end: shadowed where something is
  stored, lit where nothing is. Before, every receiver was also a caster and lay within the map;
  the sampler's clamping to the map's edge would otherwise shadow a floor that casts nothing
  wherever it reaches beyond the map.

## Alternatives considered

- **A flag on the material.** Whether something casts a shadow belongs to the object, not to its
  look: the same material serves a floor that casts none and a wall that does.
- **Leaving receivers beyond the map to the clamped edge.** It shadows a floor that casts nothing
  wherever the edge holds a caster.

## Consequences

- A game keeps the map for what casts the shadows it sees. Leaving out an instance that does cast
  a visible shadow removes it: the choice is the game's.
- Varyings interpolated across a single triangle hundreds of kilometres wide lose precision on some
  drivers (RADV), so that where such a triangle meets the map's sides, a pixel may take the wrong
  side: a floor that large should be split into smaller triangles, as terrain is.
