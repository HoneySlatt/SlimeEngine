//! Importing glTF files: their meshes, materials and scenes, cooked; the textures their materials
//! read, asked of the images that hold them.
//!
//! The importer reads `.gltf` files, whose buffers and images are files beside them, and GLB
//! files, which hold them in their binary chunk, and refuses buffers and images given as data URIs.
//! It reads triangle-list primitives: their positions; normals (when missing, each triangle gets
//! three vertices of its own with the normal of its face, as glTF asks); tangents (when missing,
//! or when the normals are, computed with MikkTSpace, as glTF asks); the first set of texture
//! coordinates (zero when missing); and indices (the vertices in order when missing). Each
//! primitive becomes a mesh holding only the vertices its indices name, so that primitives that
//! share their attributes do not each copy them all. It reads materials with glTF's
//! metallic-roughness model and `KHR_materials_emissive_strength`; the alpha modes `OPAQUE` and
//! `MASK`, `BLEND` read as opaque, with a warning; glTF's default material for primitives without
//! one. Each texture slot asks for the texture of its image in its role: colors for the base color
//! and emission, data for the metallic-roughness and occlusion, normals for the normal texture
//! (ADR-0071). It reads every scene's nodes, their transforms, the meshes and the lights
//! (`KHR_lights_punctual`) they place, and their `extras`: booleans, numbers and texts, other
//! values left out with a warning. Those named [`TYPED_PROPERTY`] and a type's name are the node's
//! typed components, each a value in the engine's notation, checked as written and kept in the
//! binary encoding of a document, which a world reads through its registry; [`TYPED_VERSIONS`]
//! gives the versions of their types, as a document's `$versions` does
//! (`docs/adr/0095-cooked-scene-files.md`).
//!
//! It refuses, with a message that says what and where: a file that is not valid glTF, which
//! includes one that requires an extension other than the two it reads; a file that uses
//! `KHR_texture_transform`, which it does not read; buffers and images that cannot be found or
//! lie outside their buffer; images that are neither PNG nor JPEG, or that are not sources of the
//! directory cooked; primitives that are not triangle lists, have no positions, or store an
//! attribute in a way glTF does not allow (types, sparse or empty accessors, data closer together
//! than its size or past its buffer); indices that name no vertex, or not a multiple of 3; values
//! that are not finite numbers, which glTF forbids; a texture read through another set of texture
//! coordinates than the first; a node hierarchy that is not a set of trees; materials and lights
//! that glTF forbids; a typed property without a type's name, whose value is not a boolean, a
//! number or a text of one line that holds one value in the engine's notation; meshes larger than
//! the GPU takes, and files whose meshes together pass [`MAX_FILE_VERTICES`] or
//! [`MAX_FILE_INDICES`], checked before their vertices are copied. It warns about, and leaves out:
//! animations, skins, cameras, vertex colors, and the extensions a file uses that it does not read.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::slice;

use glam::Vec3;
use gltf::Semantic;
use gltf::accessor::{DataType, Dimensions};
use gltf::khr_lights_punctual::Kind;
use gltf::texture::{MagFilter, MinFilter, WrappingMode};
use slime_assets::{
    AssetId, CookedMaterial, CookedMesh, CookedScene, Extra, LightKind, MaterialTexture,
    SceneLight, SceneNode, TextureFilter, TextureSampler, TextureWrap,
};
use slime_reflect::Registry;
use slime_serialize::{Document, Mode};

use crate::ids::{SubAssetKind, default_material_id, sub_asset_ids, texture_id};
use crate::image::ImageKind;
use crate::texture::TextureRole;

/// The most vertices one mesh takes: those of 48 bytes that the GPU's largest buffer holds with
/// the engine's default limits, 256 MiB.
pub const MAX_MESH_VERTICES: usize = 5_592_405;
/// The most indices one mesh takes: those of 4 bytes that the same buffer holds.
pub const MAX_MESH_INDICES: usize = 67_108_864;
/// The most vertices all the meshes of one file take together, 768 MiB of them: a file of a few
/// megabytes that names one primitive many times, or makes millions of triangles of three
/// vertices, is refused before they are copied.
pub const MAX_FILE_VERTICES: usize = 16 * 1024 * 1024;
/// The most indices all the meshes of one file take together, 256 MiB of them.
pub const MAX_FILE_INDICES: usize = 64 * 1024 * 1024;

/// What starts the name of a node's custom property that holds one of its typed components, before
/// the name of the component's type: `slime:hunnawood::Collider`.
pub const TYPED_PROPERTY: &str = "slime:";

/// The name of a node's custom property that gives the versions of the types that its typed
/// components are written in, past version 1, as the `$versions` of a document does:
/// `{ "hunnawood::Collider" = 2 }` (`docs/adr/0086-serialization.md`). A type that it leaves out is
/// at version 1.
pub const TYPED_VERSIONS: &str = "slime:$versions";

/// The extensions the importer reads; the `gltf` crate is built with their features.
const READ_EXTENSIONS: [&str; 2] = ["KHR_lights_punctual", "KHR_materials_emissive_strength"];

/// Why a glTF file cannot be imported: what is wrong, and where in the file.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ImportError(pub String);

fn refuse<T>(message: String) -> Result<T, ImportError> {
    Err(ImportError(message))
}

/// A glTF file, opened: its document, validated, and the bytes of each of its buffers.
pub struct GltfFile {
    document: gltf::Document,
    buffers: Vec<Vec<u8>>,
    /// The directory of the file, which its buffers' and images' paths start from.
    directory: PathBuf,
}

/// A texture slot of a material: its name, the texture it reads with its set of texture
/// coordinates, if any, and the role its texture is cooked for.
type Slot<'a> = (&'static str, Option<(gltf::Texture<'a>, u32)>, TextureRole);

/// An image embedded in a glTF file, which its materials read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedImage {
    /// Its ID, a sub-asset of the file.
    pub id: AssetId,
    /// Its name, for messages.
    pub name: String,
    /// What kind of image file its bytes are.
    pub kind: ImageKind,
    /// Its bytes, as the file holds them.
    pub bytes: Vec<u8>,
    /// The roles the materials read it in, each once.
    pub roles: Vec<TextureRole>,
}

/// What importing a glTF file gives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imported {
    /// One mesh per primitive of each glTF mesh.
    pub meshes: Vec<(AssetId, CookedMesh)>,
    /// One material per glTF material, and glTF's default material if a primitive has none.
    pub materials: Vec<(AssetId, CookedMaterial)>,
    /// One scene per glTF scene: the default one, or the first, takes the file's own ID.
    pub scenes: Vec<(AssetId, CookedScene)>,
    /// The images embedded in the file that its materials read, with their roles.
    pub embedded: Vec<EmbeddedImage>,
    /// The image files beside the file that its materials read: their IDs, as sources of the
    /// directory cooked, with each role, each pair once.
    pub external: Vec<(AssetId, TextureRole)>,
    /// What the file has that the importer leaves out.
    pub warnings: Vec<String>,
}

