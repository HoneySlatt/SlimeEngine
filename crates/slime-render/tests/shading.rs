//! Shading with glTF's metallic-roughness model (ADR-0035), checked by reading back an
//! `Rgba16Float` output and comparing it with what `common/brdf.rs` predicts, through the
//! tone-mapping curve of `common/tone_curve.rs`: the BRDF; the metallic-roughness, normal,
//! occlusion and emissive textures; masks; double-sided and mirrored instances; the formats of the
//! texture slots; and the clamp at the largest value of the high-dynamic-range target. Runs on
//! whatever adapter the machine offers.
//!
//! The output is 9 × 9 pixels, so that the center of pixel (4, 4) lies on the camera's axis: the
//! surface at the origin is seen straight on there, up to rounding. Textures are 1 × 1 (or 2 × 1)
//! and read with a nearest sampler, so that every pixel of a surface reads the same texel.

#[path = "common/brdf.rs"]
mod brdf;
#[path = "common/shown.rs"]
mod shown;
#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use std::f32::consts::PI;

use slime_gfx::{
    AddressMode, Filter, FloatImage, Gpu, RenderTarget, Sampler, SamplerDescriptor, Texture,
    TextureFormat,
};
use slime_render::glam::{DVec3, Mat4, Quat, Vec3, Vec4};
use slime_render::{
    AlphaMode, Camera, DirectionalLight, Instance, Light, Material, MaterialData, Mesh, MeshData,
    Renderer, SampledTexture, Scene, Sky,
};

use brdf::Surface;
use shown::assert_shows;

const SIZE: u32 = 9;
/// The pixel on the camera's axis.
const CENTER: (u32, u32) = (4, 4);
/// A white matte dielectric, the default material.
const WHITE: Surface = Surface {
    base_color: [1.0; 3],
    metallic: 0.0,
    roughness: 1.0,
};
/// A transform that flattens what it places to a point. Built column by column:
/// `Mat4::from_scale` asserts, when glam's assertions are on, that the scale is not zero.
const POINT: Mat4 = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::W);
/// A normal texture's texel tilted by about 30° towards the tangent: (0.498, 0.004, 0.867) once
/// decoded and normalized.
const TOWARDS_TANGENT: [u8; 4] = [191, 128, 238, 255];
/// The same tilt towards the bitangent, up the texture's image.
const TOWARDS_BITANGENT: [u8; 4] = [128, 191, 238, 255];

