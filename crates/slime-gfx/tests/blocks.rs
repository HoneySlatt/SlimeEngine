//! Textures of the block-compressed formats, BC7 and BC5: blocks encoded on the CPU and read back
//! from every level of the texture as `textureLoad` gives them, compared with the CPU's decoding of
//! the same blocks. Runs on whatever adapter the machine offers, like `device.rs`; the checks run
//! it on lavapipe and CI on WARP, so that every driver the project tests samples them.

use block_compression::decode::decompress_blocks_as_rgba8;
use block_compression::encode::compress_rgba8;
use block_compression::{BC7Settings, CompressionVariant};
use slime_color::LinearColor;
use slime_gfx::{
    BindGroupEntry, BindGroupLayoutEntry, BindingResource, BindingType, CullMode, FragmentState,
    Gpu, Image, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, Texture,
    TextureFormat,
};

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// A level of RGBA8 texels: its size and its bytes.
struct Level {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// `width` × `height` texels of colors and alphas that change along both directions, in steps that
/// differ in each channel, so that every block holds several.
fn pattern(width: u32, height: u32) -> Level {
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            pixels.extend([
                (x * 37 + y * 11) as u8,
                (x * 5 + y * 29 + 40) as u8,
                (x * 13 + y * 71 + 9) as u8,
                (128 + x * 7 + y * 19) as u8,
            ]);
        }
    }
    Level {
        width,
        height,
        pixels,
    }
}

/// `level`'s texels repeated at its right and bottom edges up to whole blocks of 4 × 4.
fn padded(level: &Level) -> Level {
    let (width, height) = (
        level.width.next_multiple_of(4),
        level.height.next_multiple_of(4),
    );
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let start =
                4 * (y.min(level.height - 1) * level.width + x.min(level.width - 1)) as usize;
            pixels.extend_from_slice(&level.pixels[start..start + 4]);
        }
    }
    Level {
        width,
        height,
        pixels,
    }
}

/// The blocks of `level` in `variant`, and the texels that decoding them gives back, cropped to the
/// level's size.
fn encode(variant: CompressionVariant, level: &Level) -> (Vec<u8>, Vec<u8>) {
    let full = padded(level);
    let mut blocks = vec![0; variant.blocks_byte_size(full.width, full.height)];
    compress_rgba8(
        variant,
        &full.pixels,
        &mut blocks,
        full.width,
        full.height,
        full.width * 4,
    );
    let mut decoded = vec![0; full.pixels.len()];
    decompress_blocks_as_rgba8(variant, full.width, full.height, &blocks, &mut decoded);
    let mut cropped = Vec::new();
    for y in 0..level.height {
        let start = 4 * (y * full.width) as usize;
        cropped.extend_from_slice(&decoded[start..start + 4 * level.width as usize]);
    }
    (blocks, cropped)
}

/// What `textureLoad` reads from `texture` at each texel of its level `level`, of `width` ×
/// `height` texels, written into a target of `format`.
fn load(
    gpu: &Gpu,
    texture: &Texture,
    level: u32,
    (width, height): (u32, u32),
    format: TextureFormat,
) -> Image {
    let shader = format!(
        "@group(0) @binding(0) var source: texture_2d<f32>;

         @vertex
         fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
             return vec4<f32>(f32((i & 1u) * 4u) - 1.0, f32((i & 2u) * 2u) - 1.0, 0.0, 1.0);
         }}

         @fragment
         fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {{
             return textureLoad(source, vec2<i32>(position.xy), {level});
         }}"
    );
    let module = gpu
        .create_shader_module("blocks", &shader)
        .expect("the shader compiles");
    let layout = gpu
        .create_bind_group_layout(
            "blocks",
            &[BindGroupLayoutEntry {
                binding: 0,
                visible_to: ShaderStages::Fragment,
                ty: BindingType::Texture,
            }],
        )
        .expect("the GPU has memory for it");
    let target = gpu
        .create_render_target("blocks", width, height, format)
        .expect("the GPU has memory for it");
    let pipeline = gpu
        .create_render_pipeline(&RenderPipelineDescriptor {
            label: "blocks",
            shader: &module,
            vertex_entry_point: "vs_main",
            vertex_layout: None,
            fragment: Some(FragmentState {
                entry_point: "fs_main",
                target_format: Some(format),
                blend: None,
            }),
            bind_group_layouts: &[&layout],
            depth: None,
            cull: CullMode::None,
            multisampled: false,
        })
        .expect("the pipeline is valid");
    let group = gpu
        .create_bind_group(
            "blocks",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Texture(texture),
            }],
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
    gpu.read_render_target(&target)
        .expect("the image is read back")
}

