//! The place the game is played in: its ground, the colliders that stop the hero, where the hero
//! starts, and the places of the game's loop: the key, the door it opens, the campfire. Positions
//! are x and z in metres, on the ground, at y = 0, +y up; north is −z and east +x; an angle about
//! the vertical is in radians from +x towards +z.
//!
//! The zone comes from the cooked data: `games/hunnawood/zone/build_zone.py` builds it in Blender,
//! the cooker cooks it, and [`Zone::spawn`] spawns its cooked scene in the game's world, then gives
//! its entities the game's own components, which the custom values (`extras`) of its nodes
//! describe, as `README.md` says ("The zone"): [`Collider`], [`Marker`], [`TimeOfDay`] and
//! [`Flickers`]. The rules and the camera read the zone from the world, where its entities stand.

use slime_assets::{CookedScene, Extra};
use slime_reflect::Reflect;
use slime_render::glam::{DAffine3, Vec2, Vec3};
use slime_world::{Component, Entity, Extras, Light, Name, ResolvedScene, Transform, World};

/// The key that the zone's scene is spawned under: the first part of its entities' persistent
/// identities, `zone/door`.
pub const KEY: &str = "zone";
/// How far from the campfire, on the ground, its light stands, at most, in metres: the zone places
/// it above the fire pit.
const CAMPFIRE_LIGHT_REACH: f32 = 1.0;

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

/// A collider, a component of the game's: what stops the hero, `height` metres high, as its
/// node's `collider` and `height` say. Its entity's transform in the world places it, turned about
/// the vertical, and gives its size ([`Footprint::of`]): a box's scale along its x and z axes is
/// half its size, a cylinder's along its x axis its radius.
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

/// A place of the game's loop, a component of the game's, as its node's `marker` says. Its
/// entity's transform places it.
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

/// The time of day a light shines at, a component of the game's, as its node's `time` says: the
/// lights of the other time are hidden. A light without one shines at any time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(name = "hunnawood::TimeOfDay")]
pub enum TimeOfDay {
    /// By day.
    Day,
    /// At night.
    Night,
}

impl Component for TimeOfDay {}

/// A light that flickers, a component of the game's: the campfire's, whose intensity the view
/// makes vary around `intensity`, the one the zone gives it, at each frame.
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
/// colliders, where the hero starts and the places of the game's loop. A zone loaded again with the
/// same layout keeps the run (`crate::rules::Game::reload_zone`).
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
    /// made on so (`crate::saves`), and a zone loaded again keeps the run when its layout keeps its
    /// fingerprint ([`crate::rules::Game::reload_zone`]).
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

/// The text of the custom value `name` among `extras`, if it has one that is a text.
fn text<'a>(extras: &'a Extras, name: &str) -> Option<&'a str> {
    match extras.get(name)? {
        Extra::Text(text) => Some(text),
        _ => None,
    }
}

