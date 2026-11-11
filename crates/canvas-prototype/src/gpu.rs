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
    /// wgpu's GL backend, without a window, with WebGL2's limits: the context wgpu chooses.
    Gl,
    /// As `Gl`, refused unless the context is OpenGL ES's, whose GLSL ES 3.00 is WebGL2's.
    GlEs,
}

impl Config {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "vulkan" => Some(Self::Vulkan),
            "vulkan-webgl2" => Some(Self::VulkanWebgl2),
            "gl" => Some(Self::Gl),
            "gl-es" => Some(Self::GlEs),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::VulkanWebgl2 => "vulkan-webgl2",
            Self::Gl => "gl",
            Self::GlEs => "gl-es",
        }
    }
}

pub struct Gpu {
    /// The configuration's name, `-gpu` added with `--any-adapter`: what each table's row, and
    /// each PNG's name, begins with.
    pub label: String,
    /// The label of the run whose images of S are the reference: `vulkan`, or `vulkan-gpu` with
    /// `--any-adapter`, so that a run on a GPU never reads nor writes lavapipe's.
    pub reference: String,
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
/// mistake. Refuses a context that is not OpenGL ES's on `gl-es`.
pub fn open(config: Config, any_adapter: bool) -> Result<Gpu, String> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = match config {
        Config::Vulkan | Config::VulkanWebgl2 => wgpu::Backends::VULKAN,
        Config::Gl | Config::GlEs => wgpu::Backends::GL,
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
    // wgpu takes desktop GL whenever EGL offers it; Mesa's version overrides of `run-config.sh`
    // make it take OpenGL ES, and without them a run named `gl-es` would measure desktop GL.
    if config == Config::GlEs && !info.driver_info.contains("OpenGL ES") {
        return Err(format!(
            "gl-es: the context is `{}`, not OpenGL ES: run it through run-config.sh, which sets \
             Mesa's version overrides",
            info.driver_info.trim()
        ));
    }
    let adapter_limits = adapter.limits();
    let limits = match config {
        Config::Vulkan => adapter_limits,
        Config::VulkanWebgl2 | Config::Gl | Config::GlEs => webgl2_limits(&adapter_limits),
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("canvas prototype"),
        // Nothing but the timer's: no optional feature that WebGL2 lacks and a layout could use.
        required_features: adapter.features()
            & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
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
    let suffix = if any_adapter { "-gpu" } else { "" };
    Ok(Gpu {
        label: format!("{}{suffix}", config.name()),
        reference: format!("{}{suffix}", Config::Vulkan.name()),
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

    /// Whether a timestamp can be written between two commands of an encoder, outside passes.
    pub fn times_in_encoders(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
    }

    /// The first column's header of every table: the configuration, and the adapter it ran on.
    pub fn config_header(&self) -> String {
        format!("config, on {}", self.info.name)
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

/// The bytes a frame writes into buffers and textures, staged in a buffer of the layout's own that
/// the CPU maps, then copied by commands that open the frame's encoder, after the timestamp that
/// starts `gpu with copies`. `Queue::write_buffer` and `write_texture` would copy them in commands
/// of wgpu's own, submitted before the encoder's, where no timestamp of the frame reaches them.
///
/// Each frame: the layout stages its bytes ([`Uploads::write_buffer`], [`Uploads::write_texels`]),
/// [`Uploads::flush`] copies them into the staging buffer and unmaps it, the [`Recorder`] records
/// the copies and, once the frame is submitted, maps the buffer again, ready when the frame is.
#[derive(Default)]
pub struct Uploads {
    staging: Option<Staging>,
    /// The bytes staged this frame, each with its offset in the staging buffer.
    chunks: Vec<(u64, Vec<u8>)>,
    /// The end of the bytes staged.
    end: u64,
    /// The copies of the bytes flushed, recorded by the next [`Recorder`].
    copies: Vec<Copy>,
}

/// The staging buffer, `MAP_WRITE | COPY_SRC`: memory the CPU writes and the GPU reads, as
/// wgpu's own staging is.
struct Staging {
    buffer: wgpu::Buffer,
    size: u64,
    mapping: Mapping,
}

enum Mapping {
    /// Mapped, at creation or by a map that completed.
    Mapped,
    /// A map asked for once its frame was submitted.
    Pending(mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>),
    Unmapped,
}

/// A copy out of the staging buffer, from the offset `from`.
enum Copy {
    Buffer {
        from: u64,
        to: wgpu::Buffer,
        offset: u64,
        size: u64,
    },
    /// `columns` × `rows` texels of a texture [`DATA_WIDTH`] texels wide, from `origin`.
    Texels {
        from: u64,
        to: wgpu::Texture,
        origin: wgpu::Origin3d,
        columns: u32,
        rows: u32,
    },
}

impl Uploads {
    /// Places `bytes` in the staging buffer: each at an offset that is a multiple of 256 bytes,
    /// which a copy into a texture or a buffer accepts.
    fn stage(&mut self, bytes: Vec<u8>) -> u64 {
        let from = self
            .end
            .next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.into());
        self.end = from + bytes.len() as u64;
        self.chunks.push((from, bytes));
        from
    }

    /// Writes `bytes` into `buffer` at `offset`: a whole number of 4 bytes.
    pub fn write_buffer(&mut self, buffer: &wgpu::Buffer, offset: u64, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        let size = bytes.len() as u64;
        let from = self.stage(bytes);
        self.copies.push(Copy::Buffer {
            from,
            to: buffer.clone(),
            offset,
            size,
        });
    }

    /// Writes `bytes`, whole texels of 16 bytes, into `texture`, [`DATA_WIDTH`] texels wide, from
    /// its texel `first`, counted row after row: the rest of a first row, the full rows in one
    /// copy, and the start of a last row. No padding is written.
    pub fn write_texels(&mut self, texture: &wgpu::Texture, first: usize, bytes: Vec<u8>) {
        let count = bytes.len() / DATA_TEXEL;
        if count == 0 {
            return;
        }
        let from = self.stage(bytes);
        let width = DATA_WIDTH as usize;
        let mut copy = |done: usize, columns: usize, rows: usize| {
            if columns == 0 || rows == 0 {
                return;
            }
            let texel = first + done;
            self.copies.push(Copy::Texels {
                from: from + (done * DATA_TEXEL) as u64,
                to: texture.clone(),
                origin: wgpu::Origin3d {
                    x: (texel % width) as u32,
                    y: (texel / width) as u32,
                    z: 0,
                },
                columns: columns as u32,
                rows: rows as u32,
            });
        };
        let mut done = 0;
        if first % width != 0 {
            let columns = (width - first % width).min(count);
            copy(done, columns, 1);
            done += columns;
        }
        let rows = (count - done) / width;
        copy(done, width, rows);
        done += rows * width;
        copy(done, count - done, 1);
    }

    /// Copies the bytes staged into the staging buffer, made larger first if they need it, and
    /// unmaps it, so that the frame's copies can read it.
    pub fn flush(&mut self, gpu: &Gpu) -> Result<(), String> {
        if self.chunks.is_empty() {
            return Ok(());
        }
        let mut staging = match self.staging.take() {
            Some(staging) if staging.size >= self.end => staging,
            // A larger one, by powers of two, mapped at its creation; the old one is dropped.
            _ => {
                let size = self.end.next_power_of_two().max(1 << 16);
                Staging {
                    buffer: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("staging"),
                        size,
                        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: true,
                    }),
                    size,
                    mapping: Mapping::Mapped,
                }
            }
        };
        if let Mapping::Unmapped = staging.mapping {
            staging.request_map();
        }
        if let Mapping::Pending(receiver) = std::mem::replace(&mut staging.mapping, Mapping::Mapped)
        {
            // Mapped by the wait that ended the last frame, unless that frame was not waited for.
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(_) => {
                    gpu.wait()?;
                    receiver
                        .recv()
                        .map_err(|error| format!("staging: {error}"))?
                }
            };
            result.map_err(|error| format!("staging: {error}"))?;
        }
        {
            let mut mapped = staging
                .buffer
                .get_mapped_range_mut(..)
                .map_err(|error| format!("staging: {error}"))?;
            for (from, bytes) in self.chunks.drain(..) {
                let from = from as usize;
                mapped
                    .slice(from..from + bytes.len())
                    .copy_from_slice(&bytes);
            }
        }
        staging.buffer.unmap();
        staging.mapping = Mapping::Unmapped;
        self.staging = Some(staging);
        self.end = 0;
        Ok(())
    }

    /// Records the copies of the bytes flushed into `encoder`.
    fn record(&mut self, encoder: &mut wgpu::CommandEncoder) {
        assert!(
            self.chunks.is_empty(),
            "the uploads are flushed before their copies are recorded"
        );
        let Some(staging) = &self.staging else {
            return;
        };
        for copy in self.copies.drain(..) {
            match copy {
                Copy::Buffer {
                    from,
                    to,
                    offset,
                    size,
                } => encoder.copy_buffer_to_buffer(&staging.buffer, from, &to, offset, size),
                Copy::Texels {
                    from,
                    to,
                    origin,
                    columns,
                    rows,
                } => encoder.copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer: &staging.buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: from,
                            bytes_per_row: Some(DATA_WIDTH * DATA_TEXEL as u32),
                            rows_per_image: Some(rows),
                        },
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &to,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: columns,
                        height: rows,
                        depth_or_array_layers: 1,
                    },
                ),
            }
        }
    }

    /// Maps the staging buffer again, once the frame that copies from it is submitted: the map
    /// completes when that frame does.
    fn after_submit(&mut self) {
        if let Some(staging) = &mut self.staging
            && let Mapping::Unmapped = staging.mapping
        {
            staging.request_map();
        }
    }
}

