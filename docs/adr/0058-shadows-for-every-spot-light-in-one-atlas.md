# 0058. Shadows for every spot light, in one atlas

- Status: Accepted; amends [0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) (how many
  spot lights cast shadows, and through what); amended by
  [0126](0126-views-apart-from-the-renderer.md) (the tiles drawn once a frame)
- Date: 2026-10-15

## Context

[ADR-0056](0056-stage-3d-anti-aliasing-and-spot-light-shadows.md) added Stage 3d, in which at most
four of a scene's spot lights cast shadows, each through a shadow map of its own. When the stage was
planned, the game it serves, *Nightwatch*, turned out to have five spot lights when a game starts:
three guards' flashlights and two floodlights. With four maps, one of them would still light
through walls. The renderer draws at most 16 lights (`MAX_LIGHTS`), so a scene has at most 16 spot
lights that could cast shadows.

Engines keep the shadow maps of their spot lights in one large depth texture, an *atlas*, split
into square tiles, one per light: Unity's, Godot's and Unreal's do. Godot and Unreal size each tile
by how large the light shows on screen; Unity lets a game pick a size per light.

The owner decided on 2026-10-15, when Stage 3d was planned.

## Decision

- **Every spot light drawn may cast shadows**, up to the 16 lights the renderer draws.
- **`SpotLight::casts_shadows`, on by default**, as `DirectionalLight::casts_shadows` is.
- **One atlas**, a depth texture of 4096 × 4096 texels, split into square tiles, one for each spot
  light that casts shadows, each seen in perspective from its light and filtered as the directional
  light's map is. One shadow pass draws every tile.
- **Tiles sized by how many lights cast shadows**: 2048 × 2048 each for up to four lights, 1024 ×
  1024 each from five to sixteen. Every light that casts shadows gets a tile.
- **Tiles given in the scene's order**, among the lights drawn, as the first directional light
  that casts shadows gets the directional map ([ADR-0038](0038-shadow-mapping.md)).
- **Sizing tiles by how large each light shows on screen** waits for Stage 9a. It needs to know the
  same light from one frame to the next, so that a light near the boundary between two sizes does
  not change size at every frame; lights have no identity before the world model of Stage 5. The
  change stays inside the renderer: a game sees no difference in the API.

How each tile is projected, how its depths are biased and how the scene shader reads it are
decided, and recorded, with their implementation.

## Alternatives considered

- **At most four spot lights, each with a map of its own** (ADR-0056): *Nightwatch* has five.
- **At most eight, in tiles of one size**, 1024 × 1024 in an atlas of 4096 × 2048 (32 MiB): the
  first proposal; the owner preferred tiles whose size varies.
- **A texture array**, one layer per light, all of one size: tiles of one size only, and a pass per
  light in the render graph.
- **A size that the game picks for each light**, as Unity does: a setting to give every lamp.
- **A size by how large the light shows on screen**, as Godot and Unreal do: without knowing a light
  from one frame to the next, its shadows would flicker between two sizes.
- **Shadows off by default**, as Godot and Bevy have them: the owner preferred them on, as the
  directional light has them.

## Consequences

- The atlas takes 64 MiB of GPU memory (4 bytes a texel).
- Each spot light that casts shadows draws, into its tile, the instances inside its cone and range.
  A game with many lights turns off the shadows of those that need none.
- Turning a light that casts shadows on or off can move the others across sizes: from four lights
  to five, every tile halves, and the shadows grow a little softer at once.
- glTF scenes' spot lights cast shadows: the images of the samples change.
- The roadmap's Stage 3d says so, and its Stage 9a keeps sizing tiles by the light's size on screen.
