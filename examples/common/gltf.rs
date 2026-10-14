//! A temporary loader of glTF scenes for the `scene` example and its image test: GLB files that
//! hold their buffer and PNG images in their binary chunk, drawn as they are (ADR-0029, ADR-0035,
//! ADR-0037). The asset pipeline of Stage 4 replaces it with an importer and deletes it. ADR-0035
//! holds these lists too, as ADR-0037 amends them.
//!
//! It reads triangle-list primitives: their positions; normals (when missing, each triangle gets
//! the normal of its face, as glTF requires); tangents (when missing, computed per triangle from
//! the texture coordinates, not with MikkTSpace as glTF prescribes; ignored, as glTF requires,
//! when the normals are missing); the first set of texture coordinates (zero when missing); and
//! indices (the vertices in order when missing). It reads materials with glTF's
//! metallic-roughness model: the base color, metallic, roughness and emissive factors, the
//! emissive strength of `KHR_materials_emissive_strength`, the normal texture's scale and the
//! occlusion texture's strength, the alpha mode (`OPAQUE`, and `MASK` with its cutoff, 0.5 when
//! missing) and double-sidedness, and gives glTF's default material (white, fully metallic, fully
//! rough) to primitives without one. Their base color, metallic-roughness, normal, occlusion and
//! emissive textures are each created in their slot's format (sRGB for the base color and
//! emissive, linear for the others): only the images that a material reads, each decoded once
//! and uploaded once per format, with one sampler per texture, which follows the file's settings
//! but for how levels of detail are read, which `slime-gfx` ties to the minification filter (see
//! `sampler_descriptor`). It flattens the node hierarchy of the default scene, or of the first,
//! into instances; the renderer keeps the front faces of a node whose transform mirrors. It reads
//! the lights that the nodes of that scene place (`KHR_lights_punctual`: directional, point and
//! spot, with their color, intensity, range and cone angles), each at its node's origin and along
//! its node's −Z axis in world space, whatever the node's scale.
//!
//! It refuses, with a clear error, what it cannot draw right: a file that is not valid glTF, which
//! includes one that requires an extension other than `KHR_lights_punctual` and
//! `KHR_materials_emissive_strength` (the `gltf` crate refuses it); a file without a binary chunk;
//! buffers and images in external files or in data URIs (it reads the binary chunk only); images
//! that are not PNG, or larger than the GPU accepts; primitives that are not triangle lists, or
//! have no positions; primitives whose vertices or indices need a larger buffer than the GPU
//! accepts (one without normals has three vertices per triangle); mesh data that
//! `Renderer::create_mesh` refuses (attributes of another length than the positions, indices that
//! are not a multiple of 3 or that name no vertex); a texture read through another set of texture
//! coordinates than the first, in any slot; a node hierarchy that is not a set of trees; a vertex
//! that a node places at a point that is not finite (positions or transforms that are NaN or
//! infinite, which glTF forbids, or whose product overflows), which would spoil the box around the
//! scene; a file without a scene, or whose scene draws nothing; a light that glTF forbids (a color
//! or an intensity that is negative or not finite, a range that is not positive and finite, a spot
//! light's cone outside 0 ≤ inner < outer ≤ π/2, which refuses the equal angles that Blender
//! writes for a spot whose blend is 0); a light whose node is placed at a point that is not finite
//! or flattens its −Z axis; more lights than the renderer draws (`slime_render::MAX_LIGHTS`). It
//! warns about, and draws without: animations, skins, cameras, vertex colors, the extensions a file
//! uses without requiring them that it does not read (`KHR_texture_transform` among them), and the
//! blending of materials whose alpha mode is `BLEND`, which are drawn opaque. A primitive that has
//! normals and shares its vertex attributes with other primitives, as three.js writes a mesh of
//! several materials, gets its own copy of every vertex they hold, even those its indices do not
//! name: K such primitives over V vertices upload K × V vertices. More generally, the loader
//! checks each primitive alone against the GPU's largest buffer, but sets no limit on the memory of
//! the whole file: it reads every primitive of every mesh, placed or not, before placing any, and
//! each primitive gets its own copy of what it reads, three vertices per triangle when it has no
//! normals. A file of a few megabytes that lists the same primitive many times can therefore ask
//! for tens of gigabytes, and the process runs out of memory rather than refusing it.
//!
//! The `gltf` crate's validation checks little more than that the file's objects name each other
//! correctly, and its readers panic on some of what it lets through. The loader refuses, rather
//! than panic on, such malformed files: a GLB header that declares a length shorter than itself,
//! on which the crate's parser panics in debug builds; positions that name an accessor the file
//! does not have, on which the crate's validation itself panics; images whose data lies outside
//! the binary chunk, has no MIME type or is missing; positions, normals, tangents, texture
//! coordinates or indices stored in types glTF does not allow for them, in a sparse accessor
//! (which the loader does not read), in an empty one, closer together than their size, or outside
//! the binary chunk; a node hierarchy that is not a set of trees. The crate's validation itself
//! refuses a node that names a light the file does not have, a light of an unknown type, and a spot
//! light without its `spot` object, on which its readers would panic.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::f32::consts::FRAC_PI_2;
use std::fmt;

