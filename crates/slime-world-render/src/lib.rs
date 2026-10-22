#![doc = include_str!("../README.md")]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use slime_assets::AssetId;
use slime_gfx::Gpu;
use slime_render::glam::{DVec3, Vec3};
use slime_render::{
    AssetCache, AssetHandle, DirectionalLight, Environment, Instance, Light, MAX_LIGHTS,
    PointLight, Priority, RenderError, Scene, SceneError, Sky, SpotLight,
};
use slime_world::{Entity, Meshes, Transform, World};

mod picker;

pub use picker::{Hit, Picker};

/// Builds, at each frame, what the renderer draws of a world: the instances of the meshes its
/// entities place, the lights they place, and the sky's environment around the sun among them
/// (`docs/adr/0084-what-the-world-draws.md`). It keeps the environment, which it makes again only
/// when the sky or its sun changes, and nothing of the world.
///
/// What is drawn:
///
/// - Each entity that holds [`Meshes`] and a [`Transform`] places its meshes where the world draws
///   it at the frame's fraction of a fixed step ([`World::interpolated_transform`]), its stretch
///   included; a mesh that the [`AssetCache`] does not hold yet is left out, and asked for, at
///   [`Priority::Now`]. The builder keeps a handle of each mesh that the world names
///   ([`AssetHandle`]), and drops it once the world no longer names it, or hides what names it:
///   the cache may then drop the mesh, once its budget has no room
///   (`docs/adr/0113-asset-requests-residency-and-priorities.md`). Its handles hold nothing of the
///   GPU's, and stay valid through a lost device, as the cache's requests do
///   ([`AssetCache::into_requests`]); given a cache that did not issue them, as one made again
///   without those requests, the builder asks it for the meshes again ([`AssetCache::issued`]).
///   Each instance is picked as its entity: its [`Instance::pick`](slime_render::Instance::pick)
///   is the entity's handle ([`Entity::to_bits`], `docs/adr/0130-picking.md`).
/// - Each entity that holds a [`slime_world::Light`] and a `Transform` places its light at its
///   origin, along its −Z axis. A light that the renderer would not draw ([`Light::check`]), as one
///   placed so far that its position is not a finite number, through transforms each finite, is
///   left out, with a warning that names its entity when it is first left out.
/// - An entity that holds [`Hidden`](slime_world::Hidden), or one of whose ancestors does
///   ([`World::is_hidden`]), is not drawn, nor is its light.
/// - The sky the game gives ([`FrameBuilder::set_sky`]) is drawn around the first directional light
///   drawn that shows in the sky (`in_sky`), as its sun: that light is given once, to the sky and
///   first to the lights, so that its shadows are the ones drawn when it casts any. When several
///   show in the sky, the first in the world's order, which destroying entities can change, is the
///   sun, and a warning is logged.
/// - When the world places more lights than the renderer draws, [`MAX_LIGHTS`], the scene that
///   [`Frame::scene`] gives puts first, after the sun, the other directional lights, which reach
///   everywhere, then the point and spot lights nearest a point that the game gives, the eye of its
///   main view's camera or where its player stands: the renderer draws the first ones, the same in
///   every view, and the lamps near the player are among them.
///
/// The cameras, the exposure and the bloom are the game's: it sets the exposure and the bloom on
/// the scene that [`Frame::scene`] gives, and draws it through views of its own cameras
/// (`slime_render::ViewDraw`).
///
/// # Costs
///
/// Building a frame walks the entities that hold meshes or lights, and looks each mesh up in the
/// cache; when anything moved during the last fixed step, it also walks the ancestors of each
/// entity that did not move itself, to draw it with the nearest that did
/// ([`World::interpolated_transform`]). Making the sky's environment again costs what
/// [`Environment::new`] does, tens of milliseconds: a sun that moves at every frame
/// makes it again at every frame.
pub struct FrameBuilder {
    /// The sky the game gives, without its sun.
    sky: Option<Sky>,
    /// The environment made last, with the sky, its sun included, that it was made from.
    environment: Option<(Sky, Environment)>,
    /// Whether the warning of several suns was logged, since the world last had one at most.
    warned: bool,
    /// The entities whose lights the frame built last left out: each is said when it is first
    /// left out, not at every frame.
    left_out: HashSet<Entity>,
    /// A handle of each mesh that the world named at the last frame.
    handles: HashMap<AssetId, AssetHandle>,
}

