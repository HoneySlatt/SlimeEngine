//! A bare wgpu device for `uwui-render`'s tests, as a host that is not the engine opens one, wgpu's
//! requests answered at once and read with a waker that does nothing
//! (`docs/adr/0244-the-engine-lends-its-device-through-a-guarded-door.md`). The host handles the
//! loss, as `uwui-render` leaves it to its host: once the device is lost, the errors that wgpu
//! raises for the objects made after the loss, which are invalid, are counted and ignored; before,
//! every error panics, as wgpu's own handler does. The tests, unlike the renderer, may poll the device
//! to read their targets back.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};

use uwui_render::DrawContext;

/// A device of the machine's most capable adapter, and its queue.
#[derive(Clone)]
pub struct Bare {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Whether the device is lost, as its callback or [`Bare::lose`] says.
    lost: Arc<AtomicBool>,
    /// The errors that wgpu raised once the device was lost.
    errors_after_loss: Arc<AtomicUsize>,
    /// Whether a texture can be viewed in another format: an sRGB one without its encoding.
    view_formats: bool,
}

/// The limits that a test's device is opened with.
#[derive(Clone, Copy, Debug)]
pub enum Limits {
    /// Every limit that the adapter offers: the canvas reads its items from a storage buffer.
    Adapter,
    /// WebGL2's, which give no storage buffer: the canvas reads its items from a texture.
    WebGl2,
}

/// An image read back from a target of floats: four channels a pixel, rows from the top.
pub struct FloatImage {
    pub width: u32,
    pub pixels: Vec<f32>,
}

/// An image read back from a target of bytes: four channels a pixel, rows from the top.
pub struct Image {
    pub pixels: Vec<u8>,
}

/// The engine's graphics API: Direct3D 12 on Windows, Vulkan elsewhere.
pub fn platform() -> wgpu::Backends {
    if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::VULKAN
    }
}

/// What `future` gives, which wgpu's native backends give before it is first polled.
fn now<F: Future>(future: F) -> F::Output {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("wgpu answers a native request at once"),
    }
}

impl Bare {
    /// Opens the most capable adapter of the engine's graphics API, Direct3D 12 on Windows and
    /// Vulkan elsewhere ([`platform`]), or of `WGPU_BACKEND`, with timestamps where it has them,
    /// and every limit it offers, or WebGL2's if `UWUI_RENDER_LIMITS` is `webgl2`
    /// ([`Limits::WebGl2`]). Direct3D 12 compiles shaders with wgpu's default compiler, where the
    /// engine looks for DXC beside its program.
    pub fn new() -> Self {
        let limits = match std::env::var("UWUI_RENDER_LIMITS").as_deref() {
            Ok("webgl2") => Limits::WebGl2,
            Ok("") | Err(_) => Limits::Adapter,
            Ok(other) => panic!("UWUI_RENDER_LIMITS is `webgl2` or unset, not `{other}`"),
        };
        Self::on(wgpu::Backends::from_env().unwrap_or(platform()), limits)
    }

