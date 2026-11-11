//! The device of each configuration, the screens' textures, targets and their read-back, the GPU
//! timer, and one timed frame of a layout.

use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::canvas::{Canvas, ImageId};

/// Every layout draws into this format, the colours encoded as sRGB before they mix: GL cannot view
/// a target in another format.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How the device is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Config {
    /// Vulkan, with the adapter's limits.
    Vulkan,
    /// Vulkan, with WebGL2's limits.
    VulkanWebgl2,
    /// wgpu's GL backend, without a window, with WebGL2's limits.
    Gl,
}

impl Config {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "vulkan" => Some(Self::Vulkan),
            "vulkan-webgl2" => Some(Self::VulkanWebgl2),
            "gl" => Some(Self::Gl),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::VulkanWebgl2 => "vulkan-webgl2",
            Self::Gl => "gl",
        }
    }
}

pub struct Gpu {
    /// The configuration's name, `-es` added on GL when the context is OpenGL ES's.
    pub label: String,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    /// The limits the device was made with.
    pub limits: wgpu::Limits,
    /// The errors that no error scope caught, which the uncaptured-error handler keeps.
    errors: Arc<Mutex<Vec<String>>>,
}

/// WebGL2's limits as wgpu gives them, with the adapter's sizes of textures, as a WebGL2 program
/// asks for them so that its drawing buffer may be larger than 2048 pixels (wgpu-types 30.0.1,
/// `Limits::using_resolution`: "the swapchain might need to be larger than any other image"), and
/// the adapter's alignments.
pub fn webgl2_limits(adapter: &wgpu::Limits) -> wgpu::Limits {
    wgpu::Limits::downlevel_webgl2_defaults()
        .using_resolution(adapter.clone())
        .using_alignment(adapter.clone())
}

/// Opens the device of `config`. Unless `any_adapter`, refuses an adapter that is not Mesa's
/// llvmpipe (lavapipe on Vulkan, llvmpipe on GL), so that the prototype never runs on a GPU by
/// mistake.
pub fn open(config: Config, any_adapter: bool) -> Result<Gpu, String> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = match config {
        Config::Vulkan | Config::VulkanWebgl2 => wgpu::Backends::VULKAN,
        Config::Gl => wgpu::Backends::GL,
    };
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|error| format!("no adapter: {error}"))?;
    let info = adapter.get_info();
    if !any_adapter && !info.name.contains("llvmpipe") {
        return Err(format!(
            "the adapter is `{}`, not Mesa's llvmpipe: set the variables of the README, or pass \
             --any-adapter to run on it",
            info.name
        ));
    }
    let adapter_limits = adapter.limits();
    let limits = match config {
        Config::Vulkan => adapter_limits,
        Config::VulkanWebgl2 | Config::Gl => webgl2_limits(&adapter_limits),
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("canvas prototype"),
        // Nothing but the timer: no optional feature that WebGL2 lacks.
        required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
        required_limits: limits.clone(),
        ..Default::default()
    }))
    .map_err(|error| format!("no device: {error}"))?;
    let errors = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&errors);
    device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
        sink.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(error.to_string());
    }));
    // wgpu takes desktop GL whenever EGL offers it; Mesa's version overrides make it take OpenGL
    // ES, whose GLSL ES 3.00 is WebGL2's (README).
    let es = info.backend == wgpu::Backend::Gl && info.driver_info.contains("OpenGL ES");
    Ok(Gpu {
        label: format!("{}{}", config.name(), if es { "-es" } else { "" }),
        device,
        queue,
        info,
        limits,
        errors,
    })
}

impl Gpu {
    /// Runs `create`, and gives wgpu's validation error if it raised one.
    pub fn catch<T>(&self, create: impl FnOnce() -> T) -> (T, Option<String>) {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let made = create();
        let error = pollster::block_on(scope.pop()).map(|error| error.to_string());
        (made, error)
    }

