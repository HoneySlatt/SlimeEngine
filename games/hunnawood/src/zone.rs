//! The place the game is played in: its ground, the colliders that stop the hero, where the hero
//! starts, and the places of the game's loop: the key, the door it opens, the campfire. Positions
//! are x and z in metres, on the ground, at y = 0, +y up; north is −z and east +x; an angle about
//! the vertical is in radians from +x towards +z.
//!
//! The zone comes from the cooked data: `games/hunnawood/zone/build_zone.py` builds the land and
//! the lantern in Blender, a scene file written by hand places them, the cooker cooks them, and
//! [`Zone::spawn`] spawns the scene file in the game's world. Its nodes carry the game's own
//! components as typed properties, as `README.md` says ("The zone"): [`Collider`], [`Marker`],
//! [`TimeOfDay`] and [`Flickers`]. The rules and the camera read the zone from the world, where its
//! entities stand.

use slime_reflect::Reflect;
use slime_render::glam::{DAffine3, Vec2, Vec3};
use slime_world::{Component, Entity, SceneLibrary, Transform, World};

/// The key that the zone's scene is spawned under: the first part of its entities' persistent
/// identities, `zone/land/door`.
pub const KEY: &str = "zone";

/// The footprint of a collider on the ground, around its center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle of half the size `half_size` along its own axes, turned by `yaw` about the
    /// vertical: its first axis lies along `Vec2::from_angle(yaw)`.
    Box { half_size: Vec2, yaw: f32 },
    /// A circle.
    Cylinder { radius: f32 },
}

/// Where a collider stops the hero: a box turned about the vertical or a cylinder, standing on the
/// ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    /// Its shape around its center.
    pub shape: Shape,
    /// The center of its footprint.
    pub center: Vec2,
    /// How high it stands, in metres.
    pub height: f32,
}

impl Footprint {
    /// The footprint of `collider`, whose entity the world places at `placed`, its stretch
    /// included: a box's half size is the length of its x and z axes, and its yaw the direction of
    /// its x axis; a cylinder's radius is the length of its x axis.
    pub fn of(collider: Collider, placed: DAffine3) -> Self {
        let axes = placed.matrix3.as_mat3();
        let shape = match collider.kind {
            ColliderKind::Box => Shape::Box {
                half_size: Vec2::new(axes.x_axis.length(), axes.z_axis.length()),
                yaw: axes.x_axis.z.atan2(axes.x_axis.x),
            },
            ColliderKind::Cylinder => Shape::Cylinder {
                radius: axes.x_axis.length(),
            },
        };
        Self {
            shape,
            center: ground(placed.translation.as_vec3()),
            height: collider.height,
        }
    }

    /// Where a circle of `radius` around `position` must move to overlap the footprint no more,
    /// if it overlaps it: away from the footprint's nearest point, which keeps the part of a move
    /// along the footprint's side, so that what moves slides along it. A center inside a box leaves
    /// through the nearest side.
    pub fn push_out(&self, position: Vec2, radius: f32) -> Option<Vec2> {
        match self.shape {
            Shape::Box { half_size, yaw } => {
                // In the box's own axes, where its sides lie along x and y.
                let local = Vec2::from_angle(-yaw).rotate(position - self.center);
                let away = local - local.clamp(-half_size, half_size);
                let distance = away.length();
                if distance >= radius {
                    return None;
                }
                let out = if distance > 1e-6 {
                    local + away * ((radius - distance) / distance)
                } else {
                    let depth = half_size - local.abs();
                    if depth.x < depth.y {
                        Vec2::new(local.x.signum() * (half_size.x + radius), local.y)
                    } else {
                        Vec2::new(local.x, local.y.signum() * (half_size.y + radius))
                    }
                };
                Some(self.center + Vec2::from_angle(yaw).rotate(out))
            }
            Shape::Cylinder { radius: own_radius } => {
                let offset = position - self.center;
                let distance = offset.length();
                let reach = own_radius + radius;
                if distance >= reach {
                    None
                } else if distance > 1e-6 {
                    Some(self.center + offset * (reach / distance))
                } else {
                    Some(self.center + Vec2::X * reach)
                }
            }
        }
    }

    /// Whether `point`, on the ground, lies within `margin` of the footprint, or inside it.
    pub fn covers(&self, point: Vec2, margin: f32) -> bool {
        match self.shape {
            Shape::Box { half_size, yaw } => {
                let local = Vec2::from_angle(-yaw).rotate(point - self.center);
                local.abs().cmple(half_size + margin).all()
            }
            Shape::Cylinder { radius } => point.distance(self.center) <= radius + margin,
        }
    }
}

