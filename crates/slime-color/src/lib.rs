#![doc = include_str!("../README.md")]

use std::sync::LazyLock;

use slime_reflect::Reflect;

/// A color with linear components: values proportional to the light they stand for, which light
/// adds and multiplies as they are, not sRGB-encoded as the values of a color picker or of a web
/// color are ([`LinearColor::from_srgb8`] converts those). Every color of the engine's API is one
/// (`docs/adr/0045-one-linear-color-type.md`): the color a render pass of `slime-gfx` clears its
/// target to, the colors of materials and lights and the radiances of the sky in `slime-render`,
/// and the colors of the lights of a world in `slime-world`.
///
/// Red, green and blue have no upper bound: a light's color times its strength, or the sky's
/// radiance in nits, can be far above 1. Alpha is straight, not premultiplied: the color's own
/// opacity. Each field that holds a color says which values it takes, and whether it reads alpha.
///
/// What a target keeps of a color depends on its format: 8-bit targets keep each component between
/// 0 and 1; `Rgba16Float` targets keep a half-precision value next to each component, beyond 0 to
/// 1 included: the driver rounds to the nearest value or toward zero, and a magnitude beyond
/// 65 504 becomes ±65 504 or an infinity.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "LinearColor")]
pub struct LinearColor {
    /// Red.
    pub r: f32,
    /// Green.
    pub g: f32,
    /// Blue.
    pub b: f32,
    /// Alpha (opacity), from 0, transparent, to 1, opaque.
    pub a: f32,
}

impl LinearColor {
    /// Opaque black.
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    /// Opaque white: each component 1.
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);

    /// The opaque color of linear red, green and blue.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// The color of linear red, green and blue, and alpha.
    #[must_use]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// The opaque color whose red, green and blue are sRGB-encoded bytes, as color pickers, image
    /// editors and web colors give them: `#E8A33D` is `LinearColor::from_srgb8(0xE8, 0xA3, 0x3D)`.
    /// Each component is decoded with [`srgb8_to_linear`], as the GPU decodes the texels of an sRGB
    /// texture: 0 stays 0, 255 becomes 1, 128 becomes about 0.216.
    ///
    /// ```
    /// use slime_color::LinearColor;
    ///
    /// let amber = LinearColor::from_srgb8(0xE8, 0xA3, 0x3D);
    /// // 0xA3, 163, is about 0.366 in linear light, not 163 / 255 = 0.639.
    /// assert!((amber.g - 0.366).abs() < 0.001);
    /// ```
    #[must_use]
    pub fn from_srgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(srgb8_to_linear(r), srgb8_to_linear(g), srgb8_to_linear(b))
    }

    /// The color in OKLCH (`docs/adr/0149-the-looks-foundations.md`), its alpha kept.
    #[must_use]
    pub fn to_oklch(self) -> Oklch {
        // Björn Ottosson's Oklab, from linear sRGB through the cone responses' cube roots.
        let l = (0.412_221_47 * self.r + 0.536_332_55 * self.g + 0.051_445_995 * self.b).cbrt();
        let m = (0.211_903_5 * self.r + 0.680_699_5 * self.g + 0.107_396_96 * self.b).cbrt();
        let s = (0.088_302_46 * self.r + 0.281_718_85 * self.g + 0.629_978_7 * self.b).cbrt();
        let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
        let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
        let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
        Oklch {
            l: lightness,
            c: (a * a + b * b).sqrt(),
            h: b.atan2(a).to_degrees().rem_euclid(360.0),
            a: self.a,
        }
    }
}

/// A color in OKLCH: its lightness, its chroma and its hue in Oklab, a space in which equal steps
/// look equal, so that a color lightened by a step looks one step lighter whatever its hue, and its
/// alpha. The interface's themes compute their colors in it
/// (`docs/adr/0149-the-looks-foundations.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    /// Lightness, from 0, black, to 1, white.
    pub l: f32,
    /// Chroma, how far from grey: 0 for a grey, about 0.37 at most within sRGB.
    pub c: f32,
    /// Hue, in degrees from 0 to 360: about 30 for red, 140 for green, 265 for blue. A grey's hue
    /// means nothing.
    pub h: f32,
    /// Alpha, as the color's.
    pub a: f32,
}

impl Oklch {
    /// The color in linear sRGB, outside sRGB's gamut where the lightness and the chroma lie
    /// outside it: components below 0 or above 1 ([`Oklch::to_linear_in_gamut`] keeps them in).
    #[must_use]
    pub fn to_linear(self) -> LinearColor {
        let (sin, cos) = self.h.to_radians().sin_cos();
        let (a, b) = (self.c * cos, self.c * sin);
        let l = (self.l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
        let m = (self.l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
        let s = (self.l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
        LinearColor {
            r: 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
            g: -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
            b: -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
            a: self.a,
        }
    }

    /// The color in linear sRGB, its chroma lowered as little as keeps every component from 0 to 1,
    /// its lightness and its hue kept: a lightened or darkened color that left sRGB's gamut comes
    /// back to its nearest of the same lightness and hue, as CSS maps colors into a gamut.
    #[must_use]
    pub fn to_linear_in_gamut(self) -> LinearColor {
        let l = self.l.clamp(0.0, 1.0);
        let inside = |color: LinearColor| {
            [color.r, color.g, color.b]
                .iter()
                .all(|component| (-1e-4..=1.0 + 1e-4).contains(component))
        };
        let color = Self { l, ..self }.to_linear();
        if inside(color) {
            return clamped(color);
        }
        // Halving the range of the chroma twenty times places it within a millionth.
        let (mut low, mut high) = (0.0, self.c.max(0.0));
        for _ in 0..20 {
            let middle = (low + high) / 2.0;
            if inside(
                Self {
                    l,
                    c: middle,
                    ..self
                }
                .to_linear(),
            ) {
                low = middle;
            } else {
                high = middle;
            }
        }
        clamped(Self { l, c: low, ..self }.to_linear())
    }
}

/// `color` with each of red, green and blue from 0 to 1.
fn clamped(color: LinearColor) -> LinearColor {
    LinearColor {
        r: color.r.clamp(0.0, 1.0),
        g: color.g.clamp(0.0, 1.0),
        b: color.b.clamp(0.0, 1.0),
        a: color.a,
    }
}

/// The linear value of each sRGB-encoded byte.
static SRGB_TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|byte| {
        let encoded = byte as f32 / 255.0;
        if encoded <= 0.04045 {
            encoded / 12.92
        } else {
            ((encoded + 0.055) / 1.055).powf(2.4)
        }
    })
});

