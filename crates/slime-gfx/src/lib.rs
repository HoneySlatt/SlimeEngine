#![doc = include_str!("../README.md")]

/// wgpu itself, with the `wgpu` feature: for the engine's tooling only (ADR-0031), which draws
/// with libraries built on wgpu and must use the same wgpu as this crate. Games must not enable
/// the feature.
#[cfg(feature = "wgpu")]
pub use wgpu;

mod bind_group;
mod depth_target;
mod error;
mod gpu;
mod image;
mod index_buffer;
mod pass;
mod pipeline;
mod render_target;
mod sampler;
mod shader;
mod surface;
mod texture;
mod timer;
mod uniform_buffer;
mod vertex_buffer;

pub use bind_group::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    ShaderStages,
};
pub use depth_target::DepthTarget;
pub use error::GfxError;
pub use gpu::Gpu;
pub use image::Image;
pub use index_buffer::IndexBuffer;
pub use pass::{Color, RenderPass, RenderPassDescriptor};
pub use pipeline::{
    CullMode, DepthCompare, DepthState, RenderPipeline, RenderPipelineDescriptor, VertexAttribute,
    VertexFormat, VertexLayout,
};
pub use render_target::{RenderTarget, TextureFormat};
pub use sampler::{AddressMode, Filter, Sampler, SamplerDescriptor};
pub use shader::ShaderModule;
pub use surface::{PresentMode, Surface, SurfaceFrame};
pub use texture::Texture;
pub use timer::GpuTimer;
pub use uniform_buffer::UniformBuffer;
pub use vertex_buffer::VertexBuffer;
