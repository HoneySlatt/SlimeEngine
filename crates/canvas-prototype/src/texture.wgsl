// T: the canvas of `storage.wgsl` (S), its items read from a texture rather than a storage buffer,
// which WebGL2 lacks. Each item is the same nine `vec4`s as S's, written as texels of an
// `Rgba32Uint` texture 2048 texels wide, item after item, the floats' bits as they are: the item
// `i`'s field `f` is the texel `9 i + f`, counted row after row. The vertex shader loads the quad,
// the image's coordinates and the transform's index; the fragment shader loads the kind and the
// clip, then what its kind reads. Transforms are a uniform array. The drawing is S's, line for line.

struct Frame {
    size: vec2<f32>,
    encode: f32,
    unused: f32,
}

struct Transform {
    x: vec4<f32>,
    y: vec4<f32>,
}

const BOX: u32 = 0u;
const SHADOW: u32 = 1u;
const COVERAGE: u32 = 2u;
const IMAGE: u32 = 3u;
const GRADIENT: u32 = 4u;

// The fields of an item, in S's order.
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
// The texture's width, `texture.rs`'s `WIDTH`.
const ROW: u32 = 2048u;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<uniform> transforms: array<Transform, 512>;
@group(0) @binding(2) var items: texture_2d<u32>;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

fn texel(index: u32, field: u32) -> vec4<u32> {
    let at = index * ITEM_TEXELS + field;
    return textureLoad(items, vec2<i32>(i32(at % ROW), i32(at / ROW)), 0);
}

fn floats(index: u32, field: u32) -> vec4<f32> {
    return bitcast<vec4<f32>>(texel(index, field));
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) index: u32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32) -> VertexOutput {
    let index = vertex / 6u;
    let quad = floats(index, QUAD);
    let uv = floats(index, UV);
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex % 6u];
    let point = mix(quad.xy, quad.zw, corner);
    let transform = transforms[texel(index, KIND).y];
    let placed = vec2<f32>(
        dot(transform.x.xyz, vec3<f32>(point, 1.0)),
        dot(transform.y.xyz, vec3<f32>(point, 1.0)),
    );
    var out: VertexOutput;
    let ndc = placed / frame.size * 2.0 - 1.0;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.point = point;
    out.uv = mix(uv.xy, uv.zw, corner);
    out.index = index;
    return out;
}

fn of_corner(values: vec4<f32>, q: vec2<f32>) -> f32 {
    let top = select(values.x, values.y, q.x > 0.0);
    let bottom = select(values.w, values.z, q.x > 0.0);
    return select(bottom, top, q.y < 0.0);
}

fn rounded_box(p: vec2<f32>, edges: vec4<f32>, radii: vec4<f32>) -> f32 {
    let center = (edges.xy + edges.zw) * 0.5;
    let half_size = (edges.zw - edges.xy) * 0.5;
    let q = p - center;
    let radius = of_corner(radii, q);
    let d = abs(q) - half_size + radius;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

fn coverage(distance: f32) -> f32 {
    return clamp(0.5 - distance, 0.0, 1.0);
}

fn erf(x: f32) -> f32 {
    let y = clamp(x, -10.0, 10.0) * 1.1283791671;
    let yy = y * y;
    let z = y + (0.24295 + (0.03395 + 0.0104 * yy) * yy) * (y * yy);
    return z * inverseSqrt(1.0 + z * z);
}

fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let curve = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(curve, linear * 12.92, linear <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let index = in.index;
    let kind = texel(index, KIND).x;
    let item_clip = floats(index, CLIP);
    let p = in.point;
    let clip = clamp(
        min(item_clip.zw, p + 0.5) - max(item_clip.xy, p - 0.5),
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    let clipped = clip.x * clip.y;
    var color = vec4<f32>(0.0);
    switch kind {
        case BOX: {
            let shape = floats(index, SHAPE);
            let radii = floats(index, RADII);
            let outer = coverage(rounded_box(p, shape, radii));
            let width = floats(index, WIDTHS).x;
            var filled = outer;
            if width > 0.0 {
                let inner_edges = shape + vec4<f32>(width, width, -width, -width);
                let inner_radii = max(radii - width, vec4<f32>(0.0));
                filled = min(coverage(rounded_box(p, inner_edges, inner_radii)), outer);
            }
            color = floats(index, COLOR) * filled + floats(index, BORDER) * (outer - filled);
        }
        case SHADOW: {
            let shape = floats(index, SHAPE);
            let center = (shape.xy + shape.zw) * 0.5;
            let half_size = (shape.zw - shape.xy) * 0.5;
            let q = p - center;
            let radius = of_corner(floats(index, RADII), q);
            let d = abs(q) - half_size + radius;
            let beyond = max(d, vec2<f32>(0.0));
            var corner = beyond.x + beyond.y;
            if beyond.x > 0.0 && beyond.y > 0.0 {
                let exponent = of_corner(floats(index, BORDER), q);
                corner = pow(pow(beyond.x, exponent) + pow(beyond.y, exponent), of_corner(floats(index, UV), q));
            }
            let distance = corner + min(max(d.x, d.y), 0.0) - radius;
            let widths = floats(index, WIDTHS);
            let inverse = widths.z;
            var shade = coverage(distance);
            if inverse > 0.0 {
                shade = widths.y * (erf((widths.x + distance) * inverse) - erf(distance * inverse));
            }
            color = floats(index, COLOR) * shade;
        }
        case GRADIENT: {
            let shape = floats(index, SHAPE);
            let outer = coverage(rounded_box(p, shape, floats(index, RADII)));
            let size = max(shape.zw - shape.xy, vec2<f32>(1e-6));
            let t = clamp((p - shape.xy) / size, vec2<f32>(0.0), vec2<f32>(1.0));
            let top = mix(floats(index, COLOR), floats(index, BORDER), t.x);
            let bottom = mix(floats(index, WIDTHS), floats(index, UV), t.x);
            color = mix(top, bottom, t.y) * outer;
        }
        case COVERAGE: {
            let sampled = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = floats(index, COLOR) * sampled.r;
        }
        default: {
            let sampled = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            var rgb = sampled.rgb;
            if frame.encode > 0.0 {
                rgb = linear_to_srgb(min(rgb, vec3<f32>(1.0)));
            }
            color = vec4<f32>(rgb * sampled.a, sampled.a) * floats(index, COLOR).a;
        }
    }
    return color * clipped;
}
