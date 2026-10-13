//! The reference image test of the glTF scene: the asset of `examples/assets/scene.glb`, loaded by
//! the examples' loader and rendered offscreen from two fixed camera poses, must match
//! `tests/reference/gltf-front.png` and `tests/reference/gltf-back.png` within the tolerance of
//! the driver that renders them (ADR-0018, amended by ADR-0028 and ADR-0030; the numbers are in
//! `common/reference_image.rs`).
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
//! does not: what the loader must refuse with an error rather than panic on, a mirrored node, and
//! a node hierarchy deeper than a thread's stack holds calls of a recursive walk.
//! The unit tests of `common/gltf.rs` and `common/orbit.rs` run here too, the one test program
//! that includes both.

#[path = "../common/orbit.rs"]
mod orbit;
#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/sample_scene.rs"]
mod sample_scene;

use std::borrow::Cow;

use gltf::json::Value;
use slime_gfx::{Gpu, TextureFormat};
use slime_render::glam::Vec3;

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
        let radius = bounds.radius();
        let camera = orbit::camera_at(bounds.center(), radius, radius * 2.2, yaw, pitch);
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

#[test]
fn images_whose_data_cannot_be_found_are_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let textured = || Glb::triangle().with_texture(&png(2, 2));

    let mut past_the_chunk = textured();
    let view = past_the_chunk.json["images"][0]["bufferView"].clone();
    let length = past_the_chunk.bin.len() + 4;
    past_the_chunk.json["bufferViews"][index(&view)]["byteLength"] = length.into();
    let error = refusal(&gpu, &mut past_the_chunk);
    assert!(error.contains("outside the file's binary chunk"), "{error}");

    let mut without_type = textured();
    remove(&mut without_type.json["images"][0], "mimeType");
    let error = refusal(&gpu, &mut without_type);
    assert!(error.contains("without a MIME type"), "{error}");

    let mut without_data = textured();
    remove(&mut without_data.json["images"][0], "bufferView");
    remove(&mut without_data.json["images"][0], "mimeType");
    let error = refusal(&gpu, &mut without_data);
    assert!(error.contains("no data"), "{error}");
}

#[test]
fn accessors_the_loader_cannot_read_are_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // Each change of the triangle, and what the error says.
    let cases: [(Change, &str); 7] = [
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
            |file| {
                let view = file.add_view(&[0, 1, 2].map(u16::to_le_bytes).concat());
                let indices = file.add_accessor(view, UNSIGNED_SHORT, 3, "SCALAR");
                file.json["meshes"][0]["primitives"][0]["indices"] = indices.into();
                file.json["bufferViews"][view]["byteOffset"] = file.bin.len().into();
            },
            "has indices in a buffer view that runs past the file's binary chunk",
        ),
    ];
    for (change, expected) in cases {
        let mut file = Glb::triangle();
        change(&mut file);
        let error = refusal(&gpu, &mut file);
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn an_index_that_names_no_vertex_is_refused_in_a_primitive_without_normals() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut file = Glb::triangle();
    let view = file.add_view(&[0, 1, 7].map(u16::to_le_bytes).concat());
    let indices = file.add_accessor(view, UNSIGNED_SHORT, 3, "SCALAR");
    file.json["meshes"][0]["primitives"][0]["indices"] = indices.into();
    let error = refusal(&gpu, &mut file);
    assert!(error.contains("index 7 names no vertex"), "{error}");
}

#[test]
fn a_primitive_without_normals_too_large_for_the_gpu_is_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // The triangle drawn over and over, with 8-bit indices: a file of a few MiB, whose flat
    // normals take three vertices of 32 bytes per triangle, one triangle more than the largest
    // vertex buffer holds.
    let triangles = gpu.max_buffer_size() / (3 * 32) + 1;
    let mut file = Glb::triangle();
    let view = file.add_view(&[0, 1, 2].repeat(triangles as usize));
    let indices = file.add_accessor(view, UNSIGNED_BYTE, 3 * triangles as u32, "SCALAR");
    file.json["meshes"][0]["primitives"][0]["indices"] = indices.into();
    let error = refusal(&gpu, &mut file);
    assert!(
        error.contains(&format!("{} vertices", 3 * triangles)),
        "{error}"
    );
}

#[test]
fn node_hierarchies_that_are_not_trees_are_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");

    let mut shared = Glb::triangle();
    shared.json["nodes"] = json(r#"[{ "mesh": 0 }, { "children": [0] }, { "children": [0] }]"#);
    shared.json["scenes"][0]["nodes"] = json("[1, 2]");
    let error = refusal(&gpu, &mut shared);
    assert!(error.contains("node 0 is reached twice"), "{error}");

    let mut cycle = Glb::triangle();
    cycle.json["nodes"][0]["children"] = json("[0]");
    let error = refusal(&gpu, &mut cycle);
    assert!(error.contains("node 0 is reached twice"), "{error}");
}

#[test]
fn a_node_hierarchy_deeper_than_the_stack_holds_calls_is_placed() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // A valid file: each node the only child of the one before, the last one with the triangle,
    // a hundred thousand levels down, a few MiB of JSON.
    let depth = 100_000;
    let mut file = Glb::triangle();
    let mut nodes: Vec<String> = (1..depth)
        .map(|child| format!(r#"{{ "children": [{child}] }}"#))
        .collect();
    nodes.push(r#"{ "mesh": 0 }"#.to_owned());
    file.json["nodes"] = json(&format!("[{}]", nodes.join(", ")));
    let scene = SampleScene::new(&gpu, TextureFormat::Rgba8UnormSrgb, &file.bytes())
        .expect("the file loads");
    assert_eq!(scene.bounds().max, Vec3::new(1.0, 1.0, 0.0));
}

#[test]
fn a_mirrored_node_shows_the_same_faces_as_the_node_it_mirrors() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu.create_render_target("mirrored", 64, 64, TextureFormat::Rgba8UnormSrgb);
    // The triangle faces +Z, and so does its mirror image across the plane x = 0, which glTF
    // draws with its triangles wound the other way.
    for scale_x in [1.0, -1.0] {
        let mut file = Glb::triangle();
        file.json["nodes"][0]["scale"] = json(&format!("[{scale_x}, 1, 1]"));
        let mut scene =
            SampleScene::new(&gpu, target.format(), &file.bytes()).expect("the triangle loads");
        // Seen from +Z, at the center of the image: the center of the triangle.
        let center = Vec3::new(scale_x / 3.0, 1.0 / 3.0, 0.0);
        scene.render(&gpu, &target, orbit::camera_at(center, 1.0, 3.0, 0.0, 0.0));
        let image = gpu
            .read_render_target(&target)
            .expect("the image is read back");
        let pixel = |x: usize, y: usize| &image.pixels[(y * 64 + x) * 4..][..3];
        let (middle, corner) = (pixel(32, 32), pixel(0, 0));
        assert!(
            middle
                .iter()
                .zip(corner)
                .all(|(lit, background)| *lit > background + 50),
            "scale x {scale_x}: the triangle {middle:?} is not lit above the background {corner:?}"
        );
    }
}

/// A change that a test makes to a GLB file.
type Change = fn(&mut Glb);

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
