//! What the player sees: the game's world, the zone and the hero, on an overcast day or at night,
//! drawn by the engine's renderer from the cooked data that the asset cache holds
//! (`slime_render::AssetCache`), through `slime-world-render`. The GPU objects are made when the
//! window opens, and made again with a new GPU device; the world keeps the run meanwhile. The hero
//! loads before the first frame, and the rest of the world in the background: the sky and the hero
//! show at once, lit by the zone's lights, and the zone, whole, once it is loaded. The campfire
//! flickers, its flames, its embers and its light together, and the key glows in pulses at night,
//! on game time: the view changes their materials' values, and the fire's light in the frame it
//! draws, not in the world, at each frame. The view builds each frame from the world when the
//! presentation extracts it ([`View::extract`]), and draws it without the world
//! ([`View::render`]), as `docs/adr/0111-the-frame-contract.md` says.

use std::f64::consts::TAU;
use std::fmt::Display;
use std::time::Duration;

use slime_app::AppError;
use slime_assets::{AssetId, AssetKind, CookedAssets};
use slime_color::LinearColor;
use slime_gfx::{Gpu, RenderTarget, TextureFormat};
use slime_render::glam::Vec3;
use slime_render::{
    AssetCache, AssetHandle, CacheReport, Camera, MaterialValues, Priority, RenderError,
    RenderStats, Renderer, Scene, ShaderSources, Sky, ViewDraw, ViewSettings,
};
use slime_world::{Entity, Light, Meshes, Transform, World};
use slime_world_render::FrameBuilder;

use crate::assets;
use crate::zone::{Flickers, TimeOfDay, descends};

/// An overcast sky, in nits: grey, brighter overhead than at the horizon, which lights the ground
/// with about 5 000 lux; the sun shows no disc through the clouds. The ground below the horizon is
/// dark soil of albedo about 0.12, lit by the sun and the sky: 0.12 × (9 000 + 5 000) / π ≈ 530.
const OVERCAST: Sky = Sky {
    zenith: LinearColor::rgb(2000.0, 2100.0, 2300.0),
    horizon: LinearColor::rgb(1500.0, 1550.0, 1600.0),
    ground: LinearColor::rgb(540.0, 530.0, 500.0),
    sun: None,
};
/// What the scene's light is multiplied by by day: a white matte surface facing the sun, which
/// receives about 17 000 lux from it and the sky, shows at about 0.7, the light of a grey day.
const DAY_EXPOSURE: f32 = std::f32::consts::PI / 25_000.0;
/// The night sky, in nits: a dark blue, lighter towards the horizon, around the moon's disc, which
/// the zone's moon gives; the ground below the horizon nearly black.
const NIGHT_SKY: Sky = Sky {
    zenith: LinearColor::rgb(0.03, 0.045, 0.1),
    horizon: LinearColor::rgb(0.06, 0.075, 0.12),
    ground: LinearColor::rgb(0.006, 0.006, 0.007),
    sun: None,
};
/// What the scene's light is multiplied by at night: a white matte surface 2 m below a lantern,
/// which receives about 35 lux, shows at about 3, bright under its light, while what the moon's
/// 0.8 lux alone light stays dim but readable.
const NIGHT_EXPOSURE: f32 = 0.3;
/// The bloom's strength, as in the engine's samples.
const BLOOM: f32 = 0.04;
/// The names of the zone's materials that the view changes: the campfire's flames and embers, which
/// flicker, and the key's brass, which pulses at night (`games/hunnawood/zone/props.py`).
const FLAMES: &str = "flames";
const EMBERS: &str = "embers";
const KEY: &str = "brass";
/// The waves whose sum makes the fire flicker: each a frequency in hertz, an amplitude and a phase
/// in turns. In thousandths of a hertz, their frequencies have no common divisor but 1, so that
/// the flicker repeats only every 1 000 seconds; their amplitudes add to at most a quarter of the
/// fire's light.
const FLICKER: [(f64, f64, f64); 4] = [
    (1.303, 0.10, 0.0),
    (2.917, 0.07, 0.17),
    (6.131, 0.05, 0.42),
    (11.719, 0.03, 0.71),
];
/// The seconds between two pulses of the key's glow, and how many times its glow its brightest
/// pulse reaches.
const KEY_PULSE_PERIOD: f64 = 2.0;
const KEY_PULSE_PEAK: f64 = 5.0;

