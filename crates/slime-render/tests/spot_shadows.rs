//! Shadows of spot lights (ADR-0058, ADR-0061), checked by reading back an `Rgba16Float` output and
//! comparing it with the same scene drawn without the shadow: a square between a spot light and a
//! floor darkens the floor where the light's rays through the square land, and nowhere else. Runs
//! on whatever adapter the machine offers.
//!
//! The floor is the plane z = 0, facing +Z, and "above" is along +Z. The camera stays below the
//! squares that cast shadows, so that it sees only the floor.

#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::{DAffine3, DVec3, Mat4, Vec3};
use slime_render::{
    Camera, Environment, Instance, Light, Material, MaterialData, Mesh, MeshData, RenderStats,
    Renderer, Scene, Sky, SpotLight,
};

const SIZE: u32 = 64;
/// The radiance of a uniform sky: what a white surface in shadow reflects.
const SKY: f32 = 0.125;

/// The renderer, a 64 × 64 `Rgba16Float` output, a white matte square one metre wide facing +Z,
/// and a uniform sky.
struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    output: RenderTarget,
    square: Mesh,
    white: Material,
    sky: Environment,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
        let output = gpu
            .create_render_target("spot shadows", SIZE, SIZE, TextureFormat::Rgba16Float)
            .expect("the GPU has memory for it");
        let square = renderer
            .create_mesh(&gpu, "square", &MeshData::square())
            .expect("the square is valid");
        let white = renderer
            .create_material(&gpu, "white", &MaterialData::default())
            .expect("the GPU has memory for it");
        let uniform = LinearColor::rgb(SKY, SKY, SKY);
        let sky = Environment::new(
            &gpu,
            &Sky {
                zenith: uniform,
                horizon: uniform,
                ground: uniform,
                sun: None,
            },
        )
        .expect("the GPU has memory for it");
        Self {
            gpu,
            renderer,
            output,
            square,
            white,
            sky,
        }
    }

    /// Draws the square once for each transform of `squares`, casting shadows or not as each says,
    /// seen from `camera` under `lights` and the sky, at an exposure of 1, and reads the output
    /// back.
    fn draw(
        &mut self,
        camera: Camera,
        lights: &[Light],
        squares: &[(Mat4, bool)],
    ) -> (FloatImage, RenderStats) {
        let instances: Vec<Instance> = squares
            .iter()
            .map(|&(transform, casts_shadows)| {
                let mut instance = Instance::new(
                    &self.square,
                    &self.white,
                    DAffine3::from_mat4(transform.as_dmat4()),
                );
                instance.casts_shadows = casts_shadows;
                instance
            })
            .collect();
        let mut scene = Scene::new(camera, instances.clone());
        scene.lights = lights.to_vec();
        scene.environment = Some(self.sky.clone());
        let stats = self
            .renderer
            .render(&self.gpu, &self.output, &scene)
            .expect("the frame is drawn");
        let image = self
            .gpu
            .read_float_render_target(&self.output)
            .expect("the image is read back");
        (image, stats)
    }
}

/// A square `side` metres wide, its center at `center`, facing +Z.
fn square(center: Vec3, side: f32) -> Mat4 {
    Mat4::from_translation(center) * Mat4::from_scale(Vec3::new(side, side, 1.0))
}

/// A camera 1.5 m above the point (x, y) of the floor, looking straight down at it.
fn camera_above(x: f32, y: f32) -> Camera {
    Camera {
        eye: Vec3::new(x, y, 1.5).as_dvec3(),
        target: Vec3::new(x, y, 0.0).as_dvec3(),
        ..Camera::default()
    }
}

/// The point of the floor seen at the center of `pixel`.
fn seen_point(camera: &Camera, (x, y): (u32, u32)) -> DVec3 {
    let inverse = camera.view_projection(1.0).inverse();
    let ndc_x = (f64::from(x) + 0.5) / f64::from(SIZE) * 2.0 - 1.0;
    let ndc_y = 1.0 - (f64::from(y) + 0.5) / f64::from(SIZE) * 2.0;
    let near = inverse.project_point3(DVec3::new(ndc_x, ndc_y, 1.0));
    let far = inverse.project_point3(DVec3::new(ndc_x, ndc_y, 0.5));
    near + (far - near) * (-near.z / (far.z - near.z))
}

/// The red channel of the pixel at (x, y).
fn red(image: &FloatImage, (x, y): (u32, u32)) -> f32 {
    image.pixels[((y * image.width + x) * 4) as usize]
}

