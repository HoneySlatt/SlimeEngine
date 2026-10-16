//! Image-based lighting and the sky (ADR-0039), checked by reading back an `Rgba16Float` output
//! through the tone curve of `common/tone_curve.rs`: a white sphere of any roughness, metal or not,
//! inside a uniform environment reflects its radiance, and disappears into it (the furnace test);
//! surfaces of other colours reflect what the split sum, computed here from an independent
//! integration of the BRDF, predicts; a mirror sphere reflects the sky along the reflection of each
//! view ray; the sky pass shows the sky along each view ray where no surface is. Runs on whatever
//! adapter the machine offers.

#[path = "common/brdf.rs"]
#[expect(
    dead_code,
    reason = "this file finds view rays itself: it has no use for points of a plane"
)]
mod brdf;
#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use std::f64::consts::PI;

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, RenderTarget, TextureFormat};
use slime_render::glam::{DVec3, Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Instance, Material, MaterialData, Mesh, MeshData,
    Renderer, Scene, Sky,
};

use tone_curve::tone_map;

const SIZE: u32 = 48;
/// How far the tone-mapped output may be from what each test expects, in its linear part, where it
/// is the exposed light minus 0.04. Measured on lavapipe and on an RX 6900 XT (RADV), the largest
/// differences were 2.8 × 10⁻⁴ for the furnace (two half-precision roundings of 0.46, each up to
/// 4.9 × 10⁻⁴, allow 10⁻³); 2.8 and 2.5 × 10⁻³ for the split sum, at an exposure of 4 that
/// multiplies the table's error (up to 10⁻³ in A and B from its sampling, a few 10⁻⁴ from its
/// interpolation); 8.7 × 10⁻⁴ for the mirror (the facets of the sphere and the texels of the first
/// level); 5.1 × 10⁻⁴ for the sky (the glow's curvature between texels near the sun); 3.9 × 10⁻³
/// under the lobes of rough metals and 1.9 × 10⁻³ for the matte sphere (the levels' sampling error,
/// `environment.rs`). The tolerances leave about half again, or one step of half precision.
const FURNACE_TOLERANCE: f32 = 1e-3;
const SPLIT_SUM_TOLERANCE: f32 = 4e-3;
const MIRROR_TOLERANCE: f32 = 1.5e-3;
const SKY_TOLERANCE: f32 = 8e-4;
const LOBE_TOLERANCE: f32 = 6e-3;

/// A sky of three distinct colours, the zenith's blue, the horizon's white, the ground's brown,
/// within the linear part of the tone curve once exposed at 1.
const SKY: Sky = Sky {
    zenith: LinearColor::rgb(0.1, 0.2, 0.6),
    horizon: LinearColor::rgb(0.6, 0.6, 0.6),
    ground: LinearColor::rgb(0.4, 0.25, 0.1),
    sun: None,
};

/// `SKY` with a sun of 2 lux, 40° high, whose glow peaks at 0.29 nits.
const SUNNY_SKY: Sky = Sky {
    sun: Some(DirectionalLight {
        direction: Vec3::new(0.0, -0.642_787_6, -0.766_044_4),
        color: LinearColor::WHITE,
        illuminance: 2.0,
        casts_shadows: true,
        shadow_distance: 50.0,
    }),
    ..SKY
};

/// The red, green and blue of `color`, its alpha dropped.
fn rgb(color: LinearColor) -> DVec3 {
    DVec3::new(color.r.into(), color.g.into(), color.b.into())
}

