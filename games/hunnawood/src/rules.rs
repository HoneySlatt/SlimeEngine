//! The rules of the game, without the GPU: where the hero is, which way it faces, and how it moves
//! through the zone, as functions over the game's world (`slime-world`). The world holds the run:
//! the hero, where its entity stands; the key taken and the door open, hidden ([`Hidden`]); the
//! night, [`Night`] on the zone's root. The window, the input and the drawing are elsewhere, so that
//! the rules can be tested alone. They hold no GPU object: a new game creates none.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use slime_assets::CookedScene;
use slime_render::glam::{DVec3, Quat, Vec2, Vec3};
use slime_world::{Component, Entity, Hidden, Transform, World};

use crate::zone::{Footprint, TimeOfDay, Zone, colliders};

/// The hero's radius, in metres: it moves as a circle on the ground.
pub const HERO_RADIUS: f32 = 0.35;
/// How fast the hero moves, in metres per second: a jog.
pub const HERO_SPEED: f32 = 3.0;
/// How fast the hero turns to face where it moves, in radians per second.
const HERO_TURN_SPEED: f32 = 10.0;
/// How near the hero must come to the key's place to take it, in metres: the key lies on a block
/// the hero walks up to.
pub const REACH_ITEM: f32 = 1.2;
/// How near the hero must come to the middle of the door, which stands in a thick wall, to open
/// it with the key.
pub const REACH_DOOR: f32 = 2.0;
/// How near the hero must come to the campfire to rest there.
pub const REACH_CAMPFIRE: f32 = 2.6;
/// The key that the hero's scene is spawned under: the first part of its entities' persistent
/// identities.
const HERO: &str = "hero";

/// It is night: a component of the game's, which the zone's root holds while it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Night;

impl Component for Night {}

/// The hero as the rules see it: where it stands, and which way it faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hero {
    /// Its place on the ground: x and z, in metres.
    pub position: Vec2,
    /// Which way it faces, in radians from +x towards +z.
    pub heading: f32,
}

impl Hero {
    /// Which way it faces, on the ground, of length 1. The tests read it.
    #[cfg(test)]
    pub fn facing(&self) -> Vec2 {
        Vec2::from_angle(self.heading)
    }

    /// The hero that `transform` places: its translation on the ground, and the direction of its
    /// −Z axis, the way the hero's model faces.
    fn of(transform: &Transform) -> Self {
        let facing = transform.rotation * Vec3::NEG_Z;
        Self {
            position: Vec2::new(
                transform.translation.x as f32,
                transform.translation.z as f32,
            ),
            heading: facing.z.atan2(facing.x),
        }
    }

    /// Changes `transform` to place the hero: at its place on the ground, turned about the
    /// vertical so that its −Z axis faces where it does.
    fn place(&self, transform: &mut Transform) {
        transform.translation =
            DVec3::new(f64::from(self.position.x), 0.0, f64::from(self.position.y));
        transform.rotation = Quat::from_rotation_y(-self.heading - FRAC_PI_2);
    }
}

/// The angle of `direction` from +x towards +z, in radians.
fn angle_of(direction: Vec2) -> f32 {
    direction.y.atan2(direction.x)
}

/// The turn from the angle `from` to the angle `to`, the shorter way, from −π to π radians.
fn shortest_turn(from: f32, to: f32) -> f32 {
    (to - from + PI).rem_euclid(TAU) - PI
}

/// `from` turned towards `to` by at most `by` radians, the shorter way.
fn turn_towards(from: f32, to: f32, by: f32) -> f32 {
    from + shortest_turn(from, to).clamp(-by, by)
}

/// A game in progress in a world: the hero, the zone's loop, a key that opens a door, and the time
/// of day, which resting by the campfire turns. It holds the entities it refers to, and the time of
/// day a new game starts at; where the run stands is the world's.
pub struct Game {
    zone: Zone,
    /// The root of the hero's scene, which places the hero.
    hero: Entity,
    /// Whether a new game starts at night.
    starts_at_night: bool,
}

impl Game {
    /// Spawns the zone's cooked scene `zone` and the hero's `hero` in `world` ([`Zone::spawn`]),
    /// and starts a new game in them, by day or at night as `starts_at_night` says, and again so at
    /// each restart: the hero at the start, facing the way it says, the key in its place and the
    /// door closed.
    ///
    /// # Errors
    ///
    /// When the zone cannot be played, as [`Zone::spawn`] says, or an entity has the identity
    /// `hero` already. Nothing is spawned then.
    pub fn spawn(
        world: &mut World,
        zone: &CookedScene,
        hero: &CookedScene,
        starts_at_night: bool,
    ) -> Result<Self, String> {
        let zone = Zone::spawn(world, zone)?;
        let hero = match world.spawn_scene(HERO, hero) {
            Ok(hero) => hero,
            Err(error) => {
                world.despawn(zone.root);
                return Err(error.to_string());
            }
        };
        let game = Self {
            zone,
            hero,
            starts_at_night,
        };
        game.restart(world);
        Ok(game)
    }

