//! Textures whose levels of detail the caller gives (ADR-0039): half-precision float data read back
//! exactly, each level where it was given, and cube textures whose faces lie in the directions
//! `Gpu::create_cube_texture` documents, checked by sampling every pixel of every face, with the
//! nearest filter, at the direction the documentation gives for its center. Runs on whatever
//! adapter the machine offers, like `device.rs`.

use slime_gfx::{
    BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, CullMode, Filter,
    FloatImage, FragmentState, Gpu, LinearColor, RenderPassDescriptor, RenderPipelineDescriptor,
    SamplerDescriptor, ShaderStages, Texture, TextureFormat, f32_to_f16,
};

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// `values` as the bytes of `Rgba16Float` texels.
fn halves(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|&value| f32_to_f16(value).to_le_bytes())
        .collect()
}

/// Samples `texture`, of WGSL type `texture_type` and bound at a `binding` binding, with the
/// nearest filter, once per entry of `lookups`: texture coordinates (`xy` for a two-dimensional
/// texture, `xyz`, a direction, for a cube) and, in `w`, the level of detail to read. Returns one
/// pixel per lookup, read back.
fn sample(
    gpu: &Gpu,
    texture: &Texture,
    binding: BindingType,
    texture_type: &str,
    lookups: &[[f32; 4]],
) -> FloatImage {
    let coordinates = if binding == BindingType::CubeTexture {
        "lookup.xyz"
    } else {
        "lookup.xy"
    };
    let shader = format!(
        "@group(0) @binding(0) var source: {texture_type};
         @group(0) @binding(1) var source_sampler: sampler;
         @group(0) @binding(2) var<uniform> lookups: array<vec4<f32>, {count}>;

         @vertex
         fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
             return vec4<f32>(f32((i & 1u) * 4u) - 1.0, f32((i & 2u) * 2u) - 1.0, 0.0, 1.0);
         }}

         @fragment
         fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {{
             let lookup = lookups[u32(position.x)];
             return textureSampleLevel(source, source_sampler, {coordinates}, lookup.w);
         }}",
        count = lookups.len()
    );
    let module = gpu
        .create_shader_module("levels", &shader)
        .expect("the shader compiles");
    let entry = |binding, ty| BindGroupLayoutEntry {
        binding,
        visible_to: ShaderStages::Fragment,
        ty,
    };
    let layout = gpu
        .create_bind_group_layout(
            "levels",
            &[
                entry(0, binding),
                entry(1, BindingType::Sampler),
                entry(
                    2,
                    BindingType::UniformBuffer {
                        dynamic_offset: false,
                        // One `vec4<f32>` for each lookup.
                        size: 16 * lookups.len() as u64,
                    },
                ),
            ],
        )
        .expect("the GPU has memory for it");
    let target = gpu
        .create_render_target(
            "levels",
            lookups.len() as u32,
            1,
            TextureFormat::Rgba16Float,
        )
        .expect("the GPU has memory for it");
    let pipeline = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "levels",
            shader: &module,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(target.format()),
            }),
            bind_group_layouts: &[&layout],
            depth: None,
            cull: CullMode::None,
            multisampled: false,
        })
        .expect("the pipeline is valid");
    let nearest = gpu
        .create_sampler(
            "nearest",
            &SamplerDescriptor {
                mag_filter: Filter::Nearest,
                min_filter: Filter::Nearest,
                ..SamplerDescriptor::default()
            },
        )
        .expect("the GPU has memory for it");
    let bytes: Vec<u8> = lookups
        .as_flattened()
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let uniform = gpu
        .create_uniform_buffer("lookups", &bytes)
        .expect("the GPU has memory for it");
    let group = gpu
        .create_bind_group(
            "levels",
            &layout,
            &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Texture(texture),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&nearest),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::UniformBuffer(&uniform),
                },
            ],
        )
        .expect("the GPU has memory for it");
    let pass = RenderPassDescriptor {
        target: &target,
        clear: Some(LinearColor::rgba(0.0, 0.0, 0.0, 0.0)),
        depth: None,
        timer: None,
        resolve: None,
    };
    gpu.render(&pass, |pass| {
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3);
    });
    gpu.read_float_render_target(&target)
        .expect("the image is read back")
}

