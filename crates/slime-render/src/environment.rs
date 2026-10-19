//! Image-based lighting computed on the CPU (ADR-0039): a procedural sky drawn into a cube map,
//! prefiltered into levels of increasing roughness, and the table of the BRDF's scale and bias.
//!
//! The environment's level `i` holds, for each direction r, the sky's light reflected towards r by
//! a mirror (level 0) or by GGX microfacets of roughness `i / 4` seen along their normal: the
//! split-sum prefilter of Karis, Σ L(l)(n·l) / Σ (n·l) with n = v = r and l drawn from GGX's
//! distribution. At roughness 1, GGX's distribution is uniform and the prefilter is the average of
//! the sky weighted by n·l, which is the irradiance divided by π: the last level is also the
//! diffuse light.

use std::f32::consts::{PI, TAU};
use std::sync::{Arc, LazyLock};

use glam::{DVec3, Vec3};
use slime_color::LinearColor;
use slime_gfx::{GfxError, Gpu, Texture, TextureFormat, f32_to_f16};

use crate::light::{DirectionalLight, LARGEST_LIGHT, Light, rgb, unit};

/// The width of the faces of the environment's first level, in texels: 0.90° per texel at a face's
/// centre, 0.45° at its edges.
pub(crate) const ENVIRONMENT_SIZE: u32 = 128;
/// The environment's levels, each half the size of the one before (128 down to 8): level `i` is
/// prefiltered for a roughness of `i / (ENVIRONMENT_LEVELS - 1)`. `ROUGHEST_LEVEL` in `shader.wgsl`
/// is this minus one, which a unit test checks.
pub(crate) const ENVIRONMENT_LEVELS: usize = 5;
/// The directions each texel of levels 1 to 4 averages, per level: the narrow lobes of smooth
/// levels need fewer, since filtered importance sampling reads each from a level of the sky whose
/// texels cover the solid angle it stands for.
const PREFILTER_SAMPLES: [u32; ENVIRONMENT_LEVELS] = [0, 64, 128, 256, 2048];
/// The BRDF table's size along n·v and roughness, its texels' centres on 0 and 1 of each.
/// `BRDF_TABLE_SIZE` in `shader.wgsl` is the same, which a unit test checks.
pub(crate) const BRDF_TABLE_SIZE: u32 = 32;
/// The directions each texel of the BRDF table averages.
const BRDF_TABLE_SAMPLES: u32 = 1024;
/// The sun's angular radius, in radians: half its mean apparent diameter, 0.533°.
pub(crate) const SUN_ANGULAR_RADIUS: f64 = 0.2665 * std::f64::consts::PI / 180.0;
/// The share of the sun's illuminance that the sky scatters around it, its glow (the aureole of a
/// clear sky).
const GLOW_SHARE: f32 = 0.04;
/// How narrowly the glow gathers around the sun: Henyey and Greenstein's g, from 0 (the same in
/// every direction) towards 1 (all forward).
const GLOW_FOCUS: f32 = 0.8;
/// The sine of the depression below the horizon over which the horizon's light fades into the
/// ground's by a factor e: about 1.7°, the haze over distant ground.
pub(crate) const GROUND_HAZE: f32 = 0.03;
/// The CIE standard clear sky's luminance gradation, 1 − exp(−0.32 / sin elevation), at the zenith.
const GRADATION_AT_ZENITH: f32 = 0.273_850_96;
/// The largest radiance a sky may have, the largest half-precision float, which its texels hold.
const LARGEST_HALF: f32 = 65_504.0;
/// The smallest n·v the BRDF table is computed at: at 0, the view grazes the surface and the
/// integrand is a limit.
const SMALLEST_N_DOT_V: f32 = 1e-4;

/// A procedural daytime sky: light from every direction around the scene, which lights surfaces
/// through image-based lighting and shows behind them ([`Renderer::create_environment`]).
/// Radiances are in nits (candelas per square metre), linear, like all the light of a [`Scene`]:
/// the sky's light is exposed as the surfaces' is.
///
/// Above the horizon, the sky goes from `zenith` overhead to `horizon` as the luminance of the CIE
/// standard clear sky does: a direction of elevation sine y > 0 sees
/// `zenith + (horizon − zenith) t`, t = (g(y) − g(1)) / (1 − g(1)), g(y) = 1 − exp(−0.32 / y),
/// which brightens and pales mostly within 20° of the horizon (t is 0.12 at 45°, 0.46 at 20°,
/// 0.78 at 10°). Below it, the horizon's light fades into `ground` within a couple of degrees,
/// `ground + (horizon − ground) exp(y / 0.03)`: the haze over distant ground.
///
/// With a `sun`, the sky also holds the light it scatters around the sun, its glow: 4 % of the
/// sun's illuminance, gathered around its direction by a Henyey-Greenstein lobe (g = 0.8), and
/// hidden by the ground through the same haze. The sun itself is not in the sky's light: the sky
/// pass draws its disc, of the real sun's size, whose radiance gives the sun's illuminance, hidden
/// below the horizon by the same haze, but the disc lights nothing. The same light in
/// [`Scene::lights`](crate::Scene::lights) lights surfaces and casts shadows, so that the sun's
/// light reaches surfaces once, and a shadow keeps the sky's light, as outdoors.
///
/// A sky's light in any direction, glow included, is at most 65 504 nits per channel, the largest
/// value the environment's half-precision texels hold: [`Renderer::create_environment`] refuses
/// brighter zenith, horizon or ground, and the glow is kept within it.
///
/// [`Renderer::create_environment`]: crate::Renderer::create_environment
/// [`Scene`]: crate::Scene
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// The radiance straight up, in nits; alpha is ignored.
    pub zenith: LinearColor,
    /// The radiance at the horizon, in nits, usually brighter and whiter than the zenith's; alpha
    /// is ignored.
    pub horizon: LinearColor,
    /// The radiance of the ground below the horizon, in nits: for flat ground of albedo a lit by
    /// E lux (the sun's illuminance times the sine of its elevation, plus the sky's), a × E / π.
    /// Alpha is ignored.
    pub ground: LinearColor,
    /// The sun: the direction its light travels in, its color and its illuminance place its glow
    /// and its disc (`casts_shadows` and `shadow_distance` mean nothing here). Give the scene the
    /// same light, so that the disc and the glow lie where the light and its shadows come from.
    /// `None` for a sky without a sun.
    pub sun: Option<DirectionalLight>,
}

