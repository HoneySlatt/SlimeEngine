//! Meshes: triangles, given as vertices and indices, uploaded to the GPU.

use std::ops::Range;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use glam::Vec3;
use slime_gfx::{
    GfxError, Gpu, IndexBuffer, UploadTarget, Uploader, VertexAttribute, VertexBuffer,
    VertexFormat, VertexLayout,
};

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
        // infinite position would leave the shadows' views without a size.
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
    /// the smallest box around them, which the shadows' views are fitted to (ADR-0038).
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

/// A mesh on the GPU, created by [`Renderer::create_mesh`](crate::Renderer::create_mesh). Any
/// renderer of the same `Gpu` draws it.
///
/// Its vertices and its indices lie in buffers that it shares with other meshes of the renderer
/// that created it, each in a range of its own, so that draws of different meshes bind no other
/// buffer: a *page* of 3 MiB of vertices and 1 MiB of indices at first, each later page twice the
/// size of the largest, up to 48 MiB and 16 MiB, and a mesh larger than that in a page of its own.
/// A page's buffers are named after its number in capture tools, not after its meshes.
///
/// Clones refer to the same ranges, which their page takes back when the last clone is dropped; a
/// page whose meshes are all dropped is freed.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub(crate) place: Arc<MeshPlace>,
    /// The center of the smallest box around the positions, in the mesh's own space.
    pub(crate) center: Vec3,
    /// Half the size of that box along each axis.
    pub(crate) half_size: Vec3,
}

impl Mesh {
    /// How many triangles the mesh has.
    pub fn triangle_count(&self) -> u32 {
        self.place.indices.len() as u32 / 3
    }

    /// What it and its clones share, by which the renderer tells meshes apart: clones are one mesh,
    /// drawn together.
    pub(crate) fn identity(&self) -> *const () {
        Arc::as_ptr(&self.place).cast()
    }

    /// Checks `data` and uploads it into a page of `pages`.
    pub(crate) fn new(
        gpu: &Gpu,
        pages: &MeshPages,
        label: &str,
        data: &MeshData,
    ) -> Result<Self, RenderError> {
        let place = Self::place(gpu, pages, label, data)?;
        gpu.write_vertex_buffer(
            &place.page.vertices,
            u64::from(place.vertices.start) * VERTEX_STRIDE,
            &data.vertex_bytes(),
        );
        gpu.write_index_buffer(&place.page.indices, place.indices.start, &data.indices);
        Ok(Self::placed(place, data))
    }

    /// Checks `data`, places it in a page of `pages`, and leaves its copy to `uploader`, which
    /// calls `done` once it is submitted, as [`Uploader::buffers`] says: what a load does, so that
    /// the copy waits for a frame's budget. The mesh is returned at once, to be drawn then.
    ///
    /// # Errors
    ///
    /// As [`Renderer::create_mesh`](crate::Renderer::create_mesh); `done` is not called then.
    pub(crate) fn upload(
        gpu: &Gpu,
        pages: &MeshPages,
        uploader: &Uploader,
        label: &str,
        data: &MeshData,
        done: impl FnOnce(Result<(), GfxError>) + Send + 'static,
    ) -> Result<Self, RenderError> {
        let place = Arc::new(Self::place(gpu, pages, label, data)?);
        // The ranges stay taken until their copy is submitted: a mesh dropped before, as when its
        // load is cancelled, must not give them to another, which the late copy would overwrite.
        let held = Arc::clone(&place);
        let indices: Vec<u8> = data
            .indices
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect();
        uploader.buffers(
            label,
            &[
                (
                    UploadTarget::Vertices(&place.page.vertices),
                    u64::from(place.vertices.start) * VERTEX_STRIDE,
                    &data.vertex_bytes(),
                ),
                (
                    UploadTarget::Indices(&place.page.indices),
                    u64::from(place.indices.start) * 4,
                    &indices,
                ),
            ],
            move |result| {
                drop(held);
                done(result);
            },
        )?;
        Ok(Self::placed(place, data))
    }

