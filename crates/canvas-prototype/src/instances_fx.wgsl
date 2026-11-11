// I with the effects (`fx_common.wgsl` comes first): each kind's instance carries its transform's
// six values (two `Float32x3`) in place of an index, and its clip as the target's pixels
// (`Float32x4`), since a transformed quad is no longer cut to it. Paths read their segments from an
// `Rgba32Float` texture 2048 texels wide, one texel a segment: an instance holds a fixed size.
// `instances_fx.rs` gives each format.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var segments_texture: texture_2d<f32>;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var image_sampler: sampler;

fn segment(index: u32) -> vec4<f32> {
    return textureLoad(segments_texture, vec2<i32>(i32(index % ROW), i32(index / ROW)), 0);
}

fn instance_transform(x: vec3<f32>, y: vec3<f32>) -> Transform {
    return Transform(vec4<f32>(x, 0.0), vec4<f32>(y, 0.0));
}

// A box's quad: cut to its clip for the identity, as `Canvas::push` cuts it; whole and grown for a
// transformed box, whose clip stays in the target's pixels.
fn box_quad(shape: vec4<f32>, clip: vec4<f32>, scale: f32) -> vec4<f32> {
    return select(cut_quad(shape, clip), grown(outward(shape), scale), scale > 0.0);
}

// BOX: 92 bytes.
struct BoxIn {
    @location(0) shape: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) fill: vec4<f32>,
    @location(4) border: vec4<f32>,
    @location(5) width: f32,
    @location(6) tx: vec3<f32>,
    @location(7) ty: vec3<f32>,
}

struct BoxOut {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) radii: vec4<f32>,
    @location(3) @interpolate(flat) fill: vec4<f32>,
    @location(4) @interpolate(flat) border: vec4<f32>,
    @location(5) @interpolate(flat) clip: vec4<f32>,
    // The border's width, the transform's scale.
    @location(6) @interpolate(flat) width_scale: vec2<f32>,
}

@vertex
fn vs_box(@builtin(vertex_index) vertex: u32, item: BoxIn) -> BoxOut {
    let t = instance_transform(item.tx, item.ty);
    let scale = scale_of(t);
    let quad = box_quad(item.shape, item.clip, scale);
    let point = mix(quad.xy, quad.zw, corner_of(vertex));
    var out: BoxOut;
    out.position = to_clip(placed(t, point), frame.size);
    out.point = point;
    out.shape = item.shape;
    out.radii = item.radii;
    out.fill = item.fill;
    out.border = item.border;
    out.clip = item.clip;
    out.width_scale = vec2<f32>(item.width, scale);
    return out;
}

@fragment
fn fs_box(in: BoxOut) -> @location(0) vec4<f32> {
    let s = distance_scale(in.width_scale.y);
    let color = box_color(in.point, in.shape, in.radii, in.fill, in.border, in.width_scale.x, s);
    return color * clip_share(in.position.xy, in.clip);
}

// GLASS: 108 bytes, a box's and the map to the image of what lies behind it.
struct GlassIn {
    @location(0) shape: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) tint: vec4<f32>,
    @location(4) border: vec4<f32>,
    @location(5) width: f32,
    @location(6) map: vec4<f32>,
    @location(7) tx: vec3<f32>,
    @location(8) ty: vec3<f32>,
}

struct GlassOut {
    @builtin(position) position: vec4<f32>,
    // The point, then the coordinates in the image behind.
    @location(0) point_uv: vec4<f32>,
    @location(1) @interpolate(flat) shape: vec4<f32>,
    @location(2) @interpolate(flat) radii: vec4<f32>,
    @location(3) @interpolate(flat) tint: vec4<f32>,
    @location(4) @interpolate(flat) border: vec4<f32>,
    @location(5) @interpolate(flat) clip: vec4<f32>,
    @location(6) @interpolate(flat) width_scale: vec2<f32>,
}