/// What the renderer draws of a world in one frame, which [`FrameBuilder::build`] gives. It owns
/// what it draws, its instances holding their meshes and materials as counted references, so that
/// it outlives the cache and the builder, and is drawn on another thread than the one that built
/// it (ADR-0111).
pub struct Frame {
    /// The instances of the meshes that the world places and the cache holds.
    pub instances: Vec<Instance>,
    /// The lights that the world places, each with its entity, the sky's sun first, then in the
    /// world's order, which [`Frame::scene`] changes when they are more than the renderer draws.
    /// A game's presentation may change them by their entities before it draws the frame, as a
    /// fire that flickers in what is shown but not in the world.
    pub lights: Vec<(Entity, Light)>,
    /// The sky's environment, if the game gave a sky.
    pub environment: Option<Environment>,
    /// How many meshes the cache was loading once the frame was built ([`AssetCache::loading`]),
    /// for the world or for other requests: 0 when the frame draws every mesh the world names, but
    /// those that failed. A game that shows the world whole waits for 0.
    pub loading: usize,
    /// The meshes that failed since the last frame, each with why: they are not drawn, nor asked
    /// for again until the cooked data changes ([`AssetCache::poll`]). Each is logged too.
    pub errors: Vec<(AssetId, Arc<SceneError>)>,
    /// Whether `lights` starts with the sky's sun.
    sun: bool,
}

impl Frame {
    /// The scene to draw, with the frame's instances, lights and environment, which it takes; its
    /// other fields, the exposure and the bloom, are the game's to set. When the frame has more
    /// lights than the renderer draws, [`MAX_LIGHTS`], it orders them first, as [`FrameBuilder`]
    /// says: the sun, the other directional lights, then the others nearest `near` first, in the
    /// world's order for those as near. A game gives the eye of its main view's camera, or where
    /// its player stands.
    pub fn scene(self, near: DVec3) -> Scene {
        let Self {
            instances,
            mut lights,
            environment,
            sun,
            ..
        } = self;
        if lights.len() > MAX_LIGHTS {
            nearest_first(&mut lights, sun, near);
        }
        let mut scene = Scene::new(instances);
        scene.lights = lights.into_iter().map(|(_, light)| light).collect();
        scene.environment = environment;
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
            warned: false,
            left_out: HashSet::new(),
            handles: HashMap::new(),
        }
    }

    /// Draws the world around `sky` from the next frame on, as [`FrameBuilder::new`] says.
    pub fn set_sky(&mut self, sky: Option<Sky>) {
        self.sky = sky;
    }

    /// Drops the sky's environment, a GPU object, which the next frame makes again: what a game does
    /// when its device is lost, with its asset cache and its renderer.
    pub fn release(&mut self) {
        self.environment = None;
    }

    /// Builds what the renderer draws of `world` at `fraction` of the time from the last fixed step
    /// to the next, as `slime-app`'s `GameTime::alpha` gives it, from the meshes that
    /// `cache` holds. It first polls the cache ([`AssetCache::poll`]), then asks it for the meshes
    /// that the world names and it holds no handle of, at [`Priority::Now`], which later frames
    /// draw once they arrive ([`AssetCache::request`]), and drops the handles of those the world no
    /// longer names.
    ///
    /// # Errors
    ///
    /// [`RenderError`] if the sky's environment had to be made again and could not be, as
    /// [`Environment::new`] says.
    ///
    /// # Panics
    ///
    /// As [`AssetCache::poll`] and [`AssetCache::request`] do.
    pub fn build(
        &mut self,
        world: &World,
        cache: &mut AssetCache,
        gpu: &Gpu,
        fraction: f64,
    ) -> Result<Frame, RenderError> {
        let errors = cache.poll(gpu);
        // A game that makes its cache again without the requests of the last one, after a lost
        // device, would otherwise draw none of the meshes that its handles held there.
        if self
            .handles
            .values()
            .next()
            .is_some_and(|handle| !cache.issued(handle))
        {
            self.handles.clear();
        }
        for (id, error) in &errors {
            tracing::error!(%id, "a mesh of the world cannot be drawn: {error}");
        }
        // Where each entity that places meshes is drawn, and the meshes it names.
        let mut placed = Vec::new();
        let mut named = HashSet::new();
        let mut missing = Vec::new();
        for (entity, (meshes, transform)) in world.query::<(&Meshes, &Transform)>() {
            if world.is_hidden(entity) {
                continue;
            }
            let Some(at) = world.interpolated_transform(entity, fraction) else {
                continue;
            };
            for &id in &meshes.ids {
                if named.insert(id) && !self.handles.contains_key(&id) {
                    missing.push(id);
                }
            }
            placed.push((entity, at.to_affine(transform.stretch), meshes));
        }
        if !missing.is_empty() {
            let request = cache.request(gpu, missing, Priority::Now);
            for handle in request.into_handles() {
                self.handles.insert(handle.id(), handle);
            }
        }
        self.handles.retain(|id, _| named.contains(id));
        let loading = cache.loading();
        let mut instances = Vec::with_capacity(placed.len());
        for (entity, transform, meshes) in placed {
            for &id in &meshes.ids {
                if let Some((mesh, material)) = cache.mesh(id) {
                    let mut instance = Instance::new(mesh, material, transform);
                    instance.casts_shadows = meshes.casts_shadows;
                    // Picked as its entity (ADR-0130): `Entity::from_bits` gives it back.
                    instance.pick = entity.to_bits();
                    instances.push(instance);
                }
            }
        }

        let mut left_out = Vec::new();
        let mut lights = Vec::new();
        let (sun, suns) = world_lights(world, fraction, &mut lights, &mut left_out);
        for (entity, reason) in &left_out {
            if !self.left_out.contains(entity) {
                tracing::warn!(
                    ?entity,
                    "the light of the entity is left out: {reason} (said again once it is drawn \
                     and left out again)"
                );
            }
        }
        self.left_out = left_out.into_iter().map(|(entity, _)| entity).collect();
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
                // The sky reads its sun's direction, color and illuminance only: a sun whose shadows
                // change, as a game that fits them to its camera at each frame does, keeps the sky.
                let sun = sun.map(|sun| DirectionalLight {
                    casts_shadows: false,
                    shadow_distance: DirectionalLight::default().shadow_distance,
                    ..sun
                });
                let wanted = Sky { sun, ..sky };
                if self.environment.as_ref().map(|(made, _)| *made) != Some(wanted) {
                    let environment = Environment::new(gpu, &wanted)?;
                    self.environment = Some((wanted, environment));
                }
            }
        }
        Ok(Frame {
            instances,
            lights,
            environment: self
                .environment
                .as_ref()
                .map(|(_, environment)| environment.clone()),
            loading,
            errors,
            sun: sun.is_some(),
        })
    }
}