/// What the fire's light and glow are multiplied by `seconds` after the start: from 0.75 to 1.25.
fn flicker(seconds: f64) -> f32 {
    // The part of a turn first, so that a run of hours keeps the waves smooth in f64's precision.
    let waves: f64 = FLICKER
        .iter()
        .map(|&(frequency, amplitude, phase)| {
            amplitude * (TAU * (seconds * frequency + phase).fract()).sin()
        })
        .sum();
    (1.0 + waves) as f32
}

/// What the key's glow is multiplied by `seconds` after the start, at night: a smooth pulse every
/// [`KEY_PULSE_PERIOD`], from 1 between pulses to [`KEY_PULSE_PEAK`] at their height.
fn key_pulse(seconds: f64) -> f32 {
    let rise = 0.5 - 0.5 * (TAU * (seconds / KEY_PULSE_PERIOD).fract()).cos();
    // Cubed, so that the glow rises late and falls early: the key rests dim between pulses.
    (1.0 + (KEY_PULSE_PEAK - 1.0) * rise.powi(3)) as f32
}

/// Makes the lights of a frame of `world` whose entities flicker ([`Flickers`]) shine as they do
/// `seconds` of game time after the start: around the intensity the zone gave them, by the fire's
/// flicker. The world's lights stay as the zone gave them.
pub fn flicker_the_fire(world: &World, lights: &mut [(Entity, slime_render::Light)], seconds: f64) {
    let times = flicker(seconds);
    for (entity, light) in lights {
        if let (Some(flickers), slime_render::Light::Point(point)) =
            (world.get::<Flickers>(*entity), light)
        {
            point.intensity = flickers.intensity * times;
        }
    }
}

/// A material that the view changes at each frame: its asset ID, and the strength of its emission
/// as it was cooked.
struct Glowing {
    id: AssetId,
    emissive_strength: f32,
}

impl Glowing {
    /// The material of `assets` named `name`, the first by ID when several are; a warning when none
    /// is, since the zone then looks as it was made.
    fn find(assets: &CookedAssets, name: &str) -> Option<Self> {
        let found = assets
            .manifest()
            .iter()
            .filter(|(_, entry)| entry.kind == AssetKind::Material)
            .find_map(|(id, _)| {
                let material = assets.material(id).ok()?;
                (material.name == name).then_some(Self {
                    id,
                    emissive_strength: material.emissive_strength,
                })
            });
        if found.is_none() {
            tracing::warn!("the cooked data has no material `{name}`: it does not change");
        }
        found
    }

    /// Draws it from now on with its emission's strength `times` as strong as it was cooked, once
    /// `cache` holds it.
    fn glow(&self, cache: &AssetCache, gpu: &Gpu, times: f32) -> Result<(), RenderError> {
        let Some(material) = cache.material(self.id) else {
            return Ok(());
        };
        material.set_values(
            gpu,
            MaterialValues {
                emissive_strength: self.emissive_strength * times,
                ..material.values()
            },
        )
    }
}

/// The materials that glow: the campfire's flames and embers, and the key's brass.
struct Glow {
    flames: Option<Glowing>,
    embers: Option<Glowing>,
    key: Option<Glowing>,
}

impl Glow {
    /// The materials of `assets` that glow, by their names.
    fn find(assets: &CookedAssets) -> Self {
        Self {
            flames: Glowing::find(assets, FLAMES),
            embers: Glowing::find(assets, EMBERS),
            key: Glowing::find(assets, KEY),
        }
    }
}

/// The meshes that the entities of `world` at or below `root` place.
fn meshes_below(world: &World, root: Entity) -> Vec<AssetId> {
    world
        .query::<&Meshes>()
        .filter(|&(entity, _)| descends(world, entity, root))
        .flat_map(|(_, meshes)| meshes.ids.iter().copied())
        .collect()
}