    /// An error if wgpu reported one that no scope caught since the last check.
    pub fn check(&self) -> Result<(), String> {
        let mut errors = self
            .errors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut *errors).join("\n"))
        }
    }

    pub fn can_time(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    fn wait(&self) -> Result<(), String> {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map(|_| ())
            .map_err(|error| format!("poll: {error}"))
    }

    pub fn buffer(&self, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
}

/// The screens' images, as `benches/src/render.rs` makes them, and a white texel for the items
/// that read none: index 0 is the blank, then the glyphs, the icons and the 16 pictures.
pub struct Images {
    pub views: Vec<wgpu::TextureView>,
    pub sampler: wgpu::Sampler,
}

pub const BLANK: usize = 0;
pub const GLYPHS: ImageId = ImageId(1);
pub const ICONS: ImageId = ImageId(2);
pub fn pictures() -> Vec<ImageId> {
    (3..19).map(ImageId).collect()
}

/// The index in [`Images::views`] of the image a run reads.
pub fn view_index(image: Option<ImageId>) -> usize {
    image.map_or(BLANK, |id| id.0 as usize)
}

impl Images {
    pub fn new(gpu: &Gpu) -> Self {
        let blank = texture(
            gpu,
            "blank",
            1,
            1,
            wgpu::TextureFormat::Rgba8Unorm,
            &[255; 4],
        );
        // An atlas of glyphs, 1024 × 1024 texels, in cells of 16 × 16 that a glyph covers in part.
        let glyphs: Vec<u8> = (0..1024 * 1024)
            .map(|index| {
                let (x, y) = (index % 16, (index / 1024) % 16);
                if (3..13).contains(&x) && (2..14).contains(&y) {
                    255
                } else {
                    0
                }
            })
            .collect();
        let glyphs = texture(
            gpu,
            "glyphs",
            1024,
            1024,
            wgpu::TextureFormat::R8Unorm,
            &glyphs,
        );
        let icons: Vec<u8> = (0..512 * 512)
            .flat_map(|texel| {
                let (x, y) = (texel % 512, texel / 512);
                [
                    (x % 64 * 4) as u8,
                    (y % 64 * 4) as u8,
                    ((x / 64 + y / 64 * 8) * 4) as u8,
                    255,
                ]
            })
            .collect();
        let icons = texture(
            gpu,
            "icons",
            512,
            512,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            &icons,
        );
        let mut views = vec![blank, glyphs, icons];
        for index in 0..16u8 {
            let pixels: Vec<u8> = (0..256 * 256)
                .flat_map(|texel| [(texel % 256) as u8, index * 16, (texel / 256) as u8, 255])
                .collect();
            views.push(texture(
                gpu,
                "picture",
                256,
                256,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                &pixels,
            ));
        }
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("canvas"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self { views, sampler }
    }
}

fn texture(
    gpu: &Gpu,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    texels: &[u8],
) -> wgpu::TextureView {
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let bytes_per_texel = (texels.len() / (width * height) as usize) as u32;
    gpu.queue.write_texture(
        texture.as_image_copy(),
        texels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * bytes_per_texel),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// The image bind group layout that S, I's image pipelines and T share: a filterable texture and
/// its sampler, read by the fragment shader.
pub fn image_layout(gpu: &Gpu) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("canvas image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
}

/// A bind group of `layout` ([`image_layout`]) for each image of `images`, made once.
pub fn image_groups(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    images: &Images,
) -> Vec<wgpu::BindGroup> {
    let views: Vec<&wgpu::TextureView> = images.views.iter().collect();
    image_groups_of(gpu, layout, &views, &images.sampler)
}

/// A bind group of `layout` ([`image_layout`]) for each of `views`, read through `sampler`.
pub fn image_groups_of(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    views: &[&wgpu::TextureView],
    sampler: &wgpu::Sampler,
) -> Vec<wgpu::BindGroup> {
    views
        .iter()
        .map(|view| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("canvas image"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        })
        .collect()
}

/// A uniform's bind group layout entry.
pub fn uniform_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    size: u64,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(size),
        },
        count: None,
    }
}

