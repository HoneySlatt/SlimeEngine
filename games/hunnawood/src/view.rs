//! What the player sees: the zone and the hero, on an overcast day or at night, drawn by the
//! engine's renderer from the cooked data and the game's state. The GPU objects are made once, when
//! the window opens, and kept for every frame and every new game. The hero loads before the first
//! frame, and the zone in the background: the sky and the hero show at once, lit by the zone's
//! lights, and the zone, whole, once it is loaded.

use slime_app::AppError;
use slime_assets::{CookedAssets, CookedScene};
use slime_gfx::{Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::{Mat4, Quat, Vec3};
use slime_render::{
    Camera, Environment, Light, LoadedScene, RenderError, RenderStats, Renderer, Scene, SceneError,
    SceneLoad, Sky, scene_lights,
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

/// The zone's lights, by the time of day they shine at, read from its cooked scene before it is
/// loaded.
#[derive(Clone)]
pub struct ZoneLights {
    /// The sun.
    day: Vec<Light>,
    /// The moon, the lanterns and the campfire.
    night: Vec<Light>,
}

impl ZoneLights {
    /// The lights of the zone's cooked scene `zone`: those whose node's `time` is `"day"`, and
    /// those whose node's `time` is `"night"`, each in the order of the nodes.
    ///
    /// # Errors
    ///
    /// When a node places a light where it cannot shine ([`scene_lights`]).
    pub fn of(zone: &CookedScene) -> Result<Self, SceneError> {
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
        Ok(Self {
            day: of("day"),
            night: of("night"),
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

/// The zone, loaded, and what is drawn of it.
struct ZoneView {
    scene: LoadedScene,
    /// The node that places each of the zone's instances, to leave out the key once taken and the
    /// door's leaves once open.
    nodes: Vec<usize>,
}

/// The renderer, the light of each time of day, and the GPU objects of the zone and the hero, for
/// targets of one format.
pub struct View {
    renderer: Renderer,
    day: Lighting,
    night: Lighting,
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
        Ok(Self {
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
    /// `alpha` of the way between its last two fixed steps ([`Game::hero_between`]): the sky, the
    /// zone's lights and the hero, and the zone once it is loaded.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        game: &Game,
        alpha: f32,
        camera: Camera,
    ) -> Result<RenderStats, RenderError> {
        let mut instances = Vec::new();
        if let Some(zone) = &self.zone {
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
