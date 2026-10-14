//! What the player sees: the courtyard at night, drawn by the engine's renderer from the game's
//! state. The GPU objects are made once, when the window opens, and kept for every frame and every
//! new game: what changes from frame to frame is which materials and lights the frame uses.

use slime_gfx::{Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::{Mat4, Quat, Vec2, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData, Mesh,
    PointLight, RenderError, RenderStats, Renderer, Scene, Sky, SpotLight,
};

use crate::courtyard::{BlockKind, POLE_HEIGHT, SWITCH_HEIGHT};
use crate::meshes;
use crate::rules::{
    FLASHLIGHT_HALF_ANGLE, FLASHLIGHT_RANGE, FLOODLIGHT_HALF_ANGLE, FLOODLIGHT_RANGE, Game,
    PLAYER_RADIUS, State,
};

/// The moon: a slightly blue light of 0.3 lux, about a full moon's, high in the south-west, which
/// casts the shadows the player hides in over the whole courtyard and a little beyond.
const MOON: DirectionalLight = DirectionalLight {
    direction: Vec3::new(0.35, -1.0, -0.45),
    color: LinearColor::rgb(0.75, 0.82, 1.0),
    illuminance: 0.3,
    casts_shadows: true,
    shadow_distance: 30.0,
};
/// The sky of a moonlit night, in nits, about a millionth of a clear day's, with the moon's disc.
const NIGHT: Sky = Sky {
    zenith: LinearColor::rgb(0.0012, 0.002, 0.0045),
    horizon: LinearColor::rgb(0.0025, 0.0032, 0.0048),
    ground: LinearColor::rgb(0.0008, 0.0008, 0.0009),
    sun: Some(MOON),
};
/// What the scene's light is multiplied by: a white surface facing the moon reflects about
/// 0.09 nits, which this exposure shows at about a third of white's light, as eyes used to the dark
/// would see it.
const EXPOSURE: f32 = 4.0;
/// How much brighter the courtyard shows once the game is won, reached over a second.
const WON_BRIGHTENING: f32 = 6.0;
/// The bloom's strength, as in the engine's samples.
const BLOOM: f32 = 0.04;

/// A flashlight: a warm light of 60 candelas from a guard's chest, tilted down a little. A
/// surface 4 m away receives about 4 lux, a dozen times the moon.
const FLASHLIGHT_HEIGHT: f32 = 1.3;
const FLASHLIGHT_TILT: f32 = 0.2;
const FLASHLIGHT_INTENSITY: f32 = 60.0;
/// A floodlight: 200 candelas from the top of its pole, aimed at the ground 5 m ahead, which receives
/// about 5 lux.
const FLOODLIGHT_AIM: f32 = 5.0;
const FLOODLIGHT_INTENSITY: f32 = 200.0;
const LAMP_COLOR: LinearColor = LinearColor::rgb(1.0, 0.9, 0.72);
/// The light that marks how a game ends, above the player when caught and above the gate when
/// won.
const ENDING_HEIGHT: f32 = 2.5;
const ENDING_INTENSITY: f32 = 60.0;

/// The renderer and the GPU objects of the courtyard, for targets of one format.
pub struct View {
    renderer: Renderer,
    cube: Mesh,
    sphere: Mesh,
    materials: Materials,
    environment: Environment,
}

/// Every material of the game.
struct Materials {
    ground: Material,
    wall: Material,
    crate_wood: Material,
    gate: Material,
    metal: Material,
    player: Material,
    player_caught: Material,
    guard: Material,
    guard_head: Material,
    lamp_on: Material,
    lamp_off: Material,
    switch_on: Material,
    switch_in_reach: Material,
    switch_off: Material,
}

/// A material of one `color`, without texture, of `roughness` and `metallic` from 0 to 1.
fn plain(color: LinearColor, roughness: f32, metallic: f32) -> MaterialData<'static> {
    MaterialData {
        base_color: color,
        roughness,
        metallic,
        ..MaterialData::default()
    }
}

/// A material that glows with `color` at `nits`, over a dark rough base.
fn glowing(color: LinearColor, nits: f32) -> MaterialData<'static> {
    MaterialData {
        emissive: color,
        emissive_strength: nits,
        ..plain(LinearColor::rgb(0.05, 0.05, 0.05), 0.6, 0.0)
    }
}

/// The transform of a box of `size` whose bottom stands at `ground` on the ground, turned by `yaw`
/// radians from +x towards +z.
fn standing(size: Vec3, ground: Vec2, yaw: f32) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        size,
        Quat::from_rotation_y(-yaw),
        Vec3::new(ground.x, size.y / 2.0, ground.y),
    )
}