#[test]
fn half_precision_levels_are_read_back_exactly_where_they_were_given() {
    let gpu = gpu();
    // 2 × 2 then 1 × 1, of values beyond 0 to 1 and of either sign, exact in half precision.
    let full: [f32; 16] = [
        1000.5, -3.25, 0.125, 1.0, // top left
        2.0, 65_504.0, -0.5, 1.0, // top right
        0.0, 6e-8, 7.75, 1.0, // bottom left
        -65_504.0, 0.25, 300.0, 1.0, // bottom right
    ];
    let small: [f32; 4] = [42.0, -1.5, 0.0625, 0.5];
    let texture = gpu
        .create_texture_with_levels(
            "two levels",
            2,
            2,
            TextureFormat::Rgba16Float,
            &[&halves(&full), &halves(&small)],
        )
        .expect("the GPU has memory for it");
    let lookups = [
        [0.25, 0.25, 0.0, 0.0],
        [0.75, 0.25, 0.0, 0.0],
        [0.25, 0.75, 0.0, 0.0],
        [0.75, 0.75, 0.0, 0.0],
        [0.5, 0.5, 0.0, 1.0],
    ];
    let image = sample(
        &gpu,
        &texture,
        BindingType::Texture,
        "texture_2d<f32>",
        &lookups,
    );
    // f32_to_f16 rounds 6e-8 to the subnormal 2^-24, which comes back as it is.
    let mut expected = full.to_vec();
    expected[9] = 1.0 / 16_777_216.0;
    expected.extend(small);
    assert_eq!(image.pixels, expected);
}

/// The unit-free direction of the point at column `s` and row `t` (0 to 1) of cube face `face`,
/// from the table of `Gpu::create_cube_texture`.
fn documented_direction(face: usize, s: f32, t: f32) -> [f32; 3] {
    let (sc, tc) = (2.0 * s - 1.0, 2.0 * t - 1.0);
    match face {
        0 => [1.0, -tc, -sc],
        1 => [-1.0, -tc, sc],
        2 => [sc, 1.0, tc],
        3 => [sc, -1.0, -tc],
        4 => [sc, -tc, 1.0],
        _ => [-sc, -tc, -1.0],
    }
}

#[test]
fn each_pixel_of_a_cube_lies_in_the_documented_direction() {
    let gpu = gpu();
    // Faces of 4 × 4 pixels, then 2 × 2: each pixel holds its face, column, row and level.
    let level = |size: usize, index: f32| -> Vec<u8> {
        let mut values = Vec::new();
        for face in 0..6 {
            for row in 0..size {
                for column in 0..size {
                    values.extend([face as f32, column as f32, row as f32, index]);
                }
            }
        }
        halves(&values)
    };
    let texture = gpu
        .create_cube_texture(
            "cube",
            4,
            TextureFormat::Rgba16Float,
            &[&level(4, 0.0), &level(2, 1.0)],
        )
        .expect("the GPU has memory for it");
    let mut lookups = Vec::new();
    let mut expected = Vec::new();
    for (size, index) in [(4, 0.0), (2, 1.0)] {
        for face in 0..6 {
            for row in 0..size {
                for column in 0..size {
                    let center = |i: usize| (i as f32 + 0.5) / size as f32;
                    let [x, y, z] = documented_direction(face, center(column), center(row));
                    lookups.push([x, y, z, index]);
                    expected.extend([face as f32, column as f32, row as f32, index]);
                }
            }
        }
    }
    let image = sample(
        &gpu,
        &texture,
        BindingType::CubeTexture,
        "texture_cube<f32>",
        &lookups,
    );
    for (pixel, (got, wanted)) in image.pixels.chunks(4).zip(expected.chunks(4)).enumerate() {
        assert_eq!(got, wanted, "lookup {pixel}: {:?}", lookups[pixel]);
    }
}

#[test]
#[should_panic(expected = "needs 32 bytes, but 24 were given")]
fn a_level_of_the_wrong_size_is_refused() {
    let gpu = gpu();
    gpu.create_texture_with_levels("short", 2, 2, TextureFormat::Rgba16Float, &[&[0; 24]])
        .expect("the GPU has memory for it");
}

#[test]
#[should_panic(expected = "takes 1 to 2 levels, but 3 were given")]
fn more_levels_than_down_to_one_pixel_are_refused() {
    let gpu = gpu();
    gpu.create_cube_texture(
        "deep",
        2,
        TextureFormat::Rgba8Unorm,
        &[&[0; 96], &[0; 24], &[0; 24]],
    )
    .expect("the GPU has memory for it");
}
