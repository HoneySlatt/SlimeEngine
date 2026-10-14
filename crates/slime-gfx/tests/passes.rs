//! Drawing in several passes: a fullscreen triangle without a vertex buffer, a pass that samples
//! what an earlier one drew into a 16-bit float target, a depth-only pass read through comparison
//! samplers, a depth-only pass whose fragment stage discards, depth bias, and the misuses that
//! several passes make easy. Values are chosen to be exact in half precision whichever way the
//! driver rounds, samples land on texel centers or exactly halfway between two, and no pixel center
//! lies on a triangle's edge. Runs on whatever adapter the machine offers, like `device.rs`.

use slime_gfx::{
    BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, CullMode, DepthBias,
    DepthCompare, DepthPassDescriptor, DepthState, Filter, FloatImage, FragmentState, Gpu,
    LinearColor, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, SamplerDescriptor,
    ShaderStages, TextureFormat,
};

/// The fullscreen triangle: its corners (-1, -1), (3, -1) and (-1, 3) wind counter-clockwise and
/// cover the whole target, built from the vertex index without a vertex buffer.
const FULLSCREEN: &str = "
fn corner(i: u32) -> vec2<f32> {
    return vec2<f32>(f32((i & 1u) * 4u) - 1.0, f32((i & 2u) * 2u) - 1.0);
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(corner(i), 0.0, 1.0);
}
";

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// A pipeline of the fullscreen triangle with the fragment entry point `fs_main` of `shader`,
/// which follows `FULLSCREEN`, reading the groups of `layouts`.
fn fullscreen_pipeline(
    gpu: &Gpu,
    label: &str,
    shader: &str,
    layouts: &[&slime_gfx::BindGroupLayout],
    target_format: TextureFormat,
) -> RenderPipeline {
    let module = gpu
        .create_shader_module(label, &format!("{FULLSCREEN}{shader}"))
        .expect("the shader compiles");
    gpu.create_render_pipeline(&RenderPipelineDescriptor {
        label,
        shader: &module,
        vertex_entry_point: "vs_fullscreen",
        vertex_layout: None,
        fragment: Some(FragmentState {
            entry_point: "fs_main",
            target_format: Some(target_format),
        }),
        bind_group_layouts: layouts,
        depth: None,
        cull: CullMode::Back,
    })
    .expect("the pipeline is valid")
}

/// A depth-only pipeline, whose vertex shader `vs_main` in `shader` builds its own vertices.
fn depth_pipeline(gpu: &Gpu, label: &str, shader: &str, depth: DepthState) -> RenderPipeline {
    let module = gpu
        .create_shader_module(label, &format!("{FULLSCREEN}{shader}"))
        .expect("the shader compiles");
    gpu.create_render_pipeline(&RenderPipelineDescriptor {
        label,
        shader: &module,
        vertex_entry_point: "vs_main",
        vertex_layout: None,
        fragment: None,
        bind_group_layouts: &[],
        depth: Some(depth),
        cull: CullMode::None,
    })
    .expect("the pipeline is valid")
}

fn pixel(image: &FloatImage, x: u32, y: u32) -> [f32; 4] {
    let start = ((y * image.width + x) * 4) as usize;
    image.pixels[start..start + 4]
        .try_into()
        .expect("four channels")
}

#[test]
fn a_fullscreen_triangle_without_a_vertex_buffer_covers_every_pixel() {
    let gpu = gpu();
    // 67 pixels of 8 bytes: rows of 536 bytes, padded to 768 when read back.
    let (width, height) = (67, 5);
    let target = gpu
        .create_render_target("positions", width, height, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    // Each pixel writes its own center; culling back faces proves the triangle faces the front.
    let pipeline = fullscreen_pipeline(
        &gpu,
        "positions",
        "@fragment
         fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
             return vec4<f32>(p.x, p.y, 1.0, 1.0);
         }",
        &[],
        TextureFormat::Rgba16Float,
    );
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(LinearColor::rgba(-1.0, -1.0, -1.0, -1.0)),
        depth: None,
        timer: None,
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&pipeline);
        pass.draw(0..3);
    });
    let image = gpu
        .read_float_render_target(&target)
        .expect("the image is read back");
    for y in 0..height {
        for x in 0..width {
            assert_eq!(
                pixel(&image, x, y),
                [x as f32 + 0.5, y as f32 + 0.5, 1.0, 1.0],
                "pixel ({x}, {y})"
            );
        }
    }
}

