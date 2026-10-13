//! Lights of every kind, in their physical units (ADR-0037), checked by reading back an
//! `Rgba16Float` output and comparing it with what `common/brdf.rs` predicts, through the tone
//! curve of `common/tone_curve.rs`: a point light's falloff with distance and its range, a spot
//! light's cone, a scene with the most lights the renderer draws and one with more, a light on the
//! surface it lights, and the exposure applied before the high-dynamic-range target. Runs on
//! whatever adapter the machine offers.
//!
//! The output is 9 × 9 pixels and the camera is on the Z axis at z = 3, looking at a white matte
//! square in the plane z = 0, so that the center of pixel (4, 4) sees the center of the square
//! straight on, as in `shading.rs`. The square is four triangles around a vertex at its center:
//! the fragment of that pixel lies exactly at the origin, where a light can be put. The lights'
//! falloff and cones are computed here from `KHR_lights_punctual`'s formulas, independently of
//! the shader.

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

use slime_gfx::{Color, FloatImage, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::{DVec3, Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, MAX_LIGHTS, Material, MaterialData, Mesh, MeshData,
    PointLight, Renderer, Scene, SpotLight,
};

use brdf::Surface;
use shown::assert_shows;

const SIZE: u32 = 9;
/// The pixel on the camera's axis.
const CENTER: (u32, u32) = (4, 4);
const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
/// A white matte dielectric, the default material.
const WHITE: Surface = Surface {
    base_color: [1.0; 3],
    metallic: 0.0,
    roughness: 1.0,
};

/// The renderer, a 9 × 9 `Rgba16Float` output, and a white matte square two units wide facing +Z,
/// centred on the origin, made of four triangles around a vertex there.
struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    output: RenderTarget,
    square: Mesh,
    white: Material,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
        let output = gpu.create_render_target("lights", SIZE, SIZE, TextureFormat::Rgba16Float);
        let square = renderer
            .create_mesh(
                &gpu,
                "square",
                &MeshData {
                    positions: vec![
                        [0.0, 0.0, 0.0],
                        [-1.0, -1.0, 0.0],
                        [1.0, -1.0, 0.0],
                        [1.0, 1.0, 0.0],
                        [-1.0, 1.0, 0.0],
                    ],
                    normals: vec![[0.0, 0.0, 1.0]; 5],
                    uvs: vec![[0.5, 0.5], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                    tangents: vec![[1.0, 0.0, 0.0, 1.0]; 5],
                    indices: vec![0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 1],
                },
            )
            .expect("the square is valid");
        let white = renderer.create_material(&gpu, "white", &MaterialData::default());
        Self {
            gpu,
            renderer,
            output,
            square,
            white,
        }
    }

    /// Draws the square with `material` (white when `None`) under `lights`, without ambient
    /// light, against black, at `exposure`, and reads the output back.
    fn draw_with(
        &mut self,
        material: Option<&Material>,
        lights: &[Light],
        exposure: f32,
    ) -> FloatImage {
        let instances = [Instance {
            mesh: &self.square,
            material: material.unwrap_or(&self.white),
            transform: Mat4::IDENTITY,
        }];
        self.renderer.render(
            &self.gpu,
            &self.output,
            &Scene {
                camera: camera(),
                lights,
                ambient: Vec3::ZERO,
                clear: BLACK,
                exposure,
                instances: &instances,
            },
        );
        self.gpu
            .read_float_render_target(&self.output)
            .expect("the image is read back")
    }

    /// Draws the white square under `lights` at `exposure`.
    fn draw(&mut self, lights: &[Light], exposure: f32) -> FloatImage {
        self.draw_with(None, lights, exposure)
    }
}

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 3.0),
        ..Camera::default()
    }
}

