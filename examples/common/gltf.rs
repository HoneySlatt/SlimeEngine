//! A temporary loader of glTF scenes for the `scene` example and its image test: GLB files with
//! embedded buffers and PNG images, drawn as they are (ADR-0029). The asset pipeline of Stage 4
//! replaces it with an importer and deletes it.
//!
//! It reads triangle primitives with positions, normals (when missing, each triangle gets the
//! normal of its face, as glTF requires), the first set of texture coordinates (zero when
//! missing) and indices; the base color factor and texture of materials, with the file's sampler
//! settings, but for how levels of detail are read, which `slime-gfx` ties to the minification
//! filter (see `sampler_descriptor`); and the node hierarchy of the default scene, flattened into
//! instances (a node whose transform mirrors draws its primitives with their triangles wound the
//! other way, as glTF prescribes). It refuses, with a clear error, what it cannot draw right:
//! external files, images that are not PNG, images larger than the GPU accepts, primitives whose
//! vertices or indices need a larger buffer than it accepts (one without normals has three
//! vertices per triangle), required extensions, other coordinate sets, other primitive modes. It
//! ignores, with a warning, animations, skins, cameras, vertex colors and extensions the file uses
//! without requiring them.
//! The other properties of materials are ignored without one, as the renderer has no use for
//! them yet: every material is drawn opaque, and with its back faces culled even when the file
//! marks it double-sided, as the sample asset does (its floor disappears when seen from below).
//!
//! The `gltf` crate's validation checks little more than that the file's objects name each other
//! correctly, and its readers panic on some of what it lets through. The loader refuses, rather
//! than panic on, such malformed files: images whose data lies outside the binary chunk, has no
//! MIME type or is missing; positions, normals, texture coordinates or indices stored in types
//! glTF does not allow for them, in a sparse accessor (which the loader does not read), in an
//! empty one, closer together than their size, or outside the binary chunk; a node hierarchy
//! that is not a set of trees.

use std::fmt;

use gltf::Semantic;
use gltf::accessor::{DataType, Dimensions};
use gltf::texture::{MagFilter, MinFilter, WrappingMode};
use slime_gfx::{AddressMode, Filter, Gpu, Sampler, SamplerDescriptor, Texture, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{Instance, Material, MaterialData, Mesh, MeshData, Renderer, SampledTexture};

/// Why a file could not be loaded.
#[derive(Debug)]
pub struct LoadError(String);

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

/// The smallest box around a loaded scene, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    /// The center of the box.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) / 2.0
    }

    /// Half the diagonal of the box: the radius of the sphere around it.
    pub fn radius(&self) -> f32 {
        (self.max - self.min).length() / 2.0
    }

    fn grow(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
}

/// A primitive placed in the world by a node.
struct Placement {
    mesh: usize,
    material: usize,
    transform: Mat4,
}

/// A glTF scene on the GPU: its meshes, its materials, and where to draw them.
pub struct LoadedScene {
    /// One per primitive that a node places as the file winds it, and one per primitive that a
    /// node places mirrored, with its triangles wound the other way.
    meshes: Vec<Mesh>,
    /// One per material of the file, then the default material of glTF, white.
    materials: Vec<Material>,
    placements: Vec<Placement>,
    /// The smallest box around every placed primitive.
    pub bounds: Bounds,
}

impl LoadedScene {
    /// The instances to draw: one per primitive of each node that has a mesh.
    pub fn instances(&self) -> Vec<Instance<'_>> {
        self.placements
            .iter()
            .map(|placement| Instance {
                mesh: &self.meshes[placement.mesh],
                material: &self.materials[placement.material],
                transform: placement.transform,
            })
            .collect()
    }
}

/// A primitive read from the file, uploaded to the GPU once a node places it.
struct LoadedPrimitive {
    label: String,
    data: MeshData,
    material: usize,
}

