//! Textures that render passes draw into.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::error::GfxError;
use crate::gpu::Gpu;

/// The samples per pixel of a multisampled target ([`Gpu::create_multisampled_render_target`],
/// [`Gpu::create_multisampled_depth_target`]): four, which every driver supports for every format
/// of render targets. Drivers place them at the points of the standard pattern of Vulkan and
/// Direct3D, one in each quarter of the pixel's width and one in each quarter of its height, which
/// the engine's tests check.
pub const MULTISAMPLE_COUNT: u32 = 4;

/// The formats of the engine's textures: those it renders into, and those shaders sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureFormat {
    /// Red, green, blue and alpha, 8 bits each, with sRGB encoding: shaders write and read linear
    /// colors, which the GPU encodes and decodes, as image files and displays expect.
    Rgba8UnormSrgb,
    /// The same channels and encoding in another byte order: blue, green, red, then alpha. Most
    /// windows use it.
    Bgra8UnormSrgb,
    /// Red, green, blue and alpha, 8 bits each, linear: for data that is not a color, such as a
    /// normal map, which shaders read as it is.
    Rgba8Unorm,
    /// Red, green, blue and alpha, each a 16-bit (half-precision) floating-point number, linear:
    /// values beyond 0 to 1, up to ±65 504 with 11 significant bits, such as the light of a scene
    /// before tone mapping (high dynamic range); a shader that writes more gets ±65 504 or an
    /// infinity, depending on the driver. Render targets of this format are sampled by shaders
    /// and read back by [`Gpu::read_float_render_target`]; textures of it are created from
    /// levels the caller gives ([`Gpu::create_texture_with_levels`],
    /// [`Gpu::create_cube_texture`]), not by [`Gpu::create_texture`], whose levels are computed
    /// from 8-bit pixels.
    Rgba16Float,
}

impl TextureFormat {
    /// Whether the format is sRGB-encoded, so that shaders write and read linear values.
    pub fn is_srgb(self) -> bool {
        matches!(self, Self::Rgba8UnormSrgb | Self::Bgra8UnormSrgb)
    }

    /// The size of a texel in bytes: 4, or 8 for `Rgba16Float`.
    pub(crate) fn texel_size(self) -> u32 {
        match self {
            Self::Rgba16Float => 8,
            Self::Rgba8UnormSrgb | Self::Bgra8UnormSrgb | Self::Rgba8Unorm => 4,
        }
    }

    /// The wgpu format, for the engine's tooling only, through the `wgpu` feature (ADR-0031).
    #[cfg(feature = "wgpu")]
    pub fn wgpu_format(self) -> wgpu::TextureFormat {
        self.to_wgpu()
    }

    pub(crate) fn to_wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            Self::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        }
    }

    /// The engine's name for a wgpu format, if the engine supports it.
    pub(crate) fn from_wgpu(format: wgpu::TextureFormat) -> Option<Self> {
        match format {
            wgpu::TextureFormat::Rgba8UnormSrgb => Some(Self::Rgba8UnormSrgb),
            wgpu::TextureFormat::Bgra8UnormSrgb => Some(Self::Bgra8UnormSrgb),
            wgpu::TextureFormat::Rgba8Unorm => Some(Self::Rgba8Unorm),
            wgpu::TextureFormat::Rgba16Float => Some(Self::Rgba16Float),
            _ => None,
        }
    }
}

/// A texture that render passes draw into: an offscreen image ([`Gpu::create_render_target`]),
/// which can be read back and sampled by later passes; the image of a window for one frame
/// ([`SurfaceFrame::target`](crate::SurfaceFrame::target)), which can be neither; or an offscreen
/// image of several samples per pixel ([`Gpu::create_multisampled_render_target`]), which can be
/// neither either, and which a pass resolves into an ordinary one.
///
/// Clones refer to the same texture, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct RenderTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) label: String,
    format: TextureFormat,
    /// Whether a render pass drew into the texture; set by [`Gpu::render`], shared by the clones.
    pub(crate) drawn: Arc<AtomicBool>,
}

impl RenderTarget {
    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.texture.width()
    }

    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.texture.height()
    }

    /// The format of the pixels.
    pub fn format(&self) -> TextureFormat {
        self.format
    }

    /// The label given at creation. It names the target in error messages and, in GPU capture
    /// tools, the render passes drawn into it and their command buffers, and also the texture of a
    /// target from [`Gpu::create_render_target`], but not a window's image.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Gpu {
    /// Creates a render target of `width` × `height` pixels. `label` names it in error messages
    /// and in GPU capture tools.
    ///
    /// Shaders of later passes sample it through a bind group
    /// ([`BindingResource::RenderTarget`](crate::BindingResource::RenderTarget) at a
    /// [`BindingType::Texture`](crate::BindingType::Texture) binding); a pass cannot sample the
    /// target it draws into. Until a pass draws into it, it reads as zeros.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If `width` or `height` is zero or larger than the device allows (8192 with the default
    /// limits).
    pub fn create_render_target(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<RenderTarget, GfxError> {
        self.create_target(
            label,
            width,
            height,
            format,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
        )
    }

    /// Creates a render target of `width` × `height` pixels of [`MULTISAMPLE_COUNT`] samples each,
    /// for multisample anti-aliasing: a pipeline built for it
    /// ([`RenderPipelineDescriptor::multisampled`](crate::RenderPipelineDescriptor::multisampled))
    /// tests whether a triangle covers each sample, and shades the pixel once for the samples it
    /// covers. A pass then resolves it
    /// into a render target of one sample per pixel, of its size and format
    /// ([`RenderPassDescriptor::resolve`](crate::RenderPassDescriptor::resolve)): each pixel the
    /// average of its samples, so that an edge across it shows the share of the pixel it covers.
    /// `label` names it in error messages and in GPU capture tools.
    ///
    /// Passes only draw into it: it can be neither read back nor sampled, as a window's image
    /// cannot. It takes [`MULTISAMPLE_COUNT`] times the memory of a target of one sample.
    ///
    /// # Errors
    ///
    /// As [`Gpu::create_render_target`].
    ///
    /// # Panics
    ///
    /// As [`Gpu::create_render_target`].
    pub fn create_multisampled_render_target(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<RenderTarget, GfxError> {
        self.create_target(
            label,
            width,
            height,
            format,
            MULTISAMPLE_COUNT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
    }

    /// Creates the texture of a render target and its view.
    fn create_target(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
        sample_count: u32,
        usage: wgpu::TextureUsages,
    ) -> Result<RenderTarget, GfxError> {
        let texture = self.create(label, || {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: format.to_wgpu(),
                usage,
                view_formats: &[],
            })
        })?;
        Ok(RenderTarget::new(texture, label, format))
    }
}

impl RenderTarget {
    pub(crate) fn new(texture: wgpu::Texture, label: &str, format: TextureFormat) -> Self {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            label: label.to_owned(),
            format,
            drawn: Arc::new(AtomicBool::new(false)),
        }
    }
}