/// The frame's uniform, as `canvas.wgsl` declares it: the target's size, then 1, colours encoded
/// as sRGB before they mix, on every layout.
pub fn frame_bytes((width, height): (u32, u32)) -> [u8; 16] {
    let mut bytes = [0; 16];
    for (at, value) in [width as f32, height as f32, 1.0, 0.0].iter().enumerate() {
        bytes[at * 4..at * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

/// The effects' frame uniform (`fx_common.wgsl`'s `Frame`): the target's size, 1, colours encoded
/// as sRGB before they mix; then, for T, the first texels of the transforms and of the segments in
/// its texture.
pub fn fx_frame_bytes(
    (width, height): (u32, u32),
    transforms_at: u32,
    segments_at: u32,
) -> [u8; 32] {
    let mut bytes = [0; 32];
    for (at, value) in [width as f32, height as f32, 1.0].iter().enumerate() {
        bytes[at * 4..at * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[12..16].copy_from_slice(&transforms_at.to_le_bytes());
    bytes[16..20].copy_from_slice(&segments_at.to_le_bytes());
    bytes
}

/// The size of the effects' frame uniform.
pub const FX_FRAME_BYTES: u64 = 32;

/// A buffer that grows by powers of two, from `least` bytes; true if it was made anew, so that
/// the bind groups that hold it must be made again.
pub fn ensure_buffer(
    gpu: &Gpu,
    slot: &mut Option<(wgpu::Buffer, u64)>,
    label: &str,
    needed: u64,
    least: u64,
    usage: wgpu::BufferUsages,
) -> bool {
    let size = needed.next_power_of_two().max(least);
    if slot.as_ref().is_some_and(|(_, held)| *held >= size) {
        return false;
    }
    *slot = Some((gpu.buffer(label, size, usage), size));
    true
}

/// The width, in texels, of the textures that hold variable data: WebGL2's largest.
pub const DATA_WIDTH: u32 = 2048;
/// The bytes of one texel of `Rgba32Uint` or `Rgba32Float`.
pub const DATA_TEXEL: usize = 16;

/// A texture for variable data, [`DATA_WIDTH`] texels wide, `height` rows.
pub fn data_texture(
    gpu: &Gpu,
    label: &str,
    format: wgpu::TextureFormat,
    height: u32,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: DATA_WIDTH,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// The rows a texture of variable data needs for `texels`: a power of two, at least 16.
pub fn data_rows(texels: usize) -> u32 {
    (texels.div_ceil(DATA_WIDTH as usize).max(1) as u32)
        .next_power_of_two()
        .max(16)
}

/// Writes `bytes`, whole texels, into `texture` from its texel `first`, counted row after row: the
/// part of a first row, the full rows in one write, and the start of a last row. No padding is
/// written.
pub fn write_texels(gpu: &Gpu, texture: &wgpu::Texture, first: usize, bytes: &[u8]) {
    let width = DATA_WIDTH as usize;
    let write = |texel: usize, bytes: &[u8], columns: usize, rows: usize| {
        if columns == 0 || rows == 0 {
            return;
        }
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: (texel % width) as u32,
                    y: (texel / width) as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((width * DATA_TEXEL) as u32),
                rows_per_image: Some(rows as u32),
            },
            wgpu::Extent3d {
                width: columns as u32,
                height: rows as u32,
                depth_or_array_layers: 1,
            },
        );
    };
    let count = bytes.len() / DATA_TEXEL;
    let mut texel = first;
    let mut done = 0;
    // The rest of the first row.
    if texel % width != 0 {
        let columns = (width - texel % width).min(count);
        write(texel, &bytes[..columns * DATA_TEXEL], columns, 1);
        texel += columns;
        done += columns;
    }
    let rows = (count - done) / width;
    write(
        texel,
        &bytes[done * DATA_TEXEL..(done + rows * width) * DATA_TEXEL],
        width,
        rows,
    );
    texel += rows * width;
    done += rows * width;
    write(
        texel,
        &bytes[done * DATA_TEXEL..count * DATA_TEXEL],
        count - done,
        1,
    );
}

/// The transforms' uniform of I and T: as many affine transforms of 32 bytes as WebGL2's 16 KiB
/// of uniform hold; the first is the identity, which every item uses for now.
pub const TRANSFORMS: u64 = 512;
pub const TRANSFORM_BYTES: u64 = 32;

pub fn transforms_buffer(gpu: &Gpu) -> wgpu::Buffer {
    let buffer = gpu.buffer(
        "transforms",
        TRANSFORMS * TRANSFORM_BYTES,
        wgpu::BufferUsages::UNIFORM,
    );
    let mut identity = Vec::new();
    crate::canvas::push_f32s(&mut identity, &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    gpu.queue.write_buffer(&buffer, 0, &identity);
    buffer
}

/// A render pipeline's description common to the layouts: triangles, no culling, premultiplied
/// blending into [`TARGET_FORMAT`].
pub fn pipeline(
    gpu: &Gpu,
    label: &str,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    vertex_entry: &str,
    fragment_entry: &str,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some(vertex_entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some(fragment_entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
}

/// What a layout does with a frame's canvas.
pub trait Layout {
    /// Turns `canvas`'s items into the layout's bytes, writes them, and plans the draws for a
    /// target of `size`; gives the bytes written.
    fn upload(&mut self, gpu: &Gpu, canvas: &Canvas, size: (u32, u32)) -> u64;
    /// Records the draws planned; gives the draws and the pipelines set.
    fn encode(&self, pass: &mut wgpu::RenderPass<'_>) -> (usize, usize);
}

/// A layout of the screens of `render.canvas` that can also write one item alone: the growing
/// screen's moved box (`effects.rs`).
pub trait Rewrite: Layout {
    /// Writes the bytes of the item `index` of `canvas` alone, the others and the draws planned
    /// kept from the last [`Layout::upload`] of a canvas of the same kinds and images in the same
    /// order; gives the bytes written.
    fn rewrite(&mut self, gpu: &Gpu, canvas: &Canvas, index: usize) -> u64;
}

/// A layout of the effects' pipelines, whose images include textures that the effects draw.
pub trait FxLayout: Layout {
    /// The images that items name by [`ImageId`], in place of the screens' table.
    fn set_images(&mut self, gpu: &Gpu, views: &[&wgpu::TextureView], sampler: &wgpu::Sampler);
}

/// The colour targets are cleared to: whole steps of 1/255 (51, 102, 153), so that every backend
/// stores the same bytes.
pub const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.2,
    g: 0.4,
    b: 0.6,
    a: 1.0,
};

/// A target that the canvas draws into, and that is read back.
pub struct Target {
    pub texture: wgpu::Texture,
    /// The view drawn into, of [`TARGET_FORMAT`].
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
}

impl Target {
    /// A target of [`TARGET_FORMAT`]; if `srgb`, an `Rgba8UnormSrgb` one seen through an
    /// `Rgba8Unorm` view, as `slime-render` draws into a window's image today.
    pub fn new(gpu: &Gpu, (width, height): (u32, u32), srgb: bool) -> Self {
        let format = if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            TARGET_FORMAT
        };
        let view_formats: &[wgpu::TextureFormat] = if srgb { &[TARGET_FORMAT] } else { &[] };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats,
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(TARGET_FORMAT),
            ..Default::default()
        });
        Self {
            texture,
            view,
            size: (width, height),
        }
    }

    /// A target that allows nothing but being drawn into, as a GL surface: what the blur's
    /// frames are timed on. It cannot be read back.
    pub fn surface(gpu: &Gpu, (width, height): (u32, u32)) -> Self {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("surface"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            size: (width, height),
        }
    }

    /// Fills it with an opaque colour whose components are whole steps of 1/255 (51, 102, 153), so
    /// that every backend stores the same bytes.
    pub fn clear(&self, gpu: &Gpu) -> Result<(), String> {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        gpu.queue.submit([encoder.finish()]);
        gpu.wait()?;
        gpu.check()
    }

    /// Its bytes, four per pixel, row after row from the top.
    pub fn read(&self, gpu: &Gpu) -> Result<Vec<u8>, String> {
        let (width, height) = self.size;
        let row = width * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            self.texture.size(),
        );
        gpu.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        gpu.wait()?;
        receiver
            .recv()
            .map_err(|error| error.to_string())?
            .map_err(|error| format!("readback: {error}"))?;
        let mapped = buffer
            .get_mapped_range(..)
            .map_err(|error| format!("readback: {error}"))?;
        let mut pixels = Vec::with_capacity((row * height) as usize);
        for line in mapped.chunks(padded as usize).take(height as usize) {
            pixels.extend_from_slice(&line[..row as usize]);
        }
        drop(mapped);
        buffer.unmap();
        gpu.check()?;
        Ok(pixels)
    }
}

/// Times render passes with two timestamps each, read back once the frame is waited for.
pub struct Timer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    period: f32,
    /// The most passes a frame may time.
    passes: u32,
}

impl Timer {
    /// A timer of one pass, or none where the device has no timestamp queries.
    pub fn new(gpu: &Gpu) -> Option<Self> {
        Self::with_passes(gpu, 1)
    }

    /// A timer of up to `passes` passes a frame.
    pub fn with_passes(gpu: &Gpu, passes: u32) -> Option<Self> {
        if !gpu.can_time() {
            return None;
        }
        let size = 2 * u64::from(passes) * u64::from(wgpu::QUERY_SIZE);
        Some(Self {
            passes,
            queries: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("canvas"),
                ty: wgpu::QueryType::Timestamp,
                count: 2 * passes,
            }),
            resolve: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamps"),
                size,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamps readback"),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }),
            period: gpu.queue.get_timestamp_period(),
        })
    }

    fn writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        self.writes_of(0)
    }

    /// The timestamps of the pass `pass` of a frame.
    fn writes_of(&self, pass: u32) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(2 * pass),
            end_of_pass_write_index: Some(2 * pass + 1),
        }
    }

    fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        self.resolve_passes(encoder, 1);
    }

    fn resolve_passes(&self, encoder: &mut wgpu::CommandEncoder, passes: u32) {
        encoder.resolve_query_set(&self.queries, 0..2 * passes, &self.resolve, 0);
        let bytes = 2 * u64::from(passes) * u64::from(wgpu::QUERY_SIZE);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, bytes);
    }

    fn map(&self) -> mpsc::Receiver<Result<(), wgpu::BufferAsyncError>> {
        let (sender, receiver) = mpsc::channel();
        self.readback
            .map_async(wgpu::MapMode::Read, .., move |result| {
                let _ = sender.send(result);
            });
        receiver
    }

    /// The pass's time, once its submission is done and the read-back mapped.
    fn read(
        &self,
        mapped: &mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    ) -> Result<Duration, String> {
        Ok(self.read_passes(mapped, 1)?[0])
    }

    /// The times of the first `passes` passes.
    fn read_passes(
        &self,
        mapped: &mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
        passes: u32,
    ) -> Result<Vec<Duration>, String> {
        mapped
            .recv()
            .map_err(|error| error.to_string())?
            .map_err(|error| format!("timestamps: {error}"))?;
        let view = self
            .readback
            .get_mapped_range(..)
            .map_err(|error| format!("timestamps: {error}"))?;
        let stamp = |at: usize| u64::from_le_bytes(view[at..at + 8].try_into().expect("8 bytes"));
        let times = (0..passes as usize)
            .map(|pass| {
                let ticks = stamp(16 * pass + 8).saturating_sub(stamp(16 * pass));
                Duration::from_nanos((ticks as f64 * f64::from(self.period)) as u64)
            })
            .collect();
        drop(view);
        self.readback.unmap();
        Ok(times)
    }
}