impl Staging {
    fn request_map(&mut self) {
        let (sender, receiver) = mpsc::channel();
        self.buffer
            .map_async(wgpu::MapMode::Write, .., move |result| {
                let _ = sender.send(result);
            });
        self.mapping = Mapping::Pending(receiver);
    }
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
    /// Turns `canvas`'s items into the layout's bytes, stages their writes in `uploads`, and plans
    /// the draws for a target of `size`; gives the bytes written.
    fn upload(
        &mut self,
        gpu: &Gpu,
        uploads: &mut Uploads,
        canvas: &Canvas,
        size: (u32, u32),
    ) -> u64;
    /// Records the draws planned; gives the draws and the pipelines set.
    fn encode(&self, pass: &mut wgpu::RenderPass<'_>) -> (usize, usize);
}

/// A layout of the screens of `render.canvas` that can also write one item alone: the growing
/// screen's moved box (`effects.rs`).
pub trait Rewrite: Layout {
    /// Stages in `uploads` the bytes of the item `index` of `canvas` alone, the others and the
    /// draws planned kept from the last [`Layout::upload`] of a canvas of the same kinds and
    /// images in the same order; gives the bytes written.
    fn rewrite(&mut self, uploads: &mut Uploads, canvas: &Canvas, index: usize) -> u64;
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

/// Times a frame on the GPU: one timestamp before the frame's copies, then two around each render
/// pass; read back once the frame is waited for.
///
/// wgpu writes a timestamp once the commands before it are done, so that a pass's first
/// timestamp follows the copies, and the copies' own is the frame's first command.
pub struct Timer {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    period: f32,
    /// The most passes a frame may time.
    passes: u32,
    /// Where the device cannot write a timestamp between two commands of an encoder, a 1 × 1
    /// target of an empty render pass whose first timestamp marks the copies' start.
    marker: Option<wgpu::TextureView>,
}

/// The query of the timestamp before the frame's copies; each pass's two follow it.
const COPIES_QUERY: u32 = 0;

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
        let count = 1 + 2 * passes;
        let size = u64::from(count) * u64::from(wgpu::QUERY_SIZE);
        let marker = (!gpu.times_in_encoders()).then(|| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("copies' start"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TARGET_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        Some(Self {
            passes,
            queries: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("canvas"),
                ty: wgpu::QueryType::Timestamp,
                count,
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
            marker,
        })
    }

    /// Writes the timestamp that starts the frame's copies.
    fn mark_copies(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(marker) = &self.marker else {
            encoder.write_timestamp(&self.queries, COPIES_QUERY);
            return;
        };
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("copies' start"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: marker,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
                query_set: &self.queries,
                beginning_of_pass_write_index: Some(COPIES_QUERY),
                end_of_pass_write_index: None,
            }),
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }

