//! The settings of a view (`docs/adr/0127-the-settings-of-a-view.md`): a render scale below 1
//! draws fewer pixels, which the tone mapping reads bilinearly over the view's rectangle, and one
//! of 2 averages four for each pixel; the modes of debugging show the normals and the shadows as
//! they are; a view without shadows, sky or bloom draws what a scene without them draws, and the
//! first view that draws shadows draws the spot lights' tiles; a size dragged frame after frame
//! keeps the view's targets, and its times, until it holds still, then makes them once. The
//! anti-aliasing is checked in `anti_aliasing.rs`. Runs on whatever adapter the machine offers.

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::{DAffine3, DQuat, DVec3, Vec3};
use slime_render::{
    Camera, DebugMode, DirectionalLight, Environment, Instance, Light, Material, MaterialData,
    Mesh, MeshData, RESIZE_DELAY, RenderStats, Renderer, Scene, Sky, SpotLight, View, ViewDraw,
    ViewSettings, Viewport,
};

const FORMAT: TextureFormat = TextureFormat::Rgba16Float;

fn gpu() -> Gpu {
    Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers")
}

fn output(gpu: &Gpu, width: u32, height: u32) -> RenderTarget {
    gpu.create_render_target("output", width, height, FORMAT)
        .expect("the GPU has memory for it")
}

/// A view of `renderer` with the default settings that `change` changes.
fn view(gpu: &Gpu, renderer: &Renderer, change: impl FnOnce(&mut ViewSettings)) -> View {
    let mut view = renderer
        .create_view(gpu, "view")
        .expect("the view is created");
    let mut settings = ViewSettings::default();
    change(&mut settings);
    view.set_settings(settings);
    view
}

/// Draws `scene` through `view`, from `camera`, into a new output of `width` × `height`, and
/// reads it back.
fn draw(
    gpu: &Gpu,
    renderer: &mut Renderer,
    scene: &Scene,
    view: &mut View,
    camera: Camera,
    (width, height): (u32, u32),
) -> FloatImage {
    let target = output(gpu, width, height);
    renderer
        .render(gpu, scene, &mut [ViewDraw::new(view, camera, &target)])
        .expect("the frame is drawn");
    gpu.read_float_render_target(&target)
        .expect("the image is read back")
}

/// The statistics of `view` once the GPU times of its passes have come back, on adapters that can
/// time their work: `scene` drawn from `camera` into a new output of `width` × `height`, frame
/// after frame, each waited for, until the times of its scene pass and of its tone mapping, the
/// last, come back. A pass whose time is `None` then did not run.
fn timed(
    gpu: &Gpu,
    renderer: &mut Renderer,
    scene: &Scene,
    view: &mut View,
    camera: Camera,
    (width, height): (u32, u32),
) -> RenderStats {
    let target = output(gpu, width, height);
    for _ in 0..16 {
        renderer
            .render(gpu, scene, &mut [ViewDraw::new(view, camera, &target)])
            .expect("the frame is drawn");
        gpu.wait_idle().expect("the GPU finishes");
        let stats = view.stats();
        let back = stats.scene_gpu_time.is_some() && stats.tone_mapping_gpu_time.is_some();
        if back || !gpu.can_time() {
            return stats;
        }
    }
    panic!("the GPU times come back within a few frames");
}

/// The red, green and blue of the pixel at (x, y).
fn pixel(image: &FloatImage, x: u32, y: u32) -> [f32; 3] {
    let start = ((y * image.width + x) * 4) as usize;
    [
        image.pixels[start],
        image.pixels[start + 1],
        image.pixels[start + 2],
    ]
}

fn assert_close(actual: [f32; 3], expected: [f32; 3], tolerance: f32, case: &str) {
    let off = actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f32::max);
    assert!(
        off <= tolerance,
        "{case}: {actual:?} where {expected:?} was expected"
    );
}

/// What the scenes of these tests are made of.
struct Things {
    square: Mesh,
    cube: Mesh,
    white: Material,
}

impl Things {
    fn new(gpu: &Gpu, renderer: &Renderer) -> Self {
        Self {
            square: renderer
                .create_mesh(gpu, "square", &MeshData::square())
                .expect("the square is valid"),
            cube: renderer
                .create_mesh(gpu, "cube", &MeshData::cube())
                .expect("the cube is valid"),
            white: renderer
                .create_material(gpu, "white", &MaterialData::default())
                .expect("the GPU has memory for it"),
        }
    }

