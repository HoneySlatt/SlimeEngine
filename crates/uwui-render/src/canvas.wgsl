// The canvas (docs/adr/0137-the-canvas.md): boxes with rounded corners and borders, their shadows,
// gradients and images, drawn in two dimensions over a target, each a quad of two triangles built from the
// vertex's index alone, in the order given, every item clipped to its rectangle. Colors come
// multiplied by their alpha, and are laid over the target so: into a target of an sRGB format,
// which the canvas draws into without its encoding, or of 8 bits a channel without one, encoded
// first, so that they mix as sRGB values, as design tools and browsers mix them; into one of
// floats, as they are, linear. `canvas.rs` mixes the
// items' colors so; an image's texels are encoded here.

struct Frame {
    // The target's size, in pixels.
    size: vec2<f32>,
    // 1 if colors are encoded before they mix: the target is sRGB, drawn into without its
    // encoding, or of 8 bits a channel without one.
    encode: f32,
    // Where the transforms start in the store, in words: past the items.
    transforms: u32,
}

// The store: what `Canvas` in `canvas.rs` writes, as words of four 32-bit values, floats as their
// bits. Each item is nine words, the item `i`'s field `f` the word `9 i + f`; the transforms follow
// the items, two words each. `word(at)`, the one function that reads it, is appended to this source
// by `canvas.rs`, as the device allows: from a storage buffer (`read_buffer.wgsl`) or from a
// texture (`read_texture.wgsl`), as `docs/adr/0243-the-canvas-reads-its-items-by-index-from-a-buffer-or-a-texture.md`
// decides. Each stage reads the fields that the item's kind needs, and no other.
//
// An item's fields: a shadow keeps in some of them the values of its approximation, which the
// fragment shader reads; a gradient keeps its corners' colors, mixed as the target mixes colors,
// in COLOR, BORDER, UV and WIDTHS: top left, top right, bottom right, bottom left.
//
// The quad drawn, the pixels that the item may touch: left, top, right, bottom, in pixels of the
// target.
const QUAD: u32 = 0u;
// The box that a box draws, the same edges; for a shadow, the box of the approximation.
const SHAPE: u32 = 1u;
// The radii of a box's corners: top left, top right, bottom right, bottom left; for a shadow, the
// approximation's radius at each corner.
const RADII: u32 = 2u;
// The box's fill, or the shadow's or an image's color, multiplied by its alpha, mixed as the target
// mixes colors.
const COLOR: u32 = 3u;
// The border's color, the same way; for a shadow, the approximation's exponent at each corner.
const BORDER: u32 = 4u;
// The image's coordinates at the quad's corners: left, top, right, bottom; for a shadow, the
// inverse of the exponent at each corner.
const UV: u32 = 5u;
// The clip: left, top, right, bottom.
const CLIP: u32 = 6u;
// The border's width; for a shadow, the box's shorter side, the approximation's scale, and the
// inverse of its deviation, 0 for a sharp shadow.
const WIDTHS: u32 = 7u;
// What the item is (BOX, SHADOW, COVERAGE, IMAGE or GRADIENT), then the index of its transform
// and that of its layer, as unsigned integers.
const KIND: u32 = 8u;
const ITEM_WORDS: u32 = 9u;

const BOX: u32 = 0u;
const SHADOW: u32 = 1u;
// An image of one channel, how much of each texel a shape covers, drawn in the item's color.
const COVERAGE: u32 = 2u;
// An image of colors, not multiplied by their alpha, drawn faded by the item's alpha.
const IMAGE: u32 = 3u;
// A box filled with its corners' colors, mixed across it by each point's place.
const GRADIENT: u32 = 4u;

@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

