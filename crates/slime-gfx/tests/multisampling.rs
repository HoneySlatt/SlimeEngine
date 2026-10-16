//! Multisampling: what a pass draws into a target of four samples per pixel, resolved into a target
//! of one, and the misuses of such targets. The edges drawn fall between the samples of the
//! standard pattern of Vulkan and Direct3D, which has one sample in each quarter of a pixel's width
//! (at 1/8, 3/8, 5/8 and 7/8 of it): an edge at a quarter of a pixel covers one sample or three, at
//! half of it two. The values are exact in half precision, and the average of four of them too.
//! Runs on whatever adapter the machine offers, like `device.rs`.

use slime_color::LinearColor;
use slime_gfx::{
    BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, CullMode, DepthBias,
    DepthCompare, DepthState, FloatImage, FragmentState, Gpu, MULTISAMPLE_COUNT,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RenderTarget, ShaderStages,
    TextureFormat,
};

/// The width and height of the targets, in pixels.
const WIDTH: u32 = 8;
const HEIGHT: u32 = 2;

/// Two rectangles across the whole height of the target, from x = 1.25 to 2.5 pixels and from 4 to
/// 5.25, in white, at depth 0.5; and two more for the depth test, at the vertex indices 12 to 23:
/// a red one from x = 0 to 1.5 at depth 0.75, then a green one across the whole target at depth
/// 0.25, behind it.
const SHADER: &str = "
struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