/// The renderer, a 9 × 9 `Rgba16Float` output, a white square two units wide facing +Z centred
/// on the origin (texture coordinates from (0, 1) at its bottom left, tangents along +X), and a
/// nearest sampler for the tests' small textures.
struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    output: RenderTarget,
    square: Mesh,
    nearest: Sampler,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
        let output = gpu.create_render_target("shading", SIZE, SIZE, TextureFormat::Rgba16Float);
        let square = renderer
            .create_mesh(
                &gpu,
                "square",
                &MeshData {
                    positions: vec![
                        [-1.0, -1.0, 0.0],
                        [1.0, -1.0, 0.0],
                        [1.0, 1.0, 0.0],
                        [-1.0, 1.0, 0.0],
                    ],
                    normals: vec![[0.0, 0.0, 1.0]; 4],
                    uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                    tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
                    indices: vec![0, 1, 2, 0, 2, 3],
                },
            )
            .expect("the square is valid");
        let nearest = gpu.create_sampler(
            "nearest",
            &SamplerDescriptor {
                mag_filter: Filter::Nearest,
                min_filter: Filter::Nearest,
                address_u: AddressMode::ClampToEdge,
                address_v: AddressMode::ClampToEdge,
            },
        );
        Self {
            gpu,
            renderer,
            output,
            square,
            nearest,
        }
    }

    fn material(&self, data: &MaterialData<'_>) -> Material {
        self.renderer.create_material(&self.gpu, "material", data)
    }

    /// A texture of `width` × `height` pixels of `format`, from `pixels`, rows from the top.
    fn texture(&self, format: TextureFormat, width: u32, pixels: &[[u8; 4]]) -> Texture {
        let height = u32::try_from(pixels.len()).expect("a small texture") / width;
        self.gpu
            .create_texture("texture", width, height, format, pixels.as_flattened())
    }

    /// `texture` read through the nearest sampler.
    fn sampled<'a>(&'a self, texture: &'a Texture) -> SampledTexture<'a> {
        SampledTexture {
            texture,
            sampler: &self.nearest,
        }
    }

    /// Draws the square once for each of `instances`, with its material and transform, in this
    /// order, seen from `camera` under `lighting`, and reads the output back.
    fn draw(
        &mut self,
        camera: Camera,
        (light, sky): Lighting,
        instances: &[(&Material, Mat4)],
    ) -> FloatImage {
        let instances: Vec<Instance<'_>> = instances
            .iter()
            .map(|&(material, transform)| Instance {
                mesh: &self.square,
                material,
                transform,
            })
            .collect();
        let uniform = Vec3::splat(sky);
        let environment = (sky > 0.0).then(|| {
            self.renderer.create_environment(
                &self.gpu,
                &Sky {
                    zenith: uniform,
                    horizon: uniform,
                    ground: uniform,
                    sun: None,
                },
            )
        });
        self.renderer.render(
            &self.gpu,
            &self.output,
            &Scene {
                camera,
                lights: &[Light::Directional(light)],
                environment: environment.as_ref(),
                exposure: 1.0,
                bloom: 0.0,
                instances: &instances,
            },
        );
        self.gpu
            .read_float_render_target(&self.output)
            .expect("the image is read back")
    }
}

/// A camera on the Z axis at `z`, looking at the origin.
fn camera(z: f32) -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, z),
        ..Camera::default()
    }
}

/// A white directional light, and the radiance of a uniform sky (none at 0).
type Lighting = (DirectionalLight, f32);

/// A directional light that arrives from `to_light`, of `illuminance`, under a uniform sky of
/// radiance `sky`.
fn lighting(to_light: Vec3, illuminance: f32, sky: f32) -> Lighting {
    let light = DirectionalLight {
        direction: -to_light,
        color: Vec3::ONE,
        illuminance,
        casts_shadows: true,
    };
    (light, sky)
}

/// What `surface`, of normal `normal`, reflects towards the camera at `z` from the center of the
/// output under a light of `illuminance` arriving from `to_light`.
fn expected(
    surface: Surface,
    normal: DVec3,
    to_light: Vec3,
    illuminance: f32,
    camera_z: f32,
) -> [f64; 3] {
    let to_camera = brdf::to_camera(&camera(camera_z), (SIZE, SIZE), CENTER, 0.0);
    brdf::reflected(surface, normal, to_light.as_dvec3().normalize(), to_camera)
        .map(|channel| channel * f64::from(illuminance))
}

/// Draws the square with `transform`, a white matte material whose normal texture is `texel` at
/// `normal_scale`, from the camera at z = 3, under a light of π / 2 arriving from `to_light`.
fn with_normal_texture(
    setup: &mut Setup,
    texel: [u8; 4],
    normal_scale: f32,
    transform: Mat4,
    to_light: Vec3,
) -> FloatImage {
    let texture = setup.texture(TextureFormat::Rgba8Unorm, 1, &[texel]);
    let material = setup.material(&MaterialData {
        normal_texture: Some(setup.sampled(&texture)),
        normal_scale,
        ..MaterialData::default()
    });
    setup.draw(
        camera(3.0),
        lighting(to_light, PI / 2.0, 0.0),
        &[(&material, transform)],
    )
}

/// The decoded, normalized normal of a normal texture's texel.
fn decoded(texel: [u8; 4]) -> DVec3 {
    DVec3::new(texel[0].into(), texel[1].into(), texel[2].into())
        .map(|channel| channel / 255.0 * 2.0 - 1.0)
        .normalize()
}

