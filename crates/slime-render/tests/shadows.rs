//! Shadows of the first directional light that casts shadows (ADR-0038), checked by reading back an
//! `Rgba16Float` output and comparing pixels with what `common/brdf.rs` predicts, through the tone
//! curve of `common/tone_curve.rs`, with the share of light that the shadow lets through computed
//! here from the geometry: a square above a floor darkens a square of the floor, its edges blended
//! over the three texels of the filter; a lit floor does not shadow itself; only the first
//! directional light that casts shadows casts them; a masked material casts none where it discards.
//! Runs on whatever adapter the machine offers.
//!
//! The floor is the plane z = 0, facing +Z, and "above" is along +Z. A directional light in the
//! x-z plane gives the shadow map one axis along x and one along y on the floor, where each texel
//! is `side / SHADOW_MAP_SIZE` metres wide, `side` being the floor's: the map covers exactly the
//! instances' extent across the light, which the floor alone sets when every other instance lies
//! over it.

#[path = "common/brdf.rs"]
mod brdf;
#[path = "common/shown.rs"]
mod shown;
#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use std::f32::consts::PI;

use slime_gfx::{FloatImage, Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::{DVec3, EulerRot, Mat4, Vec3};
use slime_render::{
    AlphaMode, Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData,
    Mesh, MeshData, PointLight, RenderStats, Renderer, Scene, Sky,
};

use brdf::Surface;
use shown::assert_shows;

/// The shadow map's width in texels: `SHADOW_MAP_SIZE` in `crates/slime-render/src/shadow.rs`.
const SHADOW_MAP_SIZE: f64 = 2048.0;
const SIZE: u32 = 64;
const WHITE: Surface = Surface {
    base_color: [1.0; 3],
    metallic: 0.0,
    roughness: 1.0,
};
/// The radiance of a uniform sky: what a white surface in shadow reflects.
const SKY: f64 = 0.125;

/// The renderer, a 64 × 64 `Rgba16Float` output, a white matte square two units wide facing +Z,
/// centred on the origin, and its material.
struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    output: RenderTarget,
    square: Mesh,
    white: Material,
    /// A uniform sky of radiance `SKY`.
    sky: Environment,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
        let output = gpu
            .create_render_target("shadows", SIZE, SIZE, TextureFormat::Rgba16Float)
            .expect("the GPU has memory for it");
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
                    tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
                    indices: vec![0, 1, 2, 0, 2, 3],
                },
            )
            .expect("the square is valid");
        let white = renderer
            .create_material(&gpu, "white", &MaterialData::default())
            .expect("the GPU has memory for it");
        let uniform = LinearColor::rgb(SKY as f32, SKY as f32, SKY as f32);
        let sky = renderer
            .create_environment(
                &gpu,
                &Sky {
                    zenith: uniform,
                    horizon: uniform,
                    ground: uniform,
                    sun: None,
                },
            )
            .expect("the GPU has memory for it");
        Self {
            sky,
            gpu,
            renderer,
            output,
            square,
            white,
        }
    }

    /// Draws the square once for each material and transform of `squares`, seen from `camera`
    /// under `lights` and the uniform sky, at an exposure of 1, and reads the
    /// output back.
    fn draw(
        &mut self,
        camera: Camera,
        lights: &[Light],
        squares: &[(&Material, Mat4)],
    ) -> (FloatImage, RenderStats) {
        let instances: Vec<Instance<'_>> = squares
            .iter()
            .map(|&(material, transform)| Instance::new(&self.square, material, transform))
            .collect();
        let mut scene = Scene::new(camera, &instances);
        scene.lights = lights;
        scene.environment = Some(&self.sky);
        let stats = self
            .renderer
            .render(&self.gpu, &self.output, &scene)
            .expect("the frame is drawn");
        let image = self
            .gpu
            .read_float_render_target(&self.output)
            .expect("the image is read back");
        (image, stats)
    }
}

/// A floor `side` metres wide: the square, scaled.
fn floor(side: f32) -> Mat4 {
    Mat4::from_scale(Vec3::new(side / 2.0, side / 2.0, 1.0))
}

/// The square, 1 m wide, its center at `center`, facing +Z, or −Z when `facing_down`.
fn blocker(center: Vec3, facing_down: bool) -> Mat4 {
    let turn = if facing_down {
        Mat4::from_rotation_x(PI)
    } else {
        Mat4::IDENTITY
    };
    Mat4::from_translation(center) * turn * Mat4::from_scale(Vec3::splat(0.5))
}

/// A camera 3 m above the point (x, y) of the floor, looking straight down at it.
fn camera_above(x: f32, y: f32) -> Camera {
    Camera {
        eye: Vec3::new(x, y, 3.0),
        target: Vec3::new(x, y, 0.0),
        ..Camera::default()
    }
}

