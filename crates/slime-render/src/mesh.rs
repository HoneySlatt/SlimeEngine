//! Meshes: triangles, given as vertices and indices, uploaded to the GPU.

use glam::Vec3;
use slime_gfx::{Gpu, IndexBuffer, VertexAttribute, VertexBuffer, VertexFormat, VertexLayout};

use crate::bytes::push_f32s;
use crate::error::RenderError;

/// The data of a mesh, as an application or a loader produces it.
///
/// The four vertex arrays have one entry per vertex. `indices` lists the triangles, three
/// indices each, into those arrays. Front faces wind counter-clockwise, as in glTF.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    /// The position of each vertex, in the mesh's own space.
    pub positions: Vec<[f32; 3]>,
    /// The normal of each vertex: of length one, pointing out of the surface.
    pub normals: Vec<[f32; 3]>,
    /// The texture coordinates of each vertex, (0, 0) being the top-left corner of a texture, as
    /// in glTF.
    pub uvs: Vec<[f32; 2]>,
    /// The tangent of each vertex, as glTF defines it: xyz of length one, perpendicular to the
    /// normal, pointing where the texture coordinate u grows; w, 1 or −1, the handedness: the
    /// bitangent cross(normal, xyz) × w points where v shrinks, up the texture's image. Normal
    /// textures are read in the frame these give; any unit vector perpendicular to the normal will
    /// do for a mesh whose materials have none.
    pub tangents: Vec<[f32; 4]>,
    /// The vertices of each triangle, as indices into the arrays above.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Why the data cannot be drawn, if it cannot, by a device whose largest buffer holds
    /// `max_buffer_size` bytes.
    fn check(&self, max_buffer_size: u64) -> Result<(), String> {
        let vertices = self.positions.len();
        if vertices == 0 {
            return Err("it has no vertex".to_owned());
        }
        for (name, count) in [
            ("normals", self.normals.len()),
            ("uvs", self.uvs.len()),
            ("tangents", self.tangents.len()),
        ] {
            if count != vertices {
                return Err(format!("it has {vertices} positions but {count} {name}"));
            }
        }
        // A value that is not finite would put a vertex nowhere, or shade it with NaN, and an
        // infinite position would leave the scene's shadow map without a size.
        fn not_finite(values: impl IntoIterator<Item = f32>) -> Option<f32> {
            values.into_iter().find(|value| !value.is_finite())
        }
        for (name, value) in [
            (
                "position",
                not_finite(self.positions.iter().flatten().copied()),
            ),
            ("normal", not_finite(self.normals.iter().flatten().copied())),
            ("uv", not_finite(self.uvs.iter().flatten().copied())),
            (
                "tangent",
                not_finite(self.tangents.iter().flatten().copied()),
            ),
        ] {
            if let Some(value) = value {
                return Err(format!(
                    "a vertex's {name} has a value that is not finite: {value}"
                ));
            }
        }
        if self.indices.is_empty() {
            return Err("it has no triangle".to_owned());
        }
        if !self.indices.len().is_multiple_of(3) {
            return Err(format!(
                "it has {} indices, which is not a multiple of 3",
                self.indices.len()
            ));
        }
        for (count, what, size) in [
            (vertices, "vertices", VERTEX_STRIDE),
            (self.indices.len(), "indices", 4),
        ] {
            let bytes = count as u64 * size;
            if bytes > max_buffer_size {
                return Err(format!(
                    "its {count} {what} take {bytes} bytes, more than the {max_buffer_size} \
                     bytes of the largest buffer the device accepts"
                ));
            }
        }
        if let Some(index) = self
            .indices
            .iter()
            .find(|&&index| index as usize >= vertices)
        {
            return Err(format!(
                "index {index} names no vertex: there are {vertices} vertices"
            ));
        }
        Ok(())
    }

    /// The smallest and the largest coordinates of the positions along each axis: the corners of
    /// the smallest box around them, which the shadow map is fitted to (ADR-0038).
    fn bounds(&self) -> (Vec3, Vec3) {
        self.positions
            .iter()
            .map(|&position| Vec3::from(position))
            .fold(
                (Vec3::INFINITY, Vec3::NEG_INFINITY),
                |(min, max), position| (min.min(position), max.max(position)),
            )
    }

    /// The vertices interleaved as the scene pipelines read them: position, normal, texture
    /// coordinates, tangent.
    fn vertex_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.positions.len() * VERTEX_STRIDE as usize);
        for (((position, normal), uv), tangent) in self
            .positions
            .iter()
            .zip(&self.normals)
            .zip(&self.uvs)
            .zip(&self.tangents)
        {
            push_f32s(&mut bytes, position);
            push_f32s(&mut bytes, normal);
            push_f32s(&mut bytes, uv);
            push_f32s(&mut bytes, tangent);
        }
        bytes
    }
}

/// A mesh on the GPU, created by [`Renderer::create_mesh`](crate::Renderer::create_mesh).
///
/// Clones refer to the same buffers.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub(crate) vertices: VertexBuffer,
    pub(crate) indices: IndexBuffer,
    /// The center of the smallest box around the positions, in the mesh's own space.
    pub(crate) center: Vec3,
    /// Half the size of that box along each axis.
    pub(crate) half_size: Vec3,
}

impl Mesh {
    /// How many triangles the mesh has.
    pub fn triangle_count(&self) -> u32 {
        self.indices.count() / 3
    }