#[test]
fn the_brdf_of_the_tests_gives_its_closed_forms() {
    let close = |actual: f64, expected: f64, case: &str| {
        assert!(
            (actual - expected).abs() < 1e-12,
            "{case}: expected {expected}, got {actual}"
        );
    };
    let z = DVec3::Z;
    let red = |surface: Surface, to_light: DVec3, to_camera: DVec3| {
        brdf::reflected(surface, z, to_light, to_camera)[0] * std::f64::consts::PI
    };
    // A white dielectric seen from the light: 0.96 diffused, and the specular layer's
    // 0.04 × D × V = 0.01 / α² at normal incidence, with α = roughness².
    for roughness in [1.0, 0.5, 0.25] {
        let alpha: f64 = roughness * roughness;
        close(
            red(Surface { roughness, ..WHITE }, z, z),
            0.96 + 0.01 / (alpha * alpha),
            &format!("a white dielectric of roughness {roughness}"),
        );
    }
    // A white metal reflects D × V = 1 / (4 α²) at normal incidence.
    let metal = Surface {
        metallic: 1.0,
        roughness: 0.5,
        ..WHITE
    };
    close(red(metal, z, z), 4.0, "a white metal of roughness 0.5");
    // Seen from the light at 60° from the normal, a rough dielectric diffuses 0.96 × cos 60° and
    // reflects 0.01 whatever the angle.
    let sixty = DVec3::new(3f64.sqrt() / 2.0, 0.0, 0.5);
    close(red(WHITE, sixty, sixty), 0.49, "seen from the light at 60°");
    // The shader shades no smoother than 0.045.
    close(
        red(
            Surface {
                roughness: 0.0,
                ..WHITE
            },
            z,
            z,
        ),
        red(
            Surface {
                roughness: 0.045,
                ..WHITE
            },
            z,
            z,
        ),
        "a mirror",
    );
}

#[test]
fn a_white_dielectric_lit_at_normal_incidence_reflects_what_the_brdf_predicts() {
    let mut setup = Setup::new();
    for (roughness, closed_form) in [(1.0, 0.485), (0.5, 0.56)] {
        let material = setup.material(&MaterialData {
            roughness,
            ..MaterialData::default()
        });
        let image = setup.draw(
            camera(3.0),
            lighting(Vec3::Z, PI / 2.0, 0.0),
            &[(&material, Mat4::IDENTITY)],
        );
        let surface = Surface {
            roughness: roughness.into(),
            ..WHITE
        };
        let light = expected(surface, DVec3::Z, Vec3::Z, PI / 2.0, 3.0);
        // E / π × (0.96 + 0.01 / α²), with E = π / 2.
        assert!(
            (light[0] - closed_form).abs() < 1e-6,
            "{light:?} against {closed_form}"
        );
        assert_shows(
            &image,
            CENTER,
            light,
            &format!("a white dielectric of roughness {roughness}"),
        );
    }
}

#[test]
fn a_rougher_surface_spreads_its_highlight() {
    let mut setup = Setup::new();
    // What a white metal reflects under a light of 1, measured on the GPU: each light is chosen so
    // that the surface shows a light of 0.5, which the curve lowers by 0.04 exactly.
    let mut measured = |roughness: f32, degrees: f32| {
        let to_light = Vec3::new(degrees.to_radians().sin(), 0.0, degrees.to_radians().cos());
        let surface = Surface {
            metallic: 1.0,
            roughness: roughness.into(),
            ..WHITE
        };
        let predicted = expected(surface, DVec3::Z, to_light, 1.0, 3.0)[0];
        let illuminance = (0.5 / predicted) as f32;
        let material = setup.material(&MaterialData {
            metallic: 1.0,
            roughness,
            ..MaterialData::default()
        });
        let image = setup.draw(
            camera(3.0),
            lighting(to_light, illuminance, 0.0),
            &[(&material, Mat4::IDENTITY)],
        );
        let shown = image.pixels[((CENTER.1 * SIZE + CENTER.0) * 4) as usize];
        let reflected = f64::from(shown + 0.04) / f64::from(illuminance);
        assert!(
            (reflected / predicted - 1.0).abs() < 5e-3,
            "roughness {roughness} at {degrees}°: {reflected} against {predicted}"
        );
        reflected
    };
    // Seen from the light, the smoother surface reflects more (9.82 against 0.33); 40° away from
    // it, less (0.042 against 0.170): its highlight is narrower.
    assert!(measured(0.3, 0.0) > measured(0.7, 0.0));
    assert!(measured(0.3, 40.0) < measured(0.7, 40.0));
}