/// `path` with its `.` and `..` components resolved, without asking the file system.
pub(crate) fn lexical(path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    resolved.push("..");
                }
            }
            other => resolved.push(other),
        }
    }
    resolved
}

/// `uri`, a relative URI, with its `%XX` escapes decoded, as glTF writes paths with spaces.
fn decode_uri(uri: &str) -> String {
    let bytes = uri.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u8::from_str_radix(digits, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// The refusal of `name`, a buffer or an image given as a data URI, which can hold megabytes of
/// text and is therefore not quoted.
fn data_uri(name: &str) -> ImportError {
    ImportError(format!(
        "{name} is given as a data URI, which the importer does not decode"
    ))
}

/// A name for messages: the object's own, or its kind and index.
fn named(name: Option<&str>, kind: &str, index: usize) -> String {
    name.map_or_else(|| format!("{kind} {index}"), str::to_owned)
}

impl GltfFile {
    /// Opens the glTF file `path`, whose bytes are `bytes`: parses and validates it, and reads its
    /// buffers, from its binary chunk or from the files beside it.
    ///
    /// # Errors
    ///
    /// [`ImportError`] if the file is not valid glTF, or a buffer cannot be read.
    pub fn open(path: &Path, bytes: &[u8]) -> Result<Self, ImportError> {
        // The `gltf` crate subtracts the header's 12 bytes from the length a GLB header declares
        // without checking it first, an overflow that panics in debug builds.
        let declared = bytes
            .strip_prefix(b"glTF")
            .and_then(|rest| rest.get(4..)?.first_chunk::<4>().copied())
            .map(u32::from_le_bytes);
        if let Some(length @ 0..12) = declared {
            return refuse(format!(
                "the file is not valid glTF: its GLB header declares a length of {length} bytes, \
                 shorter than the header, 12 bytes"
            ));
        }
        let not_valid = |error| ImportError(format!("the file is not valid glTF: {error}"));
        let gltf::Gltf {
            document: unvalidated,
            blob,
        } = gltf::Gltf::from_slice_without_validation(bytes).map_err(not_valid)?;
        check_positions_exist(unvalidated.as_json())?;
        let document = gltf::Document::from_json(unvalidated.into_json()).map_err(not_valid)?;
        let directory = lexical(path.parent().unwrap_or(Path::new("")));
        let mut buffers = Vec::new();
        for buffer in document.buffers() {
            let name = named(buffer.name(), "buffer", buffer.index());
            let bytes = match buffer.source() {
                gltf::buffer::Source::Bin => blob.clone().ok_or_else(|| {
                    ImportError(format!("{name} is the binary chunk, which the file lacks"))
                })?,
                gltf::buffer::Source::Uri(uri) if uri.starts_with("data:") => {
                    return Err(data_uri(&name));
                }
                gltf::buffer::Source::Uri(uri) => {
                    let file = lexical(&directory.join(decode_uri(uri)));
                    fs::read(&file).map_err(|error| {
                        ImportError(format!(
                            "{name}, the file `{}`, cannot be read: {error}",
                            file.display()
                        ))
                    })?
                }
            };
            if bytes.len() < buffer.length() {
                return refuse(format!(
                    "{name} holds {} bytes, fewer than the {} the file declares",
                    bytes.len(),
                    buffer.length()
                ));
            }
            buffers.push(bytes);
        }
        Ok(Self {
            document,
            buffers,
            directory,
        })
    }

    /// The bytes of each buffer, in the file's order.
    pub fn buffers(&self) -> &[Vec<u8>] {
        &self.buffers
    }

    /// The image files beside the file that it names, by their paths, with `.` and `..` resolved.
    pub fn image_files(&self) -> Vec<PathBuf> {
        self.document
            .as_json()
            .images
            .iter()
            .filter_map(|image| image.uri.as_deref())
            .filter(|uri| !uri.starts_with("data:"))
            .map(|uri| lexical(&self.directory.join(decode_uri(uri))))
            .collect()
    }

    /// Imports the file, whose ID is `file`; `image_ids` gives the ID of each source image of the
    /// directory cooked, by its path as [`GltfFile::image_files`] gives it.
    ///
    /// # Errors
    ///
    /// [`ImportError`] for what the module refuses.
    pub fn import(
        &self,
        file: AssetId,
        image_ids: &HashMap<PathBuf, AssetId>,
    ) -> Result<Imported, ImportError> {
        let document = &self.document;
        let mut imported = Imported::default();
        self.warn_about_what_is_left_out(&mut imported.warnings)?;

        // Every image's ID: embedded ones are sub-assets of the file, files beside it sources.
        let image_names: Vec<Option<&str>> = document.images().map(|image| image.name()).collect();
        let image_sub_ids = sub_asset_ids(file, SubAssetKind::Image, &image_names);
        warn_about_names("images", &image_names, &mut imported.warnings);

        let material_names: Vec<Option<&str>> = document
            .materials()
            .map(|material| material.name())
            .collect();
        let material_ids = sub_asset_ids(file, SubAssetKind::Material, &material_names);
        warn_about_names("materials", &material_names, &mut imported.warnings);
        let mut embedded: HashMap<usize, EmbeddedImage> = HashMap::new();
        for (material, id) in document.materials().zip(&material_ids) {
            let cooked = self.material(
                &material,
                &image_sub_ids,
                image_ids,
                &mut embedded,
                &mut imported,
            )?;
            imported.materials.push((id.id, cooked));
        }
        let mut embedded: Vec<EmbeddedImage> = embedded.into_values().collect();
        embedded.sort_by_key(|image| image.id);
        imported.embedded = embedded;

        let mesh_names: Vec<Option<&str>> = document.meshes().map(|mesh| mesh.name()).collect();
        let mesh_ids = sub_asset_ids(file, SubAssetKind::Mesh, &mesh_names);
        warn_about_names("meshes", &mesh_names, &mut imported.warnings);
        check_sizes(document)?;
        let mut needs_default = false;
        let mut primitive_ids = Vec::new();
        for (mesh, mesh_id) in document.meshes().zip(&mesh_ids) {
            let count = mesh.primitives().len();
            let ids = sub_asset_ids(mesh_id.id, SubAssetKind::Mesh, &vec![None; count]);
            let mut own = Vec::new();
            for (primitive, id) in mesh.primitives().zip(ids) {
                let material = match primitive.material().index() {
                    Some(index) => material_ids[index].id,
                    None => {
                        needs_default = true;
                        default_material_id(file)
                    }
                };
                let label = format!(
                    "{} primitive {}",
                    named(mesh.name(), "mesh", mesh.index()),
                    primitive.index()
                );
                let cooked = self.primitive(&primitive, label, material)?;
                imported.meshes.push((id.id, cooked));
                own.push(id.id);
            }
            primitive_ids.push(own);
        }
        if needs_default {
            imported
                .materials
                .push((default_material_id(file), CookedMaterial::default()));
        }

        let scene_names: Vec<Option<&str>> = document.scenes().map(|scene| scene.name()).collect();
        let scene_ids = sub_asset_ids(file, SubAssetKind::Scene, &scene_names);
        let default_scene = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .map(|scene| scene.index());
        for (scene, id) in document.scenes().zip(&scene_ids) {
            let id = if Some(scene.index()) == default_scene {
                file
            } else {
                id.id
            };
            let cooked = self.scene(&scene, &primitive_ids, &mut imported.warnings)?;
            imported.scenes.push((id, cooked));
        }
        Ok(imported)
    }

    /// Warns about what the file has that the importer leaves out, and refuses
    /// `KHR_texture_transform`, which would draw textures wrong.
    fn warn_about_what_is_left_out(&self, warnings: &mut Vec<String>) -> Result<(), ImportError> {
        let document = &self.document;
        if document
            .extensions_used()
            .any(|name| name == "KHR_texture_transform")
        {
            return refuse(
                "the file uses KHR_texture_transform, which the importer does not read: bake the \
                 transform into the texture coordinates"
                    .to_owned(),
            );
        }
        let ignored: Vec<&str> = document
            .extensions_used()
            .filter(|name| !READ_EXTENSIONS.contains(name))
            .collect();
        if !ignored.is_empty() {
            warnings.push(format!(
                "the file uses extensions that the importer leaves out: {}",
                ignored.join(", ")
            ));
        }
        let colored = document
            .meshes()
            .flat_map(|mesh| mesh.primitives())
            .filter(|primitive| primitive.get(&Semantic::Colors(0)).is_some())
            .count();
        let blended = document
            .materials()
            .filter(|material| material.alpha_mode() == gltf::material::AlphaMode::Blend)
            .count();
        for (count, what) in [
            (document.animations().len(), "animations"),
            (document.skins().len(), "skins"),
            (document.cameras().len(), "cameras"),
            (colored, "primitives with vertex colors"),
            (
                blended,
                "materials whose alpha mode is BLEND, read as opaque",
            ),
        ] {
            if count > 0 {
                warnings.push(format!(
                    "the file has {count} {what}, which the importer leaves out"
                ));
            }
        }
        Ok(())
    }

    /// The cooked material of `material`: its values, and for each texture slot the ID of the
    /// texture of its image in the slot's role, which it adds to the images asked for.
    fn material(
        &self,
        material: &gltf::Material<'_>,
        image_sub_ids: &[crate::ids::SubAssetId],
        image_ids: &HashMap<PathBuf, AssetId>,
        embedded: &mut HashMap<usize, EmbeddedImage>,
        imported: &mut Imported,
    ) -> Result<CookedMaterial, ImportError> {
        let name = named(material.name(), "material", material.index().unwrap_or(0));
        let pbr = material.pbr_metallic_roughness();
        let slots: [Slot<'_>; 5] = [
            (
                "base color",
                pbr.base_color_texture()
                    .map(|info| (info.texture(), info.tex_coord())),
                TextureRole::Color,
            ),
            (
                "metallic-roughness",
                pbr.metallic_roughness_texture()
                    .map(|info| (info.texture(), info.tex_coord())),
                TextureRole::Data,
            ),
            (
                "normal",
                material
                    .normal_texture()
                    .map(|normal| (normal.texture(), normal.tex_coord())),
                TextureRole::Normal,
            ),
            (
                "occlusion",
                material
                    .occlusion_texture()
                    .map(|occlusion| (occlusion.texture(), occlusion.tex_coord())),
                TextureRole::Data,
            ),
            (
                "emissive",
                material
                    .emissive_texture()
                    .map(|info| (info.texture(), info.tex_coord())),
                TextureRole::Color,
            ),
        ];
        let mut textures = [None; 5];
        for (index, (slot, texture, role)) in slots.into_iter().enumerate() {
            let Some((texture, tex_coord)) = texture else {
                continue;
            };
            if tex_coord != 0 {
                return refuse(format!(
                    "material `{name}` reads its {slot} texture through coordinate set \
                     {tex_coord}; only the first set is read"
                ));
            }
            let image = self.image(
                texture.source(),
                role,
                image_sub_ids,
                image_ids,
                embedded,
                imported,
            )?;
            textures[index] = Some(MaterialTexture {
                texture: texture_id(image, role),
                sampler: sampler(&texture.sampler()),
            });
        }
        let [
            base_color_texture,
            metallic_roughness_texture,
            normal_texture,
            occlusion_texture,
            emissive_texture,
        ] = textures;
        let cooked = CookedMaterial {
            name,
            base_color: pbr.base_color_factor(),
            base_color_texture,
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            metallic_roughness_texture,
            normal_texture,
            normal_scale: material
                .normal_texture()
                .map_or(1.0, |normal| normal.scale()),
            occlusion_texture,
            occlusion_strength: material
                .occlusion_texture()
                .map_or(1.0, |occlusion| occlusion.strength()),
            emissive: material.emissive_factor(),
            emissive_strength: material.emissive_strength().unwrap_or(1.0),
            emissive_texture,
            alpha_cutoff: match material.alpha_mode() {
                gltf::material::AlphaMode::Mask => Some(material.alpha_cutoff().unwrap_or(0.5)),
                // Blending is not drawn: warned about.
                gltf::material::AlphaMode::Opaque | gltf::material::AlphaMode::Blend => None,
            },
            double_sided: material.double_sided(),
        };
        check_factors(&cooked)?;
        Ok(cooked)
    }

    /// The ID of `image`, read in `role`: an image embedded in the file, which it adds to
    /// `embedded` with the role, or an image file beside it, which it adds to the external images
    /// asked for.
    fn image(
        &self,
        image: gltf::Image<'_>,
        role: TextureRole,
        image_sub_ids: &[crate::ids::SubAssetId],
        image_ids: &HashMap<PathBuf, AssetId>,
        embedded: &mut HashMap<usize, EmbeddedImage>,
        imported: &mut Imported,
    ) -> Result<AssetId, ImportError> {
        let name = named(image.name(), "image", image.index());
        // `Image::source` panics on what the `gltf` crate's validation lets through: an image
        // embedded without a MIME type, or with neither a buffer view nor a URI.
        let json = &self.document.as_json().images[image.index()];
        match (&json.buffer_view, &json.mime_type, &json.uri) {
            (Some(_), None, _) => return refuse(format!("{name} is embedded without a MIME type")),
            (None, _, None) => {
                return refuse(format!(
                    "{name} has no data: neither a buffer view nor a URI"
                ));
            }
            _ => {}
        }
        match image.source() {
            gltf::image::Source::View { view, mime_type } => {
                if let Some(found) = embedded.get_mut(&image.index()) {
                    if !found.roles.contains(&role) {
                        found.roles.push(role);
                    }
                    return Ok(found.id);
                }
                let kind = match mime_type {
                    "image/png" => ImageKind::Png,
                    "image/jpeg" => ImageKind::Jpeg,
                    other => {
                        return refuse(format!(
                            "{name} is a {other} image; only PNG and JPEG images are read"
                        ));
                    }
                };
                let buffer = &self.buffers[view.buffer().index()];
                let bytes = view
                    .offset()
                    .checked_add(view.length())
                    .and_then(|end| buffer.get(view.offset()..end))
                    .ok_or_else(|| ImportError(format!("{name} lies outside its buffer")))?;
                let id = image_sub_ids[image.index()].id;
                embedded.insert(
                    image.index(),
                    EmbeddedImage {
                        id,
                        name,
                        kind,
                        bytes: bytes.to_vec(),
                        roles: vec![role],
                    },
                );
                Ok(id)
            }
            gltf::image::Source::Uri { uri, .. } if uri.starts_with("data:") => {
                Err(data_uri(&name))
            }
            gltf::image::Source::Uri { uri, .. } => {
                let path = lexical(&self.directory.join(decode_uri(uri)));
                let id = *image_ids.get(&path).ok_or_else(|| {
                    ImportError(format!(
                        "{name} is the file `{uri}`, which is not a PNG or JPEG source of the \
                         directory cooked"
                    ))
                })?;
                if !imported.external.contains(&(id, role)) {
                    imported.external.push((id, role));
                }
                Ok(id)
            }
        }
    }

    /// The cooked mesh of `primitive`, named `label`, drawn with `material`.
    fn primitive(
        &self,
        primitive: &gltf::Primitive<'_>,
        label: String,
        material: AssetId,
    ) -> Result<CookedMesh, ImportError> {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            return refuse(format!(
                "{label} is drawn as {:?}; only triangle lists are read",
                primitive.mode()
            ));
        }
        // The `gltf` crate's readers panic on some accessors its validation lets through: what
        // they would read is checked first.
        let accessors: [(_, _, _, &[_]); 5] = [
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
                "tangents",
                primitive.get(&Semantic::Tangents),
                Dimensions::Vec4,
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
                check_accessor(
                    &label,
                    what,
                    &accessor,
                    dimensions,
                    data_types,
                    &self.buffers,
                )?;
            }
        }
        let reader = primitive.reader(|buffer| Some(&self.buffers[buffer.index()][..]));
        let positions: Vec<[f32; 3]> = reader
            .read_positions()
            .ok_or_else(|| ImportError(format!("{label} has no positions")))?
            .collect();
        let vertex_count = positions.len();
        let indices: Vec<u32> = match reader.read_indices() {
            Some(indices) => indices.into_u32().collect(),
            None => {
                (0..u32::try_from(vertex_count).expect("fewer than 4 billion vertices")).collect()
            }
        };
        if indices.is_empty() || !indices.len().is_multiple_of(3) {
            return refuse(format!(
                "{label} has {} indices, not a positive multiple of 3",
                indices.len()
            ));
        }
        if let Some(index) = indices
            .iter()
            .find(|&&index| index as usize >= vertex_count)
        {
            return refuse(format!(
                "{label}: index {index} names no vertex: there are {vertex_count} vertices"
            ));
        }
        let uvs: Vec<[f32; 2]> = match reader.read_tex_coords(0) {
            Some(uvs) => uvs.into_f32().collect(),
            None => vec![[0.0, 0.0]; vertex_count],
        };
        let normals: Option<Vec<[f32; 3]>> = reader.read_normals().map(Iterator::collect);
        // glTF: a primitive without normals ignores its tangents, since its normals are computed.
        let tangents: Option<Vec<[f32; 4]>> = match normals {
            Some(_) => reader.read_tangents().map(Iterator::collect),
            None => None,
        };
        for (what, count) in [
            ("texture coordinates", Some(uvs.len())),
            ("normals", normals.as_ref().map(Vec::len)),
            ("tangents", tangents.as_ref().map(Vec::len)),
        ] {
            if let Some(count) = count
                && count != vertex_count
            {
                return refuse(format!(
                    "{label} has {vertex_count} positions but {count} {what}"
                ));
            }
        }

        let finite = positions
            .iter()
            .flatten()
            .chain(uvs.as_flattened())
            .chain(normals.iter().flat_map(|normals| normals.as_flattened()))
            .chain(tangents.iter().flat_map(|tangents| tangents.as_flattened()))
            .all(|value| value.is_finite());
        if !finite {
            return refuse(format!(
                "{label} has a vertex value that is not a finite number, which glTF forbids"
            ));
        }

        let mut mesh = match &normals {
            Some(normals) => compacted(&positions, normals, &uvs, tangents.as_deref(), &indices),
            None => flat_shaded(&positions, &uvs, &indices),
        };
        if tangents.is_none() {
            with_mikktspace_tangents(&mut mesh);
        }
        let (min, max) = mesh.positions.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(min, max), &position| (min.min(position.into()), max.max(position.into())),
        );
        Ok(CookedMesh {
            name: label,
            material,
            positions: mesh.positions,
            normals: mesh.normals,
            uvs: mesh.uvs,
            tangents: mesh.tangents,
            indices: mesh.indices,
            bounds: [min.to_array(), max.to_array()],
        })
    }

    /// The cooked scene of `scene`: its nodes walked depth first, in the file's order, each after
    /// its parent, with a stack of its own rather than by recursion, since a valid file can nest
    /// nodes deeper than a thread's stack holds calls.
    fn scene(
        &self,
        scene: &gltf::Scene<'_>,
        primitive_ids: &[Vec<AssetId>],
        warnings: &mut Vec<String>,
    ) -> Result<CookedScene, ImportError> {
        let mut nodes = Vec::new();
        let mut reached = vec![false; self.document.nodes().len()];
        // The nodes left to walk, the next one last, each with its parent's index.
        let mut stack: Vec<(gltf::Node<'_>, Option<u32>)> =
            scene.nodes().map(|node| (node, None)).collect();
        stack.reverse();
        while let Some((node, parent)) = stack.pop() {
            if std::mem::replace(&mut reached[node.index()], true) {
                return refuse(format!(
                    "node {} is reached twice: the node hierarchy must be a set of trees",
                    node.index()
                ));
            }
            let transform: [f32; 16] = *node
                .transform()
                .matrix()
                .as_flattened()
                .first_chunk()
                .expect("16 numbers");
            if !transform.iter().all(|value| value.is_finite()) {
                return refuse(format!(
                    "node {}'s transform is not made of finite numbers, which glTF forbids",
                    node.index()
                ));
            }
            let (extras, typed) = extras(&node, warnings);
            let components = components(&named(node.name(), "node", node.index()), &typed)?;
            let index = u32::try_from(nodes.len()).expect("fewer than 4 billion nodes");
            nodes.push(SceneNode {
                name: node.name().unwrap_or_default().to_owned(),
                parent,
                transform,
                meshes: node
                    .mesh()
                    .map(|mesh| primitive_ids[mesh.index()].clone())
                    .unwrap_or_default(),
                light: node
                    .light()
                    .map(|light| read_light(&light, node.index()))
                    .transpose()?,
                extras,
                components,
            });
            // Reversed on the stack, so that the first child and its descendants come next.
            let children = stack.len();
            stack.extend(node.children().map(|child| (child, Some(index))));
            stack[children..].reverse();
        }
        Ok(CookedScene {
            name: named(scene.name(), "scene", scene.index()),
            nodes,
        })
    }
}