    /// The timestamps of the pass `pass` of a frame.
    fn writes_of(&self, pass: u32) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(1 + 2 * pass),
            end_of_pass_write_index: Some(2 + 2 * pass),
        }
    }

    fn resolve(&self, encoder: &mut wgpu::CommandEncoder, passes: u32) {
        let count = 1 + 2 * passes;
        encoder.resolve_query_set(&self.queries, 0..count, &self.resolve, 0);
        let bytes = u64::from(count) * u64::from(wgpu::QUERY_SIZE);
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

    /// The times of the first `passes` passes, and the time from the copies' start to the last
    /// pass's end.
    fn read(
        &self,
        mapped: &mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
        passes: u32,
    ) -> Result<(Vec<Duration>, Duration), String> {
        mapped
            .recv()
            .map_err(|error| error.to_string())?
            .map_err(|error| format!("timestamps: {error}"))?;
        let view = self
            .readback
            .get_mapped_range(..)
            .map_err(|error| format!("timestamps: {error}"))?;
        let stamp = |query: u32| {
            let at = (query * wgpu::QUERY_SIZE) as usize;
            u64::from_le_bytes(view[at..at + 8].try_into().expect("8 bytes"))
        };
        let between = |first: u32, last: u32| {
            let ticks = stamp(last).saturating_sub(stamp(first));
            Duration::from_nanos((ticks as f64 * f64::from(self.period)) as u64)
        };
        let times = (0..passes)
            .map(|pass| between(1 + 2 * pass, 2 + 2 * pass))
            .collect();
        let with_copies = between(COPIES_QUERY, 2 * passes);
        drop(view);
        self.readback.unmap();
        Ok((times, with_copies))
    }
}