    /// A material that gives `light` and reflects none.
    fn glow(gpu: &Gpu, renderer: &Renderer, light: f32) -> Material {
        renderer
            .create_material(
                gpu,
                "glow",
                &MaterialData {
                    base_color: LinearColor::BLACK,
                    emissive: LinearColor::WHITE,
                    emissive_strength: light,
                    ..MaterialData::default()
                },
            )
            .expect("the GPU has memory for it")
    }

    /// A floor 20 m wide with a cube of 1 m on it, at the origin, lit by `lights`.
    fn floor_and_cube(&self, lights: Vec<Light>) -> Scene {
        let flat = DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2);
        let mut scene = Scene::new(vec![
            Instance::new(
                &self.square,
                &self.white,
                DAffine3::from_scale_rotation_translation(
                    DVec3::new(20.0, 20.0, 1.0),
                    flat,
                    DVec3::ZERO,
                ),
            ),
            Instance::new(
                &self.cube,
                &self.white,
                DAffine3::from_translation(DVec3::new(0.0, 0.5, 0.0)),
            ),
        ]);
        scene.lights = lights;
        scene
    }
}

/// The camera of the tests that look at the plane z = 0: 10 m in front of it, with the default
/// field of view of 60°, which shows the plane 2 × 5.77 m high.
fn facing() -> Camera {
    Camera {
        eye: DVec3::new(0.0, 0.0, 10.0),
        target: DVec3::ZERO,
        ..Camera::default()
    }
}

/// The camera of the floor and the cube: 10 m above the origin, looking down, −Z up the image, so
/// that +X runs to the right; a point of the floor at x metres lies at column
/// (x / 5.77 / 2 + 1 / 2) × the output's width.
fn above() -> Camera {
    Camera {
        eye: DVec3::new(0.0, 10.0, 0.0),
        target: DVec3::ZERO,
        up: Vec3::NEG_Z,
        ..Camera::default()
    }
}

/// The sun, from the left at 45°, which casts the cube's shadow on the floor from x = 0.5 m to
/// 1.5 m.
fn sun() -> Light {
    Light::Directional(DirectionalLight {
        direction: Vec3::new(1.0, -1.0, 0.0).normalize(),
        illuminance: 3.0,
        shadow_distance: 30.0,
        ..DirectionalLight::default()
    })
}

/// In the images of the floor and the cube, 64 × 64 seen from [`above`]: a pixel of the floor in
/// the cube's shadow, at x = 1 m, and one lit, at x = −2 m.
const SHADOWED: (u32, u32) = (37, 32);
const LIT: (u32, u32) = (20, 32);