use gltf::Semantic;
use gltf::accessor::{DataType, Dimensions};
use gltf::khr_lights_punctual::Kind;
use gltf::texture::{MagFilter, MinFilter, WrappingMode};
use slime_gfx::{
    AddressMode, Filter, GfxError, Gpu, Sampler, SamplerDescriptor, Texture, TextureFormat,
};
use slime_render::glam::{Mat4, Vec2, Vec3};
use slime_render::{
    AlphaMode, DirectionalLight, Instance, Light, MAX_LIGHTS, Material, MaterialData, Mesh,
    MeshData, PointLight, Renderer, SampledTexture, SpotLight,
};

/// Why a file could not be loaded.
#[derive(Debug)]
pub struct LoadError(String);

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

/// A GPU object of the file that could not be created: the GPU's memory is full, or the device
/// was lost.
fn gpu_error(error: GfxError) -> LoadError {
    LoadError(error.to_string())
}

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
    /// One per primitive that a node places.
    meshes: Vec<Mesh>,
    /// One per material of the file, then glTF's default material: white, fully metallic, fully
    /// rough.
    materials: Vec<Material>,
    placements: Vec<Placement>,
    /// The smallest box around every placed primitive.
    pub bounds: Bounds,
    /// The lights that the nodes of the scene place, in world space, at most [`MAX_LIGHTS`]; none
    /// for a file without `KHR_lights_punctual`.
    pub lights: Vec<Light>,
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

/// The extensions the loader reads; the `gltf` crate is built with their features.
const READ_EXTENSIONS: [&str; 2] = ["KHR_lights_punctual", "KHR_materials_emissive_strength"];

/// The label of a material: its name, or its index.
fn material_label(material: &gltf::Material<'_>) -> String {
    material.name().map_or_else(
        || format!("material {}", material.index().unwrap_or(0)),
        str::to_owned,
    )
}

/// A texture slot of a material: its name, the texture it reads with its set of texture
/// coordinates, if any, and the format its texture is created in.
type TextureSlot<'a> = (
    &'static str,
    Option<(gltf::Texture<'a>, u32)>,
    TextureFormat,
);

/// The texture slots of `material` in the renderer's order (ADR-0035), each created in its
/// slot's format: sRGB for colors, linear for data.
fn texture_slots<'a>(material: &gltf::Material<'a>) -> [TextureSlot<'a>; 5] {
    let pbr = material.pbr_metallic_roughness();
    [
        (
            "base color",
            pbr.base_color_texture()
                .map(|info| (info.texture(), info.tex_coord())),
            TextureFormat::Rgba8UnormSrgb,
        ),
        (
            "metallic-roughness",
            pbr.metallic_roughness_texture()
                .map(|info| (info.texture(), info.tex_coord())),
            TextureFormat::Rgba8Unorm,
        ),
        (
            "normal",
            material
                .normal_texture()
                .map(|normal| (normal.texture(), normal.tex_coord())),
            TextureFormat::Rgba8Unorm,
        ),
        (
            "occlusion",
            material
                .occlusion_texture()
                .map(|occlusion| (occlusion.texture(), occlusion.tex_coord())),
            TextureFormat::Rgba8Unorm,
        ),
        (
            "emissive",
            material
                .emissive_texture()
                .map(|info| (info.texture(), info.tex_coord())),
            TextureFormat::Rgba8UnormSrgb,
        ),
    ]
}

