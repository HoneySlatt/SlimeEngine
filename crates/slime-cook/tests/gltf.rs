//! Importing glTF files: small files built in memory, which hold what the importer must read,
//! leave out with a warning, or refuse with an error rather than panic on; and `.gltf` files with
//! their buffers and images beside them, cooked in temporary directories.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use gltf::json::Value;
use slime_assets::{
    AssetId, CookedAssets, Extra, LightKind, TextureEncoding, TextureFilter, TextureWrap,
};
use slime_cook::gltf_import::{GltfFile, Imported, MAX_FILE_VERTICES, MAX_MESH_VERTICES};
use slime_cook::ids::{SubAssetKind, default_material_id, sub_asset_ids, texture_id};
use slime_cook::texture::TextureRole;
use slime_cook::{CookError, Mode, cook, scan};
use slime_world::{PersistentId, ResolvedScene, World};

/// The ID the tests give their files.
const FILE: AssetId = AssetId::from_u128(0x6f1c2b3a_0d4e_4f5a_8b6c_7d8e9f0a1b2c);

/// A GLB file built in memory: one triangle facing +Z, with positions only, in the one node of the
/// default scene, which each test changes.
struct Glb {
    json: Value,
    /// The binary chunk.
    bin: Vec<u8>,
}

impl Glb {
    fn triangle() -> Self {
        let mut file = Self {
            json: json(
                r#"{
                    "asset": { "version": "2.0" },
                    "buffers": [{ "byteLength": 0 }],
                    "bufferViews": [],
                    "accessors": [],
                    "meshes": [{ "primitives": [{ "attributes": {} }] }],
                    "nodes": [{ "mesh": 0 }],
                    "scenes": [{ "nodes": [0] }],
                    "scene": 0
                }"#,
            ),
            bin: Vec::new(),
        };
        let corners: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let view = file.add_view(&corners.map(f32::to_le_bytes).concat());
        let positions = file.add_accessor(view, FLOAT, 3, "VEC3");
        file.json["accessors"][positions]["min"] = json("[0, 0, 0]");
        file.json["accessors"][positions]["max"] = json("[1, 1, 0]");
        file.json["meshes"][0]["primitives"][0]["attributes"]["POSITION"] = positions.into();
        file
    }

    /// Adds `bytes` to the binary chunk, in a buffer view of their own, and returns its index.
    fn add_view(&mut self, bytes: &[u8]) -> usize {
        let view = json(&format!(
            r#"{{ "buffer": 0, "byteOffset": {}, "byteLength": {} }}"#,
            self.bin.len(),
            bytes.len()
        ));
        self.bin.extend_from_slice(bytes);
        // Each view starts at a multiple of 4, as glTF requires of the data of accessors.
        self.bin.resize(self.bin.len().next_multiple_of(4), 0);
        push(&mut self.json["bufferViews"], view)
    }

    /// Adds an accessor to `count` elements of `kind` (`"VEC3"`, ...), made of components of
    /// `component_type` (`FLOAT`, ...), in buffer view `view`, and returns its index.
    fn add_accessor(&mut self, view: usize, component_type: u32, count: u32, kind: &str) -> usize {
        let accessor = json(&format!(
            r#"{{ "bufferView": {view}, "componentType": {component_type}, "count": {count},
                  "type": "{kind}" }}"#
        ));
        push(&mut self.json["accessors"], accessor)
    }

    /// Embeds the PNG image `png` in the file as its only image, read by its only texture.
    fn with_image(mut self, png: &[u8]) -> Self {
        let view = self.add_view(png);
        self.json["images"] = json(&format!(
            r#"[{{ "bufferView": {view}, "mimeType": "image/png" }}]"#
        ));
        self.json["textures"] = json(r#"[{ "source": 0 }]"#);
        self
    }

    /// Gives the triangle `material`, a glTF material in JSON, the only one of the file.
    fn with_material(mut self, material: &str) -> Self {
        self.json["materials"] = json(&format!("[{material}]"));
        self.json["meshes"][0]["primitives"][0]["material"] = 0.into();
        self
    }

    /// Adds `light`, a light of `KHR_lights_punctual` in JSON, the only one of the file, on a new
    /// node of the scene.
    fn with_light(mut self, light: &str) -> Self {
        self.json["extensionsUsed"] = json(r#"["KHR_lights_punctual"]"#);
        self.json["extensions"] = json(&format!(
            r#"{{ "KHR_lights_punctual": {{ "lights": [{light}] }} }}"#
        ));
        let node = push(
            &mut self.json["nodes"],
            json(r#"{ "extensions": { "KHR_lights_punctual": { "light": 0 } } }"#),
        );
        push(&mut self.json["scenes"][0]["nodes"], node.into());
        self
    }

    /// Gives the triangle's three vertices the attribute `semantic`, of type `kind` (`"VEC3"`,
    /// ...), from `values`, floats.
    fn with_attribute(mut self, semantic: &str, kind: &str, values: &[f32]) -> Self {
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let view = self.add_view(&bytes);
        let accessor = self.add_accessor(view, FLOAT, 3, kind);
        self.json["meshes"][0]["primitives"][0]["attributes"][semantic] = accessor.into();
        self
    }

    fn bytes(&mut self) -> Vec<u8> {
        self.json["buffers"][0]["byteLength"] = self.bin.len().into();
        let json = gltf::json::serialize::to_vec(&self.json).expect("the JSON is written");
        gltf::Glb {
            header: gltf::binary::Header {
                magic: *b"glTF",
                version: 2,
                // `to_vec` computes it.
                length: 0,
            },
            json: Cow::Owned(json),
            bin: Some(Cow::Borrowed(&self.bin)),
        }
        .to_vec()
        .expect("the GLB file is written")
    }

    /// What the importer gives of the file, or its error.
    fn import(&mut self) -> Result<Imported, String> {
        import_bytes(&self.bytes())
    }

    /// The message of the error with which the importer refuses the file.
    fn refusal(&mut self) -> String {
        match self.import() {
            Ok(_) => panic!("the importer accepted a file it must refuse"),
            Err(error) => error,
        }
    }
}

/// What the importer gives of the GLB file `bytes`, or its error.
fn import_bytes(bytes: &[u8]) -> Result<Imported, String> {
    GltfFile::open(Path::new("test.glb"), bytes)
        .and_then(|file| file.import(FILE, &HashMap::new()))
        .map_err(|error| error.to_string())
}

/// Component types of glTF: 8-bit signed and unsigned integers, 16-bit unsigned integers and
/// 32-bit floating-point numbers.
const BYTE: u32 = 5120;
const UNSIGNED_BYTE: u32 = 5121;
const UNSIGNED_SHORT: u32 = 5123;
const FLOAT: u32 = 5126;

fn json(text: &str) -> Value {
    gltf::json::deserialize::from_str(text).expect("the JSON of a test is valid")
}

/// Appends `value` to `array`, a JSON array, and returns its index.
fn push(array: &mut Value, value: Value) -> usize {
    let array = array.as_array_mut().expect("a JSON array");
    array.push(value);
    array.len() - 1
}

/// Removes `key` from `object`, a JSON object.
fn remove(object: &mut Value, key: &str) {
    object.as_object_mut().expect("a JSON object").remove(key);
}

/// The index that `value`, a JSON number, gives.
fn index(value: &Value) -> usize {
    value.as_u64().expect("an index") as usize
}

/// A white PNG image of `width` × `height` texels.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().expect("the PNG header is written");
    writer
        .write_image_data(&vec![255; width as usize * height as usize * 4])
        .expect("the PNG image is written");
    writer.finish().expect("the PNG image is finished");
    bytes
}

#[test]
fn a_triangle_becomes_a_mesh_its_default_material_and_the_file_s_scene() {
    let imported = Glb::triangle().import().expect("imported");
    let [(mesh_id, mesh)] = &imported.meshes[..] else {
        panic!("one mesh: {:?}", imported.meshes);
    };
    // Flat normals, three vertices of its own per triangle, and MikkTSpace tangents.
    assert_eq!(mesh.positions.len(), 3);
    assert!(mesh.normals.iter().all(|normal| *normal == [0.0, 0.0, 1.0]));
    assert_eq!(mesh.bounds, [[0.0; 3], [1.0, 1.0, 0.0]]);
    assert_eq!(mesh.material, default_material_id(FILE));
    let [(material_id, material)] = &imported.materials[..] else {
        panic!("glTF's default material: {:?}", imported.materials);
    };
    assert_eq!(*material_id, default_material_id(FILE));
    assert_eq!(
        material.metallic, 1.0,
        "glTF's default material is metallic"
    );
    // The default scene takes the file's own ID.
    let [(scene_id, scene)] = &imported.scenes[..] else {
        panic!("one scene");
    };
    assert_eq!(*scene_id, FILE);
    assert_eq!(scene.nodes[0].meshes, [*mesh_id]);
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("no name of their own")),
        "{:?}",
        imported.warnings
    );
}

