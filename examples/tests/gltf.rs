//! The reference image test of the glTF scene: the asset of `examples/assets/scene.glb`, cooked by
//! `slime-cook` and loaded by the renderer, rendered offscreen from two fixed camera poses, must
//! match `tests/reference/gltf-front.png` and `tests/reference/gltf-back.png` within the tolerance
//! of the driver that renders them (ADR-0018, amended by ADR-0028 and ADR-0030; the numbers are in
//! `common/reference_image.rs`).
//!
//! The references were rendered with lavapipe, like the others. To render them again, after a
//! deliberate change of the asset, of the cooker or of the renderer:
//!
//! ```sh
//! SLIME_UPDATE_REFERENCE_IMAGES=1 VK_DRIVER_FILES="$SLIME_LAVAPIPE_ICD" \
//!     cargo test -p slime-examples --test gltf
//! ```
//!
//! One test reads the cooked asset itself: the textures of its boxes stand upright on every side.
//! The other tests cook small glTF files built in memory, which hold what the asset does not, and
//! draw them: a mirrored node; an emission of `KHR_materials_emissive_strength`; masked, blended
//! and double-sided materials; a file's own tangents; a texture read both as colors and as data;
//! lights that light the file instead of the sample's sun. One moves a cooked `.gltf` file and
//! renames it, which changes nothing it draws. One spawns the cooked asset and a mirrored node in a
//! world (`slime-world`), which places each node where its file does. What the importer refuses is
//! tested with it, in `crates/slime-cook/tests/gltf.rs`. The unit tests of `common/orbit.rs` run here
//! too.

#[path = "../common/cooked.rs"]
mod cooked;
#[path = "../common/orbit.rs"]
mod orbit;
#[path = "../common/reference_image.rs"]
mod reference_image;
#[path = "../common/sample_scene.rs"]
#[expect(
    dead_code,
    reason = "the `scene` example loads the scene and the shaders again when they change; this test \
              does not"
)]
mod sample_scene;

use std::borrow::Cow;
use std::fs;
use std::path::Path;

use cooked::{directory, sample};
use gltf::json::Value;
use slime_assets::{AssetId, CookedAssets, CookedScene};
use slime_gfx::{Gpu, Image, TextureFormat};
use slime_render::glam::{DMat4, Mat4, Quat, Vec3};

use sample_scene::{BLOOM, EXPOSURE, SCENE, SampleScene};
use slime_render::Renderer;
use slime_world::{ResolvedScene, Transform, World};

const SIZE: u32 = 256;

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// The scene `scene` of `assets`, loaded and drawn by a renderer for targets of `format`.
fn load(gpu: &Gpu, format: TextureFormat, assets: &CookedAssets, scene: AssetId) -> SampleScene {
    let renderer = Renderer::new(gpu, format).expect("the renderer");
    let loaded = renderer
        .load_scene(gpu, assets, scene)
        .expect("the scene loads");
    SampleScene::new(gpu, renderer, loaded).expect("the scene places a mesh, under a sky")
}