#[test]
fn a_second_pass_samples_the_target_a_first_pass_drew_into() {
    let gpu = gpu();
    let size = 8;
    let first = gpu
        .create_render_target("first", size, size, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let second = gpu
        .create_render_target("second", size, size, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let display = gpu
        .create_render_target("display", size, size, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");

    // The first pass writes each pixel's center, and a value below 0.
    let draw_first = fullscreen_pipeline(
        &gpu,
        "first",
        "@fragment
         fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
             return vec4<f32>(p.x, p.y, -2.5, 1.0);
         }",
        &[],
        TextureFormat::Rgba16Float,
    );
    let pass = |target| RenderPassDescriptor {
        target,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
    };
    gpu.render(&pass(&first), |pass| {
        pass.set_pipeline(&draw_first);
        pass.draw(0..3);
    });
    let image = gpu
        .read_float_render_target(&first)
        .expect("the image is read back");
    for (x, y) in [(0, 0), (7, 0), (3, 5), (7, 7)] {
        assert_eq!(
            pixel(&image, x, y),
            [x as f32 + 0.5, y as f32 + 0.5, -2.5, 1.0]
        );
    }

    let layout = gpu
        .create_bind_group_layout(
            "first",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Texture,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Sampler,
                },
            ],
        )
        .expect("the GPU has memory for it");
    let nearest = gpu
        .create_sampler(
            "nearest",
            &SamplerDescriptor {
                mag_filter: Filter::Nearest,
                min_filter: Filter::Nearest,
                address_u: slime_gfx::AddressMode::ClampToEdge,
                address_v: slime_gfx::AddressMode::ClampToEdge,
            },
        )
        .expect("the GPU has memory for it");
    let group = gpu
        .create_bind_group(
            "first",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::RenderTarget(&first),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&nearest),
                },
            ],
        )
        .expect("the GPU has memory for it");
    let bindings = "
        @group(0) @binding(0) var first: texture_2d<f32>;
        @group(0) @binding(1) var nearest: sampler;
    ";

    // The second pass reads the first mirrored left to right, at texel centers, and doubles it.
    let mirror = fullscreen_pipeline(
        &gpu,
        "mirror",
        &format!(
            "{bindings}
             @fragment
             fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {{
                 return textureSample(first, nearest, vec2<f32>(1.0 - p.x / 8.0, p.y / 8.0)) * 2.0;
             }}"
        ),
        &[&layout],
        TextureFormat::Rgba16Float,
    );
    gpu.render(&pass(&second), |pass| {
        pass.set_pipeline(&mirror);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3);
    });
    let image = gpu
        .read_float_render_target(&second)
        .expect("the image is read back");
    for y in 0..size {
        for x in 0..size {
            assert_eq!(
                pixel(&image, x, y),
                [15.0 - 2.0 * x as f32, 2.0 * y as f32 + 1.0, -5.0, 2.0],
                "pixel ({x}, {y})"
            );
        }
    }

    // A third pass copies the float target into an sRGB one, as tone mapping will: values are
    // limited to 0 to 1, and 0.5 is encoded as 188.
    let copy = fullscreen_pipeline(
        &gpu,
        "copy",
        &format!(
            "{bindings}
             @fragment
             fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {{
                 return textureSample(first, nearest, p.xy / 8.0);
             }}"
        ),
        &[&layout],
        TextureFormat::Rgba8UnormSrgb,
    );
    gpu.render(&pass(&display), |pass| {
        pass.set_pipeline(&copy);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3);
    });
    let image = gpu
        .read_render_target(&display)
        .expect("the image is read back");
    for y in 0..size {
        for x in 0..size {
            let start = ((y * size + x) * 4) as usize;
            let expected = |at: u32| if at == 0 { 188 } else { 255 };
            let actual = &image.pixels[start..start + 4];
            assert!(
                actual[0].abs_diff(expected(x)) <= 1
                    && actual[1].abs_diff(expected(y)) <= 1
                    && actual[2..] == [0, 255],
                "pixel ({x}, {y}): {actual:?}"
            );
        }
    }
}

