//! Buffers of uniform data: values that a whole draw reads, such as a camera's matrices.

use crate::gpu::Gpu;

/// A buffer of uniform data, which shaders read through a bind group
/// ([`BindingResource::UniformBuffer`](crate::BindingResource::UniformBuffer)) and which the CPU
/// writes again whenever its content changes ([`Gpu::write_uniform_buffer`]).
///
/// Clones refer to the same buffer, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct UniformBuffer {
    pub(crate) buffer: wgpu::Buffer,
    label: String,
    /// The length of the contents given at creation, in bytes, which every write has.
    size: usize,
}

impl UniformBuffer {
    /// The size of the buffer, in bytes: the length of the contents it was created with.
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Gpu {
    /// Creates a uniform buffer holding `contents`, laid out as the shader that reads it declares:
    /// WGSL's uniform layout rules, where vectors of three or four components and matrices are
    /// aligned to 16 bytes. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Panics
    ///
    /// If `contents` is empty or its length is not a multiple of 4 bytes, which GPU copies
    /// require, or if it is larger than the device allows (256 MiB with the default limits). A
    /// binding of the whole buffer is limited to 64 KiB with the default limits; a buffer holding
    /// the values of many draws, bound one range at a time, can be larger.
    pub fn create_uniform_buffer(&self, label: &str, contents: &[u8]) -> UniformBuffer {
        assert!(
            !contents.is_empty() && contents.len().is_multiple_of(4),
            "uniform buffer `{label}` holds {} bytes; it needs at least 4, in multiples of 4",
            contents.len()
        );
        let buffer = self.create_buffer_with(
            label,
            contents,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        UniformBuffer {
            buffer,
            label: label.to_owned(),
            size: contents.len(),
        }
    }

    /// Replaces the content of `buffer` with `contents`. The draws of work submitted after this
    /// call ([`Gpu::render`]) read the new content; those of work submitted before read the
    /// previous one.
    ///
    /// # Panics
    ///
    /// If `contents` does not have the size the buffer was created with.
    pub fn write_uniform_buffer(&self, buffer: &UniformBuffer, contents: &[u8]) {
        assert!(
            contents.len() == buffer.size,
            "uniform buffer `{}` holds {} bytes, but {} were written",
            buffer.label,
            buffer.size,
            contents.len()
        );
        self.queue.write_buffer(&buffer.buffer, 0, contents);
    }
}
