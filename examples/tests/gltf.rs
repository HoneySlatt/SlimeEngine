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
//! does not: what the loader must refuse with an error rather than panic on; a mirrored node; the
//! extensions a file requires; masked, blended and double-sided materials; a file's own tangents;
//! a texture read both as colors and as data; and a node hierarchy deeper than a thread's stack
//! holds calls of a recursive walk.
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
        let stats = scene.render(&gpu, &target, camera, 1.0);
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
fn a_glb_header_that_declares_a_length_shorter_than_itself_is_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // A header alone: the magic, version 2, and a length of 0, from which the `gltf` crate
    // subtracts the header's 12 bytes, an overflow that panics in debug builds.
    let header = [b"glTF".as_slice(), &2u32.to_le_bytes(), &0u32.to_le_bytes()].concat();
    let error = refusal_of(&gpu, &header);
    assert!(
        error.contains("declares a length of 0 bytes, shorter than the header"),
        "{error}"
    );
}

#[test]
fn data_outside_the_binary_chunk_is_refused_without_quoting_data_uris() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // What a data URI holds, as text: a whole image or buffer in a real file, which the error
    // must leave out.
    const PAYLOAD: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAAC";
    let data_uri = format!("data:application/octet-stream;base64,{PAYLOAD}");
    for (uri, expected) in [
        (data_uri.as_str(), "is given as a data URI"),
        ("other.bin", "is an external file, `other.bin`"),
    ] {
        let mut image = Glb::triangle().with_texture(&png(2, 2));
        remove(&mut image.json["images"][0], "bufferView");
        image.json["images"][0]["uri"] = uri.into();
        let error = refusal(&gpu, &mut image);
        assert!(
            error.contains(&format!("image 0 {expected}")) && !error.contains(PAYLOAD),
            "{error}"
        );

        let mut buffer = Glb::triangle();
        push(
            &mut buffer.json["buffers"],
            json(&format!(r#"{{ "byteLength": 24, "uri": "{uri}" }}"#)),
        );
        let error = refusal(&gpu, &mut buffer);
        assert!(
            error.contains(&format!("buffer 1 {expected}")) && !error.contains(PAYLOAD),
            "{error}"
        );
    }
}

#[test]
fn accessors_the_loader_cannot_read_are_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
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
            // `gltf` crate's validation refuses it before the loader's checks.
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
fn an_index_that_names_no_vertex_is_refused_in_a_primitive_with_normals() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // The tangents, computed from the triangles, skip the one that names no vertex: the renderer
    // refuses it.
    let mut file = Glb::triangle();
    let view = file.add_view(
        &[0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]
            .map(f32::to_le_bytes)
            .concat(),
    );
    let normals = file.add_accessor(view, FLOAT, 3, "VEC3");
    file.json["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = normals.into();
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
    // normals take three vertices of 48 bytes per triangle, one triangle more than the largest
    // vertex buffer holds.
    let triangles = gpu.max_buffer_size() / (3 * 48) + 1;
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
    // The triangle faces +Z, and so does its mirror image across the plane x = 0, whose triangles
    // wind the other way on screen: the renderer keeps their front faces, as glTF does.
    for scale_x in [1.0, -1.0] {
        let mut file = Glb::triangle();
        file.json["nodes"][0]["scale"] = json(&format!("[{scale_x}, 1, 1]"));
        let centre = TRIANGLE_CENTRE * Vec3::new(scale_x, 1.0, 1.0);
        let (middle, corner) = centre_and_corner(&gpu, &mut file, centre, 0.0, 1.0);
        assert!(
            brighter(middle, corner),
            "scale x {scale_x}: the triangle {middle:?} is not lit above the background {corner:?}"
        );
    }
}

#[test]
fn extensions_are_refused_when_required_unless_the_loader_reads_them() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut file = Glb::triangle();
    file.json["extensionsUsed"] = json(r#"["KHR_texture_transform"]"#);
    file.json["extensionsRequired"] = json(r#"["KHR_texture_transform"]"#);
    let error = refusal(&gpu, &mut file);
    assert!(
        error.contains(r#"extensionsRequired[0] = "KHR_texture_transform": Unsupported extension"#),
        "{error}"
    );
    // A black surface that emits, whose emission an exposure of 1/8 keeps below white: four times
    // brighter with the strength of KHR_materials_emissive_strength, which a file may require.
    let emitting = |strength: &str, required: bool| {
        let mut file = Glb::triangle().with_material(&format!(
            r#"{{ "pbrMetallicRoughness": {{ "baseColorFactor": [0, 0, 0, 1], "metallicFactor": 0 }},
                  "emissiveFactor": [1, 0.5, 0.25] {strength} }}"#
        ));
        file.json["extensionsUsed"] = json(r#"["KHR_materials_emissive_strength"]"#);
        if required {
            file.json["extensionsRequired"] = json(r#"["KHR_materials_emissive_strength"]"#);
        }
        centre_and_corner(&gpu, &mut file, TRIANGLE_CENTRE, 0.0, 1.0 / 8.0).0
    };
    let strong = emitting(
        r#", "extensions": { "KHR_materials_emissive_strength": { "emissiveStrength": 4 } }"#,
        true,
    );
    let plain = emitting("", false);
    assert!(strong[0] > plain[0] + 50, "{strong:?} against {plain:?}");
}

#[test]
fn a_masked_material_draws_nothing_below_its_cutoff_and_a_blended_one_is_drawn_opaque() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let shown = |material: &str| {
        let mut file = Glb::triangle().with_material(material);
        centre_and_corner(&gpu, &mut file, TRIANGLE_CENTRE, 0.0, 1.0)
    };
    // glTF's default cutoff, 0.5, is above the alpha of 0.25.
    let (middle, corner) = shown(
        r#"{ "alphaMode": "MASK", "pbrMetallicRoughness": { "baseColorFactor": [1, 1, 1, 0.25] } }"#,
    );
    assert!(
        middle.iter().zip(corner).all(|(a, b)| a.abs_diff(b) <= 2),
        "masked: {middle:?} against {corner:?}"
    );
    for (material, case) in [
        (
            r#"{ "alphaMode": "MASK", "alphaCutoff": 0.2,
                 "pbrMetallicRoughness": { "baseColorFactor": [1, 1, 1, 0.25] } }"#,
            "masked, above a cutoff of 0.2",
        ),
        (
            r#"{ "alphaMode": "BLEND", "pbrMetallicRoughness": { "baseColorFactor": [1, 1, 1, 0.25] } }"#,
            "blended",
        ),
    ] {
        let (middle, corner) = shown(material);
        assert!(
            brighter(middle, corner),
            "{case}: {middle:?} against {corner:?}"
        );
    }
}

#[test]
fn a_double_sided_material_is_drawn_from_behind() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // Seen from −Z, behind the triangle: the light comes from the other side, and only the ambient
    // light reaches the back face.
    let shown = |material: &str| {
        let mut file = Glb::triangle().with_material(material);
        centre_and_corner(&gpu, &mut file, TRIANGLE_CENTRE, std::f32::consts::PI, 1.0)
    };
    let (middle, corner) = shown("{}");
    assert!(
        middle.iter().zip(corner).all(|(a, b)| a.abs_diff(b) <= 2),
        "single-sided: {middle:?} against {corner:?}"
    );
    let (middle, corner) = shown(r#"{ "doubleSided": true }"#);
    assert!(
        brighter(middle, corner),
        "double-sided: {middle:?} against {corner:?}"
    );
}

#[test]
fn a_file_s_tangents_are_used_rather_than_computed() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // A normal texture that tilts the normal by 45° towards the tangent. The texture coordinates
    // grow along +X, where the computed tangents point; the file's point along +Y, closer to the
    // light, which comes from above.
    let file = || {
        Glb::triangle()
            .with_attribute(
                "NORMAL",
                "VEC3",
                &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            )
            .with_attribute("TEXCOORD_0", "VEC2", &[0.0, 1.0, 1.0, 1.0, 0.0, 0.0])
            .with_image(&png_of(1, 1, &[218, 128, 218, 255]))
            .with_material(
                r#"{ "normalTexture": { "index": 0 },
                     "pbrMetallicRoughness": { "metallicFactor": 0 } }"#,
            )
    };
    let mut with_tangents = file().with_attribute(
        "TANGENT",
        "VEC4",
        &[0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
    );
    let (towards_y, _) = centre_and_corner(&gpu, &mut with_tangents, TRIANGLE_CENTRE, 0.0, 1.0);
    let (towards_x, _) = centre_and_corner(&gpu, &mut file(), TRIANGLE_CENTRE, 0.0, 1.0);
    assert!(
        towards_y[0] > towards_x[0] + 10,
        "{towards_y:?} with the file's tangents, {towards_x:?} with computed ones"
    );
}

