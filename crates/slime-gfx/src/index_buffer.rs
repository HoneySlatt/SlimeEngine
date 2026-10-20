//! Buffers of vertex indices.

use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::storage_buffer::write_range;

/// A buffer of 32-bit indices into a vertex buffer, which indexed draws read
/// ([`RenderPass::draw_indexed`](crate::RenderPass::draw_indexed)). It has two uses: indices, and
/// copies into it, so that it is written by ranges ([`Gpu::write_index_buffer`]) and holds the
/// indices of many meshes.
///
/// Clones refer to the same buffer, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct IndexBuffer {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) label: String,
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
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If `indices` is empty (wgpu cannot bind an empty buffer), holds more than `u32::MAX`
    /// indices, or is larger than the device allows ([`Gpu::max_buffer_size`], 256 MiB with the
    /// default limits: 4 bytes per index).
    pub fn create_index_buffer(
        &self,
        label: &str,
        indices: &[u32],
    ) -> Result<IndexBuffer, GfxError> {
        assert!(
            !indices.is_empty(),
            "index buffer `{label}` is empty: wgpu cannot bind an empty buffer"
        );
        let count = u32::try_from(indices.len())
            .unwrap_or_else(|_| panic!("index buffer `{label}` holds more than u32::MAX indices"));
        let buffer = self.create(label, || {
            self.create_buffer_with(label, &index_bytes(indices), INDEX_USES)
        })?;
        Ok(IndexBuffer {
            buffer,
            label: label.to_owned(),
            count,
        })
    }

    /// Creates an index buffer of `count` indices, all zero, which [`Gpu::write_index_buffer`]
    /// fills. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// As [`Gpu::create_index_buffer`].
    ///
    /// # Panics
    ///
    /// If `count` is zero, or the buffer larger than the device allows ([`Gpu::max_buffer_size`]).
    pub fn create_zeroed_index_buffer(
        &self,
        label: &str,
        count: u32,
    ) -> Result<IndexBuffer, GfxError> {
        assert!(
            count > 0,
            "index buffer `{label}` is empty: wgpu cannot bind an empty buffer"
        );
        let buffer = self.create(label, || {
            self.create_zeroed_buffer(label, u64::from(count) * 4, INDEX_USES)
        })?;
        Ok(IndexBuffer {
            buffer,
            label: label.to_owned(),
            count,
        })
    }

    /// Replaces the `indices.len()` indices of `buffer` from the index `first` with `indices`, and
    /// leaves the others as they are. The draws of work submitted after this call
    /// ([`Gpu::render`]) read the new indices; those of work submitted before read the previous
    /// ones.
    ///
    /// # Panics
    ///
    /// If `indices` is empty, or the range reaches beyond the buffer.
    pub fn write_index_buffer(&self, buffer: &IndexBuffer, first: u32, indices: &[u32]) {
        let (offset, contents) = (u64::from(first) * 4, index_bytes(indices));
        let size = u64::from(buffer.count) * 4;
        write_range("index", &buffer.label, size, offset, &contents);
        self.queue.write_buffer(&buffer.buffer, offset, &contents);
    }
}

/// The uses of an index buffer: indices, and copies into it.
const INDEX_USES: wgpu::BufferUsages =
    wgpu::BufferUsages::INDEX.union(wgpu::BufferUsages::COPY_DST);

/// `indices` in the GPU's byte order.
fn index_bytes(indices: &[u32]) -> Vec<u8> {
    indices
        .iter()
        .flat_map(|index| index.to_le_bytes())
        .collect()
}
