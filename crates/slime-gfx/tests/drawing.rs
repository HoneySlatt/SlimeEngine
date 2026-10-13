//! Drawing with the resources a renderer needs: indexed draws, a uniform buffer read through a
//! bind group, a pass that keeps the image, depth testing and face culling, checked by reading
//! pixels back. The colors are pure, whose sRGB encoding every driver gets exactly. Runs on
//! whatever adapter the machine offers, like `device.rs`.

use slime_gfx::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    Color, CullMode, DepthBias, DepthCompare, DepthState, DepthTarget, FragmentState, Gpu, Image,
    RenderPass, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RenderTarget,
    ShaderStages, TextureFormat, UniformBuffer, VertexAttribute, VertexBuffer, VertexFormat,
    VertexLayout,
};

/// Vertices with a position in clip space and a color; a uniform offset moves them all.
const SHADER: &str = "
struct Offset {
    value: vec4<f32>,
}
@group(0) @binding(0) var<uniform> offset: Offset;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(@location(0) position: vec3<f32>, @location(1) color: vec3<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(position.xy + offset.value.xy, position.z, 1.0);
    output.color = color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
";

const SIZE: u32 = 8;
const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];
const BLUE: [f32; 3] = [0.0, 0.0, 1.0];

/// A position in clip space and a color.
type Vertex = ([f32; 3], [f32; 3]);

/// The triangle that covers the bottom-left quarter of the target. Moved by (1, 1), it covers the
/// top-right quarter.
const QUARTER: [Vertex; 3] = [
    ([-1.0, -1.0, 0.5], WHITE),
    ([0.0, -1.0, 0.5], WHITE),
    ([-1.0, 0.0, 0.5], WHITE),
];
/// A pixel inside `QUARTER`, and one inside it once moved by (1, 1).
const BOTTOM_LEFT: (u32, u32) = (1, 6);
const TOP_RIGHT: (u32, u32) = (5, 2);
const CENTER: (u32, u32) = (4, 4);

/// What every test draws with: a target, its depth target, and the offset uniform bound at
/// group 0.
struct Scene {
    gpu: Gpu,
    target: RenderTarget,
    depth: DepthTarget,
    layout: BindGroupLayout,
    offset: UniformBuffer,
    group: BindGroup,
}

impl Scene {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let target = gpu.create_render_target("scene", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
        let depth = gpu.create_depth_target("scene depth", SIZE, SIZE);
        let layout = gpu.create_bind_group_layout(
            "offset",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Vertex,
                ty: BindingType::UniformBuffer {
                    dynamic_offset: false,
                },
            }],
        );
        let offset = gpu.create_uniform_buffer("offset", &offset_bytes(0.0, 0.0));
        let group = gpu.create_bind_group(
            "offset",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::UniformBuffer(&offset),
            }],
        );
        Self {
            gpu,
            target,
            depth,
            layout,
            offset,
            group,
        }
    }

    /// A pipeline for `SHADER`, with the given depth test and culling, reading the offset at
    /// group 0 through the scene's layout.
    fn pipeline(&self, depth: Option<DepthState>, cull: CullMode) -> RenderPipeline {
        pipeline_for(&self.gpu, &self.layout, self.target.format(), depth, cull)
    }

    fn vertices(&self, vertices: &[Vertex]) -> VertexBuffer {
        let bytes: Vec<u8> = vertices
            .iter()
            .flat_map(|(position, color)| position.iter().chain(color))
            .flat_map(|value| value.to_le_bytes())
            .collect();
        self.gpu.create_vertex_buffer("vertices", &bytes)
    }

    /// Clears the target to black, binds the offset, lets `draw` draw, with the depth target when
    /// `with_depth`, then reads the image back.
    fn render(&self, with_depth: bool, draw: impl FnOnce(&mut RenderPass<'_>)) -> Image {
        let pass = RenderPassDescriptor {
            target: &self.target,
            clear: Some(BLACK),
            depth: with_depth.then_some(&self.depth),
            timer: None,
        };
        self.gpu.render(&pass, |pass| {
            pass.set_bind_group(0, &self.group, &[]);
            draw(pass);
        });
        self.image()
    }

    fn image(&self) -> Image {
        self.gpu
            .read_render_target(&self.target)
            .expect("the image is read back")
    }
}