    /// Checks `data`, and takes ranges of a page of `pages` for it.
    fn place(
        gpu: &Gpu,
        pages: &MeshPages,
        label: &str,
        data: &MeshData,
    ) -> Result<MeshPlace, RenderError> {
        data.check(gpu.max_buffer_size())
            .map_err(|reason| RenderError::InvalidMesh {
                label: label.to_owned(),
                reason,
            })?;
        let vertices = u32::try_from(data.positions.len()).expect("checked by MeshData");
        let indices = u32::try_from(data.indices.len()).expect("checked by MeshData");
        Ok(pages.place(gpu, vertices, indices)?)
    }

    /// The mesh of `data` at `place`.
    fn placed(place: impl Into<Arc<MeshPlace>>, data: &MeshData) -> Self {
        let (min, max) = data.bounds();
        Self {
            place: place.into(),
            center: (min + max) / 2.0,
            half_size: (max - min) / 2.0,
        }
    }
}

/// The buffers of one page of meshes.
#[derive(Debug)]
pub(crate) struct Page {
    pub(crate) vertices: VertexBuffer,
    pub(crate) indices: IndexBuffer,
}

/// Where a mesh's vertices and indices are: a range of each of its page's buffers, counted in
/// vertices and in indices, which the page takes back when this is dropped.
pub(crate) struct MeshPlace {
    pub(crate) page: Arc<Page>,
    pub(crate) vertices: Range<u32>,
    pub(crate) indices: Range<u32>,
    pages: MeshPages,
}

impl std::fmt::Debug for MeshPlace {
    // Without the pages, which every mesh would print whole.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshPlace")
            .field("vertices", &self.vertices)
            .field("indices", &self.indices)
            .finish_non_exhaustive()
    }
}

impl Drop for MeshPlace {
    fn drop(&mut self) {
        self.pages.give_back(self);
    }
}

/// The pages of the meshes of a renderer, which its loads share: clones refer to the same pages.
#[derive(Clone, Debug, Default)]
pub(crate) struct MeshPages {
    pages: Arc<Mutex<Pages>>,
}

#[derive(Debug, Default)]
struct Pages {
    pages: Vec<PageSpace>,
    /// The number the next page is named after.
    next: usize,
}

/// A page, and its free ranges.
#[derive(Debug)]
struct PageSpace {
    page: Arc<Page>,
    vertices: FreeRanges,
    indices: FreeRanges,
    /// How many meshes it holds.
    meshes: usize,
}

/// The vertices of the first page, and the most of a page shared by several meshes: 3 MiB and
/// 48 MiB of vertices of 48 bytes. Each page holds four times as many indices as vertices: 1 MiB
/// and 16 MiB.
const SMALLEST_PAGE: u32 = 1 << 16;
const LARGEST_PAGE: u32 = 1 << 20;
const INDICES_PER_VERTEX: u32 = 4;

impl MeshPages {
    /// Takes ranges for `vertex_count` vertices and `index_count` indices in the first page with
    /// room for both, or in a new page.
    fn place(&self, gpu: &Gpu, vertex_count: u32, index_count: u32) -> Result<MeshPlace, GfxError> {
        let (page, (vertex_range, index_range)) = {
            let mut pages = self.lock();
            let room = pages.pages.iter_mut().find_map(|space| {
                let taken = space.take(vertex_count, index_count)?;
                Some((Arc::clone(&space.page), taken))
            });
            match room {
                Some(room) => room,
                None => {
                    let space = pages.add(gpu, vertex_count, index_count)?;
                    let taken = space
                        .take(vertex_count, index_count)
                        .expect("a new page has room for what it was made for");
                    (Arc::clone(&space.page), taken)
                }
            }
        };
        Ok(MeshPlace {
            page,
            vertices: vertex_range,
            indices: index_range,
            pages: self.clone(),
        })
    }

