// The outline (ADR-0159): over the view's rectangle of its output, after tone mapping, one triangle
// covers the rectangle, and each pixel that the mask does not cover, less than the outline's width
// and a pixel from the centre of one that it covers, is laid over by the outline's color where the
// scene shows what it outlines nearby, and by its hidden part's color where it shows none of it,
// fading over the last pixel: an outline of a width of 2 covers the two pixels beside what it
// outlines along a row or a column whole.
// The mask, at the size of the view's targets, holds in its green channel whether each texel is
// covered, and in its red whether the scene shows what covers it.
@group(0) @binding(0) var mask: texture_2d<f32>;
struct Settings {
    // The color of the outline where the scene shows what it outlines, and where it hides it,
    // with their alphas: linear, written as they are, in the output's colors.
    seen: vec4<f32>,
    hidden: vec4<f32>,
    // The pixel of the output where the view's rectangle starts, its top left corner.
    origin: vec2<f32>,
    // How many of the mask's texels make a pixel of the rectangle, along each side, as in the
    // tone-mapping pass.
    scale: vec2<f32>,
    // The outline's width, in the output's pixels, from 0.5 to REACH.
    width: f32,
}
@group(0) @binding(1) var<uniform> settings: Settings;

// The widest outline, in pixels: OUTLINE_REACH in drawings.rs, which a unit test compares with it.
const REACH: i32 = 8;

// The corners (-1, -1), (3, -1) and (-1, 3), counter-clockwise: one triangle that covers the
// rectangle, built from the vertex index without a vertex buffer.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index & 1u) * 4u) - 1.0, f32((index & 2u) * 2u) - 1.0);
    return vec4<f32>(corner, 0.0, 1.0);
}

// The mask's texel under the point `at` of the rectangle, in its pixels, kept within the mask.
fn mask_at(at: vec2<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(mask));
    let texel = clamp(vec2<i32>(floor(at * settings.scale)), vec2<i32>(0), size - 1);
    return textureLoad(mask, texel, 0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let at = position.xy - settings.origin;
    // Inside what it outlines, nothing is drawn.
    if mask_at(at).g > 0.5 {
        discard;
    }
    let reach = min(i32(ceil(settings.width)), REACH);
    var nearest = f32(REACH) + 1.0;
    var seen = 0.0;
    for (var y = -reach; y <= reach; y++) {
        for (var x = -reach; x <= reach; x++) {
            let offset = vec2<f32>(f32(x), f32(y));
            let distance = length(offset);
            if distance >= settings.width + 1.0 {
                continue;
            }
            let texel = mask_at(at + offset);
            if texel.g > 0.5 {
                nearest = min(nearest, distance);
                seen = max(seen, texel.r);
            }
        }
    }
    let coverage = clamp(settings.width + 1.0 - nearest, 0.0, 1.0);
    if coverage <= 0.0 {
        discard;
    }
    let color = mix(settings.hidden, settings.seen, seen);
    return vec4<f32>(color.rgb, color.a * coverage);
}