/// The radiance of `sky` along the unit vector `d`, computed here from its definition (`Sky`): the
/// CIE clear sky's gradation above the horizon, the haze below it, and the sun's glow.
fn radiance(sky: &Sky, d: DVec3) -> DVec3 {
    let [zenith, horizon, ground] = [sky.zenith, sky.horizon, sky.ground].map(rgb);
    let gradation = |y: f64| 1.0 - (-0.32 / y).exp();
    let haze = (d.y / 0.03).exp();
    let gradient = if d.y > 0.0 {
        zenith.lerp(
            horizon,
            (gradation(d.y) - gradation(1.0)) / (1.0 - gradation(1.0)),
        )
    } else {
        ground.lerp(horizon, haze)
    };
    let glow = sky.sun.map_or(DVec3::ZERO, |sun| {
        let cosine = d.dot(-sun.direction.as_dvec3().normalize());
        let g: f64 = 0.8;
        let phase = (1.0 - g * g) / (4.0 * PI * (1.0 + g * g - 2.0 * g * cosine).powf(1.5));
        let seen = if d.y > 0.0 { 1.0 } else { haze };
        rgb(sun.color) * f64::from(sun.illuminance) * 0.04 * phase * seen
    });
    gradient + glow
}

/// The radiance of `SKY` along the unit vector `d`.
fn sky(d: DVec3) -> DVec3 {
    radiance(&SKY, d)
}

struct Setup {
    gpu: Gpu,
    renderer: Renderer,
    output: RenderTarget,
    sphere: Mesh,
}

impl Setup {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
        let output = gpu
            .create_render_target("environment", SIZE, SIZE, TextureFormat::Rgba16Float)
            .expect("the GPU has memory for it");
        let sphere = renderer
            .create_mesh(&gpu, "sphere", &MeshData::sphere(64, 128))
            .expect("the sphere is valid");
        Self {
            gpu,
            renderer,
            output,
            sphere,
        }
    }

    fn environment(&self, sky: &Sky) -> Environment {
        self.renderer
            .create_environment(&self.gpu, sky)
            .expect("the GPU has memory for it")
    }

    /// Draws the unit sphere at the origin with `material` (none when `None`), seen from `camera`,
    /// lit by `environment` alone, at `exposure`, and reads the output back.
    fn draw(
        &mut self,
        camera: Camera,
        environment: &Environment,
        material: Option<&Material>,
        exposure: f32,
    ) -> FloatImage {
        let instances: Vec<Instance<'_>> = material
            // `MeshData::sphere` is one metre across: drawn twice that, the unit sphere.
            .map(|material| {
                Instance::new(&self.sphere, material, Mat4::from_scale(Vec3::splat(2.0)))
            })
            .into_iter()
            .collect();
        let mut scene = Scene::new(camera, &instances);
        scene.environment = Some(environment);
        scene.exposure = exposure;
        self.renderer
            .render(&self.gpu, &self.output, &scene)
            .expect("the frame is drawn");
        self.gpu
            .read_float_render_target(&self.output)
            .expect("the image is read back")
    }

    fn material(&self, base_color: [f32; 3], metallic: f32, roughness: f32) -> Material {
        let [r, g, b] = base_color;
        self.renderer
            .create_material(
                &self.gpu,
                "sphere",
                &MaterialData {
                    base_color: LinearColor::rgb(r, g, b),
                    metallic,
                    roughness,
                    ..MaterialData::default()
                },
            )
            .expect("the GPU has memory for it")
    }
}

/// A camera 3 units from the origin along `direction`, looking at it, with a field of view that
/// the unit sphere fills but for the image's corners.
fn camera(direction: Vec3) -> Camera {
    Camera {
        eye: direction.normalize() * 3.0,
        target: Vec3::ZERO,
        fov_y: 45f32.to_radians(),
        ..Camera::default()
    }
}

/// The unit vector from `camera`'s eye through the centre of pixel `(x, y)`.
fn view_ray(camera: &Camera, (x, y): (u32, u32)) -> DVec3 {
    let inverse = camera.view_projection(1.0).as_dmat4().inverse();
    let ndc_x = (f64::from(x) + 0.5) / f64::from(SIZE) * 2.0 - 1.0;
    let ndc_y = 1.0 - (f64::from(y) + 0.5) / f64::from(SIZE) * 2.0;
    // Two points along the ray: the near plane, at depth 1, and twice as far, at depth 0.5 (depth
    // is reversed, and 0 is infinitely far).
    let near = inverse.project_point3(DVec3::new(ndc_x, ndc_y, 1.0));
    let far = inverse.project_point3(DVec3::new(ndc_x, ndc_y, 0.5));
    (far - near).normalize()
}