/// Records a frame of several render passes in one command buffer, each timed where the device
/// has timestamps, then submits it and waits for it.
pub struct Recorder<'a> {
    gpu: &'a Gpu,
    timer: Option<&'a Timer>,
    encoder: wgpu::CommandEncoder,
    start: Instant,
    /// The names of the passes recorded, in their order.
    pub names: Vec<String>,
    pub draws: usize,
    pub pipelines: usize,
}

/// What a frame recorded by a [`Recorder`] cost.
#[derive(Clone, Debug)]
pub struct Recorded {
    /// CPU: from the recorder's start to the end of the submission.
    pub encode: Duration,
    /// From just before the submission to the end of the wait for it.
    pub wall: Duration,
    /// GPU: each pass, from its timestamps; none without timestamp queries.
    pub passes: Vec<Option<Duration>>,
    pub names: Vec<String>,
    pub draws: usize,
    pub pipelines: usize,
}

impl<'a> Recorder<'a> {
    pub fn new(gpu: &'a Gpu, timer: Option<&'a Timer>) -> Self {
        Self {
            gpu,
            timer,
            encoder: gpu.device.create_command_encoder(&Default::default()),
            start: Instant::now(),
            names: Vec::new(),
            draws: 0,
            pipelines: 0,
        }
    }

