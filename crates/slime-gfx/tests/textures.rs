//! Sampling textures: nearest and linear filtering, the address modes, set on their own for each
//! direction of a sampler, a magnified texture read with the magnification filter rather than the
//! minification one, and the decoding of sRGB formats, checked by reading pixels back. Runs on
//! whatever adapter the machine offers, like `device.rs`.

use slime_gfx::{
    AddressMode, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource,
    BindingType, CullMode, Filter, FragmentState, Gpu, Image, LinearColor, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, RenderTarget, Sampler, SamplerDescriptor,
    ShaderStages, Texture, TextureFormat, VertexAttribute, VertexFormat, VertexLayout,
};

/// Draws the texture at group 0 with the texture coordinates of the vertices.
const SHADER: &str = "
@group(0) @binding(0) var color_texture: texture_2d<f32>;
@group(0) @binding(1) var color_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(position, 0.0, 1.0);
    output.uv = uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(color_texture, color_sampler, input.uv);
}
";

const SIZE: u32 = 8;
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

struct Setup {
    gpu: Gpu,
    target: RenderTarget,
    layout: BindGroupLayout,
    pipeline: RenderPipeline,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let target = gpu
            .create_render_target("textures", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let layout = gpu
            .create_bind_group_layout(
                "texture and sampler",
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
        let shader = gpu
            .create_shader_module("textures", SHADER)
            .expect("the shader compiles");
        let pipeline = gpu
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: "textures",
                shader: &shader,
                vertex_entry_point: "vs_main",
                vertex_layout: Some(VertexLayout {
                    stride: 4 * 4,
                    attributes: &[
                        VertexAttribute {
                            location: 0,
                            offset: 0,
                            format: VertexFormat::Float32x2,
                        },
                        VertexAttribute {
                            location: 1,
                            offset: 2 * 4,
                            format: VertexFormat::Float32x2,
                        },
                    ],
                }),
                fragment: Some(FragmentState {
                    entry_point: "fs_main",
                    target_format: Some(target.format()),
                }),
                bind_group_layouts: &[&layout],
                depth: None,
                cull: CullMode::None,
            })
            .expect("the pipeline is valid");
        Self {
            gpu,
            target,
            layout,
            pipeline,
        }
    }

    /// A 2 × 2 texture of `format`: red and green on the top row, blue and white below.
    fn two_by_two(&self, format: TextureFormat) -> Texture {
        let pixels: Vec<u8> = [RED, GREEN, BLUE, WHITE].concat();
        self.gpu
            .create_texture("two by two", 2, 2, format, &pixels)
            .expect("the GPU has memory for it")
    }

    fn sampler(&self, filter: Filter, address: AddressMode) -> Sampler {
        self.gpu
            .create_sampler(
                "sampler",
                &SamplerDescriptor {
                    mag_filter: filter,
                    min_filter: filter,
                    address_u: address,
                    address_v: address,
                },
            )
            .expect("the GPU has memory for it")
    }

    /// Draws `texture` over the whole target with `sampler`, the texture coordinates running
    /// from (0, 0) at the top left to `uv_max` at the bottom right, and reads the image back.
    fn draw(&self, texture: &Texture, sampler: &Sampler, uv_max: [f32; 2]) -> Image {
        let [u, v] = uv_max;
        let corners: [[f32; 4]; 4] = [
            [-1.0, 1.0, 0.0, 0.0],
            [-1.0, -1.0, 0.0, v],
            [1.0, -1.0, u, v],
            [1.0, 1.0, u, 0.0],
        ];
        let bytes: Vec<u8> = corners
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let vertices = self
            .gpu
            .create_vertex_buffer("quad", &bytes)
            .expect("the GPU has memory for it");
        let indices = self
            .gpu
            .create_index_buffer("quad", &[0, 1, 2, 0, 2, 3])
            .expect("the GPU has memory for it");
        let group = self
            .gpu
            .create_bind_group(
                "texture and sampler",
                &self.layout,
                &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Texture(texture),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(sampler),
                    },
                ],
            )
            .expect("the GPU has memory for it");
        let pass = RenderPassDescriptor {
            target: &self.target,
            clear: Some(LinearColor::BLACK),
            depth: None,
            timer: None,
        };
        self.gpu.render(&pass, |pass| {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.set_vertex_buffer(0, &vertices);
            pass.set_index_buffer(&indices);
            pass.draw_indexed(0..6);
        });
        self.gpu
            .read_render_target(&self.target)
            .expect("the image is read back")
    }
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