/// A world that holds only the lights of `world` that shine at night, or by day, as `night` says,
/// where they stand: a frame of it computes the sky of that time of day, around the moon at night,
/// as a frame of `world` at that time would.
fn lights_at(world: &World, night: bool) -> World {
    let time = if night {
        TimeOfDay::Night
    } else {
        TimeOfDay::Day
    };
    let mut lights = World::new();
    for (entity, light) in world.query::<&Light>() {
        if world
            .get::<TimeOfDay>(entity)
            .is_some_and(|shines| *shines != time)
        {
            continue;
        }
        let Some(at) = world.global_transform(entity) else {
            continue;
        };
        let copy = lights.spawn();
        lights.insert(
            copy,
            Transform {
                translation: at.translation,
                rotation: at.rotation,
                scale: at.scale,
                stretch: Vec3::ONE,
            },
        );
        lights.insert(copy, *light);
    }
    lights
}

/// The error of a load of `what` that failed, with how to cook the data it reads.
fn cannot_load(what: &str, error: impl Display) -> AppError {
    format!(
        "cannot load the cooked {what}: {error}; {}",
        assets::HOW_TO_COOK
    )
    .into()
}

/// The renderer, the GPU objects of what the world draws, and what builds its frames, for targets
/// of one format.
pub struct View {
    renderer: Renderer,
    /// What drawing the hero's camera into the window keeps from one frame to the next.
    shown: slime_render::View,
    cache: AssetCache,
    /// The handles of the hero's meshes, which the cache keeps for the run, whatever the world
    /// draws.
    hero: Vec<AssetHandle>,
    /// What builds the frames by day, around the overcast sky, and at night, around the night sky
    /// and its moon: each keeps the environment of its sky, so that each is computed once.
    day: FrameBuilder,
    night: FrameBuilder,
    glow: Glow,
    /// Whether a frame drew every mesh that the world names: the zone, whole.
    zone_shown: bool,
    /// The frame that `extract` built, which `render` draws, and the camera it is seen from.
    scene: Option<(Scene, Camera)>,
}

impl View {
    /// Creates the renderer for targets of `format` and the asset cache of `assets`, and returns
    /// once the meshes of the hero, the entity `hero` of `world` and its descendants, are loaded,
    /// so that the first frame draws the hero: its first shadows change the frame's shape, which
    /// they would do in the middle of the zone's load otherwise
    /// (`docs/adr/0074-uploading-textures-without-holding-up-frames.md`). While the hero loads, the
    /// rest of what `world` draws starts loading in the background, and the skies of both times of
    /// day are computed, the time the game is at saying which is drawn first (`night`), so that
    /// resting by the campfire computes none. The zone is shown once a frame draws it whole
    /// ([`View::zone_shown`]).
    ///
    /// The game asks for the hero's meshes itself, as ADR-0074 decided: they are what the first
    /// frame draws, which the frame builder would ask for only then.
    ///
    /// # Errors
    ///
    /// When the renderer or the skies cannot be created, or the hero or the zone cannot be loaded
    /// before the hero is, with a message that says how to cook them.
    pub fn new(
        gpu: &Gpu,
        format: TextureFormat,
        assets: &CookedAssets,
        world: &World,
        hero: Entity,
        night: bool,
    ) -> Result<Self, AppError> {
        let renderer = Renderer::new(gpu, format)?;
        let mut view = Self {
            cache: AssetCache::new(&renderer, assets.clone()),
            hero: Vec::new(),
            shown: renderer.create_view(gpu, "hero")?,
            renderer,
            day: FrameBuilder::new(Some(OVERCAST)),
            night: FrameBuilder::new(Some(NIGHT_SKY)),
            glow: Glow::find(assets),
            zone_shown: false,
            scene: None,
        };
        let hero = meshes_below(world, hero);
        // Held for the run, as the frame builders hold what the world names.
        view.hero = view
            .cache
            .request(gpu, hero.iter().copied(), Priority::Now)
            .into_handles();
        // A frame of the world asks for what else it draws, in a batch of its own, and computes the
        // sky of the time the game is at; one of the other time's lights alone, that time's sky.
        let mut errors = Vec::new();
        for at_night in [night, !night] {
            let other;
            let lit = if at_night == night {
                world
            } else {
                other = lights_at(world, at_night);
                &other
            };
            let builder = if at_night {
                &mut view.night
            } else {
                &mut view.day
            };
            errors.extend(builder.build(lit, &mut view.cache, gpu, 1.0)?.errors);
        }
        // Errors first: the cache gives each once (`AssetCache::poll`), and the first frames would
        // not see them again.
        loop {
            if let Some((_, error)) = errors.first() {
                // A load that a lost device failed misses no data: the loss is the run's to
                // handle (`Context::exit_with_error`), which the message of `cannot_load` hides.
                gpu.check()?;
                return Err(cannot_load("zone or hero", error));
            }
            if hero.iter().all(|&id| view.cache.mesh(id).is_some()) {
                return Ok(view);
            }
            std::thread::sleep(Duration::from_millis(1));
            errors = view.cache.poll(gpu);
        }
    }

