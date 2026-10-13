//! A temporary loader of glTF scenes for the `scene` example and its image test: GLB files with
//! embedded buffers and PNG images, drawn as they are (ADR-0029). The asset pipeline of Stage 4
//! replaces it with an importer and deletes it.
//!
//! It reads triangle primitives with positions, normals (computed from the faces when missing),
//! the first set of texture coordinates (zero when missing) and indices; the base color factor
//! and texture of materials, with the file's sampler settings; and the node hierarchy of the
//! default scene, flattened into instances. It refuses, with a clear error, what it cannot draw
//! right: external files, images that are not PNG, images larger than the GPU accepts, required
//! extensions, other coordinate sets, other primitive modes. It ignores, with a warning,
//! animations, skins, cameras and extensions the file uses without requiring them.
//!
//! The `gltf` crate's validation checks little more than that the file's objects name each other
//! correctly, and its readers panic on some of what it lets through. The loader refuses, rather
//! than panic on, such malformed files: images whose data lies outside the binary chunk, has no
//! MIME type or is missing.

use std::fmt;

use gltf::texture::{MagFilter, MinFilter, WrappingMode};
use slime_gfx::{AddressMode, Filter, Gpu, Sampler, SamplerDescriptor, Texture, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{Instance, Material, MaterialData, Mesh, MeshData, Renderer, SampledTexture};

/// Why a file could not be loaded.
#[derive(Debug)]
pub struct LoadError(String);

impl LoadError {
    pub fn new(message: String) -> Self {
        Self(message)
    }
}

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
    /// One per primitive of the file.
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

/// A primitive uploaded to the GPU, with its positions kept to compute the bounds.
struct LoadedPrimitive {
    mesh: usize,
    material: usize,
    positions: Vec<[f32; 3]>,
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
    for (count, what) in [
        (document.animations().len(), "animations"),
        (document.skins().len(), "skins"),
        (document.cameras().len(), "cameras"),
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

    let mut meshes = Vec::new();
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
            let normals: Vec<[f32; 3]> = match reader.read_normals() {
                Some(normals) => normals.collect(),
                None => face_normals(&positions, &indices),
            };
            let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
                Some(uvs) => uvs.into_f32().collect(),
                None => vec![[0.0, 0.0]; positions.len()],
            };
            let data = MeshData {
                positions: positions.clone(),
                normals,
                uvs,
                indices,
            };
            let gpu_mesh = renderer
                .create_mesh(gpu, &label, &data)
                .map_err(|error| LoadError(error.to_string()))?;
            own.push(primitives.len());
            primitives.push(LoadedPrimitive {
                mesh: meshes.len(),
                material: primitive.material().index().unwrap_or(default_material),
                positions,
            });
            meshes.push(gpu_mesh);
        }
        primitives_of_mesh.push(own);
    }

    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| LoadError("the file has no scene".to_owned()))?;
    let mut placements = Vec::new();
    let mut bounds = Bounds {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };
    for node in scene.nodes() {
        place(
            &node,
            Mat4::IDENTITY,
            &primitives_of_mesh,
            &primitives,
            &mut placements,
            &mut bounds,
        );
    }
    if placements.is_empty() {
        return Err(LoadError(
            "the scene draws nothing: no node has a mesh".to_owned(),
        ));
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

/// Places the primitives of `node` and of its descendants, under `parent`, the transform of the
/// node above, and grows `bounds` with them.
fn place(
    node: &gltf::Node<'_>,
    parent: Mat4,
    primitives_of_mesh: &[Vec<usize>],
    primitives: &[LoadedPrimitive],
    placements: &mut Vec<Placement>,
    bounds: &mut Bounds,
) {
    let transform = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    if let Some(mesh) = node.mesh() {
        for &index in &primitives_of_mesh[mesh.index()] {
            let primitive = &primitives[index];
            for position in &primitive.positions {
                bounds.grow(transform.transform_point3(Vec3::from(*position)));
            }
            placements.push(Placement {
                mesh: primitive.mesh,
                material: primitive.material,
                transform,
            });
        }
    }
    for child in node.children() {
        place(
            &child,
            transform,
            primitives_of_mesh,
            primitives,
            placements,
            bounds,
        );
    }
}

/// Normals for a primitive that has none: each vertex gets the average of the normals of the
/// faces that use it, which shades flat surfaces flat and curved ones smoothly enough.
fn face_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut sums = vec![Vec3::ZERO; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let [a, b, c] = triangle.map(|i| Vec3::from(positions[i as usize]));
        let normal = (b - a).cross(c - a);
        for &index in triangle {
            sums[index as usize] += normal;
        }
    }
    sums.into_iter()
        .map(|sum| sum.try_normalize().unwrap_or(Vec3::Y).to_array())
        .collect()
}

/// The sampler settings of a glTF texture; glTF leaves the filters open, and linear is the usual
/// choice.
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
