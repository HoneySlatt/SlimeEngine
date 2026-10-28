//! Buffers of vertex data.

use crate::error::GfxError;
use crate::gpu::Gpu;
use crate::storage_buffer::write_range;

/// A buffer of vertex data that render passes read. It has two uses: vertices, and copies into it,
/// so that it is written by ranges ([`Gpu::write_vertex_buffer`]) and holds the vertices of many
/// meshes, each drawn from its first vertex
/// ([`RenderPass::draw_indexed`](crate::RenderPass::draw_indexed)).
///
/// Clones refer to the same buffer, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct VertexBuffer {
    /// The identity of the device that created it, which each use checks.
    pub(crate) gpu: crate::gpu::GpuId,
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) label: String,
    pub(crate) size: u64,
}

impl Gpu {
    /// Creates a vertex buffer holding `contents`, laid out as the pipeline that reads it
    /// declares in its [`VertexLayout`](crate::VertexLayout). `label` names it in error messages
    /// and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If `contents` is empty (wgpu cannot bind an empty buffer), or larger than the device allows
    /// ([`Gpu::max_buffer_size`], 256 MiB at least: the engine's floor).
    pub fn create_vertex_buffer(
        &self,
        label: &str,
        contents: &[u8],
    ) -> Result<VertexBuffer, GfxError> {
        assert!(
            !contents.is_empty(),
            "vertex buffer `{label}` is empty: wgpu cannot bind an empty buffer"
        );
        let buffer = self.create(label, || {
            self.create_buffer_with(label, contents, VERTEX_USES)
        })?;
        Ok(VertexBuffer {
            gpu: self.id,
            buffer,
            label: label.to_owned(),
            size: contents.len() as u64,
        })
    }

    /// Creates a vertex buffer of `size` bytes, all zero, which [`Gpu::write_vertex_buffer`]
    /// fills. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// As [`Gpu::create_vertex_buffer`].
    ///
    /// # Panics
    ///
    /// If `size` is zero or not a multiple of 4 bytes, which GPU copies require, or larger than
    /// the device allows ([`Gpu::max_buffer_size`]).
    pub fn create_zeroed_vertex_buffer(
        &self,
        label: &str,
        size: u64,
    ) -> Result<VertexBuffer, GfxError> {
        assert!(
            size > 0 && size.is_multiple_of(4),
            "vertex buffer `{label}` holds {size} bytes; it needs at least 4, in multiples of 4"
        );
        let buffer = self.create(label, || {
            self.create_zeroed_buffer(label, size, VERTEX_USES)
        })?;
        Ok(VertexBuffer {
            gpu: self.id,
            buffer,
            label: label.to_owned(),
            size,
        })
    }

    /// Replaces the `contents.len()` bytes of `buffer` from byte `offset` with `contents`, and
    /// leaves the others as they are. The draws of work submitted after this call
    /// ([`Gpu::render`]) read the new content; those of work submitted before read the previous
    /// one.
    ///
    /// # Panics
    ///
    /// If `contents` is empty, if `offset` or the length of `contents` is not a multiple of 4
    /// bytes, which GPU copies require, or if the range reaches beyond the buffer.
    ///
    /// If an object that it is given belongs to another `Gpu` than this one and its clones.
    #[track_caller]
    pub fn write_vertex_buffer(&self, buffer: &VertexBuffer, offset: u64, contents: &[u8]) {
        self.id.owns(buffer.gpu, "a vertex buffer");
        write_range("vertex", &buffer.label, buffer.size, offset, contents);
        self.queue.write_buffer(&buffer.buffer, offset, contents);
    }
}

/// The uses of a vertex buffer: vertices, and copies into it.
const VERTEX_USES: wgpu::BufferUsages =
    wgpu::BufferUsages::VERTEX.union(wgpu::BufferUsages::COPY_DST);