impl Sky {
    /// The radiance seen along the unit vector `direction`.
    pub(crate) fn radiance(&self, direction: Vec3) -> Vec3 {
        let y = direction.y;
        let haze = (y / GROUND_HAZE).exp();
        let gradient = if y > 0.0 {
            let gradation = 1.0 - (-0.32 / y).exp();
            let t = (gradation - GRADATION_AT_ZENITH) / (1.0 - GRADATION_AT_ZENITH);
            rgb(self.zenith).lerp(rgb(self.horizon), t)
        } else {
            rgb(self.ground).lerp(rgb(self.horizon), haze)
        };
        let glow = self.sun.map_or(Vec3::ZERO, |sun| {
            let cosine = direction.dot(-unit(sun.direction));
            let g = GLOW_FOCUS;
            // Henyey and Greenstein's phase function, whose integral over every direction is 1.
            let phase = (1.0 - g * g) / (2.0 * TAU * (1.0 + g * g - 2.0 * g * cosine).powf(1.5));
            let seen = if y > 0.0 { 1.0 } else { haze };
            rgb(sun.color) * (sun.illuminance * GLOW_SHARE * phase * seen)
        });
        (gradient + glow).min(Vec3::splat(LARGEST_HALF))
    }

    /// The direction towards the sun's disc and its radiance, in nits, which gives a surface facing
    /// it the sun's illuminance: the illuminance over the disc's solid angle, at most
    /// [`LARGEST_LIGHT`]. No radiance without a sun.
    fn sun_disc(&self) -> (Vec3, Vec3) {
        self.sun.map_or((Vec3::Y, Vec3::ZERO), |sun| {
            // 2π (1 − cos r), written 4π sin²(r / 2): 1 − cos r cancels most digits of a cosine
            // so near 1.
            let solid_angle = 4.0 * std::f64::consts::PI * (SUN_ANGULAR_RADIUS / 2.0).sin().powi(2);
            // In f64, then bounded: a sun's light that a float holds can give a disc that it does
            // not, which the sky pass would multiply by 0 outside the disc.
            let radiance = rgb(sun.color).as_dvec3() * (f64::from(sun.illuminance) / solid_angle);
            (
                -unit(sun.direction),
                radiance
                    .min(DVec3::splat(f64::from(LARGEST_LIGHT)))
                    .as_vec3(),
            )
        })
    }
}

/// A sky made ready to light scenes and to be drawn behind them: its cube map, prefiltered into
/// levels of increasing roughness, on the GPU, and its sun's disc. Created by
/// [`Renderer::create_environment`](crate::Renderer::create_environment), and named by
/// [`Scene::environment`](crate::Scene::environment). Any renderer of the same [`Gpu`] draws with
/// it.
///
/// Clones refer to the same textures, which are freed when the last clone is dropped and the GPU
/// no longer uses them. A renderer keeps a clone of the last environment it drew with, until it
/// draws with another one or without one: dropping every clone of yours frees the textures only
/// then.
#[derive(Clone, Debug)]
pub struct Environment {
    pub(crate) texture: Arc<Texture>,
    /// The direction towards the sun's disc.
    pub(crate) to_sun: Vec3,
    /// The radiance of the sun's disc, in nits; zero without a sun.
    pub(crate) sun_radiance: Vec3,
}

impl Environment {
    /// Computes the environment of `sky` and uploads it.
    ///
    /// # Errors
    ///
    /// As [`Gpu::create_cube_texture`].
    ///
    /// # Panics
    ///
    /// As [`environment_levels`].
    pub(crate) fn new(gpu: &Gpu, sky: &Sky) -> Result<Self, GfxError> {
        let (to_sun, sun_radiance) = sky.sun_disc();
        Ok(Self {
            to_sun,
            sun_radiance,
            ..Self::upload(
                gpu,
                "environment",
                ENVIRONMENT_SIZE,
                &environment_levels(sky),
            )?
        })
    }

    /// One black texel and no sun: the environment of scenes without one.
    pub(crate) fn black(gpu: &Gpu) -> Result<Self, GfxError> {
        Self::upload(gpu, "no environment", 1, &[[0.0, 0.0, 0.0, 1.0].repeat(6)])
    }

    /// Uploads `levels`, each six faces of RGBA values, the first `size` texels wide, as
    /// half-precision floats.
    fn upload(gpu: &Gpu, label: &str, size: u32, levels: &[Vec<f32>]) -> Result<Self, GfxError> {
        let levels: Vec<Vec<u8>> = levels.iter().map(|level| halves(level)).collect();
        let levels: Vec<&[u8]> = levels.iter().map(Vec::as_slice).collect();
        Ok(Self {
            texture: Arc::new(gpu.create_cube_texture(
                label,
                size,
                TextureFormat::Rgba16Float,
                &levels,
            )?),
            to_sun: Vec3::Y,
            sun_radiance: Vec3::ZERO,
        })
    }

    /// Whether `other` refers to the same textures.
    pub(crate) fn is(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.texture, &other.texture)
    }
}

/// `values` as the bytes of half-precision floats, little-endian, as `Rgba16Float` texels hold
/// them.
fn halves(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|&value| f32_to_f16(value).to_le_bytes())
        .collect()
}

