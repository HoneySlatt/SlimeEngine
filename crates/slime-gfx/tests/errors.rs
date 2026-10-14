//! Errors from creating GPU objects: they are returned, readable, and name the object's label;
//! and the misuses that are caught at creation or when reading back, which panic with the label.

use slime_gfx::{
    BindGroupLayoutEntry, BindingType, CullMode, FragmentState, GfxError, Gpu,
    RenderPipelineDescriptor, ShaderStages, TextureFormat, VertexAttribute, VertexFormat,
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

/// The same, moved by a uniform that the pipeline must provide a layout for.
const SHADER_WITH_UNIFORM: &str = "
@group(0) @binding(0) var<uniform> offset: vec4<f32>;

@vertex
fn vs_main(@location(0) position: vec2<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(position + offset.xy, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
";

/// A position in clip space, and nothing else.
const POSITION_ONLY: VertexLayout<'static> = VertexLayout {
    stride: 8,
    attributes: &[VertexAttribute {
        location: 0,
        offset: 0,
        format: VertexFormat::Float32x2,
    }],
};

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
            vertex_layout: Some(POSITION_ONLY),
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(TextureFormat::Rgba8UnormSrgb),
            }),
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
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
fn a_pipeline_whose_shader_bindings_are_not_in_its_layouts_is_rejected() {
    let gpu = gpu();
    let shader = gpu
        .create_shader_module("offset", SHADER_WITH_UNIFORM)
        .expect("the shader compiles");
    let error = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "pipeline without the layout of its uniform",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: Some(POSITION_ONLY),
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(TextureFormat::Rgba8UnormSrgb),
            }),
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
        })
        .expect_err("a binding that no layout holds must be rejected");
    let GfxError::Pipeline { label, message } = &error else {
        panic!("expected a pipeline error, got {error:?}");
    };
    assert_eq!(label, "pipeline without the layout of its uniform");
    assert!(
        message.to_lowercase().contains("binding"),
        "wgpu's message names the binding: {message}"
    );
}

#[test]
#[should_panic(expected = "vertex buffer `empty mesh` is empty")]
fn an_empty_vertex_buffer_is_rejected_at_creation() {
    gpu().create_vertex_buffer("empty mesh", &[]);
}

#[test]
#[should_panic(expected = "index buffer `empty mesh` is empty")]
fn an_empty_index_buffer_is_rejected_at_creation() {
    gpu().create_index_buffer("empty mesh", &[]);
}

#[test]
#[should_panic(expected = "uniform buffer `odd` holds 6 bytes")]
fn a_uniform_buffer_whose_size_is_not_a_multiple_of_4_is_rejected_at_creation() {
    gpu().create_uniform_buffer("odd", &[0; 6]);
}

#[test]
#[should_panic(expected = "texture `empty` is 0x4: a texture has at least one pixel")]
fn a_texture_without_pixels_is_a_bug_named_in_the_message() {
    gpu().create_texture("empty", 0, 4, TextureFormat::Rgba8UnormSrgb, &[]);
}

#[test]
#[should_panic(expected = "texture `tiny` is 2x2 and needs 16 bytes of pixels, but 12 were given")]
fn a_texture_with_the_wrong_number_of_bytes_is_rejected_at_creation() {
    gpu().create_texture("tiny", 2, 2, TextureFormat::Rgba8UnormSrgb, &[0; 12]);
}

#[test]
#[should_panic(expected = "uniform buffer `offset` holds 16 bytes, but 8 were written")]
fn a_write_of_another_size_than_the_uniform_buffer_is_rejected() {
    let gpu = gpu();
    let buffer = gpu.create_uniform_buffer("offset", &[0; 16]);
    assert_eq!(buffer.size(), 16);
    gpu.write_uniform_buffer(&buffer, &[0; 8]);
}

/// The pipeline error of `descriptor`, which must be rejected.
fn pipeline_error(gpu: &Gpu, descriptor: &RenderPipelineDescriptor<'_>) -> String {
    let error = gpu
        .create_render_pipeline(descriptor)
        .expect_err("the pipeline must be rejected");
    let GfxError::Pipeline { label, message } = error else {
        panic!("expected a pipeline error, got {error:?}");
    };
    assert_eq!(label, descriptor.label);
    message
}

