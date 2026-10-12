//! Errors from creating GPU objects: they are returned, readable, and name the object's label.

use slime_gfx::{
    GfxError, Gpu, RenderPipelineDescriptor, TextureFormat, VertexAttribute, VertexFormat,
    VertexLayout,
};

const SHADER: &str = "
@vertex
fn vs_main(@location(0) position: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(position, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
";

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

#[test]
fn a_wgsl_error_is_returned_with_its_position() {
    let error = gpu()
        .create_shader_module("broken shader", "@fragment fn main( -> {}")
        .expect_err("invalid WGSL must be rejected");
    let GfxError::Shader { label, message } = &error else {
        panic!("expected a shader error, got {error:?}");
    };
    assert_eq!(label, "broken shader");
    assert!(
        message.contains(":1:"),
        "the message gives the line of the error: {message}"
    );
}

#[test]
fn a_pipeline_error_names_the_pipeline() {
    let gpu = gpu();
    let shader = gpu
        .create_shader_module("white", SHADER)
        .expect("the shader compiles");
    let error = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "pipeline with a missing entry point",
            shader: &shader,
            vertex_entry_point: "does_not_exist",
            fragment_entry_point: "fs_main",
            vertex_layout: VertexLayout {
                stride: 8,
                attributes: &[VertexAttribute {
                    location: 0,
                    offset: 0,
                    format: VertexFormat::Float32x2,
                }],
            },
            target_format: TextureFormat::Rgba8UnormSrgb,
        })
        .expect_err("a missing entry point must be rejected");
    let GfxError::Pipeline { message, .. } = &error else {
        panic!("expected a pipeline error, got {error:?}");
    };
    assert!(
        message.contains("pipeline with a missing entry point"),
        "wgpu's message names the label: {message}"
    );
}

#[test]
#[should_panic(expected = "vertex buffer `empty mesh` is empty")]
fn an_empty_vertex_buffer_is_rejected_at_creation() {
    gpu().create_vertex_buffer("empty mesh", &[]);
}
