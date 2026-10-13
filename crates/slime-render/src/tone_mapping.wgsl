// The tone-mapping pass (ADR-0034): one triangle covers the output, and each pixel reads the
// texel of the scene's high-dynamic-range target under it, multiplies it by the exposure, maps it
// to the range a display shows with Khronos PBR Neutral, and writes it opaque. The output target's
// format encodes the result (sRGB for a window).

// The scene's light, drawn by the scene pass at the output's size: linear, and may exceed 1.
@group(0) @binding(0) var scene_light: texture_2d<f32>;

struct ToneMappingUniform {
    // What the scene's light is multiplied by before the curve; the other three floats pad the
    // uniform to 16 bytes.
    exposure: vec4<f32>,
}
@group(0) @binding(1) var<uniform> tone_mapping: ToneMappingUniform;

// The corners (-1, -1), (3, -1) and (-1, 3), counter-clockwise: one triangle that covers the
// target, built from the vertex index without a vertex buffer.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index & 1u) * 4u) - 1.0, f32((index & 2u) * 2u) - 1.0);
    return vec4<f32>(corner, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    // A pixel's center is at (x + 0.5, y + 0.5): truncating gives its texel, since the scene's
    // target has the output's size.
    let light = textureLoad(scene_light, vec2<u32>(position.xy), 0).rgb;
    return vec4<f32>(pbr_neutral(light * tone_mapping.exposure.x), 1.0);
}

// Khronos PBR Neutral, written from the equations of its specification:
// https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md

// Where the compression of highlights starts, K_s: 0.8 minus F90 = 0.04, the light that common
// dielectrics reflect at normal incidence, which the offset below removes.
const START_COMPRESSION: f32 = 0.76;
// How fast compressed highlights move towards white, K_d.
const DESATURATION: f32 = 0.15;

// In: linear Rec. 709, not negative. Out: linear, from 0 to below 1, with the hue kept. Colors
// whose channels all lie between 0.08 and 0.8 come out lowered by 0.04; darker ones lose less,
// nothing at 0; brighter ones are compressed towards 1 and move towards white.
fn pbr_neutral(color: vec3<f32>) -> vec3<f32> {
    let x = min(color.r, min(color.g, color.b));
    let offset = select(0.04, x - 6.25 * x * x, x < 0.08);
    let shifted = color - offset;
    let peak = max(shifted.r, max(shifted.g, shifted.b));
    if peak < START_COMPRESSION {
        return shifted;
    }
    let d = 1.0 - START_COMPRESSION;
    let new_peak = 1.0 - d * d / (peak + d - START_COMPRESSION);
    let g = 1.0 - 1.0 / (DESATURATION * (peak - new_peak) + 1.0);
    return mix(shifted * (new_peak / peak), vec3<f32>(new_peak), g);
}
