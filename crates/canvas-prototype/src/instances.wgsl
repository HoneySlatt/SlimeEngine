// I: the canvas of `storage.wgsl` (S), each kind of item drawn by its own pipeline from its own
// instances in a vertex buffer, six vertices an instance, the quad's corner from the vertex's index.
// The vertex shader hands the fragment shader what its kind reads as flat values between the
// stages; nothing is read again by an index. The drawing is S's, line for line; what changes is
// where its values come from. `instances.rs` writes the instances, and each kind's format is there.
//
// Colours come multiplied by their alpha and sRGB-encoded on the CPU, as S mixes them, rounded to
// 16 bits for boxes, shadows and gradients and to 8 bits for glyphs and images. A box's and a gradient's quad is not sent: it is their shape grown to whole pixels and cut
// to their clip grown so, as `Canvas::push` cuts it on the CPU, computed here again. An image's
// clip is sent as `Snorm16x4`, its edges' offsets from the quad's, each within one pixel: a clip's
// edge further out changes no pixel of the quad (see `instances.rs`, `clip_offsets`).

struct Frame {
    size: vec2<f32>,
    encode: f32,
    unused: f32,
}

struct Transform {
    x: vec4<f32>,
    y: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<uniform> transforms: array<Transform, 512>;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

// The corner of the quad that vertex `vertex` of an instance is: two triangles, (0, 0), (1, 0),
// (0, 1), then (0, 1), (1, 0), (1, 1).
fn corner_of(vertex: u32) -> vec2<f32> {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    return corners[vertex % 6u];
}

// `point`, in the target's pixels, placed by the transform `index`, in clip space.
fn place(point: vec2<f32>, index: u32) -> vec4<f32> {
    let transform = transforms[index];
    let placed = vec2<f32>(
        dot(transform.x.xyz, vec3<f32>(point, 1.0)),
        dot(transform.y.xyz, vec3<f32>(point, 1.0)),
    );
    let ndc = placed / frame.size * 2.0 - 1.0;
    return vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
}

// The quad of a shape `shape` clipped to `clip`, as `Canvas::push` cuts it.
fn cut_quad(shape: vec4<f32>, clip: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(max(floor(shape.xy), floor(clip.xy)), min(ceil(shape.zw), ceil(clip.zw)));
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

// The share of the pixel at `p` inside `clip`.
fn clip_share(p: vec2<f32>, clip: vec4<f32>) -> f32 {
    let share = clamp(
        min(clip.zw, p + 0.5) - max(clip.xy, p - 0.5),
        vec2<f32>(0.0),
        vec2<f32>(1.0),
    );
    return share.x * share.y;
}

// BOX: 72 bytes an instance.
struct BoxIn {
    @location(0) shape: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) fill: vec4<f32>,
    @location(4) border: vec4<f32>,
    @location(5) width: f32,
    @location(6) transform: u32,
}

struct BoxOut {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) radii: vec4<f32>,
    @location(3) @interpolate(flat) fill: vec4<f32>,
    @location(4) @interpolate(flat) border: vec4<f32>,
    @location(5) @interpolate(flat) clip: vec4<f32>,
    @location(6) @interpolate(flat) width: f32,
}

@vertex
fn vs_box(@builtin(vertex_index) vertex: u32, item: BoxIn) -> BoxOut {
    let quad = cut_quad(item.shape, item.clip);
    let point = mix(quad.xy, quad.zw, corner_of(vertex));
    var out: BoxOut;
    out.position = place(point, item.transform);
    out.point = point;
    out.shape = item.shape;
    out.radii = item.radii;
    out.fill = item.fill;
    out.border = item.border;
    out.clip = item.clip;
    out.width = item.width;
    return out;
}

@fragment
fn fs_box(in: BoxOut) -> @location(0) vec4<f32> {
    let p = in.point;
    let outer = coverage(rounded_box(p, in.shape, in.radii));
    let width = in.width;
    var filled = outer;
    if width > 0.0 {
        let inner_edges = in.shape + vec4<f32>(width, width, -width, -width);
        let inner_radii = max(in.radii - width, vec4<f32>(0.0));
        filled = min(coverage(rounded_box(p, inner_edges, inner_radii)), outer);
    }
    let color = in.fill * filled + in.border * (outer - filled);
    return color * clip_share(p, in.clip);
}

// SHADOW: 104 bytes an instance. The exponents' inverses are computed here, not sent.
struct ShadowIn {
    @location(0) quad: vec4<f32>,
    @location(1) shape: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) exponent: vec4<f32>,
    @location(4) clip: vec4<f32>,
    // The box's shorter side, the approximation's scale, the inverse of its deviation.
    @location(5) widths: vec3<f32>,
    @location(6) color: vec4<f32>,
    @location(7) transform: u32,
}

struct ShadowOut {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) radii: vec4<f32>,
    @location(3) @interpolate(flat) color: vec4<f32>,
    @location(4) @interpolate(flat) exponent: vec4<f32>,
    @location(5) @interpolate(flat) inverse_exponent: vec4<f32>,
    @location(6) @interpolate(flat) clip: vec4<f32>,
    @location(7) @interpolate(flat) widths: vec3<f32>,
}

