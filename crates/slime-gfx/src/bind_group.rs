//! Bind groups: the resources that a pipeline's shaders read, in the layout they declare.

use std::num::NonZeroU64;

use crate::gpu::Gpu;
use crate::sampler::Sampler;
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
    },
    /// A [`Texture`], declared `texture_2d<f32>` in WGSL and read with a [`Sampler`].
    Texture,
    /// A [`Sampler`], declared `sampler` in WGSL.
    Sampler,
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
/// Clones refer to the same layout.
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
    /// A texture.
    Texture(&'a Texture),
    /// A sampler.
    Sampler(&'a Sampler),
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
    /// # Panics
    ///
    /// If two entries have the same binding number: this is a bug in the caller (ADR-0016).
    pub fn create_bind_group_layout(
        &self,
        label: &str,
        entries: &[BindGroupLayoutEntry],
    ) -> BindGroupLayout {
        let entries: Vec<wgpu::BindGroupLayoutEntry> = entries
            .iter()
            .map(|entry| wgpu::BindGroupLayoutEntry {
                binding: entry.binding,
                visibility: entry.visible_to.to_wgpu(),
                ty: match entry.ty {
                    BindingType::UniformBuffer { dynamic_offset } => wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: dynamic_offset,
                        min_binding_size: None,
                    },
                    BindingType::Texture => wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    BindingType::Sampler => {
                        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                    }
                },
                count: None,
            })
            .collect();
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &entries,
            });
        BindGroupLayout { layout }
    }

    /// Creates a bind group of `layout` holding `entries`: one per entry of the layout, each of
    /// the kind that entry declares. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Panics
    ///
    /// If a range has a size of zero or larger than its buffer, or if the entries do not match the
    /// layout (a binding missing, or of another kind): these are bugs in the caller (ADR-0016).
    pub fn create_bind_group(
        &self,
        label: &str,
        layout: &BindGroupLayout,
        entries: &[BindGroupEntry<'_>],
    ) -> BindGroup {
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
                    BindingResource::Texture(texture) => {
                        wgpu::BindingResource::TextureView(&texture.view)
                    }
                    BindingResource::Sampler(sampler) => {
                        wgpu::BindingResource::Sampler(&sampler.sampler)
                    }
                },
            })
            .collect();
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &layout.layout,
            entries: &entries,
        });
        BindGroup { group }
    }
}
