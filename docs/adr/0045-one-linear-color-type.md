# 0045. One linear color type

- Status: Accepted
- Date: 2026-10-14

## Context

The engine's API wrote colors three ways: `slime_gfx::Color`, four `f64`s, for the color a render
pass clears its target to; `[f32; 4]` and `[f32; 3]` for a material's base color and emission; and
glam's `Vec3` for the color of a light and the radiances of the sky. All three hold linear values,
as [ADR-0027](0027-the-render-crate.md) requires, but only the documentation said so: nothing in a
signature told a caller that `[0.5, 0.5, 0.5, 1.0]` is a linear grey, which a display shows as
`#BCBCBC`, not the `#808080` of a color picker. Colors that games take from artists, palettes and
color pickers are sRGB-encoded; passed as they are, they come out washed out and too bright, with
no error. This is the most common color mistake in game code, and a game engine that aims at large
productions should make it hard.

An audit of the debt before 0.1 listed the three representations; the owner decided on 2026-10-14
on a dedicated linear color type, as Unreal's `FLinearColor`, for materials, lights and the sky,
and if possible for clears too, so that there is one way.

## Decision

- `slime_gfx::LinearColor` is the color type of every API of the engine: four `f32`s, `r`, `g`, `b`
  and `a`, public. Red, green and blue are linear and have no upper bound: a light's color times
  its strength, or a sky's radiance in nits, can be far above 1. Alpha is straight (not
  premultiplied).
- It replaces `slime_gfx::Color` in `RenderPassDescriptor::clear`, the arrays of
  `MaterialData::base_color` and `MaterialData::emissive`, and the `Vec3` of the lights' `color`
  and of `Sky::zenith`, `Sky::horizon` and `Sky::ground`.
- Each field that holds one says which values it takes and whether it reads alpha. Only a
  material's base color and a clear color read it; emission, lights and the sky ignore it. The
  fields check their own ranges, as before
  ([ADR-0042](0042-lights-beyond-the-most-drawn-and-checked-scene-values.md)); the type checks
  nothing.
- `LinearColor::rgb`, `LinearColor::rgba`, `LinearColor::BLACK` and `LinearColor::WHITE` make one
  from linear values. `LinearColor::from_srgb8(r, g, b)` decodes sRGB-encoded bytes with the sRGB
  transfer function, through the table that already decodes sRGB texels when mipmaps are computed
  ([ADR-0032](0032-mipmaps-computed-at-texture-creation.md)).
- It lives in `slime-gfx`, the lowest crate that has a color in its API; `slime-render` uses it and
  does not re-export it, as it does not re-export the textures and samplers of materials. Inside
  `slime-render`, the light's math converts the colors whose alpha is ignored to glam's `Vec3`.
- Textures of colors are unchanged: they keep an sRGB format, which the GPU decodes
  ([ADR-0035](0035-physically-based-materials.md)).

## Alternatives considered

- **Keeping arrays and vectors, documented as linear.** Three ways to write a color, and nothing
  in the code that tells an sRGB value from a linear one.
- **A type per encoding (`LinearRgba` and `Srgba`, and a `Color` enum over them, as Bevy does).**
  Conversions at every use, and more API to learn, for a stored sRGB color that nothing in the
  engine needs: games convert once, where the color comes in.
- **A type without alpha, with alpha apart in the two fields that read it.** Two types, or a
  base color split in two fields, against glTF's single factor and the habit of every engine.
- **`f64` components, as `wgpu::Color`.** No target holds more than an `f32`, and materials and
  lights are uploaded as `f32`s; the clear color converts to `wgpu::Color` when a pass starts.
- **The type in `slime-render`.** Clears, in `slime-gfx`, would keep a color type of their own.
- **A crate of its own (a `slime-color` or a math crate).** No crate below `slime-gfx` needs
  colors yet. If the asset pipeline or the editor needs them without the GPU layer, the type moves
  down.
- **Constructors that refuse negative or non-finite values.** What is valid depends on the field:
  a clear color may be negative on a floating-point target, a sky must stay within half precision.

## Consequences

- A color in the API reads as linear at every use, and an sRGB color has one way in.
- Every game that wrote a color changes: `[1.0, 0.0, 0.0, 1.0]` becomes `LinearColor::rgb(1.0,
  0.0, 0.0)`, `Vec3::ONE` becomes `LinearColor::WHITE`. Nothing is promised stable before 1.0.
  Images do not change.
- Light math in `slime-render` converts colors to `Vec3`, a one-line function; tests that compute
  expected light do the same.
- Moving the type to a lower crate later changes its path, not its use.