@vertex
fn vs_shadow(@builtin(vertex_index) vertex: u32, item: ShadowIn) -> ShadowOut {
    let point = mix(item.quad.xy, item.quad.zw, corner_of(vertex));
    var out: ShadowOut;
    out.position = place(point, item.transform);
    out.point = point;
    out.shape = item.shape;
    out.radii = item.radii;
    out.color = item.color;
    out.exponent = item.exponent;
    out.inverse_exponent = 1.0 / item.exponent;
    out.clip = item.clip;
    out.widths = item.widths;
    return out;
}

@fragment
fn fs_shadow(in: ShadowOut) -> @location(0) vec4<f32> {
    let p = in.point;
    let center = (in.shape.xy + in.shape.zw) * 0.5;
    let half_size = (in.shape.zw - in.shape.xy) * 0.5;
    let q = p - center;
    let radius = of_corner(in.radii, q);
    let d = abs(q) - half_size + radius;
    let beyond = max(d, vec2<f32>(0.0));
    var corner = beyond.x + beyond.y;
    if beyond.x > 0.0 && beyond.y > 0.0 {
        let exponent = of_corner(in.exponent, q);
        corner = pow(pow(beyond.x, exponent) + pow(beyond.y, exponent), of_corner(in.inverse_exponent, q));
    }
    let distance = corner + min(max(d.x, d.y), 0.0) - radius;
    let inverse = in.widths.z;
    var shade = coverage(distance);
    if inverse > 0.0 {
        shade = in.widths.y * (erf((in.widths.x + distance) * inverse) - erf(distance * inverse));
    }
    let color = in.color * shade;
    return color * clip_share(p, in.clip);
}

// GRADIENT: 84 bytes an instance; its corners' colours top left, top right, bottom right, bottom
// left.
struct GradientIn {
    @location(0) shape: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) top_left: vec4<f32>,
    @location(4) top_right: vec4<f32>,
    @location(5) bottom_right: vec4<f32>,
    @location(6) bottom_left: vec4<f32>,
    @location(7) transform: u32,
}

struct GradientOut {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) radii: vec4<f32>,
    @location(3) @interpolate(flat) top_left: vec4<f32>,
    @location(4) @interpolate(flat) top_right: vec4<f32>,
    @location(5) @interpolate(flat) bottom_right: vec4<f32>,
    @location(6) @interpolate(flat) bottom_left: vec4<f32>,
    @location(7) @interpolate(flat) clip: vec4<f32>,
}

@vertex
fn vs_gradient(@builtin(vertex_index) vertex: u32, item: GradientIn) -> GradientOut {
    let quad = cut_quad(item.shape, item.clip);
    let point = mix(quad.xy, quad.zw, corner_of(vertex));
    var out: GradientOut;
    out.position = place(point, item.transform);
    out.point = point;
    out.shape = item.shape;
    out.radii = item.radii;
    out.top_left = item.top_left;
    out.top_right = item.top_right;
    out.bottom_right = item.bottom_right;
    out.bottom_left = item.bottom_left;
    out.clip = item.clip;
    return out;
}

@fragment
fn fs_gradient(in: GradientOut) -> @location(0) vec4<f32> {
    let p = in.point;
    let outer = coverage(rounded_box(p, in.shape, in.radii));
    let size = max(in.shape.zw - in.shape.xy, vec2<f32>(1e-6));
    let t = clamp((p - in.shape.xy) / size, vec2<f32>(0.0), vec2<f32>(1.0));
    let top = mix(in.top_left, in.top_right, t.x);
    let bottom = mix(in.bottom_left, in.bottom_right, t.x);
    let color = mix(top, bottom, t.y) * outer;
    return color * clip_share(p, in.clip);
}

// COVERAGE and IMAGE: 48 bytes an instance, one format and one vertex shader; only their fragment
// shaders differ. IMAGE reads its colour's alpha alone.
struct ImageIn {
    @location(0) quad: vec4<f32>,
    @location(1) uv: vec4<f32>,
    // The clip's edges less the quad's, each within [-1, 1].
    @location(2) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) transform: u32,
}

struct ImageOut {
    @builtin(position) position: vec4<f32>,
    // The point, then the image's coordinates.
    @location(0) point_uv: vec4<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
}

@vertex
fn vs_image(@builtin(vertex_index) vertex: u32, item: ImageIn) -> ImageOut {
    let corner = corner_of(vertex);
    let point = mix(item.quad.xy, item.quad.zw, corner);
    var out: ImageOut;
    out.position = place(point, item.transform);
    out.point_uv = vec4<f32>(point, mix(item.uv.xy, item.uv.zw, corner));
    out.color = item.color;
    out.clip = item.quad + item.clip;
    return out;
}

@fragment
fn fs_coverage(in: ImageOut) -> @location(0) vec4<f32> {
    let sampled = textureSampleLevel(image, image_sampler, in.point_uv.zw, 0.0);
    let color = in.color * sampled.r;
    return color * clip_share(in.point_uv.xy, in.clip);
}

@fragment
fn fs_image(in: ImageOut) -> @location(0) vec4<f32> {
    let sampled = textureSampleLevel(image, image_sampler, in.point_uv.zw, 0.0);
    var rgb = sampled.rgb;
    if frame.encode > 0.0 {
        rgb = linear_to_srgb(min(rgb, vec3<f32>(1.0)));
    }
    let color = vec4<f32>(rgb * sampled.a, sampled.a) * in.color.a;
    return color * clip_share(in.point_uv.xy, in.clip);
}
