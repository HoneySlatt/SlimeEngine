// S with the effects (`fx_common.wgsl` comes first): the items of `storage.wgsl` in their storage
// buffer, each naming its transform in the transforms' storage buffer, the paths' segments in a
// third. The vertex shader computes the transform's scale and hands it to the fragment shader.

struct Item {
    quad: vec4<f32>,
    shape: vec4<f32>,
    radii: vec4<f32>,
    color: vec4<f32>,
    border: vec4<f32>,
    uv: vec4<f32>,
    clip: vec4<f32>,
    widths: vec4<f32>,
    // The kind, the transform's index, and for a path its first segment and their count.
    kind: vec4<u32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> items: array<Item>;
@group(0) @binding(2) var<storage, read> transforms: array<Transform>;
@group(0) @binding(3) var<storage, read> segments: array<vec4<f32>>;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

fn segment(index: u32) -> vec4<f32> {
    return segments[index];
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // Where the fragment lies in the item's own space.
    @location(0) point: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) index: u32,
    @location(3) @interpolate(flat) scale: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32) -> VertexOutput {
    let index = vertex / 6u;
    let item = items[index];
    let transform = transforms[item.kind.y];
    let scale = scale_of(transform);
    let corner = corner_of(vertex);
    let quad = grown(item.quad, scale);
    let point = mix(quad.xy, quad.zw, corner);
    var out: VertexOutput;
    out.position = to_clip(placed(transform, point), frame.size);
    out.point = point;
    out.uv = select(image_uv(point, item.quad, item.uv, corner, scale), glass_uv(point, item.uv), item.kind.x == GLASS);
    out.index = index;
    out.scale = scale;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let item = items[in.index];
    let p = in.point;
    let s = distance_scale(in.scale);
    let clipped = clip_share(in.position.xy, item.clip);
    var color = vec4<f32>(0.0);
    switch item.kind.x {
        case BOX: {
            color = box_color(p, item.shape, item.radii, item.color, item.border, item.widths.x, s);
        }
        case GLASS: {
            let behind = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = box_color(p, item.shape, item.radii, glass_fill(item.color, behind), item.border, item.widths.x, s);
        }
        case SHADOW: {
            color = shadow_color(p, item.shape, item.radii, item.border, item.uv, item.widths.xyz, item.color, s);
        }
        case GRADIENT: {
            color = gradient_color(p, item.shape, item.radii, item.color, item.border, item.uv, item.widths, s);
        }
        case COVERAGE: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = item.color * texel.r * image_edges(p, item.quad, in.scale);
        }
        case IMAGE: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = image_color(texel, item.color.a, frame.encode) * image_edges(p, item.quad, in.scale);
        }
        case LAYER: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = texel * item.color.a * image_edges(p, item.quad, in.scale);
        }
        case PATH: {
            color = item.color * path_coverage(p, item.kind.z, item.kind.w, s);
        }
        default: {}
    }
    return color * clipped;
}