/// Refuses a file whose meshes are larger than the importer takes, one by one or together, from
/// the counts its accessors declare, before anything is read: a mesh without normals takes three
/// vertices per triangle, one with them at most its positions.
fn check_sizes(document: &gltf::Document) -> Result<(), ImportError> {
    let (mut vertices, mut indices) = (0, 0);
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let label = format!(
                "{} primitive {}",
                named(mesh.name(), "mesh", mesh.index()),
                primitive.index()
            );
            let positions = primitive
                .get(&Semantic::Positions)
                .map_or(0, |accessor| accessor.count());
            let own_indices = primitive
                .indices()
                .map_or(positions, |accessor| accessor.count());
            let own_vertices = if primitive.get(&Semantic::Normals).is_some() {
                positions
            } else {
                own_indices
            };
            if own_vertices > MAX_MESH_VERTICES || own_indices > MAX_MESH_INDICES {
                return refuse(format!(
                    "{label} needs {own_vertices} vertices and {own_indices} indices; a mesh takes \
                     up to {MAX_MESH_VERTICES} vertices and {MAX_MESH_INDICES} indices, what the \
                     GPU's largest buffer holds"
                ));
            }
            vertices += own_vertices;
            indices += own_indices;
            if vertices > MAX_FILE_VERTICES || indices > MAX_FILE_INDICES {
                return refuse(format!(
                    "the file's meshes, up to {label}, need {vertices} vertices and {indices} \
                     indices; a file takes up to {MAX_FILE_VERTICES} vertices and \
                     {MAX_FILE_INDICES} indices"
                ));
            }
        }
    }
    Ok(())
}