fn rgb(color: [u8; 4]) -> [u8; 3] {
    [color[0], color[1], color[2]]
}

#[test]
fn nearest_filtering_reads_whole_pixels() {
    let setup = Setup::new();
    let texture = setup.two_by_two(TextureFormat::Rgba8UnormSrgb);
    let sampler = setup.sampler(Filter::Nearest, AddressMode::ClampToEdge);
    let image = setup.draw(&texture, &sampler, [1.0, 1.0]);
    // Each pixel of the texture covers a quarter of the target.
    assert_eq!(pixel(&image, (1, 1)), rgb(RED));
    assert_eq!(pixel(&image, (6, 1)), rgb(GREEN));
    assert_eq!(pixel(&image, (1, 6)), rgb(BLUE));
    assert_eq!(pixel(&image, (6, 6)), rgb(WHITE));
}

#[test]
fn linear_filtering_blends_the_nearest_pixels() {
    let setup = Setup::new();
    // A linear format, so that the blend happens on the bytes as they are.
    let texture = setup.two_by_two(TextureFormat::Rgba8Unorm);
    let sampler = setup.sampler(Filter::Linear, AddressMode::ClampToEdge);
    let image = setup.draw(&texture, &sampler, [1.0, 1.0]);
    // The center of pixel (4, 4) is at 0.5625 of the texture in both directions, 0.625 of the
    // way from the first pixel of the texture to the second: 0.375 of red and green on the top
    // row, 0.625 of blue and white below, each row blended in the same proportions. The target
    // then encodes the blend to sRGB.
    let expected = [0.375 * 0.375 + 0.625 * 0.625, 0.625, 0.625].map(srgb);
    for (channel, expected) in pixel(&image, (4, 4)).into_iter().zip(expected) {
        assert!(
            channel.abs_diff(expected) <= 3,
            "expected about {expected}, got {channel}"
        );
    }
}