    /// Starts loading again what changed in `assets`, the cooked data read again, in the
    /// background ([`AssetCache::reload`]): what the world draws stays as it is drawn until all of
    /// it is loaded. A game calls it once the view is settled ([`View::settled`]).
    pub fn reload(&mut self, gpu: &Gpu, assets: CookedAssets) {
        self.glow = Glow::find(&assets);
        self.cache.reload(gpu, assets);
    }

    /// Makes the renderer's pipelines again from `sources` ([`Renderer::set_shaders`]): the frames
    /// are drawn with them from now on, or as before if they do not compile.
    ///
    /// # Errors
    ///
    /// As [`Renderer::set_shaders`].
    pub fn set_shaders(&mut self, gpu: &Gpu, sources: &ShaderSources) -> Result<(), RenderError> {
        self.renderer.set_shaders(gpu, sources)
    }

    /// Whether nothing loads, nor loads again: a reload started then stops no load half-way.
    pub fn settled(&self) -> bool {
        self.cache.loading() == 0 && !self.cache.reloading()
    }

    /// Whether a frame drew the zone whole.
    pub fn zone_shown(&self) -> bool {
        self.zone_shown
    }

    /// Builds the frame of `world` that [`View::render`] draws, as `camera` sees it, by day or at
    /// night as `night` says, what moves `alpha` of the way between its last two fixed steps
    /// (`GameTime::alpha`), the materials that glow and the fire's light as they do `seconds` of
    /// game time after the start: the sky, and what the world places once the cache holds it. The
    /// campfire's flames and embers flicker with its light ([`flicker_the_fire`]), and the key
    /// pulses at night.
    ///
    /// # Errors
    ///
    /// As [`Material::set_values`](slime_render::Material::set_values) and
    /// [`FrameBuilder::build`]; and when the meshes of the world cannot be loaded before the zone
    /// is first shown, with a message that says how to cook them. Once it is shown, meshes that
    /// fail to load are logged, and not drawn.
    pub fn extract(
        &mut self,
        gpu: &Gpu,
        world: &World,
        night: bool,
        alpha: f64,
        seconds: f64,
        camera: Camera,
    ) -> Result<(), AppError> {
        // The loads that finished first, so that the materials they bring glow from their first
        // frame: the frame builder, which takes them otherwise, holds the cache while its frame is.
        let taken = self.cache.poll(gpu);
        for (id, error) in &taken {
            tracing::error!(%id, "a mesh of the world cannot be drawn: {error}");
        }
        let flicker = flicker(seconds);
        // The embers glow steadier than the flames, with the same flicker.
        let embers = 1.0 + (flicker - 1.0) / 2.0;
        let key = if night { key_pulse(seconds) } else { 1.0 };
        for (glowing, times) in [
            (&self.glow.flames, flicker),
            (&self.glow.embers, embers),
            (&self.glow.key, key),
        ] {
            if let Some(glowing) = glowing {
                glowing.glow(&self.cache, gpu, times)?;
            }
        }
        let (builder, exposure) = if night {
            (&mut self.night, NIGHT_EXPOSURE)
        } else {
            (&mut self.day, DAY_EXPOSURE)
        };
        let mut frame = builder.build(world, &mut self.cache, gpu, alpha)?;
        if !self.zone_shown {
            if let Some((_, error)) = taken.first().or(frame.errors.first()) {
                // As in `View::new`: the loss itself, not a message that hides it.
                gpu.check()?;
                return Err(cannot_load("zone", error));
            }
            self.zone_shown = frame.loading == 0;
        }
        flicker_the_fire(world, &mut frame.lights, seconds);
        let mut scene = frame.scene(camera.eye);
        scene.exposure = exposure;
        scene.bloom = BLOOM;
        self.scene = Some((scene, camera));
        Ok(())
    }