/// Warns about the `names` of the `what` of a file that are missing or repeated: those take their
/// index in their IDs, which change when objects are added or removed before them.
fn warn_about_names(what: &str, names: &[Option<&str>], warnings: &mut Vec<String>) {
    let by_index = names
        .iter()
        .filter(|name| {
            name.is_none_or(|name| {
                name.is_empty() || names.iter().filter(|other| **other == Some(name)).count() > 1
            })
        })
        .count();
    if by_index > 0 {
        warnings.push(format!(
            "{by_index} {what} of the file have no name of their own: their IDs follow their \
             order in the file, and change when {what} are added or removed before them"
        ));
    }
}

/// Checks that the positions of every primitive of `json`, a file not validated yet, name an
/// accessor of the file. The `gltf` crate's validation reports any other index that names
/// nothing, but reads this accessor without checking that it exists, and panics on one that does
/// not.
fn check_positions_exist(json: &gltf::json::Root) -> Result<(), ImportError> {
    for (index, mesh) in json.meshes.iter().enumerate() {
        for (primitive, data) in mesh.primitives.iter().enumerate() {
            if let Some(accessor) = data
                .attributes
                .get(&gltf::json::validation::Checked::Valid(Semantic::Positions))
                && accessor.value() >= json.accessors.len()
            {
                let name = named(mesh.name.as_deref(), "mesh", index);
                return refuse(format!(
                    "the file is not valid glTF: {name} primitive {primitive} reads its positions \
                     from accessor {accessor}, which does not exist"
                ));
            }
        }
    }
    Ok(())
}

