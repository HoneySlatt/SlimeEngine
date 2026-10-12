// The shader of every draw: vertices transformed by their instance and the camera, colored by
// their material.

struct FrameUniform {
    // From world space to clip space.
    view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct MaterialUniform {
    base_color: vec4<f32>,
}
@group(1) @binding(0) var<uniform> material: MaterialUniform;

struct InstanceUniform {
    // From the mesh's space to world space.
    model: mat4x4<f32>,
}
@group(2) @binding(0) var<uniform> instance: InstanceUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> @builtin(position) vec4<f32> {
    return frame.view_proj * instance.model * vec4<f32>(input.position, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return material.base_color;
}