#[test]
fn a_depth_only_pass_is_read_through_a_comparison_sampler() {
    let gpu = gpu();
    let size = 8;
    // The shadow map has twice the size of the target that reads it: a depth-only pass takes its
    // size from its depth target alone. The occluder covers its left half at depth 0.75, near the
    // light (depth is reversed): its right
    // edge, x = 0, lies between columns 7 and 8; its left edge stays left of the map on every row.
    let occluder = depth_pipeline(
        &gpu,
        "occluder",
        "@vertex
         fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
             var corners = array(vec2<f32>(-3.0, -1.0), vec2<f32>(0.0, -1.0), vec2<f32>(0.0, 3.0));
             return vec4<f32>(corners[i], 0.75, 1.0);
         }",
        DepthState {
            compare: DepthCompare::Greater,
            write: true,
            bias: DepthBias::default(),
        },
    );
    let shadow_map = gpu
        .create_depth_target("shadow map", 2 * size, 2 * size)
        .expect("the GPU has memory for it");
    gpu.render_depth(
        &DepthPassDescriptor {
            depth: &shadow_map,
            timer: None,
        },
        |pass| {
            pass.set_pipeline(&occluder);
            pass.draw(0..3);
        },
    );

    let layout = gpu
        .create_bind_group_layout(
            "shadow lookup",
            &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::DepthTexture,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::ComparisonSampler,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::ComparisonSampler,
                },
            ],
        )
        .expect("the GPU has memory for it");
    let nearest = gpu
        .create_comparison_sampler(
            "nearest comparison",
            DepthCompare::GreaterEqual,
            Filter::Nearest,
        )
        .expect("the GPU has memory for it");
    let linear = gpu
        .create_comparison_sampler(
            "linear comparison",
            DepthCompare::GreaterEqual,
            Filter::Linear,
        )
        .expect("the GPU has memory for it");
    let group = gpu
        .create_bind_group(
            "shadow lookup",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::DepthTarget(&shadow_map),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::ComparisonSampler(&nearest),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::ComparisonSampler(&linear),
                },
            ],
        )
        .expect("the GPU has memory for it");
    // Each pixel of the lookup covers 2 × 2 texels of the shadow map, which hold the same depth.
    // Red compares a reference of 0.5 there, green one of 0.9, blue 0.5 halfway between texels 2x
    // + 1 and 2x + 2, horizontally, with linear filtering, and alpha the very depth stored there,
    // which GreaterEqual finds equal and Greater would not.
    let lookup = fullscreen_pipeline(
        &gpu,
        "shadow lookup",
        "@group(0) @binding(0) var shadow_map: texture_depth_2d;
         @group(0) @binding(1) var nearest_compare: sampler_comparison;
         @group(0) @binding(2) var linear_compare: sampler_comparison;

         @fragment
         fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
             let uv = p.xy / 8.0;
             let between = (p.xy + vec2<f32>(0.5, 0.0)) / 8.0;
             return vec4<f32>(
                 textureSampleCompare(shadow_map, nearest_compare, uv, 0.5),
                 textureSampleCompare(shadow_map, nearest_compare, uv, 0.9),
                 textureSampleCompare(shadow_map, linear_compare, between, 0.5),
                 textureSampleCompare(
                     shadow_map,
                     nearest_compare,
                     uv,
                     textureLoad(shadow_map, vec2<i32>(p.xy) * 2, 0),
                 ),
             );
         }",
        &[&layout],
        TextureFormat::Rgba16Float,
    );
    let lit = gpu
        .create_render_target("lit", size, size, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &lit,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&lookup);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3);
    });
    let image = gpu
        .read_float_render_target(&lit)
        .expect("the image is read back");
    let filtered = [0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
    eprintln!(
        "adapter `{}`: comparison halfway between a shadowed and a lit texel gives {}",
        gpu.adapter_name(),
        pixel(&image, 3, 0)[2]
    );
    for y in 0..size {
        for x in 0..size {
            let [r, g, b, a] = pixel(&image, x, y);
            // 0.5 >= 0.75 fails where the occluder is, and 0.5 >= 0 (cleared) holds elsewhere;
            // a stored depth is greater than or equal to itself.
            let lit = if x <= 3 { 0.0 } else { 1.0 };
            assert_eq!((r, g, a), (lit, 1.0, 1.0), "pixel ({x}, {y})");
            // Filtering weights are fixed-point numbers on some GPUs.
            assert!(
                (b - filtered[x as usize]).abs() <= 1.0 / 256.0,
                "pixel ({x}, {y}): {b}"
            );
        }
    }
}

