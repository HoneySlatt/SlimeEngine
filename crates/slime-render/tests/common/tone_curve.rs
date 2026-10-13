//! What the renderer displays for a light, computed on the CPU independently of its shaders:
//! Khronos PBR Neutral, written from its specification
//! (<https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md>), then the
//! encoding of an 8-bit sRGB target. Shared by the tests through `#[path]`; `tone_mapping.rs`
//! checks it against values derived by hand from the specification.

/// Khronos PBR Neutral of a linear color already multiplied by the exposure, not negative.
pub fn tone_map(color: [f32; 3]) -> [f32; 3] {
    const START_COMPRESSION: f32 = 0.76;
    const DESATURATION: f32 = 0.15;
    let x = color[0].min(color[1]).min(color[2]);
    let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    let shifted = color.map(|channel| channel - offset);
    let peak = shifted[0].max(shifted[1]).max(shifted[2]);
    if peak < START_COMPRESSION {
        return shifted;
    }
    let d = 1.0 - START_COMPRESSION;
    let new_peak = 1.0 - d * d / (peak + d - START_COMPRESSION);
    let g = 1.0 - 1.0 / (DESATURATION * (peak - new_peak) + 1.0);
    shifted.map(|channel| {
        let scaled = channel * (new_peak / peak);
        scaled + (new_peak - scaled) * g
    })
}

/// The sRGB encoding of a linear value, out of 255.
pub fn srgb(linear: f32) -> u8 {
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// What an 8-bit sRGB target shows for a linear color already multiplied by the exposure.
pub fn displayed(color: [f32; 3]) -> [u8; 3] {
    tone_map(color).map(srgb)
}