/// The unit normal of the unit sphere where the view ray of pixel `pixel` meets it, and the unit
/// vector from there towards the camera, or `None` for a pixel that misses it or grazes it (n·v
/// below `min_n_dot_v`).
fn sphere_hit(camera: &Camera, pixel: (u32, u32), min_n_dot_v: f64) -> Option<(DVec3, DVec3)> {
    let eye = camera.eye.as_dvec3();
    let d = view_ray(camera, pixel);
    let b = eye.dot(d);
    let c = eye.length_squared() - 1.0;
    let discriminant = b * b - c;
    if discriminant <= 0.0 {
        return None;
    }
    let n = eye + d * (-b - discriminant.sqrt());
    let v = -d;
    (n.dot(v) >= min_n_dot_v).then_some((n, v))
}

/// Checks that pixel `(x, y)` shows `expected` exposed light, tone mapped, within `tolerance`, and
/// returns how far it is.
fn assert_pixel(
    image: &FloatImage,
    (x, y): (u32, u32),
    expected: DVec3,
    tolerance: f32,
    case: &str,
) -> f32 {
    let expected = tone_map(expected.as_vec3().to_array());
    let start = ((y * image.width + x) * 4) as usize;
    let actual = &image.pixels[start..start + 3];
    let largest = actual
        .iter()
        .zip(expected)
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f32::max);
    assert!(
        largest <= tolerance,
        "{case}, pixel ({x}, {y}): expected {expected:?}, got {actual:?} (off by {largest})"
    );
    largest
}

/// The share of a uniform environment that a surface reflecting `f0` at normal incidence reflects
/// specularly at `n_dot_v` and `roughness`, given the BRDF's scale and bias `(a, b)` there: the
/// Sample Viewer's formula.
fn environment_fresnel(f0: f64, n_dot_v: f64, roughness: f64, (a, b): (f64, f64)) -> f64 {
    let rough_fresnel = f0 + ((1.0 - roughness).max(f0) - f0) * (1.0 - n_dot_v).powi(5);
    let single = rough_fresnel * a + b;
    let lost = 1.0 - (a + b);
    let average = f0 + (1.0 - f0) / 21.0;
    single + lost * single * average / (1.0 - average * lost)
}

