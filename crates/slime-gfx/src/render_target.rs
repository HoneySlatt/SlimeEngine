//! Textures that render passes draw into.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::gpu::Gpu;

/// The formats of the engine's textures: those it renders into, and those shaders sample.
/// `Rgba16Float` is a format of render targets only.
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
    /// infinity, depending on the driver. A format of render targets only: shaders sample
    /// them, [`Gpu::read_float_render_target`] reads them back, and [`Gpu::create_texture`] does
    /// not take it.
    Rgba16Float,
}

impl TextureFormat {
    /// Whether the format is sRGB-encoded, so that shaders write and read linear values.
    pub fn is_srgb(self) -> bool {
        matches!(self, Self::Rgba8UnormSrgb | Self::Bgra8UnormSrgb)
    }

    /// The wgpu format, for the engine's tooling only (ADR-0031).
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
/// which can be read back and sampled by later passes, or the image of a window for one frame
/// ([`SurfaceFrame::target`](crate::SurfaceFrame::target)), which can be neither.
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

    /// The label given at creation, which names the target in error messages and in GPU capture
    /// tools.
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
    ) -> RenderTarget {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.to_wgpu(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        RenderTarget::new(texture, label, format)
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