/// Checks that `accessor`, which holds the `what` of `label`, stores them as `dimensions` of one
/// of `data_types`, laid out so that the `gltf` crate reads them without panicking, inside its
/// buffer.
fn check_accessor(
    label: &str,
    what: &str,
    accessor: &gltf::Accessor<'_>,
    dimensions: Dimensions,
    data_types: &[DataType],
    buffers: &[Vec<u8>],
) -> Result<(), ImportError> {
    let data_type = accessor.data_type();
    if accessor.dimensions() != dimensions || !data_types.contains(&data_type) {
        return refuse(format!(
            "{label} stores its {what} as {data_type:?} {:?}, which the importer does not read",
            accessor.dimensions()
        ));
    }
    // A sparse accessor needs no buffer view, and its reader then allocates at once as many
    // elements as the file claims, which no data backs. An accessor that is neither sparse nor in
    // a buffer view, zeros in glTF, never gets here: the `gltf` crate's validation refuses it.
    let (None, Some(view)) = (accessor.sparse(), accessor.view()) else {
        return refuse(format!(
            "{label} stores its {what} in a sparse accessor, which the importer does not read"
        ));
    };
    let count = accessor.count();
    if count == 0 {
        return refuse(format!("{label} has an empty accessor of {what}"));
    }
    let size = accessor.size();
    let stride = view.stride().unwrap_or(size);
    if stride < size {
        return refuse(format!(
            "{label} lays out its {what} {stride} bytes apart, closer than their size, {size} bytes"
        ));
    }
    let buffer_length = buffers[view.buffer().index()].len();
    if view
        .offset()
        .checked_add(view.length())
        .is_none_or(|end| end > buffer_length)
    {
        return refuse(format!(
            "{label} has {what} in a buffer view that runs past the end of its buffer"
        ));
    }
    let end = (count - 1)
        .checked_mul(stride)
        .and_then(|bytes| bytes.checked_add(accessor.offset()))
        .and_then(|bytes| bytes.checked_add(size));
    if end.is_none_or(|end| end > view.length()) {
        return refuse(format!(
            "{label} has {what} that run past the end of their buffer view"
        ));
    }
    Ok(())
}