/// Loads the GLB file `bytes` into GPU objects created through `renderer`.
pub fn load(gpu: &Gpu, renderer: &Renderer, bytes: &[u8]) -> Result<LoadedScene, LoadError> {
    // The `gltf` crate subtracts the header's 12 bytes from the length a GLB header declares
    // without checking it first, an overflow that panics in debug builds. It refuses a header cut
    // short itself.
    let declared = bytes
        .strip_prefix(b"glTF")
        .and_then(|rest| rest.get(4..8))
        .map(|length| u32::from_le_bytes(length.try_into().expect("4 bytes")));
    if let Some(length @ 0..12) = declared {
        return Err(LoadError(format!(
            "the file is not valid glTF: its GLB header declares a length of {length} bytes, \
             shorter than the header, 12 bytes"
        )));
    }
    let not_valid = |error| LoadError(format!("the file is not valid glTF: {error}"));
    let gltf::Gltf {
        document: unvalidated,
        blob,
    } = gltf::Gltf::from_slice_without_validation(bytes).map_err(not_valid)?;
    check_positions_exist(unvalidated.as_json())?;
    let document = gltf::Document::from_json(unvalidated.into_json()).map_err(not_valid)?;
    let document = &document;

    // The `gltf` crate refuses a file that requires an extension it is not built to read.
    let ignored: Vec<&str> = document
        .extensions_used()
        .filter(|name| !READ_EXTENSIONS.contains(name))
        .collect();
    if !ignored.is_empty() {
        tracing::warn!(extensions = ?ignored, "the file uses extensions that the loader ignores");
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
            "materials whose alpha mode is BLEND (drawn opaque)",
        ),
    ] {
        if count > 0 {
            tracing::warn!(count, "the file has {what}, which the loader ignores");
        }
    }

    let blob = blob.as_deref().ok_or_else(|| {
        LoadError(
            "the file has no binary chunk: only GLB files that hold their buffer and PNG images \
             in it are supported"
                .to_owned(),
        )
    })?;
    for buffer in document.buffers() {
        if let gltf::buffer::Source::Uri(uri) = buffer.source() {
            let name = buffer
                .name()
                .map_or_else(|| format!("buffer {}", buffer.index()), str::to_owned);
            return Err(outside_the_binary_chunk(&name, uri));
        }
    }
    let buffer_data = |buffer: gltf::Buffer<'_>| match buffer.source() {
        gltf::buffer::Source::Bin => Some(blob),
        gltf::buffer::Source::Uri(_) => None,
    };

    // The textures that materials read: each image decoded once and uploaded once per format it
    // is read in, each glTF texture's sampler created once. Images that no material reads are not
    // decoded.
    let mut decoded: HashMap<usize, DecodedImage> = HashMap::new();
    let mut uploaded: HashMap<(usize, TextureFormat), Texture> = HashMap::new();
    let mut samplers: HashMap<usize, Sampler> = HashMap::new();
    for material in document.materials() {
        for (slot, texture, format) in texture_slots(&material) {
            let Some((texture, tex_coord)) = texture else {
                continue;
            };
            if tex_coord != 0 {
                return Err(LoadError(format!(
                    "material `{}` reads its {slot} texture through coordinate set {tex_coord}; \
                     only the first set is supported",
                    material_label(&material)
                )));
            }
            let image = texture.source();
            if let Entry::Vacant(entry) = uploaded.entry((image.index(), format)) {
                let pixels = match decoded.entry(image.index()) {
                    Entry::Occupied(found) => found.into_mut(),
                    Entry::Vacant(missing) => missing.insert(decode_png(
                        document,
                        image.clone(),
                        blob,
                        gpu.max_texture_dimension(),
                    )?),
                };
                let name = image
                    .name()
                    .map_or_else(|| format!("image {}", image.index()), str::to_owned);
                let label = format!(
                    "{name}, {}",
                    if format.is_srgb() { "sRGB" } else { "linear" }
                );
                entry.insert(
                    gpu.create_texture(&label, pixels.width, pixels.height, format, &pixels.pixels)
                        .map_err(gpu_error)?,
                );
            }
            if let Entry::Vacant(entry) = samplers.entry(texture.index()) {
                let label = texture
                    .name()
                    .map_or_else(|| format!("texture {}", texture.index()), str::to_owned);
                entry.insert(
                    gpu.create_sampler(&label, &sampler_descriptor(&texture.sampler()))
                        .map_err(gpu_error)?,
                );
            }
        }
    }
    drop(decoded);

    let mut materials: Vec<Material> = document
        .materials()
        .map(|material| {
            let [
                base_color_texture,
                metallic_roughness_texture,
                normal_texture,
                occlusion_texture,
                emissive_texture,
            ] = texture_slots(&material).map(|(_, texture, format)| {
                texture.map(|(texture, _)| SampledTexture {
                    texture: &uploaded[&(texture.source().index(), format)],
                    sampler: &samplers[&texture.index()],
                })
            });
            let pbr = material.pbr_metallic_roughness();
            let alpha_mode = match material.alpha_mode() {
                // Blending is not drawn: warned about above.
                gltf::material::AlphaMode::Opaque | gltf::material::AlphaMode::Blend => {
                    AlphaMode::Opaque
                }
                gltf::material::AlphaMode::Mask => AlphaMode::Mask {
                    cutoff: material.alpha_cutoff().unwrap_or(0.5),
                },
            };
            renderer.create_material(
                gpu,
                &material_label(&material),
                &MaterialData {
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
                    alpha_mode,
                    double_sided: material.double_sided(),
                },
            )
        })
        .collect();
    let default_material = materials.len();
    // glTF's default material: white, fully metallic, fully rough, opaque, single-sided.
    materials.push(renderer.create_material(
        gpu,
        "default",
        &MaterialData {
            metallic: 1.0,
            ..MaterialData::default()
        },
    ));

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
            let has_normals = normals.is_some();
            let mut data = match normals {
                Some(normals) => MeshData {
                    positions,
                    normals: normals.collect(),
                    uvs,
                    tangents: Vec::new(),
                    indices,
                },
                None => flat_shaded(&label, &positions, &uvs, &indices)?,
            };
            data.tangents = match reader.read_tangents() {
                // glTF: a primitive without normals ignores its tangents, since its normals are
                // computed.
                Some(tangents) if has_normals => tangents.collect(),
                _ => computed_tangents(&data),
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
    let mut lights = Vec::new();
    for node in scene.nodes() {
        place(
            &node,
            &primitives_of_mesh,
            &primitives,
            &mut reached,
            &mut placed,
            &mut bounds,
            &mut lights,
        )?;
    }
    if placed.is_empty() {
        return Err(LoadError(
            "the scene draws nothing: no node has a mesh".to_owned(),
        ));
    }

    let mut meshes = Vec::new();
    // The mesh of each primitive, as an index into `meshes`, once a placement needs it. A node
    // whose transform mirrors places the same mesh: the renderer keeps its front faces (ADR-0035).
    let mut mesh_of = vec![None; primitives.len()];
    let mut placements = Vec::with_capacity(placed.len());
    for (index, transform) in placed {
        let primitive = &primitives[index];
        let mesh = match mesh_of[index] {
            Some(mesh) => mesh,
            None => {
                let mesh = renderer
                    .create_mesh(gpu, &primitive.label, &primitive.data)
                    .map_err(|error| LoadError(error.to_string()))?;
                meshes.push(mesh);
                mesh_of[index] = Some(meshes.len() - 1);
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
        textures = uploaded.len(),
        instances = placements.len(),
        lights = lights.len(),
        "glTF scene loaded"
    );
    Ok(LoadedScene {
        meshes,
        materials,
        placements,
        bounds,
        lights,
    })
}

/// Checks that the positions of every primitive of `json`, a file not validated yet, name an
/// accessor of the file. The `gltf` crate's validation reports any other index that names nothing,
/// but reads this accessor without checking that it exists, and panics on one that does not.
fn check_positions_exist(json: &gltf::json::Root) -> Result<(), LoadError> {
    for (index, mesh) in json.meshes.iter().enumerate() {
        for (primitive, data) in mesh.primitives.iter().enumerate() {
            if let Some(accessor) = data
                .attributes
                .get(&gltf::json::validation::Checked::Valid(Semantic::Positions))
                && accessor.value() >= json.accessors.len()
            {
                let name = mesh.name.clone().unwrap_or_else(|| format!("mesh {index}"));
                return Err(LoadError(format!(
                    "the file is not valid glTF: {name} primitive {primitive} reads its positions \
                     from accessor {accessor}, which does not exist",
                )));
            }
        }
    }
    Ok(())
}

/// Places the primitives of `node`, a node of the scene, and of its descendants: adds each to
/// `placed`, as an index into `primitives` with its transform, and grows `bounds` with it; adds
/// the light of each node that has one to `lights`, refusing more than [`MAX_LIGHTS`] in all.
/// `reached` says which nodes were placed already.
///
/// The walk goes depth first, in the file's order, with a stack of its own rather than by
/// recursion: a valid file can nest nodes deeper than a thread's stack holds calls. It fails on a
/// node reached a second time: glTF requires the node hierarchy to be a set of trees, which the
/// `gltf` crate does not check, and a node that is its own descendant would otherwise be placed
/// without end. It fails on a vertex placed at a point that is not finite: NaN or infinite
/// positions or transforms, which glTF forbids, or finite ones whose product overflows.
fn place(
    node: &gltf::Node<'_>,
    primitives_of_mesh: &[Vec<usize>],
    primitives: &[LoadedPrimitive],
    reached: &mut [bool],
    placed: &mut Vec<(usize, Mat4)>,
    bounds: &mut Bounds,
    lights: &mut Vec<Light>,
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
                    // A point that is not finite would not grow the box but replace it, so that
                    // the box would hold only the points after it, or be NaN if it came last.
                    let point = transform.transform_point3(Vec3::from(*position));
                    if !point.is_finite() {
                        return Err(LoadError(format!(
                            "node {} places {} at {point}: its positions or its transform are \
                             not finite numbers, which glTF forbids, or their product overflows",
                            node.index(),
                            primitives[index].label
                        )));
                    }
                    bounds.grow(point);
                }
                placed.push((index, transform));
            }
        }
        if let Some(light) = node.light() {
            if lights.len() == MAX_LIGHTS {
                return Err(LoadError(format!(
                    "the scene places more than {MAX_LIGHTS} lights, the most the renderer draws"
                )));
            }
            lights.push(read_light(&light, transform, node.index())?);
        }
        // Reversed on the stack, so that the first child and its descendants are placed next.
        let children = stack.len();
        stack.extend(node.children().map(|child| (child, transform)));
        stack[children..].reverse();
    }
    Ok(())
}

