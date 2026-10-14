// The bloom chain (ADR-0040): the scene's exposed light filtered down into levels of half the size
// of the one before, then averaged back up, level by level, into a blurred copy of the scene that
// the tone-mapping pass mixes with it. Each pass draws one triangle over its target and reads the
// level before it through a linear sampler mirrored at the edges, so that light beyond the image is
// the image's own light reflected: a uniform image stays uniform, no halo wraps around to the
// other side, and a pixel near an edge keeps its light, where clamping to the edge would count the
// edge's texels again at every level. Every read is at level 0, without derivatives and outside
// any loop, which FXC requires (ADR-0036).

// The level read: the finer level when filtering down (the scene's light for the first level), the
// average of the coarser levels when averaging up.
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

// Averaging up only: this level's own light, filtered down, and its share of the average.
@group(0) @binding(2) var level: texture_2d<f32>;
struct Weight {
    // 1 / the number of levels from this one to the coarsest, so that every level ends with the
    // same share of the blurred copy.
    own: f32,
}
@group(0) @binding(3) var<uniform> weight: Weight;

// The corners (-1, -1), (3, -1) and (-1, 3), as the tone-mapping pass's: one triangle that covers
// the target, built from the vertex index without a vertex buffer.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index & 1u) * 4u) - 1.0, f32((index & 2u) * 2u) - 1.0);
    return vec4<f32>(corner, 0.0, 1.0);
}

// The source between its texels, `offset` texels from `at` (texture coordinates).
fn tap(at: vec2<f32>, offset: vec2<f32>, texel: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(source, source_sampler, at + offset * texel, 0.0).rgb;
}

// Jimenez's 13-tap filter (Next Generation Post Processing in Call of Duty: Advanced Warfare,
// 2014). The target is half the source's size, rounded down or up, and its pixel (x, y) covers the
// source's texels 2x and 2x + 1 across, 2y and 2y + 1 down: its centre is the corner between them,
// twice its own position, whatever the sizes' parity (normalized coordinates would drift across an
// odd source and skip or double whole texels). Each tap falls on a corner between four texels,
// which the linear sampler averages: five overlapping boxes of 2 x 2 taps, the central one
// weighted 1/2 and the four around it 1/8 each, cover 6 x 6 texels.
@fragment
fn fs_downsample(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    let at = 2.0 * position.xy * texel;
    let a = tap(at, vec2<f32>(-2.0, -2.0), texel);
    let b = tap(at, vec2<f32>(0.0, -2.0), texel);
    let c = tap(at, vec2<f32>(2.0, -2.0), texel);
    let d = tap(at, vec2<f32>(-2.0, 0.0), texel);
    let e = tap(at, vec2<f32>(0.0, 0.0), texel);
    let f = tap(at, vec2<f32>(2.0, 0.0), texel);
    let g = tap(at, vec2<f32>(-2.0, 2.0), texel);
    let h = tap(at, vec2<f32>(0.0, 2.0), texel);
    let i = tap(at, vec2<f32>(2.0, 2.0), texel);
    let j = tap(at, vec2<f32>(-1.0, -1.0), texel);
    let k = tap(at, vec2<f32>(1.0, -1.0), texel);
    let l = tap(at, vec2<f32>(-1.0, 1.0), texel);
    let m = tap(at, vec2<f32>(1.0, 1.0), texel);
    let inner = (j + k + l + m) * 0.25;
    let corners = (a + b + d + e) * 0.25 + (b + c + e + f) * 0.25 + (d + e + g + h) * 0.25
        + (e + f + h + i) * 0.25;
    return vec4<f32>(inner * 0.5 + corners * 0.125, 1.0);
}

// The 3 x 3 tent of one texel of `source` around `at`: the steps that magnifying a coarse level
// by a linear filter alone would leave, smoothed.
fn tent(at: vec2<f32>) -> vec3<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(source));
    let corners = tap(at, vec2<f32>(-1.0, -1.0), texel) + tap(at, vec2<f32>(1.0, -1.0), texel)
        + tap(at, vec2<f32>(-1.0, 1.0), texel) + tap(at, vec2<f32>(1.0, 1.0), texel);
    let sides = tap(at, vec2<f32>(0.0, -1.0), texel) + tap(at, vec2<f32>(-1.0, 0.0), texel)
        + tap(at, vec2<f32>(1.0, 0.0), texel) + tap(at, vec2<f32>(0.0, 1.0), texel);
    return (corners + 2.0 * sides + 4.0 * tap(at, vec2<f32>(0.0, 0.0), texel)) / 16.0;
}

// The average of a level and the coarser ones: the coarser average, twice this level's texels,
// read through the tent at half the pixel's position, mixed with this level's own light by its
// weight.
@fragment
fn fs_upsample(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let coarser = tent(0.5 * position.xy / vec2<f32>(textureDimensions(source)));
    let own = textureLoad(level, vec2<u32>(position.xy), 0).rgb;
    return vec4<f32>(mix(coarser, own, weight.own), 1.0);
}
