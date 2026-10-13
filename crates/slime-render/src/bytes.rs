//! Packing values for the GPU: little-endian bytes, laid out as the shaders declare.

use glam::{Mat3, Mat4};

/// Appends `values` as little-endian 32-bit floats.
pub(crate) fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

/// Appends `values` as little-endian 32-bit unsigned integers.
pub(crate) fn push_u32s(bytes: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

/// Appends `matrix` as a `mat4x4<f32>`: sixteen floats, column after column.
pub(crate) fn push_mat4(bytes: &mut Vec<u8>, matrix: Mat4) {
    push_f32s(bytes, &matrix.to_cols_array());
}

/// Appends `matrix` as a `mat3x3<f32>`: three columns of three floats, each padded to four, as
/// WGSL lays them out in a uniform.
pub(crate) fn push_mat3(bytes: &mut Vec<u8>, matrix: Mat3) {
    for column in [matrix.x_axis, matrix.y_axis, matrix.z_axis] {
        push_f32s(bytes, &[column.x, column.y, column.z, 0.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float(bytes: &[u8], index: usize) -> f32 {
        f32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
    }

    #[test]
    fn a_matrix_is_packed_column_by_column() {
        let mut bytes = Vec::new();
        push_mat4(
            &mut bytes,
            Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0)),
        );
        assert_eq!(bytes.len(), 64);
        // The translation is the fourth column: floats 12, 13 and 14.
        let floats = [0, 12, 13, 14, 15].map(|index| float(&bytes, index));
        assert_eq!(floats, [1.0, 1.0, 2.0, 3.0, 1.0]);
    }

    #[test]
    fn a_3x3_matrix_pads_each_column_to_four_floats() {
        let mut bytes = Vec::new();
        push_mat3(
            &mut bytes,
            Mat3::from_cols(
                glam::Vec3::new(1.0, 2.0, 3.0),
                glam::Vec3::new(4.0, 5.0, 6.0),
                glam::Vec3::new(7.0, 8.0, 9.0),
            ),
        );
        assert_eq!(bytes.len(), 48);
        let floats: Vec<f32> = (0..12).map(|index| float(&bytes, index)).collect();
        assert_eq!(
            floats,
            [1.0, 2.0, 3.0, 0.0, 4.0, 5.0, 6.0, 0.0, 7.0, 8.0, 9.0, 0.0]
        );
    }
}