#[test]
fn a_metal_reflects_in_its_base_color() {
    let mut setup = Setup::new();
    let base_color = [1.0, 0.5, 0.25];
    // At normal incidence and a roughness of 0.5, under a light of 0.15π: the metal reflects 0.6
    // of its base color; the dielectric diffuses 0.144 of it and reflects 0.024 in white.
    for (metallic, closed_form) in [
        (1.0, [0.6, 0.3, 0.15]),
        (0.0, [0.168, 0.096, 0.06]),
        (0.5, [0.384, 0.198, 0.105]),
    ] {
        let material = setup.material(&MaterialData {
            base_color: [1.0, 0.5, 0.25, 1.0],
            metallic,
            roughness: 0.5,
            ..MaterialData::default()
        });
        let image = setup.draw(
            camera(3.0),
            lighting(Vec3::Z, 0.15 * PI, 0.0),
            &[(&material, Mat4::IDENTITY)],
        );
        let surface = Surface {
            base_color,
            metallic: metallic.into(),
            roughness: 0.5,
        };
        let light = expected(surface, DVec3::Z, Vec3::Z, 0.15 * PI, 3.0);
        assert!(
            light
                .iter()
                .zip(closed_form)
                .all(|(light, closed_form)| (light - closed_form).abs() < 1e-6),
            "{light:?} against {closed_form:?}"
        );
        assert_shows(&image, CENTER, light, &format!("metallic {metallic}"));
    }
}

#[test]
fn the_metallic_roughness_texture_gives_roughness_in_green_and_metallic_in_blue() {
    let mut setup = Setup::new();
    // Red, which glTF leaves to the occlusion, at 0: read as metallic, it would make a dielectric.
    let texture = setup.texture(TextureFormat::Rgba8Unorm, 1, &[[0, 128, 255, 255]]);
    let material = setup.material(&MaterialData {
        metallic: 1.0,
        roughness: 1.0,
        metallic_roughness_texture: Some(setup.sampled(&texture)),
        ..MaterialData::default()
    });
    let image = setup.draw(
        camera(3.0),
        lighting(Vec3::Z, 0.4, 0.0),
        &[(&material, Mat4::IDENTITY)],
    );
    // A metal of roughness 128/255: 0.501 (swapped channels would give 0.078).
    let surface = Surface {
        metallic: 1.0,
        roughness: 128.0 / 255.0,
        ..WHITE
    };
    assert_shows(
        &image,
        CENTER,
        expected(surface, DVec3::Z, Vec3::Z, 0.4, 3.0),
        "metallic 1, roughness 128/255",
    );
}