#[test]
fn a_fragment_stage_without_a_render_target_discards_in_a_depth_only_pass() {
    let gpu = gpu();
    let size = 8;
    // A triangle covering the target at depth 0.75, whose fragment stage writes no color and
    // discards the left half: as a shadow pass cuts out what masked materials discard.
    let module = gpu
        .create_shader_module(
            "cut out",
            &format!(
                "{FULLSCREEN}
                 @vertex
                 fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
                     return vec4<f32>(corner(i), 0.75, 1.0);
                 }}

                 @fragment
                 fn fs_main(@builtin(position) p: vec4<f32>) {{
                     if p.x < 4.0 {{
                         discard;
                     }}
                 }}"
            ),
        )
        .expect("the shader compiles");
    let cut_out = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "cut out",
            shader: &module,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: None,
            }),
            bind_group_layouts: &[],
            depth: Some(DepthState {
                compare: DepthCompare::Greater,
                write: true,
                bias: DepthBias::default(),
            }),
            cull: CullMode::None,
        })
        .expect("the pipeline is valid");
    let depth = gpu
        .create_depth_target("depth", size, size)
        .expect("the GPU has memory for it");
    gpu.render_depth(
        &DepthPassDescriptor {
            depth: &depth,
            timer: None,
        },
        |pass| {
            pass.set_pipeline(&cut_out);
            pass.draw(0..3);
        },
    );

    // Each pixel shows the depth stored under it: the cleared 0 where the fragment was
    // discarded, 0.75 elsewhere.
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
    let group = gpu
        .create_bind_group(
            "depth",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::DepthTarget(&depth),
            }],
        )
        .expect("the GPU has memory for it");
    let show = fullscreen_pipeline(
        &gpu,
        "show depth",
        "@group(0) @binding(0) var depth: texture_depth_2d;

         @fragment
         fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
             return vec4<f32>(textureLoad(depth, vec2<i32>(p.xy), 0), 0.0, 0.0, 1.0);
         }",
        &[&layout],
        TextureFormat::Rgba16Float,
    );
    let shown = gpu
        .create_render_target("shown", size, size, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &shown,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&show);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3);
    });
    let image = gpu
        .read_float_render_target(&shown)
        .expect("the image is read back");
    for y in 0..size {
        for x in 0..size {
            let expected = if x < 4 { 0.0 } else { 0.75 };
            assert_eq!(pixel(&image, x, y)[0], expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn a_depth_bias_moves_the_depth_by_the_expected_amount() {
    let gpu = gpu();
    let size = 8;
    // At depth 0.375, whose exponent is -2, the unit r of the constant bias is 2^-25: a driver
    // that used 2^-24, the unit of depths from 0.5 to 1, would measure twice the expected amount.
    let flat = "@vertex
        fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            return vec4<f32>(corner(i), 0.375, 1.0);
        }";
    // 0.25 at x = -1 and 0.75 at x = 3: 1/32 more per pixel of an 8-pixel target, along x only.
    let tilted = "@vertex
        fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
            let xy = corner(i);
            return vec4<f32>(xy, 0.25 + (xy.x + 1.0) / 8.0, 1.0);
        }";
    let depth = |bias| DepthState {
        compare: DepthCompare::Always,
        write: true,
        bias,
    };
    // A constant bias of 16 384 units of 2^-25: 2^-11. A slope-scaled bias of 2 times the slope of
    // 1/32 per pixel: 1/16.
    let cases = [
        ("plain flat", flat, DepthBias::default()),
        (
            "biased flat",
            flat,
            DepthBias {
                constant: 16_384,
                slope_scale: 0.0,
            },
        ),
        ("plain tilted", tilted, DepthBias::default()),
        (
            "biased tilted",
            tilted,
            DepthBias {
                constant: 0,
                slope_scale: 2.0,
            },
        ),
    ];
    let targets: Vec<_> = cases
        .iter()
        .map(|&(label, shader, bias)| {
            let target = gpu
                .create_depth_target(label, size, size)
                .expect("the GPU has memory for it");
            let pipeline = depth_pipeline(&gpu, label, shader, depth(bias));
            gpu.render_depth(
                &DepthPassDescriptor {
                    depth: &target,
                    timer: None,
                },
                |pass| {
                    pass.set_pipeline(&pipeline);
                    pass.draw(0..3);
                },
            );
            target
        })
        .collect();

    let entries: Vec<_> = (0..4)
        .map(|binding| BindGroupLayoutEntry {
            binding,
            visible_to: ShaderStages::Fragment,
            ty: BindingType::DepthTexture,
        })
        .collect();
    let layout = gpu
        .create_bind_group_layout("depths", &entries)
        .expect("the GPU has memory for it");
    let resources: Vec<_> = targets
        .iter()
        .zip(0..)
        .map(|(target, binding)| BindGroupEntry {
            binding,
            resource: BindingResource::DepthTarget(target),
        })
        .collect();
    let group = gpu
        .create_bind_group("depths", &layout, &resources)
        .expect("the GPU has memory for it");
    // Red and green scale the two biases so that the expected value is 1; blue and alpha give the
    // unbiased depths, which confirm the geometry.
    let measure = fullscreen_pipeline(
        &gpu,
        "measure",
        "@group(0) @binding(0) var plain_flat: texture_depth_2d;
         @group(0) @binding(1) var biased_flat: texture_depth_2d;
         @group(0) @binding(2) var plain_tilted: texture_depth_2d;
         @group(0) @binding(3) var biased_tilted: texture_depth_2d;

         @fragment
         fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
             let c = vec2<i32>(p.xy);
             let flat_depth = textureLoad(plain_flat, c, 0);
             let tilted_depth = textureLoad(plain_tilted, c, 0);
             return vec4<f32>(
                 (textureLoad(biased_flat, c, 0) - flat_depth) * 2048.0,
                 (textureLoad(biased_tilted, c, 0) - tilted_depth) * 16.0,
                 flat_depth,
                 tilted_depth,
             );
         }",
        &[&layout],
        TextureFormat::Rgba16Float,
    );
    let measured = gpu
        .create_render_target("measured", size, size, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &measured,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&measure);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3);
    });
    let image = gpu
        .read_float_render_target(&measured)
        .expect("the image is read back");
    let [constant, slope, _, _] = pixel(&image, 0, 0);
    eprintln!(
        "adapter `{}`: constant bias {constant} and slope-scaled bias {slope} of the expected \
         amounts",
        gpu.adapter_name()
    );
    for y in 0..size {
        for x in 0..size {
            let [constant, slope, flat_depth, tilted_depth] = pixel(&image, x, y);
            assert!(
                (constant - 1.0).abs() <= 1.0 / 256.0,
                "pixel ({x}, {y}): constant bias {constant}"
            );
            assert!(
                (slope - 1.0).abs() <= 1.0 / 256.0,
                "pixel ({x}, {y}): slope-scaled bias {slope}"
            );
            assert_eq!(flat_depth, 0.375, "pixel ({x}, {y})");
            let expected = (2.0 * x as f32 + 17.0) / 64.0;
            assert!(
                (tilted_depth - expected).abs() <= 1.0 / 1024.0,
                "pixel ({x}, {y}): depth {tilted_depth}, expected {expected}"
            );
        }
    }
}