// The field `field` of the item `index`, as four floats.
fn field(index: u32, field: u32) -> vec4<f32> {
    return bitcast<vec4<f32>>(word(index * ITEM_WORDS + field));
}

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
    let quad = field(index, QUAD);
    let kind = word(index * ITEM_WORDS + KIND);
    // Two triangles: the corners (0, 0), (1, 0), (0, 1), then (0, 1), (1, 0), (1, 1).
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex % 6u];
    let point = mix(quad.xy, quad.zw, corner);
    let at = frame.transforms + kind.y * 2u;
    let transform_x = bitcast<vec4<f32>>(word(at));
    let transform_y = bitcast<vec4<f32>>(word(at + 1u));
    let placed = vec2<f32>(
        dot(transform_x.xyz, vec3<f32>(point, 1.0)),
        dot(transform_y.xyz, vec3<f32>(point, 1.0)),
    );
    var out: VertexOutput;
    // Pixels from the top left, y down, to clip space, y up.
    let ndc = placed / frame.size * 2.0 - 1.0;
    out.position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    out.point = point;
    out.uv = vec2<f32>(0.0);
    if kind.x == COVERAGE || kind.x == IMAGE {
        let uv = field(index, UV);
        out.uv = mix(uv.xy, uv.zw, corner);
    }
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
    let index = in.index;
    let item_clip = field(index, CLIP);
    let p = in.point;
    // The share of the pixel inside the clip: its overlap with the clip across, times its overlap
    // down.
    let clip = clamp(
        min(item_clip.zw, p + 0.5) - max(item_clip.xy, p - 0.5),
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    let clipped = clip.x * clip.y;
    var color = vec4<f32>(0.0);
    switch word(index * ITEM_WORDS + KIND).x {
        case BOX: {
            let shape = field(index, SHAPE);
            let radii = field(index, RADII);
            let outer = coverage(rounded_box(p, shape, radii));
            let width = field(index, WIDTHS).x;
            var filled = outer;
            if width > 0.0 {
                let inner_edges = shape + vec4<f32>(width, width, -width, -width);
                let inner_radii = max(radii - width, vec4<f32>(0.0));
                filled = min(coverage(rounded_box(p, inner_edges, inner_radii)), outer);
            }
            color = field(index, COLOR) * filled + field(index, BORDER) * (outer - filled);
        }
        case SHADOW: {
            // Raph Levien's approximation of a rounded box blurred by a Gaussian, from the values
            // that `Canvas::draw_shadow` made: the distance to the box, its corners measured as
            // superellipses, and across it the Gaussian's integral over the box's shorter side.
            let shape = field(index, SHAPE);
            let center = (shape.xy + shape.zw) * 0.5;
            let half_size = (shape.zw - shape.xy) * 0.5;
            let q = p - center;
            let radius = of_corner(field(index, RADII), q);
            let d = abs(q) - half_size + radius;
            let beyond = max(d, vec2<f32>(0.0));
            // Along an edge, the distance beyond it; past a corner, the superellipse's.
            var corner = beyond.x + beyond.y;
            if beyond.x > 0.0 && beyond.y > 0.0 {
                let exponent = of_corner(field(index, BORDER), q);
                corner = pow(pow(beyond.x, exponent) + pow(beyond.y, exponent), of_corner(field(index, UV), q));
            }
            let distance = corner + min(max(d.x, d.y), 0.0) - radius;
            let widths = field(index, WIDTHS);
            let inverse = widths.z;
            var shade = coverage(distance);
            if inverse > 0.0 {
                shade = widths.y * (erf((widths.x + distance) * inverse) - erf(distance * inverse));
            }
            color = field(index, COLOR) * shade;
        }
        case GRADIENT: {
            let shape = field(index, SHAPE);
            let outer = coverage(rounded_box(p, shape, field(index, RADII)));
            let size = max(shape.zw - shape.xy, vec2<f32>(1e-6));
            let t = clamp((p - shape.xy) / size, vec2<f32>(0.0), vec2<f32>(1.0));
            let top = mix(field(index, COLOR), field(index, BORDER), t.x);
            let bottom = mix(field(index, WIDTHS), field(index, UV), t.x);
            color = mix(top, bottom, t.y) * outer;
        }
        case COVERAGE: {
            // Level 0, the image's own pixels, as an interface draws them.
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            color = field(index, COLOR) * texel.r;
        }
        default: {
            let texel = textureSampleLevel(image, image_sampler, in.uv, 0.0);
            var rgb = texel.rgb;
            if frame.encode > 0.0 {
                rgb = linear_to_srgb(min(rgb, vec3<f32>(1.0)));
            }
            color = vec4<f32>(rgb * texel.a, texel.a) * field(index, COLOR).a;
        }
    }
    return color * clipped;
}