/// The light `light`, which node `node` places with `transform`, in world space: at the node's
/// origin, along its −Z axis, whatever its scale (`KHR_lights_punctual`). It is refused if
/// `KHR_lights_punctual` does not allow it, which the `gltf` crate does not check: a color or an
/// intensity that is negative or not finite, a range that is not positive and finite, or a spot
/// light's cone outside 0 ≤ inner < outer ≤ π / 2; or if the transform places it at a point that is
/// not finite, or gives it no direction.
fn read_light(
    light: &gltf::khr_lights_punctual::Light<'_>,
    transform: Mat4,
    node: usize,
) -> Result<Light, LoadError> {
    let name = light
        .name()
        .map_or_else(|| format!("light {}", light.index()), str::to_owned);
    let refused = |what: String| LoadError(format!("node {node}'s light `{name}` {what}"));
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
    let position = transform.transform_point3(Vec3::ZERO);
    if !position.is_finite() {
        return Err(refused(format!(
            "is placed at {position}: the node's transform is not finite"
        )));
    }
    // Scale changes the length of the axis, not the direction it gives.
    let direction = transform
        .transform_vector3(Vec3::NEG_Z)
        .try_normalize()
        .ok_or_else(|| {
            refused(
                "has no direction: the node's transform flattens its −Z axis or is not finite"
                    .to_owned(),
            )
        })?;
    let color = Vec3::from(color);
    Ok(match light.kind() {
        // A directional light has no range: glTF allows one on point and spot lights only.
        Kind::Directional => Light::Directional(DirectionalLight {
            direction,
            color,
            illuminance: intensity,
            casts_shadows: true,
        }),
        Kind::Point => Light::Point(PointLight {
            position,
            color,
            intensity,
            range,
        }),
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
            Light::Spot(SpotLight {
                position,
                direction,
                color,
                intensity,
                range,
                inner_cone_angle,
                outer_cone_angle,
            })
        }
    })
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
    // elements as the file claims, which no data backs. An accessor that is neither sparse nor
    // in a buffer view, zeros in glTF, never gets here: the `gltf` crate's validation refuses it.
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
/// buffers of `max_buffer_size` bytes, the GPU's largest. [`Renderer::create_mesh`] refuses a larger
/// mesh too, but this check runs before `flat_shaded` copies three vertices per triangle. A vertex
/// takes 48 bytes there (a position, a normal, texture coordinates and a tangent), an index 4.
fn check_buffer_sizes(
    label: &str,
    vertices: usize,
    indices: usize,
    max_buffer_size: u64,
) -> Result<(), LoadError> {
    for (count, what, size) in [(vertices, "vertices", 48), (indices, "indices", 4)] {
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

/// Tangents for a primitive whose file has none (ADR-0035), computed per triangle: each
/// triangle's directions of growing u and v, from its edges and texture coordinates (Lengyel's
/// method), summed at its vertices, then the u direction made perpendicular to each vertex's
/// normal; w is −1 where the texture is mirrored on the surface. glTF prescribes MikkTSpace, which
/// the importer of Stage 4 will compute: the two differ where the triangles of a vertex disagree.
/// A vertex whose triangles give no direction (texture coordinates without area) gets any
/// direction perpendicular to its normal. Triangles that name a vertex the arrays lack are
/// skipped: [`Renderer::create_mesh`] refuses the mesh afterwards.
fn computed_tangents(data: &MeshData) -> Vec<[f32; 4]> {
    let mut along_u = vec![Vec3::ZERO; data.positions.len()];
    let mut along_v = vec![Vec3::ZERO; data.positions.len()];
    for triangle in data.indices.as_chunks::<3>().0 {
        let corners = triangle.map(|index| index as usize);
        if corners
            .iter()
            .any(|&index| index >= data.positions.len() || index >= data.uvs.len())
        {
            continue;
        }
        let [p0, p1, p2] = corners.map(|index| Vec3::from(data.positions[index]));
        let [t0, t1, t2] = corners.map(|index| Vec2::from(data.uvs[index]));
        let (e1, e2, d1, d2) = (p1 - p0, p2 - p0, t1 - t0, t2 - t0);
        let area = d1.perp_dot(d2);
        if area == 0.0 || !area.is_finite() {
            continue;
        }
        // Where the position goes as u grows, and as v grows (down the texture's image).
        let u = (e1 * d2.y - e2 * d1.y) / area;
        let v = (e2 * d1.x - e1 * d2.x) / area;
        for index in corners {
            along_u[index] += u;
            along_v[index] += v;
        }
    }
    data.normals
        .iter()
        .zip(along_u.iter().zip(&along_v))
        .map(|(&normal, (&u, &v))| {
            let normal = Vec3::from(normal);
            let tangent = (u - normal * normal.dot(u))
                .try_normalize()
                .unwrap_or_else(|| normal.any_orthonormal_vector());
            // glTF's bitangent, cross(normal, tangent) × w, points up the image, where v shrinks.
            let w = if normal.cross(tangent).dot(v) > 0.0 {
                -1.0
            } else {
                1.0
            };
            [tangent.x, tangent.y, tangent.z, w]
        })
        .collect()
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

/// The refusal of `name`, a buffer or an image whose data the file gives at `uri` instead of in its
/// binary chunk: in an external file, or as text in a data URI, which can hold megabytes and is
/// therefore not quoted. The `gltf` crate decodes data URIs only with its `import` feature, which
/// the examples leave out (ADR-0029).
fn outside_the_binary_chunk(name: &str, uri: &str) -> LoadError {
    LoadError(if uri.starts_with("data:") {
        format!("{name} is given as a data URI, which the loader does not decode")
    } else {
        format!("{name} is an external file, `{uri}`, which the loader does not read")
    })
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
        gltf::image::Source::Uri { uri, .. } => return Err(outside_the_binary_chunk(&name, uri)),
    };
    if mime_type != "image/png" {
        return Err(LoadError(format!(
            "{name} is a {mime_type} image; only PNG images are supported"
        )));
    }
    decode_png_data(&name, bytes, max_dimension)
}

/// Decodes `bytes`, the PNG image `name`, into RGBA bytes of 8 bits per channel, whatever the image
/// stores, if neither of its sides is longer than `max_dimension`.
fn decode_png_data(
    name: &str,
    bytes: &[u8],
    max_dimension: u32,
) -> Result<DecodedImage, LoadError> {
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
    fn lights_are_placed_in_world_space_along_the_node_s_negative_z_axis() {
        use slime_render::glam::Quat;

        let file = gltf::Gltf::from_slice(
            br#"{
                "asset": { "version": "2.0" },
                "extensionsUsed": ["KHR_lights_punctual"],
                "extensions": { "KHR_lights_punctual": { "lights": [
                    { "type": "directional", "color": [1, 0.5, 0.25], "intensity": 3 },
                    { "type": "point", "intensity": 2, "range": 5 },
                    { "type": "spot", "intensity": 7,
                      "spot": { "innerConeAngle": 0.1, "outerConeAngle": 0.2 } }
                ] } }
            }"#,
        )
        .expect("the JSON is valid glTF");
        let lights: Vec<_> = file.lights().expect("the file has lights").collect();
        // Turned by 90° about +Y, which turns −Z to −X, scaled unevenly and moved: the scale
        // changes neither the direction nor the cone.
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 3.0, 0.5),
            Quat::from_rotation_y(FRAC_PI_2),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let read: Vec<Light> = lights
            .iter()
            .map(|light| read_light(light, transform, 0).expect("the light is valid"))
            .collect();
        let close = |a: Vec3, b: Vec3| a.abs_diff_eq(b, 1e-6);
        let Light::Directional(sun) = read[0] else {
            panic!("{:?}", read[0])
        };
        assert!(close(sun.direction, Vec3::NEG_X), "{sun:?}");
        assert_eq!(
            (sun.color, sun.illuminance),
            (Vec3::new(1.0, 0.5, 0.25), 3.0)
        );
        let Light::Point(bulb) = read[1] else {
            panic!("{:?}", read[1])
        };
        assert_eq!(
            bulb,
            PointLight {
                position: Vec3::new(1.0, 2.0, 3.0),
                color: Vec3::ONE,
                intensity: 2.0,
                range: Some(5.0),
            }
        );
        let Light::Spot(spot) = read[2] else {
            panic!("{:?}", read[2])
        };
        assert!(close(spot.direction, Vec3::NEG_X), "{spot:?}");
        assert_eq!(
            (spot.position, spot.intensity, spot.range),
            (Vec3::new(1.0, 2.0, 3.0), 7.0, None)
        );
        assert_eq!((spot.inner_cone_angle, spot.outer_cone_angle), (0.1, 0.2));
    }

    #[test]
    fn meshes_larger_than_the_gpu_buffers_are_refused() {
        // Buffers of 480 bytes: ten vertices, or 120 indices.
        assert!(check_buffer_sizes("mesh", 10, 120, 480).is_ok());
        let error = check_buffer_sizes("mesh", 11, 3, 480)
            .unwrap_err()
            .to_string();
        assert!(error.contains("11 vertices, 528 bytes"), "{error}");
        let error = check_buffer_sizes("mesh", 3, 121, 480)
            .unwrap_err()
            .to_string();
        assert!(error.contains("121 indices, 484 bytes"), "{error}");
    }

    #[test]
    fn tangents_are_computed_where_the_file_has_none() {
        // A square facing +Z, two triangles, with glTF's texture coordinates: u grows along +X,
        // v down the image, along −Y.
        let square = |uvs: [[f32; 2]; 4], normal: [f32; 3]| MeshData {
            positions: vec![
                [-1.0, -1.0, 0.0],
                [1.0, -1.0, 0.0],
                [1.0, 1.0, 0.0],
                [-1.0, 1.0, 0.0],
            ],
            normals: vec![normal; 4],
            uvs: uvs.to_vec(),
            tangents: Vec::new(),
            indices: vec![0, 1, 2, 0, 2, 3],
        };
        let upright = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let facing_z = [0.0, 0.0, 1.0];
        assert_eq!(
            computed_tangents(&square(upright, facing_z)),
            vec![[1.0, 0.0, 0.0, 1.0]; 4]
        );
        // The texture mirrored along u: the tangent turns around, and so does the handedness.
        let mirrored = [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]];
        assert_eq!(
            computed_tangents(&square(mirrored, facing_z)),
            vec![[-1.0, 0.0, 0.0, -1.0]; 4]
        );
        // Normals that lean: the tangent is made perpendicular to them.
        for tangent in computed_tangents(&square(upright, [0.6, 0.0, 0.8])) {
            let expected = [0.8, 0.0, -0.6, 1.0];
            assert!(
                tangent
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| (actual - expected).abs() < 1e-6),
                "{tangent:?}"
            );
        }
        // Texture coordinates without area give no direction: any one perpendicular to the normal.
        for [x, y, z, w] in computed_tangents(&square([[0.0; 2]; 4], facing_z)) {
            let tangent = Vec3::new(x, y, z);
            assert!((tangent.length() - 1.0).abs() < 1e-6, "{tangent}");
            assert!(tangent.dot(Vec3::Z).abs() < 1e-6, "{tangent}");
            assert_eq!(w, 1.0);
        }
        // A triangle that names no vertex is skipped, without a panic; `create_mesh` refuses it.
        let mut broken = square(upright, facing_z);
        broken.indices[5] = 9;
        assert_eq!(computed_tangents(&broken).len(), 4);
    }

    #[test]
    fn samplers_follow_the_file_on_each_axis() {
        use AddressMode::{ClampToEdge, MirrorRepeat, Repeat};
        use Filter::{Linear, Nearest};

        // glTF's codes: wraps 33071 CLAMP_TO_EDGE and 33648 MIRRORED_REPEAT; filters 9728 NEAREST,
        // 9729 LINEAR, 9984 NEAREST_MIPMAP_NEAREST, 9985 LINEAR_MIPMAP_NEAREST and 9986
        // NEAREST_MIPMAP_LINEAR. Missing fields default to REPEAT, and to linear filters here.
        let file = gltf::Gltf::from_slice(
            br#"{
                "asset": { "version": "2.0" },
                "samplers": [
                    { "wrapS": 33071, "wrapT": 33648, "magFilter": 9728, "minFilter": 9984 },
                    { "wrapS": 33648, "wrapT": 33071, "magFilter": 9729, "minFilter": 9985 },
                    { "minFilter": 9986 },
                    {}
                ]
            }"#,
        )
        .expect("the JSON is valid glTF");
        let descriptors: Vec<_> = file
            .samplers()
            .map(|sampler| sampler_descriptor(&sampler))
            .collect();
        let descriptor = |address_u, address_v, mag_filter, min_filter| SamplerDescriptor {
            mag_filter,
            min_filter,
            address_u,
            address_v,
        };
        assert_eq!(
            descriptors,
            [
                descriptor(ClampToEdge, MirrorRepeat, Nearest, Nearest),
                descriptor(MirrorRepeat, ClampToEdge, Linear, Linear),
                descriptor(Repeat, Repeat, Linear, Nearest),
                descriptor(Repeat, Repeat, Linear, Linear),
            ]
        );
    }

    #[test]
    fn png_images_of_every_kind_are_decoded_into_rgba() {
        use png::{BitDepth, ColorType};

        // An image of two pixels side by side, of `color` in `depth`, stored as `data`, with
        // `palette` and `transparency` chunks unless they are empty, decoded.
        let decoded = |color, depth, data: &[u8], palette: &[u8], transparency: &[u8]| {
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(color);
            encoder.set_depth(depth);
            if !palette.is_empty() {
                encoder.set_palette(palette.to_vec());
            }
            if !transparency.is_empty() {
                encoder.set_trns(transparency.to_vec());
            }
            let mut writer = encoder.write_header().expect("the PNG header is written");
            writer
                .write_image_data(data)
                .expect("the PNG image is written");
            writer.finish().expect("the PNG image is finished");
            let image = decode_png_data("image", &bytes, 2).expect("the image is decoded");
            assert_eq!((image.width, image.height), (2, 1));
            image.pixels
        };
        let (eight, none) = (BitDepth::Eight, &[]);

        assert_eq!(
            decoded(
                ColorType::Rgba,
                eight,
                &[10, 20, 30, 40, 50, 60, 70, 80],
                none,
                none
            ),
            [10, 20, 30, 40, 50, 60, 70, 80]
        );
        let rgb = [10, 20, 30, 50, 60, 70];
        assert_eq!(
            decoded(ColorType::Rgb, eight, &rgb, none, none),
            [10, 20, 30, 255, 50, 60, 70, 255]
        );
        // The color of the transparency chunk, written in 16 bits per channel, is transparent.
        assert_eq!(
            decoded(ColorType::Rgb, eight, &rgb, none, &[0, 10, 0, 20, 0, 30]),
            [10, 20, 30, 0, 50, 60, 70, 255]
        );
        // 16 bits per channel, big-endian: the most significant byte is kept.
        assert_eq!(
            decoded(
                ColorType::Rgb,
                BitDepth::Sixteen,
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
                none,
                none
            ),
            [1, 3, 5, 255, 7, 9, 11, 255]
        );
        assert_eq!(
            decoded(ColorType::Grayscale, eight, &[10, 200], none, none),
            [10, 10, 10, 255, 200, 200, 200, 255]
        );
        // One bit per pixel, from the most significant bit: white, then black.
        assert_eq!(
            decoded(
                ColorType::Grayscale,
                BitDepth::One,
                &[0b1000_0000],
                none,
                none
            ),
            [255, 255, 255, 255, 0, 0, 0, 255]
        );
        assert_eq!(
            decoded(
                ColorType::GrayscaleAlpha,
                eight,
                &[10, 40, 200, 80],
                none,
                none
            ),
            [10, 10, 10, 40, 200, 200, 200, 80]
        );
        // A palette of red and blue, without and with an alpha per entry.
        let palette = [255, 0, 0, 0, 0, 255];
        assert_eq!(
            decoded(ColorType::Indexed, eight, &[0, 1], &palette, none),
            [255, 0, 0, 255, 0, 0, 255, 255]
        );
        assert_eq!(
            decoded(ColorType::Indexed, eight, &[0, 1], &palette, &[128, 255]),
            [255, 0, 0, 128, 0, 0, 255, 255]
        );
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