/// A white directional light `degrees` from the floor's normal, travelling towards +x, so that
/// shadows fall towards +x, whose illuminance gives a floor facing up `reflected` × π lux, and the
/// unit vector towards it.
fn sun(degrees: f64, reflected: f64) -> (DirectionalLight, DVec3) {
    let angle = degrees.to_radians();
    let to_light = DVec3::new(-angle.sin(), 0.0, angle.cos());
    let light = DirectionalLight {
        direction: (-to_light).as_vec3(),
        color: LinearColor::WHITE,
        illuminance: (reflected * std::f64::consts::PI / angle.cos()) as f32,
        casts_shadows: true,
    };
    (light, to_light)
}

/// What a white matte floor reflects towards `camera` at `pixel`, lit by `light` arriving from
/// `to_light`, of which `lit` reaches it, plus the sky's light.
fn floor_shows(
    camera: &Camera,
    pixel: (u32, u32),
    (light, to_light): (DirectionalLight, DVec3),
    lit: f64,
) -> [f64; 3] {
    let to_camera = brdf::to_camera(camera, (SIZE, SIZE), pixel, 0.0);
    brdf::reflected(WHITE, DVec3::Z, to_light, to_camera)
        .map(|channel| channel * f64::from(light.illuminance) * lit + SKY)
}

/// The share of a shadow at `u`, in texels, along an axis where it covers the texels from `from`
/// to `to`, both texel edges: 3 × 3 comparisons one texel apart, each blending its two nearest
/// texels along the axis, ramp linearly over three texels centred on each edge.
fn shadowed_along(u: f64, (from, to): (f64, f64)) -> f64 {
    let ramp = |x: f64| ((x + 1.5) / 3.0).clamp(0.0, 1.0);
    ramp(u - from) - ramp(u - to)
}

/// The largest difference, in any channel, between each pixel of `image` and the tone-mapped
/// light that `expected` gives for it, kept apart for the pixels it marks as in a penumbra.
fn largest_differences(
    image: &FloatImage,
    mut expected: impl FnMut((u32, u32)) -> ([f64; 3], bool),
) -> (f32, f32) {
    let (mut elsewhere, mut in_penumbra) = (0.0f32, 0.0f32);
    for y in 0..image.height {
        for x in 0..image.width {
            let (light, penumbra) = expected((x, y));
            let shown = tone_curve::tone_map(light.map(|channel| channel as f32));
            let start = ((y * image.width + x) * 4) as usize;
            let difference = image.pixels[start..start + 3]
                .iter()
                .zip(shown)
                .map(|(actual, expected)| (actual - expected).abs())
                .fold(0.0, f32::max);
            let largest = if penumbra {
                &mut in_penumbra
            } else {
                &mut elsewhere
            };
            *largest = largest.max(difference);
        }
    }
    (elsewhere, in_penumbra)
}

#[test]
fn a_square_above_a_floor_darkens_a_square_of_the_floor_blended_over_three_texels() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    // The floor sets the map's extent: 256 m wide, texels of 1/8 m. A square 1 m wide, 1.5 m above
    // it and off its center, under a light 45° from the normal, shadows x from 1 to 2 and y from
    // 0.25 to 1.25: texel edges. The camera above the shadow's center sees 3.5 m of floor, 54 mm
    // per pixel, the penumbra over 7 pixels, and not the square.
    let side = (SHADOW_MAP_SIZE / 8.0) as f32;
    let texel = f64::from(side) / SHADOW_MAP_SIZE;
    let sun = sun(45.0, 0.4);
    let camera = camera_above(1.5, 0.75);
    let to_texels = |from: f64, to: f64| {
        let half = f64::from(side) / 2.0;
        ((from + half) / texel, (to + half) / texel)
    };
    let (across, along) = (to_texels(1.0, 2.0), to_texels(0.25, 1.25));
    // Every face casts, whichever way it faces the light.
    for (facing_down, case) in [(false, "facing the light"), (true, "facing away from it")] {
        let (image, stats) = setup.draw(
            camera,
            &[Light::Directional(sun.0)],
            &[
                (&white, floor(side)),
                (&white, blocker(Vec3::new(0.0, 0.75, 1.5), facing_down)),
            ],
        );
        assert_eq!(
            (stats.draw_calls, stats.shadow_draw_calls),
            (2, 2),
            "{case}"
        );
        let mut dark = 0;
        let (elsewhere, in_penumbra) = largest_differences(&image, |pixel| {
            let point = brdf::seen_point(&camera, (SIZE, SIZE), pixel, 0.0);
            let shadowed = shadowed_along((point.x + f64::from(side) / 2.0) / texel, across)
                * shadowed_along((point.y + f64::from(side) / 2.0) / texel, along);
            if shadowed == 1.0 {
                dark += 1;
            }
            (
                floor_shows(&camera, pixel, sun, 1.0 - shadowed),
                shadowed > 0.0 && shadowed < 1.0,
            )
        });
        eprintln!(
            "adapter `{}`, a square {case}: {dark} pixels in full shadow; largest difference \
             {elsewhere} outside the penumbra, {in_penumbra} in it",
            setup.gpu.adapter_name()
        );
        // About 11 × 11 pixels: 1 m less the penumbra's 3/8 m, of 54 mm pixels.
        assert!(dark > 100, "{case}: {dark} pixels in full shadow");
        // Within 2 × 10⁻³ as `shown.rs` explains; in the penumbra 4 × 10⁻³, for GPUs that keep
        // the filter's weights in 8 bits: up to 1/256 of the light there.
        assert!(elsewhere <= 2e-3, "{case}: off by {elsewhere}");
        assert!(
            in_penumbra <= 4e-3,
            "{case}: off by {in_penumbra} in the penumbra"
        );
    }
}