#[test]
fn the_gltf_scene_matches_its_reference_images() {
    let gpu = gpu();
    let target = gpu
        .create_render_target("reference glTF", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let mut scene = load(&gpu, target.format(), sample(), SCENE);
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
        let stats = scene
            .render(&gpu, &target, camera, EXPOSURE, BLOOM)
            .expect("the frame is drawn");
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
fn the_textures_of_the_asset_s_boxes_stand_upright_on_their_sides() {
    // Each side of a box shows its texture as an image is seen: u growing to the right, as seen
    // from outside the box, and v, which glTF counts down the image, growing downward. The wall's
    // bricks then lie along the wall, three times across its 3 m.
    let assets = sample();
    let scene = assets.scene(SCENE).expect("the sample's scene");
    let boxes = scene
        .nodes
        .iter()
        .filter(|node| matches!(node.name.as_str(), "wall" | "crate" | "small crate"));
    let mut sides = 0;
    for node in boxes {
        for &id in &node.meshes {
            let mesh = assets.mesh(id).expect("a box's mesh");
            let positions: Vec<Vec3> = mesh.positions.iter().copied().map(Vec3::from).collect();
            let normals: Vec<Vec3> = mesh.normals.iter().copied().map(Vec3::from).collect();
            // The corners of a side share its normal; in the mesh's own space, the sides face
            // along x and z, y up.
            for normal in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
                let corners: Vec<usize> = (0..positions.len())
                    .filter(|&corner| normals[corner].abs_diff_eq(normal, 1e-3))
                    .collect();
                assert!(
                    !corners.is_empty(),
                    "{} has a side facing {normal}",
                    node.name
                );
                sides += 1;
                let right = Vec3::Y.cross(normal);
                for &a in &corners {
                    for &b in &corners {
                        let across = (positions[a] - positions[b]).dot(right);
                        let up = positions[a].y - positions[b].y;
                        let du = mesh.uvs[a][0] - mesh.uvs[b][0];
                        let dv = mesh.uvs[a][1] - mesh.uvs[b][1];
                        assert!(
                            across.abs() < 1e-4 || du * across > 0.0,
                            "{}, side facing {normal}: u does not grow to the right",
                            node.name
                        );
                        assert!(
                            up.abs() < 1e-4 || dv * up < 0.0,
                            "{}, side facing {normal}: v does not grow downward",
                            node.name
                        );
                    }
                }
            }
        }
    }
    assert_eq!(sides, 12, "four sides for each of the three boxes");
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

    /// Adds an accessor to `count` elements of `kind` (`"VEC3"`, ...) of 32-bit floats, in buffer
    /// view `view`, and returns its index.
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

    /// Cooks the file, as `test.glb` alone in a directory of sources of its own named `name`, and
    /// returns the cooked data and the file's ID, its default scene's.
    fn cooked(&mut self, name: &str) -> (CookedAssets, AssetId) {
        let root = directory(name);
        let sources = root.join("sources");
        fs::create_dir_all(&sources).expect("created");
        fs::write(sources.join("test.glb"), self.bytes()).expect("written");
        slime_cook::cook(&sources, &root.join("cooked")).expect("the file cooks");
        let id = slime_cook::scan(&sources, slime_cook::Mode::Check)
            .expect("scanned")
            .sources[0]
            .id;
        (
            CookedAssets::open(root.join("cooked")).expect("the manifest is read"),
            id,
        )
    }
}

/// The component type of glTF's 32-bit floating-point numbers.
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

/// The color of the pixel at the center of a 64 × 64 sRGB image of the scene `scene` of `assets`,
/// drawn as the sample does: under the scene's lights, or the sample's sun for a scene without
/// lights, and the sample's sky, at the sample's exposure, without bloom, seen from 3 units away
/// from `centre`, turned by `yaw` around the vertical axis (0 looks from +Z).
fn centre_of(gpu: &Gpu, assets: &CookedAssets, scene: AssetId, centre: Vec3, yaw: f32) -> [u8; 3] {
    let image = image_of(gpu, assets, scene, centre, yaw);
    let start = (32 * 64 + 32) * 4;
    [0, 1, 2].map(|channel| image.pixels[start + channel])
}

/// The 64 × 64 sRGB image of [`centre_of`].
fn image_of(gpu: &Gpu, assets: &CookedAssets, scene: AssetId, centre: Vec3, yaw: f32) -> Image {
    let target = gpu
        .create_render_target("triangle", 64, 64, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    let mut sample = load(gpu, target.format(), assets, scene);
    sample
        .render(
            gpu,
            &target,
            orbit::camera_at(centre.as_dvec3(), 1.0, 3.0, yaw, 0.0),
            EXPOSURE,
            // Without bloom, which would spread the disc of a sun behind the triangle onto it.
            0.0,
        )
        .expect("the frame is drawn");
    gpu.read_render_target(&target)
        .expect("the image is read back")
}

/// The color of the pixel at the center of the image of `file`, cooked as `name`, and the color it
/// has once every node of `file` is moved 1 km aside: the sky behind it (see [`centre_of`]).
fn centre_and_sky_behind(
    gpu: &Gpu,
    file: &mut Glb,
    name: &str,
    centre: Vec3,
    yaw: f32,
) -> ([u8; 3], [u8; 3]) {
    let (assets, scene) = file.cooked(name);
    let drawn = centre_of(gpu, &assets, scene, centre, yaw);
    let mut aside = Glb {
        json: file.json.clone(),
        bin: file.bin.clone(),
    };
    let roots = aside.json["scenes"][0]["nodes"].clone();
    for root in roots.as_array().expect("a list of nodes") {
        aside.json["nodes"][index(root)]["translation"] = json("[1000, 0, 0]");
    }
    let (assets, scene) = aside.cooked(&format!("{name} aside"));
    let background = centre_of(gpu, &assets, scene, centre, yaw);
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

#[test]
fn a_mirrored_node_shows_the_same_faces_as_the_node_it_mirrors() {
    let gpu = gpu();
    // The triangle faces +Z, and so does its mirror image across the plane x = 0, whose triangles
    // wind the other way on screen: the renderer keeps their front faces, as glTF does.
    for scale_x in [1.0, -1.0] {
        let mut file = Glb::triangle();
        file.json["nodes"][0]["scale"] = json(&format!("[{scale_x}, 1, 1]"));
        let centre = TRIANGLE_CENTRE * Vec3::new(scale_x, 1.0, 1.0);
        let name = format!("mirrored {scale_x}");
        let (middle, behind) = centre_and_sky_behind(&gpu, &mut file, &name, centre, 0.0);
        assert!(
            brighter(middle, behind),
            "scale x {scale_x}: the triangle {middle:?} is not lit above the sky behind it \
             {behind:?}"
        );
    }
}

#[test]
fn an_emission_has_the_strength_of_khr_materials_emissive_strength() {
    let gpu = gpu();
    // A black surface that emits: with the strength of KHR_materials_emissive_strength, which a
    // file may require, 0.5 / EXPOSURE nits in red, which the sample shows at 0.46; without it,
    // 1 nit, which the sample shows as black.
    let emitting = |strength: &str, required: bool, name: &str| {
        let mut file = Glb::triangle().with_material(&format!(
            r#"{{ "pbrMetallicRoughness": {{ "baseColorFactor": [0, 0, 0, 1], "metallicFactor": 0 }},
                  "emissiveFactor": [1, 0.5, 0.25] {strength} }}"#
        ));
        file.json["extensionsUsed"] = json(r#"["KHR_materials_emissive_strength"]"#);
        if required {
            file.json["extensionsRequired"] = json(r#"["KHR_materials_emissive_strength"]"#);
        }
        let (assets, scene) = file.cooked(name);
        centre_of(&gpu, &assets, scene, TRIANGLE_CENTRE, 0.0)
    };
    let strong = emitting(
        &format!(
            r#", "extensions": {{ "KHR_materials_emissive_strength": {{ "emissiveStrength": {} }} }}"#,
            0.5 / EXPOSURE
        ),
        true,
        "strong",
    );
    let plain = emitting("", false, "plain");
    assert!(strong[0] > plain[0] + 50, "{strong:?} against {plain:?}");
}

#[test]
fn a_masked_material_draws_nothing_below_its_cutoff_and_a_blended_one_is_drawn_opaque() {
    let gpu = gpu();
    let shown = |material: &str, name: &str| {
        let mut file = Glb::triangle().with_material(material);
        centre_and_sky_behind(&gpu, &mut file, name, TRIANGLE_CENTRE, 0.0)
    };
    // glTF's default cutoff, 0.5, is above the alpha of 0.25.
    let (middle, behind) = shown(
        r#"{ "alphaMode": "MASK", "pbrMetallicRoughness": { "baseColorFactor": [1, 1, 1, 0.25] } }"#,
        "masked",
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
        let (middle, behind) = shown(material, case);
        assert!(
            brighter(middle, behind),
            "{case}: {middle:?} against {behind:?}"
        );
    }
}

#[test]
fn a_double_sided_material_is_drawn_from_behind() {
    let gpu = gpu();
    // Seen from −Z, behind the triangle, lit by a sun on that side, which travels along +Z.
    let shown = |material: &str, name: &str| {
        let mut file = Glb::triangle().with_material(material).with_light(
            r#"{ "type": "directional", "intensity": 100000 }"#,
            Quat::from_rotation_y(std::f32::consts::PI),
        );
        centre_and_sky_behind(&gpu, &mut file, name, TRIANGLE_CENTRE, std::f32::consts::PI)
    };
    let (middle, behind) = shown("{}", "single-sided");
    assert!(
        middle.iter().zip(behind).all(|(a, b)| a.abs_diff(b) <= 2),
        "single-sided: {middle:?} against {behind:?}"
    );
    let (middle, behind) = shown(r#"{ "doubleSided": true }"#, "double-sided");
    assert!(
        brighter(middle, behind),
        "double-sided: {middle:?} against {behind:?}"
    );
}

#[test]
fn a_file_s_tangents_are_used_rather_than_computed() {
    let gpu = gpu();
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
    let (assets, scene) = with_tangents.cooked("tangents of the file");
    let towards_y = centre_of(&gpu, &assets, scene, TRIANGLE_CENTRE, 0.0);
    let (assets, scene) = file().cooked("tangents computed");
    let towards_x = centre_of(&gpu, &assets, scene, TRIANGLE_CENTRE, 0.0);
    assert!(
        towards_y[0] > towards_x[0] + 10,
        "{towards_y:?} with the file's tangents, {towards_x:?} with computed ones"
    );
}

#[test]
fn a_texture_read_as_a_color_and_as_data_is_cooked_in_both_roles() {
    let gpu = gpu();
    // The renderer panics on an sRGB texture in the occlusion slot.
    let mut file = Glb::triangle().with_texture(&png(4, 4));
    file.json["materials"][0]["occlusionTexture"] = json(r#"{ "index": 0 }"#);
    let (assets, scene) = file.cooked("both roles");
    centre_of(&gpu, &assets, scene, TRIANGLE_CENTRE, 0.0);
}

#[test]
fn a_file_that_requires_khr_lights_punctual_is_lit_by_its_own_lights_only() {
    let gpu = gpu();
    // A white matte triangle, and a directional light as strong as the sample's sun on a node
    // turned by `yaw` around the vertical axis: at 0 it travels down the node's −Z axis, onto the
    // front of the triangle, which faces +Z.
    const MATTE: &str = r#"{ "pbrMetallicRoughness": { "metallicFactor": 0 } }"#;
    let lit_by = |yaw: f32, intensity: f32, name: &str| {
        let mut file = Glb::triangle().with_material(MATTE).with_light(
            &format!(r#"{{ "type": "directional", "intensity": {intensity} }}"#),
            Quat::from_rotation_y(yaw),
        );
        file.json["extensionsRequired"] = json(r#"["KHR_lights_punctual"]"#);
        centre_and_sky_behind(&gpu, &mut file, name, TRIANGLE_CENTRE, 0.0)
    };
    let (towards, background) = lit_by(0.0, 100_000.0, "towards");
    assert!(
        brighter(towards, background),
        "lit from the front: {towards:?} against {background:?}"
    );
    // Turned away, the light reaches the back of the triangle, and the sample adds no sun: the
    // front shows the sky's light alone, as under a light of no intensity (the light's glow in the
    // sky lies behind the triangle, too faint there to change a pixel).
    let (away, _) = lit_by(std::f32::consts::PI, 100_000.0, "away");
    let (dark, _) = lit_by(0.0, 0.0, "dark");
    let mut without_lights = Glb::triangle().with_material(MATTE);
    let (sunlit, _) =
        centre_and_sky_behind(&gpu, &mut without_lights, "sunlit", TRIANGLE_CENTRE, 0.0);
    assert_eq!(away, dark, "lit from behind");
    assert!(
        brighter(sunlit, away),
        "the sample's sun lights a file without lights: {sunlit:?} against {away:?}"
    );
}

#[test]
fn a_gltf_file_moved_and_renamed_with_its_metadata_draws_the_same_and_cooks_nothing_again() {
    // The exit criterion of Stage 4a: a `.gltf` file, its buffer and the image it reads beside it,
    // as Blender exports them, cooked; then the directory moved and the file renamed, with its
    // metadata: the same IDs, nothing cooked again, the same pixels.
    let gpu = gpu();
    let root = directory("moved");
    let sources = root.join("sources");
    let mut file = Glb::triangle()
        .with_attribute("TEXCOORD_0", "VEC2", &[0.0, 1.0, 1.0, 1.0, 0.0, 0.0])
        .with_material(
            r#"{ "pbrMetallicRoughness": { "baseColorTexture": { "index": 0 }, "metallicFactor": 0 } }"#,
        );
    file.json["images"] = json(r#"[{ "uri": "textures/red.png" }]"#);
    file.json["textures"] = json(r#"[{ "source": 0 }]"#);
    file.json["buffers"][0] = json(&format!(
        r#"{{ "byteLength": {}, "uri": "triangle.bin" }}"#,
        file.bin.len()
    ));
    fs::create_dir_all(sources.join("textures")).expect("created");
    fs::write(sources.join("triangle.bin"), &file.bin).expect("written");
    fs::write(
        sources.join("triangle.gltf"),
        gltf::json::serialize::to_vec(&file.json).expect("the JSON is written"),
    )
    .expect("written");
    let red = [255, 0, 0, 255].repeat(16);
    fs::write(sources.join("textures/red.png"), png_of(4, 4, &red)).expect("written");

    let cooked = root.join("cooked");
    let first = slime_cook::cook(&sources, &cooked).expect("the first cook succeeds");
    assert_eq!(
        first.cooked.len(),
        4,
        "a mesh, its material, the scene and the texture"
    );
    let id = |sources: &Path, name: &str| {
        slime_cook::scan(sources, slime_cook::Mode::Check)
            .expect("scanned")
            .sources
            .into_iter()
            .find(|source| source.path.ends_with(name))
            .expect("the source is there")
            .id
    };
    let scene = id(&sources, "triangle.gltf");
    let assets = CookedAssets::open(&cooked).expect("the manifest is read");
    let before = image_of(&gpu, &assets, scene, TRIANGLE_CENTRE, 0.0);
    let centre = &before.pixels[(32 * 64 + 32) * 4..][..3];
    assert!(
        centre[0] > centre[1] + 50,
        "the red texture is drawn: {centre:?}"
    );

    let moved = root.join("moved").join("elsewhere");
    fs::create_dir_all(moved.parent().expect("a parent")).expect("created");
    fs::rename(&sources, &moved).expect("moved");
    fs::rename(moved.join("triangle.gltf"), moved.join("ruin.gltf")).expect("renamed");
    fs::rename(
        moved.join("triangle.gltf.meta"),
        moved.join("ruin.gltf.meta"),
    )
    .expect("renamed");
    let again = slime_cook::cook(&moved, &cooked).expect("the second cook succeeds");
    assert!(
        again.cooked.is_empty(),
        "nothing is cooked again: {again:?}"
    );
    assert_eq!(id(&moved, "ruin.gltf"), scene, "the file keeps its ID");
    let assets = CookedAssets::open(&cooked).expect("the manifest is read");
    let after = image_of(&gpu, &assets, scene, TRIANGLE_CENTRE, 0.0);
    assert_eq!(after.pixels, before.pixels, "the same pixels");
}

/// The world places each node of `scene` where its file does: each entity's transform in the world,
/// after its own stretch, against the node's matrix in the scene, within `f32`'s rounding.
fn assert_world_places(scene: &CookedScene) {
    let mut world = World::new();
    let root = world
        .spawn_scene(
            "scene",
            &ResolvedScene::from_cooked(AssetId::from_u128(1), scene, world.registry())
                .expect("resolved"),
        )
        .expect("spawned");
    world.update_transforms();
    // The scene's nodes are in the order of a walk of the hierarchy, which the entities keep.
    let mut walk = Vec::new();
    let mut next: Vec<_> = world.children(root).rev().collect();
    while let Some(entity) = next.pop() {
        walk.push(entity);
        next.extend(world.children(entity).rev());
    }
    assert_eq!(walk.len(), scene.nodes.len());
    for ((entity, node), expected) in walk
        .into_iter()
        .zip(&scene.nodes)
        .zip(scene.world_transforms())
    {
        let stretch = world.get::<Transform>(entity).expect("placed").stretch;
        let placed = world
            .global_transform(entity)
            .expect("placed")
            .to_affine(stretch);
        let expected = Mat4::from_cols_array(&expected);
        let cells = DMat4::from(placed).to_cols_array();
        for (a, b) in cells.iter().zip(expected.to_cols_array()) {
            assert!(
                (a - f64::from(b)).abs() <= 1e-5 * f64::from(b.abs()).max(1.0),
                "node `{}`: {placed:?} against {expected:?}",
                node.name
            );
        }
    }
}

#[test]
fn a_world_places_the_sample_s_nodes_and_a_mirrored_one_where_their_files_do() {
    let scene = sample().scene(SCENE).expect("the sample's scene is cooked");
    assert_world_places(&scene);
    let mut file = Glb::triangle();
    file.json["nodes"][0]["scale"] = json("[-1, 1, 1]");
    let (assets, id) = file.cooked("mirrored in a world");
    assert_world_places(&assets.scene(id).expect("the scene is cooked"));
}
