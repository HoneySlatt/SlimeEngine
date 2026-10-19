//! Sources damaged (`docs/adr/0096-the-files-the-engine-reads.md`): a GLB file, PNG and JPEG
//! images, and a metadata file, each valid, cut at every byte, its bytes changed and mutated at
//! random, are refused or imported, as the cooker imports them, and never make it panic.

#[path = "../../slime-assets/tests/common/damage.rs"]
mod damage;

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use damage::damage;
use slime_assets::AssetId;
use slime_cook::gltf_import::GltfFile;
use slime_cook::image::{self, ImageKind};
use slime_cook::metadata;
use slime_cook::texture::cook_texture;

/// A PNG image of `width` × `height` texels of red, green, blue and alpha.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    let mut writer = encoder.write_header().expect("the PNG header is written");
    let texels: Vec<u8> = (0..width * height * 4).map(|index| index as u8).collect();
    writer
        .write_image_data(&texels)
        .expect("the PNG image is written");
    writer.finish().expect("the PNG image is finished");
    bytes
}

#[test]
fn damaged_images_are_refused_or_decoded_never_a_panic() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (kind, file) in [
        (ImageKind::Png, png(6, 5)),
        (
            ImageKind::Jpeg,
            std::fs::read(fixtures.join("blue.jpg")).expect("read"),
        ),
        (
            ImageKind::Jpeg,
            std::fs::read(fixtures.join("stripes-cmyk.jpg")).expect("read"),
        ),
    ] {
        assert!(image::decode(kind, &file).is_ok());
        damage(&file, |bytes| image::decode(kind, bytes));
    }
}

#[test]
fn a_damaged_metadata_file_is_refused_or_read_never_a_panic() {
    let file = metadata::write(AssetId::from_u128(
        0x6f1c_2b3a_0d4e_4f5a_8b6c_7d8e_9f0a_1b2c,
    ));
    assert!(metadata::read(&file).is_ok());
    damage(file.as_bytes(), |bytes| {
        metadata::read(&String::from_utf8_lossy(bytes))
    });
}

#[test]
fn a_damaged_glb_file_is_refused_or_imported_never_a_panic() {
    // A textured triangle with normals, texture coordinates and indices, a material that reads an
    // embedded image, a node with a spot light and a typed property under another node, as Blender
    // exports them.
    let image = png(4, 4);
    let mut bin = Vec::new();
    let floats =
        |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let mut views = Vec::new();
    for part in [
        floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        floats(&[0.0, 0.0, 1.0].repeat(3)),
        floats(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]),
        [0u16, 1, 2].iter().flat_map(|i| i.to_le_bytes()).collect(),
        image,
    ] {
        views.push((bin.len(), part.len()));
        bin.extend_from_slice(&part);
        bin.resize(bin.len().next_multiple_of(4), 0);
    }
    let view = |index: usize| {
        format!(
            r#"{{ "buffer": 0, "byteOffset": {}, "byteLength": {} }}"#,
            views[index].0, views[index].1
        )
    };
    let json = format!(
        r#"{{
            "asset": {{ "version": "2.0" }},
            "extensionsUsed": ["KHR_lights_punctual"],
            "extensions": {{ "KHR_lights_punctual": {{ "lights": [
                {{ "type": "spot", "intensity": 40, "range": 8,
                   "spot": {{ "innerConeAngle": 0.2, "outerConeAngle": 0.5 }} }}
            ] }} }},
            "buffers": [{{ "byteLength": {} }}],
            "bufferViews": [{}, {}, {}, {}, {}],
            "accessors": [
                {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                   "min": [0, 0, 0], "max": [1, 1, 0] }},
                {{ "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" }},
                {{ "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" }},
                {{ "bufferView": 3, "componentType": 5123, "count": 3, "type": "SCALAR" }}
            ],
            "images": [{{ "bufferView": 4, "mimeType": "image/png", "name": "bark" }}],
            "textures": [{{ "source": 0 }}],
            "materials": [{{ "name": "bark",
                "pbrMetallicRoughness": {{ "baseColorTexture": {{ "index": 0 }} }} }}],
            "meshes": [{{ "name": "leaf", "primitives": [{{
                "attributes": {{ "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 }},
                "indices": 3, "material": 0 }}] }}],
            "nodes": [
                {{ "name": "tree", "mesh": 0, "children": [1],
                   "extras": {{ "height": 2.5, "slime:Name": "\"tree\"" }} }},
                {{ "name": "lantern", "translation": [0, 2, 0],
                   "extensions": {{ "KHR_lights_punctual": {{ "light": 0 }} }} }}
            ],
            "scenes": [{{ "nodes": [0] }}],
            "scene": 0
        }}"#,
        bin.len(),
        view(0),
        view(1),
        view(2),
        view(3),
        view(4),
    );
    let glb = gltf::Glb {
        header: gltf::binary::Header {
            magic: *b"glTF",
            version: 2,
            // `to_vec` computes it.
            length: 0,
        },
        json: Cow::Owned(json.into_bytes()),
        bin: Some(Cow::Borrowed(&bin)),
    }
    .to_vec()
    .expect("the GLB file is written");
    // What the cooker makes of a file: its import, its images decoded and cooked, and every asset
    // encoded.
    let cook = |bytes: &[u8]| -> Result<usize, String> {
        let file = GltfFile::open(Path::new("tree.glb"), bytes, Path::new("."))
            .map_err(|error| error.to_string())?;
        let imported = file
            .import(AssetId::from_u128(9), &HashMap::new())
            .map_err(|error| error.to_string())?;
        let mut encoded = 0;
        for embedded in &imported.embedded {
            let decoded =
                image::decode(embedded.kind, &embedded.bytes).map_err(|error| error.to_string())?;
            for &role in &embedded.roles {
                encoded += cook_texture(&decoded, role).to_ktx2().len();
            }
        }
        encoded += imported
            .meshes
            .iter()
            .map(|(_, mesh)| mesh.to_bytes().len())
            .sum::<usize>();
        encoded += imported
            .materials
            .iter()
            .map(|(_, material)| material.to_bytes().len())
            .sum::<usize>();
        encoded += imported
            .scenes
            .iter()
            .map(|(_, scene)| scene.to_bytes().len())
            .sum::<usize>();
        Ok(encoded)
    };
    cook(&glb).expect("the valid file cooks");
    damage(&glb, cook);
}