/// A collider, a component of the game's, which the zone's nodes carry as a typed property: what
/// stops the hero, `height` metres high. Its entity's transform in the world places it, turned
/// about the vertical, and gives its size ([`Footprint::of`]): a box's scale along its x and z axes
/// is half its size, a cylinder's along its x axis its radius.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "hunnawood::Collider")]
pub struct Collider {
    /// A box or a cylinder.
    pub kind: ColliderKind,
    /// How high it stands, in metres.
    #[reflect(range(min = 0.0), unit = "m")]
    pub height: f32,
}

impl Component for Collider {}

/// The shape of a [`Collider`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "hunnawood::ColliderKind")]
pub enum ColliderKind {
    /// A box turned about the vertical.
    Box,
    /// A cylinder standing on the ground.
    Cylinder,
}

/// A place of the game's loop, a component of the game's, which the zone's nodes carry as a typed
/// property; each is held by one entity of the zone. Its entity's transform places it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "hunnawood::Marker")]
pub enum Marker {
    /// Where the hero starts, facing along the entity's −Z axis.
    Start,
    /// The key, the entity's descendants, which the hero takes.
    Item,
    /// The sealed door, the entity's descendants: its leaves and its collider.
    Door,
    /// Where the hero rests.
    Campfire,
    /// The terrace behind the door.
    Terrace,
    /// The ground the hero may walk: the entity's scale along its x and z axes is half its size.
    Bounds,
}

impl Component for Marker {}

/// The time of day a light shines at, a component of the game's, which the zone's lights carry as a
/// typed property: the lights of the other time are hidden. A light without one shines at any time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "hunnawood::TimeOfDay")]
pub enum TimeOfDay {
    /// By day.
    Day,
    /// At night.
    Night,
}

impl Component for TimeOfDay {}

/// A light that flickers, a component of the game's, which the campfire's light carries as a typed
/// property: its intensity, which the view makes vary around `intensity` at each frame.
#[derive(Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(name = "hunnawood::Flickers")]
pub struct Flickers {
    /// The intensity it flickers around, in candelas.
    #[reflect(range(min = 0.0), unit = "cd")]
    pub intensity: f32,
}

impl Component for Flickers {}

/// A zone spawned in a world: the entity it was spawned under, and the entity of each of its
/// markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Zone {
    /// The entity its scene was spawned under, whose persistent identity is `zone`.
    pub root: Entity,
    /// The entity of its [`Marker::Start`].
    pub start: Entity,
    /// The entity of its [`Marker::Item`], whose descendants are the key.
    pub item: Entity,
    /// The entity of its [`Marker::Door`], whose descendants are the door.
    pub door: Entity,
    /// The entity of its [`Marker::Campfire`].
    pub campfire: Entity,
    /// The entity of its [`Marker::Terrace`].
    pub terrace: Entity,
    /// The entity of its [`Marker::Bounds`].
    pub bounds: Entity,
}

/// The layout of a zone, which decides how the game plays in it: the size of its ground, its
/// colliders, where the hero starts and the places of the game's loop. A zone loaded again whose
/// new game has the same layout as the one before keeps the run ([`crate::saves::reload`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Half the size of the ground along x and z, around the origin; the hero does not leave it.
    pub half_size: Vec2,
    /// The colliders, but the door's, in the world's order.
    pub colliders: Vec<Footprint>,
    /// The colliders that close the door, which open with it.
    pub door_colliders: Vec<Footprint>,
    /// Where the hero starts.
    pub start: Vec2,
    /// Which way the hero and the camera look at the start, of length 1.
    pub facing: Vec2,
    /// Where the key lies, which opens the door.
    pub item: Vec2,
    /// The middle of the sealed door.
    pub door: Vec2,
    /// The campfire, where the hero rests.
    pub campfire: Vec2,
    /// The terrace behind the door.
    pub terrace: Vec2,
}

impl Layout {
    /// A fingerprint of the layout, as 16 hexadecimal digits: the FNV-1a hash of its numbers, each
    /// rounded to a ten-thousandth, of a metre or of a radian, in order, each list of colliders led
    /// by its length and each footprint by its shape. Rounded, the numbers that the platforms'
    /// maths gives, a box's yaw from its axes among them, are the same on every platform, and so is
    /// the hash; two layouts that differ by less are the same layout. A save says the layout it was
    /// made on so (`crate::saves`), and a zone loaded again keeps the run when the layout of its
    /// new game keeps its fingerprint ([`crate::saves::reload`]).
    pub fn fingerprint(&self) -> String {
        let mut numbers = vec![self.half_size.x, self.half_size.y];
        let mut counts = Vec::new();
        for colliders in [&self.colliders, &self.door_colliders] {
            counts.push(colliders.len());
            for footprint in colliders {
                numbers.extend(match footprint.shape {
                    Shape::Box { half_size, yaw } => [0.0, half_size.x, half_size.y, yaw],
                    Shape::Cylinder { radius } => [1.0, radius, 0.0, 0.0],
                });
                numbers.extend([footprint.center.x, footprint.center.y, footprint.height]);
            }
        }
        for place in [
            self.start,
            self.facing,
            self.item,
            self.door,
            self.campfire,
            self.terrace,
        ] {
            numbers.extend([place.x, place.y]);
        }
        // A saturating cast: −0.0 and 0.0 are one number, and NaN, which no layout holds, is 0.
        let words = counts.iter().map(|&count| count as i64).chain(
            numbers
                .iter()
                .map(|&number| (number * 10_000.0).round() as i64),
        );
        let hash = words
            .flat_map(i64::to_le_bytes)
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        format!("{hash:016x}")
    }
}