#[test]
fn a_normal_texture_tilts_the_shading_as_a_tilted_surface_would() {
    let mut setup = Setup::new();
    let illuminance = PI / 2.0;
    let draw = |setup: &mut Setup, texel: [u8; 4], normal_scale: f32, to_light: Vec3| {
        with_normal_texture(setup, texel, normal_scale, Mat4::IDENTITY, to_light)
    };
    // Tilted by 30° towards the tangent, +X, and lit from the camera: 0.421.
    let tilted = decoded(TOWARDS_TANGENT);
    let light = expected(WHITE, tilted, Vec3::Z, illuminance, 3.0);
    let image = draw(&mut setup, TOWARDS_TANGENT, 1.0, Vec3::Z);
    assert_shows(&image, CENTER, light, "tilted towards the tangent");
    // A plain square turned the same way shows the same.
    let turned = setup.material(&MaterialData::default());
    let rotation = Quat::from_rotation_arc(Vec3::Z, tilted.as_vec3());
    let image = setup.draw(
        camera(3.0),
        lighting(Vec3::Z, illuminance, 0.0),
        &[(&turned, Mat4::from_quat(rotation))],
    );
    assert_shows(&image, CENTER, light, "a plain square turned the same way");
    // Green is +Y, up the texture's image: lit from above, a normal tilted up is lit more (0.469)
    // than one tilted down (0.127). A bitangent turned the wrong way would swap them.
    let from_above = Vec3::new(0.0, 1.0, 1.0).normalize();
    for (texel, case) in [
        (TOWARDS_BITANGENT, "tilted up the image"),
        ([128, 64, 238, 255], "tilted down the image"),
    ] {
        let light = expected(WHITE, decoded(texel), from_above, illuminance, 3.0);
        assert_shows(
            &draw(&mut setup, texel, 1.0, from_above),
            CENTER,
            light,
            case,
        );
    }
    // The scale multiplies x and y before the normal is normalized; 0 keeps the mesh's normal.
    let texel = TOWARDS_BITANGENT.map(f64::from);
    let half = DVec3::new(
        (texel[0] / 255.0 * 2.0 - 1.0) * 0.5,
        (texel[1] / 255.0 * 2.0 - 1.0) * 0.5,
        texel[2] / 255.0 * 2.0 - 1.0,
    )
    .normalize();
    let light = expected(WHITE, half, from_above, illuminance, 3.0);
    assert_shows(
        &draw(&mut setup, TOWARDS_BITANGENT, 0.5, from_above),
        CENTER,
        light,
        "a scale of 0.5",
    );
    let light = expected(WHITE, DVec3::Z, Vec3::Z, illuminance, 3.0);
    assert_shows(
        &draw(&mut setup, TOWARDS_TANGENT, 0.0, Vec3::Z),
        CENTER,
        light,
        "a scale of 0",
    );
}

#[test]
fn a_masked_material_discards_fragments_below_its_cutoff() {
    let mut setup = Setup::new();
    let illuminance = PI / 2.0;
    let lit_from_the_camera = lighting(Vec3::Z, illuminance, 0.0);
    // A white square behind, twice as large, drawn after the near one: it shows wherever the near
    // one wrote no depth.
    let far = setup.material(&MaterialData::default());
    let behind =
        Mat4::from_translation(Vec3::new(0.0, 0.0, -1.0)) * Mat4::from_scale(Vec3::splat(2.0));
    let black = Surface {
        base_color: [0.0; 3],
        ..WHITE
    };
    let near_shown = expected(black, DVec3::Z, Vec3::Z, illuminance, 3.0);
    let far_shown = {
        let to_camera = brdf::to_camera(&camera(3.0), (SIZE, SIZE), CENTER, -1.0);
        brdf::reflected(WHITE, DVec3::Z, DVec3::Z, to_camera).map(|c| c * f64::from(illuminance))
    };
    for (alpha, alpha_mode, shown, case) in [
        (
            0.49,
            AlphaMode::Mask { cutoff: 0.5 },
            far_shown,
            "below the cutoff",
        ),
        (
            0.5,
            AlphaMode::Mask { cutoff: 0.5 },
            near_shown,
            "at the cutoff",
        ),
        (0.49, AlphaMode::Opaque, near_shown, "opaque"),
    ] {
        let near = setup.material(&MaterialData {
            base_color: [0.0, 0.0, 0.0, alpha],
            alpha_mode,
            ..MaterialData::default()
        });
        let image = setup.draw(
            camera(3.0),
            lit_from_the_camera,
            &[(&near, Mat4::IDENTITY), (&far, behind)],
        );
        assert_shows(&image, CENTER, shown, case);
    }
    // The texture's alpha counts too: its left half is transparent.
    let texture = setup.texture(
        TextureFormat::Rgba8UnormSrgb,
        2,
        &[[255, 255, 255, 0], [255, 255, 255, 255]],
    );
    let near = setup.material(&MaterialData {
        base_color: [0.0, 0.0, 0.0, 1.0],
        base_color_texture: Some(setup.sampled(&texture)),
        alpha_mode: AlphaMode::Mask { cutoff: 0.5 },
        ..MaterialData::default()
    });
    let image = setup.draw(
        camera(3.0),
        lit_from_the_camera,
        &[(&near, Mat4::IDENTITY), (&far, behind)],
    );
    for (pixel, surface, plane_z, case) in [
        ((2, 4), WHITE, -1.0, "where the texture is transparent"),
        ((6, 4), black, 0.0, "where the texture is opaque"),
    ] {
        let to_camera = brdf::to_camera(&camera(3.0), (SIZE, SIZE), pixel, plane_z);
        let light = brdf::reflected(surface, DVec3::Z, DVec3::Z, to_camera)
            .map(|channel| channel * f64::from(illuminance));
        assert_shows(&image, pixel, light, case);
    }
}