// A rectangle from x = `left` to `right`, in pixels of the 8-pixel-wide target, across its height.
fn rectangle(corner: u32, left: f32, right: f32) -> vec2<f32> {
    let xs = array<f32, 6>(left, right, right, left, right, left);
    let ys = array<f32, 6>(-1.0, -1.0, 1.0, -1.0, 1.0, 1.0);
    return vec2<f32>(xs[corner] / 4.0 - 1.0, ys[corner]);
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> Vertex {
    let lefts = array<f32, 4>(1.25, 4.0, 0.0, 0.0);
    let rights = array<f32, 4>(2.5, 5.25, 1.5, 8.0);
    let depths = array<f32, 4>(0.5, 0.5, 0.75, 0.25);
    let colors = array<vec4<f32>, 4>(
        vec4<f32>(1.0, 1.0, 1.0, 1.0),
        vec4<f32>(1.0, 1.0, 1.0, 1.0),
        vec4<f32>(1.0, 0.0, 0.0, 1.0),
        vec4<f32>(0.0, 1.0, 0.0, 1.0),
    );
    let quad = i / 6u;
    var out: Vertex;
    out.position = vec4<f32>(rectangle(i % 6u, lefts[quad], rights[quad]), depths[quad], 1.0);
    out.color = colors[quad];
    return out;
}

@fragment
fn fs_main(input: Vertex) -> @location(0) vec4<f32> {
    return input.color;
}
";

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// The pipeline of `SHADER`, multisampled or not, with or without a depth test.
fn pipeline(gpu: &Gpu, multisampled: bool, depth: Option<DepthState>) -> RenderPipeline {
    let shader = gpu
        .create_shader_module("rectangles", SHADER)
        .expect("the shader compiles");
    gpu.create_render_pipeline(&RenderPipelineDescriptor {
        label: "rectangles",
        shader: &shader,
        vertex_entry_point: "vs_main",
        vertex_layout: None,
        fragment: Some(FragmentState {
            entry_point: "fs_main",
            target_format: Some(TextureFormat::Rgba16Float),
        }),
        bind_group_layouts: &[],
        depth,
        cull: CullMode::None,
        multisampled,
    })
    .expect("the pipeline is valid")
}

fn multisampled_target(gpu: &Gpu) -> RenderTarget {
    gpu.create_multisampled_render_target("samples", WIDTH, HEIGHT, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it")
}

fn resolved_target(gpu: &Gpu) -> RenderTarget {
    gpu.create_render_target("resolved", WIDTH, HEIGHT, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it")
}

/// The red, green and blue of each pixel of the first row of `image`.
fn first_row(image: &FloatImage) -> Vec<[f32; 3]> {
    image.pixels[..(WIDTH * 4) as usize]
        .chunks(4)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect()
}

#[test]
fn a_resolved_edge_shows_the_share_of_the_pixel_it_covers() {
    let gpu = gpu();
    assert_eq!(MULTISAMPLE_COUNT, 4);
    let pipeline = pipeline(&gpu, true, None);
    let samples = multisampled_target(&gpu);
    let resolved = resolved_target(&gpu);
    // The rectangles are drawn in one pass and resolved in the next, which draws nothing: the
    // samples stay in the target between passes.
    gpu.submit("rectangles", |commands| {
        let drawn = RenderPassDescriptor {
            target: &samples,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: None,
            resolve: None,
        };
        commands.render(&drawn, |pass| {
            pass.set_pipeline(&pipeline);
            pass.draw(0..12);
        });
        let resolving = RenderPassDescriptor {
            clear: None,
            resolve: Some(&resolved),
            ..drawn
        };
        commands.render(&resolving, |_pass| {});
    });
    let image = gpu
        .read_float_render_target(&resolved)
        .expect("the resolved target is read back");
    // From 1.25 to 2.5, then from 4 to 5.25: a pixel crossed by an edge at a quarter of its width
    // keeps the samples on the rectangle's side, three or one of four; at half of it, two.
    let shares = [0.0, 0.75, 0.5, 0.0, 1.0, 0.25, 0.0, 0.0];
    let expected: Vec<[f32; 3]> = shares.iter().map(|&share| [share; 3]).collect();
    assert_eq!(first_row(&image), expected);
    assert_eq!(
        image.pixels[..(WIDTH * 4) as usize],
        image.pixels[(WIDTH * 4) as usize..]
    );
}

#[test]
fn the_depth_test_holds_for_each_sample() {
    let gpu = gpu();
    let pipeline = pipeline(
        &gpu,
        true,
        Some(DepthState {
            compare: DepthCompare::Greater,
            write: true,
            bias: DepthBias::default(),
        }),
    );
    let samples = multisampled_target(&gpu);
    let depth = gpu
        .create_multisampled_depth_target("depth", WIDTH, HEIGHT)
        .expect("the GPU has memory for it");
    let resolved = resolved_target(&gpu);
    let pass = RenderPassDescriptor {
        target: &samples,
        clear: Some(LinearColor::BLACK),
        depth: Some(&depth),
        timer: None,
        resolve: Some(&resolved),
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&pipeline);
        // The red rectangle, near, then the green one behind it, which shows only where the red
        // does not cover a sample.
        pass.draw(12..24);
    });
    let image = gpu
        .read_float_render_target(&resolved)
        .expect("the resolved target is read back");
    // The red rectangle ends halfway across the second pixel, on two of its samples: a depth per
    // pixel would show it all red or all green.
    let mut expected = vec![[0.0, 1.0, 0.0]; WIDTH as usize];
    expected[0] = [1.0, 0.0, 0.0];
    expected[1] = [0.5, 0.5, 0.0];
    assert_eq!(first_row(&image), expected);
}

#[test]
#[should_panic(expected = "COPY_SRC")]
fn a_multisampled_target_cannot_be_read_back() {
    let gpu = gpu();
    let _ = gpu.read_float_render_target(&multisampled_target(&gpu));
}

#[test]
#[should_panic(expected = "expects multisampled = false")]
fn a_multisampled_target_is_not_bound_as_a_texture_of_one_sample() {
    let gpu = gpu();
    let samples = multisampled_target(&gpu);
    let layout = gpu
        .create_bind_group_layout(
            "samples",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::Texture,
            }],
        )
        .expect("the GPU has memory for it");
    gpu.create_bind_group(
        "samples",
        &layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::RenderTarget(&samples),
        }],
    )
    .expect("the GPU has memory for it");
}

#[test]
#[should_panic(expected = "expects multisampled = false")]
fn a_multisampled_depth_target_cannot_be_bound() {
    let gpu = gpu();
    let depth = gpu
        .create_multisampled_depth_target("depth", WIDTH, HEIGHT)
        .expect("the GPU has memory for it");
    let layout = gpu
        .create_bind_group_layout(
            "depth",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::DepthTexture,
            }],
        )
        .expect("the GPU has memory for it");
    gpu.create_bind_group(
        "depth",
        &layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::DepthTarget(&depth),
        }],
    )
    .expect("the GPU has memory for it");
}