@vertex
fn vs_glass(@builtin(vertex_index) vertex: u32, item: GlassIn) -> GlassOut {
    let t = instance_transform(item.tx, item.ty);
    let scale = scale_of(t);
    let quad = box_quad(item.shape, item.clip, scale);
    let point = mix(quad.xy, quad.zw, corner_of(vertex));
    var out: GlassOut;
    out.position = to_clip(placed(t, point), frame.size);
    out.point_uv = vec4<f32>(point, glass_uv(point, item.map));
    out.shape = item.shape;
    out.radii = item.radii;
    out.tint = item.tint;
    out.border = item.border;
    out.clip = item.clip;
    out.width_scale = vec2<f32>(item.width, scale);
    return out;
}

@fragment
fn fs_glass(in: GlassOut) -> @location(0) vec4<f32> {
    let s = distance_scale(in.width_scale.y);
    let behind = textureSampleLevel(image, image_sampler, in.point_uv.zw, 0.0);
    let color = box_color(in.point_uv.xy, in.shape, in.radii, glass_fill(in.tint, behind), in.border, in.width_scale.x, s);
    return color * clip_share(in.position.xy, in.clip);
}

// SHADOW: 124 bytes.
struct ShadowIn {
    @location(0) quad: vec4<f32>,
    @location(1) shape: vec4<f32>,
    @location(2) radii: vec4<f32>,
    @location(3) exponent: vec4<f32>,
    @location(4) clip: vec4<f32>,
    @location(5) widths: vec3<f32>,
    @location(6) color: vec4<f32>,
    @location(7) tx: vec3<f32>,
    @location(8) ty: vec3<f32>,
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
    // The box's shorter side, the approximation's scale, the inverse of its deviation, the
    // transform's scale.
    @location(7) @interpolate(flat) widths_scale: vec4<f32>,
}

@vertex
fn vs_shadow(@builtin(vertex_index) vertex: u32, item: ShadowIn) -> ShadowOut {
    let t = instance_transform(item.tx, item.ty);
    let scale = scale_of(t);
    let quad = grown(item.quad, scale);
    let point = mix(quad.xy, quad.zw, corner_of(vertex));
    var out: ShadowOut;
    out.position = to_clip(placed(t, point), frame.size);
    out.point = point;
    out.shape = item.shape;
    out.radii = item.radii;
    out.color = item.color;
    out.exponent = item.exponent;
    out.inverse_exponent = 1.0 / item.exponent;
    out.clip = item.clip;
    out.widths_scale = vec4<f32>(item.widths, scale);
    return out;
}

@fragment
fn fs_shadow(in: ShadowOut) -> @location(0) vec4<f32> {
    let s = distance_scale(in.widths_scale.w);
    let color = shadow_color(in.point, in.shape, in.radii, in.exponent, in.inverse_exponent, in.widths_scale.xyz, in.color, s);
    return color * clip_share(in.position.xy, in.clip);
}

// GRADIENT: 104 bytes.
struct GradientIn {
    @location(0) shape: vec4<f32>,
    @location(1) radii: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) top_left: vec4<f32>,
    @location(4) top_right: vec4<f32>,
    @location(5) bottom_right: vec4<f32>,
    @location(6) bottom_left: vec4<f32>,
    @location(7) tx: vec3<f32>,
    @location(8) ty: vec3<f32>,
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
    @location(8) @interpolate(flat) scale: f32,
}

@vertex
fn vs_gradient(@builtin(vertex_index) vertex: u32, item: GradientIn) -> GradientOut {
    let t = instance_transform(item.tx, item.ty);
    let scale = scale_of(t);
    let quad = box_quad(item.shape, item.clip, scale);
    let point = mix(quad.xy, quad.zw, corner_of(vertex));
    var out: GradientOut;
    out.position = to_clip(placed(t, point), frame.size);
    out.point = point;
    out.shape = item.shape;
    out.radii = item.radii;
    out.top_left = item.top_left;
    out.top_right = item.top_right;
    out.bottom_right = item.bottom_right;
    out.bottom_left = item.bottom_left;
    out.clip = item.clip;
    out.scale = scale;
    return out;
}