#[test]
fn a_texture_read_as_a_color_and_as_data_is_created_in_both_formats() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // The renderer panics on an sRGB texture in the occlusion slot.
    let mut file = Glb::triangle().with_texture(&png(2, 2));
    file.json["materials"][0]["occlusionTexture"] = json(r#"{ "index": 0 }"#);
    SampleScene::new(&gpu, TextureFormat::Rgba8UnormSrgb, &file.bytes())
        .expect("the triangle loads");
}

#[test]
fn a_texture_read_through_another_coordinate_set_is_refused_in_any_slot() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut file = Glb::triangle()
        .with_image(&png(2, 2))
        .with_material(r#"{ "normalTexture": { "index": 0, "texCoord": 1 } }"#);
    let error = refusal(&gpu, &mut file);
    assert!(
        error.contains("reads its normal texture through coordinate set 1"),
        "{error}"
    );
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
    fn with_texture(self, png: &[u8]) -> Self {
        self.with_image(png)
            .with_material(r#"{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 } } }"#)
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
    png_of(
        width,
        height,
        &vec![255; width as usize * height as usize * 4],
    )
}

/// A PNG image of `width` × `height` pixels, `pixels` in RGBA, rows from the top.
fn png_of(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("the PNG header is written");
    writer
        .write_image_data(pixels)
        .expect("the PNG image is written");
    writer.finish().expect("the PNG image is finished");
    bytes
}

/// The center of the triangle of [`Glb::triangle`].
const TRIANGLE_CENTRE: Vec3 = Vec3::new(1.0 / 3.0, 1.0 / 3.0, 0.0);

/// Draws `file` into a 64 × 64 sRGB target at `exposure`, under the samples' light, seen from 3
/// units away from `centre`, turned by `yaw` around the vertical axis (0 looks from +Z), and
/// returns the color of the pixel at the center of the image, and that of its corner, the
/// background.
fn centre_and_corner(
    gpu: &Gpu,
    file: &mut Glb,
    centre: Vec3,
    yaw: f32,
    exposure: f32,
) -> ([u8; 3], [u8; 3]) {
    let target = gpu.create_render_target("triangle", 64, 64, TextureFormat::Rgba8UnormSrgb);
    let mut scene =
        SampleScene::new(gpu, target.format(), &file.bytes()).expect("the triangle loads");
    scene.render(
        gpu,
        &target,
        orbit::camera_at(centre, 1.0, 3.0, yaw, 0.0),
        exposure,
    );
    let image = gpu
        .read_render_target(&target)
        .expect("the image is read back");
    let pixel = |x: usize, y: usize| {
        let start = (y * 64 + x) * 4;
        [0, 1, 2].map(|channel| image.pixels[start + channel])
    };
    (pixel(32, 32), pixel(0, 0))
}

/// Whether every channel of `drawn` is well above that of `background`.
fn brighter(drawn: [u8; 3], background: [u8; 3]) -> bool {
    drawn
        .iter()
        .zip(background)
        .all(|(drawn, background)| *drawn > background + 50)
}

/// The message of the error with which the loader refuses `file`.
fn refusal(gpu: &Gpu, file: &mut Glb) -> String {
    refusal_of(gpu, &file.bytes())
}

/// The message of the error with which the loader refuses the file `bytes`.
fn refusal_of(gpu: &Gpu, bytes: &[u8]) -> String {
    match SampleScene::new(gpu, TextureFormat::Rgba8UnormSrgb, bytes) {
        Ok(_) => panic!("the loader accepted a file it must refuse"),
        Err(error) => error.to_string(),
    }
}