    /// Records a pass named `name` into `view`, which `load` begins with; `record` records its
    /// draws and gives the draws and the pipelines set.
    pub fn pass(
        &mut self,
        name: &str,
        view: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
        record: impl FnOnce(&mut wgpu::RenderPass<'_>) -> (usize, usize),
    ) {
        let index = self.names.len() as u32;
        let timestamps = self.timer.and_then(|timer| {
            assert!(
                index < timer.passes,
                "the timer holds {} passes",
                timer.passes
            );
            Some(timer.writes_of(index))
        });
        let mut pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(name),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timestamps,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let (draws, pipelines) = record(&mut pass);
        self.draws += draws;
        self.pipelines += pipelines;
        self.names.push(name.to_owned());
    }

    /// Submits the frame and waits for it.
    pub fn finish(self) -> Result<Recorded, String> {
        let Self {
            gpu,
            timer,
            mut encoder,
            start,
            names,
            draws,
            pipelines,
        } = self;
        let passes = names.len() as u32;
        if let Some(timer) = timer {
            timer.resolve_passes(&mut encoder, passes);
        }
        let commands = encoder.finish();
        let submitting = Instant::now();
        gpu.queue.submit([commands]);
        let submitted = Instant::now();
        let mapped = timer.map(Timer::map);
        gpu.wait()?;
        let waited = Instant::now();
        let times = match (timer, mapped) {
            (Some(timer), Some(mapped)) => timer
                .read_passes(&mapped, passes)?
                .into_iter()
                .map(Some)
                .collect(),
            _ => vec![None; names.len()],
        };
        gpu.check()?;
        Ok(Recorded {
            encode: submitted - start,
            wall: waited - submitting,
            passes: times,
            names,
            draws,
            pipelines,
        })
    }
}

/// What one frame of a layout cost, and what it drew.
#[derive(Clone, Copy, Debug)]
pub struct FrameCost {
    /// CPU: the items turned into the layout's bytes and written.
    pub write: Duration,
    /// CPU: the pass encoded, and submitted.
    pub encode: Duration,
    /// GPU: the pass, from its timestamps; none without timestamp queries.
    pub gpu: Option<Duration>,
    /// From just before the submission to the end of the wait for it.
    pub wall: Duration,
    pub bytes: u64,
    pub draws: usize,
    pub pipelines: usize,
}

/// Draws `canvas` once with `layout` over what `target` holds, and waits for it.
pub fn frame(
    gpu: &Gpu,
    layout: &mut dyn Layout,
    canvas: &Canvas,
    target: &Target,
    timer: Option<&Timer>,
) -> Result<FrameCost, String> {
    let start = Instant::now();
    let bytes = layout.upload(gpu, canvas, target.size);
    let written = Instant::now();
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let (draws, pipelines) = {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("canvas"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timer.map(Timer::writes),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        layout.encode(&mut pass)
    };
    if let Some(timer) = timer {
        timer.resolve(&mut encoder);
    }
    let commands = encoder.finish();
    let submitting = Instant::now();
    gpu.queue.submit([commands]);
    let submitted = Instant::now();
    let mapped = timer.map(Timer::map);
    gpu.wait()?;
    let waited = Instant::now();
    let gpu_time = match (timer, mapped) {
        (Some(timer), Some(mapped)) => Some(timer.read(&mapped)?),
        _ => None,
    };
    gpu.check()?;
    Ok(FrameCost {
        write: written - start,
        encode: submitted - written,
        gpu: gpu_time,
        wall: waited - submitting,
        bytes,
        draws,
        pipelines,
    })
}
