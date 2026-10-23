// The canvas (docs/adr/0137-the-canvas.md): boxes with rounded corners and borders, their shadows,
// and images, drawn in two dimensions over a target, each a quad of two triangles built from the
// vertex's index alone, in the order given, every item clipped to its rectangle. Colors come
// multiplied by their alpha, and are laid over the target so: into a target of an sRGB format,
// which the canvas draws into without its encoding, encoded first, so that they mix as sRGB values,
// as design tools and browsers mix them; into another, as they are, linear. `canvas.rs` mixes the
// items' colors so; an image's texels are encoded here.

struct Frame {
    // The target's size, in pixels.
    size: vec2<f32>,
    // 1 if colors are encoded before they mix: the target is sRGB, drawn into without its
    // encoding.
    encode: f32,
    unused: f32,
}

// What `Canvas` in `canvas.rs` writes of each item. A shadow keeps in some fields the values of
// its approximation, which the fragment shader reads.
struct Item {
    // The quad drawn, the pixels that the item may touch: left, top, right, bottom, in pixels of
    // the target.
    quad: vec4<f32>,
    // The box that a box draws, the same edges; for a shadow, the box of the approximation.
    shape: vec4<f32>,
    // The radii of a box's corners: top left, top right, bottom right, bottom left; for a shadow,
    // the approximation's radius at each corner.
    radii: vec4<f32>,
    // The box's fill, or the shadow's or an image's color, multiplied by its alpha, mixed as the
    // target mixes colors.
    color: vec4<f32>,
    // The border's color, the same way; for a shadow, the approximation's exponent at each corner.
    border: vec4<f32>,
    // The image's coordinates at the quad's corners: left, top, right, bottom; for a shadow, the
    // inverse of the exponent at each corner.
    uv: vec4<f32>,
    // The clip: left, top, right, bottom.
    clip: vec4<f32>,
    // The border's width; for a shadow, the box's shorter side, the approximation's scale, and the
    // inverse of its deviation, 0 for a sharp shadow.
    widths: vec4<f32>,
    // What the item is (BOX, SHADOW, COVERAGE or IMAGE), then the indices of its transform and of
    // its layer.
    kind: vec4<u32>,
}

// A transform of the plane: x' = x.x * x + x.y * y + x.z, and y' likewise with y.
struct Transform {
    x: vec4<f32>,
    y: vec4<f32>,
}

const BOX: u32 = 0u;
const SHADOW: u32 = 1u;
// An image of one channel, how much of each texel a shape covers, drawn in the item's color.
const COVERAGE: u32 = 2u;
// An image of colors, not multiplied by their alpha, drawn faded by the item's alpha.
const IMAGE: u32 = 3u;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> items: array<Item>;
@group(0) @binding(2) var<storage, read> transforms: array<Transform>;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

// The fragment shader reads its item again by its index: handing the item on as values of the
// vertices costs more for the many small quads of glyphs (`docs/measurements.md`, M6b.3a).
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    // Where the fragment lies, in the target's pixels, before the transform.
    @location(0) point: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) index: u32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex: u32) -> VertexOutput {
    let index = vertex / 6u;
    let item = items[index];
    // Two triangles: the corners (0, 0), (1, 0), (0, 1), then (0, 1), (1, 0), (1, 1).
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex % 6u];
    let point = mix(item.quad.xy, item.quad.zw, corner);
    let transform = transforms[item.kind.y];
    let placed = vec2<f32>(
        dot(transform.x.xyz, vec3<f32>(point, 1.0)),
        dot(transform.y.xyz, vec3<f32>(point, 1.0)),
    );
    var out: VertexOutput;
    // Pixels from the top left, y down, to clip space, y up.
    let ndc = placed / frame.size * 2.0 - 1.0;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.point = point;
    out.uv = mix(item.uv.xy, item.uv.zw, corner);
    out.index = index;
    return out;
}

