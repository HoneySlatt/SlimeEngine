//! Buffers of storage data: arrays that shaders index, such as the values of many instances.

use crate::error::GfxError;
use crate::gpu::Gpu;

/// A buffer of storage data, which shaders read through a bind group
/// ([`BindingResource::StorageBuffer`](crate::BindingResource::StorageBuffer)) as an array they
/// index, `var<storage, read>` in WGSL, and which the CPU writes by ranges
/// ([`Gpu::write_storage_buffer`]). It has two uses: storage, and copies into it.
///
/// Clones refer to the same buffer, which is freed when the last clone is dropped and the GPU no
/// longer uses it.
#[derive(Clone, Debug)]
pub struct StorageBuffer {
    pub(crate) buffer: wgpu::Buffer,
    label: String,
    size: u64,
}

impl Gpu {
    /// Creates a storage buffer of `size` bytes, all zero, which [`Gpu::write_storage_buffer`]
    /// fills. `label` names it in error messages and in GPU capture tools.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] if the GPU's memory cannot hold it, and [`GfxError::DeviceLost`]
    /// once the device is lost ([`Gpu::check`]): nothing is created then.
    ///
    /// # Panics
    ///
    /// If `size` is zero or not a multiple of 4 bytes, which GPU copies require, or if it is
    /// larger than a shader can read at once ([`Gpu::max_storage_buffer_size`], 128 MiB at
    /// least: the engine's floor).
    pub fn create_storage_buffer(&self, label: &str, size: u64) -> Result<StorageBuffer, GfxError> {
        assert!(
            size > 0 && size.is_multiple_of(4),
            "storage buffer `{label}` holds {size} bytes; it needs at least 4, in multiples of 4"
        );
        let largest = self.max_storage_buffer_size();
        assert!(
            size <= largest,
            "storage buffer `{label}` holds {size} bytes, more than the {largest} that a shader \
             reads at once"
        );
        let buffer = self.create(label, || {
            self.create_zeroed_buffer(
                label,
                size,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            )
        })?;
        Ok(StorageBuffer {
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
    pub fn write_storage_buffer(&self, buffer: &StorageBuffer, offset: u64, contents: &[u8]) {
        write_range("storage", &buffer.label, buffer.size, offset, contents);
        self.queue.write_buffer(&buffer.buffer, offset, contents);
    }
}

/// Checks that `contents` can be written at `offset` into the buffer `label` of the kind `kind`,
/// which holds `size` bytes, as the methods that write a buffer by ranges document it.
///
/// # Panics
///
/// If it cannot: `contents` empty, `offset` or the length of `contents` not a multiple of 4 bytes,
/// or the range beyond the buffer.
pub(crate) fn write_range(kind: &str, label: &str, size: u64, offset: u64, contents: &[u8]) {
    let length = contents.len() as u64;
    assert!(
        length > 0 && length.is_multiple_of(4) && offset.is_multiple_of(4),
        "{kind} buffer `{label}`: {length} bytes cannot be written at byte {offset}: at least 4, \
         in multiples of 4, at a multiple of 4"
    );
    assert!(
        offset.checked_add(length).is_some_and(|end| end <= size),
        "{kind} buffer `{label}` holds {size} bytes: {length} bytes cannot be written at byte \
         {offset}"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_inside_the_buffer_in_multiples_of_4_bytes_is_written() {
        write_range("storage", "values", 16, 0, &[0; 16]);
        write_range("storage", "values", 16, 12, &[0; 4]);
    }

    #[test]
    #[should_panic(expected = "storage buffer `values` holds 16 bytes: 8 bytes cannot be written")]
    fn a_range_beyond_the_buffer_is_refused() {
        write_range("storage", "values", 16, 12, &[0; 8]);
    }

    #[test]
    #[should_panic(expected = "in multiples of 4, at a multiple of 4")]
    fn a_range_at_an_offset_that_is_not_a_multiple_of_4_is_refused() {
        write_range("storage", "values", 16, 2, &[0; 4]);
    }

    #[test]
    #[should_panic(expected = "at least 4")]
    fn an_empty_range_is_refused() {
        write_range("storage", "values", 16, 0, &[]);
    }
}