/// The illuminance that `light` gives the point `at`, per channel, in lux, and the unit vector
/// from `at` towards the light: `KHR_lights_punctual`'s inverse square of the distance, its window
/// clamp(1 − (d / range)⁴, 0, 1), and its cone, (clamp((cos − cos outer) / (cos inner − cos
/// outer), 0, 1))².
fn arriving(light: &Light, at: DVec3) -> ([f64; 3], DVec3) {
    let per_channel = |color: Vec3, amount: f64| color.as_dvec3().to_array().map(|c| c * amount);
    match *light {
        Light::Directional(light) => (
            per_channel(light.color, f64::from(light.illuminance)),
            -light.direction.as_dvec3().normalize(),
        ),
        Light::Point(light) => {
            let to_light = light.position.as_dvec3() - at;
            let distance = to_light.length();
            let amount =
                f64::from(light.intensity) * window(distance, light.range) / (distance * distance);
            (per_channel(light.color, amount), to_light / distance)
        }
        Light::Spot(light) => {
            let to_light = light.position.as_dvec3() - at;
            let distance = to_light.length();
            let cosine = light
                .direction
                .as_dvec3()
                .normalize()
                .dot(-to_light / distance);
            let (inner, outer) = (
                f64::from(light.inner_cone_angle).cos(),
                f64::from(light.outer_cone_angle).cos(),
            );
            let t = ((cosine - outer) / (inner - outer)).clamp(0.0, 1.0);
            let amount = f64::from(light.intensity) * window(distance, light.range) * t * t
                / (distance * distance);
            (per_channel(light.color, amount), to_light / distance)
        }
    }
}

/// `KHR_lights_punctual`'s window: 1 without range.
fn window(distance: f64, range: Option<f32>) -> f64 {
    range.map_or(1.0, |range| {
        (1.0 - (distance / f64::from(range)).powi(4)).clamp(0.0, 1.0)
    })
}

/// What the white square reflects towards the camera at the center pixel under `lights`, at
/// `exposure`.
fn expected(lights: &[Light], exposure: f32) -> [f64; 3] {
    let at = brdf::seen_point(&camera(), (SIZE, SIZE), CENTER, 0.0);
    let to_camera = brdf::to_camera(&camera(), (SIZE, SIZE), CENTER, 0.0);
    let mut sum = [0.0; 3];
    for light in lights {
        let (illuminance, to_light) = arriving(light, at);
        let reflected = brdf::reflected(WHITE, DVec3::Z, to_light, to_camera);
        for channel in 0..3 {
            sum[channel] += reflected[channel] * illuminance[channel] * f64::from(exposure);
        }
    }
    sum
}

/// The color of the pixel at `(x, y)`.
fn pixel(image: &FloatImage, (x, y): (u32, u32)) -> [f32; 3] {
    let start = ((y * image.width + x) * 4) as usize;
    [0, 1, 2].map(|channel| image.pixels[start + channel])
}

/// A white point light of `intensity` candelas on the camera's axis, `distance` metres in front of
/// the square.
fn point(distance: f32, intensity: f32, range: Option<f32>) -> Light {
    Light::Point(PointLight {
        position: Vec3::new(0.0, 0.0, distance),
        intensity,
        range,
        ..PointLight::default()
    })
}

#[test]
fn a_point_light_gives_a_quarter_of_its_light_at_twice_the_distance() {
    let mut setup = Setup::new();
    // Without range: the inverse square of the distance alone. At the center the light arrives
    // along the normal at both distances, so the BRDF is the same: 0.97 / π of π / 2 lux, 0.485
    // nits, then a quarter of it, 0.12125, both on the straight part of the tone curve.
    let (near, far) = (
        expected(&[point(1.0, PI / 2.0, None)], 1.0),
        expected(&[point(2.0, PI / 2.0, None)], 1.0),
    );
    assert!(
        (near[0] / far[0] - 4.0).abs() < 1e-12,
        "{near:?} against {far:?}"
    );
    for (distance, light, case) in [(1.0, near, "1 m away"), (2.0, far, "2 m away")] {
        let image = setup.draw(&[point(distance, PI / 2.0, None)], 1.0);
        assert_shows(&image, CENTER, light, case);
    }
}

#[test]
fn a_point_light_reaches_nothing_beyond_its_range() {
    let mut setup = Setup::new();
    // Inside the range, the window 1 − (d / range)⁴: 0.9375 at half of it, 0.684 at three
    // quarters.
    for (distance, range) in [(1.0, 2.0), (1.5, 2.0)] {
        let light = point(distance, PI / 2.0 * distance * distance, Some(range));
        let image = setup.draw(&[light], 1.0);
        assert_shows(
            &image,
            CENTER,
            expected(&[light], 1.0),
            &format!("{distance} m away, range {range} m"),
        );
    }
    // At the range and beyond, nothing at all, not a little.
    for (distance, range) in [(2.0, 2.0), (2.0, 1.5)] {
        let image = setup.draw(&[point(distance, 1000.0, Some(range))], 1.0);
        assert_eq!(
            pixel(&image, CENTER),
            [0.0; 3],
            "{distance} m away, range {range} m"
        );
    }
}

