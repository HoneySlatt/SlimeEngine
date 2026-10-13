//! The shapes that the image test `tests/scene.rs` draws (the `scene` example draws a glTF scene
//! instead): a unit cube and a smooth sphere, four materials, two of them with a checker texture,
//! four instances that overlap from most viewpoints, and in front of them six small spheres, a
//! row of dielectrics and a row of metals of increasing roughness (ADR-0035); a directional light,
//! and a camera that orbits them with time.

use std::f32::consts::{PI, TAU};

use slime_gfx::{Color, Gpu, RenderTarget, SamplerDescriptor, TextureFormat};
use slime_render::glam::{Mat4, Quat, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, Material, MaterialData, Mesh, MeshData, RenderError,
    RenderStats, Renderer, SampledTexture, Scene,
};

/// The color behind the shapes.
pub const BACKGROUND: Color = Color {
    r: 0.05,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

/// A slightly warm light of π lux from above and from the side the camera starts on, under which a
/// white matte surface facing it reflects about 0.97 nits (ADR-0035), shown as they are at an
/// exposure of 1. Light in these units keeps the image of this test as it was before lights had
/// physical units: it checks the shapes and their materials, not the units.
const LIGHT: Light = Light::Directional(DirectionalLight {
    direction: Vec3::new(-0.6, -1.0, -0.5),
    color: Vec3::new(1.0, 0.98, 0.9),
    illuminance: PI,
});
/// A slightly cold ambient light, in nits.
const AMBIENT: Vec3 = Vec3::new(0.15, 0.16, 0.2);

/// How far the camera orbits from the origin, and how high above it.
const ORBIT_RADIUS: f32 = 4.0;
const ORBIT_HEIGHT: f32 = 1.8;
/// How fast the camera orbits, in radians per second.
const ORBIT_SPEED: f32 = 0.5;

/// The color of the small spheres, and the roughness of each column of them.
const SPHERE_COLOR: [f32; 4] = [0.9, 0.6, 0.3, 1.0];
const SPHERE_ROUGHNESS: [f32; 3] = [0.3, 0.55, 0.8];

/// The side of the checker texture, in pixels, and of its squares.
const CHECKER_SIZE: u32 = 64;
const CHECKER_SQUARE: u32 = 8;

/// The renderer and the GPU objects of the shapes, for targets of one format.
pub struct Shapes {
    renderer: Renderer,
    cube: Mesh,
    sphere: Mesh,
    orange: Material,
    blue: Material,
    checkered: Material,
    checkered_green: Material,
    /// The small spheres' materials: a row of dielectrics, then a row of metals, each from the
    /// smoothest to the roughest.
    spheres: [[Material; 3]; 2],
}

impl Shapes {
    /// Creates the renderer, the meshes, the checker texture and the materials, for targets of
    /// `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let renderer = Renderer::new(gpu, format)?;
        let cube = renderer.create_mesh(gpu, "cube", &cube())?;
        let sphere = renderer.create_mesh(gpu, "sphere", &sphere(0.5, 16, 24))?;
        let checker = gpu.create_texture(
            "checker",
            CHECKER_SIZE,
            CHECKER_SIZE,
            TextureFormat::Rgba8UnormSrgb,
            &checker(),
        );
        let sampler = gpu.create_sampler("checker", &SamplerDescriptor::default());
        let checkered_by = Some(SampledTexture {
            texture: &checker,
            sampler: &sampler,
        });
        let material = |label: &str, base_color: [f32; 4], base_color_texture| {
            renderer.create_material(
                gpu,
                label,
                &MaterialData {
                    base_color,
                    base_color_texture,
                    ..MaterialData::default()
                },
            )
        };
        let orange = material("orange", [0.9, 0.45, 0.1, 1.0], None);
        let blue = material("blue", [0.15, 0.4, 0.9, 1.0], None);
        let checkered = material("checkered", [1.0, 1.0, 1.0, 1.0], checkered_by);
        let checkered_green = material("checkered green", [0.45, 0.9, 0.55, 1.0], checkered_by);
        let spheres = [0.0, 1.0].map(|metallic| {
            SPHERE_ROUGHNESS.map(|roughness| {
                renderer.create_material(
                    gpu,
                    &format!("sphere, metallic {metallic}, roughness {roughness}"),
                    &MaterialData {
                        base_color: SPHERE_COLOR,
                        metallic,
                        roughness,
                        ..MaterialData::default()
                    },
                )
            })
        });
        Ok(Self {
            renderer,
            cube,
            sphere,
            orange,
            blue,
            checkered,
            checkered_green,
            spheres,
        })
    }

    /// Draws the shapes into `target` as the camera sees them `time` seconds after the start: it
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
        let mut instances = vec![
            Instance {
                mesh: &self.cube,
                material: &self.checkered,
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
            // The sphere sits on the first cube.
            Instance {
                mesh: &self.sphere,
                material: &self.checkered_green,
                transform: Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            },
        ];
        // In front of the cubes, two rows of three, the metals nearer.
        for (row, materials) in self.spheres.iter().enumerate() {
            for (column, material) in materials.iter().enumerate() {
                instances.push(Instance {
                    mesh: &self.sphere,
                    material,
                    transform: Mat4::from_scale_rotation_translation(
                        Vec3::splat(0.4),
                        Quat::IDENTITY,
                        Vec3::new(0.6 * column as f32 - 0.6, -0.6, 1.2 + 0.6 * row as f32),
                    ),
                });
            }
        }
        self.renderer.render(
            gpu,
            target,
            &Scene {
                camera,
                lights: &[LIGHT],
                ambient: AMBIENT,
                clear: BACKGROUND,
                exposure: 1.0,
                instances: &instances,
            },
        )
    }
}

