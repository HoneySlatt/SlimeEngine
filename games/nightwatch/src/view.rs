//! What the player sees: the courtyard at night, drawn by the engine's renderer from the game's
//! state. The GPU objects are made once, when the window opens, and kept for every frame.

use slime_gfx::{Gpu, LinearColor, RenderTarget, TextureFormat};
use slime_render::glam::{Mat4, Vec3};
use slime_render::{
    Camera, DirectionalLight, Environment, Instance, Light, Material, MaterialData, Mesh,
    RenderError, RenderStats, Renderer, Scene, Sky,
};

use crate::courtyard::BlockKind;
use crate::meshes;
use crate::rules::{Game, PLAYER_RADIUS};

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
/// The bloom's strength, as in the engine's samples.
const BLOOM: f32 = 0.04;

/// The renderer and the GPU objects of the courtyard, for targets of one format.
pub struct View {
    renderer: Renderer,
    cube: Mesh,
    sphere: Mesh,
    ground: Material,
    wall: Material,
    crate_wood: Material,
    gate: Material,
    player: Material,
    environment: Environment,
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

impl View {
    /// Creates the renderer and the courtyard's meshes, materials and night sky, for targets of
    /// `format`.
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Result<Self, RenderError> {
        let renderer = Renderer::new(gpu, format)?;
        let material =
            |label: &str, data: &MaterialData<'_>| renderer.create_material(gpu, label, data);
        Ok(Self {
            cube: renderer.create_mesh(gpu, "cube", &meshes::cube())?,
            sphere: renderer.create_mesh(gpu, "sphere", &meshes::sphere(16, 24))?,
            ground: material(
                "ground",
                &plain(LinearColor::rgb(0.32, 0.31, 0.3), 0.9, 0.0),
            )?,
            wall: material(
                "wall",
                &plain(LinearColor::rgb(0.45, 0.42, 0.38), 0.85, 0.0),
            )?,
            crate_wood: material(
                "crate",
                &plain(LinearColor::rgb(0.42, 0.26, 0.13), 0.7, 0.0),
            )?,
            gate: material(
                "gate",
                &plain(LinearColor::rgb(0.55, 0.56, 0.58), 0.45, 1.0),
            )?,
            // The slime glows faintly, so that the player always finds it in the dark.
            player: material(
                "player",
                &MaterialData {
                    emissive: LinearColor::rgb(0.25, 0.9, 0.35),
                    emissive_strength: 0.02,
                    ..plain(LinearColor::rgb(0.2, 0.75, 0.3), 0.3, 0.0)
                },
            )?,
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
        let courtyard = game.courtyard();
        // The ground reaches past the outer walls, and casts no shadow: it would only stretch the
        // depths the shadow map spans (ADR-0049).
        let ground_size = (courtyard.half_size + 4.0) * 2.0;
        let mut ground = Instance::new(
            &self.cube,
            &self.ground,
            Mat4::from_scale_rotation_translation(
                Vec3::new(ground_size.x, 0.2, ground_size.y),
                Default::default(),
                Vec3::new(0.0, -0.1, 0.0),
            ),
        );
        ground.casts_shadows = false;
        let mut instances = vec![ground];
        for block in &courtyard.blocks {
            let material = match block.kind {
                BlockKind::Wall => &self.wall,
                BlockKind::Crate => &self.crate_wood,
                BlockKind::Gate => &self.gate,
            };
            let size = Vec3::new(
                2.0 * block.half_size.x,
                block.height,
                2.0 * block.half_size.y,
            );
            let center = Vec3::new(block.center.x, block.height / 2.0, block.center.y);
            instances.push(Instance::new(
                &self.cube,
                material,
                Mat4::from_scale_rotation_translation(size, Default::default(), center),
            ));
        }
        let player = game.player();
        instances.push(Instance::new(
            &self.sphere,
            &self.player,
            Mat4::from_scale_rotation_translation(
                Vec3::splat(PLAYER_RADIUS),
                Default::default(),
                Vec3::new(player.x, PLAYER_RADIUS, player.y),
            ),
        ));
        let lights = [Light::Directional(MOON)];
        let mut scene = Scene::new(camera, &instances);
        scene.lights = &lights;
        scene.environment = Some(&self.environment);
        scene.exposure = EXPOSURE;
        scene.bloom = BLOOM;
        self.renderer.render(gpu, target, &scene)
    }
}
