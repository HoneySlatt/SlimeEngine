//! Bind groups: the resources that a pipeline's shaders read, in the layout they declare.

use std::num::NonZeroU64;

use crate::depth_target::DepthTarget;
use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::render_target::RenderTarget;
use crate::sampler::{ComparisonSampler, Sampler};
use crate::storage_buffer::StorageBuffer;
use crate::texture::Texture;
use crate::uniform_buffer::UniformBuffer;

/// The shader stages that read a binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderStages {
    /// The vertex shader only.
    Vertex,
    /// The fragment shader only.
    Fragment,
    /// Both shaders.
    VertexAndFragment,
}

impl ShaderStages {
    fn to_wgpu(self) -> wgpu::ShaderStages {
        match self {
            Self::Vertex => wgpu::ShaderStages::VERTEX,
            Self::Fragment => wgpu::ShaderStages::FRAGMENT,
            Self::VertexAndFragment => wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }
}

/// The kind of resource a binding holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BindingType {
    /// A [`UniformBuffer`], declared `var<uniform>` in WGSL.
    UniformBuffer {
        /// Whether the start of the bound range is given each time the group is bound
        /// ([`RenderPass::set_bind_group`](crate::RenderPass::set_bind_group)), so that one buffer
        /// holds the values of many draws, one range each. The group then binds a
        /// [`BindingResource::UniformBufferRange`].
        dynamic_offset: bool,
        /// The size, in bytes, of the values bound there: the buffer or range that a group binds,
        /// at least what the shaders read, and not zero. A pipeline whose shader reads more is
        /// refused when it is made ([`Gpu::create_render_pipeline`]), rather than when a draw binds
        /// the group; binding a smaller buffer or range panics when the group is made
        /// ([`Gpu::create_bind_group`]).
        size: u64,
    },
    /// A [`StorageBuffer`], declared `var<storage, read>` in WGSL: an array, or a structure ending
    /// with one, that the shaders index and do not write.
    StorageBuffer {
        /// The least size, in bytes, of the buffer that a group binds there: at least what the
        /// shaders read whatever the array's length, one element of it for an array, and not zero.
        /// A pipeline whose shader reads more is refused when it is made, and binding a smaller
        /// buffer panics when the group is made, as for a uniform binding.
        size: u64,
    },
    /// A [`Texture`], or a [`RenderTarget`] that an earlier pass drew into, declared
    /// `texture_2d<f32>` in WGSL: sampled through a [`Sampler`] (`textureSample`), or read texel by
    /// texel with `textureLoad`.
    Texture,
    /// A [`Texture`] created by [`Gpu::create_cube_texture`], declared `texture_cube<f32>` in WGSL:
    /// sampled through a [`Sampler`] by direction.
    CubeTexture,
    /// A [`RenderTarget`] of several samples per pixel
    /// ([`Gpu::create_multisampled_render_target`]), declared `texture_multisampled_2d<f32>` in
    /// WGSL: read sample by sample with `textureLoad`, as a shader that resolves it its own way
    /// does.
    MultisampledTexture,
    /// A [`Sampler`], declared `sampler` in WGSL.
    Sampler,
    /// A [`DepthTarget`], declared `texture_depth_2d` in WGSL: read with `textureLoad`, or
    /// compared with a reference depth through a [`ComparisonSampler`] (`textureSampleCompare`).
    DepthTexture,
    /// A [`ComparisonSampler`], declared `sampler_comparison` in WGSL.
    ComparisonSampler,
}

/// One binding of a [`BindGroupLayout`]: its number in the shaders and what it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindGroupLayoutEntry {
    /// The `@binding` number of the resource in the shaders.
    pub binding: u32,
    /// Which shader stages read it.
    pub visible_to: ShaderStages,
    /// What the binding holds.
    pub ty: BindingType,
}

/// The layout of a bind group: which bindings it has and what each holds. Pipelines list the
/// layouts of the groups their shaders read, and bind groups are created from them.
///
/// Clones refer to the same layout, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct BindGroupLayout {
    pub(crate) layout: wgpu::BindGroupLayout,
}