    /// Starts a new game in the same zone: the hero at the start, the key in its place, the door
    /// closed, at the time of day the game started at. Out of a fixed step, the hero is drawn at
    /// the start at once.
    pub fn restart(&self, world: &mut World) {
        world.remove::<Hidden>(self.zone.item);
        world.remove::<Hidden>(self.zone.door);
        self.set_night(world, self.starts_at_night);
        let (position, facing) = self.zone.start(world);
        self.place(
            world,
            Hero {
                position,
                heading: angle_of(facing),
            },
        );
    }

    /// Takes `scene`, the zone's cooked scene read again after it changed on disk, and spawns it in
    /// `world` in place of the zone. When its layout changed ([`Zone::layout`]), the game starts
    /// again in it, as [`Game::restart`] does, since the hero could stand in a new wall, or the key
    /// have moved after it was taken; otherwise the run goes on in it, the key and the door as they
    /// were, at the same time of day. Returns whether it started again.
    ///
    /// # Errors
    ///
    /// When the new zone cannot be played, as [`Zone::spawn`] says. Nothing changes then: the zone
    /// is spawned in a world of its own first.
    pub fn reload_zone(&mut self, world: &mut World, scene: &CookedScene) -> Result<bool, String> {
        let mut alone = World::new();
        let layout = Zone::spawn(&mut alone, scene)?.layout(&alone);
        let changed = layout != self.zone.layout(world);
        let (has_item, door_open, night) = (
            self.has_item(world),
            self.door_open(world),
            self.night(world),
        );
        world.despawn(self.zone.root);
        self.zone = Zone::spawn(world, scene).expect("the zone was spawned in a world of its own");
        if changed {
            self.restart(world);
        } else {
            if has_item {
                world.insert(self.zone.item, Hidden);
            }
            if door_open {
                world.insert(self.zone.door, Hidden);
            }
            self.set_night(world, night);
        }
        Ok(changed)
    }

    /// Takes `scene`, the hero's cooked scene read again after it changed on disk, and spawns it in
    /// `world` in place of the hero, where the hero stood.
    ///
    /// # Errors
    ///
    /// As [`World::spawn_scene`], which cannot fail once the hero before is gone.
    pub fn reload_hero(&mut self, world: &mut World, scene: &CookedScene) -> Result<(), String> {
        let placed = world.get::<Transform>(self.hero).copied();
        world.despawn(self.hero);
        self.hero = world
            .spawn_scene(HERO, scene)
            .map_err(|error| error.to_string())?;
        if let Some(placed) = placed {
            world.insert(self.hero, placed);
        }
        Ok(())
    }

    /// The zone's entities.
    pub fn zone(&self) -> &Zone {
        &self.zone
    }

    /// The root of the hero's scene, whose transform places the hero.
    pub fn hero_entity(&self) -> Entity {
        self.hero
    }

    /// Whether it is night.
    pub fn night(&self, world: &World) -> bool {
        world.get::<Night>(self.zone.root).is_some()
    }

    /// Makes it night, or day: the zone's root holds [`Night`] or not, and the zone's lights of the
    /// other time of day are hidden.
    fn set_night(&self, world: &mut World, night: bool) {
        if night {
            world.insert(self.zone.root, Night);
        } else {
            world.remove::<Night>(self.zone.root);
        }
        let now = if night {
            TimeOfDay::Night
        } else {
            TimeOfDay::Day
        };
        let lights: Vec<(Entity, TimeOfDay)> = world
            .query::<&TimeOfDay>()
            .map(|(entity, time)| (entity, *time))
            .collect();
        for (entity, time) in lights {
            if time == now {
                world.remove::<Hidden>(entity);
            } else {
                world.insert(entity, Hidden);
            }
        }
    }

    /// Rests by the campfire, when the hero is near enough to it ([`Game::can_rest`]): the day
    /// turns to night, or the night to day. Returns whether the hero rested.
    pub fn rest(&self, world: &mut World) -> bool {
        let rested = self.can_rest(world);
        if rested {
            let night = self.night(world);
            self.set_night(world, !night);
        }
        rested
    }

    /// Whether the hero has taken the key: then the key is hidden.
    pub fn has_item(&self, world: &World) -> bool {
        world.get::<Hidden>(self.zone.item).is_some()
    }

    /// Whether the key has opened the door: then the door is hidden, and its colliders stop the
    /// hero no more.
    pub fn door_open(&self, world: &World) -> bool {
        world.get::<Hidden>(self.zone.door).is_some()
    }

    /// Whether the hero is near enough to the campfire to rest there.
    pub fn can_rest(&self, world: &World) -> bool {
        let campfire = self.zone.place(world, self.zone.campfire);
        self.hero(world).position.distance(campfire) <= REACH_CAMPFIRE
    }

    /// The hero as the world places it after the last step. The world draws it between its last
    /// two steps itself ([`World::interpolated_transform`]).
    pub fn hero(&self, world: &World) -> Hero {
        world.get::<Transform>(self.hero).map_or(
            Hero {
                position: Vec2::ZERO,
                heading: 0.0,
            },
            Hero::of,
        )
    }

    /// Places the hero's entity as `hero` says.
    fn place(&self, world: &mut World, hero: Hero) {
        if let Some(transform) = world.get_mut::<Transform>(self.hero) {
            hero.place(transform);
        }
    }

