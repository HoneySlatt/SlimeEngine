//! What the player sees: the zone and the hero, on an overcast day or at night, drawn by the
//! engine's renderer from the cooked data and the game's state. The GPU objects are made once,
//! when the window opens, and kept for every frame and every new game.

use slime_app::AppError;
use slime_assets::CookedAssets;
use slime_gfx::{Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::{Mat4, Quat, Vec3};
use slime_render::{
    Camera, Environment, Light, LoadedScene, RenderError, RenderStats, Renderer, Scene, Sky,
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

/// What lights the zone at one time of day.
struct Lighting {
    /// The zone's lights of that time.
    lights: Vec<Light>,
    environment: Environment,
    exposure: f32,
}

/// The renderer and the GPU objects of the zone and the hero, for targets of one format.
pub struct View {
    renderer: Renderer,
    zone: LoadedScene,
    /// The node that places each of the zone's instances, to leave out the key once taken and the
    /// door's leaves once open.
    zone_nodes: Vec<usize>,
    hero: LoadedScene,
    day: Lighting,
    night: Lighting,
}

/// The lights of `zone` whose node's `time` is `time`, in the order of the nodes.
fn lights_of(zone: &LoadedScene, time: &str) -> Vec<Light> {
    // The scene's lights come in the order of the nodes that place them.
    zone.scene
        .nodes
        .iter()
        .filter(|node| node.light.is_some())
        .zip(&zone.lights)
        .filter(|(node, _)| text(node, "time") == Some(time))
        .map(|(_, light)| *light)
        .collect()
}

impl View {
    /// Creates the renderer, loads the zone and the hero from `assets`, and computes the light of
    /// the overcast sky and of the night sky, whose moon is the zone's directional light of the
    /// night, for targets of `format`.
    ///
    /// # Errors
    ///
    /// When the renderer cannot be created, or the zone or the hero cannot be loaded, with a
    /// message that says how to cook them.
    pub fn new(gpu: &Gpu, format: TextureFormat, assets: &CookedAssets) -> Result<Self, AppError> {
        let renderer = Renderer::new(gpu, format)?;
        let load = |what: &str, id| {
            renderer.load_scene(gpu, assets, id).map_err(|error| {
                format!(
                    "cannot load the cooked {what}: {error}; {}",
                    assets::HOW_TO_COOK
                )
            })
        };
        let zone = load("zone", assets::zone_id())?;
        let hero = load("hero", assets::hero_id())?;
        let night_lights = lights_of(&zone, "night");
        let moon = night_lights.iter().find_map(|light| match light {
            Light::Directional(moon) => Some(*moon),
            _ => None,
        });
        let night_sky = Sky {
            sun: moon,
            ..NIGHT_SKY
        };
        Ok(Self {
            day: Lighting {
                lights: lights_of(&zone, "day"),
                environment: renderer.create_environment(gpu, &OVERCAST)?,
                exposure: DAY_EXPOSURE,
            },
            night: Lighting {
                lights: night_lights,
                environment: renderer.create_environment(gpu, &night_sky)?,
                exposure: NIGHT_EXPOSURE,
            },
            zone_nodes: zone.instance_nodes(),
            zone,
            hero,
            renderer,
        })
    }

    /// Draws `game` into `target` as `camera` sees it, by day or at night as the game is, the hero
    /// `alpha` of the way between its last two fixed steps ([`Game::hero_between`]).
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        game: &Game,
        alpha: f32,
        camera: Camera,
    ) -> Result<RenderStats, RenderError> {
        let zone = game.zone();
        let mut instances: Vec<_> = self
            .zone
            .instances()
            .into_iter()
            .zip(&self.zone_nodes)
            .filter(|(_, node)| {
                !(game.has_item() && zone.item_nodes.contains(node))
                    && !(game.door_open() && zone.door_nodes.contains(node))
            })
            .map(|(instance, _)| instance)
            .collect();
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
