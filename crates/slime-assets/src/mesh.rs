//! Cooked meshes: the triangles of one glTF primitive, ready for the GPU.

use crate::AssetId;
use crate::bytes::{Reader, Writer};
use crate::error::CookedError;

const MAGIC: &[u8; 4] = b"SLMS";
/// The version of the format of cooked meshes; a change of format changes it.
const VERSION: u32 = 1;

/// A cooked mesh: triangles of vertices that each have a position, a normal, texture coordinates
/// and a tangent, as the renderer draws them (`slime_render::MeshData` holds the same arrays), the
/// box around them, and the material they are drawn with.
#[derive(Clone, Debug, PartialEq)]
pub struct CookedMesh {
    /// Its name, for labels and messages: the glTF mesh's, and the primitive's index.
    pub name: String,
    /// The material it is drawn with.
    pub material: AssetId,
    /// Each vertex's position, in the mesh's own space.
    pub positions: Vec<[f32; 3]>,
    /// Each vertex's normal, of length one.
    pub normals: Vec<[f32; 3]>,
    /// Each vertex's texture coordinates, (0, 0) at the top-left corner of a texture, as glTF.
    pub uvs: Vec<[f32; 2]>,
    /// Each vertex's tangent, as glTF defines it: xyz of length one, w the handedness, 1 or −1.
    pub tangents: Vec<[f32; 4]>,
    /// The vertices of each triangle, three indices each.
    pub indices: Vec<u32>,
    /// The smallest box around its positions: its lowest corner, then its highest.
    pub bounds: [[f32; 3]; 2],
}

impl CookedMesh {
    /// The mesh's file, which [`CookedMesh::from_bytes`] reads back: the format's four bytes,
    /// `SLMS`, and version, then its name, its material, its counts of vertices and indices, its
    /// box, and its arrays, positions, normals, texture coordinates, tangents and indices.
    ///
    /// # Panics
    ///
    /// If the mesh is not one the renderer draws: arrays of different lengths, no vertex, a number
    /// of indices that is not a multiple of 3, an index that names no vertex, or a value that is
    /// not finite.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.check()
            .unwrap_or_else(|reason| panic!("mesh `{}`: {reason}", self.name));
        let mut writer = Writer::new(MAGIC, VERSION);
        writer.text(&self.name);
        writer.id(self.material);
        writer.count(self.positions.len());
        writer.count(self.indices.len());
        writer.f32s(self.bounds.as_flattened());
        writer.f32s(self.positions.as_flattened());
        writer.f32s(self.normals.as_flattened());
        writer.f32s(self.uvs.as_flattened());
        writer.f32s(self.tangents.as_flattened());
        for &index in &self.indices {
            writer.u32(index);
        }
        writer.finish()
    }

    /// The mesh that `bytes`, a file of [`CookedMesh::to_bytes`], holds.
    ///
    /// # Errors
    ///
    /// [`CookedError::Version`] if another version of the format wrote it, and
    /// [`CookedError::Malformed`] if it is not a mesh, is cut short, or holds a mesh that the
    /// renderer does not draw.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CookedError> {
        let mut reader = Reader::open(bytes, MAGIC, VERSION, "mesh")?;
        let name = reader.text()?;
        let material = reader.id()?;
        // A vertex takes 48 bytes, an index 4.
        let vertices = reader.count(48)?;
        let indices = reader.count(4)?;
        let [min_x, min_y, min_z, max_x, max_y, max_z] = reader.f32s()?;
        let mesh = Self {
            name,
            material,
            bounds: [[min_x, min_y, min_z], [max_x, max_y, max_z]],
            positions: reader.f32_arrays(vertices)?,
            normals: reader.f32_arrays(vertices)?,
            uvs: reader.f32_arrays(vertices)?,
            tangents: reader.f32_arrays(vertices)?,
            indices: (0..indices)
                .map(|_| reader.u32())
                .collect::<Result<_, _>>()?,
        };
        reader.finish()?;
        mesh.check().map_err(|reason| CookedError::Malformed {
            what: "mesh",
            reason,
        })?;
        Ok(mesh)
    }

    /// Why the renderer would not draw the mesh, if it would not.
    fn check(&self) -> Result<(), String> {
        let vertices = self.positions.len();
        if vertices == 0 {
            return Err("it has no vertex".to_owned());
        }
        if [self.normals.len(), self.uvs.len(), self.tangents.len()] != [vertices; 3] {
            return Err("its arrays of vertices differ in length".to_owned());
        }
        if !self.indices.len().is_multiple_of(3) || self.indices.is_empty() {
            return Err(format!(
                "it has {} indices, not a positive multiple of 3",
                self.indices.len()
            ));
        }
        if let Some(index) = self
            .indices
            .iter()
            .find(|&&index| index as usize >= vertices)
        {
            return Err(format!(
                "index {index} names no vertex: there are {vertices}"
            ));
        }
        let finite = self
            .positions
            .iter()
            .chain(&self.normals)
            .flatten()
            .chain(self.uvs.as_flattened())
            .chain(self.tangents.as_flattened())
            .chain(self.bounds.as_flattened())
            .all(|value| value.is_finite());
        if !finite {
            return Err("a value is not a finite number".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> CookedMesh {
        CookedMesh {
            name: "triangle".to_owned(),
            material: AssetId::from_u128(5),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0]],
            tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
            bounds: [[0.0; 3], [1.0, 1.0, 0.0]],
        }
    }

    #[test]
    fn a_mesh_is_read_back_from_its_file() {
        let mesh = triangle();
        assert_eq!(CookedMesh::from_bytes(&mesh.to_bytes()), Ok(mesh));
    }

    #[test]
    fn a_damaged_file_is_refused() {
        let bytes = triangle().to_bytes();
        for cut in [0, 8, 40, bytes.len() - 1] {
            assert!(
                CookedMesh::from_bytes(&bytes[..cut]).is_err(),
                "cut at {cut}"
            );
        }
        // An index past the vertices, written as it would be.
        let mut wrong = bytes.clone();
        let last = wrong.len() - 4;
        wrong[last..].copy_from_slice(&7u32.to_le_bytes());
        let error = CookedMesh::from_bytes(&wrong).expect_err("refused");
        assert!(
            error.to_string().contains("index 7 names no vertex"),
            "{error}"
        );
    }

    #[test]
    #[should_panic(expected = "names no vertex")]
    fn a_mesh_the_renderer_would_not_draw_is_not_written() {
        let mut mesh = triangle();
        mesh.indices[2] = 3;
        let _ = mesh.to_bytes();
    }
}
