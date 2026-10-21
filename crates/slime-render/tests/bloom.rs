//! The bloom (ADR-0040), checked by reading back `Rgba16Float` outputs: a single bright pixel
//! spreads into a halo that is symmetric, falls with the distance and moves with the pixel; a
//! uniform image, dark or not, is unchanged; a strength of 0, and outputs too small for a chain,
//! skip the bloom's passes; the chain follows the output's size. Runs on whatever adapter the
//! machine offers.

#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, TextureFormat};
use slime_render::glam::{DAffine3, DVec3, Vec3};
use slime_render::{
    Camera, Environment, Instance, Material, MaterialData, Mesh, MeshData, Projection, RenderStats,
    Renderer, Scene, Sky, View, ViewDraw,
};

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

/// The camera of the pixel scenes: 10 m in front of the plane z = 0, with a 60° field of view.
const DISTANCE: f32 = 10.0;
const FOV_Y: f32 = std::f32::consts::FRAC_PI_3;

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, DISTANCE).as_dvec3(),
        target: DVec3::ZERO,
        projection: Projection::Perspective { fov_y: FOV_Y },
        near: 1.0,
        ..Camera::default()
    }
}

/// A square in the plane z = 0 that covers the four samples of the pixel (x, y) of a `width` ×
/// `height` output, and none of another pixel's: 0.9 pixel wide, its edges beyond the samples,
/// which lie up to 0.375 pixel from the pixel's centre along each axis, and short of the nearest
/// samples of its neighbours, 0.625 pixel from it.
fn pixel_square(width: u32, height: u32, (x, y): (u32, u32)) -> MeshData {
    let half_height = DISTANCE * (FOV_Y / 2.0).tan();
    let half_width = half_height * width as f32 / height as f32;
    let cx = ((x as f32 + 0.5) / width as f32 * 2.0 - 1.0) * half_width;
    let cy = (1.0 - (y as f32 + 0.5) / height as f32 * 2.0) * half_height;
    let half = 0.45 * 2.0 * half_height / height as f32;
    MeshData {
        positions: vec![
            [cx - half, cy - half, 0.0],
            [cx + half, cy - half, 0.0],
            [cx + half, cy + half, 0.0],
            [cx - half, cy + half, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// A renderer drawing one emissive pixel on black: no light, no environment, at an exposure of 1.
struct Pixel {
    gpu: Gpu,
    renderer: Renderer,
    view: View,
    emitter: Material,
}

impl Pixel {
    fn new(emission: f32) -> Self {
        let gpu = gpu();
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
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
        let view = renderer
            .create_view(&gpu, "view")
            .expect("the view is created");
        Self {
            gpu,
            renderer,
            view,
            emitter,
        }
    }

    /// Draws the pixel (x, y) of a `width` × `height` output at `bloom`.
    fn draw(
        &mut self,
        (width, height): (u32, u32),
        at: (u32, u32),
        bloom: f32,
    ) -> (FloatImage, RenderStats) {
        let output = self
            .gpu
            .create_render_target("output", width, height, TextureFormat::Rgba16Float)
            .expect("the GPU has memory for it");
        let mesh: Mesh = self
            .renderer
            .create_mesh(&self.gpu, "pixel", &pixel_square(width, height, at))
            .expect("the square is valid");
        let instances = [Instance::new(&mesh, &self.emitter, DAffine3::IDENTITY)];
        let mut scene = Scene::new(instances.clone());
        scene.bloom = bloom;
        self.renderer
            .render(
                &self.gpu,
                &scene,
                &mut [ViewDraw::new(&mut self.view, camera(), &output)],
            )
            .expect("the frame is drawn");
        let stats = self.view.stats();
        let image = self
            .gpu
            .read_float_render_target(&output)
            .expect("the image is read back");
        (image, stats)
    }
}

/// The grey light that Khronos PBR Neutral shows as `shown`, below 0.76, where the curve is
/// x − 0.04 above x = 0.08 and 6.25 x² below.
fn untone(shown: f32) -> f32 {
    if shown < 0.04 {
        (shown / 6.25).sqrt()
    } else {
        shown + 0.04
    }
}

/// The red channel at (x, y).
fn red(image: &FloatImage, x: u32, y: u32) -> f32 {
    image.pixels[((y * image.width + x) * 4) as usize]
}

#[test]
fn a_single_bright_pixel_spreads_into_a_halo_symmetric_about_its_diagonal() {
    // About the diagonal through the pixel, x and y swapped, which maps every level's grid to
    // itself. Not about the other diagonal, nor mirror-symmetric: the pixel's centre is on no
    // corner of the levels' texels (the next test).
    let mut scene = Pixel::new(200.0);
    let (image, _) = scene.draw((256, 256), (128, 128), 0.04);
    // 4 px away, a pixel gets about 2.6 / 1 000 of the spread light: 200 x 0.04 x 0.0026, about
    // 0.021.
    let halo = untone(red(&image, 132, 128));
    assert!((0.01..0.04).contains(&halo), "the halo 4 px away: {halo}");
    for dy in -48i32..=48 {
        for dx in -48i32..=48 {
            let at = |dx: i32, dy: i32| red(&image, (128 + dx) as u32, (128 + dy) as u32);
            let (a, b) = (at(dx, dy), at(dy, dx));
            assert!(
                (a - b).abs() <= 1e-3 * a.max(b) + 1e-6,
                "({dx}, {dy}) shows {a}, ({dy}, {dx}) shows {b}"
            );
        }
    }
}

#[test]
fn the_halos_of_two_neighbouring_pixels_mirror_each_other() {
    // A pixel's centre is never on a corner of the chain's texels, so its halo cannot be mirror
    // symmetric about itself; the pixels (128, 128) and (127, 127) lie symmetrically about the
    // corner (128, 128) of every level, so each halo is the other's mirror image.
    let mut scene = Pixel::new(200.0);
    let (right, _) = scene.draw((256, 256), (128, 128), 0.04);
    let (left, _) = scene.draw((256, 256), (127, 127), 0.04);
    for dy in -48i32..=48 {
        for dx in -48i32..=48 {
            let a = red(&right, (128 + dx) as u32, (128 + dy) as u32);
            let b = red(&left, (127 - dx) as u32, (127 - dy) as u32);
            assert!(
                (a - b).abs() <= 1e-3 * a.max(b) + 1e-6,
                "({dx}, {dy}): {a} against its mirror {b}"
            );
        }
    }
}

#[test]
fn the_halo_falls_with_the_distance_and_moves_with_the_pixel_on_odd_outputs() {
    let mut scene = Pixel::new(200.0);
    // An odd output, whose levels round some sides down and others up: the halo must keep its light
    // wherever the pixel is.
    let size = (257, 131);
    let mut reference = None;
    for x in [100u32, 101, 117, 128, 141, 150] {
        // At a strength of 1 the halo is the blurred copy itself, bright enough to read far out.
        let (image, _) = scene.draw(size, (x, 65), 1.0);
        let profile: Vec<f32> = (1..=40).map(|d| untone(red(&image, x + d, 65))).collect();
        for pair in profile.windows(2) {
            assert!(pair[1] <= pair[0], "the halo of x = {x} rises: {profile:?}");
        }
        // Far from the pixel, where its position modulo 2 no longer matters, the halo is the same
        // wherever the pixel is.
        let far = [8usize, 16, 32].map(|d| profile[d - 1]);
        let reference = *reference.get_or_insert(far);
        for (actual, expected) in far.iter().zip(reference) {
            assert!(
                (actual - expected).abs() <= 0.03 * expected,
                "x = {x}: {far:?} against {reference:?}"
            );
        }
    }
}

#[test]
fn a_halo_does_not_wrap_around_to_the_other_edge() {
    let mut scene = Pixel::new(200.0);
    let (image, _) = scene.draw((256, 64), (0, 32), 0.04);
    let near = red(&image, 1, 32);
    let far = red(&image, 255, 32);
    assert!(far < 0.01 * near, "1 px away: {near}, 255 px away: {far}");
}

/// Draws a uniform sky of `light` at `bloom` into a `width` × `height` output.
fn uniform(
    gpu: &Gpu,
    renderer: &mut Renderer,
    (width, height): (u32, u32),
    light: f32,
    bloom: f32,
) -> (FloatImage, RenderStats) {
    let output = gpu
        .create_render_target("output", width, height, TextureFormat::Rgba16Float)
        .expect("the GPU has memory for it");
    let environment = Environment::new(
        gpu,
        &Sky {
            zenith: LinearColor::rgb(light, light, light),
            horizon: LinearColor::rgb(light, light, light),
            ground: LinearColor::rgb(light, light, light),
            sun: None,
        },
    )
    .expect("the GPU has memory for it");
    let mut scene = Scene::new(Vec::new());
    scene.environment = Some(environment.clone());
    scene.bloom = bloom;
    let mut view = renderer
        .create_view(gpu, "view")
        .expect("the view is created");
    renderer
        .render(
            gpu,
            &scene,
            &mut [ViewDraw::new(&mut view, Camera::default(), &output)],
        )
        .expect("the frame is drawn");
    let stats = view.stats();
    (
        gpu.read_float_render_target(&output)
            .expect("the image is read back"),
        stats,
    )
}

#[test]
fn a_uniform_image_dark_or_bright_is_unchanged_by_any_strength() {
    let gpu = gpu();
    let mut renderer =
        Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    for size in [(64, 64), (33, 17), (16, 16), (15, 15), (3, 2), (1, 1)] {
        for light in [0.0, 0.0625, 0.5, 4.0] {
            let (image, stats) = uniform(&gpu, &mut renderer, size, light, 1.0);
            let expected = tone_curve::tone_map([light; 3]);
            for pixel in image.pixels.chunks(4) {
                assert!(
                    (pixel[0] - expected[0]).abs() <= 1e-3,
                    "{size:?}, light {light}: {pixel:?} instead of {expected:?}"
                );
            }
            // Below 15 x 15 no level fits: the bloom's passes are skipped.
            if size.0 < 15 || size.1 < 15 {
                assert_eq!(stats.bloom_gpu_time, None, "{size:?}");
            }
        }
    }
}

#[test]
fn an_output_too_small_for_a_chain_shows_no_bloom_at_any_strength() {
    // Below 15 x 15 the chain has no level: the tone-mapping pass must mix nothing in, whatever
    // the strength, rather than read the scene's own light magnified as if it were the chain's.
    let mut scene = Pixel::new(200.0);
    for size in [(14, 14), (14, 64), (3, 2)] {
        let at = (size.0 / 2, size.1 / 2);
        let (image, stats) = scene.draw(size, at, 1.0);
        assert_eq!(stats.bloom_gpu_time, None, "{size:?}");
        for y in 0..size.1 {
            for x in 0..size.0 {
                let expected = if (x, y) == at {
                    tone_curve::tone_map([200.0; 3])[0]
                } else {
                    0.0
                };
                assert!(
                    (red(&image, x, y) - expected).abs() <= 1e-3,
                    "{size:?}, ({x}, {y}): {}",
                    red(&image, x, y)
                );
            }
        }
    }
}

#[test]
fn a_strength_of_0_skips_the_bloom() {
    let mut scene = Pixel::new(200.0);
    for _ in 0..4 {
        let (image, stats) = scene.draw((64, 64), (32, 32), 0.0);
        assert_eq!(stats.bloom_gpu_time, None);
        for y in 0..64 {
            for x in 0..64 {
                let expected = if (x, y) == (32, 32) {
                    tone_curve::tone_map([200.0; 3])[0]
                } else {
                    0.0
                };
                assert!(
                    (red(&image, x, y) - expected).abs() <= 1e-3,
                    "({x}, {y}): {}",
                    red(&image, x, y)
                );
            }
        }
    }
}

#[test]
fn the_chain_follows_the_size_of_the_output() {
    let mut scene = Pixel::new(200.0);
    let mut images = Vec::new();
    for size in [(64, 64), (96, 64), (64, 64)] {
        let (image, _) = scene.draw(size, (40, 24), 0.04);
        // A chain kept from the previous size would move the halo off its pixel.
        for d in [1u32, 2, 4, 6] {
            let (across, down) = (red(&image, 40 + d, 24), red(&image, 40, 24 + d));
            assert!(
                (across - down).abs() <= 0.02 * across.max(down),
                "{size:?}: {across} across and {down} down at {d} px"
            );
        }
        images.push(image.pixels);
    }
    assert_eq!(
        images[0], images[2],
        "the same size draws the same image again"
    );
}

#[test]
fn a_view_whose_frame_changes_shape_draws_what_a_new_one_draws_and_times_it_again() {
    // The bloom stops and resumes, the output grows and shrinks back: each change builds the
    // frame's graph again, with new targets and timers (ADR-0041).
    let shapes = [
        ((64, 48), 0.3),
        ((64, 48), 0.0),
        ((64, 48), 0.3),
        ((96, 48), 0.3),
        ((64, 48), 0.3),
    ];
    let mut scene = Pixel::new(200.0);
    for (index, &(size, bloom)) in shapes.iter().enumerate() {
        let (image, _) = scene.draw(size, (20, 20), bloom);
        let (fresh, _) = Pixel::new(200.0).draw(size, (20, 20), bloom);
        assert!(
            image.pixels == fresh.pixels,
            "change {index}, {size:?} at {bloom}: not what a new view draws"
        );
        if !scene.gpu.can_time() {
            continue;
        }
        let mut timed = false;
        for _ in 0..8 {
            scene.gpu.wait_idle().expect("the GPU finishes the frame");
            let (_, stats) = scene.draw(size, (20, 20), bloom);
            timed = stats.scene_gpu_time.is_some()
                && stats.sky_gpu_time.is_some()
                && stats.tone_mapping_gpu_time.is_some()
                && (bloom == 0.0 || stats.bloom_gpu_time.is_some());
            if timed {
                break;
            }
        }
        assert!(
            timed,
            "change {index}: the GPU times come back within a few frames"
        );
    }
}

#[test]
#[should_panic(expected = "the scene's bloom is 2; it is from 0 to 1")]
fn a_bloom_beyond_1_is_a_bug() {
    // It would take more light from a pixel than it holds.
    Pixel::new(1.0).draw((16, 16), (8, 8), 2.0);
}

#[test]
fn the_statistics_give_the_bloom_s_gpu_time_on_adapters_that_can_time_their_work() {
    let mut scene = Pixel::new(200.0);
    let mut time = None;
    for _ in 0..8 {
        let (_, stats) = scene.draw((64, 64), (32, 32), 0.04);
        time = time.or(stats.bloom_gpu_time);
        if time.is_some() {
            break;
        }
        scene.gpu.wait_idle().expect("the GPU finishes the frame");
    }
    eprintln!(
        "adapter `{}`: bloom timed {time:?}",
        scene.gpu.adapter_name()
    );
    assert_eq!(time.is_some(), scene.gpu.can_time());
}

/// A level of the CPU model: `width` × `height` values of one channel.
struct Level {
    width: usize,
    height: usize,
    values: Vec<f32>,
}

impl Level {
    /// The value at texel (x, y), the level mirrored beyond its edges (texel −1 reads texel 0, as a
    /// mirror-repeat sampler reads it).
    fn texel(&self, x: i64, y: i64) -> f32 {
        let mirrored = |i: i64, size: usize| {
            let period = 2 * size as i64;
            let i = i.rem_euclid(period);
            (if i < size as i64 { i } else { period - 1 - i }) as usize
        };
        self.values[mirrored(y, self.height) * self.width + mirrored(x, self.width)]
    }

    /// What a linear sampler reads at (u, v), in texels from the level's top left corner.
    fn sample(&self, u: f32, v: f32) -> f32 {
        let (x, y) = (u - 0.5, v - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let top = self.texel(x0, y0) * (1.0 - fx) + self.texel(x0 + 1, y0) * fx;
        let bottom = self.texel(x0, y0 + 1) * (1.0 - fx) + self.texel(x0 + 1, y0 + 1) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    /// The 3 x 3 tent of one texel around (u, v).
    fn tent(&self, u: f32, v: f32) -> f32 {
        let mut sum = 0.0;
        for (dy, wy) in [(-1.0, 1.0), (0.0, 2.0), (1.0, 1.0)] {
            for (dx, wx) in [(-1.0, 1.0), (0.0, 2.0), (1.0, 1.0)] {
                sum += self.sample(u + dx, v + dy) * wx * wy;
            }
        }
        sum / 16.0
    }
}

/// What the scene pass divides exposed light by before it writes the high-dynamic-range target,
/// which the chain filters as it is (`HEADROOM` in `shader.wgsl`).
const HEADROOM: f32 = 64.0;

/// `value`, exposed light, as the high-dynamic-range target and the chain store it: divided by
/// [`HEADROOM`], rounded to half precision, multiplied again.
fn stored(value: f32) -> f32 {
    half(value / HEADROOM) * HEADROOM
}

/// `value` rounded to half precision, as an `Rgba16Float` target stores it.
fn half(value: f32) -> f32 {
    let bits = slime_gfx::f32_to_f16(value);
    let exponent = i32::from((bits >> 10) & 0x1f);
    let mantissa = f32::from(bits & 0x3ff);
    let magnitude = if exponent == 0 {
        mantissa * 2f32.powi(-24)
    } else {
        (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15)
    };
    if bits & 0x8000 != 0 {
        -magnitude
    } else {
        magnitude
    }
}

/// The blurred copy that the chain of ADR-0040 makes of `scene`, computed on the CPU from the
/// ADR's description, not from the shaders: level i the output divided by 2^i, rounded to the
/// nearest, while both sides keep 8 texels; Jimenez's 13 taps down, read at twice the pixel's
/// position; tents up, read at half, each level averaged with the same weight; every level read
/// mirrored beyond its edges and stored as the target stores it ([`stored`]); then the
/// tone-mapping pass's tent, at the output's size.
fn blurred(scene: &Level) -> Vec<f32> {
    let mut downs: Vec<Level> = Vec::new();
    for index in 1.. {
        let finer = downs.last().unwrap_or(scene);
        let level = |side: usize| (side + (1 << (index - 1))) >> index;
        let (width, height) = (level(scene.width), level(scene.height));
        if width < 8 || height < 8 {
            break;
        }
        let mut values = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let (u, v) = (2.0 * x as f32 + 1.0, 2.0 * y as f32 + 1.0);
                let tap = |dx: f32, dy: f32| finer.sample(u + dx, v + dy);
                let boxed = |cx: f32, cy: f32| {
                    (tap(cx - 1.0, cy - 1.0)
                        + tap(cx + 1.0, cy - 1.0)
                        + tap(cx - 1.0, cy + 1.0)
                        + tap(cx + 1.0, cy + 1.0))
                        * 0.25
                };
                // The inner box of the four taps at ±1, and the four boxes of 2 x 2 taps around it.
                let inner = boxed(0.0, 0.0);
                let corners =
                    [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].map(|(cx, cy)| {
                        (tap(2.0 * cx, 2.0 * cy)
                            + tap(0.0, 2.0 * cy)
                            + tap(2.0 * cx, 0.0)
                            + tap(0.0, 0.0))
                            * 0.25
                    });
                values.push(stored(inner * 0.5 + corners.iter().sum::<f32>() * 0.125));
            }
        }
        downs.push(Level {
            width,
            height,
            values,
        });
    }
    let count = downs.len();
    assert!(count > 0, "the model needs a chain");
    // From the coarsest up, each sum the running average of its level and the coarser sum.
    let mut sum = downs.pop().expect("a level");
    for (index, level) in downs.iter().enumerate().rev() {
        let own = 1.0 / (count - index) as f32;
        let mut values = Vec::with_capacity(level.values.len());
        for y in 0..level.height {
            for x in 0..level.width {
                let coarser = sum.tent(0.5 * (x as f32 + 0.5), 0.5 * (y as f32 + 0.5));
                let own_light = level.values[y * level.width + x];
                values.push(stored(coarser + (own_light - coarser) * own));
            }
        }
        sum = Level {
            width: level.width,
            height: level.height,
            values,
        };
    }
    let mut out = Vec::with_capacity(scene.values.len());
    for y in 0..scene.height {
        for x in 0..scene.width {
            out.push(sum.tent(0.5 * (x as f32 + 0.5), 0.5 * (y as f32 + 0.5)));
        }
    }
    out
}

#[test]
fn the_halo_is_the_blur_that_an_independent_model_of_the_chain_computes() {
    let mut scene = Pixel::new(200.0);
    // Inside, and at corners, where every pass reads its level mirrored beyond the edges.
    for (size, at) in [
        ((256u32, 256u32), (128u32, 128u32)),
        ((257, 131), (101, 64)),
        ((64, 64), (0, 0)),
        ((257, 131), (256, 130)),
    ] {
        assert_the_halo_is_the_model_s(&mut scene, size, at, 200.0);
    }
}

#[test]
fn light_beyond_the_largest_half_keeps_its_halo() {
    // A pixel of 4 × 65 536, exposed: the high-dynamic-range target holds exposed light divided by
    // 64, up to 64 × 65 504, so that the pixel keeps its light and its halo is the model's. Written
    // as 65 504 first, as the target would without that headroom, its halo would hold a quarter
    // of it, as the sun's disc did once the exposure was raised by half a stop.
    let mut scene = Pixel::new(262_144.0);
    assert_the_halo_is_the_model_s(&mut scene, (64, 64), (29, 29), 262_144.0);
}

/// Checks that the halo of the pixel `at`, of light `light`, drawn by `scene` at a strength of 1
/// into an output of `size`, where the image is the blurred copy itself, is what [`blurred`]
/// computes, at every pixel where the tone curve can be inverted precisely.
fn assert_the_halo_is_the_model_s(scene: &mut Pixel, size: (u32, u32), at: (u32, u32), light: f32) {
    let (image, _) = scene.draw(size, at, 1.0);
    let mut given = Level {
        width: size.0 as usize,
        height: size.1 as usize,
        values: vec![0.0; (size.0 * size.1) as usize],
    };
    given.values[(at.1 * size.0 + at.0) as usize] = stored(light);
    let expected = blurred(&given);
    let (mut largest, mut compared) = (0.0f32, 0);
    for y in 0..size.1 {
        for x in 0..size.0 {
            let shown = red(&image, x, y);
            // Where the curve is exactly invertible, and the light, divided by 64 in the target,
            // at least half precision's smallest normal value (1e-4 shown is 0.004, stored as
            // 6.25e-5).
            if !(1e-4..0.7).contains(&shown) {
                continue;
            }
            let model = expected[(y * size.0 + x) as usize];
            let relative = (untone(shown) - model).abs() / model;
            largest = largest.max(relative);
            compared += 1;
            assert!(
                relative <= 6e-3,
                "{size:?}, ({x}, {y}): {} against the model's {model}",
                untone(shown)
            );
        }
    }
    assert!(compared > 500, "{size:?}: {compared} pixels compared");
    eprintln!("{size:?}: largest relative difference from the model {largest} ({compared} pixels)");
}

#[test]
fn the_chain_keeps_a_pixel_s_light_where_the_edges_lie_on_every_level_s_grid() {
    // The model, which the test above holds the GPU to: mirrored at edges that lie on every level's
    // grid (an output whose sides are multiples of 2^levels), a pixel's halo holds its light
    // wherever the pixel lies, at the centre, a corner or an edge, but for the chain's
    // half-precision rounding. Clamped to the edge instead, a corner pixel's halo would hold twice
    // its light.
    for at in [(29, 29), (0, 0), (63, 40), (5, 63)] {
        let mut light = Level {
            width: 64,
            height: 64,
            values: vec![0.0; 64 * 64],
        };
        light.values[at.1 * 64 + at.0] = 128.0;
        let held: f32 = blurred(&light).iter().sum();
        assert!(
            (held / 128.0 - 1.0).abs() < 3e-3,
            "pixel {at:?}: its halo holds {held} of 128"
        );
    }
}