#[test]
#[should_panic(expected = "conflicting usages")]
fn a_pass_cannot_sample_the_target_it_draws_into() {
    let gpu = gpu();
    let hdr = gpu
        .create_render_target("hdr", 8, 8, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let layout = gpu
        .create_bind_group_layout(
            "hdr",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::Texture,
            }],
        )
        .expect("the GPU has memory for it");
    let group = gpu
        .create_bind_group(
            "hdr",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::RenderTarget(&hdr),
            }],
        )
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &hdr,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
    };
    gpu.render(&pass, |pass| pass.set_bind_group(0, &group, &[]));
}

#[test]
#[should_panic(expected = "Incompatible color attachments")]
fn a_depth_only_pipeline_does_not_draw_in_a_pass_with_a_render_target() {
    let gpu = gpu();
    let pipeline = depth_pipeline(
        &gpu,
        "depth only",
        "@vertex
         fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
             return vec4<f32>(corner(i), 0.5, 1.0);
         }",
        DepthState {
            compare: DepthCompare::Greater,
            write: true,
            bias: DepthBias::default(),
        },
    );
    let target = gpu
        .create_render_target("color", 8, 8, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let depth = gpu
        .create_depth_target("depth", 8, 8)
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(LinearColor::BLACK),
        depth: Some(&depth),
        timer: None,
    };
    gpu.render(&pass, |pass| pass.set_pipeline(&pipeline));
}