/// The levels of a 16 × 12 texture down to one texel: 16 × 12, 8 × 6, 4 × 3, 2 × 1 and 1 × 1, the
/// last three smaller than a block in one direction or both.
fn levels() -> Vec<Level> {
    (0..5)
        .map(|index| pattern((16 >> index).max(1), (12 >> index).max(1)))
        .collect()
}

/// Creates a 16 × 12 texture of `format` from the blocks of [`levels`] in `variant`, reads every
/// level back into a target of `target_format`, and checks each channel among `channels` against
/// the CPU's decoding of the blocks, within `tolerance`. Returns the texels read back.
fn check(
    format: TextureFormat,
    variant: CompressionVariant,
    target_format: TextureFormat,
    channels: std::ops::Range<usize>,
    tolerance: u8,
) -> Vec<Image> {
    let gpu = gpu();
    assert!(
        gpu.can_sample_bc(),
        "{} cannot sample BC textures, which every driver the project tests can",
        gpu.adapter_name()
    );
    let levels = levels();
    let encoded: Vec<(Vec<u8>, Vec<u8>)> =
        levels.iter().map(|level| encode(variant, level)).collect();
    let blocks: Vec<&[u8]> = encoded
        .iter()
        .map(|(blocks, _)| blocks.as_slice())
        .collect();
    let texture = gpu
        .create_texture_with_levels("blocks", 16, 12, format, &blocks)
        .expect("the GPU has memory for it");
    let mut images = Vec::new();
    for (index, (level, (_, expected))) in levels.iter().zip(&encoded).enumerate() {
        let image = load(
            &gpu,
            &texture,
            index as u32,
            (level.width, level.height),
            target_format,
        );
        for (texel, (got, want)) in image.pixels.chunks(4).zip(expected.chunks(4)).enumerate() {
            for channel in channels.clone() {
                assert!(
                    got[channel].abs_diff(want[channel]) <= tolerance,
                    "{format:?}, level {index}, texel {texel}, channel {channel}: {} instead of {}",
                    got[channel],
                    want[channel]
                );
            }
        }
        images.push(image);
    }
    images
}

#[test]
fn bc7_textures_read_back_as_their_blocks_decode_at_every_level() {
    check(
        TextureFormat::Bc7RgbaUnorm,
        CompressionVariant::BC7(BC7Settings::alpha_basic()),
        TextureFormat::Rgba8Unorm,
        0..4,
        0,
    );
}

#[test]
fn bc7_srgb_textures_are_decoded_to_linear_values() {
    // Read back through an sRGB target, which encodes again what the shader read decoded: the
    // blocks' values come back, within a step of rounding.
    check(
        TextureFormat::Bc7RgbaUnormSrgb,
        CompressionVariant::BC7(BC7Settings::alpha_basic()),
        TextureFormat::Rgba8UnormSrgb,
        0..4,
        1,
    );
}

#[test]
fn bc5_textures_read_back_their_two_channels_with_blue_0_and_alpha_1() {
    // The CPU decoder interpolates between a block's two values with an integer division, which
    // drops up to 6/7 of a step; GPUs interpolate in more precision and round: up to 2 steps
    // apart, as lavapipe and RADV give them.
    let images = check(
        TextureFormat::Bc5RgUnorm,
        CompressionVariant::BC5,
        TextureFormat::Rgba8Unorm,
        0..2,
        2,
    );
    for image in &images {
        for texel in image.pixels.chunks(4) {
            assert_eq!((texel[2], texel[3]), (0, 255));
        }
    }
}

#[test]
#[should_panic(expected = "a whole number of blocks")]
fn a_compressed_texture_whose_side_is_not_a_multiple_of_four_is_refused() {
    let gpu = gpu();
    let _ = gpu.create_texture_with_levels("odd", 6, 4, TextureFormat::Bc7RgbaUnorm, &[&[0; 32]]);
}
