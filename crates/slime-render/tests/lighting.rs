//! Lighting, checked by reading pixels back: a surface facing the light is fully lit, one facing
//! away gets the ambient light only, and normals follow the rotation of an instance but not its
//! scale. Runs on whatever adapter the machine offers.

use slime_gfx::{Color, Gpu, Image, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Material, MaterialData, Mesh, MeshData, Renderer, Scene,
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
        let target =
            self.gpu
                .create_render_target("lighting", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
        let camera = Camera {
            eye: Vec3::new(0.0, 0.0, 3.0),
            ..Camera::default()
        };
        let instances = [Instance {
            mesh: &self.square,
            material: &self.white,
            transform,
        }];
        self.renderer.render(
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
        let center = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
        image.pixels[center]
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
}