/// The uniform: the offset as a `vec4<f32>`, 16 bytes.
fn offset_bytes(x: f32, y: f32) -> Vec<u8> {
    [x, y, 0.0, 0.0]
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// A triangle covering the whole target at depth `z`, wound counter-clockwise: a front face.
fn full_screen(z: f32, color: [f32; 3]) -> [Vertex; 3] {
    [
        ([-1.0, -1.0, z], color),
        ([3.0, -1.0, z], color),
        ([-1.0, 3.0, z], color),
    ]
}

/// The red, green and blue of the pixel at `(x, y)`, `y` counted from the top.
fn pixel(image: &Image, (x, y): (u32, u32)) -> [u8; 3] {
    let start = ((y * image.width + x) * 4) as usize;
    [
        image.pixels[start],
        image.pixels[start + 1],
        image.pixels[start + 2],
    ]
}

#[test]
fn an_indexed_quad_fills_the_target() {
    let scene = Scene::new();
    let pipeline = scene.pipeline(None, CullMode::None);
    let corners = scene.vertices(&[
        ([-1.0, -1.0, 0.5], WHITE),
        ([1.0, -1.0, 0.5], WHITE),
        ([1.0, 1.0, 0.5], WHITE),
        ([-1.0, 1.0, 0.5], WHITE),
    ]);
    let indices = scene.gpu.create_index_buffer("quad", &[0, 1, 2, 0, 2, 3]);
    assert_eq!(indices.count(), 6);
    let image = scene.render(false, |pass| {
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, &corners);
        pass.set_index_buffer(&indices);
        pass.draw_indexed(0..indices.count());
    });
    for pixel in image.pixels.chunks(4) {
        assert_eq!(pixel, [255, 255, 255, 255]);
    }
}

#[test]
fn a_uniform_offset_moves_the_triangle() {
    let scene = Scene::new();
    let pipeline = scene.pipeline(None, CullMode::None);
    let triangle = scene.vertices(&QUARTER);
    let draw = |pass: &mut RenderPass<'_>| {
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, &triangle);
        pass.draw(0..3);
    };

    let image = scene.render(false, draw);
    assert_eq!(pixel(&image, BOTTOM_LEFT), [255, 255, 255]);
    assert_eq!(pixel(&image, TOP_RIGHT), [0, 0, 0]);

    scene
        .gpu
        .write_uniform_buffer(&scene.offset, &offset_bytes(1.0, 1.0));
    let image = scene.render(false, draw);
    assert_eq!(pixel(&image, TOP_RIGHT), [255, 255, 255], "moved");
    assert_eq!(pixel(&image, BOTTOM_LEFT), [0, 0, 0]);
}

#[test]
fn a_pass_without_a_clear_keeps_what_was_drawn_before() {
    let scene = Scene::new();
    let pipeline = scene.pipeline(None, CullMode::None);
    let triangle = scene.vertices(&QUARTER);
    let draw = |pass: &mut RenderPass<'_>| {
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, &triangle);
        pass.draw(0..3);
    };

    scene.render(false, draw);
    scene
        .gpu
        .write_uniform_buffer(&scene.offset, &offset_bytes(1.0, 1.0));
    let pass = RenderPassDescriptor {
        target: &scene.target,
        clear: None,
        depth: None,
        timer: None,
    };
    scene.gpu.render(&pass, |pass| {
        pass.set_bind_group(0, &scene.group, &[]);
        draw(pass);
    });
    let image = scene.image();
    assert_eq!(pixel(&image, BOTTOM_LEFT), [255, 255, 255], "kept");
    assert_eq!(pixel(&image, TOP_RIGHT), [255, 255, 255], "added");
}

#[test]
fn the_depth_test_keeps_the_nearest_triangle_whatever_the_order() {
    let scene = Scene::new();
    let near = scene.vertices(&full_screen(0.25, RED));
    let far = scene.vertices(&full_screen(0.75, BLUE));
    let tested = scene.pipeline(
        Some(DepthState {
            compare: DepthCompare::Less,
            write: true,
            bias: DepthBias::default(),
        }),
        CullMode::None,
    );
    for order in [[&near, &far], [&far, &near]] {
        let image = scene.render(true, |pass| {
            pass.set_pipeline(&tested);
            for triangle in order {
                pass.set_vertex_buffer(0, triangle);
                pass.draw(0..3);
            }
        });
        assert_eq!(
            pixel(&image, CENTER),
            [255, 0, 0],
            "the nearer, red one wins"
        );
    }

    let untested = scene.pipeline(None, CullMode::None);
    let image = scene.render(false, |pass| {
        pass.set_pipeline(&untested);
        for triangle in [&near, &far] {
            pass.set_vertex_buffer(0, triangle);
            pass.draw(0..3);
        }
    });
    assert_eq!(
        pixel(&image, CENTER),
        [0, 0, 255],
        "without a depth test, the last one drawn wins"
    );
}

