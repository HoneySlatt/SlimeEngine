// The unlit kind of material (ADR-0129): its color, times its texture, shown as it is, whatever
// lights the scene and whatever its exposure. The scene pass draws it, reading the instances'
// values as shader.wgsl does; its surfaces cast no shadows, so it has no shadow passes.

// What the scene's target holds is divided by it, and the tone-mapping pass multiplies it back:
// HEADROOM in shader.wgsl is the same, which a unit test checks.
const HEADROOM: f32 = 64.0;

// The modes of debugging, as in shader.wgsl: the normals, or the share of the shadowing lights'
// light that reaches a surface, all of it for an unlit one.
const DEBUG_NORMALS: u32 = 1u;

// The start of shader.wgsl's FrameUniform, all that this shader reads of it.
struct FrameUniform {
    // From the frame's space, world space's axes around the camera, to clip space.
    view_proj: mat4x4<f32>,
    sky_from_clip: mat4x4<f32>,
    exposure: f32,
    light_count: u32,
    shadowed_light: u32,
    // 0 the scene; DEBUG_NORMALS or another a mode of debugging.
    debug: u32,
}
@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct MaterialUniform {
    color: vec4<f32>,
    // glTF's MASK: below it, nothing is drawn; 0 keeps every fragment.
    alpha_cutoff: f32,
    // 1 for a blended material, whose alpha lays it over what is behind it; 0 for the others,
    // written opaque.
    blended: f32,
}
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(1) @binding(1) var color_texture: texture_2d<f32>;
@group(1) @binding(2) var color_sampler: sampler;

// As in shader.wgsl.
struct Instance {
    model: mat4x4<f32>,
    normal: mat3x3<f32>,
    handedness: f32,
}
@group(2) @binding(0) var<storage, read> instances: array<Instance>;
@group(2) @binding(1) var<storage, read> drawn: array<u32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // With world space's axes, of any length.
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // The instance's handedness, -1 when its transform mirrors it, the same at every vertex.
    @location(2) @interpolate(flat) handedness: f32,
}

@vertex
fn vs_main(input: VertexInput, @builtin(instance_index) index: u32) -> VertexOutput {
    let instance = instances[drawn[index]];
    var output: VertexOutput;
    output.clip_position = frame.view_proj * (instance.model * vec4<f32>(input.position, 1.0));
    output.normal = instance.normal * input.normal;
    output.uv = input.uv;
    output.handedness = instance.handedness;
    return output;
}

@fragment
fn fs_main(
    input: VertexOutput,
    @builtin(front_facing) front_facing: bool,
) -> @location(0) vec4<f32> {
    let color = material.color * textureSample(color_texture, color_sampler, input.uv);
    if color.a < material.alpha_cutoff {
        discard;
    }
    let alpha = select(1.0, color.a, material.blended > 0.5);
    // Written as they are, without exposure, as a mode of debugging's colors.
    if frame.debug == DEBUG_NORMALS {
        // A back face, which a double-sided material draws, shows the normal it faces with; a
        // mirrored instance's front faces wind clockwise, which the rasterizer counts as back.
        let facing = front_facing == (input.handedness > 0.0);
        let normal = normalize(select(-input.normal, input.normal, facing));
        return vec4<f32>((normal * 0.5 + 0.5) / HEADROOM, alpha);
    }
    if frame.debug != 0u {
        return vec4<f32>(vec3<f32>(1.0 / HEADROOM), alpha);
    }
    return vec4<f32>(color.rgb / HEADROOM, alpha);
}
