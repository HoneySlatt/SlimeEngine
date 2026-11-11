// What the effects' shaders of S, I and T share (`storage_fx.wgsl`, `instances_fx.wgsl`,
// `texture_fx.wgsl`): Rust puts this file before each. The drawing is that of `storage.wgsl`, with:
//
// - a transform per item: the shape's signed distance is computed in the item's own space, then
//   multiplied by the transform's scale, so that the band where an edge covers a pixel in part
//   stays one pixel of the target wide; the clip stays in the target's pixels and is tested against
//   the fragment's position. The identity draws exactly as `storage.wgsl` does: no quad grown, no
//   image's edge smoothed.
// - three kinds: GLASS, a box filled with its tint over the blurred region behind it; LAYER, a group
//   drawn apart, its texels already multiplied by their alpha and encoded as the target is; PATH,
//   filled by the non-zero rule from its segments, which each layout keeps in its own place and
//   reads through `segment`, which each file defines.

struct Frame {
    // The target's size, in pixels.
    size: vec2<f32>,
    // 1 if colors are encoded before they mix.
    encode: f32,
    // T: the first texel of the transforms in the items' texture.
    transforms_at: u32,
    // T: the first texel of the segments in the items' texture.
    segments_at: u32,
    unused0: u32,
    unused1: u32,
    unused2: u32,
}

// x' = x.x * x + x.y * y + x.z, and y' likewise with y; `.w` unused.
struct Transform {
    x: vec4<f32>,
    y: vec4<f32>,
}

const BOX: u32 = 0u;
const SHADOW: u32 = 1u;
const COVERAGE: u32 = 2u;
const IMAGE: u32 = 3u;
const GRADIENT: u32 = 4u;
const PATH: u32 = 5u;
const GLASS: u32 = 6u;
const LAYER: u32 = 7u;

// The width of the textures that hold variable data, in texels: WebGL2's largest.
const ROW: u32 = 2048u;

fn corner_of(vertex: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    return corners[vertex % 6u];
}

// The pixels of the target that one unit of the item's space spans: the square root of the
// determinant of the transform's linear part, computed by the vertex shader. 0 for the identity,
// which draws as an item without a transform.
fn scale_of(t: Transform) -> f32 {
    let identity = all(t.x.xyz == vec3<f32>(1.0, 0.0, 0.0)) && all(t.y.xyz == vec3<f32>(0.0, 1.0, 0.0));
    return select(sqrt(abs(t.x.x * t.y.y - t.x.y * t.y.x)), 0.0, identity);
}

// The scale that distances are multiplied by: 1 for the identity.
fn distance_scale(scale: f32) -> f32 {
    return select(scale, 1.0, scale == 0.0);
}

// The quad drawn: grown by one pixel of the target for a transformed item, so that the band of an
// edge outside the shape is drawn whatever the rotation.
fn grown(quad: vec4<f32>, scale: f32) -> vec4<f32> {
    let grow = select(0.0, 1.0 / scale, scale > 0.0);
    return quad + vec4<f32>(-grow, -grow, grow, grow);
}

fn placed(t: Transform, point: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(dot(t.x.xyz, vec3<f32>(point, 1.0)), dot(t.y.xyz, vec3<f32>(point, 1.0)));
}

// A point in the target's pixels, from the top left, y down, in clip space, y up.
fn to_clip(point: vec2<f32>, size: vec2<f32>) -> vec4<f32> {
    let ndc = point / size * 2.0 - 1.0;
    return vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
}

// An image's coordinates at `point` of its grown quad, from those at the quad's corners `uv`; at
// `corner` itself for the identity, as `storage.wgsl` computes them.
fn image_uv(point: vec2<f32>, quad: vec4<f32>, uv: vec4<f32>, corner: vec2<f32>, scale: f32) -> vec2<f32> {
    let along = select(corner, (point - quad.xy) / max(quad.zw - quad.xy, vec2<f32>(1e-6)), scale > 0.0);
    return mix(uv.xy, uv.zw, along);
}

// A glass's coordinates in the image of what lies behind it.
fn glass_uv(point: vec2<f32>, map: vec4<f32>) -> vec2<f32> {
    return (point - map.xy) * map.zw;
}

// The quad of a shape `shape` clipped to `clip`, as `Canvas::push` cuts it on the CPU.
fn cut_quad(shape: vec4<f32>, clip: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(max(floor(shape.xy), floor(clip.xy)), min(ceil(shape.zw), ceil(clip.zw)));
}