#[test]
#[should_panic(expected = "Incompatible color attachments")]
fn a_pipeline_with_a_fragment_stage_does_not_draw_in_a_depth_only_pass() {
    let gpu = gpu();
    let pipeline = fullscreen_pipeline(
        &gpu,
        "color",
        "@fragment
         fn fs_main() -> @location(0) vec4<f32> {
             return vec4<f32>(1.0);
         }",
        &[],
        TextureFormat::Rgba16Float,
    );
    let depth = gpu
        .create_depth_target("depth", 8, 8)
        .expect("the GPU has memory for it");
    gpu.render_depth(
        &DepthPassDescriptor {
            depth: &depth,
            timer: None,
        },
        |pass| pass.set_pipeline(&pipeline),
    );
}

/// A bind group holding `depth`, at a depth-texture binding.
fn depth_group(gpu: &Gpu, depth: &slime_gfx::DepthTarget) -> slime_gfx::BindGroup {
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
            resource: BindingResource::DepthTarget(depth),
        }],
    )
    .expect("the GPU has memory for it")
}

#[test]
#[should_panic(expected = "conflicting usages")]
fn a_depth_only_pass_cannot_sample_the_depth_target_it_draws_into() {
    let gpu = gpu();
    let depth = gpu
        .create_depth_target("shadow map", 8, 8)
        .expect("the GPU has memory for it");
    let group = depth_group(&gpu, &depth);
    gpu.render_depth(
        &DepthPassDescriptor {
            depth: &depth,
            timer: None,
        },
        |pass| pass.set_bind_group(0, &group, &[]),
    );
}

#[test]
#[should_panic(expected = "conflicting usages")]
fn a_pass_cannot_sample_the_depth_target_it_tests_against() {
    let gpu = gpu();
    let target = gpu
        .create_render_target("color", 8, 8, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let depth = gpu
        .create_depth_target("depth", 8, 8)
        .expect("the GPU has memory for it");
    let group = depth_group(&gpu, &depth);
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(LinearColor::BLACK),
        depth: Some(&depth),
        timer: None,
    };
    gpu.render(&pass, |pass| pass.set_bind_group(0, &group, &[]));
}