/// A cube of side 1 centred on the origin: four vertices per face, so that each face has its own
/// normal, texture coordinates and tangent.
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
        // u grows from the first corner to the second, and the fourth is up the texture's image,
        // where the bitangent cross(normal, tangent) points on every face: the handedness is 1.
        let tangent = (Vec3::from(corners[1]) - Vec3::from(corners[0])).normalize();
        for (corner, uv) in corners.iter().zip(UVS) {
            data.positions
                .push(corner.map(|coordinate| coordinate * 0.5));
            data.normals.push(*normal);
            data.uvs.push(uv);
            data.tangents.push([tangent.x, tangent.y, tangent.z, 1.0]);
        }
        data.indices
            .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    data
}

/// A sphere of radius `radius` centred on the origin, smooth: the normal of each vertex points
/// away from the centre. `rings` bands go from the top pole to the bottom one and `segments`
/// go around; the texture wraps around it once, its top row at the top pole. It is mirrored on the
/// surface: u grows with the azimuth, and cross(normal, tangent) points down the image, so the
/// handedness is −1.
pub fn sphere(radius: f32, rings: u32, segments: u32) -> MeshData {
    let mut data = MeshData::default();
    for ring in 0..=rings {
        let polar = PI * ring as f32 / rings as f32;
        let (y, ring_radius) = (polar.cos(), polar.sin());
        for segment in 0..=segments {
            let azimuth = TAU * segment as f32 / segments as f32;
            let normal = [ring_radius * azimuth.cos(), y, ring_radius * azimuth.sin()];
            data.positions
                .push(normal.map(|coordinate| coordinate * radius));
            data.normals.push(normal);
            data.uvs
                .push([segment as f32 / segments as f32, ring as f32 / rings as f32]);
            data.tangents
                .push([-azimuth.sin(), 0.0, azimuth.cos(), -1.0]);
        }
    }
    // Each ring is a row of `segments + 1` vertices, the last one repeating the first with the
    // texture's right edge. Between two rows, each quad gives two triangles wound
    // counter-clockwise from outside, except at the poles, where one of them has no area.
    let row = segments + 1;
    for ring in 0..rings {
        for segment in 0..segments {
            let a = ring * row + segment;
            let b = a + row;
            let c = b + 1;
            let d = a + 1;
            if ring != 0 {
                data.indices.extend([a, d, c]);
            }
            if ring != rings - 1 {
                data.indices.extend([a, c, b]);
            }
        }
    }
    data
}

/// A checker of light and dark squares, as sRGB-encoded pixels, rows from top to bottom.
fn checker() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((CHECKER_SIZE * CHECKER_SIZE * 4) as usize);
    for y in 0..CHECKER_SIZE {
        for x in 0..CHECKER_SIZE {
            let light = (x / CHECKER_SQUARE + y / CHECKER_SQUARE).is_multiple_of(2);
            pixels.extend(if light {
                [235, 225, 205, 255]
            } else {
                [95, 75, 55, 255]
            });
        }
    }
    pixels
}