/// The point on the ground under `position`.
fn ground(position: Vec3) -> Vec2 {
    Vec2::new(position.x, position.z)
}

/// Whether `ancestor` is `entity` or one of its ancestors.
pub fn descends(world: &World, entity: Entity, ancestor: Entity) -> bool {
    let mut next = Some(entity);
    while let Some(entity) = next {
        if entity == ancestor {
            return true;
        }
        next = world.parent(entity);
    }
    false
}

/// Where the world places `entity`, its stretch included, if it has a transform.
fn placed(world: &World, entity: Entity) -> Option<DAffine3> {
    let stretch = world.get::<Transform>(entity)?.stretch;
    Some(world.global_transform(entity)?.to_affine(stretch))
}

/// The footprints of the colliders in `world`, in its order, those that `keep` keeps.
fn footprints(world: &World, mut keep: impl FnMut(Entity) -> bool) -> Vec<Footprint> {
    world
        .query::<&Collider>()
        .filter(|&(entity, _)| keep(entity))
        .filter_map(|(entity, collider)| Some(Footprint::of(*collider, placed(world, entity)?)))
        .collect()
}

/// The footprints of the colliders that stop the hero now: those of `world` that are not hidden
/// ([`World::is_hidden`]), in its order. The door's, hidden with it once it opens, stop no one.
pub fn colliders(world: &World) -> Vec<Footprint> {
    footprints(world, |entity| !world.is_hidden(entity))
}

impl Zone {
    /// Spawns the zone's scene file, as `scenes` resolves it with the world's types
    /// ([`SceneLibrary::resolve`]), in `world`, under the key `zone` ([`World::spawn_scene`]): the
    /// land and the lanterns that it places, and the game's components that their nodes carry. The
    /// game's components must be registered in `world` first, as [`crate::rules::Game::spawn`]
    /// registers them.
    ///
    /// # Errors
    ///
    /// A message that says what is wrong: a scene that cannot be resolved, a typed property among
    /// them, or a component's value that its type refuses ([`slime_world::SceneError`]); an entity
    /// alive with the key `zone` already; a marker held by no entity, or by several. Nothing is
    /// spawned then.
    pub fn spawn(world: &mut World, scenes: &mut SceneLibrary) -> Result<Self, String> {
        let scene = scenes
            .resolve(world.registry(), crate::assets::zone_id())
            .map_err(|error| error.to_string())?;
        let root = world
            .spawn_scene(KEY, &scene)
            .map_err(|error| error.to_string())?;
        let zone = Self::find(world, root, scene.name());
        if zone.is_err() {
            world.despawn(root);
        }
        zone
    }

    /// The zone spawned under `root`, whose places are the entities that hold its [`Marker`]s;
    /// `scene` names the zone in messages.
    ///
    /// # Errors
    ///
    /// When a marker is held by no entity under `root`, or by several.
    fn find(world: &World, root: Entity, scene: &str) -> Result<Self, String> {
        let markers: Vec<(Entity, Marker)> = world
            .query::<&Marker>()
            .filter(|&(entity, _)| descends(world, entity, root))
            .map(|(entity, marker)| (entity, *marker))
            .collect();
        let find = |marker: Marker| {
            let found: Vec<Entity> = markers
                .iter()
                .filter(|&&(_, m)| m == marker)
                .map(|&(entity, _)| entity)
                .collect();
            match found[..] {
                [entity] => Ok(entity),
                _ => Err(format!(
                    "scene `{scene}`: {} markers `{}`, where one is needed",
                    found.len(),
                    format!("{marker:?}").to_lowercase()
                )),
            }
        };
        Ok(Self {
            root,
            start: find(Marker::Start)?,
            item: find(Marker::Item)?,
            door: find(Marker::Door)?,
            campfire: find(Marker::Campfire)?,
            terrace: find(Marker::Terrace)?,
            bounds: find(Marker::Bounds)?,
        })
    }

