//! The cubes that the `scene` example and its image test draw: a unit cube mesh, two materials,
//! three instances that overlap from most viewpoints, and a camera that orbits them with time.

use slime_gfx::{Color, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::{Mat4, Quat, Vec3};
use slime_render::{
    Camera, Instance, Material, MaterialData, Mesh, MeshData, RenderError, RenderStats, Renderer,
    Scene,
};

/// The color behind the cubes.
pub const BACKGROUND: Color = Color {
    r: 0.05,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

/// How far the camera orbits from the origin, and how high above it.
const ORBIT_RADIUS: f32 = 4.0;
const ORBIT_HEIGHT: f32 = 1.8;
/// How fast the camera orbits, in radians per second.
const ORBIT_SPEED: f32 = 0.5;

/// The renderer and the GPU objects of the cubes, for targets of one format.
pub struct Cubes {
    renderer: Renderer,
    cube: Mesh,
    orange: Material,
    blue: Material,
}

impl Cubes {
    /// Creates the renderer, the cube mesh and the materials, for targets of `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let renderer = Renderer::new(gpu, format)?;
        let cube = renderer.create_mesh(gpu, "cube", &cube())?;
        let orange = renderer.create_material(
            gpu,
            "orange",
            &MaterialData {
                base_color: [0.9, 0.45, 0.1, 1.0],
            },
        );
        let blue = renderer.create_material(
            gpu,
            "blue",
            &MaterialData {
                base_color: [0.15, 0.4, 0.9, 1.0],
            },
        );
        Ok(Self {
            renderer,
            cube,
            orange,
            blue,
        })
    }

    /// Draws the cubes into `target` as the camera sees them `time` seconds after the start: it
    /// has orbited them by then.
    pub fn render(&mut self, gpu: &Gpu, target: &RenderTarget, time: f32) -> RenderStats {
        let angle = time * ORBIT_SPEED;
        let camera = Camera {
            eye: Vec3::new(
                ORBIT_RADIUS * angle.sin(),
                ORBIT_HEIGHT,
                ORBIT_RADIUS * angle.cos(),
            ),
            target: Vec3::ZERO,
            ..Camera::default()
        };
        let instances = [
            Instance {
                mesh: &self.cube,
                material: &self.orange,
                transform: Mat4::IDENTITY,
            },
            Instance {
                mesh: &self.cube,
                material: &self.blue,
                transform: Mat4::from_rotation_translation(
                    Quat::from_rotation_y(35f32.to_radians()),
                    Vec3::new(1.2, 0.3, -0.6),
                ),
            },
            Instance {
                mesh: &self.cube,
                material: &self.orange,
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.6),
                    Quat::from_rotation_x(20f32.to_radians()),
                    Vec3::new(-1.3, -0.2, 0.4),
                ),
            },
        ];
        self.renderer.render(
            gpu,
            target,
            &Scene {
                camera,
                clear: BACKGROUND,
                instances: &instances,
            },
        )
    }
}

/// A cube of side 1 centred on the origin: four vertices per face, so that each face has its own
/// normal and texture coordinates.
pub fn cube() -> MeshData {
    // Each face: its normal, then its four corners counter-clockwise as seen from outside.
    const FACES: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [0.0, 0.0, 1.0],
            [
                [-1.0, -1.0, 1.0],
                [1.0, -1.0, 1.0],
                [1.0, 1.0, 1.0],
                [-1.0, 1.0, 1.0],
            ],
        ),
        (
            [0.0, 0.0, -1.0],
            [
                [1.0, -1.0, -1.0],
                [-1.0, -1.0, -1.0],
                [-1.0, 1.0, -1.0],
                [1.0, 1.0, -1.0],
            ],
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [1.0, -1.0, 1.0],
                [1.0, -1.0, -1.0],
                [1.0, 1.0, -1.0],
                [1.0, 1.0, 1.0],
            ],
        ),
        (
            [-1.0, 0.0, 0.0],
            [
                [-1.0, -1.0, -1.0],
                [-1.0, -1.0, 1.0],
                [-1.0, 1.0, 1.0],
                [-1.0, 1.0, -1.0],
            ],
        ),
        (
            [0.0, 1.0, 0.0],
            [
                [-1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, -1.0],
                [-1.0, 1.0, -1.0],
            ],
        ),
        (
            [0.0, -1.0, 0.0],
            [
                [-1.0, -1.0, -1.0],
                [1.0, -1.0, -1.0],
                [1.0, -1.0, 1.0],
                [-1.0, -1.0, 1.0],
            ],
        ),
    ];
    // The corners of a face from its bottom left, counter-clockwise; (0, 0) is the top left of
    // the texture.
    const UVS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    let mut data = MeshData::default();
    for (face, (normal, corners)) in FACES.iter().enumerate() {
        let first = u32::try_from(face * 4).expect("a cube has 24 vertices");
        for (corner, uv) in corners.iter().zip(UVS) {
            data.positions
                .push(corner.map(|coordinate| coordinate * 0.5));
            data.normals.push(*normal);
            data.uvs.push(uv);
        }
        data.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    data
}
