//! Depth targets: textures holding the depth of what was drawn, for depth testing.

use crate::gpu::Gpu;

/// The format of every depth target: 32-bit floating-point depth, without stencil. The same on
/// every driver, which image tests prefer to the driver-dependent precision of `Depth24Plus`.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A texture holding the depth of each pixel drawn, with which a render pass keeps the nearest of
/// the surfaces drawn at a pixel whatever their order
/// ([`RenderPassDescriptor::depth`](crate::RenderPassDescriptor::depth)). It cannot be read back.
///
/// Clones refer to the same texture, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct DepthTarget {
    pub(crate) view: wgpu::TextureView,
    pub(crate) label: String,
    width: u32,
    height: u32,
}

impl DepthTarget {
    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Gpu {
    /// Creates a depth target of `width` × `height` pixels, the size of the render targets it is
    /// used with. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Panics
    ///
    /// If `width` or `height` is zero or larger than the device allows (8192 with the default
    /// limits).
    pub fn create_depth_target(&self, label: &str, width: u32, height: u32) -> DepthTarget {
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
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        // The view keeps the texture alive.
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        DepthTarget {
            view,
            label: label.to_owned(),
            width,
            height,
        }
    }
}