#[test]
fn a_spot_light_is_full_inside_its_inner_cone_dark_outside_its_outer_cone_and_smooth_between() {
    let mut setup = Setup::new();
    // 2 m in front of the square, its axis turned by `degrees` from the center of the square, of
    // 2π candelas: π / 2 lux at the center when it is full. The direction is three units long:
    // its length does not matter.
    let spot = |degrees: f32| {
        let angle = degrees.to_radians();
        Light::Spot(SpotLight {
            position: Vec3::new(0.0, 0.0, 2.0),
            direction: Vec3::new(angle.sin(), 0.0, -angle.cos()) * 3.0,
            intensity: 2.0 * PI,
            inner_cone_angle: 20f32.to_radians(),
            outer_cone_angle: 40f32.to_radians(),
            ..SpotLight::default()
        })
    };
    let full = expected(&[spot(0.0)], 1.0);
    for degrees in [0.0, 10.0, 20.0] {
        let image = setup.draw(&[spot(degrees)], 1.0);
        assert_shows(&image, CENTER, full, &format!("{degrees}°, inside"));
    }
    // Between the cones, falling from full to nothing: 0.652, 0.332 and 0.094 of the light.
    let mut previous = pixel(&setup.draw(&[spot(20.0)], 1.0), CENTER)[0];
    for degrees in [25.0, 30.0, 35.0, 40.0] {
        let light = expected(&[spot(degrees)], 1.0);
        let image = setup.draw(&[spot(degrees)], 1.0);
        assert_shows(&image, CENTER, light, &format!("{degrees}°, between"));
        let shown = pixel(&image, CENTER)[0];
        assert!(shown < previous, "{degrees}°: {shown}, after {previous}");
        previous = shown;
    }
    let image = setup.draw(&[spot(50.0)], 1.0);
    assert_eq!(pixel(&image, CENTER), [0.0; 3], "50°, outside");
}

/// The most lights a scene holds, of every kind, each of which adds at least 0.01 to one channel
/// at the center: one missing, or read from the wrong place, shows.
fn many_lights() -> Vec<Light> {
    let red = Vec3::X;
    let green = Vec3::Y;
    let blue = Vec3::Z;
    let spot = |position: Vec3, degrees: f32, color: Vec3, intensity: f32, range: Option<f32>| {
        let angle = degrees.to_radians();
        Light::Spot(SpotLight {
            position,
            direction: Vec3::new(angle.sin(), 0.0, -angle.cos()),
            color,
            intensity,
            range,
            inner_cone_angle: 0.1,
            outer_cone_angle: 0.4,
        })
    };
    let lights = vec![
        Light::Directional(DirectionalLight {
            direction: Vec3::NEG_Z,
            color: red,
            illuminance: 0.1,
        }),
        // Turned by 60° from the normal, two units long.
        Light::Directional(DirectionalLight {
            direction: Vec3::new(-3f32.sqrt(), 0.0, -1.0),
            color: green,
            illuminance: 0.2,
        }),
        Light::Directional(DirectionalLight {
            direction: Vec3::new(0.0, -1.0, -1.0),
            color: blue,
            illuminance: 0.15,
        }),
        Light::Point(PointLight {
            position: Vec3::new(0.0, 0.0, 1.0),
            color: Vec3::ONE,
            intensity: 0.05,
            range: None,
        }),
        Light::Point(PointLight {
            position: Vec3::new(0.0, 0.0, 2.0),
            color: red,
            intensity: 0.2,
            range: Some(4.0),
        }),
        Light::Point(PointLight {
            position: Vec3::new(0.5, 0.0, 1.0),
            color: green,
            intensity: 0.15,
            range: None,
        }),
        Light::Point(PointLight {
            position: Vec3::new(0.0, -1.0, 2.0),
            color: blue,
            intensity: 0.5,
            range: Some(10.0),
        }),
        Light::Point(PointLight {
            position: Vec3::new(0.0, 0.0, 0.5),
            color: Vec3::new(1.0, 0.5, 0.0),
            intensity: 0.01,
            range: None,
        }),
        spot(Vec3::new(0.0, 0.0, 1.0), 0.0, red, 0.06, None),
        spot(Vec3::new(0.0, 0.0, 2.0), 0.0, green, 0.2, Some(3.0)),
        // Between its cones.
        spot(Vec3::new(0.0, 0.0, 1.5), 15.0, blue, 0.3, None),
        spot(Vec3::new(0.0, 0.0, 1.0), 10.0, Vec3::ONE, 0.06, Some(2.0)),
        Light::Directional(DirectionalLight {
            direction: Vec3::new(1.0, 0.0, -2.0),
            color: Vec3::new(0.0, 0.5, 1.0),
            illuminance: 0.1,
        }),
        Light::Point(PointLight {
            position: Vec3::new(-0.3, 0.4, 1.2),
            color: red,
            intensity: 0.1,
            range: Some(5.0),
        }),
        spot(Vec3::new(0.0, 0.0, 3.0), 5.0, green, 0.4, None),
        // The last slot.
        spot(Vec3::new(0.0, 0.0, 2.5), 12.0, blue, 0.4, Some(6.0)),
    ];
    assert_eq!(lights.len(), MAX_LIGHTS);
    lights
}

