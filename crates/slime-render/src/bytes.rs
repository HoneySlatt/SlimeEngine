//! Packing values for the GPU: little-endian bytes, laid out as the shaders declare.

use glam::Mat4;

/// Appends `values` as little-endian 32-bit floats.
pub(crate) fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

/// Appends `matrix` as a `mat4x4<f32>`: sixteen floats, column after column.
pub(crate) fn push_mat4(bytes: &mut Vec<u8>, matrix: Mat4) {
    push_f32s(bytes, &matrix.to_cols_array());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matrix_is_packed_column_by_column() {
        let mut bytes = Vec::new();
        push_mat4(
            &mut bytes,
            Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
        );
        assert_eq!(bytes.len(), 64);
        // The translation is the fourth column: floats 12, 13 and 14.
        let float =
            |index: usize| f32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap());
        assert_eq!(
            (float(0), float(12), float(13), float(14), float(15)),
            (1.0, 1.0, 2.0, 3.0, 1.0)
        );
    }
}