#[test]
fn faces_are_culled_as_the_pipeline_says() {
    let scene = Scene::new();
    let front = scene.vertices(&full_screen(0.5, WHITE));
    // The same triangle with two corners swapped winds clockwise: a back face.
    let back = scene.vertices(&[
        ([-1.0, -1.0, 0.5], WHITE),
        ([-1.0, 3.0, 0.5], WHITE),
        ([3.0, -1.0, 0.5], WHITE),
    ]);
    for (cull, triangle, expected, case) in [
        (
            CullMode::None,
            &back,
            [255, 255, 255],
            "no culling draws a back face",
        ),
        (
            CullMode::Back,
            &back,
            [0, 0, 0],
            "back-face culling drops it",
        ),
        (
            CullMode::Back,
            &front,
            [255, 255, 255],
            "and keeps a front face",
        ),
        (
            CullMode::Front,
            &front,
            [0, 0, 0],
            "front-face culling drops a front face",
        ),
    ] {
        let pipeline = scene.pipeline(None, cull);
        let image = scene.render(false, |pass| {
            pass.set_pipeline(&pipeline);
            pass.set_vertex_buffer(0, triangle);
            pass.draw(0..3);
        });
        assert_eq!(pixel(&image, CENTER), expected, "{case}");
    }
}

#[test]
#[should_panic(expected = "depth target `small depth` is 4x4 but render target `scene` is 8x8")]
fn a_depth_target_of_another_size_is_rejected() {
    let scene = Scene::new();
    let small = scene.gpu.create_depth_target("small depth", 4, 4);
    let pass = RenderPassDescriptor {
        target: &scene.target,
        clear: Some(BLACK),
        depth: Some(&small),
        timer: None,
    };
    scene.gpu.render(&pass, |_pass| {});
}

/// A pipeline for `SHADER` reading the offset at group 0 through `layout`, with the given depth
/// test and culling.
fn pipeline_for(
    gpu: &Gpu,
    layout: &BindGroupLayout,
    format: TextureFormat,
    depth: Option<DepthState>,
    cull: CullMode,
) -> RenderPipeline {
    let shader = gpu
        .create_shader_module("drawing", SHADER)
        .expect("the shader compiles");
    gpu.create_render_pipeline(&RenderPipelineDescriptor {
        label: "drawing",
        shader: &shader,
        vertex_entry_point: "vs_main",
        vertex_layout: Some(VertexLayout {
            stride: 6 * 4,
            attributes: &[
                VertexAttribute {
                    location: 0,
                    offset: 0,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    location: 1,
                    offset: 3 * 4,
                    format: VertexFormat::Float32x3,
                },
            ],
        }),
        fragment: Some(FragmentState {
            entry_point: "fs_main",
            target_format: format,
        }),
        bind_group_layouts: &[layout],
        depth,
        cull,
    })
    .expect("the pipeline is valid")
}

#[test]
fn a_dynamic_offset_selects_the_values_of_each_draw() {
    let scene = Scene::new();
    let alignment = scene.gpu.uniform_offset_alignment();
    // The values of two draws in one buffer, each at a multiple of the alignment.
    let mut slots = vec![0; alignment * 2];
    slots[..16].copy_from_slice(&offset_bytes(0.0, 0.0));
    slots[alignment..alignment + 16].copy_from_slice(&offset_bytes(1.0, 1.0));
    let buffer = scene.gpu.create_uniform_buffer("offsets", &slots);
    let layout = scene.gpu.create_bind_group_layout(
        "dynamic offset",
        &[BindGroupLayoutEntry {
            binding: 0,
            visible_to: ShaderStages::Vertex,
            ty: BindingType::UniformBuffer {
                dynamic_offset: true,
            },
        }],
    );
    let group = scene.gpu.create_bind_group(
        "dynamic offset",
        &layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::UniformBufferRange {
                buffer: &buffer,
                size: 16,
            },
        }],
    );
    let pipeline = pipeline_for(
        &scene.gpu,
        &layout,
        scene.target.format(),
        None,
        CullMode::None,
    );
    let triangle = scene.vertices(&QUARTER);

    let pass = RenderPassDescriptor {
        target: &scene.target,
        clear: Some(BLACK),
        depth: None,
        timer: None,
    };
    scene.gpu.render(&pass, |pass| {
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, &triangle);
        for offset in [0, alignment] {
            let offset = u32::try_from(offset).expect("two slots fit");
            pass.set_bind_group(0, &group, &[offset]);
            pass.draw(0..3);
        }
    });
    let image = scene.image();
    assert_eq!(pixel(&image, BOTTOM_LEFT), [255, 255, 255], "first draw");
    assert_eq!(pixel(&image, TOP_RIGHT), [255, 255, 255], "second draw");
    assert_eq!(pixel(&image, (6, 6)), [0, 0, 0], "in neither triangle");
}