/// The arrays of a mesh being cooked.
struct MeshArrays {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    tangents: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

/// The vertices that `indices` name, in the order they are first named, and the indices into
/// them: a primitive that shares its attributes with others keeps only its own vertices. `tangents`
/// are kept if the file gives them.
fn compacted(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    tangents: Option<&[[f32; 4]]>,
    indices: &[u32],
) -> MeshArrays {
    let mut remap = vec![u32::MAX; positions.len()];
    let mut mesh = MeshArrays {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        tangents: Vec::new(),
        indices: Vec::with_capacity(indices.len()),
    };
    for &index in indices {
        let vertex = index as usize;
        if remap[vertex] == u32::MAX {
            remap[vertex] = mesh.positions.len() as u32;
            mesh.positions.push(positions[vertex]);
            mesh.normals.push(normals[vertex]);
            mesh.uvs.push(uvs[vertex]);
            if let Some(tangents) = tangents {
                mesh.tangents.push(tangents[vertex]);
            }
        }
        mesh.indices.push(remap[vertex]);
    }
    mesh
}

/// The mesh of a primitive that has no normals, with the flat normals glTF requires of it: each
/// triangle gets three vertices of its own, which take the normal of its face.
fn flat_shaded(positions: &[[f32; 3]], uvs: &[[f32; 2]], indices: &[u32]) -> MeshArrays {
    let mut mesh = MeshArrays {
        positions: Vec::with_capacity(indices.len()),
        normals: Vec::with_capacity(indices.len()),
        uvs: Vec::with_capacity(indices.len()),
        tangents: Vec::new(),
        indices: (0..u32::try_from(indices.len()).expect("fewer than 4 billion indices")).collect(),
    };
    for triangle in indices.as_chunks::<3>().0 {
        let [a, b, c] = triangle.map(|index| Vec3::from(positions[index as usize]));
        let normal = (b - a).cross(c - a).try_normalize().unwrap_or(Vec3::Y);
        for &index in triangle {
            mesh.positions.push(positions[index as usize]);
            mesh.normals.push(normal.to_array());
            mesh.uvs.push(uvs[index as usize]);
        }
    }
    mesh
}

/// The mesh's corners as MikkTSpace reads them, and the tangent it gives each corner.
struct Corners<'a> {
    mesh: &'a MeshArrays,
    tangents: Vec<[f32; 4]>,
}

impl Corners<'_> {
    fn vertex(&self, face: usize, corner: usize) -> usize {
        self.mesh.indices[face * 3 + corner] as usize
    }
}

impl bevy_mikktspace::Geometry for Corners<'_> {
    fn num_faces(&self) -> usize {
        self.mesh.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, corner: usize) -> [f32; 3] {
        self.mesh.positions[self.vertex(face, corner)]
    }

    fn normal(&self, face: usize, corner: usize) -> [f32; 3] {
        self.mesh.normals[self.vertex(face, corner)]
    }

    fn tex_coord(&self, face: usize, corner: usize) -> [f32; 2] {
        // glTF counts v down the image, MikkTSpace up it, as Blender, whose tangents normal
        // textures are baked against: turned over, the handedness comes out as glTF's.
        let [u, v] = self.mesh.uvs[self.vertex(face, corner)];
        [u, 1.0 - v]
    }

    fn set_tangent(
        &mut self,
        tangent_space: Option<bevy_mikktspace::TangentSpace>,
        face: usize,
        corner: usize,
    ) {
        let tangent = match tangent_space {
            Some(space) => space.tangent_encoded(),
            // A corner of degenerate triangles only: any direction across its normal.
            None => {
                let normal = Vec3::from(self.normal(face, corner));
                let across = normal
                    .try_normalize()
                    .unwrap_or(Vec3::Y)
                    .any_orthonormal_vector();
                [across.x, across.y, across.z, 1.0]
            }
        };
        self.tangents[face * 3 + corner] = tangent;
    }
}

/// Gives `mesh` the tangents of MikkTSpace, against which normal textures are baked: one per
/// corner of each triangle; a vertex whose corners get different tangents is split into one vertex
/// per tangent.
fn with_mikktspace_tangents(mesh: &mut MeshArrays) {
    let mut corners = Corners {
        mesh,
        tangents: vec![[0.0; 4]; mesh.indices.len()],
    };
    // It fails only on a mesh of no triangle, or of more than it can number, which the limits
    // refuse before.
    bevy_mikktspace::generate_tangents(&mut corners)
        .expect("MikkTSpace takes every mesh of the importer's limits");
    let tangents = corners.tangents;
    let mut split = MeshArrays {
        positions: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        tangents: Vec::new(),
        indices: Vec::with_capacity(mesh.indices.len()),
    };
    let mut made: HashMap<(u32, [u32; 4]), u32> = HashMap::new();
    for (corner, &vertex) in mesh.indices.iter().enumerate() {
        let tangent = tangents[corner];
        let key = (vertex, tangent.map(f32::to_bits));
        let index = *made.entry(key).or_insert_with(|| {
            let v = vertex as usize;
            split.positions.push(mesh.positions[v]);
            split.normals.push(mesh.normals[v]);
            split.uvs.push(mesh.uvs[v]);
            split.tangents.push(tangent);
            (split.positions.len() - 1) as u32
        });
        split.indices.push(index);
    }
    *mesh = split;
}

/// The sampler of a glTF texture; glTF leaves the filters open, and linear is the usual choice.
///
/// The GPU layer ties how levels of detail are read to the minification filter: linear blends
/// two, nearest reads one. Of glTF's six minification filters, only `NEAREST_MIPMAP_NEAREST` and
/// `LINEAR_MIPMAP_LINEAR` are therefore drawn as the file asks; the others keep their filter
/// between the texels of a level but not their choice of levels.
fn sampler(sampler: &gltf::texture::Sampler<'_>) -> TextureSampler {
    let wrap = |mode: WrappingMode| match mode {
        WrappingMode::ClampToEdge => TextureWrap::ClampToEdge,
        WrappingMode::MirroredRepeat => TextureWrap::MirroredRepeat,
        WrappingMode::Repeat => TextureWrap::Repeat,
    };
    TextureSampler {
        mag_filter: match sampler.mag_filter() {
            Some(MagFilter::Nearest) => TextureFilter::Nearest,
            _ => TextureFilter::Linear,
        },
        min_filter: match sampler.min_filter() {
            Some(
                MinFilter::Nearest
                | MinFilter::NearestMipmapNearest
                | MinFilter::NearestMipmapLinear,
            ) => TextureFilter::Nearest,
            _ => TextureFilter::Linear,
        },
        wrap_u: wrap(sampler.wrap_s()),
        wrap_v: wrap(sampler.wrap_t()),
    }
}

