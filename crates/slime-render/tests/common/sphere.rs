//! A sphere mesh for the tests that draw one.

use slime_render::MeshData;

/// A unit sphere of `rings` × `segments` quads, its normals along its positions, its tangents
/// along the parallels.
pub fn sphere(rings: u32, segments: u32) -> MeshData {
    let mut data = MeshData {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        tangents: Vec::new(),
        indices: Vec::new(),
    };
    for ring in 0..=rings {
        let theta = std::f32::consts::PI * ring as f32 / rings as f32;
        for segment in 0..=segments {
            let phi = std::f32::consts::TAU * segment as f32 / segments as f32;
            let normal = [
                theta.sin() * phi.cos(),
                theta.cos(),
                -theta.sin() * phi.sin(),
            ];
            data.positions.push(normal);
            data.normals.push(normal);
            data.uvs
                .push([segment as f32 / segments as f32, ring as f32 / rings as f32]);
            data.tangents.push([-phi.sin(), 0.0, -phi.cos(), 1.0]);
        }
    }
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let corner = ring * row + segment;
            data.indices.extend([
                corner,
                corner + row,
                corner + 1,
                corner + 1,
                corner + row,
                corner + row + 1,
            ]);
        }
    }
    data
}