/// The number of the custom value `name` among `extras`, if it has one that is a number.
fn number(extras: &Extras, name: &str) -> Option<f32> {
    match extras.get(name)? {
        Extra::Number(number) => Some(*number as f32),
        _ => None,
    }
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
    /// Spawns the zone's cooked `scene` in `world`, under the key `zone`
    /// ([`World::spawn_scene`]), and gives its entities the game's components that their extras
    /// describe:
    ///
    /// - a [`Collider`] to each whose `collider` is `"box"` or `"cylinder"`, with its `height`;
    /// - a [`Marker`] to each whose `marker` is `start`, `item`, `door`, `campfire`, `terrace` or
    ///   `bounds`, each given to one entity; another marker is ignored;
    /// - a [`TimeOfDay`] to each whose `time` is `"day"` or `"night"`; the night's directional
    ///   light, the moon, shows in the sky (`in_sky`), while the day's sun stays behind the
    ///   overcast's clouds;
    /// - [`Flickers`] to the night's first point light within [`CAMPFIRE_LIGHT_REACH`] of the
    ///   campfire on the ground, if one is.
    ///
    /// Every light shines, whatever its time of day: the game hides those of the other time.
    ///
    /// # Errors
    ///
    /// A message that says what is wrong: an entity alive with the key `zone` already, a marker
    /// missing or given twice, a collider of another kind or without a height, a time of another
    /// kind. Nothing is spawned then.
    pub fn spawn(world: &mut World, scene: &CookedScene) -> Result<Self, String> {
        let root = world
            .spawn_scene(
                KEY,
                &ResolvedScene::from_cooked(crate::assets::zone_id(), scene),
            )
            .map_err(|error| error.to_string())?;
        let zone = Self::prepare(world, root, &scene.name);
        if zone.is_err() {
            world.despawn(root);
        }
        zone
    }

    /// Gives the entities of the zone spawned under `root` the game's components, as
    /// [`Zone::spawn`] says; `scene` names the zone in messages.
    fn prepare(world: &mut World, root: Entity, scene: &str) -> Result<Self, String> {
        let mut colliders = Vec::new();
        let mut markers = Vec::new();
        let mut times = Vec::new();
        for (entity, extras) in world.query::<&Extras>() {
            if !descends(world, entity, root) {
                continue;
            }
            let name = || world.get::<Name>(entity).map_or("", |name| name.0.as_str());
            if let Some(kind) = text(extras, "collider") {
                let kind = match kind {
                    "box" => ColliderKind::Box,
                    "cylinder" => ColliderKind::Cylinder,
                    other => return Err(format!("collider `{}` is a `{other}`", name())),
                };
                let height = number(extras, "height")
                    .ok_or_else(|| format!("collider `{}` has no height", name()))?;
                colliders.push((entity, Collider { kind, height }));
            }
            let marker = match text(extras, "marker") {
                Some("start") => Some(Marker::Start),
                Some("item") => Some(Marker::Item),
                Some("door") => Some(Marker::Door),
                Some("campfire") => Some(Marker::Campfire),
                Some("terrace") => Some(Marker::Terrace),
                Some("bounds") => Some(Marker::Bounds),
                _ => None,
            };
            if let Some(marker) = marker {
                markers.push((entity, marker));
            }
            match text(extras, "time") {
                Some("day") => times.push((entity, TimeOfDay::Day)),
                Some("night") => times.push((entity, TimeOfDay::Night)),
                Some(other) => return Err(format!("light `{}` shines at `{other}`", name())),
                None => {}
            }
        }
        for (entity, collider) in colliders {
            world.insert(entity, collider);
        }
        for (entity, marker) in markers {
            world.insert(entity, marker);
        }
        for (entity, time) in times {
            world.insert(entity, time);
        }
        let zone = Self::find(world, root, scene)?;
        let campfire = zone.place(world, zone.campfire);
        world.query_mut::<(&TimeOfDay, &mut Light)>(|_, (time, light)| {
            if let (TimeOfDay::Night, Light::Directional { in_sky, .. }) = (time, light) {
                *in_sky = true;
            }
        });
        // The zone's script places the fire's light above the fire pit, among the lanterns' lights
        // of the night, which stand farther: the first near the campfire is the fire's.
        let fire = world
            .query::<(&TimeOfDay, &Light)>()
            .filter(|&(entity, (time, light))| {
                *time == TimeOfDay::Night
                    && matches!(light, Light::Point { .. })
                    && descends(world, entity, root)
                    && placed(world, entity).is_some_and(|at| {
                        ground(at.translation.as_vec3()).distance(campfire) <= CAMPFIRE_LIGHT_REACH
                    })
            })
            .find_map(|(entity, (_, light))| match light {
                Light::Point { intensity, .. } => Some((entity, *intensity)),
                _ => None,
            });
        if let Some((entity, intensity)) = fire {
            world.insert(entity, Flickers { intensity });
        }
        Ok(zone)
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
    use std::collections::{HashSet, VecDeque};

    use slime_assets::{AssetKind, CookedAssets, CookedScene};
    use slime_render::glam::Vec2;
    use slime_world::{Hidden, Light, Meshes, ResolvedScene, World};

    use super::{Footprint, TimeOfDay, Zone, descends};
    use crate::assets;
    use crate::rules::tests::no_hero;
    use crate::rules::{Game, HERO_RADIUS};

    /// The cooked data, and the zone's scene, from `target/cooked/hunnawood`.
    pub fn zone() -> (CookedAssets, CookedScene) {
        let assets = CookedAssets::open(assets::cooked_in_target())
            .unwrap_or_else(|error| panic!("{error}; {}", assets::HOW_TO_COOK));
        let scene = assets.scene(assets::zone_id()).expect("the zone is cooked");
        (assets, scene)
    }

    /// A game in the cooked zone, in a world of its own, with a hero that draws nothing.
    pub fn game() -> (World, Game) {
        let (_, scene) = zone();
        let mut world = World::new();
        let game = Game::spawn(&mut world, &scene, &no_hero(), false).expect("the zone is whole");
        (world, game)
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
    /// the size of its cooked data. They were set at Stage 4a, before the zone was made, from a
    /// synthetic forest measured on an RX 6900 XT and on lavapipe.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_keeps_within_its_budgets() {
        let (assets, scene) = zone();
        let mut triangles = 0;
        let mut instances = 0;
        let mut meshes = std::collections::HashMap::new();
        for node in &scene.nodes {
            for &id in &node.meshes {
                let count = *meshes.entry(id).or_insert_with(|| {
                    assets.mesh(id).expect("the mesh is cooked").indices.len() / 3
                });
                triangles += count;
                instances += 1;
            }
        }
        let (world, _) = game();
        let night_spots = world
            .query::<(&Light, &TimeOfDay)>()
            .filter(|(_, (light, time))| {
                matches!(light, Light::Spot { .. }) && **time == TimeOfDay::Night
            })
            .count();
        // Every asset the scene needs, through its meshes and materials, each once.
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

    /// The zone spawned in a world, each node an entity placed where the file puts it, under its
    /// name.
    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_cooked_zone_spawns_in_a_world_as_its_file_places_it() {
        use slime_render::glam::{DMat4, Mat4};
        use slime_world::{PersistentId, Transform};

        let (_, scene) = zone();
        let mut world = World::new();
        world
            .spawn_scene(
                "zone",
                &ResolvedScene::from_cooked(crate::assets::zone_id(), &scene),
            )
            .expect("spawned");
        world.update_transforms();
        assert_eq!(
            world.len(),
            scene.nodes.len() + 1,
            "a root and an entity per node"
        );
        for (node, expected) in scene.nodes.iter().zip(scene.world_transforms()) {
            // Every node of the zone has a name of its own, so none is known by its index.
            let entity = world
                .find(&PersistentId::new(["zone", node.name.as_str()]))
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

    /// Prints what resolving the zone's cooked scene and spawning it in a world cost, the median of
    /// 21 runs. Run it in a release build:
    /// `cargo test --release -p hunnawood -- --ignored --nocapture spawning_the_zone`.
    #[test]
    #[ignore = "a measurement, which needs the cooked zone"]
    fn what_spawning_the_zone_costs() {
        use std::time::{Duration, Instant};

        let (_, scene) = zone();
        let mut resolving: Vec<Duration> = Vec::new();
        let mut spawning: Vec<Duration> = Vec::new();
        for _ in 0..21 {
            let started = Instant::now();
            let resolved = ResolvedScene::from_cooked(crate::assets::zone_id(), &scene);
            resolving.push(started.elapsed());
            let mut world = World::new();
            let started = Instant::now();
            world.spawn_scene("zone", &resolved).expect("spawned");
            spawning.push(started.elapsed());
        }
        resolving.sort_unstable();
        spawning.sort_unstable();
        eprintln!(
            "the zone's {} nodes: resolved in {:?}, spawned in {:?}",
            scene.nodes.len(),
            resolving[10],
            spawning[10]
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

        let (assets, scene) = zone();
        let mut world = World::new();
        world
            .spawn_scene(
                "zone",
                &ResolvedScene::from_cooked(crate::assets::zone_id(), &scene),
            )
            .expect("spawned");
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