#[test]
#[should_panic(expected = "sample type")]
fn a_depth_target_cannot_be_bound_as_a_float_texture() {
    let gpu = gpu();
    let depth = gpu
        .create_depth_target("depth", 8, 8)
        .expect("the GPU has memory for it");
    let layout = gpu
        .create_bind_group_layout(
            "float texture",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::Texture,
            }],
        )
        .expect("the GPU has memory for it");
    gpu.create_bind_group(
        "float texture",
        &layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::DepthTarget(&depth),
        }],
    )
    .expect("the GPU has memory for it");
}

#[test]
#[should_panic(expected = "comparison")]
fn a_filtering_sampler_cannot_be_bound_as_a_comparison_sampler() {
    let gpu = gpu();
    let sampler = gpu
        .create_sampler("filtering", &SamplerDescriptor::default())
        .expect("the GPU has memory for it");
    let layout = gpu
        .create_bind_group_layout(
            "comparison",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::ComparisonSampler,
            }],
        )
        .expect("the GPU has memory for it");
    gpu.create_bind_group(
        "comparison",
        &layout,
        &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Sampler(&sampler),
        }],
    )
    .expect("the GPU has memory for it");
}

#[test]
fn the_passes_of_one_submission_run_in_the_order_they_were_recorded() {
    let gpu = gpu();
    let size = 8;
    let first = gpu
        .create_render_target("first", size, size, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let second = gpu
        .create_render_target("second", size, size, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let positions = fullscreen_pipeline(
        &gpu,
        "positions",
        "@fragment
         fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
             return vec4<f32>(p.x, p.y, -2.5, 1.0);
         }",
        &[],
        TextureFormat::Rgba16Float,
    );
    let seven = fullscreen_pipeline(
        &gpu,
        "seven",
        "@fragment
         fn fs_main() -> @location(0) vec4<f32> {
             return vec4<f32>(7.0);
         }",
        &[],
        TextureFormat::Rgba16Float,
    );
    let layout = gpu
        .create_bind_group_layout(
            "first",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::Texture,
            }],
        )
        .expect("the GPU has memory for it");
    let group = gpu
        .create_bind_group(
            "first",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::RenderTarget(&first),
            }],
        )
        .expect("the GPU has memory for it");
    // Reads the first target mirrored left to right, texel by texel, and doubles it.
    let mirror = fullscreen_pipeline(
        &gpu,
        "mirror",
        "@group(0) @binding(0) var first: texture_2d<f32>;
         @fragment
         fn fs_main(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
             return textureLoad(first, vec2<u32>(7u - u32(p.x), u32(p.y)), 0) * 2.0;
         }",
        &[&layout],
        TextureFormat::Rgba16Float,
    );
    let pass = |target| RenderPassDescriptor {
        target,
        clear: Some(LinearColor::BLACK),
        depth: None,
        timer: None,
    };
    // Three passes in one submission: the second reads what the first drew, before the third draws
    // over it.
    gpu.submit("three passes", |commands| {
        commands.render(&pass(&first), |pass| {
            pass.set_pipeline(&positions);
            pass.draw(0..3);
        });
        commands.render(&pass(&second), |pass| {
            pass.set_pipeline(&mirror);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3);
        });
        commands.render(&pass(&first), |pass| {
            pass.set_pipeline(&seven);
            pass.draw(0..3);
        });
    });
    let mirrored = gpu
        .read_float_render_target(&second)
        .expect("the image is read back");
    let overdrawn = gpu
        .read_float_render_target(&first)
        .expect("the image is read back");
    for y in 0..size {
        for x in 0..size {
            assert_eq!(
                pixel(&mirrored, x, y),
                [15.0 - 2.0 * x as f32, 2.0 * y as f32 + 1.0, -5.0, 2.0],
                "pixel ({x}, {y}) of the second pass"
            );
            assert_eq!(
                pixel(&overdrawn, x, y),
                [7.0; 4],
                "pixel ({x}, {y}) of the third"
            );
        }
    }
}