    /// Where its marker `entity` stands on the ground.
    pub fn place(&self, world: &World, entity: Entity) -> Vec2 {
        world
            .global_transform(entity)
            .map_or(Vec2::ZERO, |at| ground(at.translation.as_vec3()))
    }

    /// Half the size of its ground along x and z: its bounds' scale along their x and z axes.
    pub fn half_size(&self, world: &World) -> Vec2 {
        placed(world, self.bounds).map_or(Vec2::ZERO, |at| {
            let axes = at.matrix3.as_mat3();
            Vec2::new(axes.x_axis.length(), axes.z_axis.length())
        })
    }

    /// Where the hero starts, and which way it faces there, of length 1: along its start's −Z
    /// axis, on the ground.
    pub fn start(&self, world: &World) -> (Vec2, Vec2) {
        let at = world.global_transform(self.start).unwrap_or_default();
        let facing = ground(at.rotation * Vec3::NEG_Z).normalize();
        (ground(at.translation.as_vec3()), facing)
    }

    /// Its layout in `world` now, whether its door is open or not.
    pub fn layout(&self, world: &World) -> Layout {
        let (start, facing) = self.start(world);
        Layout {
            half_size: self.half_size(world),
            colliders: footprints(world, |entity| {
                descends(world, entity, self.root) && !descends(world, entity, self.door)
            }),
            door_colliders: footprints(world, |entity| descends(world, entity, self.door)),
            start,
            facing,
            item: self.place(world, self.item),
            door: self.place(world, self.door),
            campfire: self.place(world, self.campfire),
            terrace: self.place(world, self.terrace),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turned_box_covers_its_turned_footprint() {
        // 4 m long along its first axis, turned a quarter turn: it lies along z.
        let collider = Footprint {
            shape: Shape::Box {
                half_size: Vec2::new(2.0, 0.5),
                yaw: std::f32::consts::FRAC_PI_2,
            },
            center: Vec2::ZERO,
            height: 1.0,
        };
        assert!(collider.covers(Vec2::new(0.0, 1.9), 0.0));
        assert!(!collider.covers(Vec2::new(1.9, 0.0), 0.0));
        assert!(collider.covers(Vec2::new(0.7, 0.0), 0.25));
        assert!(!collider.covers(Vec2::new(0.8, 0.0), 0.25));
    }
}

/// The tests that read the cooked zone, which they need fetched and cooked
/// (`scripts/check.sh` does both): `cargo test` leaves them out.
#[cfg(test)]
pub mod cooked {
    use std::collections::{HashMap, HashSet, VecDeque};

    use slime_assets::{AssetId, AssetKind, CookedAssets, CookedScene};
    use slime_render::glam::Vec2;
    use slime_world::{Hidden, Light, Meshes, PersistentId, SceneLibrary, World};

    use super::{Footprint, Shape, TimeOfDay, Zone, descends};
    use crate::assets;
    use crate::rules::{Game, HERO_RADIUS};

    /// The cooked data, from `target/cooked/hunnawood`, and a library of its scenes: the zone's,
    /// those it places and the hero's.
    pub fn zone() -> (CookedAssets, SceneLibrary) {
        let assets = CookedAssets::open(assets::cooked_in_target())
            .unwrap_or_else(|error| panic!("{error}; {}", assets::HOW_TO_COOK));
        let scenes = SceneLibrary::of(assets.clone());
        (assets, scenes)
    }

    /// A game in the cooked zone, in a world of its own.
    pub fn game() -> (World, Game) {
        let (_, mut scenes) = zone();
        let mut world = World::new();
        let game = Game::spawn(&mut world, &mut scenes, false).expect("the zone is whole");
        (world, game)
    }

    /// The cooked glTF scene `id`.
    fn scene(assets: &CookedAssets, id: AssetId) -> CookedScene {
        assets.scene(id).expect("the scene is cooked")
    }

    /// The entity of the zone `world` names `id`, from the zone's root: `lantern 3/light`.
    fn find(world: &World, id: &str) -> Option<slime_world::Entity> {
        world.find(
            &format!("zone/{id}")
                .parse::<PersistentId>()
                .expect("an identity"),
        )
    }

    /// The cells of a grid of `CELL` metres over the ground that the hero reaches from `start`,
    /// walking between `colliders` within `half_size`.
    struct Reach {
        origin: Vec2,
        columns: usize,
        reached: Vec<bool>,
    }

    const CELL: f32 = 0.25;

    impl Reach {
        fn new(half_size: Vec2, start: Vec2, colliders: &[Footprint]) -> Self {
            let origin = -half_size;
            let columns = (2.0 * half_size.x / CELL) as usize;
            let rows = (2.0 * half_size.y / CELL) as usize;
            let center =
                |c: usize, r: usize| origin + Vec2::new(c as f32 + 0.5, r as f32 + 0.5) * CELL;
            let free = |p: Vec2| {
                p.abs().cmplt(half_size - HERO_RADIUS).all()
                    && !colliders.iter().any(|c| c.covers(p, HERO_RADIUS))
            };
            let mut reached = vec![false; columns * rows];
            let start = ((start - origin) / CELL).as_uvec2();
            let (sc, sr) = (start.x as usize, start.y as usize);
            assert!(free(center(sc, sr)), "the start is not free");
            let mut queue = VecDeque::from([(sc, sr)]);
            reached[sr * columns + sc] = true;
            while let Some((c, r)) = queue.pop_front() {
                let neighbours = [
                    (c.wrapping_sub(1), r),
                    (c + 1, r),
                    (c, r.wrapping_sub(1)),
                    (c, r + 1),
                ];
                for (nc, nr) in neighbours {
                    if nc >= columns || nr >= rows || reached[nr * columns + nc] {
                        continue;
                    }
                    if free(center(nc, nr)) {
                        reached[nr * columns + nc] = true;
                        queue.push_back((nc, nr));
                    }
                }
            }
            Self {
                origin,
                columns,
                reached,
            }
        }

        /// Whether the hero comes within `distance` of `point`.
        fn near(&self, point: Vec2, distance: f32) -> bool {
            self.reached.iter().enumerate().any(|(i, &reached)| {
                let cell = self.origin
                    + Vec2::new(
                        (i % self.columns) as f32 + 0.5,
                        (i / self.columns) as f32 + 0.5,
                    ) * CELL;
                reached && cell.distance(point) <= distance
            })
        }
    }

    /// Whether an entity below `ancestor` places meshes.
    fn draws_below(world: &World, ancestor: slime_world::Entity) -> bool {
        world
            .query::<&Meshes>()
            .any(|(entity, _)| entity != ancestor && descends(world, entity, ancestor))
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_has_its_places_and_its_start_is_clear() {
        let (world, game) = game();
        let zone: &Zone = game.zone();
        let layout = zone.layout(&world);
        assert!(layout.start.abs().cmplt(layout.half_size - 1.0).all());
        for collider in layout.colliders.iter().chain(&layout.door_colliders) {
            assert!(!collider.covers(layout.start, HERO_RADIUS), "{collider:?}");
        }
        assert!((layout.facing.length() - 1.0).abs() < 1e-4);
        assert!(!layout.door_colliders.is_empty(), "the door has a collider");
        assert!(draws_below(&world, zone.item), "the key has a mesh");
        assert!(draws_below(&world, zone.door), "the door has leaves");
        for place in [layout.item, layout.door, layout.campfire, layout.terrace] {
            assert!(place.abs().cmplt(layout.half_size).all(), "{place}");
        }
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_hero_reaches_the_item_and_the_campfire_and_the_door_keeps_the_terrace() {
        let (mut world, game) = game();
        let layout = game.zone().layout(&world);
        let (half_size, start) = (layout.half_size, layout.start);
        let closed = Reach::new(half_size, start, &super::colliders(&world));
        assert!(closed.near(layout.item, 1.2), "the item is out of reach");
        assert!(
            closed.near(layout.campfire, 2.0),
            "the campfire is out of reach"
        );
        assert!(closed.near(layout.door, 1.5), "the door is out of reach");
        assert!(
            !closed.near(layout.terrace, 2.0),
            "the terrace is reached without the door"
        );
        // Once the door opens, hidden, the terrace is reached through it.
        world.insert(game.zone().door, Hidden);
        assert!(
            Reach::new(half_size, start, &super::colliders(&world)).near(layout.terrace, 1.0),
            "the open door leads nowhere"
        );
    }

    /// The zone's budgets: what the renderer draws in every frame (it culls nothing, so every frame
    /// draws all of it), the lights that cast shadows at night, the GPU memory of its textures and
    /// the size of its cooked data, counted over the whole zone, the land and every lantern. They
    /// were set at Stage 4a, before the zone was made, from a synthetic forest measured on an RX
    /// 6900 XT and on lavapipe.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_keeps_within_its_budgets() {
        let (assets, _) = zone();
        let (world, game) = game();
        let mut triangles = 0;
        let mut instances = 0;
        let mut meshes = HashMap::new();
        for (entity, placed) in world.query::<&Meshes>() {
            if !descends(&world, entity, game.zone().root) {
                continue;
            }
            for &id in &placed.ids {
                let count = *meshes.entry(id).or_insert_with(|| {
                    assets.mesh(id).expect("the mesh is cooked").indices.len() / 3
                });
                triangles += count;
                instances += 1;
            }
        }
        let night_spots = world
            .query::<(&Light, &TimeOfDay)>()
            .filter(|(_, (light, time))| {
                matches!(light, Light::Spot { .. }) && **time == TimeOfDay::Night
            })
            .count();
        // Every asset the zone needs, through the scenes it places, their meshes and materials,
        // each once.
        let manifest = assets.manifest();
        let mut seen = HashSet::new();
        let mut stack = vec![assets::zone_id()];
        let (mut size, mut texture_bytes) = (0u64, 0u64);
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let entry = manifest.get(id).expect("listed");
            size += entry.size;
            if entry.kind == AssetKind::Texture {
                let texture = assets.texture(id).expect("the texture is cooked");
                texture_bytes += texture.levels.iter().map(|l| l.len() as u64).sum::<u64>();
            }
            stack.extend(entry.needs.iter().copied());
        }
        let mib = |bytes: u64| bytes as f64 / (1 << 20) as f64;
        eprintln!(
            "zone: {triangles} triangles, {instances} instances, {night_spots} spot lights at \
             night, textures {:.0} MiB, cooked {:.0} MiB",
            mib(texture_bytes),
            mib(size)
        );
        assert!(triangles <= 1_000_000, "{triangles} triangles");
        assert!(instances <= 800, "{instances} instances");
        assert!(night_spots <= 6, "{night_spots} spot lights");
        assert!(
            mib(texture_bytes) <= 384.0,
            "{:.0} MiB of textures",
            mib(texture_bytes)
        );
        assert!(mib(size) <= 320.0, "{:.0} MiB of cooked data", mib(size));
    }

    /// The zone spawned in a world: the land placed where its file puts each node, under its name,
    /// six lanterns, and the two entities that the scene file removes from the fifth gone.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_spawns_the_land_as_its_file_places_it_and_six_lanterns() {
        use slime_render::glam::{DMat4, Mat4};
        use slime_world::Transform;

        let (assets, _) = zone();
        let (land, lantern) = (
            scene(&assets, assets::land_id()),
            scene(&assets, assets::lantern_id()),
        );
        let (mut world, game) = game();
        world.update_transforms();
        let in_zone = world
            .entities()
            .filter(|&entity| descends(&world, entity, game.zone().root))
            .count();
        assert_eq!(
            in_zone,
            1 + (1 + land.nodes.len()) + 6 * (1 + lantern.nodes.len()) - 2,
            "the zone's root, the land and six lanterns, but the fifth's light and glass"
        );
        for (node, expected) in land.nodes.iter().zip(land.world_transforms()) {
            let entity = find(&world, &format!("land/{}", node.name))
                .unwrap_or_else(|| panic!("node `{}` is known by its name", node.name));
            let stretch = world.get::<Transform>(entity).expect("placed").stretch;
            let placed = world
                .global_transform(entity)
                .expect("placed")
                .to_affine(stretch);
            let cells = DMat4::from(placed).to_cols_array();
            let expected = Mat4::from_cols_array(&expected).to_cols_array();
            for (a, b) in cells.iter().zip(expected) {
                assert!(
                    (a - f64::from(b)).abs() <= 1e-5 * f64::from(b.abs()).max(1.0),
                    "node `{}`: {placed:?}",
                    node.name
                );
            }
        }
    }

    /// Six lanterns of one prefab, the lantern's scene: the third lights less than the others, and
    /// the fifth has neither its light nor its glass.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn six_lanterns_from_one_prefab_the_third_dimmer_and_the_fifth_broken() {
        let (world, _) = game();
        let intensity = |world: &World, lantern: u32| {
            let light = find(world, &format!("lantern {lantern}/light"))?;
            match world.get::<Light>(light)? {
                Light::Spot { intensity, .. } => Some(*intensity),
                _ => None,
            }
        };
        for lantern in 1..=6 {
            let root = find(&world, &format!("lantern {lantern}")).expect("placed");
            let placement = world.placement(root).expect("a placement");
            assert_eq!(placement.prefab().id(), assets::lantern_id());
            assert!(find(&world, &format!("lantern {lantern}/post collider")).is_some());
        }
        let full = intensity(&world, 1).expect("the first lantern's light");
        for lantern in [2, 4, 6] {
            assert_eq!(intensity(&world, lantern), Some(full));
        }
        assert!(intensity(&world, 3).expect("the third's light") < full);
        assert_eq!(intensity(&world, 5), None);
        assert!(find(&world, "lantern 5/glass").is_none());
        assert!(find(&world, "lantern 4/glass").is_some());
    }

