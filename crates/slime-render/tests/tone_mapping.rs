//! The scene drawn in high dynamic range and mapped to the output by the tone-mapping pass
//! (ADR-0034), checked by reading back an `Rgba16Float` output. Known light reaches the
//! tone-mapping pass as the uniform sky of a scene without instances, which the sky pass exposes
//! (ADR-0037, ADR-0039), and must come out as Khronos PBR Neutral gives it. Every input is exact in
//! half precision. Runs on whatever adapter the machine offers.

#[path = "common/tone_curve.rs"]
mod tone_curve;

use slime_gfx::{FloatImage, Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::Vec3;
use slime_render::{Camera, DirectionalLight, Light, RenderStats, Renderer, Scene, Sky};

/// Light, and what Khronos PBR Neutral makes of it, each derived by hand from the specification's
/// equations (K_s = 0.76 where compression starts, K_d = 0.15, F90 = 0.04).
const CURVE: [([f32; 3], [f32; 3]); 9] = [
    // Black stays black: no offset at 0.
    ([0.0; 3], [0.0; 3]),
    // The toe: below 0.08, the offset is x − 6.25 x², so 0.0625 gives 6.25 × 0.0625².
    ([0.0625; 3], [0.024_414_063; 3]),
    // Between 0.08 and 0.8, minus F90, with the hue kept.
    ([0.25, 0.5, 0.75], [0.21, 0.46, 0.71]),
    // Above, the peak p = x − 0.04 is compressed to 1 − 0.24² / (p + 0.24 − 0.76).
    ([1.0; 3], [0.869_090_9; 3]),
    ([2.0; 3], [0.96; 3]),
    ([4.0; 3], [0.983_255_8; 3]),
    ([16.0; 3], [0.996_269_4; 3]),
    // A compressed color turns towards white by g = 1 − 1 / (0.15 (p − p') + 1).
    ([1.0, 0.0, 0.0], [0.88, 0.015_559_9, 0.015_559_9]),
    // ... keeping its hue: (g − b) / (r − b) stays 0.2.
    ([4.0, 1.0, 0.25], [0.983_255_8, 0.468_299_2, 0.339_56]),
];

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// Draws a scene without instances under a uniform sky of radiance `light`, into `output` at
/// `exposure`.
fn draw(
    gpu: &Gpu,
    renderer: &mut Renderer,
    output: &RenderTarget,
    light: [f32; 3],
    exposure: f32,
) -> RenderStats {
    let [r, g, b] = light;
    let uniform = LinearColor::rgb(r, g, b);
    let environment = renderer
        .create_environment(
            gpu,
            &Sky {
                zenith: uniform,
                horizon: uniform,
                ground: uniform,
                sun: None,
            },
        )
        .expect("the GPU has memory for it");
    let mut scene = Scene::new(Camera::default(), &[]);
    scene.environment = Some(&environment);
    scene.exposure = exposure;
    renderer
        .render(gpu, output, &scene)
        .expect("the frame is drawn")
}

/// Checks that every pixel of `image` shows `expected`, opaque, within 10^-3: about two steps of
/// half precision below 1 (one step is 4.9 × 10^-4), which covers a driver that rounds toward zero
/// and the differences between the shader's arithmetic and the tests' copy of the curve.
fn assert_every_pixel(image: &FloatImage, expected: [f32; 3], case: &str) {
    for pixel in image.pixels.chunks(4) {
        let close = pixel[..3]
            .iter()
            .zip(expected)
            .all(|(actual, expected)| (actual - expected).abs() <= 1e-3);
        assert!(
            close && pixel[3] == 1.0,
            "{case}: expected {expected:?}, got {pixel:?}"
        );
    }
}

#[test]
fn the_tone_curve_of_the_tests_follows_the_specification() {
    for (light, expected) in CURVE {
        let actual = tone_curve::tone_map(light);
        let close = actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| (actual - expected).abs() <= 1e-6);
        assert!(close, "{light:?}: expected {expected:?}, got {actual:?}");
    }
}