/// The sRGB encoding of a linear value, out of 255.
fn srgb(linear: f32) -> u8 {
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

#[test]
fn address_modes_say_what_lies_beyond_the_texture() {
    let setup = Setup::new();
    let texture = setup.two_by_two(TextureFormat::Rgba8UnormSrgb);
    // Horizontally, the texture coordinates run from 0 to 2 across the target: the pixels at
    // x = 5 and x = 7 read 1.375 and 1.875, beyond the texture. Vertically they stay in its top
    // row.
    for (address, expected, case) in [
        (
            AddressMode::Repeat,
            [RED, GREEN],
            "repeat: 0.375 then 0.875",
        ),
        (
            AddressMode::ClampToEdge,
            [GREEN, GREEN],
            "clamp: the right edge",
        ),
        (
            AddressMode::MirrorRepeat,
            [GREEN, RED],
            "mirror: 0.625 then 0.125",
        ),
    ] {
        let sampler = setup.sampler(Filter::Nearest, address);
        let image = setup.draw(&texture, &sampler, [2.0, 1.0]);
        assert_eq!(pixel(&image, (5, 1)), rgb(expected[0]), "{case}");
        assert_eq!(pixel(&image, (7, 1)), rgb(expected[1]), "{case}");
    }
}

#[test]
fn each_direction_of_a_sampler_is_its_own_and_magnifying_uses_the_mag_filter() {
    let setup = Setup::new();
    let texture = setup.two_by_two(TextureFormat::Rgba8UnormSrgb);

    // Every field different from its pair. The texture is magnified, two target pixels per
    // texture pixel, and pixel (5, 5) reads (1.375, 1.375): repeated horizontally, 0.375, the
    // left column; clamped vertically, the bottom row: blue. Directions swapped would read green,
    // and filters swapped, a linear blend of blue and white. Nothing is minified, so the
    // minification filter is not read.
    let sampler = setup
        .gpu
        .create_sampler(
            "repeat u, clamp v",
            &SamplerDescriptor {
                mag_filter: Filter::Nearest,
                min_filter: Filter::Linear,
                address_u: AddressMode::Repeat,
                address_v: AddressMode::ClampToEdge,
            },
        )
        .expect("the GPU has memory for it");
    let image = setup.draw(&texture, &sampler, [2.0, 2.0]);
    assert_eq!(pixel(&image, (5, 5)), rgb(BLUE));

    // Mirrored vertically: pixels (1, 5) and (1, 7) read 1.375 and 1.875, mirrored to 0.625 and
    // 0.125: the bottom row, then the top one.
    let sampler = setup
        .gpu
        .create_sampler(
            "clamp u, mirror v",
            &SamplerDescriptor {
                mag_filter: Filter::Nearest,
                min_filter: Filter::Nearest,
                address_u: AddressMode::ClampToEdge,
                address_v: AddressMode::MirrorRepeat,
            },
        )
        .expect("the GPU has memory for it");
    let image = setup.draw(&texture, &sampler, [1.0, 2.0]);
    assert_eq!(pixel(&image, (1, 5)), rgb(BLUE));
    assert_eq!(pixel(&image, (1, 7)), rgb(RED));
}

#[test]
fn srgb_textures_are_decoded_and_linear_ones_are_read_as_they_are() {
    let setup = Setup::new();
    let sampler = setup.sampler(Filter::Nearest, AddressMode::ClampToEdge);
    // The same byte, 128, read through both formats and written to the sRGB target: decoded then
    // encoded again, it comes back as 128; read as the linear value 0.502, it is encoded to 188.
    for (format, expected) in [
        (TextureFormat::Rgba8UnormSrgb, 128),
        (TextureFormat::Rgba8Unorm, 188),
    ] {
        let texture = setup
            .gpu
            .create_texture("grey", 1, 1, format, &[128, 128, 128, 255])
            .expect("the GPU has memory for it");
        let image = setup.draw(&texture, &sampler, [1.0, 1.0]);
        let [red, ..] = pixel(&image, (4, 4));
        assert!(
            red.abs_diff(expected) <= 1,
            "{format:?}: expected about {expected}, got {red}"
        );
    }
}

#[test]
fn minified_textures_are_read_from_their_smaller_levels() {
    let setup = Setup::new();
    // Black and white pixels alternating, in a linear format: every level but the first is grey,
    // 128, which the sRGB target encodes as 188.
    let mut pixels = Vec::new();
    for index in 0..16 {
        let value = if (index + index / 4) % 2 == 0 { 0 } else { 255 };
        pixels.extend([value, value, value, 255]);
    }
    let texture = setup
        .gpu
        .create_texture("checker", 4, 4, TextureFormat::Rgba8Unorm, &pixels)
        .expect("the GPU has memory for it");
    let sampler = setup.sampler(Filter::Linear, AddressMode::Repeat);
    // The texture repeats six times across the eight pixels of the target: each pixel covers three
    // of its pixels in each direction, so levels 1 and 2 (2 × 2 and 1 × 1 pixels, all grey) are
    // read and blended. The center of each pixel of the target falls on the center of one pixel
    // of the texture (at 3x + 1.5, wrapped to 0..4), so the first level alone would give pure
    // black or white: black at (0, 0), (4, 4) and (7, 7), white at (1, 0).
    let image = setup.draw(&texture, &sampler, [6.0, 6.0]);
    for position in [(0, 0), (1, 0), (4, 4), (7, 7)] {
        let [red, ..] = pixel(&image, position);
        assert!(
            red.abs_diff(188) <= 2,
            "{position:?}: expected grey, got {red}"
        );
    }
}