    /// Opens the most capable adapter of `backends`, with `limits`, as [`Bare::new`] does. wgpu's
    /// GL backend is taken on Mesa's GL on the CPU alone, in OpenGL ES 3.0, whose shaders are
    /// WebGL2's: wgpu takes desktop GL wherever it is offered, and a GPU's driver draws its own
    /// images (`scripts/check.sh gles` asks Mesa for them).
    pub fn on(backends: wgpu::Backends, limits: Limits) -> Self {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = now(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let info = adapter.get_info();
        if info.backend == wgpu::Backend::Gl {
            assert!(
                info.device_type == wgpu::DeviceType::Cpu
                    && info.driver_info.starts_with("OpenGL ES 3.0"),
                "the tests on GL take Mesa's GL on the CPU in OpenGL ES 3.0, not {info:?}"
            );
        }
        let view_formats = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::VIEW_FORMATS);
        let (device, queue) = now(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uwui-render's tests"),
            required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
            required_limits: match limits {
                Limits::Adapter => adapter.limits(),
                Limits::WebGl2 => wgpu::Limits::downlevel_webgl2_defaults(),
            },
            ..Default::default()
        }))
        .expect("the adapter opens a device");
        let lost = Arc::new(AtomicBool::new(false));
        device.set_device_lost_callback({
            let lost = Arc::clone(&lost);
            move |_, _| lost.store(true, Ordering::Relaxed)
        });
        let errors_after_loss = Arc::new(AtomicUsize::new(0));
        device.on_uncaptured_error(Arc::new({
            let (lost, errors) = (Arc::clone(&lost), Arc::clone(&errors_after_loss));
            move |error: wgpu::Error| {
                assert!(lost.load(Ordering::Relaxed), "wgpu error: {error}");
                errors.fetch_add(1, Ordering::Relaxed);
            }
        }));
        Self {
            device,
            queue,
            lost,
            errors_after_loss,
            view_formats,
        }
    }

    /// A format of 8 bits a channel into which the canvas writes colors encoded as sRGB: sRGB's,
    /// drawn into without its encoding, where the device views a texture in another format;
    /// `Rgba8Unorm`, which holds the same bytes, where it does not, as OpenGL ES.
    pub fn encoded(&self) -> wgpu::TextureFormat {
        if self.view_formats {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        }
    }

    /// Whether the device views an sRGB target without its encoding.
    pub fn views_without_srgb(&self) -> bool {
        self.view_formats
    }

    /// Whether the device times passes.
    pub fn can_time(&self) -> bool {
        self.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    /// A target of `width` × `height` pixels of `format`, which can be viewed without its sRGB
    /// encoding, sampled and read back, cleared to opaque black.
    pub fn target(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        self.target_of(label, width, height, format, 1)
    }

    /// A target as [`Bare::target`], of `samples` samples a pixel.
    pub fn target_of(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> wgpu::Texture {
        let unencoded = format.remove_srgb_suffix();
        let mut usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        if samples == 1 {
            usage |= wgpu::TextureUsages::COPY_SRC;
        }
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: if unencoded == format {
                &[]
            } else {
                std::slice::from_ref(&unencoded)
            },
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view(&target),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.queue.submit([encoder.finish()]);
        target
    }

    /// A texture of `width` × `height` texels of `format`, holding `texels`, row after row.
    pub fn texture(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        texels: &[u8],
    ) -> wgpu::Texture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: extent(width, height),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texel = format.block_copy_size(None).expect("a format of texels");
        self.queue.write_texture(
            texture.as_image_copy(),
            texels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * texel),
                rows_per_image: Some(height),
            },
            extent(width, height),
        );
        texture
    }

    /// Records what `record` draws into an encoder, then submits it, as a host does.
    pub fn record<T>(&self, record: impl FnOnce(&mut DrawContext<'_>) -> T) -> T {
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let drawn = record(&mut DrawContext {
            device: &self.device,
            queue: &self.queue,
            encoder: &mut encoder,
        });
        self.queue.submit([encoder.finish()]);
        drawn
    }

    /// Waits until the GPU has finished what was submitted.
    pub fn wait_idle(&self) {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device answers");
    }

    /// Loses the device, as a driver reset does.
    pub fn lose(&self) {
        self.lost.store(true, Ordering::Relaxed);
        self.device.destroy();
    }

    /// How many errors wgpu raised once the device was lost: none from what `uwui-render` does
    /// with objects made before the loss.
    pub fn errors_after_loss(&self) -> usize {
        self.errors_after_loss.load(Ordering::Relaxed)
    }

    /// The pixels of `target`, of 8 bytes a pixel, `Rgba16Float`, as floats.
    pub fn read_floats(&self, target: &wgpu::Texture) -> FloatImage {
        assert_eq!(target.format(), wgpu::TextureFormat::Rgba16Float);
        let bytes = self.read(target, 8);
        let pixels = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&half| f16_to_f32(u16::from_le_bytes(half)))
            .collect();
        FloatImage {
            width: target.width(),
            pixels,
        }
    }

    /// The bytes of `target`'s pixels, of 4 or 8 bytes each, row after row.
    pub fn read_bytes_of(&self, target: &wgpu::Texture) -> Vec<u8> {
        let texel = target
            .format()
            .block_copy_size(None)
            .expect("a format of texels");
        self.read(target, texel)
    }

    /// The pixels of `target`, of 4 bytes a pixel.
    pub fn read_bytes(&self, target: &wgpu::Texture) -> Image {
        Image {
            pixels: self.read(target, 4),
        }
    }

    /// The bytes of `target`'s pixels, of `texel` bytes each, row after row without padding.
    fn read(&self, target: &wgpu::Texture, texel: u32) -> Vec<u8> {
        let (width, height) = (target.width(), target.height());
        let row = width * texel;
        let padded = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            extent(width, height),
        );
        encoder.map_buffer_on_submit(&buffer, wgpu::MapMode::Read, .., |result| {
            result.expect("the readback is mapped");
        });
        self.queue.submit([encoder.finish()]);
        self.wait_idle();
        let mapped = buffer.get_mapped_range(..).expect("mapped");
        mapped
            .chunks_exact(padded as usize)
            .flat_map(|line| &line[..row as usize])
            .copied()
            .collect()
    }
}

/// The view of the whole of `texture`, in its format.
pub fn view(texture: &wgpu::Texture) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

/// `half`, an IEEE 754 binary16 value, as an `f32`.
fn f16_to_f32(half: u16) -> f32 {
    let sign = if half & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((half >> 10) & 0x1f);
    let fraction = f32::from(half & 0x3ff);
    sign * match exponent {
        0 => fraction * 2f32.powi(-24),
        31 if fraction == 0.0 => f32::INFINITY,
        31 => f32::NAN,
        _ => (1.0 + fraction / 1024.0) * 2f32.powi(exponent - 15),
    }
}
