//! Buffers of vertex data.

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
    ///
    /// # Panics
    ///
    /// If `contents` is empty (wgpu cannot bind an empty buffer), or larger than the device allows
    /// (256 MiB with the default limits).
    pub fn create_vertex_buffer(&self, label: &str, contents: &[u8]) -> VertexBuffer {
        assert!(
            !contents.is_empty(),
            "vertex buffer `{label}` is empty: wgpu cannot bind an empty buffer"
        );
        let buffer = self.create_buffer_with(label, contents, wgpu::BufferUsages::VERTEX);
        VertexBuffer { buffer }
    }
}
