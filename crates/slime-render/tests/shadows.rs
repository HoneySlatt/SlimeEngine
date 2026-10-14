//! Shadows of the first directional light that casts shadows (ADR-0038, ADR-0050), checked by
//! reading back an `Rgba16Float` output and comparing pixels with what `common/brdf.rs` predicts,
//! through the tone curve of `common/tone_curve.rs`, with the share of light that the shadow lets
//! through computed here from the geometry: a square above a floor darkens a square of the floor,
//! its edges blended by the filter; a lit floor, or a lit sphere, does not shadow itself; only the
//! first directional light that casts shadows casts them; a masked material casts none where it
//! discards; an instance that casts no shadow is left out of the map, and what lies beyond the map
//! is lit. Runs on whatever adapter the machine offers.
//!
//! The floor is the plane z = 0, facing +Z, and "above" is along +Z. The shadow map covers what the
//! camera sees up to the light's shadow distance, its texels on a grid fixed in the world along the
//! light's axes (`light_axes`, `texel_width`): a caster's shadow is the texels whose centres it
//! covers, and each point of the floor is looked up off it along its normal (`looked_up`), as the
//! renderer does, so that the expected shadow is exact.

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

/// The renderer, a 64 × 64 `Rgba16Float` output, a white matte square drawn two units wide,
/// facing +Z, centred on the origin, and its material.
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
            .create_mesh(&gpu, "square", &MeshData::square())
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
        let casting: Vec<_> = squares
            .iter()
            .map(|&(material, transform)| (material, transform, true))
            .collect();
        self.draw_casting(camera, lights, &casting)
    }

    /// As `draw`, each square casting shadows or not as `squares` says.
    fn draw_casting(
        &mut self,
        camera: Camera,
        lights: &[Light],
        squares: &[(&Material, Mat4, bool)],
    ) -> (FloatImage, RenderStats) {
        let instances: Vec<Instance<'_>> = squares
            .iter()
            .map(|&(material, transform, casts_shadows)| {
                // `MeshData::square` is one unit wide: drawn two.
                let mut instance = Instance::new(
                    &self.square,
                    material,
                    transform * Mat4::from_scale(Vec3::splat(2.0)),
                );
                instance.casts_shadows = casts_shadows;
                instance
            })
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
        ..DirectionalLight::default()
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

/// The share of a shadow at `u`, in texels along one of the map's axes, counted from the world's
/// origin, where a caster covers from `from` to `to`: the texels whose centres it covers hold it,
/// and the filter averages three comparisons one texel apart, each blending the two texels nearest
/// to it. Where `from` and `to` are texel edges, the shadow ramps linearly over three texels
/// centred on each.
fn shadowed_along(u: f64, (from, to): (f64, f64)) -> f64 {
    let covered = |texel: f64| f64::from(u8::from(from < texel + 0.5 && texel + 0.5 < to));
    [-1.0, 0.0, 1.0]
        .iter()
        .map(|offset| {
            let at = u + offset - 0.5;
            let (texel, weight) = (at.floor(), at - at.floor());
            (1.0 - weight) * covered(texel) + weight * covered(texel + 1.0)
        })
        .sum::<f64>()
        / 3.0
}

/// The axes of the shadow map of a light travelling along `direction`, across it, as the renderer
/// takes them: the x and y of glam's view along the light.
fn light_axes(direction: Vec3) -> [DVec3; 2] {
    let forward = direction.as_dvec3().normalize();
    let x = forward.cross(forward.any_orthogonal_vector()).normalize();
    [x, x.cross(forward)]
}

/// The width of a texel of the shadow map, in metres, for `camera` drawing the square output, and
/// a light that casts shadows up to `distance` (`DirectionalLight::shadow_distance`): 1/2048 of the
/// side of the square around the smallest sphere, centred on the view's axis, that holds the
/// corners of the camera's near plane and of the plane at `distance`.
fn texel_width(camera: &Camera, distance: f32) -> f64 {
    let (near, far) = (f64::from(camera.near), f64::from(distance));
    let tan = (f64::from(camera.fov_y) / 2.0).tan();
    let corner = 2.0 * tan * tan;
    let along = ((far + near) * (corner + 1.0) / 2.0).min(far);
    let radius = (far * far * corner + (far - along) * (far - along)).sqrt();
    2.0 * radius / SHADOW_MAP_SIZE
}

/// Where the shadow map is read for the floor's point `point` under a light arriving from
/// `to_light`: the point moved along the floor's normal towards the light by 3 texels of `texel`
/// metres times the sine of the light's incidence (`NORMAL_OFFSET_TEXELS` in `shader.wgsl`).
fn looked_up(point: DVec3, to_light: DVec3, texel: f64) -> DVec3 {
    let cosine = DVec3::Z.dot(to_light);
    point + DVec3::Z * (3.0 * texel * (1.0 - cosine * cosine).sqrt())
}

/// What `squares`, given as transforms of the unit square, cover along each of the map's `axes`,
/// in texels: the light-space box around their corners.
fn covered_along(axes: [DVec3; 2], texel: f64, squares: &[Mat4]) -> [(f64, f64); 2] {
    axes.map(|axis| {
        let along: Vec<f64> = squares
            .iter()
            .flat_map(|square| {
                [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .map(|(x, y)| square.transform_point3(Vec3::new(x, y, 0.0)))
            })
            .map(|corner| corner.as_dvec3().dot(axis) / texel)
            .collect();
        let min = along.iter().copied().fold(f64::INFINITY, f64::min);
        let max = along.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (min, max)
    })
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
    // A floor 256 m wide, and a square 1 m wide, 1.5 m above it and off its center, under a light
    // 45° from the normal that casts shadows up to 150 m from the camera: the map covers about
    // 250 m around it, in texels of about 12 cm. The square shadows x from 1 to 2 and y from 0.25 to
    // 1.25. The camera above the shadow's center sees 3.5 m of floor, 54 mm per pixel, the penumbra
    // over 7 pixels, and not the square.
    let side = 256.0;
    let (light, to_light) = sun(45.0, 0.4);
    let sun = (
        DirectionalLight {
            shadow_distance: 150.0,
            ..light
        },
        to_light,
    );
    let camera = camera_above(1.5, 0.75);
    let texel = texel_width(&camera, sun.0.shadow_distance);
    let axes = light_axes(sun.0.direction);
    // Every face casts, whichever way it faces the light.
    for (facing_down, case) in [(false, "facing the light"), (true, "facing away from it")] {
        let square = blocker(Vec3::new(0.0, 0.75, 1.5), facing_down);
        // Its edges run along the map's axes: its shadow is the box of its corners there.
        let covered = covered_along(axes, texel, &[square]);
        let (image, stats) = setup.draw(
            camera,
            &[Light::Directional(sun.0)],
            &[(&white, floor(side)), (&white, square)],
        );
        assert_eq!(
            (stats.draw_calls, stats.shadow_draw_calls),
            (2, 2),
            "{case}"
        );
        let mut dark = 0;
        let (elsewhere, in_penumbra) = largest_differences(&image, |pixel| {
            let point = brdf::seen_point(&camera, (SIZE, SIZE), pixel, 0.0);
            let looked_up = looked_up(point, sun.1, texel);
            let [u, v] = axes.map(|axis| looked_up.dot(axis) / texel);
            let shadowed = shadowed_along(u, covered[0]) * shadowed_along(v, covered[1]);
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
        // About 11 × 11 pixels: 1 m less the penumbra's 37 cm, of 54 mm pixels.
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
    // and moved 10 km away, where the shadow map's grid meets it elsewhere across a texel: the floor
    // facing the light, without any slope, compares its own depths computed two ways.
    let canonical = Camera {
        eye: Vec3::new(0.3, 0.2, 2.0),
        target: Vec3::new(0.1, -0.1, 0.0),
        ..Camera::default()
    };
    let placements = [
        Mat4::IDENTITY,
        Mat4::from_translation(Vec3::new(40.0, -20.0, 10.0))
            * Mat4::from_euler(EulerRot::XYZ, 0.7, -0.4, 1.9),
        Mat4::from_translation(Vec3::new(10_000.0, 0.0, -10_000.0)),
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

#[test]
fn an_instance_that_casts_no_shadow_is_left_out_and_what_lies_beyond_the_map_is_lit() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    // A floor 1 km wide that casts no shadow, and a strip 1 m wide and 100 m long that does, 1 m
    // above it along +x, lit from straight above, the light casting shadows up to 5 m from a camera
    // low beside the strip that looks along it. The map covers the strip's first 8 m: there, the
    // strip's shadow darkens the floor; beyond, the floor under the strip is lit, as everything
    // beyond the map is, though the map's far edge holds the strip's shadow.
    let (light, to_light) = sun(0.0, 0.4);
    let sun = (
        DirectionalLight {
            shadow_distance: 5.0,
            ..light
        },
        to_light,
    );
    let camera = Camera {
        eye: Vec3::new(0.0, -2.0, 0.9),
        target: Vec3::new(10.0, 0.0, 0.0),
        ..Camera::default()
    };
    let strip = Mat4::from_translation(Vec3::new(50.0, 0.0, 1.0))
        * Mat4::from_scale(Vec3::new(50.0, 0.5, 1.0));
    let (image, stats) = setup.draw_casting(
        camera,
        &[Light::Directional(sun.0)],
        &[(&white, floor(1_000.0), false), (&white, strip, true)],
    );
    assert_eq!((stats.draw_calls, stats.shadow_draw_calls), (2, 1));
    let forward = (camera.target - camera.eye).as_dvec3();
    let (mut near, mut far) = (0, 0);
    let (largest, _) = largest_differences(&image, |pixel| {
        let point = brdf::seen_point(&camera, (SIZE, SIZE), pixel, 0.0);
        let on_the_floor = point.x.abs() < 400.0 && point.y.abs() < 400.0;
        if (point - camera.eye.as_dvec3()).dot(forward) <= 0.0 || !on_the_floor {
            // Above the horizon, or beyond the floor's edge, 500 m away: the sky.
            return ([0.0; 3], true);
        }
        let under = point.y.abs() < 0.4;
        let lit = if under && (1.0..3.0).contains(&point.x) {
            near += 1;
            0.0
        } else if (under && point.x > 10.0) || point.y.abs() > 0.6 {
            far += usize::from(under);
            1.0
        } else {
            // Near the strip's shadow's edges, or the map's.
            return ([0.0; 3], true);
        };
        (floor_shows(&camera, pixel, sun, lit), false)
    });
    assert!(near > 20 && far > 5, "{near} pixels near, {far} far");
    assert!(largest <= 2e-3, "off by {largest}");

    // The strip casting no shadow either: no shadow, and no shadow pass.
    let (image, stats) = setup.draw_casting(
        camera,
        &[Light::Directional(sun.0)],
        &[(&white, floor(1_000.0), false), (&white, strip, false)],
    );
    assert_eq!((stats.shadow_draw_calls, stats.shadow_gpu_time), (0, None));
    let (largest, _) = largest_differences(&image, |pixel| {
        let point = brdf::seen_point(&camera, (SIZE, SIZE), pixel, 0.0);
        let seen = (point - camera.eye.as_dvec3()).dot(forward) > 0.0
            && point.x.abs() < 400.0
            && point.y.abs() < 400.0;
        (floor_shows(&camera, pixel, sun, 1.0), !seen)
    });
    assert!(largest <= 2e-3, "off by {largest}");
}

#[test]
fn a_lit_sphere_does_not_shadow_itself() {
    let mut setup = Setup::new();
    // A sphere 40 cm wide of 16 × 24 facets, each about 5 cm, as the samples' small spheres, under
    // a light that casts shadows up to 30 m from the camera: texels of about 3 cm, so that the
    // filter reaches across facets that slope otherwise. Where the sphere faces the light, it is
    // drawn as without shadows: a convex surface casts none on itself there.
    let mesh = setup
        .renderer
        .create_mesh(&setup.gpu, "sphere", &MeshData::sphere(16, 24))
        .expect("the sphere is valid");
    let radius = 0.2;
    // `MeshData::sphere` is one metre across.
    let transform = Mat4::from_scale(Vec3::splat(2.0 * radius as f32));
    let to_light = DVec3::new(0.5, 0.8, 0.6).normalize();
    let light = DirectionalLight {
        direction: (-to_light).as_vec3(),
        illuminance: 3.0,
        shadow_distance: 30.0,
        ..DirectionalLight::default()
    };
    let camera = Camera {
        eye: Vec3::new(0.3, 0.6, 0.9),
        target: Vec3::ZERO,
        ..Camera::default()
    };
    let mut draw = |light: DirectionalLight| {
        let instances = [Instance::new(&mesh, &setup.white, transform)];
        let lights = [Light::Directional(light)];
        let mut scene = Scene::new(camera, &instances);
        scene.lights = &lights;
        scene.environment = Some(&setup.sky);
        setup
            .renderer
            .render(&setup.gpu, &setup.output, &scene)
            .expect("the frame is drawn");
        setup
            .gpu
            .read_float_render_target(&setup.output)
            .expect("the image is read back")
    };
    let shadowed = draw(light);
    let unshadowed = draw(DirectionalLight {
        casts_shadows: false,
        ..light
    });
    let inverse = camera.view_projection(1.0).as_dmat4().inverse();
    let eye = camera.eye.as_dvec3();
    let (mut facing, mut largest) = (0, 0.0f32);
    for y in 0..SIZE {
        for x in 0..SIZE {
            // Where the view ray through the pixel meets the smooth sphere, and its normal there.
            let ndc = DVec3::new(
                (f64::from(x) + 0.5) / f64::from(SIZE) * 2.0 - 1.0,
                1.0 - (f64::from(y) + 0.5) / f64::from(SIZE) * 2.0,
                0.5,
            );
            let ray = (inverse.project_point3(ndc) - eye).normalize();
            let along = -eye.dot(ray);
            let squared = radius * radius - (eye + ray * along).length_squared();
            if squared <= 0.0 {
                continue;
            }
            let normal = (eye + ray * (along - squared.sqrt())).normalize();
            // Clear of the silhouette and of the terminator, where the facets part from the sphere.
            if normal.dot(to_light) < 0.3 || normal.dot(-ray) < 0.3 {
                continue;
            }
            facing += 1;
            let start = ((y * SIZE + x) * 4) as usize;
            for channel in 0..3 {
                let difference =
                    (shadowed.pixels[start + channel] - unshadowed.pixels[start + channel]).abs();
                largest = largest.max(difference);
            }
        }
    }
    assert!(facing > 200, "{facing} pixels facing the light");
    assert!(largest <= 2e-3, "off by {largest}");
}