#[test]
fn a_double_sided_material_shows_its_back_faces_lit_from_their_side() {
    let mut setup = Setup::new();
    let single = setup.material(&MaterialData::default());
    let double = setup.material(&MaterialData {
        double_sided: true,
        ..MaterialData::default()
    });
    // Seen from behind, from −Z, under a uniform sky of 0.25.
    let from_the_camera = lighting(-Vec3::Z, PI / 2.0, 0.25);
    let image = setup.draw(camera(-3.0), from_the_camera, &[(&single, Mat4::IDENTITY)]);
    assert_shows(
        &image,
        CENTER,
        [0.25; 3],
        "single-sided: culled, the sky behind it",
    );
    // The back face is shaded with its normal turned around, towards the camera.
    let back = expected(WHITE, -DVec3::Z, -Vec3::Z, PI / 2.0, -3.0).map(|light| light + 0.25);
    let image = setup.draw(camera(-3.0), from_the_camera, &[(&double, Mat4::IDENTITY)]);
    assert_shows(
        &image,
        CENTER,
        back,
        "double-sided, lit from the camera's side",
    );
    // Lit from the other side, it gets the sky's light only. The light comes from exactly
    // opposite the camera there: the half vector has no direction.
    let image = setup.draw(
        camera(-3.0),
        lighting(Vec3::Z, PI / 2.0, 0.25),
        &[(&double, Mat4::IDENTITY)],
    );
    assert_shows(
        &image,
        CENTER,
        [0.25; 3],
        "double-sided, lit from the other side",
    );
}