/// A resource bound in a bind group.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum BindingResource<'a> {
    /// A whole uniform buffer.
    UniformBuffer(&'a UniformBuffer),
    /// `size` bytes of a uniform buffer, starting at the offset given each time the group is
    /// bound: for a binding with a dynamic offset. The offset is a multiple of
    /// [`Gpu::uniform_offset_alignment`], and the range must stay inside the buffer.
    UniformBufferRange {
        /// The buffer.
        buffer: &'a UniformBuffer,
        /// The size of the range, in bytes: what the shader's uniform declares.
        size: usize,
    },
    /// A whole storage buffer, for a [`BindingType::StorageBuffer`] binding.
    StorageBuffer(&'a StorageBuffer),
    /// A texture, for a [`BindingType::Texture`] binding, or a [`BindingType::CubeTexture`] one
    /// for a cube texture.
    Texture(&'a Texture),
    /// A sampler.
    Sampler(&'a Sampler),
    /// A render target that an earlier pass drew into: one created by
    /// [`Gpu::create_render_target`], for a [`BindingType::Texture`] binding, or by
    /// [`Gpu::create_multisampled_render_target`], for a [`BindingType::MultisampledTexture`] one.
    /// A pass that binds it cannot draw into it, and the image of a window cannot be bound. The
    /// group keeps this target: a target created again (at a new size) needs a new group.
    RenderTarget(&'a RenderTarget),
    /// A depth target created by [`Gpu::create_depth_target`], for a [`BindingType::DepthTexture`]
    /// binding: a multisampled one cannot be bound. A pass that binds it cannot draw into it. The
    /// group keeps this depth target, as above.
    DepthTarget(&'a DepthTarget),
    /// A comparison sampler, for a [`BindingType::ComparisonSampler`] binding.
    ComparisonSampler(&'a ComparisonSampler),
}

/// One binding of a [`BindGroup`]: its number and the resource it holds.
#[derive(Clone, Copy, Debug)]
pub struct BindGroupEntry<'a> {
    /// The `@binding` number, as in the entry of the layout.
    pub binding: u32,
    /// The resource, of the kind the layout's entry declares.
    pub resource: BindingResource<'a>,
}

/// A set of resources that a pipeline's shaders read, bound together at one `@group` index
/// ([`RenderPass::set_bind_group`](crate::RenderPass::set_bind_group)).
///
/// Clones refer to the same group, which keeps its resources alive.
#[derive(Clone, Debug)]
pub struct BindGroup {
    pub(crate) group: wgpu::BindGroup,
}

impl Gpu {
    /// Creates a bind group layout with `entries`. `label` names it in error messages and in GPU
    /// capture tools. A pipeline lists the layouts of the groups its shaders read, one per `@group`
    /// index ([`RenderPipelineDescriptor::bind_group_layouts`](crate::RenderPipelineDescriptor::bind_group_layouts)),
    /// and [`Gpu::create_bind_group`] creates the groups bound to it.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]), including when the GPU's
    /// memory cannot hold it: wgpu then loses the device. Nothing is created then.
    ///
    /// # Panics
    ///
    /// If two entries have the same binding number, or a uniform or storage binding a size of zero:
    /// this is a bug in the caller (ADR-0016).
    pub fn create_bind_group_layout(
        &self,
        label: &str,
        entries: &[BindGroupLayoutEntry],
    ) -> Result<BindGroupLayout, GfxError> {
        let entries: Vec<wgpu::BindGroupLayoutEntry> = entries
            .iter()
            .map(|entry| wgpu::BindGroupLayoutEntry {
                binding: entry.binding,
                visibility: entry.visible_to.to_wgpu(),
                ty: match entry.ty {
                    BindingType::UniformBuffer {
                        dynamic_offset,
                        size,
                    } => wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: dynamic_offset,
                        min_binding_size: Some(
                            NonZeroU64::new(size).expect("a uniform binding's size is not zero"),
                        ),
                    },
                    BindingType::StorageBuffer { size } => wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            NonZeroU64::new(size).expect("a storage binding's size is not zero"),
                        ),
                    },
                    BindingType::Texture => wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    BindingType::CubeTexture => wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    // wgpu filters no multisampled texture.
                    BindingType::MultisampledTexture => wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: true,
                    },
                    BindingType::Sampler => {
                        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                    }
                    BindingType::DepthTexture => wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    BindingType::ComparisonSampler => {
                        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)
                    }
                },
                count: None,
            })
            .collect();
        let layout = self.create(label, || {
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(label),
                    entries: &entries,
                })
        })?;
        Ok(BindGroupLayout { layout })
    }

    /// Creates a bind group of `layout` holding `entries`: one per entry of the layout, each of
    /// the kind that entry declares. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::DeviceLost`] once the device is lost ([`Gpu::check`]), including when the GPU's
    /// memory cannot hold it: wgpu then loses the device. Nothing is created then.
    ///
    /// # Panics
    ///
    /// If a range has a size of zero or larger than its buffer, or if the entries do not match the
    /// layout: a binding missing, a buffer or range smaller than its binding's size, or a
    /// resource of another kind than its entry declares, such as a depth target at a `Texture`
    /// binding, a cube texture at a `Texture` binding or the reverse, a render target or texture at
    /// a `DepthTexture` binding, or a [`Sampler`] at a `ComparisonSampler` binding or the reverse.
    /// Also if a render target is the image of a window, which cannot be sampled. These are bugs in
    /// the caller, which wgpu rejects (ADR-0016).
    pub fn create_bind_group(
        &self,
        label: &str,
        layout: &BindGroupLayout,
        entries: &[BindGroupEntry<'_>],
    ) -> Result<BindGroup, GfxError> {
        let entries: Vec<wgpu::BindGroupEntry<'_>> = entries
            .iter()
            .map(|entry| wgpu::BindGroupEntry {
                binding: entry.binding,
                resource: match entry.resource {
                    BindingResource::UniformBuffer(buffer) => buffer.buffer.as_entire_binding(),
                    BindingResource::UniformBufferRange { buffer, size } => {
                        let size = NonZeroU64::new(size as u64).unwrap_or_else(|| {
                            panic!(
                                "binding {} of bind group `{label}` has a size of zero",
                                entry.binding
                            )
                        });
                        wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &buffer.buffer,
                            offset: 0,
                            size: Some(size),
                        })
                    }
                    BindingResource::StorageBuffer(buffer) => buffer.buffer.as_entire_binding(),
                    BindingResource::Texture(texture) => {
                        wgpu::BindingResource::TextureView(&texture.view)
                    }
                    BindingResource::Sampler(sampler) => {
                        wgpu::BindingResource::Sampler(&sampler.sampler)
                    }
                    BindingResource::RenderTarget(target) => {
                        wgpu::BindingResource::TextureView(&target.view)
                    }
                    BindingResource::DepthTarget(depth) => {
                        wgpu::BindingResource::TextureView(&depth.view)
                    }
                    BindingResource::ComparisonSampler(sampler) => {
                        wgpu::BindingResource::Sampler(&sampler.sampler)
                    }
                },
            })
            .collect();
        let group = self.create(label, || {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &layout.layout,
                entries: &entries,
            })
        })?;
        Ok(BindGroup { group })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextureFormat;

    /// A stand-in for the image of a window: a texture with the usage that `Surface` configures,
    /// wrapped as the render target of a frame.
    fn window_image(gpu: &Gpu) -> RenderTarget {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("window, frame 1"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: crate::surface::WINDOW_IMAGE_USAGE,
            view_formats: &[],
        });
        RenderTarget::new(texture, "window, frame 1", TextureFormat::Bgra8UnormSrgb)
    }

    #[test]
    fn a_window_image_that_cannot_be_copied_is_not_read_back() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let read = gpu.read_render_target(&window_image(&gpu));
        assert!(
            matches!(read, Err(GfxError::NotReadable { .. })),
            "{read:?}"
        );
    }

    #[test]
    #[should_panic(expected = "TEXTURE_BINDING")]
    fn a_window_image_cannot_be_bound() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let image = window_image(&gpu);
        let layout = gpu
            .create_bind_group_layout(
                "window image",
                &[BindGroupLayoutEntry {
                    binding: 0,
                    visible_to: ShaderStages::Fragment,
                    ty: BindingType::Texture,
                }],
            )
            .expect("the GPU has memory for it");
        gpu.create_bind_group(
            "window image",
            &layout,
            &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::RenderTarget(&image),
            }],
        )
        .expect("the GPU has memory for it");
    }
}
