//! Multisample anti-aliasing (ADR-0056, ADR-0060), checked by reading back `Rgba16Float` outputs:
//! an emissive square on the black of a scene without an environment, whose left edge lies a
//! quarter of the way across a column of pixels. Of the four samples of each pixel of that column,
//! at an eighth, three eighths, five eighths and seven eighths of its width (the standard pattern),
//! three see the square, of light E, and one the black: the resolve weighs each by 1 / (1 + its
//! brightest channel), so that the column shows 3E / (3 + (1 + E)) = 3E / (4 + E), where a plain
//! average would show 3E / 4. Runs on whatever adapter the machine offers.

#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{Camera, Instance, MaterialData, MeshData, Renderer, Scene};

/// The output's side, in pixels; the square's left edge crosses its column `EDGE_COLUMN`.
const SIZE: u32 = 8;
const EDGE_COLUMN: u32 = 4;

/// The camera: 10 m in front of the plane z = 0, with a 60° field of view.
const DISTANCE: f32 = 10.0;
const FOV_Y: f32 = std::f32::consts::FRAC_PI_3;

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, DISTANCE),
        target: Vec3::ZERO,
        fov_y: FOV_Y,
        near: 1.0,
        ..Camera::default()
    }
}

/// Draws the square, of light `emission`, from a quarter of the way across column `EDGE_COLUMN` to
/// beyond the output's right edge, and beyond its top and bottom, at an exposure of 1 and without
/// bloom.
fn draw(emission: f32) -> FloatImage {
    let gpu =
        Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    let output = gpu
        .create_render_target("anti-aliasing", SIZE, SIZE, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let square = renderer
        .create_mesh(&gpu, "square", &MeshData::square())
        .expect("the square is valid");
    let emitter = renderer
        .create_material(
            &gpu,
            "emitter",
            &MaterialData {
                base_color: LinearColor::BLACK,
                emissive: LinearColor::WHITE,
                emissive_strength: emission,
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");
    // Where a column of pixels lies on the plane z = 0, which the output shows 2 × half wide.
    let half = DISTANCE * (FOV_Y / 2.0).tan();
    let x = |column: f32| (column / SIZE as f32 * 2.0 - 1.0) * half;
    let (left, right) = (x(EDGE_COLUMN as f32 + 0.25), x(SIZE as f32 + 1.0));
    let transform = Mat4::from_translation(Vec3::new((left + right) / 2.0, 0.0, 0.0))
        * Mat4::from_scale(Vec3::new(right - left, 3.0 * half, 1.0));
    let instances = [Instance::new(&square, &emitter, transform)];
    let scene = Scene::new(camera(), &instances);
    renderer
        .render(&gpu, &output, &scene)
        .expect("the frame is drawn");
    gpu.read_float_render_target(&output)
        .expect("the image is read back")
}

#[test]
fn an_edge_shows_its_samples_weighed_by_their_brightness() {
    for emission in [0.5, 4.0] {
        let image = draw(emission);
        let shows = |light: f32| tone_curve::tone_map([light; 3])[0];
        let weighted = 3.0 * emission / (4.0 + emission);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let expected = match x.cmp(&EDGE_COLUMN) {
                    std::cmp::Ordering::Less => shows(0.0),
                    std::cmp::Ordering::Equal => shows(weighted),
                    std::cmp::Ordering::Greater => shows(emission),
                };
                let start = ((y * SIZE + x) * 4) as usize;
                for &channel in &image.pixels[start..start + 3] {
                    assert!(
                        (channel - expected).abs() < 2e-3,
                        "light {emission}, pixel ({x}, {y}): {channel}, expected {expected}"
                    );
                }
            }
        }
        // A plain average would show the edge brighter, by more than the tolerance.
        assert!(shows(0.75 * emission) - shows(weighted) > 1e-2);
    }
}
