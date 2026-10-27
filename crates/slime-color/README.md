# slime-color

The color type of the engine's API, and the sRGB transfer function
(`docs/adr/0045-one-linear-color-type.md`, `docs/adr/0079-the-engines-own-world.md`).

## Role

- `LinearColor`: four `f32`s, red, green and blue in linear light, without an upper bound, and
  straight alpha. Every color of the engine's API is one: the clear colors of `slime-gfx`, the
  colors of materials, lights and skies in `slime-render`, and those of a world's lights in
  `slime-world`.
- `srgb8_to_linear` and `linear_to_srgb8`: the sRGB transfer function between bytes and linear
  values, which `LinearColor::from_srgb8`, the mipmaps of `slime-gfx` and the cooker's mipmaps use;
  `linear_to_srgb`, the encoding not rounded to a byte, with which the renderer's canvas encodes
  the colors that it mixes as sRGB values (`docs/adr/0137-the-canvas.md`), and `srgb_to_linear`,
  its inverse, with which the interface's color wells read the colors that they pick
  (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`).
- `Oklch`: a color's lightness, chroma and hue in Oklab, where equal steps look equal
  (`LinearColor::to_oklch`, `Oklch::to_linear`), and a color brought back into sRGB's gamut by
  lowering its chroma alone (`Oklch::to_linear_in_gamut`): the interface's themes compute their
  colors with it (`docs/adr/0149-the-looks-foundations.md`).

It lives below every crate that holds a color, so that one that has no GPU, such as the world,
holds colors without the GPU layer.

## Entry points

```rust
use slime_color::LinearColor;

let warm = LinearColor::rgb(1.0, 0.8, 0.6);
let amber = LinearColor::from_srgb8(0xE8, 0xA3, 0x3D);
assert!(amber.r > amber.g);
assert_eq!(warm.a, 1.0);
```

The rustdoc holds the contracts: which values each component takes, and how bytes are decoded.

## Dependencies

`slime-reflect`, which it describes `LinearColor` to
(`docs/adr/0080-reflection-and-serialization.md`).

## Must not

- Depend on another crate of the engine than `slime-reflect`, or on the GPU: every crate that holds
  a color depends on it.
- Check the values of a color: what a valid color is depends on the field that holds it (a clear
  color may be negative on a floating-point target, a sky must stay within half precision).
