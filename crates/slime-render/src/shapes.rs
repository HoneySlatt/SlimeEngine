//! Simple shapes, made in code: a cube, a sphere and a square, each one metre across and centred
//! on its origin, for an instance's transform to place and scale.

use std::f32::consts::{PI, TAU};

use glam::Vec3;

use crate::mesh::MeshData;

impl MeshData {
    /// A cube one metre on a side, centred on the origin, its faces across the axes.
    ///
    /// Each face has four vertices of its own, so that its normal is flat, and the whole texture,
    /// upright as seen from outside; the top of the texture points to −z on the face that looks
    /// up (+y), and to +z on the one that looks down. 24 vertices, 12 triangles.
    ///
    /// ```
    /// use slime_render::MeshData;
    /// use slime_render::glam::{Mat4, Vec3};
    ///
    /// let cube = MeshData::cube();
    /// assert_eq!(cube.indices.len() / 3, 12);
    /// // An instance with this transform draws a wall 4 m long, 2.5 m high and 20 cm thick,
    /// // standing on the ground: the cube reaches 0.5 m on each side of its origin.
    /// let wall = Mat4::from_scale_rotation_translation(
    ///     Vec3::new(4.0, 2.5, 0.2),
    ///     Default::default(),
    ///     Vec3::new(0.0, 1.25, 0.0),
    /// );
    /// # let _ = wall;
    /// ```
    pub fn cube() -> Self {
        // Each face: its normal, then the directions of its right and of its top as seen from
        // outside, whose cross product is the normal.
        const FACES: [[Vec3; 3]; 6] = [
            [Vec3::Z, Vec3::X, Vec3::Y],
            [Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y],
            [Vec3::X, Vec3::NEG_Z, Vec3::Y],
            [Vec3::NEG_X, Vec3::Z, Vec3::Y],
            [Vec3::Y, Vec3::X, Vec3::NEG_Z],
            [Vec3::NEG_Y, Vec3::X, Vec3::Z],
        ];
        let mut data = Self::default();
        for (first, [normal, right, up]) in (0..).step_by(4).zip(FACES) {
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

    /// A smooth sphere one metre across (its radius is 0.5 m), centred on the origin, of `rings`
    /// bands from its pole at +y to its pole at −y, and `segments` segments around the y axis.
    ///
    /// Each vertex's normal points away from the centre. The texture wraps around it once: u grows
    /// with the angle around y from +x towards +z, and v from the top pole to the bottom one; seen
    /// from outside, the texture is mirrored, so the tangents' handedness is −1. The seam, where a
    /// column of vertices repeats the first one with u = 1, faces +x. The sphere has
    /// (`rings` + 1) × (`segments` + 1) vertices and 2 × `segments` × (`rings` − 1) triangles: each
    /// band is a row of quads, two triangles each, but at the poles, where it is one triangle.
    ///
    /// # Panics
    ///
    /// If `rings` is less than 2 or `segments` less than 3, which give no solid.
    pub fn sphere(rings: u32, segments: u32) -> Self {
        assert!(
            rings >= 2 && segments >= 3,
            "a sphere needs at least 2 rings and 3 segments, not {rings} and {segments}"
        );
        let mut data = Self::default();
        for ring in 0..=rings {
            let polar = PI * ring as f32 / rings as f32;
            let (y, ring_radius) = (polar.cos(), polar.sin());
            for segment in 0..=segments {
                let azimuth = TAU * segment as f32 / segments as f32;
                let normal = [ring_radius * azimuth.cos(), y, ring_radius * azimuth.sin()];
                data.positions
                    .push(normal.map(|coordinate| coordinate * 0.5));
                data.normals.push(normal);
                data.uvs
                    .push([segment as f32 / segments as f32, ring as f32 / rings as f32]);
                data.tangents
                    .push([-azimuth.sin(), 0.0, azimuth.cos(), -1.0]);
            }
        }
        // Each ring is a row of `segments + 1` vertices, the last one repeating the first with the
        // texture's right edge. Between two rows, each quad gives two triangles wound
        // counter-clockwise from outside, but at the poles, where one of them would have no area.
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

    /// A square one metre on a side, centred on the origin in the plane z = 0 and facing +z, with
    /// the whole texture on it, upright as seen from +z. 4 vertices, 2 triangles.
    ///
    /// Turned a quarter turn about x, it lies on the ground, facing up:
    ///
    /// ```
    /// use slime_render::MeshData;
    /// use slime_render::glam::{Mat4, Vec3};
    ///
    /// let square = MeshData::square();
    /// // A floor 10 m on a side, facing +y.
    /// let floor = Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
    ///     * Mat4::from_scale(Vec3::new(10.0, 10.0, 1.0));
    /// assert!(floor.transform_vector3(Vec3::from(square.normals[0])).abs_diff_eq(Vec3::Y, 1e-6));
    /// ```
    pub fn square() -> Self {
        Self {
            positions: vec![
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [0.5, 0.5, 0.0],
                [-0.5, 0.5, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks, for every triangle of `data`, that it winds counter-clockwise as seen from where
    /// its corners' normals point, and that each vertex's normal and tangent are of length one and
    /// perpendicular, its tangent along increasing u and its bitangent, normal × tangent × w, up
    /// the image, where v shrinks, as `MeshData::tangents` asks.
    fn check_winding_and_tangents(data: &MeshData) {
        let at = |index: u32| Vec3::from(data.positions[index as usize]);
        let uv = |index: u32| glam::Vec2::from(data.uvs[index as usize]);
        for triangle in data.indices.chunks(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]];
            let face = (at(b) - at(a)).cross(at(c) - at(a));
            assert!(face.length() > 0.0, "triangle {triangle:?} has no area");
            let normal = Vec3::from(data.normals[a as usize]);
            assert!(face.dot(normal) > 0.0, "triangle {triangle:?} faces inward");
            // The direction in which u and v grow across the triangle, from its corners.
            let (edge1, edge2) = (at(b) - at(a), at(c) - at(a));
            let (duv1, duv2) = (uv(b) - uv(a), uv(c) - uv(a));
            let det = duv1.x * duv2.y - duv2.x * duv1.y;
            let along_u = (edge1 * duv2.y - edge2 * duv1.y) / det;
            let along_v = (edge2 * duv1.x - edge1 * duv2.x) / det;
            let tangent = data.tangents[a as usize];
            let xyz = Vec3::new(tangent[0], tangent[1], tangent[2]);
            assert!(
                xyz.dot(along_u) > 0.0,
                "triangle {triangle:?}: tangent against u"
            );
            let bitangent = normal.cross(xyz) * tangent[3];
            assert!(
                bitangent.dot(along_v) < 0.0,
                "triangle {triangle:?}: bitangent along v"
            );
        }
        for (normal, tangent) in data.normals.iter().zip(&data.tangents) {
            let normal = Vec3::from(*normal);
            let xyz = Vec3::new(tangent[0], tangent[1], tangent[2]);
            assert!((normal.length() - 1.0).abs() < 1e-5);
            assert!((xyz.length() - 1.0).abs() < 1e-5);
            assert!(normal.dot(xyz).abs() < 1e-5);
            assert!(tangent[3].abs() == 1.0);
        }
        let vertices = data.positions.len();
        assert_eq!(
            [data.normals.len(), data.uvs.len(), data.tangents.len()],
            [vertices; 3]
        );
    }

    #[test]
    fn the_cube_has_six_flat_faces_wound_outward() {
        let cube = MeshData::cube();
        assert_eq!((cube.positions.len(), cube.indices.len()), (24, 36));
        check_winding_and_tangents(&cube);
        for (corner, normal) in cube.positions.iter().zip(&cube.normals) {
            assert!(corner.iter().all(|coordinate| coordinate.abs() == 0.5));
            // Each corner lies on the face its normal says.
            assert_eq!(Vec3::from(*corner).dot(Vec3::from(*normal)), 0.5);
        }
    }

    #[test]
    fn the_sphere_is_one_metre_across_and_wound_outward() {
        for (rings, segments) in [(2, 3), (8, 12), (16, 24)] {
            let sphere = MeshData::sphere(rings, segments);
            let (rows, columns) = (rings as usize + 1, segments as usize + 1);
            assert_eq!(sphere.positions.len(), rows * columns);
            assert_eq!(
                sphere.indices.len(),
                3 * 2 * segments as usize * (rings as usize - 1)
            );
            check_winding_and_tangents(&sphere);
            for (corner, normal) in sphere.positions.iter().zip(&sphere.normals) {
                assert!((Vec3::from(*corner).length() - 0.5).abs() < 1e-6);
                assert!(Vec3::from(*corner).abs_diff_eq(Vec3::from(*normal) * 0.5, 1e-6));
            }
        }
    }

    #[test]
    #[should_panic(expected = "a sphere needs at least 2 rings and 3 segments, not 1 and 8")]
    fn a_sphere_of_one_ring_is_refused() {
        MeshData::sphere(1, 8);
    }

    #[test]
    fn the_square_faces_z_one_metre_wide() {
        let square = MeshData::square();
        check_winding_and_tangents(&square);
        for corner in &square.positions {
            assert_eq!(
                [corner[0].abs(), corner[1].abs(), corner[2]],
                [0.5, 0.5, 0.0]
            );
        }
    }
}