impl View {
    /// Creates the renderer and the courtyard's meshes, materials and night sky, for targets of
    /// `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let renderer = Renderer::new(gpu, format)?;
        let material =
            |label: &str, data: &MaterialData<'_>| renderer.create_material(gpu, label, data);
        let rgb = LinearColor::rgb;
        let materials = Materials {
            ground: material("ground", &plain(rgb(0.32, 0.31, 0.3), 0.9, 0.0))?,
            wall: material("wall", &plain(rgb(0.45, 0.42, 0.38), 0.85, 0.0))?,
            crate_wood: material("crate", &plain(rgb(0.42, 0.26, 0.13), 0.7, 0.0))?,
            gate: material("gate", &plain(rgb(0.55, 0.56, 0.58), 0.45, 1.0))?,
            metal: material("metal", &plain(rgb(0.3, 0.3, 0.32), 0.5, 1.0))?,
            // The slime glows faintly, so that the player always finds it in the dark.
            player: material(
                "player",
                &MaterialData {
                    emissive: rgb(0.25, 0.9, 0.35),
                    emissive_strength: 0.02,
                    ..plain(rgb(0.2, 0.75, 0.3), 0.3, 0.0)
                },
            )?,
            player_caught: material("player caught", &glowing(rgb(1.0, 0.1, 0.08), 0.5))?,
            guard: material("guard", &plain(rgb(0.08, 0.1, 0.18), 0.8, 0.0))?,
            guard_head: material("guard head", &plain(rgb(0.6, 0.45, 0.35), 0.6, 0.0))?,
            lamp_on: material("lamp on", &glowing(LAMP_COLOR, 40.0))?,
            lamp_off: material("lamp off", &plain(rgb(0.2, 0.2, 0.2), 0.4, 0.0))?,
            switch_on: material("switch on", &glowing(rgb(1.0, 0.15, 0.1), 0.6))?,
            switch_in_reach: material("switch in reach", &glowing(rgb(1.0, 0.3, 0.15), 3.0))?,
            switch_off: material("switch off", &glowing(rgb(0.2, 0.8, 0.3), 0.08))?,
        };
        Ok(Self {
            cube: renderer.create_mesh(gpu, "cube", &meshes::cube())?,
            sphere: renderer.create_mesh(gpu, "sphere", &meshes::sphere(16, 24))?,
            materials,
            environment: renderer.create_environment(gpu, &NIGHT)?,
            renderer,
        })
    }

    /// Draws `game` into `target` as `camera` sees it.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        game: &Game,
        camera: Camera,
    ) -> Result<RenderStats, RenderError> {
        let (cube, sphere, materials) = (&self.cube, &self.sphere, &self.materials);
        let courtyard = game.courtyard();
        let state = game.state();
        // The ground reaches past the outer walls, and casts no shadow: it would only stretch the
        // depths the shadow map spans (ADR-0049).
        let ground_size = (courtyard.half_size + 4.0) * 2.0;
        let mut ground = Instance::new(
            cube,
            &materials.ground,
            Mat4::from_scale_rotation_translation(
                Vec3::new(ground_size.x, 0.2, ground_size.y),
                Quat::IDENTITY,
                Vec3::new(0.0, -0.1, 0.0),
            ),
        );
        ground.casts_shadows = false;
        let mut instances = vec![ground];
        let mut lights = vec![Light::Directional(MOON)];

        let switch_in_reach = game.switch_in_reach();
        for block in &courtyard.blocks {
            let material = match block.kind {
                BlockKind::Wall => &materials.wall,
                BlockKind::Crate => &materials.crate_wood,
                BlockKind::Gate => &materials.gate,
                BlockKind::Pole | BlockKind::Switch => &materials.metal,
            };
            let size = Vec3::new(
                2.0 * block.half_size.x,
                block.height,
                2.0 * block.half_size.y,
            );
            instances.push(Instance::new(
                cube,
                material,
                standing(size, block.center, 0.0),
            ));
        }

        // The switches: a button on top of the post, red while its floodlight is on, brighter
        // within reach, green once used.
        for (index, switch) in courtyard.switches.iter().enumerate() {
            let material = if !game.floodlight_on(switch.floodlight) {
                &materials.switch_off
            } else if switch_in_reach == Some(index) {
                &materials.switch_in_reach
            } else {
                &materials.switch_on
            };
            instances.push(Instance::new(
                cube,
                material,
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.18),
                    Quat::IDENTITY,
                    Vec3::new(switch.position.x, SWITCH_HEIGHT + 0.09, switch.position.y),
                ),
            ));
        }

        // The floodlights: a lamp on top of each pole, and its light while it is on.
        for (index, floodlight) in courtyard.floodlights.iter().enumerate() {
            let on = game.floodlight_on(index);
            let lamp = Vec3::new(floodlight.position.x, POLE_HEIGHT, floodlight.position.y);
            let aim = floodlight.direction * FLOODLIGHT_AIM;
            let direction = Vec3::new(aim.x, -POLE_HEIGHT, aim.y).normalize();
            instances.push(Instance::new(
                cube,
                if on {
                    &materials.lamp_on
                } else {
                    &materials.lamp_off
                },
                Mat4::from_scale_rotation_translation(
                    Vec3::new(0.5, 0.3, 0.5),
                    Quat::IDENTITY,
                    lamp + Vec3::Y * 0.15,
                ),
            ));
            if on {
                lights.push(Light::Spot(SpotLight {
                    position: lamp,
                    direction,
                    color: LAMP_COLOR,
                    intensity: FLOODLIGHT_INTENSITY,
                    range: Some(FLOODLIGHT_RANGE + 3.0),
                    inner_cone_angle: FLOODLIGHT_HALF_ANGLE * 0.7,
                    outer_cone_angle: FLOODLIGHT_HALF_ANGLE,
                }));
            }
        }

        // The guards: a body, a head, and the flashlight they hold.
        for guard in game.guards() {
            let facing = guard.facing();
            instances.push(Instance::new(
                cube,
                &materials.guard,
                standing(Vec3::new(0.35, 1.55, 0.5), guard.position, guard.heading),
            ));
            instances.push(Instance::new(
                sphere,
                &materials.guard_head,
                Mat4::from_scale_rotation_translation(
                    Vec3::splat(0.17),
                    Quat::IDENTITY,
                    Vec3::new(guard.position.x, 1.72, guard.position.y),
                ),
            ));
            let hand = guard.position + facing * 0.3;
            let position = Vec3::new(hand.x, FLASHLIGHT_HEIGHT, hand.y);
            instances.push(Instance::new(
                cube,
                &materials.lamp_on,
                Mat4::from_scale_rotation_translation(Vec3::splat(0.1), Quat::IDENTITY, position),
            ));
            lights.push(Light::Spot(SpotLight {
                position,
                direction: Vec3::new(facing.x, -FLASHLIGHT_TILT, facing.y),
                color: LAMP_COLOR,
                intensity: FLASHLIGHT_INTENSITY,
                range: Some(FLASHLIGHT_RANGE + 1.0),
                inner_cone_angle: FLASHLIGHT_HALF_ANGLE * 0.6,
                outer_cone_angle: FLASHLIGHT_HALF_ANGLE,
            }));
        }

        // The player, red once caught.
        let player = game.player();
        instances.push(Instance::new(
            sphere,
            if matches!(state, State::Caught { .. }) {
                &materials.player_caught
            } else {
                &materials.player
            },
            Mat4::from_scale_rotation_translation(
                Vec3::splat(PLAYER_RADIUS),
                Quat::IDENTITY,
                Vec3::new(player.x, PLAYER_RADIUS, player.y),
            ),
        ));

        // How the game ends shows in light: red above the player when caught; green above the
        // gate, and the whole courtyard brightening, when won.
        let mut exposure = EXPOSURE;
        match state {
            State::Playing => {}
            State::Caught { .. } => lights.push(Light::Point(PointLight {
                position: Vec3::new(player.x, ENDING_HEIGHT, player.y),
                color: LinearColor::rgb(1.0, 0.1, 0.05),
                intensity: ENDING_INTENSITY,
                range: Some(8.0),
            })),
            State::Won { time } => {
                let exit = courtyard.exit;
                lights.push(Light::Point(PointLight {
                    position: Vec3::new(exit.x, ENDING_HEIGHT, exit.y),
                    color: LinearColor::rgb(0.2, 1.0, 0.35),
                    intensity: ENDING_INTENSITY,
                    range: Some(8.0),
                }));
                exposure *= 1.0 + (WON_BRIGHTENING - 1.0) * time.min(1.0);
            }
        }

        let mut scene = Scene::new(camera, &instances);
        scene.lights = &lights;
        scene.environment = Some(&self.environment);
        scene.exposure = exposure;
        scene.bloom = BLOOM;
        self.renderer.render(gpu, target, &scene)
    }
}