/// The levels of the environment of `sky`, as [`Environment::upload`] takes them: level 0 the sky
/// itself, each texel averaging 2 × 2 points of it, then each level prefiltered for its roughness.
/// A sky of one radiance in every direction, without a sun, has that radiance in every texel of
/// every level, which the integrals would give: they are skipped, which keeps the tests, whose
/// skies are mostly such, fast.
///
/// # Panics
///
/// If the zenith's, the horizon's or the ground's radiance is negative, not finite, or above
/// 65 504, or if the sun is not a light that the renderer draws ([`Light::check`]).
pub(crate) fn environment_levels(sky: &Sky) -> Vec<Vec<f32>> {
    let [zenith, horizon, ground] = [sky.zenith, sky.horizon, sky.ground].map(rgb);
    for (name, radiance) in [("zenith", zenith), ("horizon", horizon), ("ground", ground)] {
        assert!(
            radiance.is_finite()
                && radiance.cmpge(Vec3::ZERO).all()
                && radiance.cmple(Vec3::splat(LARGEST_HALF)).all(),
            "the sky's {name} radiance is {radiance}: each channel must be from 0 to 65 504 nits"
        );
    }
    if let Some(sun) = sky.sun {
        // Its shadows mean nothing here.
        let light = Light::Directional(DirectionalLight {
            shadow_distance: DirectionalLight::default().shadow_distance,
            ..sun
        });
        if let Err(reason) = light.check() {
            panic!("the sky's sun is not a light that the renderer draws: {reason}");
        }
    }
    if sky.sun.is_none() && zenith == horizon && horizon == ground {
        let texel = [zenith.x, zenith.y, zenith.z, 1.0];
        return (0..ENVIRONMENT_LEVELS)
            .map(|level| {
                let size = (ENVIRONMENT_SIZE >> level) as usize;
                texel.repeat(6 * size * size)
            })
            .collect();
    }
    let size = ENVIRONMENT_SIZE;
    let base = CubeMap::from_fn(size, |face, x, y| {
        // The average of four points, a quarter of a texel from its centre on each axis.
        [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
            .iter()
            .map(|&(dx, dy)| {
                sky.radiance(texel_direction(face, x as f32 + dx, y as f32 + dy, size).normalize())
            })
            .sum::<Vec3>()
            / 4.0
    });
    prefiltered_levels(base)
}

/// The environment's levels from its first, `base`: each next level half the size of the one
/// before and prefiltered for its roughness (see the module's documentation).
fn prefiltered_levels(base: CubeMap) -> Vec<Vec<f32>> {
    let chain = base.chain();
    let mut levels = vec![chain[0].rgba()];
    for (level, &count) in PREFILTER_SAMPLES.iter().enumerate().skip(1) {
        let roughness = level as f32 / (ENVIRONMENT_LEVELS - 1) as f32;
        let samples = prefilter_samples(roughness, count, chain[0].size);
        let size = chain[0].size >> level;
        let prefiltered = CubeMap::from_fn(size, |face, x, y| {
            let n = texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, size).normalize();
            let (t, b) = n.any_orthonormal_pair();
            let mut sum = Vec3::ZERO;
            let mut weights = 0.0;
            for sample in &samples {
                let l = t * sample.direction.x + b * sample.direction.y + n * sample.direction.z;
                sum += sample_chain(&chain, l, sample.lod) * sample.weight;
                weights += sample.weight;
            }
            sum / weights
        });
        levels.push(prefiltered.rgba());
    }
    levels
}

/// One direction of the prefilter, the same around every texel's direction.
struct PrefilterSample {
    /// In the frame of the texel: x and y across, z along the texel's direction.
    direction: Vec3,
    /// n·l.
    weight: f32,
    /// The level of the sky's chain to read it from.
    lod: f32,
}

/// The `count` directions l of GGX's prefilter for `roughness`, with n = v = (0, 0, 1): half
/// vectors h from a Hammersley sequence, distributed as D(h)(n·h), and l the reflection of v about
/// h; those below the horizon dropped. Each is read from the level of the chain whose texels cover
/// the solid angle it stands for, 1 / (count × its density), against a texel of the first level
/// (`base_size` wide): Křivánek and Colbert's filtered importance sampling, which removes the noise
/// of sharp features, such as the horizon, that few samples would show.
fn prefilter_samples(roughness: f32, count: u32, base_size: u32) -> Vec<PrefilterSample> {
    let alpha2 = (roughness * roughness).powi(2);
    let texel_solid_angle = 4.0 * PI / (6.0 * (base_size * base_size) as f32);
    (0..count)
        .filter_map(|i| {
            let (u, v) = hammersley(i, count);
            // GGX's inverse distribution: cos²θ = (1 − u) / (1 + (α² − 1) u).
            let cos2 = (1.0 - u) / (1.0 + (alpha2 - 1.0) * u);
            let (cos, sin) = (cos2.sqrt(), (1.0 - cos2).max(0.0).sqrt());
            let phi = 2.0 * PI * v;
            let h = Vec3::new(sin * phi.cos(), sin * phi.sin(), cos);
            let l = 2.0 * cos * h - Vec3::Z;
            (l.z > 0.0).then(|| {
                // D(h), written without the cancellation of (n·h)²(α² − 1) + 1 near n·h = 1.
                let f = sin * sin + cos2 * alpha2;
                let distribution = alpha2 / (PI * f * f);
                // The density of l is D(h)(n·h) / (4 v·h), and n·h = v·h here.
                let density = distribution / 4.0;
                let solid_angle = 1.0 / (count as f32 * density);
                PrefilterSample {
                    direction: l,
                    weight: l.z,
                    lod: (0.5 * (solid_angle / texel_solid_angle).log2()).max(0.0),
                }
            })
        })
        .collect()
}

/// The `i`-th of `count` points of the Hammersley set in the unit square: i / count, and i's bits
/// mirrored about the binary point.
fn hammersley(i: u32, count: u32) -> (f32, f32) {
    (
        i as f32 / count as f32,
        i.reverse_bits() as f32 * (1.0 / 4_294_967_296.0),
    )
}

/// The direction through the point (x, y) of `face` of a cube map whose faces are `size` texels
/// wide, x along the face's columns and y down its rows, from 0 to `size` (texel centres at
/// i + 0.5); faces in wgpu's order +X, −X, +Y, −Y, +Z, −Z and orientation, which
/// `Gpu::create_cube_texture` documents and a test checks on the GPU. Not normalized.
pub(crate) fn texel_direction(face: usize, x: f32, y: f32, size: u32) -> Vec3 {
    let s = 2.0 * x / size as f32 - 1.0;
    let t = 2.0 * y / size as f32 - 1.0;
    match face {
        0 => Vec3::new(1.0, -t, -s),
        1 => Vec3::new(-1.0, -t, s),
        2 => Vec3::new(s, 1.0, t),
        3 => Vec3::new(s, -1.0, -t),
        4 => Vec3::new(s, -t, 1.0),
        _ => Vec3::new(-s, -t, -1.0),
    }
}

/// The face that `direction` reads, and the point (s, t) of it, each from −1 to 1: the inverse of
/// [`texel_direction`].
fn face_point(direction: Vec3) -> (usize, f32, f32) {
    let a = direction.abs();
    let d = direction;
    if a.x >= a.y && a.x >= a.z {
        if d.x > 0.0 {
            (0, -d.z / a.x, -d.y / a.x)
        } else {
            (1, d.z / a.x, -d.y / a.x)
        }
    } else if a.y >= a.z {
        if d.y > 0.0 {
            (2, d.x / a.y, d.z / a.y)
        } else {
            (3, d.x / a.y, -d.z / a.y)
        }
    } else if d.z > 0.0 {
        (4, d.x / a.z, -d.y / a.z)
    } else {
        (5, -d.x / a.z, -d.y / a.z)
    }
}