/// The BRDF's scale and bias: ∫ (1 − F_c) D V (n·l) and ∫ F_c D V (n·l) over the hemisphere, with
/// F_c = (1 − v·h)⁵, by the midpoint rule over 400 × 128 cells of the half vector's angles, θ
/// through tan θ = α s / (1 − s), in f64, with `common/brdf.rs`'s distribution and visibility.
fn scale_and_bias(n_dot_v: f64, roughness: f64) -> (f64, f64) {
    let alpha = roughness * roughness;
    if alpha == 0.0 {
        let fresnel = (1.0 - n_dot_v).powi(5);
        return (1.0 - fresnel, fresnel);
    }
    let v = DVec3::new((1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v);
    let (rows, columns) = (400, 128);
    let (mut a, mut b) = (0.0, 0.0);
    for i in 0..rows {
        let s = (i as f64 + 0.5) / rows as f64;
        let t = alpha * s / (1.0 - s);
        let theta = t.atan();
        let d_theta = alpha / ((1.0 - s) * (1.0 - s)) / (1.0 + t * t) / rows as f64;
        let (sin, cos) = theta.sin_cos();
        for j in 0..columns {
            let phi = 2.0 * PI * (j as f64 + 0.5) / columns as f64;
            let h = DVec3::new(sin * phi.cos(), sin * phi.sin(), cos);
            let v_dot_h = v.dot(h);
            let l = 2.0 * v_dot_h * h - v;
            if v_dot_h <= 0.0 || l.z <= 0.0 {
                continue;
            }
            let white_metal = brdf::Surface {
                base_color: [1.0; 3],
                metallic: 1.0,
                roughness,
            };
            // A white metal's BRDF is F(1) D V = D V: the integrand without Fresnel.
            let specular = brdf::reflected(white_metal, DVec3::Z, l, v)[0];
            // dω_l = 4 (v·h) dω_h, dω_h = sin θ dθ dφ.
            let value = specular * 4.0 * v_dot_h * sin * d_theta * 2.0 * PI / columns as f64;
            let fresnel = (1.0 - v_dot_h).powi(5);
            a += value * (1.0 - fresnel);
            b += value * fresnel;
        }
    }
    (a, b)
}

#[test]
fn a_white_sphere_inside_a_uniform_environment_reflects_its_radiance() {
    // The furnace test: whatever its roughness, a white surface, metal or dielectric, returns
    // all the light of uniform surroundings, so that it disappears into them. 0.5 and every value
    // computed from it are exact in half precision; a step is 4.9 × 10⁻⁴ there.
    let mut setup = Setup::new();
    let uniform = setup.environment(&Sky {
        zenith: LinearColor::rgb(0.5, 0.5, 0.5),
        horizon: LinearColor::rgb(0.5, 0.5, 0.5),
        ground: LinearColor::rgb(0.5, 0.5, 0.5),
        sun: None,
    });
    let camera = camera(Vec3::new(0.3, 0.4, 1.0));
    // The sphere is there: black, it reflects less than its surroundings at every pixel it covers.
    let black = setup.material([0.0; 3], 0.0, 0.5);
    let image = setup.draw(camera, &uniform, Some(&black), 1.0);
    let mut covered = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            if sphere_hit(&camera, (x, y), 0.0).is_some() {
                let start = ((y * SIZE + x) * 4) as usize;
                assert!(
                    image.pixels[start] < 0.4,
                    "pixel ({x}, {y}) is not the sphere"
                );
                covered += 1;
            }
        }
    }
    assert!(
        covered > SIZE * SIZE / 2,
        "the sphere covers {covered} pixels"
    );
    let mut largest = 0.0_f32;
    for metallic in [0.0, 1.0] {
        for roughness in [0.0, 0.25, 0.5, 0.75, 1.0, 0.1, 0.6] {
            let white = setup.material([1.0; 3], metallic, roughness);
            let image = setup.draw(camera, &uniform, Some(&white), 1.0);
            for y in 0..SIZE {
                for x in 0..SIZE {
                    largest = largest.max(assert_pixel(
                        &image,
                        (x, y),
                        DVec3::splat(0.5),
                        FURNACE_TOLERANCE,
                        &format!("metallic {metallic}, roughness {roughness}"),
                    ));
                }
            }
        }
    }
    println!("largest difference {largest}");
}

