#![doc = include_str!("../README.md")]

mod bind_group;
mod depth_target;
mod error;
mod gpu;
mod image;
mod index_buffer;
mod pass;
mod pipeline;
mod render_target;
mod shader;
mod surface;
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
pub use shader::ShaderModule;
pub use surface::{PresentMode, Surface, SurfaceFrame};
pub use uniform_buffer::UniformBuffer;
pub use vertex_buffer::VertexBuffer;