/// A render scale of one half draws an image of half the size, which the tone mapping stretches
/// over the view's rectangle, reading between its texels bilinearly; one of 2 draws an image of
/// twice the size, of which each pixel shows the average of its four texels. Two lights in the
/// straight part of the tone curve, x − 0.04 from 0.08 to 0.76, make the output the bilinear
/// reading of the image drawn at a scale of 1, at the size that both other views draw at.
#[test]
fn a_render_scale_draws_fewer_or_more_pixels_read_bilinearly() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let (dim, bright) = (
        Things::glow(&gpu, &renderer, 0.3),
        Things::glow(&gpu, &renderer, 0.6),
    );
    // A wall that fills the view, and a brighter patch whose edges cross pixels.
    let scene = Scene::new(vec![
        Instance::new(
            &things.square,
            &dim,
            DAffine3::from_scale(DVec3::new(100.0, 100.0, 1.0)),
        ),
        Instance::new(
            &things.square,
            &bright,
            DAffine3::from_scale_rotation_translation(
                DVec3::new(3.3, 2.1, 1.0),
                DQuat::IDENTITY,
                DVec3::new(0.37, -0.21, 1.0),
            ),
        ),
    ]);
    let mut drawn = |scale: f32, side: u32| {
        let mut view = view(&gpu, &renderer, |settings| settings.render_scale = scale);
        let image = draw(
            &gpu,
            &mut renderer,
            &scene,
            &mut view,
            facing(),
            (side, side),
        );
        assert_eq!(view.render_size(), Some((32, 32)), "at a scale of {scale}");
        image
    };
    let whole = drawn(1.0, 32);
    let half = drawn(0.5, 64);
    let double = drawn(2.0, 16);
    // The texel of the image of 32 × 32 at (x, y), mirrored at its edges as the sampler reads it.
    let texel = |x: i64, y: i64| pixel(&whole, x.clamp(0, 31) as u32, y.clamp(0, 31) as u32);
    // Read bilinearly at (x, y), in texels from the image's top left corner.
    let read = |x: f64, y: f64| -> [f32; 3] {
        let (left, top) = ((x - 0.5).floor(), (y - 0.5).floor());
        let (across, down) = ((x - 0.5 - left) as f32, (y - 0.5 - top) as f32);
        let (left, top) = (left as i64, top as i64);
        let mut light = [0.0; 3];
        for (dx, dy, weight) in [
            (0, 0, (1.0 - across) * (1.0 - down)),
            (1, 0, across * (1.0 - down)),
            (0, 1, (1.0 - across) * down),
            (1, 1, across * down),
        ] {
            let shown = texel(left + dx, top + dy);
            for (light, shown) in light.iter_mut().zip(shown) {
                *light += weight * shown;
            }
        }
        light
    };
    let mut stretched = 0;
    for (image, scale) in [(&half, 0.5), (&double, 2.0)] {
        for y in 0..image.height {
            for x in 0..image.width {
                let centre = |side: u32| (f64::from(side) + 0.5) * scale;
                let expected = read(centre(x), centre(y));
                assert_close(
                    pixel(image, x, y),
                    expected,
                    2e-3,
                    &format!("({x}, {y}) at a scale of {scale}"),
                );
                stretched += usize::from(expected[0] > 0.27 && expected[0] < 0.55);
            }
        }
    }
    // The patch's edges are read between both lights.
    assert!(stretched > 50, "{stretched} pixels between both lights");
    // At one half, into a rectangle of a larger output: the rectangle shows what the view draws
    // into an output of its size, and the rest stays as it was.
    let mut framed = view(&gpu, &renderer, |settings| settings.render_scale = 0.5);
    let target = output(&gpu, 96, 80);
    let mut draw = ViewDraw::new(&mut framed, facing(), &target);
    draw.viewport = Some(Viewport {
        x: 16,
        y: 8,
        width: 64,
        height: 64,
    });
    renderer
        .render(&gpu, &scene, &mut [draw])
        .expect("the frame is drawn");
    let image = gpu
        .read_float_render_target(&target)
        .expect("the image is read back");
    for y in 0..80 {
        for x in 0..96 {
            let inside = (16..80).contains(&x) && (8..72).contains(&y);
            let expected = if inside {
                pixel(&half, x - 16, y - 8)
            } else {
                [0.0; 3]
            };
            assert_eq!(pixel(&image, x, y), expected, "({x}, {y})");
        }
    }
}

#[test]
#[should_panic(expected = "the view `view` is given a render scale of 3; it is from 0.25 to 2")]
fn a_render_scale_beyond_its_range_is_a_bug_of_the_caller() {
    let gpu = gpu();
    let renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    view(&gpu, &renderer, |settings| settings.render_scale = 3.0);
}

/// The normals' mode shows each surface's normal in world space, as colors written as they are,
/// whatever the exposure, the bloom or the sky, and black where no surface is.
#[test]
fn the_normals_mode_shows_each_surface_s_normal_as_a_color() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let turned = DQuat::from_rotation_y(std::f64::consts::FRAC_PI_4);
    let mut scene = Scene::new(vec![
        Instance::new(
            &things.square,
            &things.white,
            DAffine3::from_scale_rotation_translation(
                DVec3::splat(3.0),
                DQuat::IDENTITY,
                DVec3::new(-2.5, 0.0, 0.0),
            ),
        ),
        Instance::new(
            &things.square,
            &things.white,
            DAffine3::from_scale_rotation_translation(
                DVec3::splat(3.0),
                turned,
                DVec3::new(2.5, 0.0, 0.0),
            ),
        ),
    ]);
    scene.lights = vec![sun()];
    scene.exposure = 1e-3;
    scene.bloom = 0.5;
    scene.environment = Some(
        Environment::new(
            &gpu,
            &Sky {
                zenith: LinearColor::rgb(0.2, 0.3, 0.6),
                horizon: LinearColor::rgb(0.5, 0.5, 0.5),
                ground: LinearColor::rgb(0.2, 0.15, 0.1),
                sun: None,
            },
        )
        .expect("the GPU has memory for it"),
    );
    let mut view = view(&gpu, &renderer, |settings| {
        settings.debug = DebugMode::Normals;
    });
    let image = draw(&gpu, &mut renderer, &scene, &mut view, facing(), (64, 64));
    let half = std::f32::consts::FRAC_1_SQRT_2 / 2.0;
    // Facing +Z, then turned 45° about Y, towards +X.
    assert_close(pixel(&image, 18, 32), [0.5, 0.5, 1.0], 2e-3, "facing +Z");
    assert_close(
        pixel(&image, 45, 32),
        [0.5 + half, 0.5, 0.5 + half],
        2e-3,
        "turned about Y",
    );
    assert_close(pixel(&image, 32, 4), [0.0; 3], 0.0, "no surface");
    let stats = timed(&gpu, &mut renderer, &scene, &mut view, facing(), (64, 64));
    assert_eq!(stats.bloom_gpu_time, None, "no bloom");
    assert_eq!(stats.sky_gpu_time, None, "no sky");
    assert_eq!(stats.shadow_draw_calls, 0, "no shadows");
}