fn outward(edges: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(floor(edges.xy), ceil(edges.zw));
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

// The share of the pixel centred on `p`, in the target's pixels, inside `clip`.
fn clip_share(p: vec2<f32>, clip: vec4<f32>) -> f32 {
    let share = clamp(min(clip.zw, p + 0.5) - max(clip.xy, p - 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    return share.x * share.y;
}

// How much of the pixel at `p` an image's quad covers: 1 for the identity, whose images keep the
// target's pixels; for a transformed image, its edges smoothed as a box's.
fn image_edges(p: vec2<f32>, quad: vec4<f32>, scale: f32) -> f32 {
    if scale == 0.0 {
        return 1.0;
    }
    return coverage(rounded_box(p, quad, vec4<f32>(0.0)) * scale);
}

// A box, or a glass whose `fill` is its tint over what lies behind it.
fn box_color(p: vec2<f32>, shape: vec4<f32>, radii: vec4<f32>, fill: vec4<f32>, border: vec4<f32>, width: f32, s: f32) -> vec4<f32> {
    let outer = coverage(rounded_box(p, shape, radii) * s);
    var filled = outer;
    if width > 0.0 {
        let inner_edges = shape + vec4<f32>(width, width, -width, -width);
        let inner_radii = max(radii - width, vec4<f32>(0.0));
        filled = min(coverage(rounded_box(p, inner_edges, inner_radii) * s), outer);
    }
    return fill * filled + border * (outer - filled);
}

// A tint laid over what lies behind a glass, both multiplied by their alpha and encoded.
fn glass_fill(tint: vec4<f32>, behind: vec4<f32>) -> vec4<f32> {
    return tint + behind * (1.0 - tint.a);
}

// Raph Levien's approximation, as `storage.wgsl` draws it; a sharp shadow's edge scaled as a box's.
fn shadow_color(p: vec2<f32>, shape: vec4<f32>, radii: vec4<f32>, exponent: vec4<f32>, inverse_exponent: vec4<f32>, widths: vec3<f32>, color: vec4<f32>, s: f32) -> vec4<f32> {
    let center = (shape.xy + shape.zw) * 0.5;
    let half_size = (shape.zw - shape.xy) * 0.5;
    let q = p - center;
    let radius = of_corner(radii, q);
    let d = abs(q) - half_size + radius;
    let beyond = max(d, vec2<f32>(0.0));
    var corner = beyond.x + beyond.y;
    if beyond.x > 0.0 && beyond.y > 0.0 {
        let e = of_corner(exponent, q);
        corner = pow(pow(beyond.x, e) + pow(beyond.y, e), of_corner(inverse_exponent, q));
    }
    let distance = corner + min(max(d.x, d.y), 0.0) - radius;
    let inverse = widths.z;
    var shade = coverage(distance * s);
    if inverse > 0.0 {
        shade = widths.y * (erf((widths.x + distance) * inverse) - erf(distance * inverse));
    }
    return color * shade;
}

fn gradient_color(p: vec2<f32>, shape: vec4<f32>, radii: vec4<f32>, top_left: vec4<f32>, top_right: vec4<f32>, bottom_right: vec4<f32>, bottom_left: vec4<f32>, s: f32) -> vec4<f32> {
    let outer = coverage(rounded_box(p, shape, radii) * s);
    let size = max(shape.zw - shape.xy, vec2<f32>(1e-6));
    let t = clamp((p - shape.xy) / size, vec2<f32>(0.0), vec2<f32>(1.0));
    let top = mix(top_left, top_right, t.x);
    let bottom = mix(bottom_left, bottom_right, t.x);
    return mix(top, bottom, t.y) * outer;
}

// An image of colours, not multiplied by their alpha, faded by `alpha`.
fn image_color(texel: vec4<f32>, alpha: f32, encode: f32) -> vec4<f32> {
    var rgb = texel.rgb;
    if encode > 0.0 {
        rgb = linear_to_srgb(min(rgb, vec3<f32>(1.0)));
    }
    return vec4<f32>(rgb * texel.a, texel.a) * alpha;
}

// How much of the pixel at `p` the path covers: the non-zero winding number of its `count`
// segments from `first` says whether `p` lies inside, the distance to the nearest says how far, and
// the coverage spans one pixel of the target across the edge.
fn path_coverage(p: vec2<f32>, first: u32, count: u32, s: f32) -> f32 {
    var winding = 0i;
    var nearest = 3.0e38;
    for (var i = 0u; i < count; i++) {
        let line = segment(first + i);
        let a = line.xy;
        let ab = line.zw - a;
        let ap = p - a;
        let along = clamp(dot(ap, ab) / max(dot(ab, ab), 1e-12), 0.0, 1.0);
        nearest = min(nearest, length(ap - ab * along));
        // Which side of the segment `p` lies on; a crossing counts by its direction.
        let side = ab.x * ap.y - ab.y * ap.x;
        if a.y <= p.y {
            if line.w > p.y && side > 0.0 {
                winding += 1;
            }
        } else if line.w <= p.y && side < 0.0 {
            winding -= 1;
        }
    }
    let distance = select(nearest, -nearest, winding != 0);
    return coverage(distance * s);
}