/// Orders `lights`, which start with the sky's sun if `sun`, so that the renderer draws those that
/// matter most to the player: the sun stays first, then come the other directional lights, which
/// reach everywhere, then the point and spot lights by their distance to `near`, each group in the
/// order it had for lights as near.
fn nearest_first(lights: &mut [(Entity, Light)], sun: bool, near: DVec3) {
    let rest = &mut lights[usize::from(sun)..];
    rest.sort_by(|(_, a), (_, b)| {
        let key = |light: &Light| match light {
            Light::Directional(_) => (0, 0.0),
            Light::Point(point) => (1, point.position.distance_squared(near)),
            Light::Spot(spot) => (1, spot.position.distance_squared(near)),
        };
        let ((a_group, a_distance), (b_group, b_distance)) = (key(a), key(b));
        a_group
            .cmp(&b_group)
            .then(a_distance.total_cmp(&b_distance))
    });
}
/// Puts into `lights` the lights that `world` draws at `fraction` of a step, with their entities,
/// the sky's sun first, so that its shadows are the ones drawn when it casts any: the first
/// directional light, in the world's order, that shows in the sky. Puts into `left_out` the
/// entities whose lights the renderer would not draw, placed so, and why ([`Light::check`]).
/// Returns the sun, if there is
/// one, and how many lights show in the sky.
fn world_lights(
    world: &World,
    fraction: f64,
    lights: &mut Vec<(Entity, Light)>,
    left_out: &mut Vec<(Entity, String)>,
) -> (Option<DirectionalLight>, usize) {
    lights.clear();
    let mut sun = None;
    let mut suns = 0;
    for (entity, light) in world.query::<&slime_world::Light>() {
        if world.is_hidden(entity) {
            continue;
        }
        let Some(at) = world.interpolated_transform(entity, fraction) else {
            continue;
        };
        let light = placed_light(light, at.translation, at.rotation * Vec3::NEG_Z);
        if let Err(reason) = light.check() {
            left_out.push((entity, reason));
            continue;
        }
        if let (Light::Directional(directional), true) = (light, shows_in_sky(entity, world)) {
            suns += 1;
            if sun.is_none() {
                sun = Some(directional);
                lights.insert(0, (entity, light));
                continue;
            }
        }
        lights.push((entity, light));
    }
    (sun, suns)
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

/// Which types may cross threads (`docs/ARCHITECTURE.md`, "Threads"): checked when the tests are
/// compiled.
#[cfg(test)]
mod threads {
    use super::*;

    fn send_and_sync<T: Send + Sync>() {}
    /// Owns what it holds: it borrows nothing, so that it can outlive the builder and the cache.
    fn owned<T: 'static>() {}

    #[test]
    fn a_frame_builder_and_its_frames_can_be_shared_between_threads() {
        send_and_sync::<FrameBuilder>();
        send_and_sync::<Frame>();
        send_and_sync::<Picker>();
        owned::<Frame>();
    }
}

#[cfg(test)]
mod tests {
    use slime_color::LinearColor;
    use slime_render::glam::{DVec3, Quat};

    use super::*;

    #[test]
    fn the_sun_comes_first_among_the_lights_so_that_it_casts_the_shadows() {
        let mut world = World::new();
        let directional = |illuminance: f32, in_sky: bool| slime_world::Light::Directional {
            color: LinearColor::WHITE,
            illuminance,
            casts_shadows: true,
            shadow_distance: None,
            in_sky,
        };
        for light in [
            directional(1.0, false),
            slime_world::Light::Point {
                color: LinearColor::WHITE,
                intensity: 2.0,
                range: None,
            },
            directional(3.0, true),
        ] {
            let entity = world.spawn();
            world.insert(entity, Transform::IDENTITY);
            world.insert(entity, light);
        }
        let mut lights = Vec::new();
        let (sun, suns) = world_lights(&world, 0.0, &mut lights, &mut Vec::new());
        assert_eq!(suns, 1);
        assert_eq!(sun.map(|sun| sun.illuminance), Some(3.0));
        assert_eq!(lights.len(), 3);
        assert!(matches!(lights[0].1, Light::Directional(light) if light.illuminance == 3.0));
    }

    /// A world of more lights than the renderer draws: its sun, then point lights placed farther
    /// from the camera the earlier they were spawned, and a directional light last. The renderer
    /// draws the first 16 it is given: the sun, the other directional light, then the 14 point
    /// lights nearest the camera, in the order of their distance.
    #[test]
    fn beyond_the_lights_drawn_the_nearest_to_the_camera_come_first() {
        let mut world = World::new();
        let place = |world: &mut World, at: DVec3, light: slime_world::Light| {
            let entity = world.spawn();
            world.insert(entity, Transform::from_translation(at));
            world.insert(entity, light);
        };
        let directional = |in_sky: bool| slime_world::Light::Directional {
            color: LinearColor::WHITE,
            illuminance: 1.0,
            casts_shadows: true,
            shadow_distance: None,
            in_sky,
        };
        place(&mut world, DVec3::ZERO, directional(true));
        for metres in (1..=30).rev() {
            let point = slime_world::Light::Point {
                color: LinearColor::WHITE,
                intensity: f32::from(u8::try_from(metres).expect("small")),
                range: None,
            };
            place(&mut world, DVec3::new(f64::from(metres), 0.0, 0.0), point);
        }
        place(&mut world, DVec3::ZERO, directional(false));
        let mut lights = Vec::new();
        let (sun, _) = world_lights(&world, 0.0, &mut lights, &mut Vec::new());
        assert_eq!(lights.len(), 32);
        nearest_first(&mut lights, sun.is_some(), DVec3::new(-0.5, 0.0, 0.0));
        assert!(matches!(lights[0].1, Light::Directional(light) if light.illuminance == 1.0));
        assert!(matches!(lights[1].1, Light::Directional(_)));
        let drawn: Vec<f32> = lights[2..MAX_LIGHTS]
            .iter()
            .map(|(_, light)| match light {
                Light::Point(point) => point.intensity,
                other => panic!("a point light, not {other:?}"),
            })
            .collect();
        assert_eq!(
            drawn,
            (1..=14).map(|metres| metres as f32).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_light_placed_where_no_number_reaches_is_left_out() {
        // Each transform finite and checked, the light placed by the two is not: 10³⁸ times
        // 10³⁰⁰ metres.
        let mut world = World::new();
        let far = world.spawn();
        world.insert(
            far,
            Transform {
                scale: 1e38,
                ..Transform::IDENTITY
            },
        );
        for in_sky in [false, true] {
            let light = world.spawn();
            world.insert(
                light,
                Transform::from_translation(DVec3::new(1e300, 0.0, 0.0)),
            );
            world.insert(
                light,
                slime_world::Light::Directional {
                    color: LinearColor::WHITE,
                    illuminance: 1.0,
                    casts_shadows: true,
                    shadow_distance: None,
                    in_sky,
                },
            );
            let point = world.spawn();
            world.insert(
                point,
                Transform::from_translation(DVec3::new(1e300, 0.0, 0.0)),
            );
            world.insert(
                point,
                slime_world::Light::Point {
                    color: LinearColor::WHITE,
                    intensity: 1.0,
                    range: None,
                },
            );
            world.set_parent(point, Some(far)).expect("a parent");
        }
        let mut lights = Vec::new();
        let mut left_out = Vec::new();
        let (sun, suns) = world_lights(&world, 0.0, &mut lights, &mut left_out);
        assert_eq!(lights.len(), 2, "{lights:?}");
        assert!(
            lights.iter().all(|(_, light)| light.check().is_ok()),
            "{lights:?}"
        );
        assert_eq!((sun.is_some(), suns), (true, 1));
        assert_eq!(left_out.len(), 2, "{left_out:?}");
        assert!(
            left_out
                .iter()
                .all(|(_, reason)| reason.starts_with("its position")
                    && reason.ends_with("is not finite")),
            "{left_out:?}"
        );
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
