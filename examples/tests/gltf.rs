//! The reference image test of the glTF scene: the asset of `examples/assets/scene.glb`, loaded by
//! the examples' loader and rendered offscreen from two fixed camera poses, must match
//! `tests/reference/gltf-front.png` and `tests/reference/gltf-back.png` within the tolerance of
//! ADR-0018 and ADR-0028.
//!
//! The references were rendered with lavapipe, like the others. To render them again, after a
//! deliberate change of the asset or of the renderer:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test gltf
//! ```
//!
//! The other tests give the loader small GLB files built in memory, which hold what the asset
//! does not: what the loader must refuse with an error rather than panic on.

#[path = "../common/orbit.rs"]
mod orbit;
#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/sample_scene.rs"]
mod sample_scene;

use std::borrow::Cow;

use gltf::json::Value;
use slime_gfx::{Gpu, TextureFormat};

use sample_scene::SampleScene;

const SIZE: u32 = 256;

#[test]
fn the_gltf_scene_matches_its_reference_images() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target =
        gpu.create_render_target("reference glTF", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
    let mut scene = SampleScene::new(&gpu, target.format(), sample_scene::ASSET)
        .expect("the sample asset loads");
    let bounds = *scene.bounds();
    // Seven primitives, one per object of the asset, and the triangles of their meshes.
    for (name, yaw, pitch) in [("gltf-front", 0.6, 0.45), ("gltf-back", 3.6, 0.25)] {
        let camera = orbit::camera_at(bounds.center(), bounds.radius() * 2.2, yaw, pitch);
        let stats = scene.render(&gpu, &target, camera);
        assert_eq!((stats.draw_calls, stats.triangles), (7, 2274));
        let image = gpu
            .read_render_target(&target)
            .expect("the image is read back");
        if reference_image::update_requested() {
            reference_image::write(name, &image);
        } else {
            reference_image::check(name, name, &image, gpu.adapter_name());
        }
    }
}

#[test]
fn an_image_larger_than_the_gpu_accepts_is_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let side = gpu.max_texture_dimension() + 1;
    let error = refusal(&gpu, &mut Glb::triangle().with_texture(&png(side, 1)));
    assert!(error.contains(&format!("{side}x1")), "{error}");
}

/// A GLB file built in memory: one triangle facing +Z, with positions only, in the one node of
/// the default scene, which each test changes.
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

    /// Gives the triangle a material whose base color texture is the PNG image `png`, embedded
    /// in the file.
    fn with_texture(mut self, png: &[u8]) -> Self {
        let view = self.add_view(png);
        self.json["images"] = json(&format!(
            r#"[{{ "bufferView": {view}, "mimeType": "image/png" }}]"#
        ));
        self.json["textures"] = json(r#"[{ "source": 0 }]"#);
        self.json["materials"] =
            json(r#"[{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } }]"#);
        self.json["meshes"][0]["primitives"][0]["material"] = 0.into();
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
}

/// The component type of 32-bit floating-point numbers, in glTF.
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

/// A white PNG image of `width` × `height` pixels.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("the PNG header is written");
    writer
        .write_image_data(&vec![255; width as usize * height as usize * 4])
        .expect("the PNG image is written");
    writer.finish().expect("the PNG image is finished");
    bytes
}

/// The message of the error with which the loader refuses `file`.
fn refusal(gpu: &Gpu, file: &mut Glb) -> String {
    match SampleScene::new(gpu, TextureFormat::Rgba8UnormSrgb, &file.bytes()) {
        Ok(_) => panic!("the loader accepted a file it must refuse"),
        Err(error) => error.to_string(),
    }
}
