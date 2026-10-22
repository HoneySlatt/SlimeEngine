#![doc = include_str!("../README.md")]

/// wgpu itself, with the `wgpu` feature: for the engine's tooling only (ADR-0031), which draws
/// with libraries built on wgpu and must use the same wgpu as this crate. Games must not enable
/// the feature.
#[cfg(feature = "wgpu")]
pub use wgpu;

mod bind_group;
mod depth_target;
mod error;
mod float16;
mod gpu;
mod image;
mod index_buffer;
mod pass;
mod pipeline;
mod readback;
mod render_target;
mod sampler;
mod shader;
mod storage_buffer;
mod surface;
mod texture;
mod timer;
mod uniform_buffer;
mod upload;
mod vertex_buffer;

pub use bind_group::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    ShaderStages,
};
pub use depth_target::DepthTarget;
pub use error::GfxError;
pub use float16::f32_to_f16;
pub use gpu::{Gpu, LossKind, MEMORY_BUDGET_ENV_VAR};
pub use image::{FloatImage, Image};
pub use index_buffer::IndexBuffer;
pub use pass::{Commands, DepthPassDescriptor, RenderPass, RenderPassDescriptor};
pub use pipeline::{
    Blend, CullMode, DepthBias, DepthCompare, DepthState, FragmentState, RenderPipeline,
    RenderPipelineDescriptor, VertexAttribute, VertexFormat, VertexLayout,
};
pub use readback::Readback;
pub use render_target::{MULTISAMPLE_COUNT, RenderTarget, TextureFormat};
pub use sampler::{AddressMode, ComparisonSampler, Filter, Sampler, SamplerDescriptor};
pub use shader::ShaderModule;
pub use storage_buffer::StorageBuffer;
pub use surface::{PresentMode, Surface, SurfaceFrame};
pub use texture::Texture;
pub use timer::GpuTimer;
pub use uniform_buffer::UniformBuffer;
pub use upload::{UploadRank, UploadTarget, Uploader};
pub use vertex_buffer::VertexBuffer;

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled, so that a field added later cannot take it away unnoticed.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}
    fn send<T: Send>() {}

    #[test]
    fn the_gpu_and_its_objects_can_be_shared_between_threads() {
        send_and_sync::<Gpu>();
        send_and_sync::<Surface>();
        send_and_sync::<SurfaceFrame>();
        send_and_sync::<RenderTarget>();
        send_and_sync::<DepthTarget>();
        send_and_sync::<Texture>();
        send_and_sync::<Sampler>();
        send_and_sync::<ComparisonSampler>();
        send_and_sync::<VertexBuffer>();
        send_and_sync::<IndexBuffer>();
        send_and_sync::<UniformBuffer>();
        send_and_sync::<StorageBuffer>();
        send_and_sync::<Uploader>();
        send_and_sync::<BindGroupLayout>();
        send_and_sync::<BindGroup>();
        send_and_sync::<ShaderModule>();
        send_and_sync::<RenderPipeline>();
        send_and_sync::<Image>();
        send_and_sync::<FloatImage>();
        send_and_sync::<GfxError>();
        // A timer reads its times back through a cell: it moves between threads, but one thread
        // uses it at a time.
        send::<GpuTimer>();
    }
}
