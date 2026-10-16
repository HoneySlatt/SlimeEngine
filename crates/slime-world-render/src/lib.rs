#![doc = include_str!("../README.md")]

use slime_gfx::Gpu;
use slime_render::glam::Vec3;
use slime_render::{
    AssetCache, Camera, DirectionalLight, Environment, Instance, Light, PointLight, RenderError,
    Renderer, Scene, SceneError, Sky, SpotLight,
};
use slime_world::{Entity, Hidden, Meshes, Transform, World};

/// Builds, at each frame, what the renderer draws of a world: the instances of the meshes its
/// entities place, the lights they place, and the sky's environment around the sun among them
/// (`docs/adr/0084-what-the-world-draws.md`). It keeps the environment, which it makes again only
/// when the sky or its sun changes, and nothing of the world.
///
/// What is drawn:
///
/// - Each entity that holds [`Meshes`] and a [`Transform`] places its meshes where the world draws
///   it at the frame's fraction of a fixed step ([`World::interpolated_transform`]), its stretch
///   included; a mesh that the [`AssetCache`] does not hold yet is left out, and asked for.
/// - Each entity that holds a [`slime_world::Light`] and a `Transform` places its light at its
///   origin, along its −Z axis.
/// - An entity that holds [`Hidden`], or one of whose ancestors does, is not drawn, nor is its
///   light.
/// - The sky the game gives ([`FrameBuilder::set_sky`]) is drawn around the first directional light
///   drawn that shows in the sky (`in_sky`), as its sun: that light is given once, to the sky and to
///   the lights. When several show in the sky, the first one does, and a warning is logged.
///
/// The camera, the exposure and the bloom are the game's, which it sets on the scene that
/// [`Frame::scene`] gives.
///
/// # Costs
///
/// Building a frame walks the entities that hold meshes or lights, and the ancestors of each, and
/// looks each mesh up in the cache. Making the sky's environment again costs what
/// [`Renderer::create_environment`] does, tens of milliseconds: a sun that moves at every frame
/// makes it again at every frame.
pub struct FrameBuilder {
    /// The sky the game gives, without its sun.
    sky: Option<Sky>,
    /// The environment made last, with the sky, its sun included, that it was made from.
    environment: Option<(Sky, Environment)>,
    /// The lights of the frame built last.
    lights: Vec<Light>,
    /// Whether the warning of several suns was logged, since the world last had one at most.
    warned: bool,
}

/// What the renderer draws of a world in one frame, which [`FrameBuilder::build`] gives.
pub struct Frame<'a> {
    /// The instances of the meshes that the world places and the cache holds.
    pub instances: Vec<Instance<'a>>,
    /// The lights that the world places.
    pub lights: &'a [Light],
    /// The sky's environment, if the game gave a sky.
    pub environment: Option<&'a Environment>,
    /// How many meshes the cache was loading once the frame was built ([`AssetCache::loading`]):
    /// 0 when the frame draws every mesh the world names, but those whose batch failed. A game
    /// that shows the world whole waits for 0.
    pub loading: usize,
    /// Why the batches of meshes that finished loading since the last frame failed, if any did:
    /// those meshes are not drawn, nor asked for again until the cooked data changes
    /// ([`AssetCache::poll`]). Each is logged too.
    pub errors: Vec<SceneError>,
}

impl Frame<'_> {
    /// The scene to draw, seen from `camera`, with the frame's instances, lights and environment;
    /// its other fields, the exposure and the bloom, are the game's to set.
    pub fn scene(&self, camera: Camera) -> Scene<'_> {
        let mut scene = Scene::new(camera, &self.instances);
        scene.lights = self.lights;
        scene.environment = self.environment;
        scene
    }
}

impl FrameBuilder {
    /// A builder that draws the world around `sky`, whose `sun` it ignores: the world gives it.
    /// `None` draws a black sky.
    pub fn new(sky: Option<Sky>) -> Self {
        Self {
            sky,
            environment: None,
            lights: Vec::new(),
            warned: false,
        }
    }

    /// Draws the world around `sky` from the next frame on, as [`FrameBuilder::new`] says.
    pub fn set_sky(&mut self, sky: Option<Sky>) {
        self.sky = sky;
    }

    /// Drops the sky's environment, a GPU object, which the next frame makes again: what a game does
    /// when its device is lost, with [`AssetCache::clear`].
    pub fn release(&mut self) {
        self.environment = None;
    }