    /// Changes how the hero's camera is drawn, from the next frame
    /// ([`slime_render::View::set_settings`]).
    pub fn set_settings(&mut self, settings: ViewSettings) {
        self.shown.set_settings(settings);
    }

    /// What its asset cache holds and loads ([`AssetCache::report`]).
    pub fn assets(&self) -> CacheReport {
        self.cache.report()
    }

    /// The size the hero's camera is drawn at ([`slime_render::View::render_size`]), which the dev
    /// overlay shows.
    #[cfg(feature = "development")]
    pub fn render_size(&self) -> Option<(u32, u32)> {
        self.shown.render_size()
    }

    /// Draws into `target` the frame that [`View::extract`] built, and returns what it cost;
    /// `None` when no frame was built since the last one drawn.
    ///
    /// # Errors
    ///
    /// As [`Renderer::render`].
    pub fn render(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
    ) -> Result<Option<RenderStats>, RenderError> {
        let Some((scene, camera)) = self.scene.take() else {
            return Ok(None);
        };
        let mut draws = [ViewDraw::new(&mut self.shown, camera, target)];
        self.renderer.render(gpu, &scene, &mut draws)?;
        Ok(Some(self.shown.stats()))
    }

    /// The view's renderer, which the dev overlay draws its panels with after the view
    /// (`docs/adr/0152-the-dev-overlay-on-the-engines-interface.md`).
    #[cfg(feature = "development")]
    pub fn renderer(&mut self) -> &mut Renderer {
        &mut self.renderer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::TimeOfDay;
    use crate::zone::cooked::{game, zone};

    /// The values of `wave` over two minutes, at 600 frames a second.
    fn sampled(wave: fn(f64) -> f32) -> Vec<f32> {
        (0..72_000)
            .map(|frame| wave(frame as f64 / 600.0))
            .collect()
    }

    #[test]
    fn the_fire_flickers_around_its_light_without_going_out() {
        let values = sampled(flicker);
        let (low, high) = values.iter().fold((f32::MAX, f32::MIN), |(low, high), &v| {
            (low.min(v), high.max(v))
        });
        assert!((0.75..0.85).contains(&low), "{low}");
        assert!((1.15..=1.25).contains(&high), "{high}");
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        assert!((mean - 1.0).abs() < 0.01, "{mean}");
        // Without jumps: at 600 frames a second, its waves change it by less than 0.011 a frame.
        let steepest = values
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0, f32::max);
        assert!(steepest < 0.011, "{steepest}");
        // As smooth after a day of play.
        let day = 86_400.0;
        assert!((flicker(day + 1.0 / 600.0) - flicker(day)).abs() < 0.011);
    }