#[test]
fn a_square_under_a_spot_light_shadows_the_floor_where_its_rays_land() {
    let mut setup = Setup::new();
    // A light 4 m above the origin, its axis tilted towards +x, wide enough to light the floor the
    // camera sees fully, and a square 1 m wide, 2 m above the floor and off the axis: the rays
    // through the square land on the floor twice as far from the light's foot, on a square 2 m
    // wide around (1.5, 0.5), seen from 1.5 m above (1.5, 0.5), 2.7 cm per pixel.
    let light = Light::Spot(SpotLight {
        position: DVec3::new(0.0, 0.0, 4.0),
        direction: Vec3::new(0.3, 0.1, -1.0),
        intensity: 4.0,
        inner_cone_angle: 0.7,
        outer_cone_angle: 0.8,
        ..SpotLight::default()
    });
    let camera = camera_above(1.5, 0.5);
    let floor = (square(Vec3::ZERO, 20.0), false);
    let blocker = square(Vec3::new(0.75, 0.25, 2.0), 1.0);
    let (shadowed, stats) = setup.draw(camera, &[light], &[floor, (blocker, true)]);
    assert_eq!(stats.spot_shadow_draw_calls, 1, "the blocker alone casts");
    let (lit, _) = setup.draw(camera, &[light], &[floor, (blocker, false)]);
    let in_shadow = tone_curve::tone_map([SKY; 3])[0];
    let (mut dark, mut checked) = (0, 0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let point = seen_point(&camera, (x, y));
            // Where the ray from the light to the point crosses the square's plane.
            let crossed = point.truncate() * 0.5;
            let from_edge = (crossed - DVec3::new(0.75, 0.25, 0.0).truncate())
                .abs()
                .max_element()
                - 0.5;
            // The filter blends the shadow's edge over 6 of the tile's texels, about 1.4 cm at the
            // square, which the normal offset moves by a few millimetres: 2 cm on either side of it
            // is left out.
            if from_edge.abs() < 0.02 {
                continue;
            }
            checked += 1;
            let expected = if from_edge < 0.0 {
                dark += 1;
                in_shadow
            } else {
                red(&lit, (x, y))
            };
            let shown = red(&shadowed, (x, y));
            assert!(
                (shown - expected).abs() <= 2e-3,
                "pixel ({x}, {y}), the square's plane crossed {from_edge} m from its edge: {shown}, \
                 expected {expected}"
            );
        }
    }
    assert!(
        dark > 500 && checked > 3000,
        "{dark} dark pixels of {checked}"
    );
}

/// Checks that `shadowed` shows what `lit` shows at every pixel, within 2 × 10⁻³.
fn assert_same(shadowed: &FloatImage, lit: &FloatImage, case: &str) {
    for (index, (shown, expected)) in shadowed.pixels.iter().zip(&lit.pixels).enumerate() {
        let pixel = index / 4;
        assert!(
            (shown - expected).abs() <= 2e-3,
            "{case}: pixel ({}, {}): {shown}, expected {expected}",
            pixel as u32 % SIZE,
            pixel as u32 / SIZE
        );
    }
}