/// Loads the GLB file `bytes` into GPU objects created through `renderer`.
pub fn load(gpu: &Gpu, renderer: &Renderer, bytes: &[u8]) -> Result<LoadedScene, LoadError> {
    let file = gltf::Gltf::from_slice(bytes)
        .map_err(|error| LoadError(format!("the file is not valid glTF: {error}")))?;
    let document = &file.document;

    let required: Vec<&str> = document.extensions_required().collect();
    if !required.is_empty() {
        return Err(LoadError(format!(
            "the file requires extensions the loader does not support: {}",
            required.join(", ")
        )));
    }
    let used: Vec<&str> = document.extensions_used().collect();
    if !used.is_empty() {
        tracing::warn!(extensions = ?used, "the file uses extensions that the loader ignores");
    }
    let colored = document
        .meshes()
        .flat_map(|mesh| mesh.primitives())
        .filter(|primitive| primitive.get(&Semantic::Colors(0)).is_some())
        .count();
    for (count, what) in [
        (document.animations().len(), "animations"),
        (document.skins().len(), "skins"),
        (document.cameras().len(), "cameras"),
        (colored, "primitives with vertex colors"),
    ] {
        if count > 0 {
            tracing::warn!(count, "the file has {what}, which the loader ignores");
        }
    }

    let blob = file.blob.as_deref().ok_or_else(|| {
        LoadError(
            "the file has no binary chunk: only GLB files with embedded buffers are supported"
                .to_owned(),
        )
    })?;
    for buffer in document.buffers() {
        if let gltf::buffer::Source::Uri(uri) = buffer.source() {
            return Err(LoadError(format!(
                "buffer `{uri}` is an external file, which the loader does not read"
            )));
        }
    }
    let buffer_data = |buffer: gltf::Buffer<'_>| match buffer.source() {
        gltf::buffer::Source::Bin => Some(blob),
        gltf::buffer::Source::Uri(_) => None,
    };

    let textures: Vec<(Texture, Sampler)> = document
        .textures()
        .map(|texture| {
            let image = decode_png(
                document,
                texture.source(),
                blob,
                gpu.max_texture_dimension(),
            )?;
            let label = texture
                .name()
                .or(texture.source().name())
                .map_or_else(|| format!("texture {}", texture.index()), str::to_owned);
            let gpu_texture = gpu.create_texture(
                &label,
                image.width,
                image.height,
                TextureFormat::Rgba8UnormSrgb,
                &image.pixels,
            );
            let sampler = gpu.create_sampler(&label, &sampler_descriptor(&texture.sampler()));
            Ok((gpu_texture, sampler))
        })
        .collect::<Result<_, LoadError>>()?;

    let mut materials: Vec<Material> = document
        .materials()
        .map(|material| {
            let label = material.name().map_or_else(
                || format!("material {}", material.index().unwrap_or(0)),
                str::to_owned,
            );
            let pbr = material.pbr_metallic_roughness();
            let base_color_texture = pbr
                .base_color_texture()
                .map(|info| {
                    if info.tex_coord() != 0 {
                        return Err(LoadError(format!(
                            "material `{label}` reads its texture through coordinate set {}; only \
                             the first set is supported",
                            info.tex_coord()
                        )));
                    }
                    let (texture, sampler) = &textures[info.texture().index()];
                    Ok(SampledTexture { texture, sampler })
                })
                .transpose()?;
            Ok(renderer.create_material(
                gpu,
                &label,
                &MaterialData {
                    base_color: pbr.base_color_factor(),
                    base_color_texture,
                },
            ))
        })
        .collect::<Result<_, LoadError>>()?;
    let default_material = materials.len();
    materials.push(renderer.create_material(gpu, "default", &MaterialData::default()));

    let mut primitives = Vec::new();
    // The primitives of each mesh of the file, as indices into `primitives`.
    let mut primitives_of_mesh = Vec::new();
    for mesh in document.meshes() {
        let name = mesh
            .name()
            .map_or_else(|| format!("mesh {}", mesh.index()), str::to_owned);
        let mut own = Vec::new();
        for primitive in mesh.primitives() {
            let label = format!("{name} primitive {}", primitive.index());
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err(LoadError(format!(
                    "{label} is drawn as {:?}; only triangle lists are supported",
                    primitive.mode()
                )));
            }
            // The `gltf` crate's readers panic on some accessors its validation lets through, and
            // return nothing, as for an attribute the primitive does not have, when the data lies
            // outside the binary chunk: what they would read is checked first.
            let accessors: [(_, _, _, &[_]); 4] = [
                (
                    "positions",
                    primitive.get(&Semantic::Positions),
                    Dimensions::Vec3,
                    &[DataType::F32],
                ),
                (
                    "normals",
                    primitive.get(&Semantic::Normals),
                    Dimensions::Vec3,
                    &[DataType::F32],
                ),
                (
                    "texture coordinates",
                    primitive.get(&Semantic::TexCoords(0)),
                    Dimensions::Vec2,
                    &[DataType::F32, DataType::U8, DataType::U16],
                ),
                (
                    "indices",
                    primitive.indices(),
                    Dimensions::Scalar,
                    &[DataType::U8, DataType::U16, DataType::U32],
                ),
            ];
            for (what, accessor, dimensions, data_types) in accessors {
                if let Some(accessor) = accessor {
                    check_accessor(&label, what, &accessor, dimensions, data_types, blob.len())?;
                }
            }
            let reader = primitive.reader(buffer_data);
            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or_else(|| LoadError(format!("{label} has no positions")))?
                .collect();
            let indices: Vec<u32> = match reader.read_indices() {
                Some(indices) => indices.into_u32().collect(),
                None => (0..u32::try_from(positions.len()).expect("fewer than 4 billion vertices"))
                    .collect(),
            };
            let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
                Some(uvs) => uvs.into_f32().collect(),
                None => vec![[0.0, 0.0]; positions.len()],
            };
            let normals = reader.read_normals();
            // Checked before `flat_shaded` gives each triangle three vertices of its own, so that
            // a small file that makes them too many, three positions and millions of 8-bit
            // indices, is refused before they are copied.
            check_buffer_sizes(
                &label,
                if normals.is_some() {
                    positions.len()
                } else {
                    indices.len()
                },
                indices.len(),
                gpu.max_buffer_size(),
            )?;
            let data = match normals {
                Some(normals) => MeshData {
                    positions,
                    normals: normals.collect(),
                    uvs,
                    indices,
                },
                None => flat_shaded(&label, &positions, &uvs, &indices)?,
            };
            own.push(primitives.len());
            primitives.push(LoadedPrimitive {
                label,
                data,
                material: primitive.material().index().unwrap_or(default_material),
            });
        }
        primitives_of_mesh.push(own);
    }

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| LoadError("the file has no scene".to_owned()))?;
    // Each primitive placed, as an index into `primitives`, with its transform.
    let mut placed = Vec::new();
    let mut bounds = Bounds {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };
    let mut reached = vec![false; document.nodes().len()];
    for node in scene.nodes() {
        place(
            &node,
            &primitives_of_mesh,
            &primitives,
            &mut reached,
            &mut placed,
            &mut bounds,
        )?;
    }
    if placed.is_empty() {
        return Err(LoadError(
            "the scene draws nothing: no node has a mesh".to_owned(),
        ));
    }

    let mut meshes = Vec::new();
    // The mesh of each primitive, as an index into `meshes`, once a placement needs it: as the
    // file winds its triangles, then wound the other way.
    let mut mesh_of = vec![[None; 2]; primitives.len()];
    let mut placements = Vec::with_capacity(placed.len());
    for (index, transform) in placed {
        let primitive = &primitives[index];
        // A transform that mirrors turns the counter-clockwise front faces clockwise, as glTF
        // says, and the renderer would cull them: a mirrored placement draws a copy whose
        // triangles wind the other way. Its normals need no change: the renderer transforms them
        // by the inverse transpose, which mirrors them too.
        let mirrored = transform.determinant() < 0.0;
        let mesh = match mesh_of[index][usize::from(mirrored)] {
            Some(mesh) => mesh,
            None => {
                let mesh = if mirrored {
                    let mut data = primitive.data.clone();
                    for triangle in data.indices.as_chunks_mut::<3>().0 {
                        triangle.swap(1, 2);
                    }
                    let label = format!("{}, mirrored", primitive.label);
                    renderer.create_mesh(gpu, &label, &data)
                } else {
                    renderer.create_mesh(gpu, &primitive.label, &primitive.data)
                }
                .map_err(|error| LoadError(error.to_string()))?;
                meshes.push(mesh);
                mesh_of[index][usize::from(mirrored)] = Some(meshes.len() - 1);
                meshes.len() - 1
            }
        };
        placements.push(Placement {
            mesh,
            material: primitive.material,
            transform,
        });
    }
    tracing::info!(
        meshes = meshes.len(),
        materials = materials.len() - 1,
        textures = textures.len(),
        instances = placements.len(),
        "glTF scene loaded"
    );
    Ok(LoadedScene {
        meshes,
        materials,
        placements,
        bounds,
    })
}

