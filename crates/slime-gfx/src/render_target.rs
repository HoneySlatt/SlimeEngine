//! Textures that render passes draw into.

use crate::gpu::Gpu;

/// The formats of the textures the engine renders into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureFormat {
    /// Red, green, blue and alpha, 8 bits each, with sRGB encoding: shaders write linear colors and
    /// the GPU encodes them, as image files and displays expect.
    Rgba8UnormSrgb,
}

impl TextureFormat {
    pub(crate) fn to_wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

/// A texture that render passes draw into, and whose content can be read back to the CPU.
///
/// Clones refer to the same texture, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct RenderTarget {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) label: String,
    format: TextureFormat,
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
}

impl Gpu {
    /// Creates a render target of `width` × `height` pixels. `label` names it in error messages
    /// and in GPU capture tools.
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        RenderTarget {
            texture,
            view,
            label: label.to_owned(),
            format,
        }
    }
}
