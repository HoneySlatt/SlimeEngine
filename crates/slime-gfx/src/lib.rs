#![doc = include_str!("../README.md")]

mod error;
mod gpu;
mod image;
mod pass;
mod pipeline;
mod render_target;
mod shader;
mod surface;
mod vertex_buffer;

pub use error::GfxError;
pub use gpu::Gpu;
pub use image::Image;
pub use pass::{Color, RenderPass};
pub use pipeline::{
    RenderPipeline, RenderPipelineDescriptor, VertexAttribute, VertexFormat, VertexLayout,
};
pub use render_target::{RenderTarget, TextureFormat};
pub use shader::ShaderModule;
pub use surface::{PresentMode, Surface, SurfaceFrame};
pub use vertex_buffer::VertexBuffer;
