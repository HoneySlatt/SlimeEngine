//! What the player sees: the zone and the hero, on an overcast day or at night, drawn by the
//! engine's renderer from the cooked data and the game's state. The GPU objects are made once, when
//! the window opens, and kept for every frame and every new game. The hero loads before the first
//! frame, and the zone in the background: the sky and the hero show at once, lit by the zone's
//! lights, and the zone, whole, once it is loaded. The campfire flickers, its flames, its embers
//! and its light together, and the key glows in pulses at night: the view changes their materials'
//! values and the fire's light at each frame.

use std::f64::consts::TAU;

use slime_app::AppError;
use slime_assets::{CookedAssets, CookedScene};
use slime_gfx::{Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::{Mat4, Quat, Vec2, Vec3};
use slime_render::{
    Camera, Environment, Light, LoadedScene, Material, MaterialValues, PointLight, RenderError,
    RenderStats, Renderer, Scene, SceneError, SceneLoad, Sky, scene_lights,
};

use crate::assets;
use crate::rules::Game;
use crate::zone::text;

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
/// The night sky, in nits: a dark blue, lighter towards the horizon, the moon's disc in it; the
/// ground below the horizon nearly black.
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
/// How far from the campfire, on the ground, its light stands, at most, in metres: the zone places
/// it above the fire pit.
const CAMPFIRE_LIGHT_REACH: f32 = 1.0;
/// The waves whose sum makes the fire flicker: each a frequency in hertz, an amplitude and a phase
/// in turns. Their frequencies have no common multiple within minutes, so that the flicker does
/// not show a period; their amplitudes add to at most a quarter of the fire's light.
const FLICKER: [(f64, f64, f64); 4] = [
    (1.3, 0.10, 0.0),
    (2.9, 0.07, 0.17),
    (6.1, 0.05, 0.42),
    (11.7, 0.03, 0.71),
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

/// The zone's lights, by the time of day they shine at, read from its cooked scene before it is
/// loaded.
#[derive(Clone)]
pub struct ZoneLights {
    /// The sun.
    day: Vec<Light>,
    /// The moon, the lanterns and the campfire.
    night: Vec<Light>,
    /// The index in `night` of the campfire's light, which flickers.
    campfire: Option<usize>,
}

impl ZoneLights {
    /// The lights of the zone's cooked scene `zone`: those whose node's `time` is `"day"`, and
    /// those whose node's `time` is `"night"`, each in the order of the nodes; the campfire's light
    /// is the point light of the night within [`CAMPFIRE_LIGHT_REACH`] of `campfire`, on the
    /// ground, if one is.
    ///
    /// # Errors
    ///
    /// When a node places a light where it cannot shine ([`scene_lights`]).
    pub fn of(zone: &CookedScene, campfire: Vec2) -> Result<Self, SceneError> {
        // The scene's lights come in the order of the nodes that place them.
        let lights = scene_lights(zone)?;
        let of = |time: &str| -> Vec<Light> {
            zone.nodes
                .iter()
                .filter(|node| node.light.is_some())
                .zip(&lights)
                .filter(|(node, _)| text(node, "time") == Some(time))
                .map(|(_, light)| *light)
                .collect()
        };
        let night = of("night");
        let campfire = night.iter().position(|light| match light {
            Light::Point(light) => {
                Vec2::new(light.position.x, light.position.z).distance(campfire)
                    <= CAMPFIRE_LIGHT_REACH
            }
            _ => false,
        });
        Ok(Self {
            day: of("day"),
            night,
            campfire,
        })
    }
}

/// What lights the zone at one time of day.
struct Lighting {
    /// The zone's lights of that time.
    lights: Vec<Light>,
    environment: Environment,
    exposure: f32,
}

/// A material of the zone that the view changes at each frame, and the values it was made with.
struct Glowing {
    material: Material,
    made: MaterialValues,
}

impl Glowing {
    /// The material of `scene` named `name`, if it has one; a warning otherwise, since the zone
    /// then looks as it was made.
    fn find(scene: &LoadedScene, name: &str) -> Option<Self> {
        let Some(material) = scene.material(name) else {
            tracing::warn!("the zone has no material `{name}`: it does not change");
            return None;
        };
        Some(Self {
            made: material.values(),
            material: material.clone(),
        })
    }

    /// Draws it from now on with its emission's strength `times` as strong as it was made.
    fn glow(&self, gpu: &Gpu, times: f32) -> Result<(), RenderError> {
        self.material.set_values(
            gpu,
            MaterialValues {
                emissive_strength: self.made.emissive_strength * times,
                ..self.made
            },
        )
    }
}

/// The zone, loaded, and what is drawn of it.
struct ZoneView {
    scene: LoadedScene,
    /// The node that places each of the zone's instances, to leave out the key once taken and the
    /// door's leaves once open.
    nodes: Vec<usize>,
    flames: Option<Glowing>,
    embers: Option<Glowing>,
    key: Option<Glowing>,
}

/// The renderer, the light of each time of day, and the GPU objects of the zone and the hero, for
/// targets of one format.
pub struct View {
    renderer: Renderer,
    day: Lighting,
    night: Lighting,
    /// The campfire's light, as the zone places it, and its index in `night`'s lights.
    campfire: Option<(usize, PointLight)>,
    hero: LoadedScene,
    /// The zone's load, until [`View::poll`] takes the zone from it.
    zone_load: Option<SceneLoad>,
    zone: Option<ZoneView>,
}

/// The error of a load of `what` that failed, with how to cook the data it reads.
fn cannot_load(what: &str, error: SceneError) -> AppError {
    format!(
        "cannot load the cooked {what}: {error}; {}",
        assets::HOW_TO_COOK
    )
    .into()
}

impl View {
    /// Creates the renderer, starts loading the zone from `assets` in the background, loads the
    /// hero, and computes the light of the overcast sky and of the night sky, whose moon is the
    /// zone's directional light of the night, for targets of `format`. [`View::poll`] takes the zone
    /// once it is loaded.
    ///
    /// # Errors
    ///
    /// When the renderer or the skies cannot be created, or the hero cannot be loaded, with a
    /// message that says how to cook it.
    pub fn new(
        gpu: &Gpu,
        format: TextureFormat,
        assets: &CookedAssets,
        lights: ZoneLights,
    ) -> Result<Self, AppError> {
        let renderer = Renderer::new(gpu, format)?;
        // The hero is drawn from the first frame: the first shadows that the renderer draws make it
        // create its shadow atlas and its frame again, which the frames that the zone's load takes
        // would wait for (ADR-0074). Its load goes first, and the skies are computed while it runs.
        let hero_load = renderer.load_scene_in_background(gpu, assets, assets::hero_id());
        let zone_load = renderer.load_scene_in_background(gpu, assets, assets::zone_id());
        let moon = lights.night.iter().find_map(|light| match light {
            Light::Directional(moon) => Some(*moon),
            _ => None,
        });
        let night_sky = Sky {
            sun: moon,
            ..NIGHT_SKY
        };
        let campfire = lights.campfire.and_then(|index| match lights.night[index] {
            Light::Point(light) => Some((index, light)),
            _ => None,
        });
        Ok(Self {
            campfire,
            day: Lighting {
                lights: lights.day,
                environment: renderer.create_environment(gpu, &OVERCAST)?,
                exposure: DAY_EXPOSURE,
            },
            night: Lighting {
                lights: lights.night,
                environment: renderer.create_environment(gpu, &night_sky)?,
                exposure: NIGHT_EXPOSURE,
            },
            hero: hero_load
                .wait()
                .map_err(|error| cannot_load("hero", error))?,
            zone_load: Some(zone_load),
            zone: None,
            renderer,
        })
    }

    /// Takes the zone from its load, once it is loaded. Called at each frame, before
    /// [`View::draw`].
    ///
    /// # Errors
    ///
    /// When the zone could not be loaded, with a message that says how to cook it.
    pub fn poll(&mut self) -> Result<(), AppError> {
        if let Some(load) = &mut self.zone_load
            && let Some(result) = load.take()
        {
            self.zone_load = None;
            let scene = result.map_err(|error| cannot_load("zone", error))?;
            self.zone = Some(ZoneView {
                nodes: scene.instance_nodes(),
                flames: Glowing::find(&scene, FLAMES),
                embers: Glowing::find(&scene, EMBERS),
                key: Glowing::find(&scene, KEY),
                scene,
            });
        }
        Ok(())
    }

    /// Whether the zone is loaded, and drawn.
    pub fn zone_shown(&self) -> bool {
        self.zone.is_some()
    }

    /// Draws `game` into `target` as `camera` sees it, by day or at night as the game is, the hero
    /// `alpha` of the way between its last two fixed steps ([`Game::hero_between`]), the fire and
    /// the key as they glow `seconds` after the start: the sky, the zone's lights and the hero, and
    /// the zone once it is loaded.
    ///
    /// # Errors
    ///
    /// As [`Renderer::render`].
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        game: &Game,
        alpha: f32,
        seconds: f64,
        camera: Camera,
    ) -> Result<RenderStats, RenderError> {
        let flicker = flicker(seconds);
        if let Some((index, light)) = self.campfire {
            self.night.lights[index] = Light::Point(PointLight {
                intensity: light.intensity * flicker,
                ..light
            });
        }
        let mut instances = Vec::new();
        if let Some(zone) = &self.zone {
            // The embers glow steadier than the flames, with the same flicker.
            let embers = 1.0 + (flicker - 1.0) / 2.0;
            let key = if game.night() {
                key_pulse(seconds)
            } else {
                1.0
            };
            for (glowing, times) in [
                (&zone.flames, flicker),
                (&zone.embers, embers),
                (&zone.key, key),
            ] {
                if let Some(glowing) = glowing {
                    glowing.glow(gpu, times)?;
                }
            }
            let places = game.zone();
            instances.extend(
                zone.scene
                    .instances()
                    .into_iter()
                    .zip(&zone.nodes)
                    .filter(|(_, node)| {
                        !(game.has_item() && places.item_nodes.contains(node))
                            && !(game.door_open() && places.door_nodes.contains(node))
                    })
                    .map(|(instance, _)| instance),
            );
        }
        // The hero's model faces −z: turned to face where the hero does, and placed where it
        // stands.
        let hero = game.hero_between(alpha);
        let facing = hero.facing();
        let place = Mat4::from_rotation_translation(
            Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(facing.x, 0.0, facing.y)),
            Vec3::new(hero.position.x, 0.0, hero.position.y),
        );
        for mut instance in self.hero.instances() {
            instance.transform = place * instance.transform;
            instances.push(instance);
        }
        let lighting = if game.night() { &self.night } else { &self.day };
        let mut scene = Scene::new(camera, &instances);
        scene.lights = &lighting.lights;
        scene.environment = Some(&lighting.environment);
        scene.exposure = lighting.exposure;
        scene.bloom = BLOOM;
        self.renderer.render(gpu, target, &scene)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::assets;
    use crate::zone::cooked::zone;

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
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_has_the_fires_light_and_the_materials_that_glow() {
        let (assets, zone) = zone();
        let scene = assets.scene(assets::zone_id()).expect("the zone is cooked");
        let lights = ZoneLights::of(&scene, zone.campfire).expect("the lights are placed");
        let campfire = lights.campfire.expect("the campfire has a light");
        assert!(matches!(lights.night[campfire], Light::Point(_)));
        // The names of the materials that the zone's meshes are drawn with.
        let mut meshes = HashSet::new();
        let names: HashSet<String> = scene
            .nodes
            .iter()
            .flat_map(|node| &node.meshes)
            .filter(|&&id| meshes.insert(id))
            .map(|&id| {
                let material = assets.mesh(id).expect("the mesh is cooked").material;
                assets
                    .material(material)
                    .expect("the material is cooked")
                    .name
            })
            .collect();
        for name in [FLAMES, EMBERS, KEY] {
            assert!(names.contains(name), "no material `{name}`");
        }
    }
}
