//! The scene drawn in high dynamic range and mapped to the output by the tone-mapping pass
//! (ADR-0034), checked by reading back an `Rgba16Float` output. Known light reaches the
//! tone-mapping pass as the background of a scene without instances (clear colors reach the driver
//! unclamped, ADR-0033), exposed by the renderer before the scene pass clears its target to it
//! (ADR-0037), and must come out as Khronos PBR Neutral gives it. Every input is exact in half
//! precision, but for the background beyond its range. Runs on whatever adapter the machine offers.

#[path = "common/tone_curve.rs"]
mod tone_curve;

use slime_gfx::{Color, FloatImage, Gpu, RenderTarget, TextureFormat};
use slime_render::{Camera, DirectionalLight, RenderStats, Renderer, Scene};

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

/// Draws a scene without instances, whose background is `light`, into `output` at `exposure`.
/// The background's alpha is 0, which the output must not show: it is opaque (ADR-0034).
fn draw(
    gpu: &Gpu,
    renderer: &mut Renderer,
    output: &RenderTarget,
    light: [f32; 3],
    exposure: f32,
) -> RenderStats {
    let [r, g, b] = light.map(f64::from);
    renderer.render(
        gpu,
        output,
        &Scene {
            camera: Camera::default(),
            light: DirectionalLight::default(),
            clear: Color { r, g, b, a: 0.0 },
            exposure,
            instances: &[],
        },
    )
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
    let output = gpu.create_render_target("output", 4, 4, TextureFormat::Rgba16Float);
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
    let output = gpu.create_render_target("output", 4, 4, TextureFormat::Rgba16Float);
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
fn the_background_is_exposed_before_the_high_dynamic_range_target_and_kept_within_its_range() {
    let gpu = gpu();
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    let output = gpu.create_render_target("output", 4, 4, TextureFormat::Rgba16Float);
    // 10⁶, which half precision does not hold, at an exposure of 2⁻²⁰: 0.954, shown at 0.854.
    // Exposed after the target, it would be stored as 65 504 and shown at 0.024, or stored as an
    // infinity.
    draw(&gpu, &mut renderer, &output, [1e6; 3], 2f32.powi(-20));
    let image = gpu
        .read_float_render_target(&output)
        .expect("the image is read back");
    let exposed = (1e6 * 2f64.powi(-20)) as f32;
    assert_every_pixel(
        &image,
        tone_curve::tone_map([exposed; 3]),
        "10⁶ at an exposure of 2⁻²⁰",
    );
    // Exposed light beyond 65 504 is stored as 65 504, as the scene shader writes surfaces, rather
    // than left to the driver, which may store an infinity that the curve turns into NaN.
    draw(&gpu, &mut renderer, &output, [1e5; 3], 1.0);
    let image = gpu
        .read_float_render_target(&output)
        .expect("the image is read back");
    assert_every_pixel(
        &image,
        tone_curve::tone_map([65_504.0; 3]),
        "10⁵ at an exposure of 1",
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
        let output = gpu.create_render_target("output", width, height, TextureFormat::Rgba16Float);
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
    let output = gpu.create_render_target("output", 4, 4, TextureFormat::Rgba8UnormSrgb);
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
