//! Materials, checked by reading pixels back: a base color texture is read at the texture
//! coordinates of the mesh and multiplied with the base color. Runs on whatever adapter the
//! machine offers.

use slime_gfx::{AddressMode, Color, Filter, Gpu, Image, SamplerDescriptor, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, MaterialData, MeshData, Renderer, SampledTexture, Scene,
};

const SIZE: u32 = 16;
const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// The red, green and blue of the pixel at `(x, y)`, `y` counted from the top.
fn pixel(image: &Image, (x, y): (u32, u32)) -> [u8; 3] {
    let start = ((y * image.width + x) * 4) as usize;
    [
        image.pixels[start],
        image.pixels[start + 1],
        image.pixels[start + 2],
    ]
}

#[test]
fn the_base_color_texture_is_read_at_the_texture_coordinates_and_tints_the_base_color() {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let target = gpu.create_render_target("materials", SIZE, SIZE, TextureFormat::Rgba8UnormSrgb);
    let mut renderer = Renderer::new(&gpu, target.format()).expect("the renderer is created");
    // A square facing +Z, two units wide, with the texture's top left at its top left.
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
    // Red and green on the top row, blue and white below.
    let texture = gpu.create_texture(
        "quarters",
        2,
        2,
        TextureFormat::Rgba8UnormSrgb,
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    );
    let sampler = gpu.create_sampler(
        "nearest",
        &SamplerDescriptor {
            mag_filter: Filter::Nearest,
            min_filter: Filter::Nearest,
            address_u: AddressMode::ClampToEdge,
            address_v: AddressMode::ClampToEdge,
        },
    );
    let sampled = SampledTexture {
        texture: &texture,
        sampler: &sampler,
    };
    let plain = renderer.create_material(
        &gpu,
        "textured",
        &MaterialData {
            base_color_texture: Some(sampled),
            ..MaterialData::default()
        },
    );
    let dimmed = renderer.create_material(
        &gpu,
        "textured and dimmed",
        &MaterialData {
            base_color: [0.5, 0.5, 0.5, 1.0],
            base_color_texture: Some(sampled),
        },
    );
    // Lit straight on, without ambient light, so that the light changes nothing.
    let light = DirectionalLight {
        direction: Vec3::new(0.0, 0.0, -1.0),
        color: Vec3::ONE,
        ambient: Vec3::ZERO,
    };
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 3.0),
        ..Camera::default()
    };

    // The pixels at 5 and 10 are inside the square's quarters, away from the texture's edges.
    for (material, expected, case) in [
        (
            &plain,
            [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]],
            "white base color",
        ),
        (
            &dimmed,
            [[188, 0, 0], [0, 188, 0], [0, 0, 188], [188, 188, 188]],
            "half base color",
        ),
    ] {
        let instances = [Instance {
            mesh: &square,
            material,
            transform: Mat4::IDENTITY,
        }];
        renderer.render(
            &gpu,
            &target,
            &Scene {
                camera,
                light,
                clear: BLACK,
                instances: &instances,
            },
        );
        let image = gpu
            .read_render_target(&target)
            .expect("the image is read back");
        for (position, expected) in [(5, 5), (10, 5), (5, 10), (10, 10)]
            .into_iter()
            .zip(expected)
        {
            let actual = pixel(&image, position);
            let close = actual
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 1);
            assert!(
                close,
                "{case} at {position:?}: expected {expected:?}, got {actual:?}"
            );
        }
    }
}
