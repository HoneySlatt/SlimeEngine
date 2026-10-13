# 0032. Mipmaps computed by the engine when a texture is created

- Status: Accepted; amended by [0039](0039-image-based-lighting-computed-on-the-cpu.md) (callers may give a texture's levels)
- Date: 2026-10-13

## Context

M3.4 of Stage 3a gave `slime-gfx` textures without mipmaps, to be added only if the image tests
or the sample showed aliasing that the tolerance of the image tests could not absorb
(`docs/PLAN.md`, M3.4). The glTF scene of M3.5 showed it: its tiled floor, minified towards the
horizon, aliased and shimmered as the camera moved, and 42 pixels of the front view differed from
the lavapipe reference by more than 2 out of 255 on RADV, by 8 at most. A texture needs levels of
detail (mipmaps, defined in `docs/glossary.md`) for samplers to read it at the size it is drawn,
and someone must compute them: the caller, the engine on the CPU, or the engine on the GPU.

## Decision

- **Every texture has a full chain of mipmaps:** one level per halving of its larger side, down
  to one pixel. A sampler reads the level that fits; with linear filtering it blends the two
  nearest levels, with nearest filtering it picks one (`Filter`, `SamplerDescriptor`).
- **The engine computes them on the CPU when the texture is created** (`Gpu::create_texture`):
  each pixel of a level is the average of the pixels it replaces in the level before (two along
  a side of even length, three along a side of odd length but one along a side of one pixel,
  each weighted by how much of it the pixel covers), computed in linear light for the sRGB
  formats (decoded, averaged, encoded again), and every level is uploaded with the texture. The
  caller gives the base level only and cannot give levels of its own.

This amends [ADR-0026](0026-resource-binding-and-depth.md): the textures it left to the
milestone that needed them come with mipmaps that the engine computes.

## Alternatives considered

- **No mipmaps.** The floor of the sample aliased and shimmered, and the image tests exceeded
  their tolerance on RADV.
- **Levels given by the caller.** glTF files carry no mipmaps, so the examples' loader would have
  computed them itself, and the asset pipeline of Stage 4, which could cook them, does not exist
  yet. Accepting caller-supplied levels can be added when a caller has some; it is an addition
  to the API, not a change.
- **Levels computed on the GPU,** a pass per level. Faster for large textures, but a pipeline
  and passes inside `Gpu::create_texture` for textures that are small today (the sample's are
  128 × 128 pixels). To reconsider when textures grow or when the asset pipeline cooks them.

## Consequences

- Creating a texture does work proportional to its pixels on the calling thread: the levels
  together hold a third as many pixels as the base level, each read from four or fewer (up to
  nine where a side has an odd length). Recorded in the known debt of `docs/PLAN.md`.
- Drivers blend levels differently where a minified texture changes level: the image tests
  differ more from the lavapipe references on other drivers, which is why
  [ADR-0030](0030-image-tests-strict-on-the-reference-driver.md) loosened their tolerance there.
- Textures take a third more GPU memory than their base level.
