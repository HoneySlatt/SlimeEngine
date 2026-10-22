// The lines that a client draws into a view (ADR-0129): each segment a band that faces the camera,
// a width in the output's pixels wide, its ends squared and stretched by half its width, its edges
// and its ends smoothed over a pixel. Six vertices draw a segment, two triangles, built from the
// vertex's index alone: the segment is the index divided by 6, the corner its remainder. Drawn in
// the scene, with its depth test, and over the view's image, without.

// The start of shader.wgsl's FrameUniform, all that this shader reads of it.
struct FrameUniform {
    // From the frame's space, world space's axes around the camera, to clip space.
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct Segment {
    // Its ends, in the frame's space.
    start: vec3<f32>,
    // Its width, in the output's pixels.
    width: f32,
    end: vec3<f32>,
    // Linear, with its alpha.
    color: vec4<f32>,
}
@group(1) @binding(0) var<storage, read> segments: array<Segment>;

struct Settings {
    // Where clip space lies in the target drawn into, in its texels: the corner and the size of
    // the view's rectangle in the output, or the whole of the scene's targets.
    origin: vec2<f32>,
    extent: vec2<f32>,
    // The size of the view's rectangle, in the output's pixels, which clip space spans at both
    // points, whatever the size of the targets that the scene is drawn into.
    rectangle: vec2<f32>,
    // What the colors are multiplied by: 1 / HEADROOM in the scene, whose target holds light
    // divided by it (shader.wgsl), 1 over the image.
    scale: f32,
}
@group(1) @binding(1) var<uniform> settings: Settings;

struct LineVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Its ends on the screen, cut at the near plane, in the output's pixels from the rectangle's
    // top left corner, and half its width: what each pixel measures itself against, rather than
    // values interpolated across the band, which perspective and the GPU's clipping would bend.
    @location(1) @interpolate(flat) start: vec2<f32>,
    @location(2) @interpolate(flat) end: vec2<f32>,
    @location(3) @interpolate(flat) half_width: f32,
}

// For the corners 0 to 5 of the two triangles, bit by bit from the lowest: at which end each lies,
// 1 at `end` (corners 1, 2 and 4); and on which side, 1 on the left of the way from `start` to
// `end` (corners 2, 4 and 5).
const AT_END: u32 = 0x16u;
const ON_THE_LEFT: u32 = 0x34u;

// Beyond its edges and its ends, the band reaches out by a pixel, over which it fades.
const SMOOTHING: f32 = 1.0;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> LineVertex {
    let segment = segments[index / 6u];
    let corner = index % 6u;
    var output: LineVertex;
    output.color = segment.color;
    output.half_width = segment.width * 0.5;
    var a = frame.view_proj * vec4<f32>(segment.start, 1.0);
    var b = frame.view_proj * vec4<f32>(segment.end, 1.0);
    // In front of the near plane, w - z is not negative (reversed depth): the part behind it is cut
    // off, and a segment wholly behind it collapses to a point off the screen.
    let in_front_a = a.w - a.z;
    let in_front_b = b.w - b.z;
    if in_front_a < 0.0 && in_front_b < 0.0 {
        output.clip_position = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        return output;
    }
    if in_front_a < 0.0 {
        a = mix(a, b, in_front_a / (in_front_a - in_front_b));
    } else if in_front_b < 0.0 {
        b = mix(b, a, in_front_b / (in_front_b - in_front_a));
    }
    // The ends on screen, in the output's pixels, from the rectangle's top left corner, y down.
    output.start = on_screen(a);
    output.end = on_screen(b);
    let along = output.end - output.start;
    let on_screen_length = length(along);
    let direction = select(vec2<f32>(1.0, 0.0), along / on_screen_length, on_screen_length > 1e-6);
    let normal = vec2<f32>(-direction.y, direction.x);
    let at_end = ((AT_END >> corner) & 1u) == 1u;
    let side = select(-1.0, 1.0, ((ON_THE_LEFT >> corner) & 1u) == 1u);
    let reach = output.half_width + SMOOTHING;
    let outward = select(-1.0, 1.0, at_end);
    // Clip space's y runs up, the screen's down.
    let offset = normal * side * reach + direction * outward * reach;
    var position = select(a, b, at_end);
    let in_clip = offset * vec2<f32>(2.0, -2.0) / settings.rectangle;
    position = vec4<f32>(position.xy + in_clip * position.w, position.zw);
    output.clip_position = position;
    return output;
}

// Where `clip` lies on the screen, in the output's pixels from the rectangle's top left corner.
fn on_screen(clip: vec4<f32>) -> vec2<f32> {
    let ndc = clip.xy / clip.w;
    return vec2<f32>(ndc.x + 1.0, 1.0 - ndc.y) * 0.5 * settings.rectangle;
}

@fragment
fn fs_main(input: LineVertex) -> @location(0) vec4<f32> {
    // The pixel's centre, in the output's pixels from the rectangle's corner.
    let pixel = (input.clip_position.xy - settings.origin) * settings.rectangle / settings.extent;
    let along = input.end - input.start;
    let length_on_screen = length(along);
    let direction = select(vec2<f32>(1.0, 0.0), along / length_on_screen, length_on_screen > 1e-6);
    let from_start = pixel - input.start;
    let across = dot(from_start, vec2<f32>(-direction.y, direction.x));
    let past_ends = max(-dot(from_start, direction), dot(from_start, direction) - length_on_screen);
    // The share of the pixel that the band covers, across it and past its squared ends.
    let covered = clamp(input.half_width + 0.5 - abs(across), 0.0, 1.0)
        * clamp(input.half_width + 0.5 - past_ends, 0.0, 1.0);
    return vec4<f32>(input.color.rgb * settings.scale, input.color.a * covered);
}
