//! Shadows of the first directional light that casts shadows (ADR-0038, ADR-0050), checked by
//! reading back an `Rgba16Float` output and comparing pixels with what `common/brdf.rs` predicts,
//! through the tone curve of `common/tone_curve.rs`, with the share of light that the shadow lets
//! through computed here from the geometry: a square above a floor darkens a square of the floor,
//! its edges blended by the filter; each cascade shadows what it holds and hands over to the next
//! across its band, and shadows fade out beyond the last; a lit floor, or a lit sphere, does not
//! shadow itself; only the first directional light that casts shadows casts them; a masked material
//! casts none where it discards; an instance that casts no shadow is left out of the cascades, and
//! what lies beyond them is lit. Runs on whatever adapter the machine offers.
//!
//! The floor is the plane z = 0, facing +Z, and "above" is along +Z. The light's four cascades are
//! modelled here apart from the renderer (`cascades`, ADR-0064, ADR-0065): each covers a square
//! across the light, its texels on a grid fixed in the world along the light's axes
//! (`light_axes`); a caster's shadow is the texels whose centres it covers (`lit_under`); and each
//! point of the floor is looked up off it along its normal (`looked_up`) in the finest cascade that
//! holds it, and in the next ones across its band (`held`, `through_cascades`), as the renderer
//! does, so that the expected shadow is exact.

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

/// The width of a cascade's tile, in texels: `CASCADE_SIZE` in `crates/slime-render/src/shadow.rs`.
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
/// and Castaño's filter (ADR-0063) weighs three comparisons, each blending the two texels nearest
/// to it, by weights that add up to 12, placed from the centre of the texel before the corner
/// between texels nearest to `u`, as `compared` in `shader.wgsl` does, computed here apart.
fn shadowed_along(u: f64, (from, to): (f64, f64)) -> f64 {
    let covered = |texel: f64| f64::from(u8::from(from < texel + 0.5 && texel + 0.5 < to));
    let blended = |at: f64| {
        let at = at - 0.5;
        let (texel, weight) = (at.floor(), at - at.floor());
        (1.0 - weight) * covered(texel) + weight * covered(texel + 1.0)
    };
    let corner = (u + 0.5).floor();
    let s = u + 0.5 - corner;
    let first = corner - 0.5;
    let weights = [4.0 - 3.0 * s, 7.0, 1.0 + 3.0 * s];
    let places = [
        (3.0 - 2.0 * s) / weights[0] - 2.0,
        (3.0 + s) / weights[1],
        s / weights[2] + 2.0,
    ];
    weights
        .iter()
        .zip(places)
        .map(|(weight, place)| weight * blended(first + place))
        .sum::<f64>()
        / 12.0
}

/// The axes of the shadow map of a light travelling along `direction`, across it, as the renderer
/// takes them: the x and y of glam's view along the light.
fn light_axes(direction: Vec3) -> [DVec3; 2] {
    let forward = direction.as_dvec3().normalize();
    let x = forward.cross(forward.any_orthogonal_vector()).normalize();
    [x, x.cross(forward)]
}

/// A cascade of the shadowed light, as `CascadeView` in `crates/slime-render/src/shadow.rs` places
/// it: the width of its texels, and its square across the light, its center and half its side, in
/// metres along the light's axes (`light_axes`) from the world's origin.
struct Cascade {
    texel: f64,
    center: [f64; 2],
    half_side: f64,
}