#[test]
fn a_coloured_sphere_inside_a_uniform_environment_reflects_what_the_split_sum_predicts() {
    // Where the BRDF's scale and bias matter: a black dielectric reflects only its specular
    // layer, F(0.04) of the light, and a grey metal F(0.5); both depend on n·v and the roughness
    // read from the table, which a swapped axis, a lookup off by half a texel or a wrong formula
    // shows. Every pixel of the sphere whose normal is within 60° of the view, against the table
    // computed here by another integration: within the table's error (2.5 × 10⁻³ in A and B,
    // `environment.rs`), at radiance 1.
    let mut setup = Setup::new();
    let uniform = setup.environment(&Sky {
        zenith: LinearColor::WHITE,
        horizon: LinearColor::WHITE,
        ground: LinearColor::WHITE,
        sun: None,
    });
    let camera = camera(Vec3::new(-0.2, 0.5, 1.0));
    let mut largest = 0.0_f32;
    for roughness in [0.2_f32, 0.45, 0.8] {
        // The scale and bias every 1/128 of n·v from 0.5 to 1, between which they are interpolated
        // linearly, within 10⁻⁵: an integration per pixel would take minutes in a debug build.
        let samples: Vec<(f64, f64)> = (0..=64)
            .map(|i| scale_and_bias(0.5 + f64::from(i) / 128.0, f64::from(roughness)))
            .collect();
        let at = |n_dot_v: f64| {
            let position = ((n_dot_v - 0.5) * 128.0).clamp(0.0, 64.0);
            let (lower, fraction) = ((position.floor() as usize).min(63), position.fract());
            let (a0, b0) = samples[lower];
            let (a1, b1) = samples[lower + 1];
            (a0 + (a1 - a0) * fraction, b0 + (b1 - b0) * fraction)
        };
        for (base, metallic, f0, exposure) in [(0.0, 0.0, 0.04, 4.0_f32), (0.5, 1.0, 0.5, 0.8)] {
            let material = setup.material([base; 3], metallic, roughness);
            let image = setup.draw(camera, &uniform, Some(&material), exposure);
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let Some((n, v)) = sphere_hit(&camera, (x, y), 0.5) else {
                        continue;
                    };
                    let n_dot_v = n.dot(v);
                    let reflected =
                        environment_fresnel(f0, n_dot_v, f64::from(roughness), at(n_dot_v))
                            * f64::from(exposure);
                    largest = largest.max(assert_pixel(
                        &image,
                        (x, y),
                        DVec3::splat(reflected),
                        SPLIT_SUM_TOLERANCE,
                        &format!("f0 {f0}, roughness {roughness}"),
                    ));
                }
            }
        }
    }
    println!("largest difference {largest}");
}

#[test]
fn a_mirror_sphere_reflects_the_sky() {
    // A white metal of roughness 0 reflects all the light along the reflection of each view ray:
    // the sky there, as the first level of the environment holds it. Pixels whose reflection is
    // within 12° of the horizon, where the first level's texels of 0.45 to 0.9° blur the haze into
    // the ground, and those whose normal is more than 60° from the view, near the sphere's edge,
    // where its 64 × 128 facets turn the reflection most, are left out.
    let mut setup = Setup::new();
    let environment = setup.environment(&SKY);
    let mirror = setup.material([1.0; 3], 1.0, 0.0);
    let mut checked = [0; 2];
    let mut largest = 0.0_f32;
    for direction in [Vec3::new(0.0, 0.3, 1.0), Vec3::new(1.0, -0.6, -0.4)] {
        let camera = camera(direction);
        let image = setup.draw(camera, &environment, Some(&mirror), 1.0);
        for y in 0..SIZE {
            for x in 0..SIZE {
                let Some((n, v)) = sphere_hit(&camera, (x, y), 0.5) else {
                    continue;
                };
                let r = 2.0 * n.dot(v) * n - v;
                if r.y.abs() < 0.2 {
                    continue;
                }
                checked[usize::from(r.y < 0.0)] += 1;
                largest = largest.max(assert_pixel(
                    &image,
                    (x, y),
                    sky(r),
                    MIRROR_TOLERANCE,
                    &format!("reflecting {r}"),
                ));
            }
        }
    }
    assert!(
        checked.iter().all(|&count| count > 50),
        "pixels reflecting the sky and the ground: {checked:?}"
    );
    println!("largest difference {largest}");
}

