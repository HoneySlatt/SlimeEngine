// The shader of every draw: vertices transformed by their instance and the camera, surfaces lit
// by the scene's directional light and colored by their material.

struct FrameUniform {
    // From world space to clip space.
    view_proj: mat4x4<f32>,
    // Towards the light, of length one, in world space; w unused.
    to_light: vec4<f32>,
    // The directional light's color and strength; w unused.
    light_color: vec4<f32>,
    // The ambient light's color and strength; w unused.
    ambient: vec4<f32>,
}
@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct MaterialUniform {
    base_color: vec4<f32>,
}
@group(1) @binding(0) var<uniform> material: MaterialUniform;
// Multiplied with the base color; white where the material has no texture.
@group(1) @binding(1) var base_color_texture: texture_2d<f32>;
@group(1) @binding(2) var base_color_sampler: sampler;

struct InstanceUniform {
    // From the mesh's space to world space.
    model: mat4x4<f32>,
    // For normals: proportional to the inverse transpose of the model's rotation and scale, so
    // that normals stay perpendicular to surfaces scaled unevenly. It gives directions only.
    normal: mat3x3<f32>,
}
@group(2) @binding(0) var<uniform> instance: InstanceUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // In world space, of any length: the normal matrix gives directions only, and interpolation
    // across the triangle shortens it.
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = frame.view_proj * instance.model * vec4<f32>(input.position, 1.0);
    output.normal = instance.normal * input.normal;
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let base = material.base_color
        * textureSample(base_color_texture, base_color_sampler, input.uv);
    let normal = normalize(input.normal);
    let diffuse = max(dot(normal, frame.to_light.xyz), 0.0);
    let light = frame.ambient.rgb + frame.light_color.rgb * diffuse;
    return vec4<f32>(base.rgb * light, base.a);
}