// Of `values`, one per corner, top left, top right, bottom right and bottom left, that of the corner
// of the quarter of a box where `q`, from its centre, lies; y down.
fn of_corner(values: vec4<f32>, q: vec2<f32>) -> f32 {
    let top = select(values.x, values.y, q.x > 0.0);
    let bottom = select(values.w, values.z, q.x > 0.0);
    return select(bottom, top, q.y < 0.0);
}

// The signed distance from `p` to the box `edges` with the corners' `radii`, negative inside.
fn rounded_box(p: vec2<f32>, edges: vec4<f32>, radii: vec4<f32>) -> f32 {
    let center = (edges.xy + edges.zw) * 0.5;
    let half_size = (edges.zw - edges.xy) * 0.5;
    let q = p - center;
    let radius = of_corner(radii, q);
    let d = abs(q) - half_size + radius;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - radius;
}

// How much of the pixel centred on a point at `distance` from an edge, negative inside, lies inside.
fn coverage(distance: f32) -> f32 {
    return clamp(0.5 - distance, 0.0, 1.0);
}

// The error function, to about 10⁻³: Raph Levien's approximation, which `erf` in `canvas.rs`
// computes too.
fn erf(x: f32) -> f32 {
    let y = clamp(x, -10.0, 10.0) * 1.1283791671;
    let yy = y * y;
    let z = y + (0.24295 + (0.03395 + 0.0104 * yy) * yy) * (y * yy);
    return z * inverseSqrt(1.0 + z * z);
}

// The sRGB encoding of linear values from 0 to 1, as `tone_mapping.wgsl` computes it.
fn linear_to_srgb(linear: vec3<f32>) -> vec3<f32> {
    let curve = 1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(curve, linear * 12.92, linear <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let item = items[in.index];
    let p = in.point;
    // The share of the pixel inside the clip: its overlap with the clip across, times its overlap
    // down.
    let clip = clamp(
        min(item.clip.zw, p + 0.5) - max(item.clip.xy, p - 0.5),
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    let clipped = clip.x * clip.y;
    var color = vec4<f32>(0.0);
    switch item.kind.x {
        case BOX: {
            let outer = coverage(rounded_box(p, item.shape, item.radii));
            let width = item.widths.x;
            var filled = outer;
            if width > 0.0 {
                let inner_edges = item.shape + vec4<f32>(width, width, -width, -width);
                let inner_radii = max(item.radii - width, vec4<f32>(0.0));
                filled = min(coverage(rounded_box(p, inner_edges, inner_radii)), outer);
            }
            color = item.color * filled + item.border * (outer - filled);
        }
        case SHADOW: {
            // Raph Levien's approximation of a rounded box blurred by a Gaussian, from the values
            // that `Canvas::draw_shadow` made: the distance to the box, its corners measured as
            // superellipses, and across it the Gaussian's integral over the box's shorter side.
            let center = (item.shape.xy + item.shape.zw) * 0.5;
            let half_size = (item.shape.zw - item.shape.xy) * 0.5;
            let q = p - center;
            let radius = of_corner(item.radii, q);
            let d = abs(q) - half_size + radius;
            let beyond = max(d, vec2<f32>(0.0));
            // Along an edge, the distance beyond it; past a corner, the superellipse's.
            var corner = beyond.x + beyond.y;
            if beyond.x > 0.0 && beyond.y > 0.0 {
                let exponent = of_corner(item.border, q);
                corner = pow(pow(beyond.x, exponent) + pow(beyond.y, exponent), of_corner(item.uv, q));
            }
            let distance = corner + min(max(d.x, d.y), 0.0) - radius;
            let inverse = item.widths.z;
            var shade = coverage(distance);
            if inverse > 0.0 {
                shade = item.widths.y * (erf((item.widths.x + distance) * inverse) - erf(distance * inverse));
            }
            color = item.color * shade;
        }
        case COVERAGE: {
            // Level 0, the image's own pixels, as an interface draws them.
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = item.color * texel.r;
        }
        default: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            var rgb = texel.rgb;
            if frame.encode > 0.0 {
                rgb = linear_to_srgb(min(rgb, vec3<f32>(1.0)));
            }
            color = vec4<f32>(rgb * texel.a, texel.a) * item.color.a;
        }
    }
    return color * clipped;
}