/// Refuses the material `cooked` if glTF does not allow its factors, which the `gltf` crate does
/// not check, and which the renderer refuses too (`slime_render::MaterialValues`): a base color or
/// emissive channel, a metallic or roughness factor or an occlusion strength outside 0 to 1; an
/// emissive strength (`KHR_materials_emissive_strength`) or an alpha cutoff below 0.
fn check_factors(cooked: &CookedMaterial) -> Result<(), ImportError> {
    let unit: [(&str, &[f32]); 5] = [
        ("a base color of", &cooked.base_color),
        ("a metallic factor of", slice::from_ref(&cooked.metallic)),
        ("a roughness factor of", slice::from_ref(&cooked.roughness)),
        (
            "an occlusion strength of",
            slice::from_ref(&cooked.occlusion_strength),
        ),
        ("an emissive factor of", &cooked.emissive),
    ];
    let not_negative = [
        ("an emissive strength of", Some(cooked.emissive_strength)),
        ("an alpha cutoff of", cooked.alpha_cutoff),
    ];
    if let Some((what, values)) = unit
        .into_iter()
        .find(|(_, values)| !values.iter().all(|value| (0.0..=1.0).contains(value)))
    {
        return refuse(format!(
            "material `{}` has {what} {values:?}; glTF requires values from 0 to 1",
            cooked.name
        ));
    }
    if let Some((what, Some(value))) = not_negative
        .into_iter()
        .find(|(_, value)| value.is_some_and(|value| !(value.is_finite() && value >= 0.0)))
    {
        return refuse(format!(
            "material `{}` has {what} {value}; glTF requires at least 0",
            cooked.name
        ));
    }
    Ok(())
}

/// The light `light` of node `node`, refused if `KHR_lights_punctual` does not allow it, which
/// the `gltf` crate does not check: a color or an intensity that is negative or not finite, a range
/// that is not positive and finite, or a spot light's cone outside 0 ≤ inner < outer ≤ π / 2.
fn read_light(
    light: &gltf::khr_lights_punctual::Light<'_>,
    node: usize,
) -> Result<SceneLight, ImportError> {
    let name = named(light.name(), "light", light.index());
    let refused = |what: String| ImportError(format!("node {node}'s light `{name}` {what}"));
    let color = light.color();
    if !color
        .iter()
        .all(|channel| channel.is_finite() && *channel >= 0.0)
    {
        return Err(refused(format!(
            "has a color of {color:?}; glTF requires channels of at least 0"
        )));
    }
    let intensity = light.intensity();
    if !(intensity.is_finite() && intensity >= 0.0) {
        return Err(refused(format!(
            "has an intensity of {intensity}; glTF requires at least 0"
        )));
    }
    let range = light.range();
    if let Some(range) = range
        && !(range.is_finite() && range > 0.0)
    {
        return Err(refused(format!(
            "has a range of {range}; glTF requires a positive range"
        )));
    }
    let kind = match light.kind() {
        Kind::Directional => LightKind::Directional,
        Kind::Point => LightKind::Point,
        Kind::Spot {
            inner_cone_angle,
            outer_cone_angle,
        } => {
            if !(0.0 <= inner_cone_angle
                && inner_cone_angle < outer_cone_angle
                && outer_cone_angle <= FRAC_PI_2)
            {
                return Err(refused(format!(
                    "has cone angles of {inner_cone_angle} and {outer_cone_angle} radians; glTF \
                     requires 0 ≤ inner < outer ≤ π/2"
                )));
            }
            LightKind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            }
        }
    };
    Ok(SceneLight {
        kind,
        color,
        intensity,
        // A directional light has no range: glTF allows one on point and spot lights only.
        range: range.filter(|_| kind != LightKind::Directional),
    })
}

/// Custom values of a node, by name.
type Extras = Vec<(String, Extra)>;

/// Custom values of a node whose names start with [`TYPED_PROPERTY`], by name, as they are.
type Typed = Vec<(String, gltf::json::Value)>;

/// The custom values of `node`, its `extras`: an object of booleans, numbers and texts, by name,
/// other values left out with a warning; then, apart, the values of those whose names start with
/// [`TYPED_PROPERTY`], whatever they are.
fn extras(node: &gltf::Node<'_>, warnings: &mut Vec<String>) -> (Extras, Typed) {
    let Some(raw) = node.extras() else {
        return (Vec::new(), Vec::new());
    };
    let label = named(node.name(), "node", node.index());
    let value: gltf::json::Value = match gltf::json::deserialize::from_str(raw.get()) {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("{label}'s extras cannot be read: {error}"));
            return (Vec::new(), Vec::new());
        }
    };
    let gltf::json::Value::Object(object) = value else {
        warnings.push(format!(
            "{label}'s extras are not an object of named values: left out"
        ));
        return (Vec::new(), Vec::new());
    };
    let mut extras = Vec::new();
    let mut typed = Vec::new();
    for (name, value) in object {
        if name.starts_with(TYPED_PROPERTY) {
            typed.push((name, value));
            continue;
        }
        let extra = match value {
            gltf::json::Value::Bool(value) => Extra::Bool(value),
            gltf::json::Value::Number(number) => match number.as_f64() {
                Some(value) => Extra::Number(value),
                None => continue,
            },
            gltf::json::Value::String(value) => Extra::Text(value),
            _ => {
                warnings.push(format!(
                    "{label}'s extra `{name}` is not a boolean, a number or a text: left out"
                ));
                continue;
            }
        };
        extras.push((name, extra));
    }
    (extras, typed)
}

