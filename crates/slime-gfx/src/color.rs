//! The color type of the engine's API (ADR-0045).

use crate::texture::SRGB_TO_LINEAR;

/// A color with linear components: values proportional to the light they stand for, which light
/// adds and multiplies as they are, not sRGB-encoded as the values of a color picker or of a web
/// color are ([`LinearColor::from_srgb8`] converts those). Every color of the engine's API is one
/// (ADR-0045): the color a render pass clears its target to, and in `slime-render` the colors of
/// materials and lights and the radiances of the sky.
///
/// Red, green and blue have no upper bound: a light's color times its strength, or the sky's
/// radiance in nits, can be far above 1. Alpha is straight, not premultiplied: the color's own
/// opacity. Each field that holds a color says which values it takes, and whether it reads alpha.
///
/// What a target keeps of a color depends on its format: 8-bit targets keep each component between
/// 0 and 1; `Rgba16Float` targets keep a half-precision value next to each component, beyond 0 to
/// 1 included: the driver rounds to the nearest value or toward zero, and a magnitude beyond
/// 65 504 becomes ±65 504 or an infinity.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Each component is decoded with the sRGB transfer function (IEC 61966-2-1), as the GPU
    /// decodes the texels of an sRGB texture: 0 stays 0, 255 becomes 1, 128 becomes about 0.216.
    ///
    /// ```
    /// use slime_gfx::LinearColor;
    ///
    /// let amber = LinearColor::from_srgb8(0xE8, 0xA3, 0x3D);
    /// // 0xA3, 163, is about 0.366 in linear light, not 163 / 255 = 0.639.
    /// assert!((amber.g - 0.366).abs() < 0.001);
    /// ```
    #[must_use]
    pub fn from_srgb8(r: u8, g: u8, b: u8) -> Self {
        let linear = |encoded: u8| SRGB_TO_LINEAR[usize::from(encoded)];
        Self::rgb(linear(r), linear(g), linear(b))
    }

    pub(crate) fn to_wgpu(self) -> wgpu::Color {
        wgpu::Color {
            r: f64::from(self.r),
            g: f64::from(self.g),
            b: f64::from(self.b),
            a: f64::from(self.a),
        }
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
}