/// Places the primitives of `node`, a node of the scene, and of its descendants: adds each to
/// `placed`, as an index into `primitives` with its transform, and grows `bounds` with it.
/// `reached` says which nodes were placed already.
///
/// The walk goes depth first, in the file's order, with a stack of its own rather than by
/// recursion: a valid file can nest nodes deeper than a thread's stack holds calls. It fails on a
/// node reached a second time: glTF requires the node hierarchy to be a set of trees, which the
/// `gltf` crate does not check, and a node that is its own descendant would otherwise be placed
/// without end.
fn place(
    node: &gltf::Node<'_>,
    primitives_of_mesh: &[Vec<usize>],
    primitives: &[LoadedPrimitive],
    reached: &mut [bool],
    placed: &mut Vec<(usize, Mat4)>,
    bounds: &mut Bounds,
) -> Result<(), LoadError> {
    // The nodes left to place, the next one last, each with the transform of the node above it.
    let mut stack = vec![(node.clone(), Mat4::IDENTITY)];
    while let Some((node, parent)) = stack.pop() {
        if std::mem::replace(&mut reached[node.index()], true) {
            return Err(LoadError(format!(
                "node {} is reached twice: the node hierarchy must be a set of trees",
                node.index()
            )));
        }
        let transform = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        if let Some(mesh) = node.mesh() {
            for &index in &primitives_of_mesh[mesh.index()] {
                for position in &primitives[index].data.positions {
                    bounds.grow(transform.transform_point3(Vec3::from(*position)));
                }
                placed.push((index, transform));
            }
        }
        // Reversed on the stack, so that the first child and its descendants are placed next.
        let children = stack.len();
        stack.extend(node.children().map(|child| (child, transform)));
        stack[children..].reverse();
    }
    Ok(())
}

