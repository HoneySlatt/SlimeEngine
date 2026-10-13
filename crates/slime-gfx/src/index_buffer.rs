//! Buffers of vertex indices.

use crate::gpu::Gpu;

/// A buffer of 32-bit indices into a vertex buffer, which indexed draws read
/// ([`RenderPass::draw_indexed`](crate::RenderPass::draw_indexed)).
///
/// Clones refer to the same buffer, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct IndexBuffer {
    pub(crate) buffer: wgpu::Buffer,
    count: u32,
}

impl IndexBuffer {
    /// How many indices the buffer holds.
    pub fn count(&self) -> u32 {
        self.count
    }
}

impl Gpu {
    /// Creates an index buffer holding `indices`, each the position of a vertex in the vertex
    /// buffer bound with it. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Panics
    ///
    /// If `indices` is empty (wgpu cannot bind an empty buffer), holds more than `u32::MAX`
    /// indices, or is larger than the device allows (256 MiB with the default limits).
    pub fn create_index_buffer(&self, label: &str, indices: &[u32]) -> IndexBuffer {
        assert!(
            !indices.is_empty(),
            "index buffer `{label}` is empty: wgpu cannot bind an empty buffer"
        );
        let count = u32::try_from(indices.len())
            .unwrap_or_else(|_| panic!("index buffer `{label}` holds more than u32::MAX indices"));
        let contents: Vec<u8> = indices
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect();
        let buffer = self.create_buffer_with(label, &contents, wgpu::BufferUsages::INDEX);
        IndexBuffer { buffer, count }
    }
}