/// The shadows' mode shows the share of the shadowing lights' light that reaches each surface:
/// none in the cube's shadow, all of it elsewhere, tinted by the sun's cascade that holds the
/// surface, and white for a spot light.
#[test]
fn the_shadows_mode_shows_the_light_that_reaches_each_surface() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let tints = [
        [1.0, 0.5, 0.5],
        [0.5, 1.0, 0.5],
        [0.5, 0.5, 1.0],
        [1.0, 1.0, 0.5],
    ];
    let mut view = view(&gpu, &renderer, |settings| {
        settings.debug = DebugMode::Shadows;
    });
    let scene = things.floor_and_cube(vec![sun()]);
    let image = draw(&gpu, &mut renderer, &scene, &mut view, above(), (64, 64));
    let lit = pixel(&image, LIT.0, LIT.1);
    assert!(
        tints.iter().any(|&tint| {
            lit.iter()
                .zip(tint)
                .all(|(lit, tint)| (lit - tint).abs() <= 2e-3)
        }),
        "the sun's light reaches the floor, tinted by its cascade: {lit:?}"
    );
    let shadowed = pixel(&image, SHADOWED.0, SHADOWED.1);
    assert!(
        shadowed.iter().all(|&channel| channel < 0.05),
        "the cube shadows the floor from the sun: {shadowed:?}"
    );
    // A spot light from the same side: its shadow is not tinted.
    let spot = Light::Spot(SpotLight {
        position: DVec3::new(-3.0, 3.0, 0.0),
        direction: Vec3::new(1.0, -1.0, 0.0).normalize(),
        intensity: 20.0,
        outer_cone_angle: 0.6,
        ..SpotLight::default()
    });
    let scene = things.floor_and_cube(vec![spot]);
    let image = draw(&gpu, &mut renderer, &scene, &mut view, above(), (64, 64));
    assert_close(pixel(&image, LIT.0, LIT.1), [1.0; 3], 2e-3, "in the cone");
    let shadowed = pixel(&image, SHADOWED.0, SHADOWED.1);
    assert!(
        shadowed.iter().all(|&channel| channel < 0.05),
        "the cube shadows the floor from the spot light: {shadowed:?}"
    );
}

