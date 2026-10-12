//! Textures that shaders sample.

use crate::gpu::Gpu;
use crate::render_target::TextureFormat;

/// A two-dimensional texture that shaders sample through a bind group
/// ([`BindingResource::Texture`](crate::BindingResource::Texture)), with a [`Sampler`](crate::Sampler).
/// It has one level of detail: no mipmaps.
///
/// Clones refer to the same texture, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct Texture {
    pub(crate) view: wgpu::TextureView,
    width: u32,
    height: u32,
    format: TextureFormat,
}

impl Texture {
    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The format of the pixels.
    pub fn format(&self) -> TextureFormat {
        self.format
    }
}

impl Gpu {
    /// Creates a texture of `width` × `height` pixels holding `pixels`: rows from top to bottom,
    /// 4 bytes per pixel in the byte order of `format` (red, green, blue, alpha for the RGBA
    /// formats). Shaders read sRGB formats decoded to linear values, and linear formats as they
    /// are. `label` names the texture in error messages and in GPU capture tools.
    ///
    /// # Panics
    ///
    /// If `width` or `height` is zero or larger than the device allows (8192 with the default
    /// limits), or if `pixels` does not hold `width * height * 4` bytes.
    pub fn create_texture(
        &self,
        label: &str,
        width: u32,
        height: u32,
        format: TextureFormat,
        pixels: &[u8],
    ) -> Texture {
        let expected = width as usize * height as usize * 4;
        assert!(
            pixels.len() == expected,
            "texture `{label}` is {width}x{height} and needs {expected} bytes of pixels, but {} \
             were given",
            pixels.len()
        );
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: format.to_wgpu(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Unlike copies recorded in a command encoder, this write needs no row alignment.
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        // The view keeps the texture alive.
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Texture {
            view,
            width,
            height,
            format,
        }
    }
}