/// Checks that `accessor`, which holds the `what` of `label`, stores them as `dimensions` of one
/// of `data_types`, laid out so that the `gltf` crate reads them without panicking, inside the
/// binary chunk of `blob_length` bytes.
fn check_accessor(
    label: &str,
    what: &str,
    accessor: &gltf::Accessor<'_>,
    dimensions: Dimensions,
    data_types: &[DataType],
    blob_length: usize,
) -> Result<(), LoadError> {
    let data_type = accessor.data_type();
    if accessor.dimensions() != dimensions || !data_types.contains(&data_type) {
        return Err(LoadError(format!(
            "{label} stores its {what} as {data_type:?} {:?}, which the loader does not read",
            accessor.dimensions()
        )));
    }
    // A sparse accessor needs no buffer view, and its reader then allocates at once as many
    // elements as the file claims, which no data backs.
    let (None, Some(view)) = (accessor.sparse(), accessor.view()) else {
        return Err(LoadError(format!(
            "{label} stores its {what} in a sparse accessor, which the loader does not read"
        )));
    };
    let count = accessor.count();
    if count == 0 {
        return Err(LoadError(format!(
            "{label} has an empty accessor of {what}"
        )));
    }
    let size = accessor.size();
    let stride = view.stride().unwrap_or(size);
    if stride < size {
        return Err(LoadError(format!(
            "{label} lays out its {what} {stride} bytes apart, closer than their size, {size} bytes"
        )));
    }
    if view
        .offset()
        .checked_add(view.length())
        .is_none_or(|end| end > blob_length)
    {
        return Err(LoadError(format!(
            "{label} has {what} in a buffer view that runs past the file's binary chunk"
        )));
    }
    let end = (count - 1)
        .checked_mul(stride)
        .and_then(|bytes| bytes.checked_add(accessor.offset()))
        .and_then(|bytes| bytes.checked_add(size));
    if end.is_none_or(|end| end > view.length()) {
        return Err(LoadError(format!(
            "{label} has {what} that run past the end of their buffer view"
        )));
    }
    Ok(())
}

