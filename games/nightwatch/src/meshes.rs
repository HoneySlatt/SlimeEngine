//! The meshes the game is built from, made in code: the engine loads no model file outside its
//! examples (a finding of Stage 3c, `docs/PLAN.md`). Each has the normals, texture coordinates
//! and tangents that `MeshData` requires.

use std::f32::consts::{PI, TAU};

use slime_render::MeshData;
use slime_render::glam::Vec3;

/// A cube one metre on a side, centered on the origin: scaled, it makes every block. Each face has
/// its own four corners, so that its normal is flat, and the whole texture, upright as seen from
/// outside.
pub fn cube() -> MeshData {
    // Each face: its normal, then the directions of its right and of its top as seen from outside,
    // whose cross product is the normal.
    const FACES: [[Vec3; 3]; 6] = [
        [Vec3::Z, Vec3::X, Vec3::Y],
        [Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y],
        [Vec3::X, Vec3::NEG_Z, Vec3::Y],
        [Vec3::NEG_X, Vec3::Z, Vec3::Y],
        [Vec3::Y, Vec3::X, Vec3::NEG_Z],
        [Vec3::NEG_Y, Vec3::X, Vec3::Z],
    ];
    let mut data = MeshData::default();
    for [normal, right, up] in FACES {
        let first = u32::try_from(data.positions.len()).expect("24 corners");
        // Counter-clockwise from outside: bottom left, bottom right, top right, top left, with
        // glTF's texture coordinates, v down the image.
        for (across, high, uv) in [
            (-0.5, -0.5, [0.0, 1.0]),
            (0.5, -0.5, [1.0, 1.0]),
            (0.5, 0.5, [1.0, 0.0]),
            (-0.5, 0.5, [0.0, 0.0]),
        ] {
            let corner = normal * 0.5 + right * across + up * high;
            data.positions.push(corner.to_array());
            data.normals.push(normal.to_array());
            data.uvs.push(uv);
            // Along increasing u, with the bitangent, normal × tangent, up the image.
            data.tangents.push(right.extend(1.0).to_array());
        }
        data.indices
            .extend([0, 1, 2, 0, 2, 3].map(|corner| first + corner));
    }
    data
}

/// A smooth sphere of radius 1, centered on the origin, of `rings` rings from pole to pole and
/// `segments` segments around.
pub fn sphere(rings: u32, segments: u32) -> MeshData {
    let mut data = MeshData::default();
    for ring in 0..=rings {
        let polar = PI * ring as f32 / rings as f32;
        let (y, ring_radius) = (polar.cos(), polar.sin());
        for segment in 0..=segments {
            let azimuth = TAU * segment as f32 / segments as f32;
            let normal = [ring_radius * azimuth.cos(), y, ring_radius * azimuth.sin()];
            data.positions.push(normal);
            data.normals.push(normal);
            data.uvs
                .push([segment as f32 / segments as f32, ring as f32 / rings as f32]);
            data.tangents
                .push([-azimuth.sin(), 0.0, azimuth.cos(), -1.0]);
        }
    }
    // Each ring is a row of `segments + 1` corners, the last repeating the first at the texture's
    // right edge. Between two rows, each quad gives two triangles wound counter-clockwise from
    // outside, but at the poles, where one of them would have no area.
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * row + segment;
            let (b, d) = (a + row, a + 1);
            let c = b + 1;
            if ring != 0 {
                data.indices.extend([a, d, c]);
            }
            if ring != rings - 1 {
                data.indices.extend([a, c, b]);
            }
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks, for every triangle of `data`, that it winds counter-clockwise as seen from where
    /// its corners' normals point, and that each corner's tangent is perpendicular to its normal.
    fn check_winding_and_tangents(data: &MeshData) {
        let at = |index: u32| Vec3::from(data.positions[index as usize]);
        for triangle in data.indices.chunks(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
            let face = (at(b) - at(a)).cross(at(c) - at(a));
            let normal = Vec3::from(data.normals[a as usize]);
            assert!(face.dot(normal) > 0.0, "triangle {triangle:?} faces inward");
        }
        for (normal, tangent) in data.normals.iter().zip(&data.tangents) {
            let tangent = Vec3::new(tangent[0], tangent[1], tangent[2]);
            assert!(Vec3::from(*normal).dot(tangent).abs() < 1e-5);
            assert!((tangent.length() - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn the_cube_has_six_flat_faces_wound_outward() {
        let cube = cube();
        assert_eq!((cube.positions.len(), cube.indices.len()), (24, 36));
        check_winding_and_tangents(&cube);
        for corner in &cube.positions {
            assert!(corner.iter().all(|coordinate| coordinate.abs() == 0.5));
        }
    }

    #[test]
    fn the_sphere_is_wound_outward_with_tangents_along_its_rings() {
        let sphere = sphere(8, 12);
        check_winding_and_tangents(&sphere);
        for corner in &sphere.positions {
            assert!((Vec3::from(*corner).length() - 1.0).abs() < 1e-5);
        }
    }
}