/// A cube map of RGB radiances on the CPU.
struct CubeMap {
    /// The width of each face.
    size: u32,
    /// Face after face, row after row.
    texels: Vec<Vec3>,
}

impl CubeMap {
    /// A cube map whose texel (x, y) of each face is `texel(face, x, y)`, computed on every core.
    fn from_fn(size: u32, texel: impl Fn(usize, u32, u32) -> Vec3 + Sync) -> Self {
        let face_texels = (size * size) as usize;
        let texels = computed_on_every_core(6 * face_texels, |index| {
            let (face, within) = (index / face_texels, index % face_texels);
            texel(face, within as u32 % size, within as u32 / size)
        });
        Self { size, texels }
    }

    /// This map, then each next level half its size, each texel the average of the four it
    /// covers, down to faces of one texel.
    fn chain(self) -> Vec<CubeMap> {
        let mut chain = vec![self];
        while chain.last().is_some_and(|level| level.size > 1) {
            let level = chain.last().expect("the chain is not empty");
            let size = level.size / 2;
            let halved = CubeMap::from_fn(size, |face, x, y| {
                let at = |dx, dy| level.texel(face, 2 * x + dx, 2 * y + dy);
                (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4.0
            });
            chain.push(halved);
        }
        chain
    }

    fn texel(&self, face: usize, x: u32, y: u32) -> Vec3 {
        self.texels[face * (self.size * self.size) as usize + (y * self.size + x) as usize]
    }

    /// The radiance along `direction`, blended between the four nearest texels of its face (the
    /// edge's texels repeated beyond the edge).
    fn sample(&self, direction: Vec3) -> Vec3 {
        let (face, s, t) = face_point(direction);
        let size = self.size as f32;
        let x = ((s + 1.0) / 2.0 * size - 0.5).clamp(0.0, size - 1.0);
        let y = ((t + 1.0) / 2.0 * size - 0.5).clamp(0.0, size - 1.0);
        let (x0, y0) = (x.floor() as u32, y.floor() as u32);
        let (x1, y1) = ((x0 + 1).min(self.size - 1), (y0 + 1).min(self.size - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let top = self.texel(face, x0, y0).lerp(self.texel(face, x1, y0), fx);
        let bottom = self.texel(face, x0, y1).lerp(self.texel(face, x1, y1), fx);
        top.lerp(bottom, fy)
    }

    /// The texels as RGBA values, alpha 1.
    fn rgba(&self) -> Vec<f32> {
        self.texels
            .iter()
            .flat_map(|texel| [texel.x, texel.y, texel.z, 1.0])
            .collect()
    }
}

/// The radiance along `direction` at the level of detail `lod` of `chain`, blended between the two
/// nearest levels.
fn sample_chain(chain: &[CubeMap], direction: Vec3, lod: f32) -> Vec3 {
    let lod = lod.clamp(0.0, (chain.len() - 1) as f32);
    let lower = lod.floor() as usize;
    let upper = (lower + 1).min(chain.len() - 1);
    chain[lower]
        .sample(direction)
        .lerp(chain[upper].sample(direction), lod - lower as f32)
}

/// The BRDF's scale and bias for image-based lighting (the split sum of Karis), for glTF's GGX
/// distribution and Smith visibility as `shader.wgsl` computes them: at texel (i, j), for
/// n·v = i / (size − 1) and roughness j / (size − 1), A = ∫ (1 − F_c) D V (n·l) dω and
/// B = ∫ F_c D V (n·l) dω with F_c = (1 − v·h)⁵, so that a surface reflecting f0 at normal
/// incidence reflects f0 A + B of a uniform environment (single scattering). In the red and green
/// channels. Computed once per process.
pub(crate) static BRDF_TABLE: LazyLock<Vec<f32>> = LazyLock::new(|| {
    let size = BRDF_TABLE_SIZE;
    let last = (size - 1) as f32;
    computed_on_every_core((size * size) as usize, |index| {
        let (i, j) = (index as u32 % size, index as u32 / size);
        let n_dot_v = (i as f32 / last).max(SMALLEST_N_DOT_V);
        let (a, b) = brdf_scale_and_bias(n_dot_v, j as f32 / last, BRDF_TABLE_SAMPLES);
        [a, b, 0.0, 1.0]
    })
    .into_iter()
    .flatten()
    .collect()
});

/// `value(i)` for each i from 0 to `count`, in order, computed by one thread per core, each taking
/// a contiguous range: whatever thread computes a value, it is the same.
fn computed_on_every_core<T: Copy + Default + Send>(
    count: usize,
    value: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    let mut values = vec![T::default(); count];
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let chunk = count.div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (index, values) in values.chunks_mut(chunk).enumerate() {
            let value = &value;
            scope.spawn(move || {
                for (offset, slot) in values.iter_mut().enumerate() {
                    *slot = value(index * chunk + offset);
                }
            });
        }
    });
    values
}

/// A and B of [`BRDF_TABLE`] at `n_dot_v` and `roughness`, from `count` half vectors h of a
/// Hammersley sequence, distributed as the microfacet normals that v sees, and l the reflection of
/// v about each.
fn brdf_scale_and_bias(n_dot_v: f32, roughness: f32, count: u32) -> (f32, f32) {
    if roughness == 0.0 {
        // A mirror: all the light comes from the reflection, with v·h = n·v.
        let fresnel = (1.0 - n_dot_v).powi(5);
        return (1.0 - fresnel, fresnel);
    }
    let alpha = roughness * roughness;
    let v = Vec3::new((1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v);
    // Heitz's sampling of the normals that v sees (JCGT 7(4), 2018): v stretched to the
    // configuration of a roughness of 1, where they are a disc projected onto a hemisphere.
    let stretched = Vec3::new(alpha * v.x, alpha * v.y, v.z).normalize();
    let across = if stretched.x * stretched.x + stretched.y * stretched.y > 0.0 {
        Vec3::new(-stretched.y, stretched.x, 0.0).normalize()
    } else {
        Vec3::X
    };
    let up = stretched.cross(across);
    let blend = 0.5 * (1.0 + stretched.z);
    let lambda_v = smith_lambda(n_dot_v, alpha);
    let (mut a, mut b) = (0.0, 0.0);
    for i in 0..count {
        let (u, w) = hammersley(i, count);
        let radius = u.sqrt();
        let phi = 2.0 * PI * w;
        let t1 = radius * phi.cos();
        let t2 = (1.0 - blend) * (1.0 - t1 * t1).sqrt() + blend * radius * phi.sin();
        let on_hemisphere =
            t1 * across + t2 * up + (1.0 - t1 * t1 - t2 * t2).max(0.0).sqrt() * stretched;
        let h = Vec3::new(
            alpha * on_hemisphere.x,
            alpha * on_hemisphere.y,
            on_hemisphere.z.max(0.0),
        )
        .normalize();
        let v_dot_h = v.dot(h);
        let n_dot_l = 2.0 * v_dot_h * h.z - v.z;
        if n_dot_l <= 0.0 {
            continue;
        }
        // Drawn with a density proportional to G1(v) D(h)(v·h) / (n·v), a direction weighs
        // G2(v, l) / G1(v), the share of its light that the microfacets around it do not mask: a
        // smooth function of l, which goes to 0 at the horizon.
        let weight = (1.0 + lambda_v) / (1.0 + lambda_v + smith_lambda(n_dot_l, alpha));
        let fresnel = (1.0 - v_dot_h).powi(5);
        a += (1.0 - fresnel) * weight;
        b += fresnel * weight;
    }
    (a / count as f32, b / count as f32)
}

/// Smith's Λ for GGX at a direction of cosine `cosine` with the normal: G1 = 1 / (1 + Λ), and the
/// height-correlated masking-shadowing that `shader.wgsl`'s visibility term holds is
/// 1 / (1 + Λ(v) + Λ(l)).
fn smith_lambda(cosine: f32, alpha: f32) -> f32 {
    ((cosine * cosine + alpha * alpha * (1.0 - cosine * cosine)).sqrt() / cosine - 1.0) / 2.0
}

/// The BRDF table as a texture.
pub(crate) fn brdf_table(gpu: &Gpu) -> Result<Texture, GfxError> {
    gpu.create_texture_with_levels(
        "BRDF scale and bias",
        BRDF_TABLE_SIZE,
        BRDF_TABLE_SIZE,
        TextureFormat::Rgba16Float,
        &[&halves(&BRDF_TABLE)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sample's sky (`examples/common/sample_scene.rs`), in nits, with its sun.
    const SAMPLE_SKY: Sky = Sky {
        zenith: LinearColor::rgb(1700.0, 3900.0, 10_000.0),
        horizon: LinearColor::rgb(7600.0, 9600.0, 12_200.0),
        ground: LinearColor::rgb(3100.0, 2950.0, 2700.0),
        sun: Some(DirectionalLight {
            direction: Vec3::new(-0.5, -1.0, -0.6),
            color: LinearColor::rgb(1.0, 0.98, 0.9),
            illuminance: 100_000.0,
            casts_shadows: true,
            shadow_distance: 50.0,
        }),
    };

    /// The texel of `level` (RGBA values, faces `size` wide) at (face, x, y), as RGB.
    fn texel(level: &[f32], size: u32, face: usize, x: u32, y: u32) -> Vec3 {
        let start = 4 * (face * (size * size) as usize + (y * size + x) as usize);
        Vec3::from_slice(&level[start..start + 3])
    }

    /// The solid angle of the texel (x, y) of a face `size` texels wide: the area it projects to on
    /// the unit sphere.
    fn texel_solid_angle(x: u32, y: u32, size: u32) -> f64 {
        let corner = |x: f64, y: f64| (x * y).atan2((x * x + y * y + 1.0).sqrt());
        let at = |i: u32| 2.0 * f64::from(i) / f64::from(size) - 1.0;
        let (s0, s1, t0, t1) = (at(x), at(x + 1), at(y), at(y + 1));
        corner(s0, t0) - corner(s0, t1) - corner(s1, t0) + corner(s1, t1)
    }

    /// The prefilter of `base` at `r` for `roughness`, computed over every texel of `base`, each
    /// weighted by D(h)(r·l) and its exact solid angle: the integral that the Monte Carlo
    /// prefilter estimates, without its sampling and its filtering. In f64.
    fn prefiltered_by_quadrature(base: &CubeMap, r: Vec3, roughness: f64) -> Vec3 {
        let alpha2 = roughness.powi(4);
        let r = r.normalize().as_dvec3();
        let (mut sum, mut weights) = (glam::DVec3::ZERO, 0.0);
        for face in 0..6 {
            for y in 0..base.size {
                for x in 0..base.size {
                    let l = texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, base.size)
                        .normalize()
                        .as_dvec3();
                    let r_dot_l = r.dot(l);
                    if r_dot_l <= 0.0 {
                        continue;
                    }
                    let n_dot_h = r.dot((r + l).normalize());
                    let f = (1.0 - n_dot_h * n_dot_h) + n_dot_h * n_dot_h * alpha2;
                    let weight = alpha2 / (f * f) * r_dot_l * texel_solid_angle(x, y, base.size);
                    sum += base.texel(face, x, y).as_dvec3() * weight;
                    weights += weight;
                }
            }
        }
        (sum / weights).as_vec3()
    }

    fn uniform_sky(radiance: f32) -> Sky {
        let radiance = LinearColor::rgb(radiance, radiance, radiance);
        Sky {
            zenith: radiance,
            horizon: radiance,
            ground: radiance,
            sun: None,
        }
    }

    /// The unit direction at `elevation` degrees above the horizon (below it if negative).
    fn at_elevation(elevation: f32) -> Vec3 {
        let (sin, cos) = elevation.to_radians().sin_cos();
        Vec3::new(cos, sin, 0.0)
    }

    #[test]
    fn the_sky_is_the_zenith_overhead_turning_to_the_horizon_and_the_ground_below() {
        let sky = Sky {
            zenith: LinearColor::rgb(1.0, 2.0, 3.0),
            horizon: LinearColor::rgb(17.0, 18.0, 19.0),
            ground: LinearColor::rgb(5.0, 6.0, 7.0),
            sun: None,
        };
        assert!(sky.radiance(Vec3::Y).abs_diff_eq(rgb(sky.zenith), 1e-5));
        assert_eq!(sky.radiance(Vec3::X), rgb(sky.horizon));
        assert!(sky.radiance(-Vec3::Y).abs_diff_eq(rgb(sky.ground), 1e-5));
        // The share of the way from the zenith to the horizon, by the CIE gradation, computed by
        // hand: 0.124 at 45°, 0.460 at 20°, 0.782 at 10°, 0.965 at 5°.
        for (elevation, share) in [
            (45.0, 0.124_14),
            (20.0, 0.459_69),
            (10.0, 0.781_90),
            (5.0, 0.964_97),
        ] {
            let expected = rgb(sky.zenith).lerp(rgb(sky.horizon), share);
            assert!(
                sky.radiance(at_elevation(elevation))
                    .abs_diff_eq(expected, 1e-4),
                "{elevation}°"
            );
        }
        // Below the horizon, the haze: e^(sin(−1°) / 0.03) = 0.559 of the horizon at 1°, 0.055 at
        // 5°.
        for (elevation, share) in [(-1.0, 0.558_92), (-5.0, 0.054_74)] {
            let expected = rgb(sky.ground).lerp(rgb(sky.horizon), share);
            assert!(
                sky.radiance(at_elevation(elevation))
                    .abs_diff_eq(expected, 1e-4),
                "{elevation}°"
            );
        }
    }

    #[test]
    fn the_sun_s_glow_is_four_percent_of_its_illuminance_around_it() {
        // A sun straight overhead in a black sky: its glow, 4 % of 1 000 lux, peaks at the sun at
        // 40 (1 − g²) / (4π (1 − g)³) = 143.24 nits (Henyey-Greenstein's lobe, g = 0.8)...
        let sky = Sky {
            sun: Some(DirectionalLight {
                direction: -Vec3::Y,
                color: LinearColor::WHITE,
                illuminance: 1000.0,
                ..DirectionalLight::default()
            }),
            ..uniform_sky(0.0)
        };
        assert!((sky.radiance(Vec3::Y).x - 143.239_45).abs() < 1e-3);
        // ... and its radiance over the sky above the horizon sums to 40 lux but the 5.07 % that
        // the lobe sends below it, (1 − g²) / 2g (1 / √(1 + g²) − 1 / (1 + g)).
        let (rows, columns) = (2000, 64);
        let mut sum = 0.0;
        for i in 0..rows {
            let theta = (f64::from(i) + 0.5) / f64::from(rows) * std::f64::consts::FRAC_PI_2;
            let (sin, cos) = theta.sin_cos();
            for j in 0..columns {
                let phi = (f64::from(j) + 0.5) / f64::from(columns) * std::f64::consts::TAU;
                let d = Vec3::new(
                    (sin * phi.cos()) as f32,
                    cos as f32,
                    (sin * phi.sin()) as f32,
                );
                sum += f64::from(sky.radiance(d).x) * sin;
            }
        }
        let integral = sum * std::f64::consts::FRAC_PI_2 / f64::from(rows) * std::f64::consts::TAU
            / f64::from(columns);
        let expected = 40.0 * (1.0 - 0.050_695);
        assert!(
            (integral / expected - 1.0).abs() < 2e-3,
            "{integral} lux, not {expected}"
        );
    }

    #[test]
    fn a_sun_direction_of_any_finite_length_places_the_same_sun() {
        let sun = SAMPLE_SKY.sun.expect("the sample's sky has a sun");
        let unit = sun.direction.as_dvec3().normalize().as_vec3();
        let reference = Sky {
            sun: Some(DirectionalLight {
                direction: unit,
                ..sun
            }),
            ..SAMPLE_SKY
        };
        // In `f32`, the square of the first length rounds to zero and that of the second overflows.
        for length in [1e-30, 1e20] {
            let sky = Sky {
                sun: Some(DirectionalLight {
                    direction: unit * length,
                    ..sun
                }),
                ..SAMPLE_SKY
            };
            assert_eq!(sky.sun_disc(), reference.sun_disc(), "{length}");
            for direction in [Vec3::Y, -unit, Vec3::new(1.0, 0.2, 0.0).normalize()] {
                assert_eq!(
                    sky.radiance(direction),
                    reference.radiance(direction),
                    "{length}, towards {direction}"
                );
            }
        }
    }

    #[test]
    fn the_sun_s_disc_gives_its_illuminance() {
        let (to_sun, radiance) = SAMPLE_SKY.sun_disc();
        let sun = SAMPLE_SKY.sun.expect("the sample's sky has a sun");
        assert!(to_sun.abs_diff_eq(-sun.direction.normalize(), 1e-6));
        // A disc of angular radius r and radiance L gives a surface facing it 2π (1 − cos r) L:
        // for the sun's 0.2665°, 6.7967 × 10⁻⁵ sr, so 100 000 lux is 1.47 × 10⁹ nits.
        let solid_angle = 2.0 * std::f64::consts::PI * (1.0 - SUN_ANGULAR_RADIUS.cos());
        assert!((solid_angle / 6.7967e-5 - 1.0).abs() < 1e-4);
        let illuminance = f64::from(radiance.x) * solid_angle;
        assert!(
            (illuminance / 100_000.0 - 1.0).abs() < 1e-6,
            "{illuminance}"
        );
        assert!((radiance.z / radiance.x - 0.9).abs() < 1e-6);
        assert_eq!(uniform_sky(1.0).sun_disc().1, Vec3::ZERO);
    }

    #[test]
    fn every_texel_direction_reads_back_its_face_and_point() {
        let size = 4;
        for face in 0..6 {
            for y in 0..size {
                for x in 0..size {
                    let (s, t) = (
                        (2 * x + 1) as f32 / size as f32 - 1.0,
                        (2 * y + 1) as f32 / size as f32 - 1.0,
                    );
                    let direction = texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, size);
                    assert_eq!(
                        face_point(direction),
                        (face, s, t),
                        "face {face}, ({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn texel_directions_follow_the_faces_of_slime_gfx_s_cube_textures() {
        // The table of `Gpu::create_cube_texture`, which `slime-gfx`'s `tests/levels.rs` checks
        // on the GPU texel by texel: the point (s, t) of a face, each from 0 at its left or top
        // edge to 1, lies along these directions. A face mirrored or swapped consistently in
        // `texel_direction` and `face_point` passes their round trip, but not this.
        let documented = |face: usize, s: f32, t: f32| {
            let (sc, tc) = (2.0 * s - 1.0, 2.0 * t - 1.0);
            match face {
                0 => Vec3::new(1.0, -tc, -sc),
                1 => Vec3::new(-1.0, -tc, sc),
                2 => Vec3::new(sc, 1.0, tc),
                3 => Vec3::new(sc, -1.0, -tc),
                4 => Vec3::new(sc, -tc, 1.0),
                _ => Vec3::new(-sc, -tc, -1.0),
            }
        };
        let size = 4;
        for face in 0..6 {
            for y in 0..size {
                for x in 0..size {
                    let (s, t) = ((x as f32 + 0.5) / 4.0, (y as f32 + 0.5) / 4.0);
                    assert_eq!(
                        texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, size),
                        documented(face, s, t),
                        "face {face}, ({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn a_uniform_sky_gives_every_level_its_radiance() {
        // The furnace test of the precomputation: whatever the roughness, a surface surrounded by
        // light of one radiance reflects that radiance. The integrals give it within the rounding
        // of their sums; `environment_levels` skips them for such a sky and gives it exactly.
        let computed = prefiltered_levels(CubeMap::from_fn(ENVIRONMENT_SIZE, |_, _, _| {
            Vec3::splat(0.75)
        }));
        let skipped = environment_levels(&uniform_sky(0.75));
        for levels in [&computed, &skipped] {
            assert_eq!(levels.len(), ENVIRONMENT_LEVELS);
            for (level, values) in levels.iter().enumerate() {
                let size = ENVIRONMENT_SIZE >> level;
                assert_eq!(values.len(), (6 * size * size * 4) as usize);
                for texel in values.chunks(4) {
                    assert!(
                        texel[..3].iter().all(|&value| (value - 0.75).abs() <= 1e-6)
                            && texel[3] == 1.0,
                        "level {level}: {texel:?}"
                    );
                }
            }
        }
        assert!(
            skipped
                .iter()
                .flatten()
                .all(|&value| value == 0.75 || value == 1.0)
        );
    }

    #[test]
    fn the_roughest_level_is_the_irradiance_over_pi() {
        // For a radiance a + b (d·axis), the irradiance over π of a surface of normal n is
        // a + 2/3 b (n·axis): the average of d·axis weighted by n·d over the hemisphere around n is
        // 2/3 n·axis. Along each axis, so that a face turned the wrong way shows.
        let (a, b) = (1.0, 0.75);
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            let base = CubeMap::from_fn(ENVIRONMENT_SIZE, |face, x, y| {
                let direction =
                    texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, ENVIRONMENT_SIZE);
                Vec3::splat(a + b * direction.normalize().dot(axis))
            });
            let levels = prefiltered_levels(base);
            let size = ENVIRONMENT_SIZE >> (ENVIRONMENT_LEVELS - 1);
            let mut largest = 0.0_f32;
            for face in 0..6 {
                for y in 0..size {
                    for x in 0..size {
                        let n = texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, size);
                        let expected = a + 2.0 / 3.0 * b * n.normalize().dot(axis);
                        let actual = texel(&levels[ENVIRONMENT_LEVELS - 1], size, face, x, y).x;
                        largest = largest.max((actual / expected - 1.0).abs());
                    }
                }
            }
            println!("along {axis}: largest relative error {largest}");
            assert!(largest <= 0.005, "along {axis}: {largest}");
        }
    }

    /// The irradiance over π that light of radiance `radiance(d)` from each direction d gives a
    /// surface of unit normal `n`: the radiance weighted by n·d over the hemisphere around n, by
    /// the midpoint rule over 600 × 1200 cells of latitude and longitude around n, in f64, from a
    /// formula rather than from a cube map.
    fn irradiance_by_quadrature(radiance: impl Fn(Vec3) -> Vec3, n: Vec3) -> Vec3 {
        use std::f64::consts::PI;
        let n = n.normalize().as_dvec3();
        let (t, b) = n.any_orthonormal_pair();
        let (rows, columns) = (600, 1200);
        let mut sum = glam::DVec3::ZERO;
        for i in 0..rows {
            let theta = (i as f64 + 0.5) / rows as f64 * PI / 2.0;
            let (sin, cos) = theta.sin_cos();
            let weight = cos * sin * (PI / 2.0 / rows as f64) * (2.0 * PI / columns as f64);
            for j in 0..columns {
                let phi = (j as f64 + 0.5) / columns as f64 * 2.0 * PI;
                let d = (t * phi.cos() + b * phi.sin()) * sin + n * cos;
                sum += radiance(d.as_vec3()).as_dvec3() * weight;
            }
        }
        (sum / PI).as_vec3()
    }

    #[test]
    fn the_roughest_level_of_a_sky_is_its_irradiance_over_pi() {
        // The quadrature first checked against a closed form: light of radiance 1 + 0.75 (d·axis)
        // gives 1 + 0.5 (n·axis), whatever the normal.
        let axis = Vec3::new(0.48, 0.6, 0.64);
        for n in [Vec3::Y, -Vec3::Y, Vec3::new(1.0, 0.2, -0.3).normalize()] {
            let linear = irradiance_by_quadrature(|d| Vec3::splat(1.0 + 0.75 * d.dot(axis)), n);
            assert!(
                (linear.x - (1.0 + 0.5 * n.dot(axis))).abs() < 1e-4,
                "{n}: {linear}"
            );
        }
        // Then the level against the sky's own irradiance, at two texels of each face.
        let levels = environment_levels(&SAMPLE_SKY);
        let size = ENVIRONMENT_SIZE >> (ENVIRONMENT_LEVELS - 1);
        let mut largest = 0.0_f32;
        for face in 0..6 {
            for (x, y) in [(size / 2, size / 2), (1, size - 2)] {
                let n = texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, size);
                let expected = irradiance_by_quadrature(|d| SAMPLE_SKY.radiance(d), n);
                let actual = texel(&levels[ENVIRONMENT_LEVELS - 1], size, face, x, y);
                largest = largest.max(((actual - expected) / expected).abs().max_element());
            }
        }
        // Measured: 0.43 % on the sample's sky, whose sun's glow the 2 048 directions of the last
        // level resolve.
        println!("largest relative error {largest}");
        assert!(largest <= 0.0065, "{largest}");
    }

    #[test]
    fn each_level_is_the_sky_averaged_under_its_lobe() {
        // Against the integral the prefilter estimates, computed over every texel of the first
        // level with its exact solid angle, at 24 texels of each level spread over the six faces:
        // the error of the Monte Carlo estimate and of its filtering.
        // 1.5 times the largest errors measured on the sample's sky, 2.2 %, 1.4 %, 1.1 % and
        // 0.31 %: they are largest in the sun's glow, whose peak the narrow lobes resolve least.
        // One percent of the sky's light is below a step of an 8-bit display at the sample's
        // exposure.
        const TOLERANCES: [f32; ENVIRONMENT_LEVELS] = [0.0, 0.033, 0.021, 0.017, 0.005];
        let levels = environment_levels(&SAMPLE_SKY);
        let size = ENVIRONMENT_SIZE;
        let base = CubeMap::from_fn(size, |face, x, y| texel(&levels[0], size, face, x, y));
        for (level, values) in levels.iter().enumerate().skip(1) {
            let level_size = size >> level;
            let roughness = level as f64 / (ENVIRONMENT_LEVELS - 1) as f64;
            let mut largest = 0.0_f32;
            for face in 0..6 {
                for (x, y) in [
                    (0, 0),
                    (level_size / 3, level_size / 2),
                    (level_size / 2, level_size - 1),
                    (level_size - 1, level_size / 4),
                ] {
                    let r = texel_direction(face, x as f32 + 0.5, y as f32 + 0.5, level_size);
                    let expected = prefiltered_by_quadrature(&base, r, roughness);
                    let actual = texel(values, level_size, face, x, y);
                    largest = largest.max(((actual - expected) / expected).abs().max_element());
                }
            }
            println!("level {level}: largest relative error {largest}");
            assert!(largest <= TOLERANCES[level], "level {level}: {largest}");
        }
    }

    /// A and B of the BRDF table at `n_dot_v` and `roughness`, by a product Gauss-Legendre rule
    /// over the half vector's angles, in f64: an integration independent of the table's, which
    /// draws visible normals by a Hammersley sequence. θ_h runs through tan θ_h = α s / (1 − s), s
    /// from 0 to 1, which spreads the nodes over the lobe.
    fn scale_and_bias_by_quadrature(n_dot_v: f64, roughness: f64) -> (f64, f64) {
        use std::f64::consts::PI;
        let (theta_nodes, phi_nodes) = (gauss_legendre(160), gauss_legendre(96));
        let alpha = roughness * roughness;
        let alpha2 = alpha * alpha;
        let v = glam::DVec3::new((1.0 - n_dot_v * n_dot_v).sqrt(), 0.0, n_dot_v);
        let (mut a, mut b) = (0.0, 0.0);
        for &(x, weight_x) in &theta_nodes {
            let s = 0.5 * (x + 1.0);
            let t = alpha * s / (1.0 - s);
            let theta = t.atan();
            let d_theta = alpha / ((1.0 - s) * (1.0 - s)) / (1.0 + t * t) * 0.5 * weight_x;
            let (sin, cos) = theta.sin_cos();
            let f = sin * sin + cos * cos * alpha2;
            let distribution = alpha2 / (PI * f * f);
            for &(y, weight_y) in &phi_nodes {
                let phi = PI * (y + 1.0);
                let h = glam::DVec3::new(sin * phi.cos(), sin * phi.sin(), cos);
                let v_dot_h = v.dot(h);
                let n_dot_l = 2.0 * v_dot_h * h.z - v.z;
                if v_dot_h <= 0.0 || n_dot_l <= 0.0 {
                    continue;
                }
                // `visibility_ggx` of shader.wgsl.
                let visibility = 0.5
                    / (n_dot_l * (n_dot_v * n_dot_v * (1.0 - alpha2) + alpha2).sqrt()
                        + n_dot_v * (n_dot_l * n_dot_l * (1.0 - alpha2) + alpha2).sqrt());
                // dω_l = 4 (v·h) dω_h, and dω_h = sin θ dθ dφ.
                let value = distribution
                    * visibility
                    * n_dot_l
                    * 4.0
                    * v_dot_h
                    * sin
                    * d_theta
                    * PI
                    * weight_y;
                let fresnel = (1.0 - v_dot_h).powi(5);
                a += value * (1.0 - fresnel);
                b += value * fresnel;
            }
        }
        (a, b)
    }

    /// The nodes and weights of the Gauss-Legendre rule of `count` points on [−1, 1].
    fn gauss_legendre(count: usize) -> Vec<(f64, f64)> {
        (0..count)
            .map(|i| {
                let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (count as f64 + 0.5)).cos();
                loop {
                    // Legendre's polynomial of degree `count` and its derivative at x.
                    let (mut p, mut previous) = (1.0, 0.0);
                    for j in 0..count {
                        (p, previous) = (
                            ((2 * j + 1) as f64 * x * p - j as f64 * previous) / (j + 1) as f64,
                            p,
                        );
                    }
                    let derivative = count as f64 * (x * p - previous) / (x * x - 1.0);
                    let step = p / derivative;
                    x -= step;
                    if step.abs() < 1e-15 {
                        return (x, 2.0 / ((1.0 - x * x) * derivative * derivative));
                    }
                }
            })
            .collect()
    }

    #[test]
    fn the_brdf_table_matches_an_independent_integration() {
        let size = BRDF_TABLE_SIZE as usize;
        let last = (size - 1) as f64;
        let table = &*BRDF_TABLE;
        for j in (0..size).step_by(5).chain([size - 1]) {
            for i in (0..size).step_by(5).chain([1, size - 1]) {
                let (n_dot_v, roughness) = ((i as f64 / last).max(1e-4), j as f64 / last);
                let (a, b) = if j == 0 {
                    // A mirror: the closed form.
                    let fresnel = (1.0 - n_dot_v).powi(5);
                    (1.0 - fresnel, fresnel)
                } else {
                    scale_and_bias_by_quadrature(n_dot_v, roughness)
                };
                let start = 4 * (j * size + i);
                let (table_a, table_b) = (f64::from(table[start]), f64::from(table[start + 1]));
                assert!(
                    (table_a - a).abs() <= 2.5e-3 && (table_b - b).abs() <= 2.5e-3,
                    "n·v {n_dot_v:.3}, roughness {roughness:.3}: ({table_a}, {table_b}), not ({a}, \
                     {b})"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "the sky's horizon radiance is")]
    fn a_sky_brighter_than_half_precision_holds_is_refused() {
        let _ = environment_levels(&Sky {
            horizon: LinearColor::rgb(1.0, 70_000.0, 1.0),
            ..SAMPLE_SKY
        });
    }

    #[test]
    #[should_panic(expected = "the sky's ground radiance is")]
    fn a_negative_or_undefined_sky_is_refused() {
        let _ = environment_levels(&Sky {
            ground: LinearColor::rgb(1.0, f32::NAN, -1.0),
            ..SAMPLE_SKY
        });
    }

    #[test]
    #[ignore = "a measurement: cargo test --release -p slime-render --lib -- --ignored --nocapture"]
    fn measure_the_precomputation() {
        let started = std::time::Instant::now();
        let _ = &*BRDF_TABLE;
        println!("BRDF table: {:?}", started.elapsed());
        let mut times: Vec<_> = (0..11)
            .map(|_| {
                let started = std::time::Instant::now();
                let _ = environment_levels(&SAMPLE_SKY);
                started.elapsed()
            })
            .collect();
        times.sort();
        println!(
            "the sample's environment: median {:?}, from {:?} to {:?}",
            times[5], times[0], times[10]
        );
    }
}
