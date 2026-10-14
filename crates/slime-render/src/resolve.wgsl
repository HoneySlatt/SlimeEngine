// The resolve pass (ADR-0060): one triangle covers the scene's target of one sample per pixel, and
// each pixel averages the samples of the scene's multisampled target under it, each weighed by
// 1 / (1 + its brightest channel, exposed). A plain average of a bright sample and a dark one is
// still bright, which tone mapping shows near white, so that an edge between a lamp and the night
// would keep its steps; weighed so, each sample counts about as much as it shows once tone mapped,
// and the edge shows the share of the pixel that each side covers.

// The scene's exposed light divided by HEADROOM, as the scene and sky passes drew it, sample by
// sample.
@group(0) @binding(0) var samples: texture_multisampled_2d<f32>;

// What the scene and sky passes divided exposed light by (ADR-0040): HEADROOM in shader.wgsl, which
// a unit test compares with it.
const HEADROOM: f32 = 64.0;

// The corners (-1, -1), (3, -1) and (-1, 3), counter-clockwise: one triangle that covers the
// target, built from the vertex index without a vertex buffer.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index & 1u) * 4u) - 1.0, f32((index & 2u) * 2u) - 1.0);
    return vec4<f32>(corner, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    // The target has the multisampled one's size: a pixel's center truncates to its texel.
    let texel = vec2<u32>(position.xy);
    var light = vec3<f32>(0.0);
    var weights = 0.0;
    for (var sample = 0u; sample < textureNumSamples(samples); sample++) {
        let sampled = textureLoad(samples, texel, sample).rgb;
        let weight = 1.0 / (1.0 + max(sampled.r, max(sampled.g, sampled.b)) * HEADROOM);
        light += sampled * weight;
        weights += weight;
    }
    return vec4<f32>(light / weights, 1.0);
}
