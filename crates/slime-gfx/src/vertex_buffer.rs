//! Buffers of vertex data.

use wgpu::util::DeviceExt;

use crate::gpu::Gpu;

/// A buffer of vertex data that render passes read.
///
/// Clones refer to the same buffer, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct VertexBuffer {
    pub(crate) buffer: wgpu::Buffer,
}

impl Gpu {
    /// Creates a vertex buffer holding `contents`, laid out as the pipeline that reads it
    /// declares in its [`VertexLayout`](crate::VertexLayout). `label` names it in error messages
    /// and in GPU capture tools.
    pub fn create_vertex_buffer(&self, label: &str, contents: &[u8]) -> VertexBuffer {
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: wgpu::BufferUsages::VERTEX,
            });
        VertexBuffer { buffer }
    }
}