    /// Plays `step` seconds: the hero moves towards `direction`, on the ground, at the share of
    /// its speed that the direction's length gives, at most 1, stopped by the colliders that are
    /// not hidden and the zone's edge and sliding along them, and turns to face that direction.
    /// Then the hero takes the key once near it ([`REACH_ITEM`]), and with it opens the door once
    /// near it ([`REACH_DOOR`]): each is hidden then.
    pub fn step(&self, world: &mut World, direction: Vec2, step: f32) {
        let colliders = colliders(world);
        let inside = self.zone.half_size(world) - HERO_RADIUS;
        let mut hero = self.hero(world);
        let travel = direction.clamp_length_max(1.0) * HERO_SPEED * step;
        // In moves of at most half the radius, so that no move carries the hero past a corner or
        // through a thin collider.
        let parts = (travel.length() / (HERO_RADIUS / 2.0)).ceil().max(1.0);
        let part = travel / parts;
        for _ in 0..parts as u32 {
            let position = push_out(hero.position + part, HERO_RADIUS, &colliders);
            hero.position = position.clamp(-inside, inside);
        }
        if direction.length_squared() > 1e-6 {
            let heading = turn_towards(hero.heading, angle_of(direction), HERO_TURN_SPEED * step);
            // Kept from −π to π, however many times the hero turns round.
            hero.heading = (heading + PI).rem_euclid(TAU) - PI;
        }
        self.place(world, hero);
        let position = hero.position;
        let item = self.zone.place(world, self.zone.item);
        if !self.has_item(world) && position.distance(item) <= REACH_ITEM {
            world.insert(self.zone.item, Hidden);
            tracing::info!("the hero takes the key");
        }
        let door = self.zone.place(world, self.zone.door);
        if self.has_item(world) && !self.door_open(world) && position.distance(door) <= REACH_DOOR {
            world.insert(self.zone.door, Hidden);
            tracing::info!("the key opens the door");
        }
    }
}

/// `position` moved out of every collider that the circle of `radius` around it overlaps.
fn push_out(mut position: Vec2, radius: f32, colliders: &[Footprint]) -> Vec2 {
    // Pushed out of one collider, the circle can enter another, in a corner between two: a few
    // rounds settle it.
    for _ in 0..4 {
        let mut pushed = false;
        for collider in colliders {
            if let Some(out) = collider.push_out(position, radius) {
                position = out;
                pushed = true;
            }
        }
        if !pushed {
            break;
        }
    }
    position
}

#[cfg(test)]
pub mod tests {
    use slime_assets::{Extra, SceneLight, SceneNode};
    use slime_render::glam::Mat4;

    use super::*;
    use crate::zone::Shape;

    /// The fixed step the game runs at: 1/60 s.
    const STEP: f32 = 1.0 / 60.0;

    /// A zone to build a cooked scene of: its markers' places, the size of its ground, and its
    /// colliders, the door's among the door's nodes.
    #[derive(Clone)]
    pub struct Plan {
        pub half_size: Vec2,
        pub start: Vec2,
        /// Which way the hero faces at the start, of length 1.
        pub facing: Vec2,
        pub item: Vec2,
        pub door: Vec2,
        pub campfire: Vec2,
        pub terrace: Vec2,
        pub colliders: Vec<Footprint>,
        pub door_colliders: Vec<Footprint>,
    }

    /// A node named `name` with the custom values `extras`, placed by `transform`.
    fn node(
        name: &str,
        parent: Option<u32>,
        transform: Mat4,
        extras: &[(&str, Extra)],
    ) -> SceneNode {
        SceneNode {
            name: name.to_owned(),
            parent,
            transform: transform.to_cols_array(),
            meshes: Vec::new(),
            light: None,
            extras: extras
                .iter()
                .map(|(name, extra)| ((*name).to_owned(), extra.clone()))
                .collect(),
        }
    }