#[test]
fn a_lit_floor_and_a_lit_sphere_do_not_shadow_themselves() {
    let mut setup = Setup::new();
    // A floor that casts shadows, under a light 3 m away whose axis leans from the floor's normal
    // by 0° to 60°, aimed at points the camera sees, so that the floor faces the axis at up to 60°
    // and the rays at up to 75°: it must show what it shows when it casts none.
    let camera = Camera {
        eye: DVec3::new(0.2, -0.3, 2.0),
        target: DVec3::new(0.0, 0.1, 0.0),
        ..Camera::default()
    };
    let floor = square(Vec3::ZERO, 20.0);
    for (lean, aim) in [
        (0.0f32, 0.0),
        (30.0, 0.0),
        (60.0, 0.0),
        (45.0, 0.8),
        (60.0, -1.5),
    ] {
        let lean = lean.to_radians();
        let direction = Vec3::new(lean.sin(), 0.0, -lean.cos());
        let light = Light::Spot(SpotLight {
            position: (Vec3::new(aim, 0.0, 0.0) - direction * 3.0).as_dvec3(),
            direction,
            intensity: 6.0,
            inner_cone_angle: 0.9,
            outer_cone_angle: 1.0,
            ..SpotLight::default()
        });
        let (shadowed, _) = setup.draw(camera, &[light], &[(floor, true)]);
        let (lit, _) = setup.draw(camera, &[light], &[(floor, false)]);
        assert_same(&shadowed, &lit, &format!("the floor, {lean} rad, {aim} m"));
    }

    // A sphere 40 cm wide of 16 × 24 facets, each about 5 cm, as the samples' small spheres: a
    // convex surface casts no shadow on what of itself faces the light.
    let sphere = setup
        .renderer
        .create_mesh(&setup.gpu, "sphere", &MeshData::sphere(16, 24))
        .expect("the sphere is valid");
    let camera = Camera {
        eye: DVec3::new(0.3, -0.6, 0.6),
        target: DVec3::ZERO,
        ..Camera::default()
    };
    let light = Light::Spot(SpotLight {
        position: DVec3::new(-1.0, -0.5, 2.0),
        direction: Vec3::new(1.0, 0.5, -2.0),
        intensity: 6.0,
        ..SpotLight::default()
    });
    let mut draw = |casts_shadows| {
        let mut instance = Instance::new(
            &sphere,
            &setup.white,
            DAffine3::from_scale(DVec3::splat(0.4)),
        );
        instance.casts_shadows = casts_shadows;
        let instances = [instance];
        let lights = [light];
        let mut scene = Scene::new(camera, instances.clone());
        scene.lights = lights.to_vec();
        scene.environment = Some(setup.sky.clone());
        setup
            .renderer
            .render(&setup.gpu, &setup.output, &scene)
            .expect("the frame is drawn");
        setup
            .gpu
            .read_float_render_target(&setup.output)
            .expect("the image is read back")
    };
    let (shadowed, lit) = (draw(true), draw(false));
    // Where the sphere turns away from the light, it shadows itself: only the half that faces the
    // light, less a band at its terminator, is compared.
    let to_light = DVec3::new(-1.0, -0.5, 2.0).normalize();
    let mut compared = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let Some(normal) = sphere_normal(&camera, (x, y), 0.2) else {
                continue;
            };
            if normal.dot(to_light) < 0.3 {
                continue;
            }
            compared += 1;
            let (shown, expected) = (red(&shadowed, (x, y)), red(&lit, (x, y)));
            assert!(
                (shown - expected).abs() <= 2e-3,
                "the sphere, pixel ({x}, {y}): {shown}, expected {expected}"
            );
        }
    }
    assert!(compared > 100, "{compared} pixels compared");
}

/// The unit normal of the sphere of `radius` at the origin where the ray of `pixel` meets it, if
/// it does, kept away from the silhouette, where the sphere's facets stray from the sphere.
fn sphere_normal(camera: &Camera, (x, y): (u32, u32), radius: f64) -> Option<DVec3> {
    let inverse = camera.view_projection(1.0).inverse();
    let ndc_x = (f64::from(x) + 0.5) / f64::from(SIZE) * 2.0 - 1.0;
    let ndc_y = 1.0 - (f64::from(y) + 0.5) / f64::from(SIZE) * 2.0;
    let eye = camera.eye;
    let far = inverse.project_point3(DVec3::new(ndc_x, ndc_y, 0.5));
    let d = (far - eye).normalize();
    let b = eye.dot(d);
    let c = eye.length_squared() - radius * radius;
    let discriminant = b * b - c;
    if discriminant <= 0.0 {
        return None;
    }
    let normal = (eye + d * (-b - discriminant.sqrt())) / radius;
    (normal.dot(-d) > 0.4).then_some(normal)
}

#[test]
fn a_spot_light_that_casts_no_shadows_lights_through_and_draws_no_tile() {
    let mut setup = Setup::new();
    let light = SpotLight {
        position: DVec3::new(0.0, 0.0, 4.0),
        direction: Vec3::new(0.3, 0.1, -1.0),
        intensity: 4.0,
        inner_cone_angle: 0.7,
        outer_cone_angle: 0.8,
        casts_shadows: false,
        ..SpotLight::default()
    };
    let camera = camera_above(1.5, 0.5);
    let floor = (square(Vec3::ZERO, 20.0), false);
    let blocker = square(Vec3::new(0.75, 0.25, 2.0), 1.0);
    let (through, stats) = setup.draw(camera, &[Light::Spot(light)], &[floor, (blocker, true)]);
    assert_eq!(
        (stats.spot_shadow_draw_calls, stats.spot_shadow_gpu_time),
        (0, None)
    );
    let (lit, _) = setup.draw(camera, &[Light::Spot(light)], &[floor, (blocker, false)]);
    assert_same(&through, &lit, "a light that casts no shadows");
}