#[test]
fn a_mirrored_instance_keeps_its_front_faces_and_the_handedness_of_its_normal_texture() {
    let mut setup = Setup::new();
    let mirror = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    let illuminance = PI / 2.0;
    let front = expected(WHITE, DVec3::Z, Vec3::Z, illuminance, 3.0);
    let single = setup.material(&MaterialData::default());
    // A square flattened to a point first, drawn by the pipeline that culls back faces, then the
    // mirrored one, by the pipeline that culls front faces: the values bound before the switch
    // stay bound.
    let image = setup.draw(
        camera(3.0),
        lighting(Vec3::Z, illuminance, 0.0),
        &[(&single, POINT), (&single, mirror)],
    );
    assert_shows(&image, CENTER, front, "single-sided and mirrored");
    let double = setup.material(&MaterialData {
        double_sided: true,
        ..MaterialData::default()
    });
    let image = setup.draw(
        camera(3.0),
        lighting(Vec3::Z, illuminance, 0.0),
        &[(&double, mirror)],
    );
    assert_shows(
        &image,
        CENTER,
        front,
        "double-sided and mirrored, seen from the front",
    );
    // The mirror carries the tangent to −X: a normal tilted towards it leans away from +X.
    let mut draw = |texel: [u8; 4], transform: Mat4, to_light: Vec3| {
        with_normal_texture(&mut setup, texel, 1.0, transform, to_light)
    };
    let from_the_left = Vec3::new(-1.0, 0.0, 1.0).normalize();
    let tilted = decoded(TOWARDS_TANGENT);
    let leaning_left = DVec3::new(-tilted.x, tilted.y, tilted.z);
    let light = expected(WHITE, leaning_left, from_the_left, illuminance, 3.0);
    assert_shows(
        &draw(TOWARDS_TANGENT, mirror, from_the_left),
        CENTER,
        light,
        "tilted towards the tangent, mirrored",
    );
    let light = expected(WHITE, tilted, from_the_left, illuminance, 3.0);
    assert_shows(
        &draw(TOWARDS_TANGENT, Mat4::IDENTITY, from_the_left),
        CENTER,
        light,
        "tilted towards the tangent",
    );
    // Up the image stays up: a mirror along X leaves the bitangent along +Y, which a bitangent
    // computed in world space without the handedness would turn down.
    let from_above = Vec3::new(0.0, 1.0, 1.0).normalize();
    let light = expected(
        WHITE,
        decoded(TOWARDS_BITANGENT),
        from_above,
        illuminance,
        3.0,
    );
    for (transform, case) in [(mirror, "mirrored"), (Mat4::IDENTITY, "not mirrored")] {
        assert_shows(
            &draw(TOWARDS_BITANGENT, transform, from_above),
            CENTER,
            light,
            &format!("tilted up the image, {case}"),
        );
    }
}

#[test]
fn occlusion_darkens_the_light_of_the_environment_only() {
    let mut setup = Setup::new();
    // 51 / 255 is exactly 0.2.
    let texture = setup.texture(TextureFormat::Rgba8Unorm, 1, &[[51, 0, 0, 255]]);
    let draw = |setup: &mut Setup, occlusion_strength: f32, illuminance: f32| {
        let material = setup.material(&MaterialData {
            occlusion_texture: Some(setup.sampled(&texture)),
            occlusion_strength,
            ..MaterialData::default()
        });
        setup.draw(
            camera(3.0),
            lighting(Vec3::Z, illuminance, 0.5),
            &[(&material, Mat4::IDENTITY)],
        )
    };
    // At a strength of 0.5, the sky's light is multiplied by 1 + 0.5 × (0.2 − 1) = 0.6.
    assert_shows(
        &draw(&mut setup, 0.5, 0.0),
        CENTER,
        [0.3; 3],
        "the sky's light only",
    );
    let direct = expected(WHITE, DVec3::Z, Vec3::Z, PI / 2.0, 3.0);
    assert_shows(
        &draw(&mut setup, 0.5, PI / 2.0),
        CENTER,
        direct.map(|light| light + 0.3),
        "and a directional light",
    );
    assert_shows(
        &draw(&mut setup, 0.0, 0.0),
        CENTER,
        [0.5; 3],
        "a strength of 0",
    );
    // A texel of 0 takes all the sky's light, but not the emitted light.
    let none = setup.texture(TextureFormat::Rgba8Unorm, 1, &[[0, 0, 0, 255]]);
    let material = setup.material(&MaterialData {
        occlusion_texture: Some(setup.sampled(&none)),
        emissive: [0.1; 3],
        ..MaterialData::default()
    });
    let image = setup.draw(
        camera(3.0),
        lighting(Vec3::Z, 0.0, 0.5),
        &[(&material, Mat4::IDENTITY)],
    );
    assert_shows(
        &image,
        CENTER,
        [0.1; 3],
        "an emitting surface fully occluded",
    );
}

