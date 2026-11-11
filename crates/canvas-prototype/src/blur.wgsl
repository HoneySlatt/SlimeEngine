// The passes of a blur of what lies behind, and the copy of the canvas's texture onto the target;
// `blur.rs` plans them. Each pass draws one triangle that covers its target. The values are those
// the canvas stored, encoded and multiplied by their alpha: they are averaged as stored.

struct Pass {
    // The source's texel that the output's pixel (0, 0) reads, at its centre for a blur and at the
    // corner of four texels for a downsampling.
    origin: vec2<f32>,
    // The source's texels an output's pixel spans: 2 for a downsampling, 1 for a blur.
    step: f32,
    // How many of `taps` a blur reads, the first at its centre.
    count: u32,
    // 1 over the source's size, in texels.
    inverse_size: vec2<f32>,
    // A blur's axis, in texels: (1, 0) or (0, 1).
    direction: vec2<f32>,
    // A blur's taps: their offset from the centre, in texels, and their weight. Each tap but the
    // first is read on both sides, between two texels, so that filtering weighs both.
    taps: array<vec4<f32>, 8>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(0) @binding(2) var<uniform> params: Pass;

@vertex
fn vs_cover(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    let x = f32(vertex & 1u) * 4.0 - 1.0;
    let y = f32(vertex >> 1u) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

// Half the size: each pixel the average of four texels, read as one sample at their shared corner.
@fragment
fn fs_down(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let at = params.origin + position.xy * params.step;
    return textureSampleLevel(source, source_sampler, at * params.inverse_size, 0.0);
}

// One axis of a Gaussian.
@fragment
fn fs_blur(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let at = (params.origin + position.xy) * params.inverse_size;
    var sum = textureSampleLevel(source, source_sampler, at, 0.0) * params.taps[0].y;
    for (var k = 1u; k < params.count; k++) {
        let tap = params.taps[k];
        let offset = params.direction * tap.x * params.inverse_size;
        sum += (textureSampleLevel(source, source_sampler, at + offset, 0.0) + textureSampleLevel(source, source_sampler, at - offset, 0.0)) * tap.y;
    }
    return sum;
}

// The canvas's texture onto the target, texel for pixel.
@fragment
fn fs_copy(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(source, vec2<i32>(position.xy), 0);
}