/// The linear value, from 0 to 1, of an sRGB-encoded byte, decoded with the sRGB transfer function
/// (IEC 61966-2-1): a straight segment up to 0.04045, then `((c + 0.055) / 1.055)^2.4`. A table
/// computed once holds the 256 values.
#[must_use]
pub fn srgb8_to_linear(encoded: u8) -> f32 {
    SRGB_TO_LINEAR[usize::from(encoded)]
}

/// The sRGB encoding of a linear value, as a byte: the inverse of [`srgb8_to_linear`], rounded to
/// the nearest byte, a value below 0 or above 1 clamped to it.
#[must_use]
pub fn linear_to_srgb8(linear: f32) -> u8 {
    (linear_to_srgb(linear).clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The sRGB encoding of a linear value from 0 to 1, not rounded: a straight segment up to
/// 0.0031308, then `1.055 c^(1/2.4) − 0.055`.
///
/// ```
/// // Half the light is sRGB's 188 of 255.
/// assert!((slime_color::linear_to_srgb(0.5) * 255.0 - 187.5).abs() < 0.1);
/// ```
#[must_use]
pub fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_bytes_are_decoded_to_linear_components() {
        assert_eq!(
            LinearColor::from_srgb8(0, 255, 0),
            LinearColor::rgb(0.0, 1.0, 0.0)
        );
        let decoded = LinearColor::from_srgb8(10, 128, 232);
        // The straight segment below 0.04045, then ((c + 0.055) / 1.055)^2.4.
        let expected = [10.0 / 255.0 / 12.92, 0.215_860_5, 0.806_952];
        for (component, expected) in [decoded.r, decoded.g, decoded.b].into_iter().zip(expected) {
            assert!((component - expected).abs() < 1e-6, "{decoded:?}");
        }
        assert_eq!(decoded.a, 1.0);
    }

    #[test]
    fn oklch_gives_known_values_and_back() {
        // White, black and sRGB's red as Oklab's author gives them.
        let white = LinearColor::WHITE.to_oklch();
        assert!((white.l - 1.0).abs() < 1e-3 && white.c < 1e-3, "{white:?}");
        let black = LinearColor::BLACK.to_oklch();
        assert!(black.l.abs() < 1e-3 && black.c < 1e-3, "{black:?}");
        let red = LinearColor::rgb(1.0, 0.0, 0.0).to_oklch();
        assert!((red.l - 0.628).abs() < 1e-3, "{red:?}");
        assert!((red.c - 0.258).abs() < 1e-3, "{red:?}");
        assert!((red.h - 29.23).abs() < 0.1, "{red:?}");
        for color in [
            LinearColor::from_srgb8(0x4C, 0x8D, 0xFF),
            LinearColor::from_srgb8(0x16, 0x17, 0x1A),
            LinearColor::rgba(0.2, 0.7, 0.1, 0.5),
        ] {
            let back = color.to_oklch().to_linear();
            for (a, b) in [
                (color.r, back.r),
                (color.g, back.g),
                (color.b, back.b),
                (color.a, back.a),
            ] {
                assert!((a - b).abs() < 1e-4, "{color:?} {back:?}");
            }
        }
    }

    #[test]
    fn a_color_out_of_gamut_keeps_its_lightness_and_hue_with_less_chroma() {
        let accent = LinearColor::from_srgb8(0x4C, 0x8D, 0xFF).to_oklch();
        let lighter = Oklch {
            l: accent.l + 0.25,
            ..accent
        };
        assert!(lighter.to_linear().b > 1.0, "out of sRGB's gamut");
        let mapped = lighter.to_linear_in_gamut();
        assert!(
            [mapped.r, mapped.g, mapped.b]
                .iter()
                .all(|c| (0.0..=1.0).contains(c))
        );
        let back = mapped.to_oklch();
        assert!((back.l - lighter.l).abs() < 2e-3, "{back:?}");
        assert!((back.h - lighter.h).abs() < 1.0, "{back:?}");
        assert!(back.c < lighter.c);
    }

    #[test]
    fn encoding_a_decoded_byte_gives_it_back() {
        for byte in 0..=255 {
            assert_eq!(linear_to_srgb8(srgb8_to_linear(byte)), byte);
        }
        assert_eq!(linear_to_srgb8(-1.0), 0);
        assert_eq!(linear_to_srgb8(2.0), 255);
    }
}