#[test]
fn emitted_light_is_added_whatever_lights_the_surface() {
    let mut setup = Setup::new();
    let dark = lighting(Vec3::Z, 0.0, 0.0);
    let black = [0.0, 0.0, 0.0, 1.0];
    let draw = |setup: &mut Setup, data: &MaterialData<'_>| {
        let material = setup.material(data);
        setup.draw(camera(3.0), dark, &[(&material, Mat4::IDENTITY)])
    };
    let image = draw(
        &mut setup,
        &MaterialData {
            base_color: black,
            emissive: [1.0, 0.5, 0.25],
            emissive_strength: 0.4,
            ..MaterialData::default()
        },
    );
    assert_shows(
        &image,
        CENTER,
        [0.4, 0.2, 0.1],
        "an emissive factor and strength",
    );
    // A strength above 1 makes the surface brighter than white.
    let image = draw(
        &mut setup,
        &MaterialData {
            base_color: black,
            emissive: [0.25, 0.125, 0.0625],
            emissive_strength: 4.0,
            ..MaterialData::default()
        },
    );
    assert_shows(&image, CENTER, [1.0, 0.5, 0.25], "a strength of 4");
    let texture = setup.texture(TextureFormat::Rgba8UnormSrgb, 1, &[[255, 255, 0, 255]]);
    let material = setup.material(&MaterialData {
        base_color: black,
        emissive: [1.0, 0.5, 0.25],
        emissive_strength: 0.4,
        emissive_texture: Some(setup.sampled(&texture)),
        ..MaterialData::default()
    });
    let image = setup.draw(camera(3.0), dark, &[(&material, Mat4::IDENTITY)]);
    assert_shows(&image, CENTER, [0.4, 0.2, 0.0], "an emissive texture");
}

#[test]
fn light_beyond_the_range_of_the_hdr_target_is_stored_as_its_largest_value() {
    let mut setup = Setup::new();
    let material = setup.material(&MaterialData {
        base_color: [0.0, 0.0, 0.0, 1.0],
        emissive: [1.0, 0.0, 0.0],
        emissive_strength: 1e7,
        ..MaterialData::default()
    });
    let image = setup.draw(
        camera(3.0),
        lighting(Vec3::Z, 0.0, 0.0),
        &[(&material, Mat4::IDENTITY)],
    );
    let start = ((CENTER.1 * SIZE + CENTER.0) * 4) as usize;
    assert!(
        image.pixels[start..start + 3]
            .iter()
            .all(|channel| channel.is_finite()),
        "{:?}",
        &image.pixels[start..start + 3]
    );
    // The target holds exposed light divided by 64: 10^7 is stored as 65 504, 64 × 65 504 once
    // multiplied back (ADR-0040).
    assert_shows(
        &image,
        CENTER,
        [64.0 * 65504.0, 0.0, 0.0],
        "an emission of 10^7",
    );
}

#[test]
#[should_panic(expected = "its normal texture is Rgba8UnormSrgb")]
fn an_srgb_texture_in_a_data_slot_is_a_bug() {
    let setup = Setup::new();
    let texture = setup.texture(TextureFormat::Rgba8UnormSrgb, 1, &[[128, 128, 255, 255]]);
    setup.material(&MaterialData {
        normal_texture: Some(setup.sampled(&texture)),
        ..MaterialData::default()
    });
}

#[test]
#[should_panic(expected = "its base color texture is Rgba8Unorm")]
fn a_linear_texture_in_a_color_slot_is_a_bug() {
    let setup = Setup::new();
    let texture = setup.texture(TextureFormat::Rgba8Unorm, 1, &[[255; 4]]);
    setup.material(&MaterialData {
        base_color_texture: Some(setup.sampled(&texture)),
        ..MaterialData::default()
    });
}