@fragment
fn fs_gradient(in: GradientOut) -> @location(0) vec4<f32> {
    let color = gradient_color(in.point, in.shape, in.radii, in.top_left, in.top_right, in.bottom_right, in.bottom_left, distance_scale(in.scale));
    return color * clip_share(in.position.xy, in.clip);
}

// COVERAGE, IMAGE and LAYER: 76 bytes, one format and one vertex shader.
struct ImageIn {
    @location(0) quad: vec4<f32>,
    @location(1) uv: vec4<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) tx: vec3<f32>,
    @location(5) ty: vec3<f32>,
}

struct ImageOut {
    @builtin(position) position: vec4<f32>,
    // The point, then the image's coordinates.
    @location(0) point_uv: vec4<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
    // The quad before it grew, whose edges a transformed image smooths.
    @location(3) @interpolate(flat) quad: vec4<f32>,
    @location(4) @interpolate(flat) scale: f32,
}

@vertex
fn vs_image(@builtin(vertex_index) vertex: u32, item: ImageIn) -> ImageOut {
    let t = instance_transform(item.tx, item.ty);
    let scale = scale_of(t);
    let corner = corner_of(vertex);
    let quad = grown(item.quad, scale);
    let point = mix(quad.xy, quad.zw, corner);
    var out: ImageOut;
    out.position = to_clip(placed(t, point), frame.size);
    out.point_uv = vec4<f32>(point, image_uv(point, item.quad, item.uv, corner, scale));
    out.color = item.color;
    out.clip = item.clip;
    out.quad = item.quad;
    out.scale = scale;
    return out;
}

@fragment
fn fs_coverage(in: ImageOut) -> @location(0) vec4<f32> {
    let texel = textureSampleLevel(image, image_sampler, in.point_uv.zw, 0.0);
    let color = in.color * texel.r * image_edges(in.point_uv.xy, in.quad, in.scale);
    return color * clip_share(in.position.xy, in.clip);
}

@fragment
fn fs_image(in: ImageOut) -> @location(0) vec4<f32> {
    let texel = textureSampleLevel(image, image_sampler, in.point_uv.zw, 0.0);
    let color = image_color(texel, in.color.a, frame.encode) * image_edges(in.point_uv.xy, in.quad, in.scale);
    return color * clip_share(in.position.xy, in.clip);
}

@fragment
fn fs_layer(in: ImageOut) -> @location(0) vec4<f32> {
    let texel = textureSampleLevel(image, image_sampler, in.point_uv.zw, 0.0);
    let color = texel * in.color.a * image_edges(in.point_uv.xy, in.quad, in.scale);
    return color * clip_share(in.position.xy, in.clip);
}

// PATH: 68 bytes; its segments are those from `segments.x`, `segments.y` of them.
struct PathIn {
    @location(0) quad: vec4<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) segments: vec2<u32>,
    @location(4) tx: vec3<f32>,
    @location(5) ty: vec3<f32>,
}

struct PathOut {
    @builtin(position) position: vec4<f32>,
    @location(0) point: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>,
    @location(2) @interpolate(flat) clip: vec4<f32>,
    @location(3) @interpolate(flat) segments: vec2<u32>,
    @location(4) @interpolate(flat) scale: f32,
}

@vertex
fn vs_path(@builtin(vertex_index) vertex: u32, item: PathIn) -> PathOut {
    let t = instance_transform(item.tx, item.ty);
    let scale = scale_of(t);
    let quad = grown(item.quad, scale);
    let point = mix(quad.xy, quad.zw, corner_of(vertex));
    var out: PathOut;
    out.position = to_clip(placed(t, point), frame.size);
    out.point = point;
    out.color = item.color;
    out.clip = item.clip;
    out.segments = item.segments;
    out.scale = scale;
    return out;
}

@fragment
fn fs_path(in: PathOut) -> @location(0) vec4<f32> {
    let covered = path_coverage(in.point, in.segments.x, in.segments.y, distance_scale(in.scale));
    return in.color * covered * clip_share(in.position.xy, in.clip);
}