#[test]
fn a_lit_floor_does_not_shadow_itself() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    // The floor, 8 m wide, lit from 0° to 85° from its normal, the light along an axis of the map
    // or diagonally to it, where the filter reaches the most texels downhill. A square 20 m above a
    // corner, whose shadow falls beyond the floor, puts the floor at the far end of the map's
    // depths, near 0, where they are finest (depth is reversed) and the constant bias, counted in
    // their last bit, smallest. Then the whole scene turned and moved 46 m away from the origin,
    // where rounding grows: the floor facing the light, without any slope, compares its own depths
    // computed two ways.
    let canonical = Camera {
        eye: Vec3::new(0.3, 0.2, 2.0),
        target: Vec3::new(0.1, -0.1, 0.0),
        ..Camera::default()
    };
    let placements = [
        Mat4::IDENTITY,
        Mat4::from_translation(Vec3::new(40.0, -20.0, 10.0))
            * Mat4::from_euler(EulerRot::XYZ, 0.7, -0.4, 1.9),
    ];
    let lightings = [
        (0.0, 0.0),
        (30.0, 0.0),
        (60.0, 0.0),
        (85.0, 0.0),
        (30.0, 45.0),
        (60.0, 45.0),
        (85.0, 45.0),
    ];
    let mut failures = Vec::new();
    for placement in placements {
        for (polar, azimuth) in lightings {
            let (light, to_light) = sun(polar, 0.4);
            let turn = Mat4::from_rotation_z((azimuth as f32).to_radians());
            let to_light = turn.as_dmat4().transform_vector3(to_light);
            let light = DirectionalLight {
                direction: turn.transform_vector3(light.direction),
                ..light
            };
            let placed = |light: DirectionalLight| DirectionalLight {
                direction: placement.transform_vector3(light.direction),
                ..light
            };
            let camera = Camera {
                eye: placement.transform_point3(canonical.eye),
                target: placement.transform_point3(canonical.target),
                up: placement.transform_vector3(Vec3::Y),
                ..canonical
            };
            let (image, _) = setup.draw(
                camera,
                &[Light::Directional(placed(light))],
                &[
                    (&white, placement * floor(8.0)),
                    (
                        &white,
                        placement * blocker(Vec3::new(-3.5, 3.5, 20.0), true),
                    ),
                ],
            );
            // The same as in the canonical frame: placing everything alike changes no light.
            let (largest, _) = largest_differences(&image, |pixel| {
                (
                    floor_shows(&canonical, pixel, (light, to_light), 1.0),
                    false,
                )
            });
            if largest > 2e-3 {
                failures.push((placement.w_axis.truncate(), polar, azimuth, largest));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "the floor shadows itself: {failures:?}"
    );
}

#[test]
fn only_the_first_directional_light_that_casts_shadows_casts_them() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    let side = (SHADOW_MAP_SIZE / 8.0) as f32;
    let squares = [
        (&white, floor(side)),
        (&white, blocker(Vec3::new(0.0, 0.0, 1.5), false)),
    ];
    // At the center of the image, the floor at (1.5, 0, 0): 4 texels inside the shadow of a light
    // 45° from the normal, outside the shadow of one from straight above, which falls under the
    // square, out of view. A point light sees it behind the square.
    let camera = camera_above(1.5, 0.0);
    let center = (SIZE / 2, SIZE / 2);
    let slanted = sun(45.0, 0.2);
    let above = sun(0.0, 0.1);
    let lamp_at = DVec3::new(-3.0, 0.0, 4.5);
    let lamp = Light::Point(PointLight {
        position: lamp_at.as_vec3(),
        intensity: 4.0,
        ..PointLight::default()
    });
    let point = brdf::seen_point(&camera, (SIZE, SIZE), center, 0.0);
    let to_camera = brdf::to_camera(&camera, (SIZE, SIZE), center, 0.0);
    let from_lamp = brdf::reflected(WHITE, DVec3::Z, (lamp_at - point).normalize(), to_camera)
        .map(|channel| channel * 4.0 / (lamp_at - point).length_squared());
    // What each directional light alone adds there.
    let reflected = |sun| floor_shows(&camera, center, sun, 1.0).map(|c| c - SKY);
    let no_shadows = |light: DirectionalLight| DirectionalLight {
        casts_shadows: false,
        ..light
    };
    for (lights, shadowed, draws, case) in [
        (
            [
                lamp,
                Light::Directional(slanted.0),
                Light::Directional(above.0),
            ],
            true,
            2,
            "a point light, then the slanted light, then the one from above",
        ),
        (
            [
                lamp,
                Light::Directional(above.0),
                Light::Directional(slanted.0),
            ],
            false,
            2,
            "a point light, then the light from above, then the slanted one",
        ),
        (
            [
                lamp,
                Light::Directional(no_shadows(above.0)),
                Light::Directional(slanted.0),
            ],
            true,
            2,
            "the light from above casting no shadows, then the slanted one",
        ),
        (
            [
                lamp,
                Light::Directional(no_shadows(slanted.0)),
                Light::Directional(no_shadows(above.0)),
            ],
            false,
            0,
            "no directional light casting shadows",
        ),
    ] {
        let (image, stats) = setup.draw(camera, &lights, &squares);
        assert_eq!(stats.shadow_draw_calls, draws, "{case}");
        let lit = if shadowed { 0.0 } else { 1.0 };
        let (from_slanted, from_above) = (reflected(slanted), reflected(above));
        let light: [f64; 3] = std::array::from_fn(|channel| {
            from_lamp[channel] + from_above[channel] + lit * from_slanted[channel] + SKY
        });
        assert_shows(&image, center, light, case);
    }
    // Without a directional light, no shadow pass.
    let (image, stats) = setup.draw(camera, &[lamp], &squares);
    assert_eq!((stats.shadow_draw_calls, stats.shadow_gpu_time), (0, None));
    assert_shows(
        &image,
        center,
        from_lamp.map(|channel| channel + SKY),
        "a point light alone",
    );
}