    /// The lantern changed, as Blender would export it again: every lantern follows it, but for
    /// what the zone's scene file overrides.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn a_lantern_changed_reaches_every_lantern_but_what_the_zone_overrides() {
        let (assets, mut scenes) = zone();
        let mut lantern = scene(&assets, assets::lantern_id());
        let light = lantern
            .nodes
            .iter_mut()
            .find(|node| node.name == "light")
            .expect("the lantern's light");
        let light = light.light.as_mut().expect("a light");
        light.color = [0.4, 0.6, 1.0];
        light.intensity = 300.0;
        let mut flag = lantern.nodes[0].clone();
        flag.name = "flag".to_owned();
        flag.parent = None;
        lantern.nodes.push(flag);
        scenes.insert_cooked(assets::lantern_id(), lantern);
        let mut world = World::new();
        Game::spawn(&mut world, &mut scenes, false).expect("the zone is whole");
        let light = |lantern: u32| {
            let light = find(&world, &format!("lantern {lantern}/light"))?;
            match world.get::<Light>(light)? {
                Light::Spot {
                    color, intensity, ..
                } => Some((color.r, *intensity)),
                _ => None,
            }
        };
        for lantern in [1, 2, 4, 6] {
            assert_eq!(light(lantern), Some((0.4, 300.0)), "lantern {lantern}");
        }
        let (red, intensity) = light(3).expect("the third's light");
        assert_eq!(red, 0.4, "its color follows the prefab");
        assert!(intensity < 300.0, "its intensity stays the zone's");
        assert_eq!(light(5), None, "the fifth stays without its light");
        for lantern in 1..=6 {
            assert!(find(&world, &format!("lantern {lantern}/flag")).is_some());
        }
    }

    /// The colliders of 0.3, before the zone was made of prefabs (`fixtures/colliders-0.3.txt`):
    /// each is where it was, within 10⁻⁴, whatever their order; a scattered rock's at its center,
    /// its size the rock's that the zone's script now places there.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_colliders_are_where_they_were_in_0_3() {
        let (world, game) = game();
        let layout = game.zone().layout(&world);
        let mut kept: Vec<(&str, Footprint, bool)> = include_str!("../fixtures/colliders-0.3.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| {
                let (numbers, name) = line.split_once('\t').expect("a name after a tab");
                // `rock 12`, but not `rock face 3`.
                let rock = name
                    .strip_prefix("rock ")
                    .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()));
                let words: Vec<&str> = numbers.split(' ').collect();
                let n = |index: usize| words[index].parse::<f32>().expect("a number");
                let center = Vec2::new(n(2), n(3));
                let (shape, height) = match words[1] {
                    "box" => (
                        Shape::Box {
                            half_size: Vec2::new(n(4), n(5)),
                            yaw: n(6),
                        },
                        n(7),
                    ),
                    _ => (Shape::Cylinder { radius: n(4) }, n(5)),
                };
                let footprint = Footprint {
                    shape,
                    center,
                    height,
                };
                (words[0], footprint, rock)
            })
            .collect();
        let near = |a: f32, b: f32| (a - b).abs() <= 1e-4;
        let same = |a: &Footprint, b: &Footprint| {
            let shape = match (a.shape, b.shape) {
                (
                    Shape::Box { half_size, yaw },
                    Shape::Box {
                        half_size: h,
                        yaw: y,
                    },
                ) => {
                    // A box turned half a turn is the same box.
                    let turn = (yaw - y).rem_euclid(std::f32::consts::PI);
                    half_size.abs_diff_eq(h, 1e-4)
                        && (turn <= 1e-4 || std::f32::consts::PI - turn <= 1e-4)
                }
                (Shape::Cylinder { radius }, Shape::Cylinder { radius: r }) => near(radius, r),
                _ => false,
            };
            shape && a.center.abs_diff_eq(b.center, 1e-4) && near(a.height, b.height)
        };
        let now = layout
            .colliders
            .iter()
            .map(|c| ("collider", c))
            .chain(layout.door_colliders.iter().map(|c| ("door", c)));
        for (list, collider) in now {
            let index = kept
                .iter()
                .position(|(was, footprint, rock)| {
                    *was == list
                        && if *rock {
                            footprint.center.abs_diff_eq(collider.center, 1e-4)
                        } else {
                            same(footprint, collider)
                        }
                })
                .unwrap_or_else(|| panic!("{list} {collider:?} was not in 0.3"));
            kept.swap_remove(index);
        }
        assert!(kept.is_empty(), "0.3's colliders gone: {kept:?}");
    }

    /// Every entity of the zone is known by the names that the zone's script gives its nodes, none
    /// of which Blender numbered, `oak 12.001`, as it numbers a name that another object holds.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn no_node_of_the_zone_is_named_with_blenders_numbers() {
        let (world, _) = game();
        let numbered = |part: &str| {
            let digits = part.rsplit_once('.').map(|(_, end)| end);
            digits.is_some_and(|d| d.len() == 3 && d.bytes().all(|b| b.is_ascii_digit()))
        };
        for entity in world.entities() {
            if let Some(id) = world.persistent_id(entity) {
                assert!(!id.parts().iter().any(|part| numbered(part)), "{id}");
            }
        }
    }

    /// The moon shows in the night's sky, as the zone's scene file says, and the sun stays behind
    /// the day's clouds.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_moon_shows_in_the_sky_and_the_sun_does_not() {
        let (world, _) = game();
        let in_sky = |name: &str| {
            let light = find(&world, &format!("land/{name}")).expect("a light");
            matches!(
                world.get::<Light>(light),
                Some(Light::Directional { in_sky: true, .. })
            )
        };
        assert!(in_sky("moon") && !in_sky("sun"));
    }

    /// Prints what reading the zone's scenes, resolving them and spawning the zone in a world cost,
    /// the median of 21 runs, a library made anew for each: what a game pays when it starts. Run it
    /// in a release build:
    /// `cargo test --release -p hunnawood -- --ignored --nocapture spawning_the_zone`.
    #[test]
    #[ignore = "a measurement, which needs the cooked zone"]
    fn what_spawning_the_zone_costs() {
        use std::time::{Duration, Instant};

        let (assets, _) = zone();
        let mut resolving: Vec<Duration> = Vec::new();
        let mut spawning: Vec<Duration> = Vec::new();
        let mut entities = 0;
        for _ in 0..21 {
            let mut scenes = SceneLibrary::of(assets.clone());
            let mut world = World::new();
            crate::rules::register(&mut world);
            let started = Instant::now();
            let resolved = scenes
                .resolve(world.registry(), assets::zone_id())
                .expect("resolved");
            resolving.push(started.elapsed());
            let started = Instant::now();
            world.spawn_scene("zone", &resolved).expect("spawned");
            spawning.push(started.elapsed());
            entities = world.len();
        }
        resolving.sort_unstable();
        spawning.sort_unstable();
        eprintln!(
            "the zone's {entities} entities: read and resolved in {:?}, spawned in {:?}",
            resolving[10], spawning[10]
        );
    }

    /// Prints what building a frame of the zone's world costs on the CPU, once the asset cache
    /// holds its meshes: the instances, the lights and the sky's environment that
    /// `slime-world-render` gives the renderer. Run it in a release build, on a GPU:
    /// `cargo test --release -p hunnawood -- --ignored --nocapture frame_of_the_zone`.
    #[test]
    #[ignore = "a measurement, which needs the cooked zone and a GPU"]
    fn what_building_a_frame_of_the_zone_s_world_costs() {
        use std::time::{Duration, Instant};

        use slime_gfx::{Gpu, TextureFormat};
        use slime_render::{AssetCache, Renderer};
        use slime_world_render::FrameBuilder;

        let (assets, mut scenes) = zone();
        // The zone alone, as 0.3 measured it: every light shines, whatever its time of day.
        let mut world = World::new();
        crate::rules::register(&mut world);
        let zone = scenes
            .resolve(world.registry(), assets::zone_id())
            .expect("resolved");
        world.spawn_scene("zone", &zone).expect("spawned");
        world.update_transforms();
        let gpu = Gpu::new().expect("a GPU adapter must be available");
        let renderer = Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer");
        let mut cache = AssetCache::new(assets);
        // Without a sky: its environment is made once, and is not what a frame costs.
        let mut builder = FrameBuilder::new(None);
        let started = Instant::now();
        loop {
            let frame = builder
                .build(&world, &mut cache, &renderer, &gpu, 0.0)
                .expect("the frame is built");
            assert!(frame.errors.is_empty(), "{:?}", frame.errors);
            if frame.loading == 0 {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "the zone still loads"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        eprintln!("the zone's meshes loaded in {:?}", started.elapsed());
        let mut times: Vec<Duration> = (0..200)
            .map(|_| {
                let started = Instant::now();
                let frame = builder
                    .build(&world, &mut cache, &renderer, &gpu, 0.5)
                    .expect("the frame is built");
                std::hint::black_box((frame.instances.len(), frame.lights.len()));
                started.elapsed()
            })
            .collect();
        times.sort_unstable();
        let frame = builder
            .build(&world, &mut cache, &renderer, &gpu, 0.5)
            .expect("the frame is built");
        eprintln!(
            "a frame of the zone's world, {} instances and {} lights: {:?} at best, {:?} at the median",
            frame.instances.len(),
            frame.lights.len(),
            times[0],
            times[times.len() / 2]
        );
    }
}