    /// Checks `data` and uploads it.
    pub(crate) fn new(gpu: &Gpu, label: &str, data: &MeshData) -> Result<Self, RenderError> {
        data.check(gpu.max_buffer_size())
            .map_err(|reason| RenderError::InvalidMesh {
                label: label.to_owned(),
                reason,
            })?;
        let vertices =
            gpu.create_vertex_buffer(&format!("{label} vertices"), &data.vertex_bytes())?;
        let indices = gpu.create_index_buffer(&format!("{label} indices"), &data.indices)?;
        let (min, max) = data.bounds();
        Ok(Self {
            vertices,
            indices,
            center: (min + max) / 2.0,
            half_size: (max - min) / 2.0,
        })
    }
}

/// The size of one vertex as the scene pipelines read it: twelve 32-bit floats: position, normal,
/// texture coordinates, tangent.
const VERTEX_STRIDE: u64 = 12 * 4;

/// How the scene pipelines read the vertices of a mesh.
pub(crate) const VERTEX_LAYOUT: VertexLayout<'static> = VertexLayout {
    stride: VERTEX_STRIDE,
    attributes: &[
        VertexAttribute {
            location: 0,
            offset: 0,
            format: VertexFormat::Float32x3,
        },
        VertexAttribute {
            location: 1,
            offset: 3 * 4,
            format: VertexFormat::Float32x3,
        },
        VertexAttribute {
            location: 2,
            offset: 6 * 4,
            format: VertexFormat::Float32x2,
        },
        VertexAttribute {
            location: 3,
            offset: 8 * 4,
            format: VertexFormat::Float32x4,
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The largest buffer of wgpu's default limits, which `Gpu::new` asks for.
    const MAX_BUFFER_SIZE: u64 = 256 << 20;

    fn triangle() -> MeshData {
        MeshData {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn consistent_data_is_accepted_and_interleaved() {
        let data = triangle();
        assert_eq!(data.check(MAX_BUFFER_SIZE), Ok(()));
        let bytes = data.vertex_bytes();
        assert_eq!(bytes.len(), 3 * 48);
        // The second vertex: position (1, 0, 0), normal (0, 0, 1), uv (1, 0), tangent (1, 0, 0, 1).
        let floats: Vec<f32> = bytes[48..96]
            .chunks(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect();
        assert_eq!(
            floats,
            [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn the_bounds_are_the_smallest_box_around_the_positions() {
        let mut data = triangle();
        data.positions[1] = [-2.0, 5.0, 0.5];
        assert_eq!(
            data.bounds(),
            (Vec3::new(-2.0, 0.0, 0.0), Vec3::new(0.0, 5.0, 0.5))
        );
    }

    #[test]
    fn inconsistent_data_is_rejected_with_a_reason() {
        let mut no_vertex = triangle();
        no_vertex.positions.clear();
        assert_eq!(
            no_vertex.check(MAX_BUFFER_SIZE),
            Err("it has no vertex".to_owned())
        );

        let mut fewer_normals = triangle();
        fewer_normals.normals.pop();
        assert_eq!(
            fewer_normals.check(MAX_BUFFER_SIZE),
            Err("it has 3 positions but 2 normals".to_owned())
        );

        let mut fewer_tangents = triangle();
        fewer_tangents.tangents.pop();
        assert_eq!(
            fewer_tangents.check(MAX_BUFFER_SIZE),
            Err("it has 3 positions but 2 tangents".to_owned())
        );

        let mut no_triangle = triangle();
        no_triangle.indices.clear();
        assert_eq!(
            no_triangle.check(MAX_BUFFER_SIZE),
            Err("it has no triangle".to_owned())
        );

        let mut half_triangle = triangle();
        half_triangle.indices.push(0);
        assert_eq!(
            half_triangle.check(MAX_BUFFER_SIZE),
            Err("it has 4 indices, which is not a multiple of 3".to_owned())
        );

        let mut out_of_range = triangle();
        out_of_range.indices[2] = 3;
        assert_eq!(
            out_of_range.check(MAX_BUFFER_SIZE),
            Err("index 3 names no vertex: there are 3 vertices".to_owned())
        );
    }

    #[test]
    fn values_that_are_not_finite_are_rejected_in_every_vertex_array() {
        let mut position = triangle();
        position.positions[1][0] = f32::INFINITY;
        assert_eq!(
            position.check(MAX_BUFFER_SIZE),
            Err("a vertex's position has a value that is not finite: inf".to_owned())
        );
        let mut normal = triangle();
        normal.normals[2][1] = f32::NAN;
        assert_eq!(
            normal.check(MAX_BUFFER_SIZE),
            Err("a vertex's normal has a value that is not finite: NaN".to_owned())
        );
        let mut uv = triangle();
        uv.uvs[0][1] = f32::NEG_INFINITY;
        assert_eq!(
            uv.check(MAX_BUFFER_SIZE),
            Err("a vertex's uv has a value that is not finite: -inf".to_owned())
        );
        let mut tangent = triangle();
        tangent.tangents[1][3] = f32::NAN;
        assert_eq!(
            tangent.check(MAX_BUFFER_SIZE),
            Err("a vertex's tangent has a value that is not finite: NaN".to_owned())
        );
    }

    #[test]
    fn data_larger_than_the_largest_buffer_is_rejected_with_a_reason() {
        // The triangle's vertices take 144 bytes, its indices 12.
        assert_eq!(triangle().check(144), Ok(()));
        assert_eq!(
            triangle().check(143),
            Err(
                "its 3 vertices take 144 bytes, more than the 143 bytes of the largest buffer the \
                 device accepts"
                    .to_owned()
            )
        );
        let mut many_indices = triangle();
        many_indices.indices = [0, 1, 2].repeat(13);
        assert_eq!(
            many_indices.check(144),
            Err(
                "its 39 indices take 156 bytes, more than the 144 bytes of the largest buffer the \
                 device accepts"
                    .to_owned()
            )
        );
    }
}