/// The typed components of the node `label`, its properties `typed` named [`TYPED_PROPERTY`] and a
/// type's name, each a text that holds one value in the engine's notation, as a scene file writes a
/// component's, or a boolean or a number, the value of a tuple struct of one field: the binary
/// encoding of the document that holds them by their types' names, which a world reads through its
/// registry; empty for none. The property [`TYPED_VERSIONS`] gives the document's `$versions`, the
/// version of each type past version 1 that the others are written in, as a document's do.
///
/// # Errors
///
/// A property without a type's name, or named after another of the format's keys than
/// `$versions`; one whose value is not a text, a boolean or a number, or whose text holds a line
/// break or is not one value in the engine's notation, which `slime_serialize` reads, a table of
/// integers for [`TYPED_VERSIONS`]: the node, the property and why.
fn components(label: &str, typed: &[(String, gltf::json::Value)]) -> Result<Vec<u8>, ImportError> {
    if typed.is_empty() {
        return Ok(Vec::new());
    }
    let registry = Registry::new();
    let mut text = String::new();
    for (name, value) in typed {
        let ty = &name[TYPED_PROPERTY.len()..];
        if ty.is_empty() {
            return refuse(format!(
                "{label}'s property `{name}` names no type after `{TYPED_PROPERTY}`"
            ));
        }
        if ty.starts_with('$') && name != TYPED_VERSIONS {
            return refuse(format!(
                "{label}'s property `{name}` names no type: the keys that start with `$` are the \
                 format's, and a node gives only `{TYPED_VERSIONS}`"
            ));
        }
        let value = match value {
            gltf::json::Value::String(text) => text.clone(),
            gltf::json::Value::Bool(value) => value.to_string(),
            // An integer stays one, for a field of integers.
            gltf::json::Value::Number(number) => number.to_string(),
            _ => {
                return refuse(format!(
                    "{label}'s property `{name}` is not a text, a boolean or a number: write the \
                     component in the engine's notation, such as `{{ height = 2.6 }}`, as a text"
                ));
            }
        };
        // A line break could end the value and start another key, another component or versions.
        if value.contains(['\n', '\r']) {
            return refuse(format!(
                "{label}'s property `{name}` holds a line break: a typed property is one line"
            ));
        }
        let key = ty.replace('\\', "\\\\").replace('"', "\\\"");
        let line = format!("\"{key}\" = {value}\n");
        // Each read alone, so that an error names its property.
        Document::parse(&registry, &line, Mode::Lenient).map_err(|error| {
            ImportError(format!(
                "{label}'s property `{name}` is not a value in the engine's notation: {error}"
            ))
        })?;
        if name == TYPED_VERSIONS {
            text.insert_str(0, &line);
        } else {
            text.push_str(&line);
        }
    }
    slime_serialize::encode(&text)
        .map_err(|error| ImportError(format!("{label}'s typed components: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_are_decoded_and_paths_resolved() {
        assert_eq!(decode_uri("rock%20face.png"), "rock face.png");
        assert_eq!(decode_uri("100%"), "100%");
        assert_eq!(
            lexical(Path::new("a/b/../c/./d.png")),
            PathBuf::from("a/c/d.png")
        );
        assert_eq!(lexical(Path::new("../x.bin")), PathBuf::from("../x.bin"));
    }

    #[test]
    fn primitives_that_share_attributes_keep_only_their_own_vertices() {
        let positions: Vec<[f32; 3]> = (0..6).map(|i| [i as f32, 0.0, 0.0]).collect();
        let normals = vec![[0.0, 0.0, 1.0]; 6];
        let uvs = vec![[0.0, 0.0]; 6];
        let second = compacted(&positions, &normals, &uvs, None, &[3, 5, 4]);
        assert_eq!(
            second.positions,
            [[3.0, 0.0, 0.0], [5.0, 0.0, 0.0], [4.0, 0.0, 0.0]]
        );
        assert_eq!(second.indices, [0, 1, 2]);
    }

    #[test]
    fn a_mesh_without_normals_gets_flat_faces() {
        // The corners of a unit cube, numbered by their coordinates: x + 2y + 4z; two triangles
        // per face, counter-clockwise seen from outside.
        let positions: Vec<[f32; 3]> = (0..8u8)
            .map(|i| [i & 1, (i >> 1) & 1, (i >> 2) & 1].map(f32::from))
            .collect();
        let uvs = vec![[0.0, 0.0]; positions.len()];
        let indices = [
            0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 4, 2, 2, 4, 6, 1, 3, 5, 3, 7, 5, 0, 1, 4, 1, 5,
            4, 2, 6, 3, 3, 6, 7,
        ];
        let mesh = flat_shaded(&positions, &uvs, &indices);
        assert_eq!(mesh.positions.len(), indices.len());
        for (corner, normal) in mesh.normals.iter().enumerate() {
            let position = Vec3::from(mesh.positions[corner]);
            let normal = Vec3::from(*normal);
            // Each face's normal points out of the cube, along one axis.
            assert!((normal.length() - 1.0).abs() < 1e-6);
            let outward = (position - Vec3::splat(0.5)).dot(normal);
            assert!(outward > 0.0, "{position} {normal}");
        }
    }

    #[test]
    fn mikktspace_splits_a_vertex_whose_corners_disagree() {
        // Two triangles that share an edge, mapped as mirror images of each other across it: the
        // shared vertices get opposite handedness on each side, so they are split.
        let mut mesh = MeshArrays {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [-1.0, 0.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            uvs: vec![[0.0, 1.0], [1.0, 1.0], [0.0, 0.0], [1.0, 1.0]],
            tangents: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        with_mikktspace_tangents(&mut mesh);
        assert_eq!(mesh.indices.len(), 6);
        assert_eq!(mesh.positions.len(), 6, "the shared corners are split");
        for tangent in &mesh.tangents {
            let length = Vec3::new(tangent[0], tangent[1], tangent[2]).length();
            assert!((length - 1.0).abs() < 1e-4, "{tangent:?}");
        }
        let handedness: Vec<f32> = mesh.tangents.iter().map(|tangent| tangent[3]).collect();
        assert!(
            handedness.contains(&1.0) && handedness.contains(&-1.0),
            "{handedness:?}"
        );
    }
}

#[cfg(test)]
mod blender {
    use super::*;

    /// The sample scene, made in Blender, which writes MikkTSpace tangents of its own.
    const SAMPLE: &[u8] = include_bytes!("../../../examples/assets/scene.glb");

    #[test]
    fn mikktspace_tangents_match_blender_s() {
        let file = GltfFile::open(Path::new("scene.glb"), SAMPLE).expect("the sample opens");
        let mut compared = 0;
        for mesh in file.document.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&file.buffers[buffer.index()][..]));
                let positions: Vec<[f32; 3]> =
                    reader.read_positions().expect("positions").collect();
                let normals: Vec<[f32; 3]> = reader.read_normals().expect("normals").collect();
                let uvs: Vec<[f32; 2]> =
                    reader.read_tex_coords(0).expect("uvs").into_f32().collect();
                let blender: Vec<[f32; 4]> = reader.read_tangents().expect("tangents").collect();
                let indices: Vec<u32> =
                    reader.read_indices().expect("indices").into_u32().collect();
                let mut ours = compacted(&positions, &normals, &uvs, None, &indices);
                // The original vertex of each corner, before compaction and splitting.
                with_mikktspace_tangents(&mut ours);
                for (corner, &original) in indices.iter().enumerate() {
                    let theirs = blender[original as usize];
                    let mine = ours.tangents[ours.indices[corner] as usize];
                    let angle =
                        Vec3::from_slice(&theirs[..3]).angle_between(Vec3::from_slice(&mine[..3]));
                    assert!(
                        angle < 1f32.to_radians() && theirs[3] == mine[3],
                        "{:?} corner {corner}: Blender {theirs:?}, MikkTSpace {mine:?}",
                        mesh.name()
                    );
                    compared += 1;
                }
            }
        }
        assert!(compared > 6000, "{compared} corners compared");
    }
}
