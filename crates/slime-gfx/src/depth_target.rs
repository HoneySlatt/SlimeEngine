//! Depth targets: textures holding the depth of what was drawn, for depth testing and for later
//! passes to sample.

use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::render_target::MULTISAMPLE_COUNT;

/// The format of every depth target: 32-bit floating-point depth, without stencil. The same on
/// every driver, which image tests prefer to the driver-dependent precision of `Depth24Plus`.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A texture holding the depth of each pixel drawn. A render pass keeps the nearest surface at
/// each pixel with it, whatever the order the surfaces are drawn in
/// ([`RenderPassDescriptor::depth`](crate::RenderPassDescriptor::depth)), or writes depth only
/// into it ([`Gpu::render_depth`]). Shaders of later passes read it through a bind group
/// ([`BindingResource::DepthTarget`](crate::BindingResource::DepthTarget) at a
/// [`BindingType::DepthTexture`](crate::BindingType::DepthTexture) binding, `texture_depth_2d` in
/// WGSL): texel by texel with `textureLoad`, or compared with a reference depth through a
/// [`ComparisonSampler`](crate::ComparisonSampler), as a shadow map is read. A pass cannot read
/// the depth target it draws into. Until a pass draws into it, it holds depth 0, the farthest
/// (depth is reversed, ADR-0047): a comparison sampler then finds every reference depth in front
/// of it. It cannot be read back to the CPU.
///
/// A depth target of several samples per pixel ([`Gpu::create_multisampled_depth_target`]) holds a
/// depth for each sample, for the passes that draw into a multisampled render target: passes only
/// test and write it, and no shader reads it.
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
    /// Creates a depth target of `width` × `height` pixels: the size of the render target of the
    /// passes that test depth with it ([`Gpu::render`] asserts it), or any size for a depth-only
    /// pass ([`Gpu::render_depth`]). `label` names it in error messages and in GPU capture tools.
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
    pub fn create_depth_target(
        &self,
        label: &str,
        width: u32,
        height: u32,
    ) -> Result<DepthTarget, GfxError> {
        self.create_depth(
            label,
            width,
            height,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        )
    }

    /// Creates a depth target of `width` × `height` pixels of [`MULTISAMPLE_COUNT`] samples each,
    /// the size of the multisampled render target of the passes that test depth with it. Passes
    /// test and write a depth for each sample; no shader can read it. `label` names it in error
    /// messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// As [`Gpu::create_depth_target`].
    ///
    /// # Panics
    ///
    /// As [`Gpu::create_depth_target`].
    pub fn create_multisampled_depth_target(
        &self,
        label: &str,
        width: u32,
        height: u32,
    ) -> Result<DepthTarget, GfxError> {
        self.create_depth(
            label,
            width,
            height,
            MULTISAMPLE_COUNT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
    }

    /// Creates the texture of a depth target and its view.
    fn create_depth(
        &self,
        label: &str,
        width: u32,
        height: u32,
        sample_count: u32,
        usage: wgpu::TextureUsages,
    ) -> Result<DepthTarget, GfxError> {
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
                format: DEPTH_FORMAT,
                usage,
                view_formats: &[],
            })
        })?;
        // The view keeps the texture alive.
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Ok(DepthTarget {
            view,
            label: label.to_owned(),
            width,
            height,
        })
    }
}