#[test]
fn the_tone_mapping_pass_maps_light_as_the_specification_says() {
    let gpu = gpu();
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    let output = gpu
        .create_render_target("output", 4, 4, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    for (light, expected) in CURVE {
        let stats = draw(&gpu, &mut renderer, &output, light, 1.0);
        assert_eq!(
            stats.draw_calls, 0,
            "the tone-mapping pass's draw is not counted"
        );
        let image = gpu
            .read_float_render_target(&output)
            .expect("the image is read back");
        assert_every_pixel(&image, expected, &format!("light {light:?}"));
    }
}

#[test]
fn the_exposure_multiplies_the_light_before_the_curve() {
    let gpu = gpu();
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    let output = gpu
        .create_render_target("output", 4, 4, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    for (light, exposure, expected) in [
        (0.25, 2.0, 0.46),
        (2.0, 0.25, 0.46),
        // Light far above 1 at an exposure of 0 is black.
        (8.0, 0.0, 0.0),
    ] {
        draw(&gpu, &mut renderer, &output, [light; 3], exposure);
        let image = gpu
            .read_float_render_target(&output)
            .expect("the image is read back");
        assert_every_pixel(
            &image,
            [expected; 3],
            &format!("light {light} at an exposure of {exposure}"),
        );
    }
}

#[test]
fn the_sky_is_exposed_before_the_high_dynamic_range_target_and_kept_within_its_range() {
    let gpu = gpu();
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    let output = gpu
        .create_render_target("output", 4, 4, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    // The target holds exposed light divided by 64 (ADR-0040); beyond 64 × 65 504 it is stored as
    // 65 504, as the sky shader writes it, rather than left to the driver, which may store an
    // infinity that the curve turns into NaN. lavapipe and RADV would store 65 504 without the
    // shader's clamp too, and near white the curve cannot tell 64 × 65 504 from 65 504: this
    // checks that the light stays finite and white, not the clamp nor the division by 64, which
    // `bloom.rs` checks.
    draw(&gpu, &mut renderer, &output, [65_504.0; 3], 128.0);
    let image = gpu
        .read_float_render_target(&output)
        .expect("the image is read back");
    assert_every_pixel(
        &image,
        tone_curve::tone_map([64.0 * 65_504.0; 3]),
        "65 504 at an exposure of 128",
    );
    // The disc of a sun of 100 000 lux has 100 000 / Ω = 1.47 × 10⁹ nits, beyond half
    // precision, exposed in the shader to 0.5 here, and to 64 × 65 504 at an exposure of 1. Seen
    // along the light, every pixel of a narrow view is in the disc, where the sun's glow adds
    // 14 324 nits, 5 × 10⁻⁶ once exposed. The same light in the scene lights surfaces but draws
    // no second disc.
    let solid_angle = 2.0 * std::f64::consts::PI * (1.0 - (0.533_f64 / 2.0).to_radians().cos());
    let mut disc = |direction: Vec3, exposure: f32| {
        let sun = DirectionalLight {
            direction,
            illuminance: 100_000.0,
            ..DirectionalLight::default()
        };
        let environment = renderer
            .create_environment(
                &gpu,
                &Sky {
                    zenith: LinearColor::BLACK,
                    horizon: LinearColor::BLACK,
                    ground: LinearColor::BLACK,
                    sun: Some(sun),
                },
            )
            .expect("the GPU has memory for it");
        let narrow = Camera {
            eye: Vec3::ZERO,
            target: -direction,
            fov_y: 0.001,
            ..Camera::default()
        };
        let lights = [Light::Directional(sun)];
        let mut scene = Scene::new(narrow, &[]);
        scene.lights = &lights;
        scene.environment = Some(&environment);
        scene.exposure = exposure;
        renderer
            .render(&gpu, &output, &scene)
            .expect("the frame is drawn");
        gpu.read_float_render_target(&output)
            .expect("the image is read back")
    };
    // A sun 26.6° high.
    let high = Vec3::new(0.0, -0.5, -1.0);
    assert_every_pixel(
        &disc(high, (0.5 * solid_angle / 100_000.0) as f32),
        tone_curve::tone_map([0.5; 3]),
        "the sun's disc",
    );
    // Exposed at 1, it would be 1.47 × 10⁹: stored as 65 504, 64 × 65 504 once multiplied back.
    assert_every_pixel(
        &disc(high, 1.0),
        tone_curve::tone_map([64.0 * 65_504.0; 3]),
        "the sun's disc at an exposure of 1",
    );
    // 11.3° below the horizon, the ground's haze leaves e^(−0.196 / 0.03) = 1.5 × 10⁻³ of it.
    let low = disc(
        Vec3::new(0.0, 0.2, -1.0),
        (0.5 * solid_angle / 100_000.0) as f32,
    );
    assert!(
        low.pixels.iter().all(|&value| value < 2e-3 || value == 1.0),
        "the sun below the horizon: {:?}",
        &low.pixels[..4]
    );
}

#[test]
fn the_targets_follow_the_size_of_the_output() {
    let gpu = gpu();
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    // A group of the tone-mapping pass kept from the previous size would read the previous
    // target, and so the previous light; every pixel of an odd-sized output shows that the
    // triangle covers it. Only the height changes, then only the width, then both.
    for ((width, height), light, expected) in [
        ((4, 4), 2.0, 0.96),
        ((4, 7), 0.5, 0.46),
        ((7, 7), 4.0, 0.983_255_8),
        ((4, 3), 2.0, 0.96),
    ] {
        let output = gpu
            .create_render_target("output", width, height, TextureFormat::Rgba16Float)
            .expect("the GPU has memory for it");
        draw(&gpu, &mut renderer, &output, [light; 3], 1.0);
        let image = gpu
            .read_float_render_target(&output)
            .expect("the image is read back");
        assert_eq!(image.pixels.len(), (width * height * 4) as usize);
        assert_every_pixel(&image, [expected; 3], &format!("a {width}x{height} output"));
    }
}

#[test]
fn an_srgb_output_encodes_the_tone_mapped_light() {
    let gpu = gpu();
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
    let output = gpu
        .create_render_target("output", 4, 4, TextureFormat::Rgba8UnormSrgb)
        .expect("the GPU has memory for it");
    draw(&gpu, &mut renderer, &output, [4.0; 3], 1.0);
    let image = gpu
        .read_render_target(&output)
        .expect("the image is read back");
    // 253.11 out of 255, far from a rounding boundary.
    let expected = tone_curve::displayed([4.0; 3]);
    for pixel in image.pixels.chunks(4) {
        let close = pixel[..3]
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 1);
        assert!(
            close && pixel[3] == 255,
            "expected {expected:?}, got {pixel:?}"
        );
    }
}
