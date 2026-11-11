// T with the effects (`fx_common.wgsl` comes first): one `Rgba32Uint` texture 2048 texels wide
// holds all the frame's variable data, the floats' bits as they are: the items, nine texels each
// as S lays them out; from `frame.transforms_at`, the transforms, two texels each; from
// `frame.segments_at`, the paths' segments, one texel each.

const QUAD: u32 = 0u;
const SHAPE: u32 = 1u;
const RADII: u32 = 2u;
const COLOR: u32 = 3u;
const BORDER: u32 = 4u;
const UV: u32 = 5u;
const CLIP: u32 = 6u;
const WIDTHS: u32 = 7u;
const KIND: u32 = 8u;
const ITEM_TEXELS: u32 = 9u;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var data: texture_2d<u32>;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

fn load(at: u32) -> vec4<u32> {
    return textureLoad(data, vec2<i32>(i32(at % ROW), i32(at / ROW)), 0);
}

fn floats(index: u32, field: u32) -> vec4<f32> {
    return bitcast<vec4<f32>>(load(index * ITEM_TEXELS + field));
}

fn segment(index: u32) -> vec4<f32> {
    return bitcast<vec4<f32>>(load(frame.segments_at + index));
}

fn transform(index: u32) -> Transform {
    let at = frame.transforms_at + 2u * index;
    return Transform(bitcast<vec4<f32>>(load(at)), bitcast<vec4<f32>>(load(at + 1u)));
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) index: u32,
    @location(3) @interpolate(flat) scale: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32) -> VertexOutput {
    let index = vertex / 6u;
    let item_quad = floats(index, QUAD);
    let uv = floats(index, UV);
    let kind = load(index * ITEM_TEXELS + KIND);
    let t = transform(kind.y);
    let scale = scale_of(t);
    let corner = corner_of(vertex);
    let quad = grown(item_quad, scale);
    let point = mix(quad.xy, quad.zw, corner);
    var out: VertexOutput;
    out.position = to_clip(placed(t, point), frame.size);
    out.point = point;
    out.uv = select(image_uv(point, item_quad, uv, corner, scale), glass_uv(point, uv), kind.x == GLASS);
    out.index = index;
    out.scale = scale;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let index = in.index;
    let kind = load(index * ITEM_TEXELS + KIND);
    let p = in.point;
    let s = distance_scale(in.scale);
    let clipped = clip_share(in.position.xy, floats(index, CLIP));
    var color = vec4<f32>(0.0);
    switch kind.x {
        case BOX: {
            color = box_color(p, floats(index, SHAPE), floats(index, RADII), floats(index, COLOR), floats(index, BORDER), floats(index, WIDTHS).x, s);
        }
        case GLASS: {
            let behind = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            let fill = glass_fill(floats(index, COLOR), behind);
            color = box_color(p, floats(index, SHAPE), floats(index, RADII), fill, floats(index, BORDER), floats(index, WIDTHS).x, s);
        }
        case SHADOW: {
            color = shadow_color(p, floats(index, SHAPE), floats(index, RADII), floats(index, BORDER), floats(index, UV), floats(index, WIDTHS).xyz, floats(index, COLOR), s);
        }
        case GRADIENT: {
            color = gradient_color(p, floats(index, SHAPE), floats(index, RADII), floats(index, COLOR), floats(index, BORDER), floats(index, UV), floats(index, WIDTHS), s);
        }
        case COVERAGE: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = floats(index, COLOR) * texel.r;
        }
        case IMAGE: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = image_color(texel, floats(index, COLOR).a, frame.encode);
        }
        case LAYER: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = texel * floats(index, COLOR).a;
        }
        case PATH: {
            color = floats(index, COLOR) * path_coverage(p, kind.z, kind.w, s);
        }
        default: {}
    }
    // A transformed image's edges, read only then: the identity's keep the target's pixels.
    if in.scale > 0.0 && (kind.x == COVERAGE || kind.x == IMAGE || kind.x == LAYER) {
        color *= image_edges(p, floats(index, QUAD), in.scale);
    }
    return color * clipped;
}