#[test]
fn a_scene_with_the_most_lights_draws_every_one_of_them() {
    let mut setup = Setup::new();
    let lights = many_lights();
    let light = expected(&lights, 1.0);
    // Each light adds at least 0.01 to a channel, five times the tolerance, and the sums stay on
    // the straight part of the tone curve, where a difference shows whole.
    let at = brdf::seen_point(&camera(), (SIZE, SIZE), CENTER, 0.0);
    for (index, one) in lights.iter().enumerate() {
        let (illuminance, _) = arriving(one, at);
        let alone = expected(std::slice::from_ref(one), 1.0);
        assert!(
            alone.iter().any(|&channel| channel >= 0.01),
            "light {index} adds {alone:?} (illuminance {illuminance:?})"
        );
    }
    assert!(
        light.iter().all(|&channel| (0.08..0.8).contains(&channel)),
        "{light:?}"
    );
    assert_shows(&setup.draw(&lights, 1.0), CENTER, light, "16 lights");
}

#[test]
#[should_panic(expected = "the scene has 17 lights; the renderer draws at most 16")]
fn a_scene_with_more_lights_than_the_renderer_draws_is_a_bug() {
    let mut setup = Setup::new();
    let mut lights = many_lights();
    lights.push(point(1.0, 1.0, None));
    setup.draw(&lights, 1.0);
}

#[test]
fn a_light_on_the_surface_it_lights_gives_no_infinity() {
    let mut setup = Setup::new();
    // On the square's center, where the center pixel's fragment lies exactly: the light grazes
    // every other point of the square, and that one has no direction to the light, at a distance
    // of 0. Nothing is lit, and no pixel holds NaN or an infinity, nor 65 504, as one that the
    // shader's limit turns into it would.
    let image = setup.draw(&[point(0.0, 1000.0, None)], 1.0);
    for (index, channel) in image.pixels.iter().enumerate() {
        assert!(
            *channel == 0.0 || (index % 4 == 3 && *channel == 1.0),
            "pixel {}: {channel}",
            index / 4
        );
    }
    // 1 mm above it, the center receives what it would 1 cm away: π / 2 lux for 0.0001 × π / 2
    // candelas, not a hundred times more.
    let near = point(0.001, 1e-4 * PI / 2.0, None);
    let at_one_centimetre = expected(&[point(0.01, 1e-4 * PI / 2.0, None)], 1.0);
    assert_shows(
        &setup.draw(&[near], 1.0),
        CENTER,
        at_one_centimetre,
        "1 mm above",
    );
}

#[test]
fn the_exposure_applies_before_the_high_dynamic_range_target() {
    let mut setup = Setup::new();
    // Light far above 65 504 before the exposure, as candelas give: a white matte surface facing a
    // light of 10⁶ candelas 1 m away reflects 308 760 nits, which this exposure brings to 0.485.
    // Exposed after the target, it would be stored as 65 504 or an infinity first. `lighting.rs`
    // checks the same for a directional light.
    let lamp = point(1.0, 1e6, None);
    let exposure = PI / 2e6;
    assert_shows(
        &setup.draw(&[lamp], exposure),
        CENTER,
        expected(&[lamp], exposure),
        "a light of 10⁶ candelas 1 m away",
    );
    // Emission is exposed too: 10⁶ nits at 5 × 10⁻⁷.
    let glowing = setup.renderer.create_material(
        &setup.gpu,
        "glowing",
        &MaterialData {
            base_color: [0.0, 0.0, 0.0, 1.0],
            emissive: [1.0, 0.0, 0.0],
            emissive_strength: 1e6,
            ..MaterialData::default()
        },
    );
    assert_shows(
        &setup.draw_with(Some(&glowing), &[], 5e-7),
        CENTER,
        [1e6 * f64::from(5e-7_f32), 0.0, 0.0],
        "an emission of 10⁶ nits",
    );
}
