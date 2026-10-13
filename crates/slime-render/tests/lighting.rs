//! Lighting, checked by reading pixels back: a surface facing the light is fully lit, one facing
//! away gets the ambient light only, and normals follow the rotation of an instance but not its
//! scale. Also what the renderer keeps from one frame to the next, with the same lit square: the
//! depth target follows the size of the target, the instance buffer grows beyond its first
//! size, and the statistics give the GPU time of a frame. Runs on whatever adapter the machine
//! offers.

use std::time::Duration;

use slime_gfx::{Color, Gpu, Image, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Material, MaterialData, Mesh, MeshData, RenderStats,
    Renderer, Scene,
};

const SIZE: u32 = 16;
const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

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

    /// Draws the square with `transform` under `light`, seen from +Z, and returns the red channel
    /// of the center pixel, sRGB-encoded.
    fn center_red(&mut self, light: DirectionalLight, transform: Mat4) -> u8 {
        self.draw(light, &[transform], (SIZE, SIZE)).0
    }

    /// Draws an instance of the square for each of `transforms` under `light`, seen from +Z, into
    /// a target of `width` × `height` pixels, and returns the red channel of the center pixel,
    /// sRGB-encoded, and the statistics of the frame.
    fn draw(
        &mut self,
        light: DirectionalLight,
        transforms: &[Mat4],
        (width, height): (u32, u32),
    ) -> (u8, RenderStats) {
        let target =
            self.gpu
                .create_render_target("lighting", width, height, TextureFormat::Rgba8UnormSrgb);
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
        let stats = self.renderer.render(
            &self.gpu,
            &target,
            &Scene {
                camera,
                light,
                clear: BLACK,
                instances: &instances,
            },
        );
        let image: Image = self
            .gpu
            .read_render_target(&target)
            .expect("the image is read back");
        let center = ((height / 2 * width + width / 2) * 4) as usize;
        (image.pixels[center], stats)
    }
}

/// A light travelling towards −Z: it comes from the camera's side.
fn from_the_front() -> DirectionalLight {
    DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: Vec3::ONE,
        ambient: Vec3::ZERO,
    }
}

/// The sRGB encoding of a linear value, out of 255.
fn srgb(linear: f32) -> u8 {
    let encoded = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

fn assert_close(actual: u8, expected: u8, case: &str) {
    assert!(
        actual.abs_diff(expected) <= 2,
        "{case}: expected about {expected}, got {actual}"
    );
}

#[test]
fn a_surface_facing_the_light_is_fully_lit_and_one_facing_away_gets_the_ambient_light() {
    let mut setup = Setup::new();
    assert_close(
        setup.center_red(from_the_front(), Mat4::IDENTITY),
        255,
        "facing the light",
    );
    let from_behind = DirectionalLight {
        direction: Vec3::new(0.0, 0.0, 1.0),
        color: Vec3::ONE,
        ambient: Vec3::splat(0.5),
    };
    assert_close(
        setup.center_red(from_behind, Mat4::IDENTITY),
        srgb(0.5),
        "facing away, ambient light only",
    );
}

#[test]
fn normals_follow_the_rotation_of_the_instance_but_not_its_scale() {
    let mut setup = Setup::new();
    // Turned by 60° about Y, the square's normal makes a 60° angle with the light.
    let turned = Mat4::from_rotation_y(60f32.to_radians());
    assert_close(
        setup.center_red(from_the_front(), turned),
        srgb(60f32.to_radians().cos()),
        "turned by 60°",
    );
    // Turned by 45° about Y, then stretched four times along the world's X axis: the surface
    // leans towards the camera, and its normal with it, which a normal transformed by the
    // model matrix would not do (it would lean away).
    let stretched =
        Mat4::from_scale(Vec3::new(4.0, 1.0, 1.0)) * Mat4::from_rotation_y(45f32.to_radians());
    let normal = Vec3::new(1.0 / 4.0, 0.0, 1.0).normalize();
    assert_close(
        setup.center_red(from_the_front(), stretched),
        srgb(normal.z),
        "turned by 45° and stretched along X",
    );
    // Flattened along Z, the axis of its normal, the square is unchanged and still faces the
    // light, although the transform has no inverse.
    let flattened = Mat4::from_scale(Vec3::new(1.0, 1.0, 0.0));
    assert_close(
        setup.center_red(from_the_front(), flattened),
        255,
        "flattened along its normal",
    );
}

#[test]
fn the_depth_target_follows_the_size_of_the_target() {
    let mut setup = Setup::new();
    // Only the height changes, then only the width, then both: a depth target of another size
    // would make the pass panic.
    for size in [(16, 16), (16, 8), (8, 8), (16, 16)] {
        let (red, _) = setup.draw(from_the_front(), &[Mat4::IDENTITY], size);
        assert_close(red, 255, &format!("a {size:?} target"));
    }
}

#[test]
fn instances_beyond_the_first_instance_buffer_are_drawn_where_they_are() {
    let mut setup = Setup::new();
    assert_close(
        setup.center_red(from_the_front(), Mat4::IDENTITY),
        255,
        "one instance",
    );
    // A hundred instances, more than the 64 the instance buffer starts with: all but the last
    // are flattened to a point, so the center is lit only if the last one, in a slot of the grown
    // buffer, gets its own transform.
    let point = Mat4::from_scale(Vec3::ZERO);
    let mut transforms = vec![point; 99];
    transforms.push(Mat4::IDENTITY);
    let (red, stats) = setup.draw(from_the_front(), &transforms, (SIZE, SIZE));
    assert_eq!(stats.draw_calls, 100);
    assert_close(red, 255, "the last of 100 instances");
    // Fewer instances again, in the grown buffer.
    let (red, _) = setup.draw(
        from_the_front(),
        &[point, Mat4::IDENTITY, point],
        (SIZE, SIZE),
    );
    assert_close(red, 255, "the second of 3 instances");
}

#[test]
fn the_statistics_give_the_gpu_time_on_adapters_that_can_time_their_work() {
    let mut setup = Setup::new();
    let mut gpu_time = None;
    for _frame in 0..8 {
        let (_, stats) = setup.draw(from_the_front(), &[Mat4::IDENTITY], (SIZE, SIZE));
        assert_eq!((stats.draw_calls, stats.triangles), (1, 2));
        gpu_time = stats.gpu_time;
        if gpu_time.is_some() {
            break;
        }
        // Waiting is not needed to get a time back, but makes the test deterministic.
        setup.gpu.wait_idle().expect("the GPU finishes the frame");
    }
    if setup.gpu.can_time() {
        let time = gpu_time.expect("the GPU time of a frame comes back within a few frames");
        assert!(time < Duration::from_secs(1), "{time:?}");
    } else {
        assert_eq!(gpu_time, None, "an adapter that cannot time gives no time");
    }
}