#[test]
fn the_sky_shows_where_no_surface_is() {
    // Every pixel of a frame without instances shows the sky along its view ray, but within 12°
    // of the horizon, exposed: looking up, along the horizon from four sides, and down; with the
    // sun's glow, but within 2° of the sun, whose disc the tone-mapping tests check.
    let mut setup = Setup::new();
    let mut largest = 0.0_f32;
    for sky in [&SKY, &SUNNY_SKY] {
        let environment = setup.environment(sky);
        let to_sun = sky.sun.map(|sun| -sun.direction.as_dvec3().normalize());
        let mut checked = 0;
        for (direction, case) in [
            (Vec3::new(0.3, -1.0, 0.4), "looking up"),
            (Vec3::new(0.0, -0.1, -1.0), "looking north"),
            (Vec3::new(1.0, -0.1, 0.0), "looking west"),
            (Vec3::new(0.0, -0.1, 1.0), "looking south"),
            (Vec3::new(-1.0, -0.1, 0.0), "looking east"),
            (Vec3::new(-0.4, 1.0, 0.3), "looking down"),
            (Vec3::new(0.0, -0.64, -0.77), "looking at the sun"),
        ] {
            // The camera at the origin's opposite, looking through the origin along −direction.
            let camera = Camera {
                eye: direction * 3.0,
                target: Vec3::ZERO,
                fov_y: 70f32.to_radians(),
                ..Camera::default()
            };
            let image = setup.draw(camera, &environment, None, 0.5);
            for y in 0..SIZE {
                for x in 0..SIZE {
                    let d = view_ray(&camera, (x, y));
                    let near_the_sun =
                        to_sun.is_some_and(|to_sun| d.dot(to_sun) > 2f64.to_radians().cos());
                    if d.y.abs() < 0.2 || near_the_sun {
                        continue;
                    }
                    largest = largest.max(assert_pixel(
                        &image,
                        (x, y),
                        radiance(sky, d) * 0.5,
                        SKY_TOLERANCE,
                        case,
                    ));
                    checked += 1;
                }
            }
        }
        assert!(checked > 10_000, "{checked} pixels");
    }
    println!("largest difference {largest}");
}

#[test]
fn an_environment_draws_with_any_renderer_and_a_scene_without_one_is_black() {
    // A renderer builds the group of the environment a scene names, and builds it again when the
    // scene names another one or none: an environment that one renderer made draws the same with
    // another, and a scene without one, drawn after it, shows nothing but black.
    let mut setup = Setup::new();
    let environment = setup.environment(&SUNNY_SKY);
    let camera = camera(Vec3::new(0.0, 0.5, 1.0));
    let material = setup.material([0.8, 0.5, 0.2], 1.0, 0.3);
    let made_here = setup.draw(camera, &environment, Some(&material), 1.0);
    let mut other =
        Renderer::new(&setup.gpu, TextureFormat::Rgba16Float).expect("the renderer is created");
    // The unit sphere, as `draw` draws it.
    let instances = [Instance::new(
        &setup.sphere,
        &material,
        Mat4::from_scale(Vec3::splat(2.0)),
    )];
    let mut scene = Scene::new(camera, &instances);
    scene.environment = Some(&environment);
    other
        .render(&setup.gpu, &setup.output, &scene)
        .expect("the frame is drawn");
    let drawn_elsewhere = setup
        .gpu
        .read_float_render_target(&setup.output)
        .expect("the image is read back");
    assert_eq!(drawn_elsewhere.pixels, made_here.pixels);
    scene.environment = None;
    other
        .render(&setup.gpu, &setup.output, &scene)
        .expect("the frame is drawn");
    let black = setup
        .gpu
        .read_float_render_target(&setup.output)
        .expect("the image is read back");
    assert!(
        black
            .pixels
            .chunks(4)
            .all(|pixel| pixel[..3].iter().all(|&value| value == 0.0)),
        "a scene without an environment and without lights is black"
    );
}

/// The sky's radiance averaged around the unit vector `r` with the weight `weight(r·l)` of each
/// direction l (zero where r·l ≤ 0), by the midpoint rule over 400 × 800 cells of latitude and
/// longitude around r, in f64.
fn sky_averaged(r: DVec3, weight: impl Fn(f64) -> f64) -> DVec3 {
    let (t, b) = r.any_orthonormal_pair();
    let (rows, columns) = (400, 800);
    let (mut sum, mut total) = (DVec3::ZERO, 0.0);
    for i in 0..rows {
        let theta = (f64::from(i) + 0.5) / f64::from(rows) * PI / 2.0;
        let (sin, cos) = theta.sin_cos();
        let cell = weight(cos) * sin;
        for j in 0..columns {
            let phi = (f64::from(j) + 0.5) / f64::from(columns) * 2.0 * PI;
            sum += sky((t * phi.cos() + b * phi.sin()) * sin + r * cos) * cell;
            total += cell;
        }
    }
    sum / total
}