    /// Gives the ranges of `place` back to its page, and frees the page if it holds no mesh then.
    fn give_back(&self, place: &MeshPlace) {
        let mut pages = self.lock();
        let index = pages
            .pages
            .iter()
            .position(|space| Arc::ptr_eq(&space.page, &place.page))
            .expect("a mesh's page is kept while it holds the mesh");
        let space = &mut pages.pages[index];
        space.vertices.give(place.vertices.clone());
        space.indices.give(place.indices.clone());
        space.meshes -= 1;
        if space.meshes == 0 {
            pages.pages.swap_remove(index);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Pages> {
        // A panic while the pages are held leaves them whole: a page is added once created, and
        // ranges move between a page and its meshes in steps that cannot panic.
        self.pages.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Pages {
    /// Adds a page with room for `vertices` vertices and `indices` indices: twice the size of the
    /// largest page, from [`SMALLEST_PAGE`] vertices to [`LARGEST_PAGE`], or as large as they need
    /// when that is more.
    fn add(&mut self, gpu: &Gpu, vertices: u32, indices: u32) -> Result<&mut PageSpace, GfxError> {
        let largest = self.pages.iter().map(|space| space.vertices.capacity).max();
        let shared = largest.map_or(SMALLEST_PAGE, |largest| (2 * largest).min(LARGEST_PAGE));
        let vertex_capacity = shared.max(vertices);
        let index_capacity = (shared * INDICES_PER_VERTEX).max(indices);
        let name = self.next;
        let page = Arc::new(Page {
            vertices: gpu.create_zeroed_vertex_buffer(
                &format!("meshes, page {name}, vertices"),
                u64::from(vertex_capacity) * VERTEX_STRIDE,
            )?,
            indices: gpu.create_zeroed_index_buffer(
                &format!("meshes, page {name}, indices"),
                index_capacity,
            )?,
        });
        self.next += 1;
        self.pages.push(PageSpace {
            page,
            vertices: FreeRanges::new(vertex_capacity),
            indices: FreeRanges::new(index_capacity),
            meshes: 0,
        });
        Ok(self.pages.last_mut().expect("pushed"))
    }
}

impl PageSpace {
    /// Takes room for `vertices` vertices and `indices` indices, both or neither.
    fn take(&mut self, vertices: u32, indices: u32) -> Option<(Range<u32>, Range<u32>)> {
        let vertex_range = self.vertices.take(vertices)?;
        let Some(index_range) = self.indices.take(indices) else {
            self.vertices.give(vertex_range);
            return None;
        };
        self.meshes += 1;
        Some((vertex_range, index_range))
    }
}

/// The free ranges of a buffer of `capacity` elements, in order, none touching another.
#[derive(Debug)]
struct FreeRanges {
    capacity: u32,
    free: Vec<Range<u32>>,
}

impl FreeRanges {
    fn new(capacity: u32) -> Self {
        Self {
            capacity,
            free: std::iter::once(0..capacity).collect(),
        }
    }

    /// Takes `count` elements from the first free range that holds them.
    fn take(&mut self, count: u32) -> Option<Range<u32>> {
        let index = self
            .free
            .iter()
            .position(|free| free.len() as u32 >= count)?;
        let start = self.free[index].start;
        self.free[index].start += count;
        if self.free[index].is_empty() {
            self.free.remove(index);
        }
        Some(start..start + count)
    }

    /// Gives `range` back, joined to the free ranges it touches.
    fn give(&mut self, range: Range<u32>) {
        let index = self.free.partition_point(|free| free.start < range.start);
        let joins_next = self
            .free
            .get(index)
            .is_some_and(|next| next.start == range.end);
        let joins_previous = index > 0 && self.free[index - 1].end == range.start;
        match (joins_previous, joins_next) {
            (true, true) => {
                self.free[index - 1].end = self.free[index].end;
                self.free.remove(index);
            }
            (true, false) => self.free[index - 1].end = range.end,
            (false, true) => self.free[index].start = range.start,
            (false, false) => self.free.insert(index, range),
        }
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

    /// The largest buffer of the engine's floor of limits, which `Gpu::new` checks.
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

    #[test]
    fn free_ranges_are_taken_from_the_first_that_fits_and_given_back_joined() {
        let mut free = FreeRanges::new(10);
        assert_eq!(free.take(4), Some(0..4));
        assert_eq!(free.take(4), Some(4..8));
        assert_eq!(free.take(4), None, "2 left");
        free.give(0..4);
        assert_eq!(free.take(3), Some(0..3), "the first that fits");
        free.give(4..8);
        // 3..4 and 4..8 join, then 8..10: one range of 7.
        assert_eq!((free.free.len(), free.free[0].clone()), (1, 3..10));
        free.give(0..3);
        assert_eq!(
            (free.free.len(), free.free[0].clone()),
            (1, 0..10),
            "whole again"
        );
        assert_eq!(free.take(10), Some(0..10));
        assert!(free.free.is_empty());
    }

    /// The number of pages of `pages`, and the vertices of each.
    fn page_sizes(pages: &MeshPages) -> Vec<u32> {
        let pages = pages.lock();
        pages
            .pages
            .iter()
            .map(|space| space.vertices.capacity)
            .collect()
    }

    /// A strip of `vertices` vertices, two triangles for every two vertices past the first two.
    fn strip(vertices: usize) -> MeshData {
        let mut data = triangle();
        data.positions = (0..vertices)
            .map(|index| [index as f32, (index % 2) as f32, 0.0])
            .collect();
        data.normals = vec![[0.0, 0.0, 1.0]; vertices];
        data.uvs = vec![[0.0, 0.0]; vertices];
        data.tangents = vec![[1.0, 0.0, 0.0, 1.0]; vertices];
        data.indices = (0..vertices as u32 - 2)
            .flat_map(|first| [first, first + 1, first + 2])
            .collect();
        data
    }

    #[test]
    fn meshes_share_a_page_until_it_is_full_and_a_page_is_freed_with_its_last_mesh() {
        let gpu = Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists them");
        let pages = MeshPages::default();
        let mesh = |data: &MeshData| Mesh::new(&gpu, &pages, "strip", data).expect("uploaded");
        let first = mesh(&triangle());
        let second = mesh(&triangle());
        assert!(
            Arc::ptr_eq(&first.place.page, &second.place.page),
            "one page"
        );
        assert_eq!(
            (first.place.vertices.clone(), second.place.vertices.clone()),
            (0..3, 3..6)
        );
        assert_eq!(second.place.indices, 3..6);
        assert_eq!(second.triangle_count(), 1);
        // The first page full, the next is twice as large; a mesh larger than the largest shared
        // page has a page of its own size.
        let full = mesh(&strip(SMALLEST_PAGE as usize - 6));
        let next = mesh(&triangle());
        let large = mesh(&strip(LARGEST_PAGE as usize + 1));
        assert_eq!(
            page_sizes(&pages),
            [SMALLEST_PAGE, 2 * SMALLEST_PAGE, LARGEST_PAGE + 1]
        );
        assert!(!Arc::ptr_eq(&next.place.page, &first.place.page));
        // A clone keeps its ranges; the last one gives them back, for the next mesh.
        let clone = first.clone();
        drop(first);
        drop(clone);
        assert_eq!(mesh(&triangle()).place.vertices, 0..3, "reused");
        // A page whose meshes are all dropped is freed.
        drop(large);
        assert_eq!(page_sizes(&pages), [SMALLEST_PAGE, 2 * SMALLEST_PAGE]);
        drop((second, full, next));
        assert!(page_sizes(&pages).is_empty());
    }
}
