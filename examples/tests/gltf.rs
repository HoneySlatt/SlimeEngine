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
//! extensions a file requires; lights that light the file instead of the sample's sun, lights that
//! glTF forbids, and more lights than the renderer draws; masked, blended and double-sided
//! materials; a file's own tangents; a texture read both as colors and as data; and a node
//! hierarchy deeper than a thread's stack holds calls of a recursive walk.
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
use slime_render::glam::{Quat, Vec3};

use sample_scene::{EXPOSURE, SampleScene};

const SIZE: u32 = 256;

#[test]
fn the_gltf_scene_matches_its_reference_images() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target =
        gpu.create_render_target("reference glTF", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
    let mut scene = SampleScene::new(&gpu, target.format(), sample_scene::ASSET)
        .expect("the sample asset loads");
    // The budget of the plan is for release builds; this is a debug build's, among other tests.
    println!(
        "the sky's environment took {:?} on the CPU",
        scene.environment_time()
    );
    let bounds = *scene.bounds();
    // Seven primitives, one per object of the asset, and the triangles of their meshes.
    for (name, yaw, pitch) in [("gltf-front", 0.6, 0.45), ("gltf-back", 3.6, 0.25)] {
        let radius = bounds.radius();
        let camera = orbit::camera_at(bounds.center(), radius, radius * 2.2, yaw, pitch);
        let stats = scene.render(&gpu, &target, camera, EXPOSURE);
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
fn positions_that_name_no_accessor_are_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // The `gltf` crate's validation reads the accessor of a primitive's positions without checking
    // that it exists.
    let mut file = Glb::triangle();
    file.json["meshes"][0]["primitives"][0]["attributes"]["POSITION"] = 99.into();
    let error = refusal(&gpu, &mut file);
    assert!(
        error.contains("mesh 0 primitive 0 reads its positions from accessor 99"),
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
fn a_vertex_placed_at_a_point_that_is_not_finite_is_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // The second corner's x is NaN, which glTF forbids and its validation, which reads no binary
    // data, lets through: the box around the scene would cover only the corners after it.
    let mut file = Glb::triangle();
    file.bin[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
    let error = refusal(&gpu, &mut file);
    assert!(
        error.contains("node 0 places mesh 0 primitive 0 at [NaN, NaN, NaN]"),
        "{error}"
    );
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
        let (middle, behind) = centre_and_sky_behind(&gpu, &mut file, centre, 0.0);
        assert!(
            brighter(middle, behind),
            "scale x {scale_x}: the triangle {middle:?} is not lit above the sky behind it \
             {behind:?}"
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
    // A black surface that emits: with the strength of KHR_materials_emissive_strength, which a
    // file may require, 0.5 / EXPOSURE nits in red, which the sample shows at 0.46; without it,
    // 1 nit, which the sample shows as black.
    let emitting = |strength: &str, required: bool| {
        let mut file = Glb::triangle().with_material(&format!(
            r#"{{ "pbrMetallicRoughness": {{ "baseColorFactor": [0, 0, 0, 1], "metallicFactor": 0 }},
                  "emissiveFactor": [1, 0.5, 0.25] {strength} }}"#
        ));
        file.json["extensionsUsed"] = json(r#"["KHR_materials_emissive_strength"]"#);
        if required {
            file.json["extensionsRequired"] = json(r#"["KHR_materials_emissive_strength"]"#);
        }
        centre_and_sky_behind(&gpu, &mut file, TRIANGLE_CENTRE, 0.0).0
    };
    let strong = emitting(
        &format!(
            r#", "extensions": {{ "KHR_materials_emissive_strength": {{ "emissiveStrength": {} }} }}"#,
            0.5 / EXPOSURE
        ),
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
        centre_and_sky_behind(&gpu, &mut file, TRIANGLE_CENTRE, 0.0)
    };
    // glTF's default cutoff, 0.5, is above the alpha of 0.25.
    let (middle, behind) = shown(
        r#"{ "alphaMode": "MASK", "pbrMetallicRoughness": { "baseColorFactor": [1, 1, 1, 0.25] } }"#,
    );
    assert!(
        middle.iter().zip(behind).all(|(a, b)| a.abs_diff(b) <= 2),
        "masked: {middle:?} against {behind:?}"
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
        let (middle, behind) = shown(material);
        assert!(
            brighter(middle, behind),
            "{case}: {middle:?} against {behind:?}"
        );
    }
}

#[test]
fn a_double_sided_material_is_drawn_from_behind() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // Seen from −Z, behind the triangle, lit by a sun on that side, which travels along +Z.
    let shown = |material: &str| {
        let mut file = Glb::triangle().with_material(material).with_light(
            r#"{ "type": "directional", "intensity": 100000 }"#,
            Quat::from_rotation_y(std::f32::consts::PI),
        );
        centre_and_sky_behind(&gpu, &mut file, TRIANGLE_CENTRE, std::f32::consts::PI)
    };
    let (middle, behind) = shown("{}");
    assert!(
        middle.iter().zip(behind).all(|(a, b)| a.abs_diff(b) <= 2),
        "single-sided: {middle:?} against {behind:?}"
    );
    let (middle, behind) = shown(r#"{ "doubleSided": true }"#);
    assert!(
        brighter(middle, behind),
        "double-sided: {middle:?} against {behind:?}"
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
    let (towards_y, _) = centre_and_sky_behind(&gpu, &mut with_tangents, TRIANGLE_CENTRE, 0.0);
    let (towards_x, _) = centre_and_sky_behind(&gpu, &mut file(), TRIANGLE_CENTRE, 0.0);
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

#[test]
fn a_file_that_requires_khr_lights_punctual_is_lit_by_its_own_lights_only() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // A white matte triangle, and a directional light as strong as the sample's sun on a node
    // turned by `yaw` around the vertical axis: at 0 it travels down the node's −Z axis, onto the
    // front of the triangle, which faces +Z.
    const MATTE: &str = r#"{ "pbrMetallicRoughness": { "metallicFactor": 0 } }"#;
    let lit_by = |yaw: f32, intensity: f32| {
        let mut file = Glb::triangle().with_material(MATTE).with_light(
            &format!(r#"{{ "type": "directional", "intensity": {intensity} }}"#),
            Quat::from_rotation_y(yaw),
        );
        file.json["extensionsRequired"] = json(r#"["KHR_lights_punctual"]"#);
        centre_and_sky_behind(&gpu, &mut file, TRIANGLE_CENTRE, 0.0)
    };
    let (towards, background) = lit_by(0.0, 100_000.0);
    assert!(
        brighter(towards, background),
        "lit from the front: {towards:?} against {background:?}"
    );
    // Turned away, the light reaches the back of the triangle, and the sample adds no sun: the
    // front shows the sky's light alone, as under a light of no intensity (the light's glow in the
    // sky lies behind the triangle, too faint there to change a pixel).
    let (away, _) = lit_by(std::f32::consts::PI, 100_000.0);
    let (dark, _) = lit_by(0.0, 0.0);
    let mut without_lights = Glb::triangle().with_material(MATTE);
    let (sunlit, _) = centre_and_sky_behind(&gpu, &mut without_lights, TRIANGLE_CENTRE, 0.0);
    assert_eq!(away, dark, "lit from behind");
    assert!(
        brighter(sunlit, away),
        "the sample's sun lights a file without lights: {sunlit:?} against {away:?}"
    );
}

#[test]
fn a_scene_with_more_lights_than_the_renderer_draws_is_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    // One light, placed by `count` nodes: each node gives the scene a light.
    let with_lights = |count: usize| {
        let mut file = Glb::triangle();
        file.json["extensions"] = json(
            r#"{ "KHR_lights_punctual": { "lights": [{ "type": "point", "intensity": 1000 }] } }"#,
        );
        for _ in 0..count {
            let node = push(
                &mut file.json["nodes"],
                json(r#"{ "extensions": { "KHR_lights_punctual": { "light": 0 } } }"#),
            );
            push(&mut file.json["scenes"][0]["nodes"], node.into());
        }
        file
    };
    SampleScene::new(
        &gpu,
        TextureFormat::Rgba8UnormSrgb,
        &with_lights(slime_render::MAX_LIGHTS).bytes(),
    )
    .expect("the most lights the renderer draws load");
    let error = refusal(&gpu, &mut with_lights(slime_render::MAX_LIGHTS + 1));
    let expected = format!(
        "the scene places more than {} lights, the most the renderer draws",
        slime_render::MAX_LIGHTS
    );
    assert!(error.contains(&expected), "{error}");
}

#[test]
fn lights_that_gltf_does_not_allow_are_refused() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
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
            r#"{ "type": "directional", "color": [1e39, 1, 1] }"#,
            "has a color of [inf, 1.0, 1.0]",
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
        (
            r#"{ "type": "spot", "spot": { "innerConeAngle": -0.1 } }"#,
            "has cone angles of -0.1 and 0.7853982 radians",
        ),
        // What the `gltf` crate's validation refuses, before its readers would panic on it.
        (r#"{ "type": "spot" }"#, "spot: Missing data"),
        (r#"{ "type": "area" }"#, "type: Invalid value"),
    ] {
        let mut file = Glb::triangle().with_light(light, Quat::IDENTITY);
        let error = refusal(&gpu, &mut file);
        assert!(error.contains(expected), "{light}: {error}");
    }
    let mut file = Glb::triangle().with_light(r#"{ "type": "point" }"#, Quat::IDENTITY);
    file.json["nodes"][1]["extensions"]["KHR_lights_punctual"]["light"] = 3.into();
    let error = refusal(&gpu, &mut file);
    assert!(
        error.contains("nodes[1].extensions") && error.contains("light: Index out of bounds"),
        "{error}"
    );
    // A node that flattens its −Z axis gives its light no direction.
    let mut file = Glb::triangle().with_light(r#"{ "type": "directional" }"#, Quat::IDENTITY);
    file.json["nodes"][1]["scale"] = json("[1, 1, 0]");
    let error = refusal(&gpu, &mut file);
    assert!(error.contains("has no direction"), "{error}");
    // 10³⁹ is beyond the largest 32-bit float: a node there places its light at no finite point.
    let mut file = Glb::triangle().with_light(r#"{ "type": "point" }"#, Quat::IDENTITY);
    file.json["nodes"][1]["translation"] = json("[1e39, 0, 0]");
    let error = refusal(&gpu, &mut file);
    assert!(
        error.contains("is placed at") && error.contains("the node's transform is not finite"),
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

    /// Adds `light`, a light of `KHR_lights_punctual` in JSON, the only one of the file, on a new
    /// node of the scene turned by `rotation` and 1 unit in front of the triangle.
    fn with_light(mut self, light: &str, rotation: Quat) -> Self {
        self.json["extensionsUsed"] = json(r#"["KHR_lights_punctual"]"#);
        self.json["extensions"] = json(&format!(
            r#"{{ "KHR_lights_punctual": {{ "lights": [{light}] }} }}"#
        ));
        let [x, y, z, w] = rotation.to_array();
        let node = push(
            &mut self.json["nodes"],
            json(&format!(
                r#"{{ "translation": [0, 0, 1], "rotation": [{x}, {y}, {z}, {w}],
                      "extensions": {{ "KHR_lights_punctual": {{ "light": 0 }} }} }}"#
            )),
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

/// Draws `file` into a 64 × 64 sRGB target as the sample does: under the file's lights, or the
/// sample's sun for a file without lights, and the sample's sky, at the sample's exposure, seen
/// from 3 units away from `centre`, turned by `yaw` around the vertical axis (0 looks from +Z).
/// Returns the color of the pixel at the center of the image, and the color it has once every
/// node of `file` is moved 1 km aside: the sky behind it.
fn centre_and_sky_behind(gpu: &Gpu, file: &mut Glb, centre: Vec3, yaw: f32) -> ([u8; 3], [u8; 3]) {
    let shown = |bytes: &[u8]| {
        let target = gpu.create_render_target("triangle", 64, 64, TextureFormat::Rgba8UnormSrgb);
        let mut scene = SampleScene::new(gpu, target.format(), bytes).expect("the triangle loads");
        scene.render(
            gpu,
            &target,
            orbit::camera_at(centre, 1.0, 3.0, yaw, 0.0),
            EXPOSURE,
        );
        let image = gpu
            .read_render_target(&target)
            .expect("the image is read back");
        let start = (32 * 64 + 32) * 4;
        [0, 1, 2].map(|channel| image.pixels[start + channel])
    };
    let drawn = shown(&file.bytes());
    let mut aside = Glb {
        json: file.json.clone(),
        bin: file.bin.clone(),
    };
    let roots = aside.json["scenes"][0]["nodes"].clone();
    for root in roots.as_array().expect("a list of nodes") {
        aside.json["nodes"][index(root)]["translation"] = json("[1000, 0, 0]");
    }
    let background = shown(&aside.bytes());
    (drawn, background)
}

/// Whether every channel of `drawn` is clearly above that of `background`: by more than 10, where a
/// white surface that the sample's sun lights at 62° from its normal is 15 to 25 above the sky's
/// horizon behind it, and drivers differ by a few at most.
fn brighter(drawn: [u8; 3], background: [u8; 3]) -> bool {
    drawn
        .iter()
        .zip(background)
        .all(|(drawn, background)| *drawn > background + 10)
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
