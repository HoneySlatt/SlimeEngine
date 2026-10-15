//! The rules of the game, without the GPU: where the hero is, which way it faces, and how it moves
//! through the zone. The window, the input and the drawing are elsewhere, so that the rules can be
//! tested alone. They hold no GPU object: a new game creates none.

use std::f32::consts::{PI, TAU};

use slime_render::glam::Vec2;

use crate::zone::{Collider, Zone};

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

/// The hero as it is drawn: where it stands, and which way it faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hero {
    /// Its place on the ground: x and z, in metres.
    pub position: Vec2,
    /// Which way it faces, in radians from +x towards +z.
    pub heading: f32,
}

impl Hero {
    /// Which way it faces, on the ground, of length 1.
    pub fn facing(&self) -> Vec2 {
        Vec2::from_angle(self.heading)
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

/// A game in progress: the hero, the zone's loop, a key that opens a door, and the time of day,
/// which resting by the campfire turns.
pub struct Game {
    zone: Zone,
    hero: Hero,
    /// The hero before the last step, so that it is drawn between its last two steps.
    previous_hero: Hero,
    /// Whether the hero has taken the key.
    has_item: bool,
    /// Whether the key has opened the door.
    door_open: bool,
    /// The colliders that stop the hero now: the zone's, but the door's once it is open.
    colliders: Vec<Collider>,
    /// Whether it is night.
    night: bool,
    /// Whether a new game starts at night.
    starts_at_night: bool,
}

impl Game {
    /// A new game in `zone`, by day: the hero at its start, facing the way it says.
    pub fn new(zone: Zone) -> Self {
        let start = start(&zone);
        Self {
            colliders: zone.colliders.clone(),
            zone,
            hero: start,
            previous_hero: start,
            has_item: false,
            door_open: false,
            night: false,
            starts_at_night: false,
        }
    }

    /// The same game, starting at night, and starting again at night.
    pub fn starting_at_night(mut self) -> Self {
        self.night = true;
        self.starts_at_night = true;
        self
    }

    /// Starts a new game in the same zone, as [`Game::new`] does: the hero at the start, the key
    /// in its place, the door closed, at the time of day the game started at.
    pub fn restart(&mut self) {
        self.hero = start(&self.zone);
        self.previous_hero = self.hero;
        self.has_item = false;
        self.door_open = false;
        self.colliders = self.zone.colliders.clone();
        self.night = self.starts_at_night;
    }

    /// Whether it is night.
    pub fn night(&self) -> bool {
        self.night
    }

    /// Rests by the campfire, when the hero is near enough to it ([`Game::can_rest`]): the day
    /// turns to night, or the night to day. Returns whether the hero rested.
    pub fn rest(&mut self) -> bool {
        let rested = self.can_rest();
        if rested {
            self.night = !self.night;
        }
        rested
    }

    pub fn zone(&self) -> &Zone {
        &self.zone
    }

    /// Whether the hero has taken the key.
    pub fn has_item(&self) -> bool {
        self.has_item
    }

    /// Whether the key has opened the door.
    pub fn door_open(&self) -> bool {
        self.door_open
    }

    /// The colliders that stop the hero now: the zone's, but the door's once it is open.
    pub fn colliders(&self) -> &[Collider] {
        &self.colliders
    }

    /// Whether the hero is near enough to the campfire to rest there.
    pub fn can_rest(&self) -> bool {
        self.hero.position.distance(self.zone.campfire) <= REACH_CAMPFIRE
    }

    /// The hero after the last step. The game draws [`Game::hero_between`] instead; the tests read
    /// this.
    #[cfg(test)]
    pub fn hero(&self) -> Hero {
        self.hero
    }

    /// The hero as it is drawn `alpha` of the way, from 0 to 1, between where the last step started
    /// and where it ended: the share of a step that the frame comes after it
    /// (`Context::fixed_step_alpha`), so that it moves smoothly at any frame rate.
    pub fn hero_between(&self, alpha: f32) -> Hero {
        let (before, after) = (self.previous_hero, self.hero);
        Hero {
            position: before.position.lerp(after.position, alpha),
            heading: before.heading + shortest_turn(before.heading, after.heading) * alpha,
        }
    }

    /// Plays `step` seconds: the hero moves towards `direction`, on the ground, at the share of
    /// its speed that the direction's length gives, at most 1, stopped by the colliders and the
    /// zone's edge and sliding along them, and turns to face that direction. Then the hero takes
    /// the key once near it ([`REACH_ITEM`]), and with it opens the door once near it
    /// ([`REACH_DOOR`]), whose colliders stop it no more.
    pub fn step(&mut self, direction: Vec2, step: f32) {
        self.previous_hero = self.hero;
        let travel = direction.clamp_length_max(1.0) * HERO_SPEED * step;
        // In moves of at most half the radius, so that no move carries the hero past a corner or
        // through a thin collider.
        let parts = (travel.length() / (HERO_RADIUS / 2.0)).ceil().max(1.0);
        let part = travel / parts;
        for _ in 0..parts as u32 {
            let position = push_out(self.hero.position + part, HERO_RADIUS, &self.colliders);
            let inside = self.zone.half_size - HERO_RADIUS;
            self.hero.position = position.clamp(-inside, inside);
        }
        if direction.length_squared() > 1e-6 {
            let heading = turn_towards(
                self.hero.heading,
                angle_of(direction),
                HERO_TURN_SPEED * step,
            );
            // Kept from −π to π, however many times the hero turns round.
            self.hero.heading = (heading + PI).rem_euclid(TAU) - PI;
        }
        let position = self.hero.position;
        if !self.has_item && position.distance(self.zone.item) <= REACH_ITEM {
            self.has_item = true;
            tracing::info!("the hero takes the key");
        }
        if self.has_item && !self.door_open && position.distance(self.zone.door) <= REACH_DOOR {
            self.door_open = true;
            let door = &self.zone.door_colliders;
            self.colliders = (0..self.zone.colliders.len())
                .filter(|index| !door.contains(index))
                .map(|index| self.zone.colliders[index])
                .collect();
            tracing::info!("the key opens the door");
        }
    }
}

/// The hero at the start of a game in `zone`.
fn start(zone: &Zone) -> Hero {
    Hero {
        position: zone.start,
        heading: angle_of(zone.facing),
    }
}

/// `position` moved out of every collider that the circle of `radius` around it overlaps.
fn push_out(mut position: Vec2, radius: f32, colliders: &[Collider]) -> Vec2 {
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
mod tests {
    use super::*;
    use crate::zone::Shape;

    /// The fixed step the game runs at: 1/60 s.
    const STEP: f32 = 1.0 / 60.0;

    /// A large zone of the given colliders, the hero starting at `start` and facing +x; the key,
    /// the door and the campfire far away.
    fn zone(start: [f32; 2], colliders: Vec<Collider>) -> Zone {
        let far = Vec2::splat(1000.0);
        Zone {
            half_size: Vec2::splat(50.0),
            colliders,
            start: Vec2::from(start),
            facing: Vec2::X,
            item: far,
            door: far,
            campfire: far,
            terrace: far,
            door_colliders: Vec::new(),
            item_nodes: Default::default(),
            door_nodes: Default::default(),
        }
    }

    /// A zone crossed by a wall along x = 10, but for a door in it at the origin of z, which
    /// its own collider closes; the key lies on the way, at x = 5.
    fn walled(item: [f32; 2]) -> Zone {
        let mut zone = zone(
            [0.0, 0.0],
            vec![
                turned_box([10.0, 0.0], [0.6, 2.0], 0.0),
                turned_box([10.0, 21.0], [0.6, 40.0], 0.0),
                turned_box([10.0, -21.0], [0.6, 40.0], 0.0),
            ],
        );
        zone.item = Vec2::from(item);
        zone.door = Vec2::new(10.0, 0.0);
        zone.door_colliders = vec![0];
        zone.campfire = Vec2::new(0.0, -10.0);
        zone
    }

    #[test]
    fn the_key_taken_on_the_way_opens_the_door() {
        let mut game = Game::new(walled([5.0, 0.0]));
        hold(&mut game, Vec2::X, 2.0);
        assert!(game.has_item() && !game.door_open());
        hold(&mut game, Vec2::X, 4.0);
        assert!(game.door_open());
        assert_eq!(game.colliders().len(), 2);
        assert!(game.hero().position.x > 12.0, "{}", game.hero().position);
    }

    #[test]
    fn without_the_key_the_door_stops_the_hero() {
        let mut game = Game::new(walled([0.0, 30.0]));
        hold(&mut game, Vec2::X, 6.0);
        assert!(!game.has_item() && !game.door_open());
        let x = game.hero().position.x;
        assert!((x - (9.7 - HERO_RADIUS)).abs() < 1e-4, "{x}");
    }

    #[test]
    fn a_restart_puts_back_the_key_and_closes_the_door() {
        let mut game = Game::new(walled([5.0, 0.0]));
        hold(&mut game, Vec2::X, 6.0);
        assert!(game.door_open());
        game.restart();
        assert_eq!(game.hero(), start(game.zone()));
        assert!(!game.has_item() && !game.door_open());
        assert_eq!(game.colliders(), game.zone().colliders.as_slice());
    }

    #[test]
    fn resting_by_the_campfire_turns_the_day_to_night_and_back() {
        let mut game = Game::new(walled([0.0, 30.0]));
        assert!(!game.can_rest());
        assert!(
            !game.rest() && !game.night(),
            "rested far from the campfire"
        );
        hold(&mut game, -Vec2::Y, 2.8);
        assert!(game.can_rest(), "{}", game.hero().position);
        assert!(game.rest() && game.night());
        assert!(game.rest() && !game.night());
        assert!(game.rest() && game.night());
        // A new game starts by day again, as this one did.
        game.restart();
        assert!(!game.night());
    }

    #[test]
    fn a_game_started_at_night_starts_again_at_night() {
        let mut game = Game::new(walled([0.0, 30.0])).starting_at_night();
        assert!(game.night());
        hold(&mut game, -Vec2::Y, 2.8);
        assert!(game.rest() && !game.night());
        game.restart();
        assert!(game.night());
    }

    fn turned_box(center: [f32; 2], size: [f32; 2], yaw: f32) -> Collider {
        Collider {
            shape: Shape::Box {
                half_size: Vec2::from(size) / 2.0,
                yaw,
            },
            center: Vec2::from(center),
            height: 2.0,
        }
    }

    fn cylinder(center: [f32; 2], radius: f32) -> Collider {
        Collider {
            shape: Shape::Cylinder { radius },
            center: Vec2::from(center),
            height: 2.0,
        }
    }

    /// Plays `seconds` of holding `direction`.
    fn hold(game: &mut Game, direction: Vec2, seconds: f32) {
        for _ in 0..(seconds / STEP).round() as u32 {
            game.step(direction, STEP);
        }
    }

    /// How far the hero's center is from `collider`'s footprint, measured outside it: at least
    /// the hero's radius when they do not overlap.
    fn clearance(position: Vec2, collider: &Collider) -> f32 {
        match collider.shape {
            Shape::Box { half_size, yaw } => {
                let local = Vec2::from_angle(-yaw).rotate(position - collider.center);
                (local.abs() - half_size).max(Vec2::ZERO).length()
            }
            Shape::Cylinder { radius } => position.distance(collider.center) - radius,
        }
    }

    #[test]
    fn the_hero_moves_at_its_speed_and_a_short_direction_slows_it() {
        let mut game = Game::new(zone([0.0, 0.0], Vec::new()));
        hold(&mut game, Vec2::X, 1.0);
        let hero = game.hero().position;
        assert!((hero.x - HERO_SPEED).abs() < 1e-4, "{hero}");
        // A diagonal of length √2 moves at the same speed, a direction of length 0.5 at half.
        let mut game = Game::new(zone([0.0, 0.0], Vec::new()));
        hold(&mut game, Vec2::ONE, 1.0);
        assert!((game.hero().position.length() - HERO_SPEED).abs() < 1e-4);
        let mut game = Game::new(zone([0.0, 0.0], Vec::new()));
        hold(&mut game, Vec2::new(0.0, 0.5), 1.0);
        assert!((game.hero().position.y - HERO_SPEED / 2.0).abs() < 1e-4);
    }

    /// A wall turned by half a radian, 0.5 m thick, whose near face lies across the line from the
    /// origin along its first axis: the normal of that face is `normal`, its line along the wall
    /// `along`.
    fn turned_wall() -> (Collider, Vec2, Vec2) {
        let yaw = 0.5;
        let normal = Vec2::from_angle(yaw);
        let along = Vec2::new(-normal.y, normal.x);
        (turned_box([3.0, 0.0], [0.5, 10.0], yaw), normal, along)
    }

    #[test]
    fn a_turned_wall_stops_the_hero_at_its_radius() {
        let (wall, normal, _) = turned_wall();
        let face = Vec2::new(3.0, 0.0).dot(normal) - 0.25;
        let mut game = Game::new(zone([0.0, 0.0], vec![wall]));
        hold(&mut game, normal, 3.0);
        let hero = game.hero().position;
        assert!(
            (hero.dot(normal) - (face - HERO_RADIUS)).abs() < 1e-4,
            "{hero}"
        );
    }

    #[test]
    fn the_hero_slides_along_a_turned_wall_it_moves_into_at_an_angle() {
        let (wall, normal, along) = turned_wall();
        let face = Vec2::new(3.0, 0.0).dot(normal) - 0.25;
        let mut game = Game::new(zone([0.0, 0.0], vec![wall]));
        // +x meets the wall at an angle, and slides along it towards −along.
        hold(&mut game, Vec2::X, 3.0);
        let hero = game.hero().position;
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
        let mut game = Game::new(zone([0.0, 0.0], vec![pillar]));
        hold(&mut game, Vec2::X, 3.0);
        let hero = game.hero().position;
        assert!((hero.x - (3.0 - 0.5 - HERO_RADIUS)).abs() < 1e-4, "{hero}");
        // A little to its side, the hero slides round it and goes on, never inside it.
        let mut game = Game::new(zone([0.0, 0.3], vec![pillar]));
        for _ in 0..240 {
            game.step(Vec2::X, STEP);
            let clear = clearance(game.hero().position, &pillar);
            assert!(clear > HERO_RADIUS - 1e-3, "{}", game.hero().position);
        }
        assert!(game.hero().position.x > 4.0, "{}", game.hero().position);
    }

    #[test]
    fn the_hero_goes_round_a_turned_corner_without_crossing_it() {
        // A block turned by 0.4 rad, its corner towards the hero, who walks at it and along it.
        let block = turned_box([3.0, 3.0], [2.0, 2.0], 0.4);
        for direction in [Vec2::ONE, Vec2::new(1.0, 0.8), Vec2::new(0.8, 1.0)] {
            let mut game = Game::new(zone([0.0, 0.0], vec![block]));
            for _ in 0..300 {
                game.step(direction, STEP);
                let clear = clearance(game.hero().position, &block);
                assert!(
                    clear > HERO_RADIUS - 1e-3,
                    "{direction}: {} overlaps the block",
                    game.hero().position
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
            let mut game = Game::new(zone([0.0, 0.0], vec![collider]));
            for _ in 0..30 {
                game.step(Vec2::X, 0.1);
            }
            assert!(
                (game.hero().position - collider.center).dot(across) < 0.0,
                "{collider:?}: {}",
                game.hero().position
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
            let mut game = Game::new(zone([start, 0.0], colliders.clone()));
            hold(&mut game, Vec2::Y, 4.0);
            assert!(
                game.hero().position.y < 3.0,
                "from x = {start}: {}",
                game.hero().position
            );
        }
        // With the pillar 0.5 m further, the gap is 1.05 m, and the hero goes through.
        let colliders = vec![colliders[0], cylinder([1.1, 3.0], 0.5)];
        let mut game = Game::new(zone([0.3, 0.0], colliders));
        hold(&mut game, Vec2::Y, 4.0);
        assert!(game.hero().position.y > 5.0, "{}", game.hero().position);
    }

    #[test]
    fn the_edge_of_the_zone_stops_the_hero() {
        let mut game = Game::new(Zone {
            half_size: Vec2::splat(5.0),
            ..zone([0.0, 0.0], Vec::new())
        });
        hold(&mut game, Vec2::new(1.0, -0.5), 10.0);
        let hero = game.hero().position;
        assert!((hero.x - (5.0 - HERO_RADIUS)).abs() < 1e-5, "{hero}");
        assert!((hero.y + (5.0 - HERO_RADIUS)).abs() < 1e-5, "{hero}");
    }

    #[test]
    fn the_hero_turns_to_face_where_it_moves_the_shorter_way() {
        let mut game = Game::new(zone([0.0, 0.0], Vec::new()));
        // Facing +x, asked to go to −z: a quarter turn, which takes a little time.
        game.step(-Vec2::Y, STEP);
        let heading = game.hero().heading;
        assert!(heading < 0.0 && heading > -PI / 2.0, "{heading}");
        hold(&mut game, -Vec2::Y, 0.5);
        assert!(game.hero().facing().abs_diff_eq(-Vec2::Y, 1e-5));
        // Standing still keeps the heading.
        hold(&mut game, Vec2::ZERO, 0.5);
        assert!(game.hero().facing().abs_diff_eq(-Vec2::Y, 1e-5));
        // From just below π to just above −π is a small turn, not a whole one.
        assert!((shortest_turn(3.0, -3.0) - (TAU - 6.0)).abs() < 1e-5);
        assert_eq!(turn_towards(0.0, 1.0, 0.25), 0.25);
    }

    #[test]
    fn the_hero_is_drawn_between_its_last_two_steps_and_a_restart_starts_again() {
        let mut game = Game::new(zone([1.0, 2.0], Vec::new()));
        game.step(Vec2::X, 0.1);
        let middle = game.hero_between(0.5);
        assert!(
            (middle.position.x - (1.0 + HERO_SPEED * 0.05)).abs() < 1e-4,
            "{middle:?}"
        );
        hold(&mut game, Vec2::Y, 1.0);
        game.restart();
        let first = Game::new(zone([1.0, 2.0], Vec::new()));
        assert_eq!(game.hero(), first.hero());
        assert_eq!(game.hero_between(0.3), first.hero());
        assert_eq!(game.zone(), first.zone());
    }

    /// Walks the hero to each of `waypoints` in turn, (x, z), straight, sliding along what it
    /// meets; fails if one is not reached within a minute.
    fn walk(game: &mut Game, waypoints: &[[f32; 2]]) {
        for &waypoint in waypoints {
            let waypoint = Vec2::from(waypoint);
            let mut steps = 0;
            while game.hero().position.distance(waypoint) > 0.3 {
                game.step((waypoint - game.hero().position).normalize(), STEP);
                steps += 1;
                assert!(
                    steps < 3600,
                    "stuck at {} on the way to {waypoint}",
                    game.hero().position
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
        let (_, zone) = crate::zone::cooked::zone();
        let mut game = Game::new(zone);
        walk(&mut game, &TO_THE_CHAPEL[..7]);
        // The branch to the campfire, and back.
        walk(&mut game, &[[-5.0, -4.5], [-9.0, -4.0], [-11.0, -4.0]]);
        assert!(game.can_rest(), "{}", game.hero().position);
        walk(&mut game, &[[-5.0, -4.5], [-0.4, -3.5]]);
        walk(&mut game, &TO_THE_CHAPEL[7..]);
        assert!(!game.has_item());
        // Up the nave to the key, then to the door, which it opens, and onto the terrace.
        walk(&mut game, &[[0.0, -27.0], [0.0, -38.0], [0.8, -41.6]]);
        assert!(game.has_item() && !game.door_open());
        walk(&mut game, &[[0.0, -43.4]]);
        assert!(game.door_open());
        walk(&mut game, &[[0.0, -46.0], [0.0, -49.0]]);
        assert!(game.hero().position.distance(game.zone().terrace) < 1.0);
        // A new game starts from the same state as the first.
        game.restart();
        assert_eq!(game.hero(), start(game.zone()));
        assert!(!game.has_item() && !game.door_open());
        assert_eq!(game.colliders(), game.zone().colliders.as_slice());
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_door_stops_the_hero_before_the_key_is_found() {
        let (_, zone) = crate::zone::cooked::zone();
        let door = zone.door;
        let mut game = Game::new(zone);
        walk(&mut game, &TO_THE_CHAPEL);
        // Up the west side of the nave, away from the key, to the door, and on against it.
        walk(&mut game, &[[-0.8, -38.0], [-0.8, -43.3]]);
        hold(&mut game, -Vec2::Y, 3.0);
        assert!(!game.has_item() && !game.door_open());
        assert!(
            game.hero().position.y > door.y + 0.4,
            "{}",
            game.hero().position
        );
    }

    #[test]
    #[ignore = "needs the cooked zone (scripts/check.sh runs it)"]
    fn the_hero_stays_in_the_cooked_zone_and_out_of_its_colliders() {
        let (_, zone) = crate::zone::cooked::zone();
        let mut game = Game::new(zone);
        let colliders = game.zone().colliders.clone();
        for direction in [
            -Vec2::Y,
            Vec2::X,
            -Vec2::X,
            Vec2::new(0.3, -1.0),
            Vec2::new(-0.6, -1.0),
            Vec2::Y,
        ] {
            for _ in 0..(12.0 / STEP) as u32 {
                game.step(direction, STEP);
                for collider in &colliders {
                    let clear = clearance(game.hero().position, collider);
                    assert!(
                        clear > HERO_RADIUS - 1e-3,
                        "{direction}: {} overlaps {collider:?}",
                        game.hero().position
                    );
                }
            }
        }
    }
}