/// A view without shadows draws what a scene whose lights cast none draws, and draws no shadow
/// pass; one without sky is black where no surface is; one without bloom draws what a scene
/// without bloom draws. The first view that draws shadows draws the spot lights' tiles for the
/// views after it.
#[test]
fn a_view_draws_without_the_passes_that_it_leaves_out() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let spot = SpotLight {
        position: DVec3::new(-3.0, 3.0, 2.0),
        direction: Vec3::new(1.0, -1.0, -0.6).normalize(),
        intensity: 20.0,
        outer_cone_angle: 0.6,
        ..SpotLight::default()
    };
    let shadowed = things.floor_and_cube(vec![sun(), Light::Spot(spot)]);
    let mut unshadowed = shadowed.clone();
    for light in &mut unshadowed.lights {
        match light {
            Light::Directional(light) => light.casts_shadows = false,
            Light::Spot(light) => light.casts_shadows = false,
            _ => {}
        }
    }
    let camera = Camera {
        eye: DVec3::new(-4.0, 3.0, 6.0),
        target: DVec3::new(0.0, 0.5, 0.0),
        ..Camera::default()
    };
    let mut drawn = |scene: &Scene, change: fn(&mut ViewSettings)| {
        let mut view = view(&gpu, &renderer, change);
        let image = draw(&gpu, &mut renderer, scene, &mut view, camera, (64, 48));
        let stats = timed(&gpu, &mut renderer, scene, &mut view, camera, (64, 48));
        (image, stats)
    };
    let (without, stats) = drawn(&shadowed, |settings| settings.shadows = false);
    let (expected, _) = drawn(&unshadowed, |_| {});
    assert!(without.pixels == expected.pixels, "a view without shadows");
    assert_eq!(
        (stats.shadow_draw_calls, stats.spot_shadow_draw_calls),
        (0, 0)
    );
    let (with_shadows, _) = drawn(&shadowed, |_| {});
    assert!(with_shadows.pixels != expected.pixels, "the shadows show");

    // The sky above the floor's far edge; the floor before the cube.
    let mut sky = shadowed.clone();
    sky.environment = Some(
        Environment::new(
            &gpu,
            &Sky {
                zenith: LinearColor::rgb(0.2, 0.3, 0.6),
                horizon: LinearColor::rgb(0.5, 0.5, 0.5),
                ground: LinearColor::rgb(0.2, 0.15, 0.1),
                sun: None,
            },
        )
        .expect("the GPU has memory for it"),
    );
    let (without, stats) = drawn(&sky, |settings| settings.sky = false);
    let (with, with_stats) = drawn(&sky, |_| {});
    assert_eq!(stats.sky_gpu_time, None);
    assert_eq!(with_stats.sky_gpu_time.is_some(), gpu.can_time());
    assert_close(pixel(&without, 32, 1), [0.0; 3], 0.0, "no sky");
    assert!(
        pixel(&with, 32, 1)[2] > 0.1,
        "the sky: {:?}",
        pixel(&with, 32, 1)
    );
    assert_eq!(pixel(&without, 32, 44), pixel(&with, 32, 44), "the floor");

    // The bloom, at a strength that shows.
    let mut bloomed = shadowed.clone();
    bloomed.bloom = 0.5;
    let (without, stats) = drawn(&bloomed, |settings| settings.bloom = false);
    let (expected, _) = drawn(&shadowed, |_| {});
    assert!(without.pixels == expected.pixels, "a view without bloom");
    assert_eq!(stats.bloom_gpu_time, None);
    let (_, with_stats) = drawn(&bloomed, |_| {});
    assert_eq!(with_stats.bloom_gpu_time.is_some(), gpu.can_time());

    // The spot lights' tiles, drawn by the second view, the first drawing no shadows.
    let (first, second) = (output(&gpu, 64, 48), output(&gpu, 64, 48));
    let mut views = (
        view(&gpu, &renderer, |settings| settings.shadows = false),
        view(&gpu, &renderer, |_| {}),
    );
    renderer
        .render(
            &gpu,
            &shadowed,
            &mut [
                ViewDraw::new(&mut views.0, camera, &first),
                ViewDraw::new(&mut views.1, camera, &second),
            ],
        )
        .expect("the frame is drawn");
    assert_eq!(views.0.stats().spot_shadow_draw_calls, 0);
    assert!(views.1.stats().spot_shadow_draw_calls > 0);
    let second = gpu
        .read_float_render_target(&second)
        .expect("the image is read back");
    assert!(
        second.pixels == with_shadows.pixels,
        "the second view draws the spot light's shadow"
    );
}

/// A window's edge dragged: the view drawn into a rectangle of another size at every frame keeps
/// its targets, and so its GPU times, and draws its image stretched over the rectangle, until the
/// size holds still for `RESIZE_DELAY`; it then makes its targets at that size, once, and draws
/// what a new view draws.
#[test]
fn a_size_dragged_frame_after_frame_makes_the_view_s_targets_once_it_holds_still() {
    let gpu = gpu();
    let mut renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
    let things = Things::new(&gpu, &renderer);
    let mut scene = things.floor_and_cube(vec![sun()]);
    scene.bloom = 0.04;
    let mut view = view(&gpu, &renderer, |_| {});
    let first = output(&gpu, 64, 48);
    let times = |view: &mut View, renderer: &mut Renderer, target: &RenderTarget| {
        renderer
            .render(&gpu, &scene, &mut [ViewDraw::new(view, above(), target)])
            .expect("the frame is drawn");
        view.stats().tone_mapping_gpu_time.is_some()
    };
    // Until the times come back, on adapters that can time their work.
    for _ in 0..8 {
        gpu.wait_idle().expect("the GPU finishes");
        if times(&mut view, &mut renderer, &first) {
            break;
        }
    }
    for width in 65..80 {
        let target = output(&gpu, width, 48);
        let timed = times(&mut view, &mut renderer, &target);
        assert_eq!(view.render_size(), Some((64, 48)), "dragged to {width}");
        assert!(
            timed || !gpu.can_time(),
            "the times keep coming back, at {width}"
        );
    }
    let last = output(&gpu, 79, 48);
    std::thread::sleep(RESIZE_DELAY);
    times(&mut view, &mut renderer, &last);
    assert_eq!(view.render_size(), Some((79, 48)), "held still");
    let image = gpu
        .read_float_render_target(&last)
        .expect("the image is read back");
    let mut fresh = self::view(&gpu, &renderer, |_| {});
    let expected = draw(&gpu, &mut renderer, &scene, &mut fresh, above(), (79, 48));
    assert!(image.pixels == expected.pixels, "what a new view draws");
}