/// Checks that a mesh of `vertices` vertices and `indices` indices, which `label` names, fits in
/// buffers of `max_buffer_size` bytes, the GPU's largest: [`Renderer::create_mesh`] panics on a
/// larger one. A vertex takes 32 bytes there, an index 4.
fn check_buffer_sizes(
    label: &str,
    vertices: usize,
    indices: usize,
    max_buffer_size: u64,
) -> Result<(), LoadError> {
    for (count, what, size) in [(vertices, "vertices", 32), (indices, "indices", 4)] {
        let bytes = (count as u64).saturating_mul(size);
        if bytes > max_buffer_size {
            return Err(LoadError(format!(
                "{label} needs {count} {what}, {bytes} bytes; the GPU accepts buffers of up to \
                 {max_buffer_size} bytes"
            )));
        }
    }
    Ok(())
}

/// The mesh of a primitive that has no normals, with the flat normals glTF requires of it: each
/// triangle gets three vertices of its own, which take the normal of its face.
///
/// What [`Renderer::create_mesh`] would refuse is refused here, before the vertices are copied in
/// the order of the triangles: texture coordinates that are not one per position, indices that
/// are not a multiple of 3, an index that names no vertex.
fn flat_shaded(
    label: &str,
    positions: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Result<MeshData, LoadError> {
    if uvs.len() != positions.len() {
        return Err(LoadError(format!(
            "{label} has {} positions but {} texture coordinates",
            positions.len(),
            uvs.len()
        )));
    }
    let (triangles, rest) = indices.as_chunks::<3>();
    if !rest.is_empty() {
        return Err(LoadError(format!(
            "{label} has {} indices, which is not a multiple of 3",
            indices.len()
        )));
    }
    let mut data = MeshData::default();
    for triangle in triangles {
        if let Some(index) = triangle
            .iter()
            .find(|&&index| index as usize >= positions.len())
        {
            return Err(LoadError(format!(
                "{label}: index {index} names no vertex: there are {} vertices",
                positions.len()
            )));
        }
        let [a, b, c] = triangle.map(|index| Vec3::from(positions[index as usize]));
        let normal = (b - a).cross(c - a).try_normalize().unwrap_or(Vec3::Y);
        for &index in triangle {
            data.positions.push(positions[index as usize]);
            data.normals.push(normal.to_array());
            data.uvs.push(uvs[index as usize]);
        }
    }
    data.indices =
        (0..u32::try_from(data.positions.len()).expect("fewer than 4 billion vertices")).collect();
    Ok(data)
}

/// The sampler settings of a glTF texture; glTF leaves the filters open, and linear is the usual
/// choice.
///
/// `slime-gfx` gives every texture mipmaps, and its minification filter also says how levels of
/// detail are read: linear blends two, nearest reads one. Of glTF's six minification filters, only
/// `NEAREST_MIPMAP_NEAREST` and `LINEAR_MIPMAP_LINEAR` are therefore drawn as the file asks. The
/// others keep their filter between the pixels of a level but not their choice of levels:
/// `LINEAR_MIPMAP_NEAREST` blends two, `NEAREST_MIPMAP_LINEAR` reads one, and `NEAREST` and
/// `LINEAR`, which glTF reads from the full image only, read smaller levels too.
fn sampler_descriptor(sampler: &gltf::texture::Sampler<'_>) -> SamplerDescriptor {
    let wrap = |mode: WrappingMode| match mode {
        WrappingMode::ClampToEdge => AddressMode::ClampToEdge,
        WrappingMode::MirroredRepeat => AddressMode::MirrorRepeat,
        WrappingMode::Repeat => AddressMode::Repeat,
    };
    SamplerDescriptor {
        mag_filter: match sampler.mag_filter() {
            Some(MagFilter::Nearest) => Filter::Nearest,
            _ => Filter::Linear,
        },
        min_filter: match sampler.min_filter() {
            Some(
                MinFilter::Nearest
                | MinFilter::NearestMipmapNearest
                | MinFilter::NearestMipmapLinear,
            ) => Filter::Nearest,
            _ => Filter::Linear,
        },
        address_u: wrap(sampler.wrap_s()),
        address_v: wrap(sampler.wrap_t()),
    }
}

/// A decoded image: RGBA bytes, rows from top to bottom.
struct DecodedImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Decodes a PNG image embedded in the file, if neither of its sides is longer than
/// `max_dimension`, the GPU's largest texture.
fn decode_png(
    document: &gltf::Document,
    image: gltf::Image<'_>,
    blob: &[u8],
    max_dimension: u32,
) -> Result<DecodedImage, LoadError> {
    let name = image
        .name()
        .map_or_else(|| format!("image {}", image.index()), str::to_owned);
    // `Image::source` panics on what the `gltf` crate's validation lets through: an image
    // embedded without a MIME type, or with neither a buffer view nor a URI.
    let json = &document.as_json().images[image.index()];
    match (&json.buffer_view, &json.mime_type, &json.uri) {
        (Some(_), None, _) => {
            return Err(LoadError(format!("{name} is embedded without a MIME type")));
        }
        (None, _, None) => {
            return Err(LoadError(format!(
                "{name} has no data: neither a buffer view nor a URI"
            )));
        }
        _ => {}
    }
    let (bytes, mime_type) = match image.source() {
        gltf::image::Source::View { view, mime_type } => (
            view.offset()
                .checked_add(view.length())
                .and_then(|end| blob.get(view.offset()..end))
                .ok_or_else(|| LoadError(format!("{name} lies outside the file's binary chunk")))?,
            mime_type,
        ),
        gltf::image::Source::Uri { uri, .. } => {
            return Err(LoadError(format!(
                "image `{uri}` is an external file, which the loader does not read"
            )));
        }
    };
    if mime_type != "image/png" {
        return Err(LoadError(format!(
            "{name} is a {mime_type} image; only PNG images are supported"
        )));
    }
    let error = |what: &str| LoadError(format!("{name} cannot be decoded: {what}"));
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // 8 bits per channel, palettes expanded, whatever the file stores.
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| error(&e.to_string()))?;
    // Checked before the pixels are allocated, which the header alone sizes: the GPU would refuse
    // the texture anyway, and a header can claim gigabytes.
    let (width, height) = reader.info().size();
    if width.max(height) > max_dimension {
        return Err(LoadError(format!(
            "{name} is {width}x{height} pixels; the GPU accepts images up to {max_dimension} \
             pixels on a side"
        )));
    }
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| error("it is too large"))?
    ];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|e| error(&e.to_string()))?;
    buffer.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        png::ColorType::Indexed => return Err(error("its palette was not expanded")),
    };
    Ok(DecodedImage {
        width: info.width,
        height: info.height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meshes_larger_than_the_gpu_buffers_are_refused() {
        // Buffers of 320 bytes: ten vertices, or eighty indices.
        assert!(check_buffer_sizes("mesh", 10, 80, 320).is_ok());
        let error = check_buffer_sizes("mesh", 11, 3, 320)
            .unwrap_err()
            .to_string();
        assert!(error.contains("11 vertices, 352 bytes"), "{error}");
        let error = check_buffer_sizes("mesh", 3, 81, 320)
            .unwrap_err()
            .to_string();
        assert!(error.contains("81 indices, 324 bytes"), "{error}");
    }

    #[test]
    fn a_cube_of_shared_vertices_without_normals_gets_flat_faces() {
        // The corners of a unit cube, numbered by their coordinates: x + 2y + 4z.
        let positions: Vec<[f32; 3]> = (0..8u8)
            .map(|i| [i & 1, (i >> 1) & 1, (i >> 2) & 1].map(f32::from))
            .collect();
        let uvs = vec![[0.0, 0.0]; positions.len()];
        // Two triangles per face, counter-clockwise seen from outside.
        let indices = [
            0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 4, 2, 2, 4, 6, 1, 3, 5, 3, 7, 5, 0, 1, 4, 1, 5,
            4, 2, 6, 3, 3, 6, 7,
        ];
        let data = flat_shaded("cube", &positions, &uvs, &indices).expect("the cube is valid");
        assert_eq!(data.indices, (0..36).collect::<Vec<_>>());
        assert_eq!(data.uvs.len(), 36);
        let center = Vec3::splat(0.5);
        for (corners, normals) in data
            .positions
            .as_chunks::<3>()
            .0
            .iter()
            .zip(data.normals.as_chunks::<3>().0)
        {
            // The triangle's face points out of the cube along the axis on which its center lies
            // farthest from the cube's: its normal, the same for its three vertices.
            let [a, b, c] = corners.map(Vec3::from);
            let face = (((a + b + c) / 3.0 - center) * 2.0).round();
            assert_eq!(face.length(), 1.0, "{corners:?}");
            assert_eq!(*normals, [face.to_array(); 3], "{corners:?}");
        }
    }
}