/// The four cascades of a light whose axes are `axes`, casting shadows up to `distance` from
/// `camera` drawing the square output (ADR-0064): the slices of the view end at 1, 3, 7 and 15
/// fifteenths of `distance`, the first starting at the near plane; each cascade covers the square
/// around the smallest sphere, centred on the view's axis, that holds the corners of its slice's
/// two planes, 2048 texels across, its center moved to a multiple of a texel from the world's
/// origin along each axis.
fn cascades(camera: &Camera, distance: f32, axes: [DVec3; 2]) -> [Cascade; 4] {
    let end = |index: usize| f64::from(distance) * ((2u32 << index) - 1) as f64 / 15.0;
    let near_plane = f64::from(camera.near);
    let tan = (f64::from(camera.fov_y) / 2.0).tan();
    let corner = 2.0 * tan * tan;
    let eye = camera.eye.as_dvec3();
    let forward = (camera.target.as_dvec3() - eye).normalize();
    std::array::from_fn(|index| {
        let near = if index == 0 {
            near_plane
        } else {
            end(index - 1).max(near_plane)
        };
        let far = end(index).max(near);
        let along = ((far + near) * (corner + 1.0) / 2.0).min(far);
        let half_side = (far * far * corner + (far - along) * (far - along)).sqrt();
        let texel = 2.0 * half_side / SHADOW_MAP_SIZE;
        let center = axes.map(|axis| ((eye + forward * along).dot(axis) / texel).round() * texel);
        Cascade {
            texel,
            center,
            half_side,
        }
    })
}

/// How much of the light's share `cascade` gives at `point`, where the point is looked up in it:
/// all of it deeper inside its square than its band, a tenth of its width; none within 4 texels of
/// its edges or beyond them; and in between, a share that falls across the band
/// (`CASCADE_MARGIN_TEXELS` and `CASCADE_BAND` in `shader.wgsl`).
fn held(cascade: &Cascade, axes: [DVec3; 2], point: DVec3) -> f64 {
    let across = (0..2)
        .map(|axis| ((point.dot(axes[axis]) - cascade.center[axis]) / cascade.half_side).abs())
        .fold(0.0, f64::max);
    let margin = 4.0 * cascade.texel / cascade.half_side;
    ((1.0 - margin - across) / 0.2).clamp(0.0, 1.0)
}

/// The share of the light, arriving from `to_light`, that reaches the floor's point `point`
/// through `cascades`, as `light_reaching` in `shader.wgsl` puts them together: each cascade that
/// holds the point, the finest first, gives what `lit` says reaches the point looked up in it, for
/// the part of the light it holds of what the ones before it left; what none gives reaches the
/// point. Also whether the point lies in a cascade's band.
fn through_cascades(
    cascades: &[Cascade; 4],
    axes: [DVec3; 2],
    point: DVec3,
    to_light: DVec3,
    lit: impl Fn(&Cascade, DVec3) -> f64,
) -> (f64, bool) {
    let (mut reached, mut left, mut in_band) = (0.0, 1.0, false);
    for cascade in cascades {
        if left == 0.0 {
            break;
        }
        let looked_up = looked_up(point, to_light, cascade.texel);
        let held = held(cascade, axes, looked_up);
        if held > 0.0 {
            reached += left * held * lit(cascade, looked_up);
            left *= 1.0 - held;
            in_band |= held < 1.0;
        }
    }
    (reached + left, in_band)
}

/// The share of the light that reaches `point`, looked up in `cascade`, under `squares` that cast
/// shadows, their edges along the light's `axes`: their shadows are the boxes of their corners
/// there, filtered (`shadowed_along`).
fn lit_under(cascade: &Cascade, axes: [DVec3; 2], squares: &[Mat4], point: DVec3) -> f64 {
    let covered = covered_along(axes, cascade.texel, squares);
    let [u, v] = axes.map(|axis| point.dot(axis) / cascade.texel);
    1.0 - shadowed_along(u, covered[0]) * shadowed_along(v, covered[1])
}