    #[test]
    fn the_key_pulses_once_a_period_and_rests_dim_between_pulses() {
        assert_eq!(key_pulse(0.0), 1.0);
        let peak = key_pulse(KEY_PULSE_PERIOD / 2.0);
        assert!((peak - KEY_PULSE_PEAK as f32).abs() < 1e-5, "{peak}");
        assert_eq!(key_pulse(KEY_PULSE_PERIOD * 3.5), peak);
        // Dim, within a tenth of the peak's rise, for nearly half of each period.
        let values = sampled(key_pulse);
        let dim = values
            .iter()
            .filter(|&&v| v < 1.0 + 0.1 * (KEY_PULSE_PEAK as f32 - 1.0))
            .count() as f32
            / values.len() as f32;
        assert!((0.45..0.5).contains(&dim), "{dim}");
    }

    #[test]
    fn only_the_lights_that_flicker_change_with_the_fire() {
        let mut world = World::new();
        let steady = world.spawn();
        let fire = world.spawn();
        let point = Light::Point {
            color: LinearColor::WHITE,
            intensity: 10.0,
            range: None,
        };
        world.insert(steady, point);
        world.insert(fire, point);
        world.insert(fire, Flickers { intensity: 20.0 });
        let drawn = |intensity| {
            slime_render::Light::Point(slime_render::PointLight {
                position: slime_render::glam::DVec3::ZERO,
                color: LinearColor::WHITE,
                intensity,
                range: None,
            })
        };
        let mut lights = [(steady, drawn(10.0)), (fire, drawn(10.0))];
        let seconds = 0.4;
        flicker_the_fire(&world, &mut lights, seconds);
        assert_eq!(
            lights,
            [
                (steady, drawn(10.0)),
                (fire, drawn(20.0 * flicker(seconds)))
            ]
        );
        assert_eq!(
            world.get::<Light>(fire),
            Some(&point),
            "the world's light stays"
        );
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_has_the_fires_light_and_the_materials_that_glow() {
        let (assets, _) = zone();
        let (world, _) = game();
        let fires: Vec<_> = world.query::<(&Flickers, &Light, &TimeOfDay)>().collect();
        let [(_, (flickers, light, time))] = fires[..] else {
            panic!(
                "{} lights flicker, where the campfire's should",
                fires.len()
            );
        };
        assert_eq!(*time, TimeOfDay::Night);
        assert!(
            matches!(light, Light::Point { intensity, .. } if *intensity == flickers.intensity),
            "{light:?}"
        );
        let glow = Glow::find(&assets);
        for (name, glowing) in [
            (FLAMES, glow.flames),
            (EMBERS, glow.embers),
            (KEY, glow.key),
        ] {
            assert!(glowing.is_some(), "no material `{name}`");
        }
    }

    /// The hero, asked for at priority now after the rest of the world at priority soon, arrives
    /// first: a request's class comes before its order, and the hero, which the first frame draws,
    /// does not wait for the zone (`docs/adr/0113-asset-requests-residency-and-priorities.md`).
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_hero_arrives_before_the_zone_asked_for_before_it() {
        use std::collections::HashSet;
        use std::time::Instant;

        let (assets, _) = zone();
        let (world, game) = game();
        let hero: Vec<AssetId> = meshes_below(&world, game.hero_entity());
        let held: HashSet<AssetId> = hero.iter().copied().collect();
        let mut rest = Vec::new();
        for (_, meshes) in world.query::<&Meshes>() {
            rest.extend(meshes.ids.iter().filter(|id| !held.contains(id)));
        }
        let gpu = Gpu::new().expect("a GPU adapter must be available");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        let mut cache = AssetCache::new(&renderer, assets);
        let zone = cache.request(&gpu, rest, Priority::Soon);
        let hero = cache.request(&gpu, hero, Priority::Now);
        let started = Instant::now();
        while !hero.is_done() {
            assert!(!zone.is_done(), "the zone arrived before the hero");
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "the hero still loads"
            );
            std::thread::sleep(Duration::from_millis(1));
            let failed = cache.poll(&gpu);
            assert!(failed.is_empty(), "{failed:?}");
        }
        assert!(!zone.is_done(), "the zone arrived with the hero");
        eprintln!(
            "the hero arrived in {:?}, the zone's {} meshes loading",
            started.elapsed(),
            cache.loading()
        );
    }
}