#[test]
fn sub_assets_take_the_ids_of_their_names() {
    let mut file = Glb::triangle().with_material(r#"{ "name": "stone" }"#);
    file.json["meshes"][0]["name"] = "ruin".into();
    file.json["scenes"] =
        json(r#"[{ "name": "zone", "nodes": [0] }, { "name": "other", "nodes": [] }]"#);
    file.json["scene"] = 1.into();
    let imported = file.import().expect("imported");
    let material = sub_asset_ids(FILE, SubAssetKind::Material, &[Some("stone")])[0].id;
    assert_eq!(imported.materials[0].0, material);
    assert_eq!(imported.meshes[0].1.material, material);
    let mesh = sub_asset_ids(FILE, SubAssetKind::Mesh, &[Some("ruin")])[0].id;
    let primitive = sub_asset_ids(mesh, SubAssetKind::Mesh, &[None])[0].id;
    assert_eq!(imported.meshes[0].0, primitive);
    // The default scene, the second, takes the file's ID; the other its name's.
    let zone = sub_asset_ids(FILE, SubAssetKind::Scene, &[Some("zone"), Some("other")])[0].id;
    let ids: Vec<AssetId> = imported.scenes.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, [zone, FILE]);
}

#[test]
fn a_file_s_tangents_are_kept_and_ignored_without_normals() {
    let normals = [0.0, 0.0, 1.0].repeat(3);
    let tangents = [0.0, 1.0, 0.0, -1.0].repeat(3);
    let mut file = Glb::triangle()
        .with_attribute("NORMAL", "VEC3", &normals)
        .with_attribute("TANGENT", "VEC4", &tangents);
    let mesh = &file.import().expect("imported").meshes[0].1;
    assert!(
        mesh.tangents
            .iter()
            .all(|tangent| *tangent == [0.0, 1.0, 0.0, -1.0])
    );
    // Without normals, which are then flat, glTF asks that tangents be computed too: along u,
    // which grows along +x, with the bitangent up the image, +y.
    let mut file = Glb::triangle()
        .with_attribute("TANGENT", "VEC4", &tangents)
        .with_attribute("TEXCOORD_0", "VEC2", &[0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    let mesh = &file.import().expect("imported").meshes[0].1;
    for tangent in &mesh.tangents {
        let close = tangent
            .iter()
            .zip([1.0, 0.0, 0.0, 1.0])
            .all(|(got, want)| (got - want).abs() < 1e-5);
        assert!(close, "{tangent:?}");
    }
}

#[test]
fn primitives_that_share_attributes_keep_only_the_vertices_they_name() {
    // Six vertices with normals, read by two primitives of three each, as three.js writes a mesh
    // of several materials.
    let mut file = Glb::triangle();
    let corners: Vec<f32> = (0..6)
        .flat_map(|i| [i as f32, (i % 2) as f32, 0.0])
        .collect();
    let view = file.add_view(
        &corners
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let positions = file.add_accessor(view, FLOAT, 6, "VEC3");
    file.json["accessors"][positions]["min"] = json("[0, 0, 0]");
    file.json["accessors"][positions]["max"] = json("[5, 1, 0]");
    let view = file.add_view(
        &[0.0f32, 0.0, 1.0]
            .repeat(6)
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let normals = file.add_accessor(view, FLOAT, 6, "VEC3");
    let mut primitives = Vec::new();
    for first in [0u16, 3] {
        let view = file.add_view(&[first, first + 1, first + 2].map(u16::to_le_bytes).concat());
        let indices = file.add_accessor(view, UNSIGNED_SHORT, 3, "SCALAR");
        primitives.push(format!(
            r#"{{ "attributes": {{ "POSITION": {positions}, "NORMAL": {normals} }},
                  "indices": {indices} }}"#
        ));
    }
    file.json["meshes"][0]["primitives"] = json(&format!("[{}]", primitives.join(", ")));
    let imported = file.import().expect("imported");
    assert_eq!(imported.meshes.len(), 2);
    for (index, (_, mesh)) in imported.meshes.iter().enumerate() {
        assert_eq!(mesh.positions.len(), 3, "primitive {index}");
        assert_eq!(mesh.positions[0][0], 3.0 * index as f32);
    }
}

#[test]
fn extras_arrive_as_written_and_other_values_are_left_out() {
    let mut file = Glb::triangle();
    file.json["nodes"][0]["extras"] = json(
        r#"{ "collider": "box", "height": 2.5, "solid": true, "list": [1, 2], "none": null }"#,
    );
    let imported = file.import().expect("imported");
    assert_eq!(
        imported.scenes[0].1.nodes[0].extras,
        [
            ("collider".to_owned(), Extra::Text("box".to_owned())),
            ("height".to_owned(), Extra::Number(2.5)),
            ("solid".to_owned(), Extra::Bool(true)),
        ]
    );
    let left_out = imported
        .warnings
        .iter()
        .filter(|warning| warning.contains("is not a boolean, a number or a text"))
        .count();
    assert_eq!(left_out, 2, "{:?}", imported.warnings);
}

#[test]
fn materials_ask_for_the_textures_of_their_images_in_their_roles() {
    // One image, read as colors by the base color slot and as data by the occlusion slot: two
    // textures, under the image's ID and one derived from it.
    let mut file = Glb::triangle().with_image(&png(4, 4)).with_material(
        r#"{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } },
             "occlusionTexture": { "index": 0, "strength": 0.5 } }"#,
    );
    file.json["samplers"] = json(r#"[{ "magFilter": 9728, "wrapS": 33071, "wrapT": 33648 }]"#);
    file.json["textures"][0]["sampler"] = 0.into();
    let imported = file.import().expect("imported");
    let image = sub_asset_ids(FILE, SubAssetKind::Image, &[None])[0].id;
    let [embedded] = &imported.embedded[..] else {
        panic!("one embedded image");
    };
    assert_eq!(embedded.id, image);
    assert_eq!(embedded.roles, [TextureRole::Color, TextureRole::Data]);
    let material = &imported.materials[0].1;
    let base = material.base_color_texture.expect("a base color texture");
    let occlusion = material.occlusion_texture.expect("an occlusion texture");
    assert_eq!(base.texture, texture_id(image, TextureRole::Color));
    assert_eq!(occlusion.texture, texture_id(image, TextureRole::Data));
    assert_eq!(material.occlusion_strength, 0.5);
    assert_eq!(base.sampler.mag_filter, TextureFilter::Nearest);
    assert_eq!(base.sampler.min_filter, TextureFilter::Linear);
    assert_eq!(
        (base.sampler.wrap_u, base.sampler.wrap_v),
        (TextureWrap::ClampToEdge, TextureWrap::MirroredRepeat)
    );
}

#[test]
fn a_node_hierarchy_deeper_than_the_stack_holds_calls_is_read() {
    // Each node the only child of the one before, the last one with the triangle, a hundred
    // thousand levels down, a few MiB of JSON.
    let depth = 100_000;
    let mut file = Glb::triangle();
    let mut nodes: Vec<String> = (1..depth)
        .map(|child| format!(r#"{{ "children": [{child}] }}"#))
        .collect();
    nodes.push(r#"{ "mesh": 0 }"#.to_owned());
    file.json["nodes"] = json(&format!("[{}]", nodes.join(", ")));
    let imported = file.import().expect("imported");
    let scene = &imported.scenes[0].1;
    assert_eq!(scene.nodes.len(), depth);
    assert_eq!(scene.nodes[depth - 1].parent, Some(depth as u32 - 2));
}

#[test]
fn lights_are_read_in_the_node_s_space() {
    let mut file = Glb::triangle().with_light(
        r#"{ "type": "spot", "color": [1, 0.5, 0.25], "intensity": 40, "range": 8,
             "spot": { "innerConeAngle": 0.2, "outerConeAngle": 0.5 } }"#,
    );
    let imported = file.import().expect("imported");
    let light = imported.scenes[0].1.nodes[1].light.expect("a light");
    assert_eq!(
        light.kind,
        LightKind::Spot {
            inner_cone_angle: 0.2,
            outer_cone_angle: 0.5
        }
    );
    assert_eq!(
        (light.color, light.intensity, light.range),
        ([1.0, 0.5, 0.25], 40.0, Some(8.0))
    );
}

#[test]
fn lights_that_gltf_does_not_allow_are_refused() {
    for (light, expected) in [
        (
            r#"{ "type": "point", "intensity": -1 }"#,
            "has an intensity of -1; glTF requires at least 0",
        ),
        // 10³⁹ is beyond the largest 32-bit float: the `gltf` crate reads it as an infinity.
        (
            r#"{ "type": "point", "intensity": 1e39 }"#,
            "has an intensity of inf",
        ),
        (
            r#"{ "type": "directional", "color": [1, -0.5, 1] }"#,
            "has a color of [1.0, -0.5, 1.0]",
        ),
        (
            r#"{ "type": "point", "range": 0 }"#,
            "has a range of 0; glTF requires a positive range",
        ),
        // Equal angles, as Blender writes them for a spot whose blend is 0.
        (
            r#"{ "type": "spot", "spot": { "innerConeAngle": 0.5, "outerConeAngle": 0.5 } }"#,
            "has cone angles of 0.5 and 0.5 radians",
        ),
        (
            r#"{ "type": "spot", "spot": { "outerConeAngle": 2 } }"#,
            "has cone angles of 0 and 2 radians",
        ),
        // What the `gltf` crate's validation refuses, before its readers would panic on it.
        (r#"{ "type": "spot" }"#, "spot: Missing data"),
        (r#"{ "type": "area" }"#, "type: Invalid value"),
    ] {
        let error = Glb::triangle().with_light(light).refusal();
        assert!(error.contains(expected), "{light}: {error}");
    }
}

#[test]
fn a_glb_header_that_declares_a_length_shorter_than_itself_is_refused() {
    // A header alone: the magic, version 2, and a length of 0, from which the `gltf` crate
    // subtracts the header's 12 bytes, an overflow that panics in debug builds.
    let header = [b"glTF".as_slice(), &2u32.to_le_bytes(), &0u32.to_le_bytes()].concat();
    let error = import_bytes(&header).expect_err("refused");
    assert!(
        error.contains("declares a length of 0 bytes, shorter than the header"),
        "{error}"
    );
}

#[test]
fn positions_that_name_no_accessor_are_refused() {
    // The `gltf` crate's validation reads the accessor of a primitive's positions without checking
    // that it exists.
    let mut file = Glb::triangle();
    file.json["meshes"][0]["primitives"][0]["attributes"]["POSITION"] = 99.into();
    let error = file.refusal();
    assert!(
        error.contains("mesh 0 primitive 0 reads its positions from accessor 99"),
        "{error}"
    );
}

#[test]
fn images_whose_data_cannot_be_found_are_refused() {
    let textured = || {
        Glb::triangle()
            .with_image(&png(2, 2))
            .with_material(r#"{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } }"#)
    };
    let mut past_the_buffer = textured();
    let view = past_the_buffer.json["images"][0]["bufferView"].clone();
    let length = past_the_buffer.bin.len() + 4;
    past_the_buffer.json["bufferViews"][index(&view)]["byteLength"] = length.into();
    let error = past_the_buffer.refusal();
    assert!(error.contains("outside its buffer"), "{error}");

    let mut without_type = textured();
    remove(&mut without_type.json["images"][0], "mimeType");
    assert!(without_type.refusal().contains("without a MIME type"));

    let mut without_data = textured();
    remove(&mut without_data.json["images"][0], "bufferView");
    remove(&mut without_data.json["images"][0], "mimeType");
    assert!(without_data.refusal().contains("no data"));

    let mut other_type = textured();
    other_type.json["images"][0]["mimeType"] = "image/webp".into();
    assert!(
        other_type
            .refusal()
            .contains("image/webp image; only PNG and JPEG images are read")
    );
}

#[test]
fn data_uris_are_refused_without_being_quoted_and_missing_files_are_named() {
    // What a data URI holds, as text: a whole image or buffer in a real file, which the error must
    // leave out.
    const PAYLOAD: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAAC";
    let data_uri = format!("data:application/octet-stream;base64,{PAYLOAD}");
    let mut image = Glb::triangle()
        .with_image(&png(2, 2))
        .with_material(r#"{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } }"#);
    remove(&mut image.json["images"][0], "bufferView");
    image.json["images"][0]["uri"] = data_uri.as_str().into();
    let error = image.refusal();
    assert!(
        error.contains("image 0 is given as a data URI") && !error.contains(PAYLOAD),
        "{error}"
    );

    let mut buffer = Glb::triangle();
    push(
        &mut buffer.json["buffers"],
        json(&format!(r#"{{ "byteLength": 24, "uri": "{data_uri}" }}"#)),
    );
    let error = buffer.refusal();
    assert!(
        error.contains("buffer 1 is given as a data URI") && !error.contains(PAYLOAD),
        "{error}"
    );

    let mut missing = Glb::triangle();
    push(
        &mut missing.json["buffers"],
        json(r#"{ "byteLength": 24, "uri": "other.bin" }"#),
    );
    let error = missing.refusal();
    assert!(
        error.contains("buffer 1, the file `other.bin`, cannot be read"),
        "{error}"
    );

    let mut not_a_source = Glb::triangle()
        .with_image(&png(2, 2))
        .with_material(r#"{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } }"#);
    remove(&mut not_a_source.json["images"][0], "bufferView");
    remove(&mut not_a_source.json["images"][0], "mimeType");
    not_a_source.json["images"][0]["uri"] = "rock.png".into();
    let error = not_a_source.refusal();
    assert!(
        error.contains("image 0 is the file `rock.png`, which is not a PNG or JPEG source"),
        "{error}"
    );
}

/// A change that a test makes to a GLB file.
type Change = fn(&mut Glb);

#[test]
fn accessors_the_importer_cannot_read_are_refused() {
    // Each change of the triangle, and what the error says.
    let cases: [(Change, &str); 9] = [
        (
            |file| {
                let view = file.add_view(&[0.0f32, 1.0, 2.0].map(f32::to_le_bytes).concat());
                let indices = file.add_accessor(view, FLOAT, 3, "SCALAR");
                file.json["meshes"][0]["primitives"][0]["indices"] = indices.into();
            },
            "stores its indices as F32 Scalar",
        ),
        (
            |file| {
                let view = file.add_view(&[0; 6]);
                let uvs = file.add_accessor(view, BYTE, 3, "VEC2");
                file.json["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"] = uvs.into();
            },
            "stores its texture coordinates as I8 Vec2",
        ),
        (
            |file| {
                let view = file.add_view(&[0.0f32; 9].map(f32::to_le_bytes).concat());
                let tangents = file.add_accessor(view, FLOAT, 3, "VEC3");
                file.json["meshes"][0]["primitives"][0]["attributes"]["TANGENT"] = tangents.into();
            },
            "stores its tangents as F32 Vec3",
        ),
        (
            |file| file.json["accessors"][0]["componentType"] = UNSIGNED_SHORT.into(),
            "stores its positions as U16 Vec3",
        ),
        (
            |file| file.json["bufferViews"][0]["byteStride"] = 4.into(),
            "lays out its positions 4 bytes apart",
        ),
        (
            |file| file.json["accessors"][0]["count"] = 0.into(),
            "has an empty accessor of positions",
        ),
        (
            // Zeros but for one vertex: a sparse accessor without a buffer view, whose count no
            // data backs.
            |file| {
                let indices = file.add_view(&2u32.to_le_bytes());
                let values = file.add_view(&[0.0f32, 1.0, 0.0].map(f32::to_le_bytes).concat());
                remove(&mut file.json["accessors"][0], "bufferView");
                file.json["accessors"][0]["sparse"] = json(&format!(
                    r#"{{ "count": 1,
                          "indices": {{ "bufferView": {indices}, "componentType": 5125 }},
                          "values": {{ "bufferView": {values} }} }}"#
                ));
            },
            "stores its positions in a sparse accessor",
        ),
        (
            // Zeros: glTF allows an accessor that is neither sparse nor in a buffer view, but the
            // `gltf` crate's validation refuses it before the importer's checks.
            |file| remove(&mut file.json["accessors"][0], "bufferView"),
            "accessors[0].bufferView: Missing data",
        ),
        (
            |file| {
                let view = file.add_view(&[0, 1, 2].map(u16::to_le_bytes).concat());
                let indices = file.add_accessor(view, UNSIGNED_SHORT, 3, "SCALAR");
                file.json["meshes"][0]["primitives"][0]["indices"] = indices.into();
                file.json["bufferViews"][view]["byteOffset"] = file.bin.len().into();
            },
            "has indices in a buffer view that runs past the end of its buffer",
        ),
    ];
    for (change, expected) in cases {
        let mut file = Glb::triangle();
        change(&mut file);
        let error = file.refusal();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn an_index_that_names_no_vertex_is_refused_with_or_without_normals() {
    for normals in [false, true] {
        let mut file = Glb::triangle();
        if normals {
            file = file.with_attribute("NORMAL", "VEC3", &[0.0, 0.0, 1.0].repeat(3));
        }
        let view = file.add_view(&[0, 1, 7].map(u16::to_le_bytes).concat());
        let indices = file.add_accessor(view, UNSIGNED_SHORT, 3, "SCALAR");
        file.json["meshes"][0]["primitives"][0]["indices"] = indices.into();
        let error = file.refusal();
        assert!(
            error.contains("index 7 names no vertex"),
            "normals {normals}: {error}"
        );
    }
}

#[test]
fn values_that_are_not_finite_numbers_are_refused() {
    // The second corner's x is NaN, which glTF forbids and its validation, which reads no binary
    // data, lets through.
    let mut file = Glb::triangle();
    file.bin[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(
        file.refusal()
            .contains("a vertex value that is not a finite number")
    );
    // 10³⁹ is beyond the largest 32-bit float.
    let mut file = Glb::triangle();
    file.json["nodes"][0]["translation"] = json("[1e39, 0, 0]");
    assert!(
        file.refusal()
            .contains("node 0's transform is not made of finite numbers")
    );
}

#[test]
fn meshes_larger_than_the_gpu_takes_are_refused_before_they_are_copied() {
    // The triangle drawn over and over, with 8-bit indices: a file of a few MiB, whose flat normals
    // take three vertices per triangle, one triangle more than a mesh takes.
    let triangles = MAX_MESH_VERTICES / 3 + 1;
    let mut file = Glb::triangle();
    let view = file.add_view(&[0, 1, 2].repeat(triangles));
    let indices = file.add_accessor(view, UNSIGNED_BYTE, 3 * triangles as u32, "SCALAR");
    file.json["meshes"][0]["primitives"][0]["indices"] = indices.into();
    let error = file.refusal();
    assert!(
        error.contains(&format!("needs {} vertices", 3 * triangles)),
        "{error}"
    );

    // Four primitives, each within a mesh's limit, that together pass a file's.
    let per_primitive = MAX_FILE_VERTICES / 4 + 3;
    let mut file = Glb::triangle();
    let view = file.add_view(&[0, 1, 2].repeat(per_primitive / 3));
    let indices = file.add_accessor(
        view,
        UNSIGNED_BYTE,
        (per_primitive / 3 * 3) as u32,
        "SCALAR",
    );
    let primitive = format!(r#"{{ "attributes": {{ "POSITION": 0 }}, "indices": {indices} }}"#);
    file.json["meshes"][0]["primitives"] =
        json(&format!("[{}]", [primitive.as_str(); 4].join(", ")));
    let error = file.refusal();
    assert!(
        error.contains("the file's meshes, up to mesh 0 primitive"),
        "{error}"
    );
}

#[test]
fn node_hierarchies_that_are_not_trees_are_refused() {
    let mut shared = Glb::triangle();
    shared.json["nodes"] = json(r#"[{ "mesh": 0 }, { "children": [0] }, { "children": [0] }]"#);
    shared.json["scenes"][0]["nodes"] = json("[1, 2]");
    assert!(shared.refusal().contains("node 0 is reached twice"));

    let mut cycle = Glb::triangle();
    cycle.json["nodes"][0]["children"] = json("[0]");
    assert!(cycle.refusal().contains("node 0 is reached twice"));
}

#[test]
fn extensions_the_importer_does_not_read_are_refused_when_required() {
    let mut file = Glb::triangle();
    file.json["extensionsUsed"] = json(r#"["EXT_meshopt_compression"]"#);
    file.json["extensionsRequired"] = json(r#"["EXT_meshopt_compression"]"#);
    assert!(file.refusal().contains("Unsupported extension"));
    // Used but not required: left out, with a warning.
    let mut file = Glb::triangle();
    file.json["extensionsUsed"] = json(r#"["EXT_meshopt_compression"]"#);
    let imported = file.import().expect("imported");
    assert!(
        imported
            .warnings
            .iter()
            .any(|w| w.contains("EXT_meshopt_compression"))
    );
    // KHR_texture_transform would draw textures wrong: refused even when only used.
    let mut file = Glb::triangle();
    file.json["extensionsUsed"] = json(r#"["KHR_texture_transform"]"#);
    assert!(
        file.refusal()
            .contains("KHR_texture_transform, which the importer does not read")
    );
}

#[test]
fn a_texture_read_through_another_coordinate_set_is_refused() {
    let mut file = Glb::triangle()
        .with_image(&png(2, 2))
        .with_material(r#"{ "normalTexture": { "index": 0, "texCoord": 1 } }"#);
    assert!(
        file.refusal()
            .contains("reads its normal texture through coordinate set 1")
    );
}

/// An empty directory of its own for `name`, under Cargo's directory for temporary files of
/// integration tests.
fn directory(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slime-cook-gltf")
        .join(name);
    if path.exists() {
        fs::remove_dir_all(&path).expect("the test's old directory can be removed");
    }
    fs::create_dir_all(&path).expect("the test's directory can be created");
    path
}

/// Writes, into `directory`, a `.gltf` file of the triangle whose buffer is `triangle.bin` beside
/// it and whose normal texture is the image file `textures/rock.png`, as Blender exports them.
fn gltf_with_files_beside(directory: &Path) {
    let mut file = Glb::triangle().with_material(r#"{ "normalTexture": { "index": 0 } }"#);
    file.json["images"] = json(r#"[{ "uri": "textures/rock.png" }]"#);
    file.json["textures"] = json(r#"[{ "source": 0 }]"#);
    file.json["buffers"][0] = json(&format!(
        r#"{{ "byteLength": {}, "uri": "triangle.bin" }}"#,
        file.bin.len()
    ));
    file.json["meshes"][0]["name"] = "triangle".into();
    file.json["materials"][0]["name"] = "rock".into();
    fs::create_dir_all(directory.join("textures")).expect("created");
    fs::write(directory.join("triangle.bin"), &file.bin).expect("written");
    fs::write(
        directory.join("triangle.gltf"),
        gltf::json::serialize::to_vec(&file.json).expect("the JSON is written"),
    )
    .expect("written");
    fs::write(directory.join("textures/rock.png"), png(4, 4)).expect("written");
}

#[test]
fn a_gltf_file_cooks_with_its_buffer_and_images_beside_it() {
    let root = directory("beside");
    let sources = root.join("sources");
    gltf_with_files_beside(&sources);
    let output = root.join("cooked");
    let cooked = cook(&sources, &output).expect("the cook succeeds");
    // A mesh, its material, the scene, and the image's texture of normals, not of colors: no
    // material reads it as colors.
    assert_eq!(cooked.cooked.len(), 4, "{cooked:?}");
    let ids = scan(&sources, Mode::Check).expect("scanned");
    let id = |name: &str| {
        ids.sources
            .iter()
            .find(|source| source.path.ends_with(name))
            .expect("the source is there")
            .id
    };
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    let scene = assets.scene(id("triangle.gltf")).expect("the file's scene");
    let mesh = assets.mesh(scene.nodes[0].meshes[0]).expect("its mesh");
    let material = assets.material(mesh.material).expect("its material");
    let normal = material.normal_texture.expect("a normal texture").texture;
    assert_eq!(normal, texture_id(id("rock.png"), TextureRole::Normal));
    assert_eq!(
        assets.texture(normal).expect("the texture").encoding,
        TextureEncoding::Bc5
    );
    assert!(
        assets.texture(id("rock.png")).is_err(),
        "no texture of colors"
    );

    // Cooked again, nothing changes; moved with their metadata, nothing is cooked again either.
    assert!(
        cook(&sources, &output)
            .expect("cooked again")
            .cooked
            .is_empty()
    );
    let moved = root.join("moved");
    fs::rename(&sources, &moved).expect("moved");
    let again = cook(&moved, &output).expect("cooked again");
    assert!(again.cooked.is_empty(), "{again:?}");
    let assets = CookedAssets::open(&output).expect("the manifest is read");
    assert!(assets.scene(id("triangle.gltf")).is_ok());
}

#[test]
fn a_gltf_file_whose_image_is_missing_fails_the_cook_with_its_path() {
    let root = directory("missing-image");
    let sources = root.join("sources");
    gltf_with_files_beside(&sources);
    fs::remove_file(sources.join("textures/rock.png")).expect("removed");
    let error = cook(&sources, &root.join("cooked")).expect_err("refused");
    assert!(matches!(error, CookError::Import { .. }), "{error:?}");
    let message = error.to_string();
    assert!(
        message.contains("triangle.gltf") && message.contains("textures/rock.png"),
        "{message}"
    );
}

#[test]
fn materials_that_gltf_does_not_allow_are_refused() {
    for (material, expected) in [
        (
            r#"{ "name": "bright", "pbrMetallicRoughness": { "baseColorFactor": [1.2, 1, 1, 1] } }"#,
            "material `bright` has a base color of [1.2, 1.0, 1.0, 1.0]; glTF requires values \
             from 0 to 1",
        ),
        (
            r#"{ "pbrMetallicRoughness": { "metallicFactor": 1.01 } }"#,
            "has a metallic factor of [1.01]",
        ),
        (
            r#"{ "emissiveFactor": [0, -0.5, 0] }"#,
            "has an emissive factor of [0.0, -0.5, 0.0]",
        ),
        (
            r#"{ "alphaMode": "MASK", "alphaCutoff": -1 }"#,
            "has an alpha cutoff of -1; glTF requires at least 0",
        ),
        (
            r#"{ "extensions": { "KHR_materials_emissive_strength": { "emissiveStrength": -2 } } }"#,
            "has an emissive strength of -2; glTF requires at least 0",
        ),
    ] {
        let mut file = Glb::triangle().with_material(material);
        file.json["extensionsUsed"] = json(r#"["KHR_materials_emissive_strength"]"#);
        let error = file.refusal();
        assert!(error.contains(expected), "{material}: {error}");
    }
    // A cutoff above 1, which glTF allows, draws nothing: it is kept.
    let mut file = Glb::triangle().with_material(r#"{ "alphaMode": "MASK", "alphaCutoff": 1.5 }"#);
    let imported = file.import().expect("imported");
    assert_eq!(imported.materials[0].1.alpha_cutoff, Some(1.5));
}

/// How a lamp glows: a component that the tests' nodes carry as a typed property.
#[derive(Clone, Copy, Debug, PartialEq, slime_reflect::Reflect)]
#[reflect(name = "test::Glow")]
struct Glow {
    #[reflect(range(min = 0.0))]
    intensity: f32,
}
impl slime_world::Component for Glow {}

#[test]
fn a_typed_property_becomes_its_component_and_a_wrong_one_refuses_the_scene() {
    let mut file = Glb::triangle();
    file.json["nodes"][0]["name"] = "lamp".into();
    file.json["nodes"][0]["extras"] =
        json(r#"{ "slime:test::Glow": "{ intensity = 40.0 }", "label": "porch" }"#);
    let imported = file.import().expect("imported");
    let scene = &imported.scenes[0].1;
    assert_eq!(
        scene.nodes[0].extras,
        [("label".to_owned(), Extra::Text("porch".to_owned()))]
    );
    let mut world = World::new();
    world.register::<Glow>();
    let resolved = ResolvedScene::from_cooked(FILE, scene, world.registry()).expect("resolved");
    world.spawn_scene("zone", &resolved).expect("spawned");
    let lamp = world
        .find(&PersistentId::new(["zone", "lamp"]))
        .expect("the lamp");
    assert_eq!(world.get::<Glow>(lamp), Some(&Glow { intensity: 40.0 }));
    // A field that the type does not have, and a value out of its range: the scene is refused.
    for (value, field) in [
        ("{ intensity = 40.0, colour = 1 }", "colour"),
        ("{ intensity = -1.0 }", "intensity"),
    ] {
        file.json["nodes"][0]["extras"] = json(&format!(r#"{{ "slime:test::Glow": "{value}" }}"#));
        let imported = file.import().expect("imported");
        let error = ResolvedScene::from_cooked(FILE, &imported.scenes[0].1, world.registry())
            .expect_err("refused");
        let text = error.to_string();
        assert!(
            text.contains("node `lamp`") && text.contains("`test::Glow") && text.contains(field),
            "{text}"
        );
    }
    // A value that is not written in the engine's notation: the import is refused.
    file.json["nodes"][0]["extras"] = json(r#"{ "slime:test::Glow": "intensity 40" }"#);
    let refusal = file.refusal();
    assert!(
        refusal.contains("lamp's property `slime:test::Glow`")
            && refusal.contains("engine's notation"),
        "{refusal}"
    );
}