#[test]
#[should_panic(expected = "Incompatible sample count")]
fn a_pipeline_of_one_sample_does_not_draw_in_a_multisampled_pass() {
    let gpu = gpu();
    let pipeline = pipeline(&gpu, false, None);
    let samples = multisampled_target(&gpu);
    let pass = RenderPassDescriptor {
        target: &samples,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
        resolve: None,
    };
    gpu.render(&pass, |pass| pass.set_pipeline(&pipeline));
}

#[test]
#[should_panic(expected = "differing sample counts")]
fn a_depth_target_of_one_sample_does_not_go_with_a_multisampled_target() {
    let gpu = gpu();
    let samples = multisampled_target(&gpu);
    let depth = gpu
        .create_depth_target("depth", WIDTH, HEIGHT)
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &samples,
        clear: Some(LinearColor::BLACK),
        depth: Some(&depth),
        timer: None,
        resolve: None,
    };
    gpu.render(&pass, |_pass| {});
}

#[test]
#[should_panic(expected = "must be multi-sampled")]
fn a_target_of_one_sample_is_not_resolved() {
    let gpu = gpu();
    let target = resolved_target(&gpu);
    let resolved = resolved_target(&gpu);
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
        resolve: Some(&resolved),
    };
    gpu.render(&pass, |_pass| {});
}

#[test]
fn a_later_pass_reads_each_sample_of_a_multisampled_target() {
    let gpu = gpu();
    let pipeline = pipeline(&gpu, true, None);
    let samples = multisampled_target(&gpu);
    let layout = gpu
        .create_bind_group_layout(
            "samples",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::MultisampledTexture,
            }],
        )
        .expect("the GPU has memory for it");
    let group = gpu
        .create_bind_group(
            "samples",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::RenderTarget(&samples),
            }],
        )
        .expect("the GPU has memory for it");
    // Each sample of the pixel in one channel of the output.
    let shader = gpu
        .create_shader_module(
            "samples",
            "@group(0) @binding(0) var samples: texture_multisampled_2d<f32>;

             @vertex
             fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                 let corner = vec2<f32>(f32((i & 1u) * 4u) - 1.0, f32((i & 2u) * 2u) - 1.0);
                 return vec4<f32>(corner, 0.0, 1.0);
             }

             @fragment
             fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
                 let texel = vec2<u32>(position.xy);
                 return vec4<f32>(
                     textureLoad(samples, texel, 0).r,
                     textureLoad(samples, texel, 1).r,
                     textureLoad(samples, texel, 2).r,
                     textureLoad(samples, texel, 3).r,
                 );
             }",
        )
        .expect("the shader compiles");
    let reader = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "samples",
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
            multisampled: false,
        })
        .expect("the pipeline is valid");
    let read = resolved_target(&gpu);
    gpu.submit("samples", |commands| {
        let drawn = RenderPassDescriptor {
            target: &samples,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: None,
            resolve: None,
        };
        commands.render(&drawn, |pass| {
            pass.set_pipeline(&pipeline);
            pass.draw(0..12);
        });
        let reading = RenderPassDescriptor {
            target: &read,
            ..drawn
        };
        commands.render(&reading, |pass| {
            pass.set_pipeline(&reader);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3);
        });
    });
    let image = gpu
        .read_float_render_target(&read)
        .expect("the read target is read back");
    // Each sample is 0 or 1, and the samples covered are as many as the resolve averages.
    let covered: Vec<f32> = image.pixels[..(WIDTH * 4) as usize]
        .chunks(4)
        .map(|pixel| {
            assert!(pixel.iter().all(|&sample| sample == 0.0 || sample == 1.0));
            pixel.iter().sum()
        })
        .collect();
    assert_eq!(covered, [0.0, 3.0, 2.0, 0.0, 4.0, 1.0, 0.0, 0.0]);
}