    /// A marker's custom values.
    fn marker(kind: &str) -> [(&'static str, Extra); 1] {
        [("marker", Extra::Text(kind.to_owned()))]
    }

    /// The node of `collider`, placed relative to `origin`, its parent's place.
    fn collider_node(
        name: &str,
        parent: Option<u32>,
        origin: Vec2,
        collider: &Footprint,
    ) -> SceneNode {
        let (kind, scale, yaw) = match collider.shape {
            Shape::Box { half_size, yaw } => ("box", Vec3::new(half_size.x, 1.0, half_size.y), yaw),
            Shape::Cylinder { radius } => ("cylinder", Vec3::new(radius, 1.0, radius), 0.0),
        };
        let center = collider.center - origin;
        node(
            name,
            parent,
            // Turned by −yaw about +y, so that its x axis lies along `Vec2::from_angle(yaw)`.
            Mat4::from_scale_rotation_translation(
                scale,
                Quat::from_rotation_y(-yaw),
                Vec3::new(center.x, 0.0, center.y),
            ),
            &[
                ("collider", Extra::Text(kind.to_owned())),
                ("height", Extra::Number(f64::from(collider.height))),
            ],
        )
    }

    /// At `place` on the ground.
    fn at(place: Vec2) -> Mat4 {
        Mat4::from_translation(Vec3::new(place.x, 0.0, place.y))
    }

    impl Plan {
        /// The cooked scene of this zone, as the zone's export holds one.
        pub fn scene(&self) -> CookedScene {
            let facing = Vec3::new(self.facing.x, 0.0, self.facing.y);
            let mut nodes = vec![
                node(
                    "start",
                    None,
                    // Its −Z axis along `facing`.
                    Mat4::from_rotation_translation(
                        Quat::from_rotation_y((-facing.x).atan2(-facing.z)),
                        Vec3::new(self.start.x, 0.0, self.start.y),
                    ),
                    &marker("start"),
                ),
                node("item", None, at(self.item), &marker("item")),
                node("campfire", None, at(self.campfire), &marker("campfire")),
                node("terrace", None, at(self.terrace), &marker("terrace")),
                node(
                    "bounds",
                    None,
                    Mat4::from_scale(Vec3::new(self.half_size.x, 1.0, self.half_size.y)),
                    &marker("bounds"),
                ),
                node("door", None, at(self.door), &marker("door")),
            ];
            let door = (nodes.len() - 1) as u32;
            for (index, collider) in self.door_colliders.iter().enumerate() {
                let name = format!("door collider {index}");
                nodes.push(collider_node(&name, Some(door), self.door, collider));
            }
            for (index, collider) in self.colliders.iter().enumerate() {
                let name = format!("collider {index}");
                nodes.push(collider_node(&name, None, Vec2::ZERO, collider));
            }
            CookedScene {
                name: "plan".to_owned(),
                nodes,
            }
        }

        /// A game in this zone, in a world of its own, with a hero that draws nothing.
        pub fn play(&self) -> (World, Game) {
            let mut world = World::new();
            let game = Game::spawn(&mut world, &self.scene(), &no_hero(), false)
                .expect("the zone can be played");
            (world, game)
        }
    }

    /// A hero's scene without nodes.
    pub fn no_hero() -> CookedScene {
        CookedScene {
            name: "hero".to_owned(),
            nodes: Vec::new(),
        }
    }

    /// A large zone of the given colliders, the hero starting at `start` and facing +x; the key,
    /// the door and the campfire far away.
    pub fn zone(start: [f32; 2], colliders: Vec<Footprint>) -> Plan {
        let far = Vec2::splat(1000.0);
        Plan {
            half_size: Vec2::splat(50.0),
            start: Vec2::from(start),
            facing: Vec2::X,
            item: far,
            door: far,
            campfire: far,
            terrace: far,
            colliders,
            door_colliders: Vec::new(),
        }
    }

    /// A zone crossed by a wall along x = 10, but for a door in it at the origin of z, which
    /// its own collider closes; the key lies on the way, at x = 5.
    fn walled(item: [f32; 2]) -> Plan {
        let mut zone = zone(
            [0.0, 0.0],
            vec![
                turned_box([10.0, 21.0], [0.6, 40.0], 0.0),
                turned_box([10.0, -21.0], [0.6, 40.0], 0.0),
            ],
        );
        zone.item = Vec2::from(item);
        zone.door = Vec2::new(10.0, 0.0);
        zone.door_colliders = vec![turned_box([10.0, 0.0], [0.6, 2.0], 0.0)];
        zone.campfire = Vec2::new(0.0, -10.0);
        zone
    }

    /// Whether `a` and `b` are the same hero, but for the rounding of a rotation.
    fn same_hero(a: Hero, b: Hero) -> bool {
        a.position.abs_diff_eq(b.position, 1e-5) && shortest_turn(a.heading, b.heading).abs() < 1e-5
    }

    #[test]
    fn the_key_taken_on_the_way_opens_the_door() {
        let (mut world, game) = walled([5.0, 0.0]).play();
        hold(&game, &mut world, Vec2::X, 2.0);
        assert!(game.has_item(&world) && !game.door_open(&world));
        hold(&game, &mut world, Vec2::X, 4.0);
        assert!(game.door_open(&world));
        assert_eq!(colliders(&world).len(), 2);
        let hero = game.hero(&world).position;
        assert!(hero.x > 12.0, "{hero}");
    }

    #[test]
    fn without_the_key_the_door_stops_the_hero() {
        let (mut world, game) = walled([0.0, 30.0]).play();
        hold(&game, &mut world, Vec2::X, 6.0);
        assert!(!game.has_item(&world) && !game.door_open(&world));
        let x = game.hero(&world).position.x;
        assert!((x - (9.7 - HERO_RADIUS)).abs() < 1e-4, "{x}");
    }

    #[test]
    fn a_restart_puts_back_the_key_and_closes_the_door() {
        let (mut world, game) = walled([5.0, 0.0]).play();
        let first = game.hero(&world);
        hold(&game, &mut world, Vec2::X, 6.0);
        assert!(game.door_open(&world));
        game.restart(&mut world);
        assert_eq!(game.hero(&world), first);
        assert!(!game.has_item(&world) && !game.door_open(&world));
        assert_eq!(colliders(&world).len(), 3);
    }

    #[test]
    fn a_change_of_layout_starts_the_run_again_and_a_change_of_looks_does_not() {
        let plan = walled([5.0, 0.0]);
        let (mut world, mut game) = plan.play();
        let first = game.hero(&world);
        hold(&game, &mut world, Vec2::X, 2.0);
        assert!(game.has_item(&world));
        let hero = game.hero(&world);
        // The same layout, which looks otherwise, with a lamp more: the run goes on in it.
        let mut looks = plan.scene();
        let mut lamp = node("lamp", None, at(Vec2::new(3.0, 3.0)), &[]);
        lamp.light = Some(SceneLight {
            kind: slime_assets::LightKind::Point,
            color: [1.0, 0.8, 0.6],
            intensity: 30.0,
            range: None,
        });
        looks.nodes.push(lamp);
        let zone = *game.zone();
        assert!(!game.reload_zone(&mut world, &looks).expect("playable"));
        assert_ne!(*game.zone(), zone, "the zone was spawned again");
        assert!(!world.contains(zone.root));
        assert_eq!(game.hero(&world), hero);
        assert!(game.has_item(&world) && world.is_hidden(game.zone().item));
        // A wall moved: the run starts again, in the new zone.
        let mut moved = plan.clone();
        moved.colliders[0].center.x = 11.0;
        assert!(
            game.reload_zone(&mut world, &moved.scene())
                .expect("playable")
        );
        assert_eq!(game.hero(&world), first);
        assert!(!game.has_item(&world));
        assert_eq!(
            game.zone().layout(&world).colliders,
            moved.play().1.zone().layout(&moved.play().0).colliders
        );
        // A zone that cannot be played, without a start, changes nothing.
        let zone = *game.zone();
        let mut broken = moved.scene();
        broken.nodes[0].extras.clear();
        let error = game.reload_zone(&mut world, &broken).expect_err("no start");
        assert!(error.contains("0 markers `start`"), "{error}");
        assert_eq!(*game.zone(), zone);
        assert!(world.contains(zone.root));
    }

    #[test]
    fn resting_by_the_campfire_turns_the_day_to_night_and_back() {
        let (mut world, game) = walled([0.0, 30.0]).play();
        assert!(!game.can_rest(&world));
        assert!(
            !game.rest(&mut world) && !game.night(&world),
            "rested far from the campfire"
        );
        hold(&game, &mut world, -Vec2::Y, 2.8);
        assert!(game.can_rest(&world), "{}", game.hero(&world).position);
        assert!(game.rest(&mut world) && game.night(&world));
        assert!(game.rest(&mut world) && !game.night(&world));
        assert!(game.rest(&mut world) && game.night(&world));
        // A new game starts by day again, as this one did.
        game.restart(&mut world);
        assert!(!game.night(&world));
    }

    #[test]
    fn a_game_started_at_night_starts_again_at_night() {
        let mut world = World::new();
        let scene = walled([0.0, 30.0]).scene();
        let game = Game::spawn(&mut world, &scene, &no_hero(), true).expect("playable");
        assert!(game.night(&world));
        hold(&game, &mut world, -Vec2::Y, 2.8);
        assert!(game.rest(&mut world) && !game.night(&world));
        game.restart(&mut world);
        assert!(game.night(&world));
    }

    #[test]
    fn the_lights_of_the_other_time_of_day_are_hidden() {
        let mut scene = walled([0.0, 30.0]).scene();
        for (name, time) in [("sun", "day"), ("lantern", "night")] {
            let mut light = node(
                name,
                None,
                at(Vec2::ZERO),
                &[("time", Extra::Text(time.to_owned()))],
            );
            light.light = Some(SceneLight {
                kind: slime_assets::LightKind::Point,
                color: [1.0; 3],
                intensity: 1.0,
                range: None,
            });
            scene.nodes.push(light);
        }
        let mut world = World::new();
        let game = Game::spawn(&mut world, &scene, &no_hero(), false).expect("playable");
        let find = |world: &World, name: &str| {
            world
                .find(&slime_world::PersistentId::new(["zone", name]))
                .expect("spawned")
        };
        let (sun, lantern) = (find(&world, "sun"), find(&world, "lantern"));
        assert!(!world.is_hidden(sun) && world.is_hidden(lantern));
        hold(&game, &mut world, -Vec2::Y, 2.8);
        assert!(game.rest(&mut world));
        assert!(world.is_hidden(sun) && !world.is_hidden(lantern));
    }

    pub fn turned_box(center: [f32; 2], size: [f32; 2], yaw: f32) -> Footprint {
        Footprint {
            shape: Shape::Box {
                half_size: Vec2::from(size) / 2.0,
                yaw,
            },
            center: Vec2::from(center),
            height: 2.0,
        }
    }

    fn cylinder(center: [f32; 2], radius: f32) -> Footprint {
        Footprint {
            shape: Shape::Cylinder { radius },
            center: Vec2::from(center),
            height: 2.0,
        }
    }

    /// Plays `seconds` of holding `direction`.
    fn hold(game: &Game, world: &mut World, direction: Vec2, seconds: f32) {
        for _ in 0..(seconds / STEP).round() as u32 {
            game.step(world, direction, STEP);
        }
    }

    /// How far the hero's center is from `collider`'s footprint, measured outside it: at least
    /// the hero's radius when they do not overlap.
    fn clearance(position: Vec2, collider: &Footprint) -> f32 {
        match collider.shape {
            Shape::Box { half_size, yaw } => {
                let local = Vec2::from_angle(-yaw).rotate(position - collider.center);
                (local.abs() - half_size).max(Vec2::ZERO).length()
            }
            Shape::Cylinder { radius } => position.distance(collider.center) - radius,
        }
    }

    #[test]
    fn the_colliders_read_from_the_world_are_those_of_the_scene() {
        let plan = walled([5.0, 0.0]);
        let turned = turned_box([-4.0, 3.0], [2.0, 0.5], 0.7);
        let pillar = cylinder([2.0, -6.0], 0.4);
        let mut plan = plan;
        plan.colliders.extend([turned, pillar]);
        let (world, game) = plan.play();
        let layout = game.zone().layout(&world);
        assert_eq!(layout.door_colliders.len(), 1);
        for (read, made) in layout
            .colliders
            .iter()
            .chain(&layout.door_colliders)
            .zip(plan.colliders.iter().chain(&plan.door_colliders))
        {
            assert!(read.center.abs_diff_eq(made.center, 1e-5), "{read:?}");
            assert_eq!(read.height, made.height);
            match (read.shape, made.shape) {
                (
                    Shape::Box { half_size, yaw },
                    Shape::Box {
                        half_size: h,
                        yaw: y,
                    },
                ) => {
                    assert!(
                        half_size.abs_diff_eq(h, 1e-5) && (yaw - y).abs() < 1e-5,
                        "{read:?}"
                    );
                }
                (Shape::Cylinder { radius }, Shape::Cylinder { radius: r }) => {
                    assert!((radius - r).abs() < 1e-5, "{read:?}");
                }
                _ => panic!("{read:?} is not {made:?}"),
            }
        }
        assert!(same_hero(
            game.hero(&world),
            Hero {
                position: plan.start,
                heading: 0.0
            }
        ));
    }

    #[test]
    fn the_hero_moves_at_its_speed_and_a_short_direction_slows_it() {
        let (mut world, game) = zone([0.0, 0.0], Vec::new()).play();
        hold(&game, &mut world, Vec2::X, 1.0);
        let hero = game.hero(&world).position;
        assert!((hero.x - HERO_SPEED).abs() < 1e-4, "{hero}");
        // A diagonal of length √2 moves at the same speed, a direction of length 0.5 at half.
        let (mut world, game) = zone([0.0, 0.0], Vec::new()).play();
        hold(&game, &mut world, Vec2::ONE, 1.0);
        assert!((game.hero(&world).position.length() - HERO_SPEED).abs() < 1e-4);
        let (mut world, game) = zone([0.0, 0.0], Vec::new()).play();
        hold(&game, &mut world, Vec2::new(0.0, 0.5), 1.0);
        assert!((game.hero(&world).position.y - HERO_SPEED / 2.0).abs() < 1e-4);
    }

    /// A wall turned by half a radian, 0.5 m thick, whose near face lies across the line from the
    /// origin along its first axis: the normal of that face is `normal`, its line along the wall
    /// `along`.
    fn turned_wall() -> (Footprint, Vec2, Vec2) {
        let yaw = 0.5;
        let normal = Vec2::from_angle(yaw);
        let along = Vec2::new(-normal.y, normal.x);
        (turned_box([3.0, 0.0], [0.5, 10.0], yaw), normal, along)
    }

    #[test]
    fn a_turned_wall_stops_the_hero_at_its_radius() {
        let (wall, normal, _) = turned_wall();
        let face = Vec2::new(3.0, 0.0).dot(normal) - 0.25;
        let (mut world, game) = zone([0.0, 0.0], vec![wall]).play();
        hold(&game, &mut world, normal, 3.0);
        let hero = game.hero(&world).position;
        assert!(
            (hero.dot(normal) - (face - HERO_RADIUS)).abs() < 1e-4,
            "{hero}"
        );
    }

    #[test]
    fn the_hero_slides_along_a_turned_wall_it_moves_into_at_an_angle() {
        let (wall, normal, along) = turned_wall();
        let face = Vec2::new(3.0, 0.0).dot(normal) - 0.25;
        let (mut world, game) = zone([0.0, 0.0], vec![wall]).play();
        // +x meets the wall at an angle, and slides along it towards −along.
        hold(&game, &mut world, Vec2::X, 3.0);
        let hero = game.hero(&world).position;
        assert!(
            (hero.dot(normal) - (face - HERO_RADIUS)).abs() < 1e-4,
            "{hero}"
        );
        // The part of the speed along the wall, for the time after the hero met it.
        let met = (face - HERO_RADIUS) / normal.x / HERO_SPEED;
        let slid = HERO_SPEED * normal.y * (3.0 - met);
        let start = met * HERO_SPEED * Vec2::X;
        assert!(
            ((start - hero).dot(along) - slid).abs() < 0.05,
            "{hero}: slid {} instead of {slid}",
            (start - hero).dot(along)
        );
    }

    #[test]
    fn a_cylinder_stops_the_hero_and_the_hero_goes_round_it() {
        // Straight at its center, the hero stops at both radii from it.
        let pillar = cylinder([3.0, 0.0], 0.5);
        let (mut world, game) = zone([0.0, 0.0], vec![pillar]).play();
        hold(&game, &mut world, Vec2::X, 3.0);
        let hero = game.hero(&world).position;
        assert!((hero.x - (3.0 - 0.5 - HERO_RADIUS)).abs() < 1e-4, "{hero}");
        // A little to its side, the hero slides round it and goes on, never inside it.
        let (mut world, game) = zone([0.0, 0.3], vec![pillar]).play();
        for _ in 0..240 {
            game.step(&mut world, Vec2::X, STEP);
            let clear = clearance(game.hero(&world).position, &pillar);
            assert!(clear > HERO_RADIUS - 1e-3, "{}", game.hero(&world).position);
        }
        let hero = game.hero(&world).position;
        assert!(hero.x > 4.0, "{hero}");
    }

    #[test]
    fn the_hero_goes_round_a_turned_corner_without_crossing_it() {
        // A block turned by 0.4 rad, its corner towards the hero, who walks at it and along it.
        let block = turned_box([3.0, 3.0], [2.0, 2.0], 0.4);
        for direction in [Vec2::ONE, Vec2::new(1.0, 0.8), Vec2::new(0.8, 1.0)] {
            let (mut world, game) = zone([0.0, 0.0], vec![block]).play();
            for _ in 0..300 {
                game.step(&mut world, direction, STEP);
                let clear = clearance(game.hero(&world).position, &block);
                assert!(
                    clear > HERO_RADIUS - 1e-3,
                    "{direction}: {} overlaps the block",
                    game.hero(&world).position
                );
            }
        }
    }

    #[test]
    fn a_fast_move_does_not_cross_a_thin_wall_or_a_thin_pole() {
        // A step of a tenth of a second at full speed is 0.3 m; the wall is 0.1 m thick and
        // turned, the pole 0.1 m across. The hero stays on the near side of each: towards +x, the
        // wall's first axis, or along x for the pole, at which it walks straight.
        for (collider, across) in [
            (
                turned_box([1.0, 0.0], [0.1, 10.0], 0.3),
                Vec2::from_angle(0.3),
            ),
            (cylinder([1.0, 0.0], 0.05), Vec2::X),
        ] {
            let (mut world, game) = zone([0.0, 0.0], vec![collider]).play();
            for _ in 0..30 {
                game.step(&mut world, Vec2::X, 0.1);
            }
            let hero = game.hero(&world).position;
            assert!(
                (hero - collider.center).dot(across) < 0.0,
                "{collider:?}: {hero}"
            );
        }
    }

    #[test]
    fn the_hero_does_not_pass_a_gap_narrower_than_itself() {
        // A turned box and a pillar whose nearest points, the box's corner at (−0.42, 2.71) and
        // the pillar's side, are 0.56 m apart, less than the hero's 0.7 m.
        let colliders = vec![
            turned_box([-1.5, 3.0], [2.0, 1.0], 0.2),
            cylinder([0.6, 3.0], 0.5),
        ];
        for start in [-0.4, -0.2, 0.0] {
            let (mut world, game) = zone([start, 0.0], colliders.clone()).play();
            hold(&game, &mut world, Vec2::Y, 4.0);
            let hero = game.hero(&world).position;
            assert!(hero.y < 3.0, "from x = {start}: {hero}");
        }
        // With the pillar 0.5 m further, the gap is 1.05 m, and the hero goes through.
        let colliders = vec![colliders[0], cylinder([1.1, 3.0], 0.5)];
        let (mut world, game) = zone([0.3, 0.0], colliders).play();
        hold(&game, &mut world, Vec2::Y, 4.0);
        let hero = game.hero(&world).position;
        assert!(hero.y > 5.0, "{hero}");
    }

    #[test]
    fn the_edge_of_the_zone_stops_the_hero() {
        let (mut world, game) = Plan {
            half_size: Vec2::splat(5.0),
            ..zone([0.0, 0.0], Vec::new())
        }
        .play();
        hold(&game, &mut world, Vec2::new(1.0, -0.5), 10.0);
        let hero = game.hero(&world).position;
        assert!((hero.x - (5.0 - HERO_RADIUS)).abs() < 1e-5, "{hero}");
        assert!((hero.y + (5.0 - HERO_RADIUS)).abs() < 1e-5, "{hero}");
    }

    #[test]
    fn the_hero_turns_to_face_where_it_moves_the_shorter_way() {
        let (mut world, game) = zone([0.0, 0.0], Vec::new()).play();
        // Facing +x, asked to go to −z: a quarter turn, which takes a little time.
        game.step(&mut world, -Vec2::Y, STEP);
        let heading = game.hero(&world).heading;
        assert!(heading < 0.0 && heading > -PI / 2.0, "{heading}");
        hold(&game, &mut world, -Vec2::Y, 0.5);
        assert!(game.hero(&world).facing().abs_diff_eq(-Vec2::Y, 1e-5));
        // Standing still keeps the heading.
        hold(&game, &mut world, Vec2::ZERO, 0.5);
        assert!(game.hero(&world).facing().abs_diff_eq(-Vec2::Y, 1e-5));
        // From just below π to just above −π is a small turn, not a whole one.
        assert!((shortest_turn(3.0, -3.0) - (TAU - 6.0)).abs() < 1e-5);
        assert_eq!(turn_towards(0.0, 1.0, 0.25), 0.25);
    }

    #[test]
    fn the_hero_is_drawn_between_its_last_two_steps_and_a_restart_starts_again() {
        let (mut world, game) = zone([1.0, 2.0], Vec::new()).play();
        let first = game.hero(&world);
        world.begin_fixed_step();
        game.step(&mut world, Vec2::X, 0.1);
        world.end_fixed_step();
        let middle = world
            .interpolated_transform(game.hero_entity(), 0.5)
            .expect("placed")
            .translation;
        assert!(
            (middle.x - f64::from(1.0 + HERO_SPEED * 0.05)).abs() < 1e-4,
            "{middle}"
        );
        world.begin_fixed_step();
        hold(&game, &mut world, Vec2::Y, 1.0);
        world.end_fixed_step();
        // Out of a step, the restart is drawn at once, whatever the share of a step.
        game.restart(&mut world);
        assert_eq!(game.hero(&world), first);
        let drawn = world
            .interpolated_transform(game.hero_entity(), 0.3)
            .expect("placed");
        assert_eq!(
            Hero::of(&Transform {
                translation: drawn.translation,
                rotation: drawn.rotation,
                ..Transform::IDENTITY
            }),
            first
        );
    }

    /// Walks the hero to each of `waypoints` in turn, (x, z), straight, sliding along what it
    /// meets; fails if one is not reached within a minute.
    fn walk(game: &Game, world: &mut World, waypoints: &[[f32; 2]]) {
        for &waypoint in waypoints {
            let waypoint = Vec2::from(waypoint);
            let mut steps = 0;
            while game.hero(world).position.distance(waypoint) > 0.3 {
                let direction = (waypoint - game.hero(world).position).normalize();
                game.step(world, direction, STEP);
                steps += 1;
                assert!(
                    steps < 3600,
                    "stuck at {} on the way to {waypoint}",
                    game.hero(world).position
                );
            }
        }
    }

    /// The way from the start along the path to the chapel's portal, in the cooked zone.
    const TO_THE_CHAPEL: [[f32; 2]; 10] = [
        [0.6, 45.0],
        [-0.8, 40.0],
        [0.4, 30.0],
        [2.0, 23.0],
        [2.0, 7.0],
        [0.6, 0.0],
        [-0.4, -3.5],
        [-1.6, -8.0],
        [-1.0, -16.0],
        [0.0, -22.5],
    ];

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn a_walk_rests_at_the_campfire_takes_the_key_and_opens_the_door() {
        let (mut world, game) = crate::zone::cooked::game();
        let first = game.hero(&world);
        walk(&game, &mut world, &TO_THE_CHAPEL[..7]);
        // The branch to the campfire, and back.
        walk(
            &game,
            &mut world,
            &[[-5.0, -4.5], [-9.0, -4.0], [-11.0, -4.0]],
        );
        assert!(game.can_rest(&world), "{}", game.hero(&world).position);
        walk(&game, &mut world, &[[-5.0, -4.5], [-0.4, -3.5]]);
        walk(&game, &mut world, &TO_THE_CHAPEL[7..]);
        assert!(!game.has_item(&world));
        // Up the nave to the key, then to the door, which it opens, and onto the terrace.
        walk(
            &game,
            &mut world,
            &[[0.0, -27.0], [0.0, -38.0], [0.8, -41.6]],
        );
        assert!(game.has_item(&world) && !game.door_open(&world));
        walk(&game, &mut world, &[[0.0, -43.4]]);
        assert!(game.door_open(&world));
        walk(&game, &mut world, &[[0.0, -46.0], [0.0, -49.0]]);
        let terrace = game.zone().place(&world, game.zone().terrace);
        assert!(game.hero(&world).position.distance(terrace) < 1.0);
        // A new game starts from the same state as the first.
        let colliders_at_first = game.zone().layout(&world);
        game.restart(&mut world);
        assert_eq!(game.hero(&world), first);
        assert!(!game.has_item(&world) && !game.door_open(&world));
        assert_eq!(
            colliders(&world).len(),
            colliders_at_first.colliders.len() + colliders_at_first.door_colliders.len()
        );
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_door_stops_the_hero_before_the_key_is_found() {
        let (mut world, game) = crate::zone::cooked::game();
        let door = game.zone().place(&world, game.zone().door);
        walk(&game, &mut world, &TO_THE_CHAPEL);
        // Up the west side of the nave, away from the key, to the door, and on against it.
        walk(&game, &mut world, &[[-0.8, -38.0], [-0.8, -43.3]]);
        hold(&game, &mut world, -Vec2::Y, 3.0);
        assert!(!game.has_item(&world) && !game.door_open(&world));
        let hero = game.hero(&world).position;
        assert!(hero.y > door.y + 0.4, "{hero}");
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_hero_stays_in_the_cooked_zone_and_out_of_its_colliders() {
        let (mut world, game) = crate::zone::cooked::game();
        let all = colliders(&world);
        for direction in [
            -Vec2::Y,
            Vec2::X,
            -Vec2::X,
            Vec2::new(0.3, -1.0),
            Vec2::new(-0.6, -1.0),
            Vec2::Y,
        ] {
            for _ in 0..(12.0 / STEP) as u32 {
                game.step(&mut world, direction, STEP);
                for collider in &all {
                    let clear = clearance(game.hero(&world).position, collider);
                    assert!(
                        clear > HERO_RADIUS - 1e-3,
                        "{direction}: {} overlaps {collider:?}",
                        game.hero(&world).position
                    );
                }
            }
        }
    }
}
