//! What a test reads in an `Rgba16Float` output, against what the renderer should show for a
//! light: the tone curve of `tone_curve.rs`. Shared by the tests that read float outputs through
//! `#[path]`, which also include `tone_curve.rs` as `tone_curve`.

use slime_gfx::FloatImage;

use crate::tone_curve::tone_map;

/// Checks that the pixel at `(x, y)` shows `light`, high-dynamic-range light already exposed,
/// tone mapped, within 2 × 10^-3: two roundings to half precision (the high-dynamic-range target
/// and the output), each up to one step of 4.9 × 10^-4 below 1 even on a driver that rounds toward
/// zero, and the rounding of the shader, about 10^-6; half a step of an 8-bit display.
pub fn assert_shows(image: &FloatImage, (x, y): (u32, u32), light: [f64; 3], case: &str) {
    let expected = tone_map(light.map(|channel| channel as f32));
    let start = ((y * image.width + x) * 4) as usize;
    let actual = &image.pixels[start..start + 3];
    let largest = actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f32::max);
    assert!(
        largest <= 2e-3,
        "{case}: expected {expected:?} for light {light:?}, got {actual:?} (off by {largest})"
    );
}
