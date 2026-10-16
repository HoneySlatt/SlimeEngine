//! Positions in `f64` (ADR-0079), drawn relative to the camera (ADR-0048): a scene 10 000 km from
//! the origin is drawn exactly as the same scene at the origin. Runs on whatever adapter the
//! machine offers.
//!
//! The scene is small and seen from close, where a millimetre is a pixel: a floor a metre wide, a
//! square leaning over it, lit by the sun without shadows and by a bulb a few centimetres above
//! the floor, seen from 30 cm away. Every position is a multiple of 2⁻¹⁰ m, which a 64-bit float
//! holds exactly up to 2⁴³ m, about 9 × 10⁹ km, from the origin: moved 10 000 km away, the scene
//! is exactly the same scene, and only how the renderer computes can tell them apart.

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, TextureFormat};
use slime_render::glam::{DAffine3, DQuat, DVec3, Vec3};
use slime_render::{
    Camera, DirectionalLight, Instance, Light, MaterialData, MeshData, PointLight, Renderer, Scene,
};

const SIZE: u32 = 64;

/// Draws the scene moved by `offset` and reads the output back.
fn draw(offset: DVec3) -> FloatImage {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    let output = gpu
        .create_render_target("precision", SIZE, SIZE, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let square = renderer
        .create_mesh(&gpu, "square", &MeshData::square())
        .expect("the square is valid");
    let white = renderer
        .create_material(&gpu, "white", &MaterialData::default())
        .expect("the GPU has memory for it");
    let glossy = renderer
        .create_material(
            &gpu,
            "glossy",
            &MaterialData {
                base_color: LinearColor::rgb(0.8, 0.3, 0.1),
                roughness: 0.3,
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");
    let moved = DAffine3::from_translation(offset);
    // The floor in the plane y = 0, a metre wide, and the square leaning over it, 6 cm wide, 2 cm
    // up.
    let floor = DAffine3::from_rotation_x(-std::f64::consts::FRAC_PI_2);
    let leaning = DAffine3::from_scale_rotation_translation(
        DVec3::splat(0.06),
        DQuat::from_euler(slime_render::glam::EulerRot::XYZ, -1.1, 0.4, 0.2),
        DVec3::new(0.015_625, 0.031_25, -0.007_812_5),
    );
    let instances = [
        Instance::new(&square, &white, moved * floor),
        Instance::new(&square, &glossy, moved * leaning),
    ];
    let lights = [
        // Without shadows: the cascades' texels lie on a grid fixed in the world (ADR-0050),
        // which the moved scene meets elsewhere across a texel. `shadows.rs` checks that surfaces
        // far from the origin do not shadow themselves.
        Light::Directional(DirectionalLight {
            direction: Vec3::new(-0.3, -1.0, -0.2),
            illuminance: 3.0,
            casts_shadows: false,
            ..DirectionalLight::default()
        }),
        Light::Point(PointLight {
            position: offset + DVec3::new(-0.039_062_5, 0.023_437_5, 0.031_25),
            color: LinearColor::rgb(1.0, 0.8, 0.6),
            intensity: 0.002,
            range: Some(0.25),
        }),
    ];
    let mut scene = Scene::new(
        Camera {
            eye: offset + DVec3::new(0.125, 0.25, 0.25),
            target: offset + DVec3::new(0.0, 0.015_625, 0.0),
            near: 0.001,
            ..Camera::default()
        },
        &instances,
    );
    scene.lights = &lights;
    renderer
        .render(&gpu, &output, &scene)
        .expect("the frame is drawn");
    gpu.read_float_render_target(&output)
        .expect("the image is read back")
}

#[test]
fn a_scene_10_000_km_from_the_origin_is_drawn_exactly_as_at_the_origin() {
    let at_origin = draw(DVec3::ZERO);
    // Enough pixels lit, by the sun and by the bulb, that the comparison means something.
    let lit = at_origin
        .pixels
        .chunks(4)
        .filter(|pixel| pixel[0] > 0.05)
        .count();
    assert!(lit > (SIZE * SIZE / 2) as usize, "{lit} pixels lit");
    let far = draw(DVec3::new(1.0e7, 0.0, -1.0e7));
    let differences: Vec<(usize, f32, f32)> = at_origin
        .pixels
        .iter()
        .zip(&far.pixels)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(index, (&a, &b))| (index / 4, a, b))
        .collect();
    let largest = differences
        .iter()
        .map(|&(_, a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(
        differences.is_empty(),
        "{} channels differ, by up to {largest}; the first: {:?}",
        differences.len(),
        &differences[..differences.len().min(5)]
    );
}