#[test]
fn every_spot_light_reads_its_own_tile_however_many_cast_shadows() {
    let mut setup = Setup::new();
    // `count` lights 4 m above the floor, 3 cm apart along y, each of whose rays through the
    // blocker land around a point of the floor 3 cm from the next light's: the one that lights,
    // `lighting`, the others of no intensity, each still casting shadows into its own tile. A light
    // that read another's tile would shadow the floor a pixel off.
    let camera = camera_above(1.5, 0.5);
    let floor = (square(Vec3::ZERO, 20.0), false);
    let blocker = Vec3::new(0.75, 0.25, 2.0);
    let squares = [floor, (square(blocker, 1.0), true)];
    for (count, lighting) in [(1, 0), (4, 3), (5, 4), (16, 0), (16, 9), (16, 15)] {
        let lights: Vec<Light> = (0..count)
            .map(|index| {
                Light::Spot(SpotLight {
                    position: Vec3::new(0.0, 0.03 * index as f32, 4.0).as_dvec3(),
                    direction: Vec3::new(0.3, 0.1, -1.0),
                    intensity: if index == lighting { 4.0 } else { 0.0 },
                    inner_cone_angle: 0.7,
                    outer_cone_angle: 0.8,
                    ..SpotLight::default()
                })
            })
            .collect();
        let (shadowed, stats) = setup.draw(camera, &lights, &squares);
        assert_eq!(stats.spot_shadow_draw_calls, count, "{count} tiles");
        let (lit, _) = setup.draw(camera, &lights, &[floor, (square(blocker, 1.0), false)]);
        let foot = DVec3::new(0.0, 0.03 * f64::from(lighting), 4.0);
        let in_shadow = tone_curve::tone_map([SKY; 3])[0];
        let mut dark = 0;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let point = seen_point(&camera, (x, y));
                let crossed = (foot + point) * 0.5;
                let from_edge = (crossed - blocker.as_dvec3())
                    .truncate()
                    .abs()
                    .max_element()
                    - 0.5;
                // Tiles of 1024 texels blend the edge over 2.5 cm at the square.
                if from_edge.abs() < 0.03 {
                    continue;
                }
                let expected = if from_edge < 0.0 {
                    dark += 1;
                    in_shadow
                } else {
                    red(&lit, (x, y))
                };
                let shown = red(&shadowed, (x, y));
                assert!(
                    (shown - expected).abs() <= 2e-3,
                    "{count} lights, light {lighting}, pixel ({x}, {y}): {shown}, expected \
                     {expected}"
                );
            }
        }
        assert!(
            dark > 500,
            "{count} lights, light {lighting}: {dark} dark pixels"
        );
    }
}

#[test]
fn a_masked_material_casts_no_shadow_where_it_discards() {
    let mut setup = Setup::new();
    // The blocker's material wholly transparent, cut out by its mask: it casts nothing.
    let cut_out = setup
        .renderer
        .create_material(
            &setup.gpu,
            "cut out",
            &MaterialData {
                base_color: LinearColor::rgba(1.0, 1.0, 1.0, 0.0),
                alpha_mode: slime_render::AlphaMode::Mask { cutoff: 0.5 },
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");
    let light = Light::Spot(SpotLight {
        position: DVec3::new(0.0, 0.0, 4.0),
        direction: Vec3::new(0.3, 0.1, -1.0),
        intensity: 4.0,
        inner_cone_angle: 0.7,
        outer_cone_angle: 0.8,
        ..SpotLight::default()
    });
    let camera = camera_above(1.5, 0.5);
    let floor = square(Vec3::ZERO, 20.0);
    let blocker = square(Vec3::new(0.75, 0.25, 2.0), 1.0);
    let draw = |setup: &mut Setup, casts_shadows: bool| {
        let mut instances = vec![Instance::new(
            &setup.square,
            &setup.white,
            DAffine3::from_mat4(floor.as_dmat4()),
        )];
        instances[0].casts_shadows = false;
        let mut cut = Instance::new(
            &setup.square,
            &cut_out,
            DAffine3::from_mat4(blocker.as_dmat4()),
        );
        cut.casts_shadows = casts_shadows;
        instances.push(cut);
        let lights = [light];
        let mut scene = Scene::new(camera, instances.clone());
        scene.lights = lights.to_vec();
        scene.environment = Some(setup.sky.clone());
        let stats = setup
            .renderer
            .render(&setup.gpu, &setup.output, &scene)
            .expect("the frame is drawn");
        let image = setup
            .gpu
            .read_float_render_target(&setup.output)
            .expect("the image is read back");
        (image, stats)
    };
    let (casting, stats) = draw(&mut setup, true);
    assert_eq!(stats.spot_shadow_draw_calls, 1, "drawn, and discarded");
    let (lit, _) = draw(&mut setup, false);
    assert_same(&casting, &lit, "a cut-out blocker");
}