#[test]
fn a_masked_material_casts_no_shadow_where_it_discards() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    // The square's left half is transparent, its right half opaque (a 2 × 1 texture read nearest).
    let texture = setup
        .gpu
        .create_texture(
            "half",
            2,
            1,
            TextureFormat::Rgba8UnormSrgb,
            &[255, 255, 255, 0, 255, 255, 255, 255],
        )
        .expect("the GPU has memory for it");
    let nearest = setup
        .gpu
        .create_sampler(
            "nearest",
            &slime_gfx::SamplerDescriptor {
                mag_filter: slime_gfx::Filter::Nearest,
                min_filter: slime_gfx::Filter::Nearest,
                address_u: slime_gfx::AddressMode::ClampToEdge,
                address_v: slime_gfx::AddressMode::ClampToEdge,
            },
        )
        .expect("the GPU has memory for it");
    let half = setup
        .renderer
        .create_material(
            &setup.gpu,
            "half",
            &MaterialData {
                base_color_texture: Some(slime_render::SampledTexture {
                    texture: &texture,
                    sampler: &nearest,
                }),
                alpha_mode: AlphaMode::Mask { cutoff: 0.5 },
                ..MaterialData::default()
            },
        )
        .expect("the GPU has memory for it");
    let side = (SHADOW_MAP_SIZE / 8.0) as f32;
    let camera = camera_above(1.5, 0.0);
    let sun = sun(45.0, 0.4);
    let (image, _) = setup.draw(
        camera,
        &[Light::Directional(sun.0)],
        &[
            (&white, floor(side)),
            (&half, blocker(Vec3::new(0.0, 0.0, 1.5), false)),
        ],
    );
    // The shadow spans x from 1 to 2: the transparent half's from 1 to 1.5, lit; the opaque
    // half's from 1.5 to 2, dark. Pixels 27 and 37 see x = 1.24 and 1.78.
    for (pixel, lit, case) in [
        ((27, 32), 1.0, "under the transparent half"),
        ((37, 32), 0.0, "under the opaque half"),
    ] {
        assert_shows(&image, pixel, floor_shows(&camera, pixel, sun, lit), case);
    }
}