/// Where a cascade is read for the floor's point `point` under a light arriving from `to_light`:
/// the point moved along the floor's normal, its triangles' too, towards the light by 4.5 texels of
/// `texel` metres times the sine of the light's incidence (`NORMAL_OFFSET_TEXELS` in
/// `shader.wgsl`).
fn looked_up(point: DVec3, to_light: DVec3, texel: f64) -> DVec3 {
    let cosine = DVec3::Z.dot(to_light);
    point + DVec3::Z * (4.5 * texel * (1.0 - cosine * cosine).sqrt())
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

/// Where the GPU samples a pixel, four times (`slime_gfx::MULTISAMPLE_COUNT`): the standard pattern
/// of Vulkan and Direct3D, in pixels from the pixel's top left corner.
const SAMPLES: [(f64, f64); 4] = [
    (0.375, 0.125),
    (0.875, 0.375),
    (0.125, 0.625),
    (0.625, 0.875),
];

/// The largest difference, in any channel, between each pixel of `image` and the tone-mapped
/// light that `expected` gives for it, kept apart for the pixels it marks as in a penumbra, and
/// leaving out those it gives nothing for.
fn largest_differences(
    image: &FloatImage,
    mut expected: impl FnMut((u32, u32)) -> Option<([f64; 3], bool)>,
) -> (f32, f32) {
    let (mut elsewhere, mut in_penumbra) = (0.0f32, 0.0f32);
    for y in 0..image.height {
        for x in 0..image.width {
            let Some((light, penumbra)) = expected((x, y)) else {
                continue;
            };
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
fn a_square_above_a_floor_darkens_a_square_of_the_floor_blended_over_six_texels() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    // A floor 256 m wide, and a square 1 m wide, 1.5 m above it and off its center, under a light
    // 45° from the normal that casts shadows up to 2 250 m from the camera: its first cascade, up
    // to 150 m, covers about 250 m around the camera, in texels of about 12 cm, and holds all the
    // floor the camera sees. The square shadows x from 1 to 2 and y from 0.25 to 1.25. The camera
    // above the shadow's center sees 3.5 m of floor, 54 mm per pixel, the penumbra over 13 pixels
    // (6 texels, ADR-0063), and not the square.
    let side = 256.0;
    let (light, to_light) = sun(45.0, 0.4);
    let sun = (
        DirectionalLight {
            shadow_distance: 2250.0,
            ..light
        },
        to_light,
    );
    let camera = camera_above(1.5, 0.75);
    let axes = light_axes(sun.0.direction);
    let cascades = cascades(&camera, sun.0.shadow_distance, axes);
    // Every face casts, whichever way it faces the light.
    for (facing_down, case) in [(false, "facing the light"), (true, "facing away from it")] {
        let square = blocker(Vec3::new(0.0, 0.75, 1.5), facing_down);
        let (image, stats) = setup.draw(
            camera,
            &[Light::Directional(sun.0)],
            &[(&white, floor(side)), (&white, square)],
        );
        // Both reach every cascade: each is drawn in all four.
        assert_eq!(
            (stats.draw_calls, stats.shadow_draw_calls),
            (2, 8),
            "{case}"
        );
        let (mut dark, mut beyond_the_first) = (0, 0);
        let (elsewhere, in_penumbra) = largest_differences(&image, |pixel| {
            let point = brdf::seen_point(&camera, (SIZE, SIZE), pixel, 0.0);
            // Its edges run along the cascades' axes: its shadow is the box of its corners there.
            let (lit, _) = through_cascades(&cascades, axes, point, sun.1, |cascade, at| {
                lit_under(cascade, axes, &[square], at)
            });
            dark += usize::from(lit == 0.0);
            let first = &cascades[0];
            beyond_the_first +=
                usize::from(held(first, axes, looked_up(point, sun.1, first.texel)) < 1.0);
            Some((
                floor_shows(&camera, pixel, sun, lit),
                lit > 0.0 && lit < 1.0,
            ))
        });
        eprintln!(
            "adapter `{}`, a square {case}: {dark} pixels in full shadow; largest difference \
             {elsewhere} outside the penumbra, {in_penumbra} in it",
            setup.gpu.adapter_name()
        );
        assert_eq!(
            beyond_the_first, 0,
            "{case}: the first cascade holds every pixel"
        );
        // About 5 × 5 pixels: 1 m less the penumbra's 72 cm, of 54 mm pixels.
        assert!(dark > 15, "{case}: {dark} pixels in full shadow");
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
                Some((
                    floor_shows(&canonical, pixel, (light, to_light), 1.0),
                    false,
                ))
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
    // At the center of the image, the floor at (1.5, 0, 0): half a metre inside the shadow of a
    // light 45° from the normal, outside the shadow of one from straight above, which falls under
    // the square, out of view. A point light sees it behind the square.
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
    // The floor and the square reach every cascade of the light that casts shadows: 8 draws.
    for (lights, shadowed, draws, case) in [
        (
            [
                lamp,
                Light::Directional(slanted.0),
                Light::Directional(above.0),
            ],
            true,
            8,
            "a point light, then the slanted light, then the one from above",
        ),
        (
            [
                lamp,
                Light::Directional(above.0),
                Light::Directional(slanted.0),
            ],
            false,
            8,
            "a point light, then the light from above, then the slanted one",
        ),
        (
            [
                lamp,
                Light::Directional(no_shadows(above.0)),
                Light::Directional(slanted.0),
            ],
            true,
            8,
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
    // Shadows up to 20 m from the camera: texels of a few millimetres there, so that the filter of
    // a point under the middle of a half, and its normal offset, reach neither of the half's edges.
    let (light, to_light) = sun(45.0, 0.4);
    let sun = (
        DirectionalLight {
            shadow_distance: 20.0,
            ..light
        },
        to_light,
    );
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

/// Whether each sample of `pixel` sees the plane z = 0 in front of `camera`, within `metres` of the
/// origin along x and y: near the horizon, the samples of one pixel see points hundreds of metres
/// apart.
fn within_at_every_sample(camera: &Camera, pixel: (u32, u32), metres: f64) -> bool {
    let forward = (camera.target - camera.eye).as_dvec3();
    SAMPLES.into_iter().all(|(dx, dy)| {
        let at = (f64::from(pixel.0) + dx, f64::from(pixel.1) + dy);
        let point = brdf::seen_point_at(camera, (SIZE, SIZE), at, 0.0);
        (point - camera.eye.as_dvec3()).dot(forward) > 0.0
            && point.x.abs() < metres
            && point.y.abs() < metres
    })
}

#[test]
fn an_instance_that_casts_no_shadow_is_left_out_and_what_lies_beyond_the_map_is_lit() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    // A floor 1 km wide that casts no shadow, and a strip 1 m wide and 100 m long that does, 1 m
    // above it along +x, lit from straight above, the light casting shadows up to 5 m from a camera
    // low beside the strip that looks along it. The cascades cover the strip up to about 9 m:
    // there, the strip's shadow darkens the floor; beyond, the floor under the strip is lit, as
    // everything beyond the cascades is, though the last one's far edge holds the strip's shadow.
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
    // The strip, 2 m beside the camera, reaches the two farther cascades only.
    assert_eq!((stats.draw_calls, stats.shadow_draw_calls), (2, 2));
    let (mut near, mut far) = (0, 0);
    let (largest, _) = largest_differences(&image, |pixel| {
        let point = brdf::seen_point(&camera, (SIZE, SIZE), pixel, 0.0);
        if !within_at_every_sample(&camera, pixel, 400.0) {
            // Above the horizon, or beyond the floor's edge, 500 m away: the sky, for some of
            // the pixel's samples at least.
            return None;
        }
        let under = point.y.abs() < 0.4;
        let lit = if under && (1.0..3.0).contains(&point.x) {
            near += 1;
            0.0
        } else if (under && point.x > 10.0) || point.y.abs() > 0.6 {
            far += usize::from(under);
            1.0
        } else {
            // Near the strip's shadow's edges, or the last cascade's.
            return None;
        };
        Some((floor_shows(&camera, pixel, sun, lit), false))
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
        within_at_every_sample(&camera, pixel, 400.0)
            .then(|| (floor_shows(&camera, pixel, sun, 1.0), false))
    });
    assert!(largest <= 2e-3, "off by {largest}");
}

#[test]
fn a_lit_sphere_does_not_shadow_itself() {
    let mut setup = Setup::new();
    // A sphere 40 cm wide of 16 × 24 facets, each about 5 cm, as the samples' small spheres, 1.1 m
    // from the camera, under a light that casts shadows up to 550 m from it, then up to 30 m: the
    // first cascade holds the sphere, in texels of about 3 cm, so that the filter reaches across
    // facets that slope otherwise, then of about 2 mm, much finer than the facets, which the
    // smooth normal leans across (ADR-0062). Where the sphere faces the light, it is drawn as
    // without shadows: a convex surface casts none on itself there.
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
    let unshadowed = draw(DirectionalLight {
        casts_shadows: false,
        ..light
    });
    let inverse = camera.view_projection(1.0).as_dmat4().inverse();
    let eye = camera.eye.as_dvec3();
    let mut facing = Vec::new();
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
            if normal.dot(to_light) >= 0.3 && normal.dot(-ray) >= 0.3 {
                facing.push(((y * SIZE + x) * 4) as usize);
            }
        }
    }
    assert!(
        facing.len() > 200,
        "{} pixels facing the light",
        facing.len()
    );
    for shadow_distance in [550.0, 30.0] {
        let shadowed = draw(DirectionalLight {
            shadow_distance,
            ..light
        });
        let largest = facing
            .iter()
            .flat_map(|&start| start..start + 3)
            .map(|channel| (shadowed.pixels[channel] - unshadowed.pixels[channel]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            largest <= 2e-3,
            "shadows up to {shadow_distance} m: off by {largest}"
        );
    }
}

#[test]
fn each_cascade_shadows_what_it_holds_and_hands_over_across_its_band() {
    let mut setup = Setup::new();
    let white = setup.white.clone();
    // A floor 120 m wide, and a sheet 500 m wide, 3 m above it, which shadows the floor on x below
    // 0.4 m, under a light that casts shadows up to 20 m from the camera. A camera 1.5 m above the
    // floor looks along +y, a little down, at the floor up to 55 m away, the shadow's edge running
    // away from it along the view: the finest cascade that holds a point of the floor shadows it,
    // each in its own texels, and hands over to the next across its band. A camera 30 m above the
    // floor looks straight down at it, beyond the last cascade, whose square ends about 16 m on
    // either side along y: there, the shadow fades out. The sheet faces away from each camera,
    // which culls it. The floor, lit beside the shadow, does not shadow itself in any cascade.
    let low = Camera {
        eye: Vec3::new(0.0, 0.0, 1.5),
        target: Vec3::new(0.0, 10.0, 0.0),
        up: Vec3::Z,
        ..Camera::default()
    };
    let high = Camera {
        eye: Vec3::new(0.0, 0.0, 30.0),
        target: Vec3::ZERO,
        ..Camera::default()
    };
    let (mut first_holding, mut beyond) = ([0; 4], 0);
    for (camera, degrees) in [(low, 30.0), (low, 70.0), (high, 30.0)] {
        let (light, to_light) = sun(degrees, 0.4);
        let sun = (
            DirectionalLight {
                shadow_distance: 20.0,
                ..light
            },
            to_light,
        );
        let edge = 0.4 - 3.0 * degrees.to_radians().tan();
        // Facing away from the camera, which culls it.
        let away = if camera.eye.z > 3.0 {
            Mat4::from_rotation_x(PI)
        } else {
            Mat4::IDENTITY
        };
        let sheet =
            Mat4::from_translation(Vec3::new(edge as f32 - 250.0, 0.0, 3.0)) * away * floor(500.0);
        let axes = light_axes(sun.0.direction);
        let cascades = cascades(&camera, sun.0.shadow_distance, axes);
        let (image, _) = setup.draw(
            camera,
            &[Light::Directional(sun.0)],
            &[(&white, floor(120.0)), (&white, sheet)],
        );
        let mut in_bands = 0;
        let (mut elsewhere, mut in_penumbra) = (0.0f32, 0.0f32);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let pixel = (x, y);
                if !within_at_every_sample(&camera, pixel, 55.0) {
                    continue;
                }
                let lit_at = |(dx, dy): (f64, f64)| {
                    let at = (f64::from(x) + 0.5 + dx, f64::from(y) + 0.5 + dy);
                    let point = brdf::seen_point_at(&camera, (SIZE, SIZE), at, 0.0);
                    through_cascades(&cascades, axes, point, sun.1, |cascade, at| {
                        lit_under(cascade, axes, &[sheet], at)
                    })
                };
                let (lit, in_band) = lit_at((0.0, 0.0));
                let point = brdf::seen_point(&camera, (SIZE, SIZE), pixel, 0.0);
                let first = cascades.iter().position(|cascade| {
                    held(cascade, axes, looked_up(point, sun.1, cascade.texel)) > 0.0
                });
                match first {
                    Some(index) => first_holding[index] += 1,
                    None => beyond += 1,
                }
                in_bands += usize::from(in_band);
                // A penumbra a fifth of a pixel wide, as each cascade's is here, turns the least
                // error in the point a pixel is shaded at into a large one in its light: GPUs place
                // a triangle on the pixels' grid to 1/256 of a pixel, and interpolate across it.
                // Where the light changes within the pixel, it may be any the model gives within
                // 1/100 of a pixel of the pixel's centre.
                let partly = lit > 0.0 && lit < 1.0;
                let near = 0.01;
                let lits = if partly {
                    [
                        (0.0, 0.0),
                        (near, 0.0),
                        (-near, 0.0),
                        (0.0, near),
                        (0.0, -near),
                    ]
                    .map(|shift| lit_at(shift).0)
                    .to_vec()
                } else {
                    vec![lit]
                };
                let lowest = lits.iter().copied().fold(f64::INFINITY, f64::min);
                let highest = lits.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let shown = |lit| {
                    tone_curve::tone_map(floor_shows(&camera, pixel, sun, lit).map(|c| c as f32))
                };
                let (low, high) = (shown(lowest), shown(highest));
                let start = ((y * image.width + x) * 4) as usize;
                let off = (0..3)
                    .map(|channel| {
                        let actual = image.pixels[start + channel];
                        (low[channel] - actual).max(actual - high[channel]).max(0.0)
                    })
                    .fold(0.0, f32::max);
                let largest = if partly {
                    &mut in_penumbra
                } else {
                    &mut elsewhere
                };
                *largest = largest.max(off);
            }
        }
        eprintln!(
            "adapter `{}`, seen from {}, a light {degrees}° from the normal: first held by each \
             cascade so far {first_holding:?}, {in_bands} pixels in a band, {beyond} beyond the \
             cascades so far; largest difference {elsewhere} outside penumbras and bands, \
             {in_penumbra} in them",
            setup.gpu.adapter_name(),
            camera.eye,
        );
        assert!(in_bands > 0, "seen from {}, {degrees}°", camera.eye);
        // Within 2 × 10⁻³ as `shown.rs` explains, and 4 × 10⁻³ where the light is partly
        // shadowed, for GPUs that keep the filter's weights in 8 bits.
        assert!(elsewhere <= 2e-3, "{degrees}°: off by {elsewhere}");
        assert!(
            in_penumbra <= 4e-3,
            "{degrees}°: off by {in_penumbra} in penumbras and bands"
        );
    }
    assert!(
        first_holding.iter().all(|&pixels| pixels > 0) && beyond > 0,
        "{first_holding:?}, {beyond}"
    );
}