/// Records a frame in one command buffer, the copies of its uploads first, then its render passes,
/// each timed where the device has timestamps; then submits it and waits for it.
pub struct Recorder<'a> {
    gpu: &'a Gpu,
    timer: Option<&'a Timer>,
    uploads: &'a mut Uploads,
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
    /// CPU: from the recorder's start, the copies and passes recorded, to the command buffer
    /// finished.
    pub encode: Duration,
    /// CPU: `Queue::submit` of the frame's command buffer alone. On GL, wgpu turns the recorded
    /// commands into GL calls there.
    pub submit: Duration,
    /// From just before the submission to the end of the wait for it.
    pub wall: Duration,
    /// GPU: each pass, from its timestamps; none without timestamp queries.
    pub passes: Vec<Option<Duration>>,
    /// GPU: from the timestamp before the frame's copies to the end of its last pass; none without
    /// timestamp queries.
    pub with_copies: Option<Duration>,
    pub names: Vec<String>,
    pub draws: usize,
    pub pipelines: usize,
}

impl Recorded {
    /// The GPU's time of the frame's passes: their sum, if each was timed.
    pub fn gpu_pass(&self) -> Option<Duration> {
        self.passes.iter().copied().sum()
    }
}

impl<'a> Recorder<'a> {
    /// Starts a frame: the timestamp that starts the copies, then the copies of `uploads`, which
    /// must be flushed.
    pub fn new(gpu: &'a Gpu, timer: Option<&'a Timer>, uploads: &'a mut Uploads) -> Self {
        let start = Instant::now();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        if let Some(timer) = timer {
            timer.mark_copies(&mut encoder);
        }
        uploads.record(&mut encoder);
        Self {
            gpu,
            timer,
            uploads,
            encoder,
            start,
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
        let timestamps = self.timer.map(|timer| {
            assert!(
                index < timer.passes,
                "the timer holds {} passes",
                timer.passes
            );
            timer.writes_of(index)
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
            uploads,
            encoder,
            start,
            names,
            draws,
            pipelines,
        } = self;
        let passes = names.len() as u32;
        assert!(passes > 0, "a frame records a pass at least");
        let commands = encoder.finish();
        let encoded = Instant::now();
        gpu.queue.submit([commands]);
        let submitted = Instant::now();
        uploads.after_submit();
        // The queries are resolved by a submission of their own: on GL, wgpu reads their results
        // there, which waits for the frame to be drawn, and `submit` would hold that wait.
        let mapped = timer.map(|timer| {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            timer.resolve(&mut encoder, passes);
            gpu.queue.submit([encoder.finish()]);
            timer.map()
        });
        gpu.wait()?;
        let waited = Instant::now();
        let (times, with_copies) = match (timer, mapped) {
            (Some(timer), Some(mapped)) => {
                let (times, with_copies) = timer.read(&mapped, passes)?;
                (times.into_iter().map(Some).collect(), Some(with_copies))
            }
            _ => (vec![None; names.len()], None),
        };
        gpu.check()?;
        Ok(Recorded {
            encode: encoded - start,
            submit: submitted - encoded,
            wall: waited - encoded,
            passes: times,
            with_copies,
            names,
            draws,
            pipelines,
        })
    }
}

/// What one frame cost, and what it drew.
#[derive(Clone, Debug)]
pub struct FrameCost {
    /// CPU: the canvas's items made (the growing screen only; 0 elsewhere).
    pub build: Duration,
    /// CPU: the items turned into the layout's bytes, staged, and copied into the staging buffer.
    pub write: Duration,
    /// The bytes written: items, transforms, segments, the frame's uniform.
    pub bytes: u64,
    pub recorded: Recorded,
}

/// Draws `canvas` once with `layout` over what `target` holds, its bytes copied through
/// `uploads`, and waits for it.
pub fn frame(
    gpu: &Gpu,
    layout: &mut dyn Layout,
    uploads: &mut Uploads,
    canvas: &Canvas,
    target: &Target,
    timer: Option<&Timer>,
) -> Result<FrameCost, String> {
    let start = Instant::now();
    let bytes = layout.upload(gpu, uploads, canvas, target.size);
    uploads.flush(gpu)?;
    let write = start.elapsed();
    let mut recorder = Recorder::new(gpu, timer, uploads);
    recorder.pass("canvas", &target.view, wgpu::LoadOp::Load, |pass| {
        layout.encode(pass)
    });
    Ok(FrameCost {
        build: Duration::ZERO,
        write,
        bytes,
        recorded: recorder.finish()?,
    })
}
