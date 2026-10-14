// The tone-mapping pass (ADR-0034, ADR-0037, ADR-0040): one triangle covers the output, and each
// pixel reads the texel of the scene's high-dynamic-range target under it, mixes it with the bloom
// chain's blurred copy of the scene by the scene's bloom strength, maps the result to the range a
// display shows with Khronos PBR Neutral, and writes it opaque. The scene pass has applied the
// exposure. The output target's format encodes the result (sRGB for a window).

// The scene's exposed light divided by HEADROOM, drawn by the scene pass at the output's size:
// linear, from 0 to 65504.
@group(0) @binding(0) var scene_light: texture_2d<f32>;
// The bloom chain's blurred copy of it, half the output's size rounded to the nearest (the scene's
// light itself, with a strength of 0, when the output is too small for a chain), and the linear
// sampler, mirrored at the edges, that magnifies it.
@group(0) @binding(1) var bloom_light: texture_2d<f32>;
@group(0) @binding(2) var bloom_sampler: sampler;
struct Bloom {
    // The share of each pixel's light that the blurred copy replaces, from 0 to 1; 0 when the
    // frame skipped the chain.
    strength: f32,
}
@group(0) @binding(3) var<uniform> bloom: Bloom;

// What the scene and sky passes divided exposed light by, so that light up to 64 × 65 504 keeps
// its energy for the bloom (ADR-0040): HEADROOM in shader.wgsl, which a unit test compares with it.
const HEADROOM: f32 = 64.0;

// The corners (-1, -1), (3, -1) and (-1, 3), counter-clockwise: one triangle that covers the
// target, built from the vertex index without a vertex buffer.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index & 1u) * 4u) - 1.0, f32((index & 2u) * 2u) - 1.0);
    return vec4<f32>(corner, 0.0, 1.0);
}

fn bloom_tap(at: vec2<f32>, offset: vec2<f32>, texel: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(bloom_light, bloom_sampler, at + offset * texel, 0.0).rgb;
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    // A pixel's center is at (x + 0.5, y + 0.5): truncating gives its texel, since the scene's
    // target has the output's size.
    var light = textureLoad(scene_light, vec2<u32>(position.xy), 0).rgb;
    // A branch on a uniform: every pixel takes the same way, and a strength of 0 costs nothing.
    if bloom.strength > 0.0 {
        // The chain's first level has twice the output's pixels per texel: the pixel falls at half
        // its position, read through the same 3 x 3 tent as the chain's own averages, so that the
        // glow of a small, bright source stays round wherever it lies.
        let texel = 1.0 / vec2<f32>(textureDimensions(bloom_light));
        let at = 0.5 * position.xy * texel;
        let corners = bloom_tap(at, vec2<f32>(-1.0, -1.0), texel)
            + bloom_tap(at, vec2<f32>(1.0, -1.0), texel)
            + bloom_tap(at, vec2<f32>(-1.0, 1.0), texel)
            + bloom_tap(at, vec2<f32>(1.0, 1.0), texel);
        let sides = bloom_tap(at, vec2<f32>(0.0, -1.0), texel)
            + bloom_tap(at, vec2<f32>(-1.0, 0.0), texel)
            + bloom_tap(at, vec2<f32>(1.0, 0.0), texel)
            + bloom_tap(at, vec2<f32>(0.0, 1.0), texel);
        let center = bloom_tap(at, vec2<f32>(0.0, 0.0), texel);
        let blurred = (corners + 2.0 * sides + 4.0 * center) / 16.0;
        // Energy-conserving: the share of the light that the lens spreads is taken from where it
        // lands, before the curve, as light rather than display values.
        light = mix(light, blurred, bloom.strength);
    }
    return vec4<f32>(pbr_neutral(light * HEADROOM), 1.0);
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
