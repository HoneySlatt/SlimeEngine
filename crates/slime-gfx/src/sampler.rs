//! Samplers: how shaders read textures.

use crate::gpu::Gpu;

/// How a texture is read between its pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    /// The nearest pixel: sharp, and blocky when the texture is magnified.
    Nearest,
    /// A blend of the four nearest pixels: smooth. The default.
    #[default]
    Linear,
}

impl Filter {
    fn to_wgpu(self) -> wgpu::FilterMode {
        match self {
            Self::Nearest => wgpu::FilterMode::Nearest,
            Self::Linear => wgpu::FilterMode::Linear,
        }
    }
}

/// What a texture coordinate outside 0 to 1 reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AddressMode {
    /// The texture again: 1.25 reads 0.25. The default.
    #[default]
    Repeat,
    /// The texture mirrored at each repetition: 1.25 reads 0.75.
    MirrorRepeat,
    /// The nearest edge of the texture: 1.25 reads 1.
    ClampToEdge,
}

impl AddressMode {
    fn to_wgpu(self) -> wgpu::AddressMode {
        match self {
            Self::Repeat => wgpu::AddressMode::Repeat,
            Self::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
            Self::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        }
    }
}

/// What [`Gpu::create_sampler`] needs: the filters and the address modes, those of glTF's
/// samplers. The default is linear filtering and repetition in both directions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SamplerDescriptor {
    /// The filter when the texture is magnified: one of its pixels covers several of the target.
    pub mag_filter: Filter,
    /// The filter when the texture is minified: several of its pixels fall into one of the
    /// target. Without mipmaps, only the nearest ones are read either way.
    pub min_filter: Filter,
    /// What a horizontal coordinate (`u`) outside 0 to 1 reads.
    pub address_u: AddressMode,
    /// What a vertical coordinate (`v`) outside 0 to 1 reads.
    pub address_v: AddressMode,
}

/// How shaders read a texture, bound next to it in a bind group
/// ([`BindingResource::Sampler`](crate::BindingResource::Sampler)).
///
/// Clones refer to the same sampler.
#[derive(Clone, Debug)]
pub struct Sampler {
    pub(crate) sampler: wgpu::Sampler,
}

impl Gpu {
    /// Creates a sampler. `label` names it in error messages and in GPU capture tools.
    pub fn create_sampler(&self, label: &str, descriptor: &SamplerDescriptor) -> Sampler {
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: descriptor.address_u.to_wgpu(),
            address_mode_v: descriptor.address_v.to_wgpu(),
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: descriptor.mag_filter.to_wgpu(),
            min_filter: descriptor.min_filter.to_wgpu(),
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..wgpu::SamplerDescriptor::default()
        });
        Sampler { sampler }
    }
}