    /// Builds what the renderer draws of `world` at `fraction` of the time from the last fixed step
    /// to the next, as `slime-app`'s `Context::fixed_step_alpha` gives it, from the meshes that
    /// `cache` holds. It first takes the batches that the cache finished loading
    /// ([`AssetCache::poll`]), then asks it for the meshes it does not hold
    /// ([`AssetCache::request`]), which later frames draw.
    ///
    /// # Errors
    ///
    /// [`RenderError`] if the sky's environment had to be made again and could not be, as
    /// [`Renderer::create_environment`] says.
    ///
    /// # Panics
    ///
    /// As [`AssetCache::poll`] and [`AssetCache::request`] do.
    pub fn build<'a>(
        &'a mut self,
        world: &World,
        cache: &'a mut AssetCache,
        renderer: &Renderer,
        gpu: &Gpu,
        fraction: f64,
    ) -> Result<Frame<'a>, RenderError> {
        let errors = cache.poll();
        for error in &errors {
            tracing::error!("meshes of the world cannot be drawn: {error}");
        }
        // Where each entity that places meshes is drawn, and the meshes the cache does not hold.
        let mut placed = Vec::new();
        let mut missing = Vec::new();
        for (entity, (meshes, transform)) in world.query::<(&Meshes, &Transform)>() {
            if hidden(world, entity) {
                continue;
            }
            let Some(at) = world.interpolated_transform(entity, fraction) else {
                continue;
            };
            missing.extend(meshes.ids.iter().filter(|&&id| cache.mesh(id).is_none()));
            placed.push((at.to_affine(transform.stretch), meshes));
        }
        cache.request(renderer, gpu, missing);
        let loading = cache.loading();
        let cache: &'a AssetCache = cache;
        let mut instances = Vec::with_capacity(placed.len());
        for (transform, meshes) in placed {
            for &id in &meshes.ids {
                if let Some((mesh, material)) = cache.mesh(id) {
                    let mut instance = Instance::new(mesh, material, transform);
                    instance.casts_shadows = meshes.casts_shadows;
                    instances.push(instance);
                }
            }
        }

        self.lights.clear();
        let mut sun = None;
        let mut suns = 0;
        for (entity, light) in world.query::<&slime_world::Light>() {
            if hidden(world, entity) {
                continue;
            }
            let Some(at) = world.interpolated_transform(entity, fraction) else {
                continue;
            };
            let light = placed_light(light, at.translation, at.rotation * Vec3::NEG_Z);
            if let (Light::Directional(directional), true) = (light, shows_in_sky(entity, world)) {
                suns += 1;
                sun.get_or_insert(directional);
            }
            self.lights.push(light);
        }
        if suns > 1 && !self.warned {
            tracing::warn!(
                suns,
                "several directional lights show in the sky: the first one drawn is the sun"
            );
        }
        self.warned = suns > 1;

        match self.sky {
            None => self.environment = None,
            Some(sky) => {
                let wanted = Sky { sun, ..sky };
                if self.environment.as_ref().map(|(made, _)| *made) != Some(wanted) {
                    let environment = renderer.create_environment(gpu, &wanted)?;
                    self.environment = Some((wanted, environment));
                }
            }
        }
        Ok(Frame {
            instances,
            lights: &self.lights,
            environment: self
                .environment
                .as_ref()
                .map(|(_, environment)| environment),
            loading,
            errors,
        })
    }
}

/// Whether `entity` or one of its ancestors holds [`Hidden`].
fn hidden(world: &World, entity: Entity) -> bool {
    let mut next = Some(entity);
    while let Some(entity) = next {
        if world.get::<Hidden>(entity).is_some() {
            return true;
        }
        next = world.parent(entity);
    }
    false
}

/// Whether the light of `entity` is a directional light that shows in the sky.
fn shows_in_sky(entity: Entity, world: &World) -> bool {
    matches!(
        world.get::<slime_world::Light>(entity),
        Some(slime_world::Light::Directional { in_sky: true, .. })
    )
}

/// The renderer's light for the world's `light`, at `position`, along `direction`.
fn placed_light(
    light: &slime_world::Light,
    position: slime_render::glam::DVec3,
    direction: Vec3,
) -> Light {
    match *light {
        slime_world::Light::Directional {
            color,
            illuminance,
            casts_shadows,
            shadow_distance,
            in_sky: _,
        } => Light::Directional(DirectionalLight {
            direction,
            color,
            illuminance,
            casts_shadows,
            shadow_distance: shadow_distance
                .unwrap_or_else(|| DirectionalLight::default().shadow_distance),
        }),
        slime_world::Light::Point {
            color,
            intensity,
            range,
        } => Light::Point(PointLight {
            position,
            color,
            intensity,
            range,
        }),
        slime_world::Light::Spot {
            color,
            intensity,
            range,
            inner_cone_angle,
            outer_cone_angle,
            casts_shadows,
        } => Light::Spot(SpotLight {
            position,
            direction,
            color,
            intensity,
            range,
            inner_cone_angle,
            outer_cone_angle,
            casts_shadows,
        }),
    }
}

#[cfg(test)]
mod tests {
    use slime_color::LinearColor;
    use slime_render::glam::{DVec3, Quat};

    use super::*;

    #[test]
    fn a_hidden_ancestor_hides_its_descendants() {
        let mut world = World::new();
        let door = world.spawn();
        let leaf = world.spawn();
        world.set_parent(leaf, Some(door)).expect("alive");
        assert!(!hidden(&world, leaf));
        world.insert(door, Hidden);
        assert!(hidden(&world, leaf) && hidden(&world, door));
    }

    #[test]
    fn a_world_light_becomes_the_renderer_s_where_its_entity_is() {
        let spot = slime_world::Light::Spot {
            color: LinearColor::WHITE,
            intensity: 8.0,
            range: Some(4.0),
            inner_cone_angle: 0.1,
            outer_cone_angle: 0.4,
            casts_shadows: true,
        };
        let at = DVec3::new(1.0e7, 2.0, 3.0);
        let down = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2) * Vec3::NEG_Z;
        let Light::Spot(light) = placed_light(&spot, at, down) else {
            panic!("a spot light");
        };
        assert_eq!(light.position, at);
        assert!(light.direction.abs_diff_eq(Vec3::NEG_Y, 1e-6));
        assert_eq!((light.intensity, light.range), (8.0, Some(4.0)));
        let sun = slime_world::Light::Directional {
            color: LinearColor::WHITE,
            illuminance: 3.0,
            casts_shadows: true,
            shadow_distance: None,
            in_sky: true,
        };
        let Light::Directional(light) = placed_light(&sun, DVec3::ZERO, Vec3::NEG_Y) else {
            panic!("a directional light");
        };
        assert_eq!(
            light.shadow_distance,
            DirectionalLight::default().shadow_distance
        );
    }
}
