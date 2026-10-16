# slime-color

The color type of the engine's API, and the sRGB transfer function
(`docs/adr/0045-one-linear-color-type.md`, `docs/adr/0079-the-engines-own-world.md`).

## Role

- `LinearColor`: four `f32`s, red, green and blue in linear light, without an upper bound, and
  straight alpha. Every color of the engine's API is one: the clear colors of `slime-gfx`, the
  colors of materials, lights and skies in `slime-render`, and those of a world's lights in
  `slime-world`.
- `srgb8_to_linear` and `linear_to_srgb8`: the sRGB transfer function between bytes and linear
  values, which `LinearColor::from_srgb8`, the mipmaps of `slime-gfx` and the cooker's mipmaps use.

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
