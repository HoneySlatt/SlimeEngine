//! Samplers: how shaders read textures, and how they compare depths with a depth target's.

use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::pipeline::DepthCompare;

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

    fn to_wgpu_mipmap(self) -> wgpu::MipmapFilterMode {
        match self {
            Self::Nearest => wgpu::MipmapFilterMode::Nearest,
            Self::Linear => wgpu::MipmapFilterMode::Linear,
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
    /// target. The level of detail that fits is read, between its pixels with this filter, and
    /// blended with the next level the same way: linear filtering blends the two nearest levels,
    /// nearest filtering reads one.
    pub min_filter: Filter,
    /// What a horizontal coordinate (`u`) outside 0 to 1 reads.
    pub address_u: AddressMode,
    /// What a vertical coordinate (`v`) outside 0 to 1 reads.
    pub address_v: AddressMode,
}

/// How shaders read a texture, bound next to it in a bind group
/// ([`BindingResource::Sampler`](crate::BindingResource::Sampler)).
///
/// Clones refer to the same sampler, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct Sampler {
    pub(crate) sampler: wgpu::Sampler,
}

/// How shaders compare a reference depth with the depths of a
/// [`DepthTarget`](crate::DepthTarget) (`sampler_comparison` in WGSL, read with
/// `textureSampleCompare`): each pixel read gives 1.0 where `reference <compare> stored` holds and
/// 0.0 where it does not, and linear filtering blends the results of the four nearest pixels
/// (percentage-closer filtering, done by the GPU). With [`DepthCompare::GreaterEqual`] and the
/// depth of a point seen from a light as the reference, 1.0 means lit: nothing nearer to the light
/// is stored there. Coordinates outside 0 to 1 read the nearest edge. Bound at a
/// [`BindingType::ComparisonSampler`](crate::BindingType::ComparisonSampler) binding
/// ([`BindingResource::ComparisonSampler`](crate::BindingResource::ComparisonSampler)).
///
/// Clones refer to the same sampler, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct ComparisonSampler {
    pub(crate) sampler: wgpu::Sampler,
}

impl Gpu {
    /// Creates a comparison sampler that compares with `compare` and reads between pixels with
    /// `filter`, magnified or minified alike. `label` names it in error messages and in GPU
    /// capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    pub fn create_comparison_sampler(
        &self,
        label: &str,
        compare: DepthCompare,
        filter: Filter,
    ) -> Result<ComparisonSampler, GfxError> {
        // Clamped to the edge, so that a lookup beyond a shadow map does not wrap around to its
        // other side.
        let sampler = self.create(label, || {
            self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter.to_wgpu(),
                min_filter: filter.to_wgpu(),
                mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                compare: Some(compare.to_wgpu()),
                ..wgpu::SamplerDescriptor::default()
            })
        })?;
        Ok(ComparisonSampler { sampler })
    }

    /// Creates a sampler. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    pub fn create_sampler(
        &self,
        label: &str,
        descriptor: &SamplerDescriptor,
    ) -> Result<Sampler, GfxError> {
        let sampler = self.create(label, || {
            self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: descriptor.address_u.to_wgpu(),
                address_mode_v: descriptor.address_v.to_wgpu(),
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: descriptor.mag_filter.to_wgpu(),
                min_filter: descriptor.min_filter.to_wgpu(),
                mipmap_filter: descriptor.min_filter.to_wgpu_mipmap(),
                ..wgpu::SamplerDescriptor::default()
            })
        })?;
        Ok(Sampler { sampler })
    }
}
