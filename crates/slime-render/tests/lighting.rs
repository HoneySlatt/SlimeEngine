//! Lighting, checked by reading pixels back through the tone-mapping curve of the renderer
//! (ADR-0034), on a white matte surface (ADR-0035): one facing the light is fully lit, one facing
//! away gets the light of a uniform sky only, normals follow the rotation of an instance but not
//! its scale, and the exposure multiplies the light, which surfaces reflect above 1 without
//! clipping, before the high-dynamic-range target is written (ADR-0037). Also what the renderer
//! keeps from one frame to the next, with the same lit square: the targets follow the size of the
//! output, the projection its aspect ratio, the instance buffer grows beyond its first size, and
//! the statistics give the GPU time of each pass, the shadow and sky passes' included. Runs on
//! whatever adapter the machine offers.

#[path = "common/tone_curve.rs"]
mod tone_curve;

use std::f32::consts::PI;
use std::time::Duration;

use slime_gfx::{Gpu, Image, TextureFormat};
use slime_render::glam::{Mat4, Vec3, Vec4};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, Material, MaterialData, Mesh, MeshData, RenderStats,
    Renderer, Scene, Sky,
};

use tone_curve::displayed;

const SIZE: u32 = 16;

/// The renderer and a white square facing +Z, two units wide, centred on the origin.
struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    square: Mesh,
    white: Material,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
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
        let white = renderer.create_material(&gpu, "white", &MaterialData::default());
        Self {
            gpu,
            renderer,
            square,
            white,
        }
    }

    /// Draws the square with `transform` under `light` and a uniform sky of radiance `sky`,
    /// seen from +Z, at an exposure of 1, and returns the red channel of the center pixel,
    /// sRGB-encoded.
    fn center_red(&mut self, light: DirectionalLight, sky: f32, transform: Mat4) -> u8 {
        self.exposed_center_red(light, sky, transform, 1.0)
    }

    /// The same, at `exposure`.
    fn exposed_center_red(
        &mut self,
        light: DirectionalLight,
        sky: f32,
        transform: Mat4,
        exposure: f32,
    ) -> u8 {
        let (image, _) = self.draw(light, sky, &[transform], (SIZE, SIZE), exposure);
        let center = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
        image.pixels[center]
    }

    /// Draws an instance of the square for each of `transforms` under `light` and a uniform sky of
    /// radiance `sky` (none at 0), seen from +Z, into a target of `width` × `height` pixels at
    /// `exposure`, and returns the image, sRGB-encoded, and the statistics of the frame.
    fn draw(
        &mut self,
        light: DirectionalLight,
        sky: f32,
        transforms: &[Mat4],
        (width, height): (u32, u32),
        exposure: f32,
    ) -> (Image, RenderStats) {
        let target = self
            .gpu
            .create_render_target("lighting", width, height, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let camera = Camera {
            eye: Vec3::new(0.0, 0.0, 3.0),
            ..Camera::default()
        };
        let instances: Vec<Instance<'_>> = transforms
            .iter()
            .map(|&transform| Instance {
                mesh: &self.square,
                material: &self.white,
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
        let stats = self.renderer.render(
            &self.gpu,
            &target,
            &Scene {
                camera,
                lights: &[Light::Directional(light)],
                environment: environment.as_ref(),
                exposure,
                bloom: 0.0,
                instances: &instances,
            },
        );
        let image = self
            .gpu
            .read_render_target(&target)
            .expect("the image is read back");
        (image, stats)
    }

    /// The red channel of the center pixel of a frame drawn by [`draw`](Self::draw), at an
    /// exposure of 1.
    fn center_of(&mut self, transforms: &[Mat4], (width, height): (u32, u32)) -> (u8, RenderStats) {
        let (image, stats) = self.draw(from_the_front(), 0.0, transforms, (width, height), 1.0);
        let center = ((height / 2 * width + width / 2) * 4) as usize;
        (image.pixels[center], stats)
    }
}

/// What the white matte square reflects towards the camera under a light of π that comes from the
/// camera's side, where its normal makes an angle of cosine `cosine` with both: 0.96 of the light
/// it diffuses, Lambert's cosine times what its specular layer lets through, and 0.01 from that
/// layer, the same at every angle for a roughness of 1 seen from the light (ADR-0035).
fn lit(cosine: f32) -> f32 {
    0.96 * cosine + 0.01
}

/// What the center of the square shows when it faces the light fully, at an exposure of 1.
fn fully_lit() -> u8 {
    displayed([lit(1.0); 3])[0]
}

/// A white light of π lux travelling towards −Z: it comes from the camera's side.
fn from_the_front() -> DirectionalLight {
    DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: Vec3::ONE,
        illuminance: PI,
        casts_shadows: true,
    }
}

fn assert_close(actual: u8, expected: u8, case: &str) {
    assert!(
        actual.abs_diff(expected) <= 2,
        "{case}: expected about {expected}, got {actual}"
    );
}

#[test]
fn a_surface_facing_the_light_is_fully_lit_and_one_facing_away_gets_the_sky_s_light() {
    let mut setup = Setup::new();
    assert_close(
        setup.center_red(from_the_front(), 0.0, Mat4::IDENTITY),
        fully_lit(),
        "facing the light",
    );
    let from_behind = DirectionalLight {
        direction: Vec3::new(0.0, 0.0, 1.0),
        ..from_the_front()
    };
    assert_close(
        setup.center_red(from_behind, 0.5, Mat4::IDENTITY),
        displayed([0.5; 3])[0],
        "facing away, the sky's light only",
    );
}

#[test]
fn normals_follow_the_rotation_of_the_instance_but_not_its_scale() {
    let mut setup = Setup::new();
    // Turned by 60° about Y, the square's normal makes a 60° angle with the light.
    let turned = Mat4::from_rotation_y(60f32.to_radians());
    assert_close(
        setup.center_red(from_the_front(), 0.0, turned),
        displayed([lit(60f32.to_radians().cos()); 3])[0],
        "turned by 60°",
    );
    // Turned by 45° about Y, then stretched four times along the world's X axis: the surface
    // leans towards the camera, and its normal with it, which a normal transformed by the
    // model matrix would not do (it would lean away).
    let stretched =
        Mat4::from_scale(Vec3::new(4.0, 1.0, 1.0)) * Mat4::from_rotation_y(45f32.to_radians());
    let normal = Vec3::new(1.0 / 4.0, 0.0, 1.0).normalize();
    assert_close(
        setup.center_red(from_the_front(), 0.0, stretched),
        displayed([lit(normal.z); 3])[0],
        "turned by 45° and stretched along X",
    );
    // Flattened along Z, the axis of its normal, the square is unchanged and still faces the
    // light, although the transform has no inverse.
    let flattened = Mat4::from_scale(Vec3::new(1.0, 1.0, 0.0));
    assert_close(
        setup.center_red(from_the_front(), 0.0, flattened),
        fully_lit(),
        "flattened along its normal",
    );
}

#[test]
fn the_exposure_multiplies_the_light_that_surfaces_reflect() {
    let mut setup = Setup::new();
    let half = DirectionalLight {
        illuminance: PI / 2.0,
        ..from_the_front()
    };
    // Both reflect the same before the curve: scaling by a power of two is exact in f32 and f16.
    assert_eq!(
        setup.exposed_center_red(half, 0.0, Mat4::IDENTITY, 2.0),
        setup.exposed_center_red(from_the_front(), 0.0, Mat4::IDENTITY, 1.0),
        "a light of π/2 at an exposure of 2 shows as a light of π at an exposure of 1"
    );
    assert_close(
        setup.exposed_center_red(from_the_front(), 0.0, Mat4::IDENTITY, 0.5),
        displayed([lit(1.0) / 2.0; 3])[0],
        "a light of π at an exposure of 0.5",
    );
    // Light above 1 reaches the curve unclipped: a light of 4π shows compressed rather than as a
    // reflected light of 1, and brought back by an exposure of 1/4, as a light of π.
    let bright = DirectionalLight {
        illuminance: 4.0 * PI,
        ..from_the_front()
    };
    assert_close(
        setup.center_red(bright, 0.0, Mat4::IDENTITY),
        displayed([4.0 * lit(1.0); 3])[0],
        "a light of 4π",
    );
    assert_eq!(
        setup.exposed_center_red(bright, 0.0, Mat4::IDENTITY, 0.25),
        setup.exposed_center_red(from_the_front(), 0.0, Mat4::IDENTITY, 1.0),
        "a light of 4π at an exposure of 1/4 shows as a light of π at an exposure of 1"
    );
    // The exposure is applied before the high-dynamic-range target is written: under a light of
    // 10⁸ π the square reflects about 97 000 000, far beyond the target's 64 × 65 504, and still
    // shows at an exposure of 10⁻⁸ as under a light of π at 1. Exposed after the target, it would
    // show the clamped light, 64 × 65 504 × 10⁻⁸, about 0.04.
    let blinding = DirectionalLight {
        illuminance: 1e8 * PI,
        ..from_the_front()
    };
    assert_close(
        setup.exposed_center_red(blinding, 0.0, Mat4::IDENTITY, 1e-8),
        fully_lit(),
        "a light of 10⁸ π at an exposure of 10⁻⁸",
    );
}

#[test]
fn the_targets_follow_the_size_of_the_output() {
    let mut setup = Setup::new();
    // Only the height changes, then only the width, then both. Each frame must be the one that a
    // renderer drawing first at that size draws: a high-dynamic-range target kept from another
    // size would frame the square differently, and a depth target of another size would make the
    // pass panic.
    for size in [(16, 16), (16, 8), (8, 8), (16, 16)] {
        let (image, _) = setup.draw(from_the_front(), 0.0, &[Mat4::IDENTITY], size, 1.0);
        let (expected, _) = Setup::new().draw(from_the_front(), 0.0, &[Mat4::IDENTITY], size, 1.0);
        assert!(image.pixels == expected.pixels, "a {size:?} target");
    }
}

#[test]
fn the_projection_follows_the_aspect_ratio_of_the_output() {
    let mut setup = Setup::new();
    // The square, two units wide at three units from the camera, covers as many pixels across as
    // down in a target twice as wide as it is high: a projection that ignored the aspect ratio
    // would stretch it.
    let (width, height) = (64, 32);
    let (image, _) = setup.draw(
        from_the_front(),
        0.0,
        &[Mat4::IDENTITY],
        (width, height),
        1.0,
    );
    let lit = |x: u32, y: u32| image.pixels[((y * width + x) * 4) as usize] > 0;
    let across = (0..width).filter(|&x| lit(x, height / 2)).count();
    let down = (0..height).filter(|&y| lit(width / 2, y)).count();
    assert!(
        across > 0 && across.abs_diff(down) <= 1,
        "{across} across, {down} down"
    );
}

#[test]
fn instances_beyond_the_first_instance_buffer_are_drawn_where_they_are() {
    let mut setup = Setup::new();
    assert_close(
        setup.center_red(from_the_front(), 0.0, Mat4::IDENTITY),
        fully_lit(),
        "one instance",
    );
    // A hundred instances, more than the 64 the instance buffer starts with: all but the last
    // are flattened to a point, so the center is lit only if the last one, in a slot of the grown
    // buffer, gets its own transform. Built column by column: `Mat4::from_scale` asserts, when
    // glam's assertions are on, that the scale is not zero.
    let point = Mat4::from_cols(Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::W);
    let mut transforms = vec![point; 99];
    transforms.push(Mat4::IDENTITY);
    let (red, stats) = setup.center_of(&transforms, (SIZE, SIZE));
    assert_eq!(stats.draw_calls, 100);
    assert_close(red, fully_lit(), "the last of 100 instances");
    // Fewer instances again, in the grown buffer.
    let (red, _) = setup.center_of(&[point, Mat4::IDENTITY, point], (SIZE, SIZE));
    assert_close(red, fully_lit(), "the second of 3 instances");
}

#[test]
fn the_statistics_give_the_gpu_time_of_each_pass_on_adapters_that_can_time_their_work() {
    let mut setup = Setup::new();
    let mut times = (None, None, None, None);
    for _frame in 0..8 {
        let (_, stats) = setup.center_of(&[Mat4::IDENTITY], (SIZE, SIZE));
        // The scene's directional light casts shadows: the shadow pass draws the instance too.
        assert_eq!(
            (stats.draw_calls, stats.triangles, stats.shadow_draw_calls),
            (1, 2, 1)
        );
        times = (
            times.0.or(stats.scene_gpu_time),
            times.1.or(stats.tone_mapping_gpu_time),
            times.2.or(stats.shadow_gpu_time),
            times.3.or(stats.sky_gpu_time),
        );
        if times.0.is_some() && times.1.is_some() && times.2.is_some() && times.3.is_some() {
            break;
        }
        // Waiting is not needed to get the times back, but makes the test deterministic.
        setup.gpu.wait_idle().expect("the GPU finishes the frame");
    }
    eprintln!(
        "adapter `{}`: scene, tone-mapping, shadow and sky passes timed {times:?}",
        setup.gpu.adapter_name()
    );
    for time in [times.0, times.1, times.2, times.3] {
        if setup.gpu.can_time() {
            let time = time.expect("the GPU time of each pass comes back within a few frames");
            assert!(time < Duration::from_secs(1), "{time:?}");
        } else {
            assert_eq!(time, None, "an adapter that cannot time gives no time");
        }
    }
}