#[test]
fn pipelines_without_what_their_shaders_or_passes_need_are_rejected() {
    let gpu = gpu();
    let shader = gpu
        .create_shader_module("white", SHADER)
        .expect("the shader compiles");
    let fragment = Some(FragmentState {
        entry_point: "fs_main",
        target_format: Some(TextureFormat::Rgba8UnormSrgb),
    });
    // The vertex shader reads a position at `@location(0)`, which no vertex buffer provides.
    let message = pipeline_error(
        &gpu,
        &RenderPipelineDescriptor {
            label: "pipeline without vertices",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment,
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
        },
    );
    assert!(
        message.contains("not provided"),
        "the message says what is missing: {message}"
    );
    // Neither a fragment stage nor a depth test: nothing to draw into.
    let message = pipeline_error(
        &gpu,
        &RenderPipelineDescriptor {
            label: "pipeline drawing into nothing",
            shader: &shader,
            vertex_entry_point: "vs_main",
            vertex_layout: Some(POSITION_ONLY),
            fragment: None,
            bind_group_layouts: &[],
            depth: None,
            cull: CullMode::None,
        },
    );
    assert!(
        message.contains("no render target"),
        "the message says what is missing: {message}"
    );
}

#[test]
fn bindings_declared_as_another_kind_than_their_layout_are_rejected() {
    let gpu = gpu();
    let fullscreen = "
        @vertex
        fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            return vec4<f32>(f32((i & 1u) * 4u) - 1.0, f32((i & 2u) * 2u) - 1.0, 0.0, 1.0);
        }
    ";
    let entry = |binding, ty| BindGroupLayoutEntry {
        binding,
        visible_to: ShaderStages::Fragment,
        ty,
    };
    let cases = [
        (
            "a float texture at a depth binding",
            "@group(0) @binding(0) var depth_map: texture_2d<f32>;
             @fragment
             fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                 return textureLoad(depth_map, vec2<i32>(p.xy), 0);
             }",
            vec![entry(0, BindingType::DepthTexture)],
        ),
        (
            "a comparison sampler at a filtering binding",
            "@group(0) @binding(0) var depth_map: texture_depth_2d;
             @group(0) @binding(1) var compare: sampler_comparison;
             @fragment
             fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                 return vec4<f32>(textureSampleCompare(depth_map, compare, p.xy / 8.0, 0.5));
             }",
            vec![
                entry(0, BindingType::DepthTexture),
                entry(1, BindingType::Sampler),
            ],
        ),
        (
            "a depth texture at a float binding",
            "@group(0) @binding(0) var depth_map: texture_depth_2d;
             @fragment
             fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                 return vec4<f32>(textureLoad(depth_map, vec2<i32>(p.xy), 0));
             }",
            vec![entry(0, BindingType::Texture)],
        ),
        (
            "a filtering sampler at a comparison binding",
            "@group(0) @binding(0) var image: texture_2d<f32>;
             @group(0) @binding(1) var linear: sampler;
             @fragment
             fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                 return textureSample(image, linear, p.xy / 8.0);
             }",
            vec![
                entry(0, BindingType::Texture),
                entry(1, BindingType::ComparisonSampler),
            ],
        ),
    ];
    for (label, fragment, entries) in cases {
        let shader = gpu
            .create_shader_module(label, &format!("{fullscreen}{fragment}"))
            .expect("the shader compiles");
        let layout = gpu.create_bind_group_layout(label, &entries);
        let message = pipeline_error(
            &gpu,
            &RenderPipelineDescriptor {
                label,
                shader: &shader,
                vertex_entry_point: "vs_main",
                vertex_layout: None,
                fragment: Some(FragmentState {
                    entry_point: "fs_main",
                    target_format: Some(TextureFormat::Rgba16Float),
                }),
                bind_group_layouts: &[&layout],
                depth: None,
                cull: CullMode::None,
            },
        );
        assert!(
            message.contains("doesn't match the shader"),
            "{label}: the message says the binding does not match: {message}"
        );
    }
}

#[test]
#[should_panic(
    expected = "render target `hdr` is Rgba16Float: read it with Gpu::read_float_render_target"
)]
fn a_float_target_is_not_read_as_bytes() {
    let gpu = gpu();
    let target = gpu.create_render_target("hdr", 1, 1, TextureFormat::Rgba16Float);
    let _ = gpu.read_render_target(&target);
}

#[test]
#[should_panic(expected = "render target `image` is Rgba8UnormSrgb, not Rgba16Float")]
fn an_8_bit_target_is_not_read_as_floats() {
    let gpu = gpu();
    let target = gpu.create_render_target("image", 1, 1, TextureFormat::Rgba8UnormSrgb);
    let _ = gpu.read_float_render_target(&target);
}

#[test]
#[should_panic(expected = "texture `hdr`: Rgba16Float textures take levels given by the caller")]
fn a_float_texture_takes_levels_given_by_the_caller() {
    gpu().create_texture("hdr", 1, 1, TextureFormat::Rgba16Float, &[0; 8]);
}