/// The sky averaged under the prefilter's lobe for `roughness` around `r`: the weight of l is
/// D(h)(r·l), h halfway between r and l (Karis's split sum, with n = v = r).
fn sky_under_lobe(r: DVec3, roughness: f64) -> DVec3 {
    let alpha2 = roughness.powi(4);
    sky_averaged(r, |r_dot_l| {
        // n·h = cos(θ / 2) for l at θ from r.
        let n_dot_h2 = (1.0 + r_dot_l) / 2.0;
        let f = (1.0 - n_dot_h2) + n_dot_h2 * alpha2;
        alpha2 / (f * f) * r_dot_l
    })
}

#[test]
fn a_white_matte_sphere_under_the_sky_shows_its_irradiance() {
    // A white dielectric of roughness 1 diffuses the sky's irradiance over π read along its
    // normal, under a specular layer that reflects it read along the reflection, mixed by the
    // split sum's Fresnel: the diffuse lookup, which a uniform environment cannot check. At 40
    // pixels spread over the sphere.
    let mut setup = Setup::new();
    let environment = setup.environment(&SKY);
    let matte = setup.material([1.0; 3], 0.0, 1.0);
    let camera = camera(Vec3::new(0.4, 0.2, 1.0));
    let image = setup.draw(camera, &environment, Some(&matte), 1.0);
    let mut largest = 0.0_f32;
    let mut checked = 0;
    for y in (2..SIZE).step_by(6) {
        for x in (1..SIZE).step_by(5) {
            let Some((n, v)) = sphere_hit(&camera, (x, y), 0.3) else {
                continue;
            };
            let r = 2.0 * n.dot(v) * n - v;
            let n_dot_v = n.dot(v);
            let fresnel = environment_fresnel(0.04, n_dot_v, 1.0, scale_and_bias(n_dot_v, 1.0));
            let cosine = |cos: f64| cos;
            let expected =
                sky_averaged(n, cosine) * (1.0 - fresnel) + sky_averaged(r, cosine) * fresnel;
            largest = largest.max(assert_pixel(
                &image,
                (x, y),
                expected,
                LOBE_TOLERANCE,
                "matte",
            ));
            checked += 1;
        }
    }
    assert!(checked >= 30, "{checked} pixels");
    println!("largest difference {largest}");
}

#[test]
fn a_rough_metal_reflects_the_sky_averaged_under_its_lobe() {
    // A white metal reflects the level of its roughness read along the reflection: at 0.5, the
    // sky under the lobe of 0.5; at 0.375, halfway between the levels of 0.25 and 0.5, the
    // average of both lobes, as the GPU blends levels. Pixels whose reflection is within 12° of
    // the horizon are left out, where the levels' texels blur the step to the ground.
    let mut setup = Setup::new();
    let environment = setup.environment(&SKY);
    let camera = camera(Vec3::new(-0.3, 0.5, 1.0));
    let mut largest = 0.0_f32;
    for roughness in [0.5, 0.375] {
        let metal = setup.material([1.0; 3], 1.0, roughness);
        let image = setup.draw(camera, &environment, Some(&metal), 1.0);
        let mut checked = 0;
        for y in (2..SIZE).step_by(7) {
            for x in (1..SIZE).step_by(6) {
                let Some((n, v)) = sphere_hit(&camera, (x, y), 0.5) else {
                    continue;
                };
                let r = 2.0 * n.dot(v) * n - v;
                if r.y.abs() < 0.2 {
                    continue;
                }
                let expected = if roughness == 0.5 {
                    sky_under_lobe(r, 0.5)
                } else {
                    (sky_under_lobe(r, 0.25) + sky_under_lobe(r, 0.5)) / 2.0
                };
                largest = largest.max(assert_pixel(
                    &image,
                    (x, y),
                    expected,
                    LOBE_TOLERANCE,
                    &format!("roughness {roughness}"),
                ));
                checked += 1;
            }
        }
        assert!(checked >= 15, "roughness {roughness}: {checked} pixels");
    }
    println!("largest difference {largest}");
}
