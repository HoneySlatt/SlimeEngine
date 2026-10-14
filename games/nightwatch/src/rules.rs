//! The rules of the game, without the GPU: where the player and the guards are, how they move
//! through the courtyard, what the lights see, and how a game is won, lost and started again. The
//! window, the input and the drawing are elsewhere, so that the rules can be tested alone and
//! played in scripted games. They hold no GPU object: a new game creates none.

use std::f32::consts::{PI, TAU};

use slime_render::glam::Vec2;

use crate::courtyard::{Block, Courtyard, Patrol};
use crate::sight::Cone;

/// The player's radius, in metres: it moves as a circle on the ground.
pub const PLAYER_RADIUS: f32 = 0.35;
/// How fast the player moves, in metres per second.
pub const PLAYER_SPEED: f32 = 3.0;
/// A guard's radius: a guard that comes this near the player, plus the player's radius, catches it
/// whichever way it looks.
pub const GUARD_RADIUS: f32 = 0.3;
/// How fast a guard walks, in metres per second, slower than the player.
pub const GUARD_SPEED: f32 = 1.4;
/// How long a guard stops at each point of its round, looking round to the next.
const GUARD_PAUSE: f32 = 1.2;
/// How fast a guard turns while it stops, in radians per second.
const GUARD_TURN_SPEED: f32 = 2.5;
/// A guard's flashlight: the angle from its axis to its edge, and how far it sees.
pub const FLASHLIGHT_HALF_ANGLE: f32 = 0.42;
pub const FLASHLIGHT_RANGE: f32 = 6.5;
/// A floodlight's cone.
pub const FLOODLIGHT_HALF_ANGLE: f32 = 0.35;
pub const FLOODLIGHT_RANGE: f32 = 9.0;
/// How near a switch's center the player's center must be to use it.
pub const SWITCH_REACH: f32 = 1.0;
/// How near the exit the player's center must be to leave.
const EXIT_REACH: f32 = 0.6;
/// How long a lost game, and a won one, show before a new game starts, in seconds.
pub const CAUGHT_PAUSE: f32 = 1.5;
pub const WON_PAUSE: f32 = 3.0;

/// What sees the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watcher {
    /// The guard of that index.
    Guard(usize),
    /// The floodlight of that index.
    Floodlight(usize),
}

/// Where a game stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    /// The player moves.
    Playing,
    /// The player was seen, `time` seconds ago; a new game starts after [`CAUGHT_PAUSE`].
    Caught { by: Watcher, time: f32 },
    /// The player left through the gate, `time` seconds ago; a new game starts after
    /// [`WON_PAUSE`].
    Won { time: f32 },
}

/// A guard on its round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guard {
    /// Where it stands, on the ground.
    pub position: Vec2,
    /// Which way it looks, in radians from +x towards +z.
    pub heading: f32,
    /// The index, in its round, of the point it walks to.
    next: usize,
    /// How long it still stops before walking on, in seconds.
    pause: f32,
}

impl Guard {
    /// A guard at the start of `patrol`, looking at its next point.
    fn new(patrol: &Patrol) -> Self {
        let points = &patrol.waypoints;
        let next = 1 % points.len();
        Self {
            position: points[0],
            heading: angle_of(points[next] - points[0]),
            next,
            pause: 0.0,
        }
    }

    /// Which way it looks, on the ground, of length 1.
    pub fn facing(&self) -> Vec2 {
        Vec2::from_angle(self.heading)
    }

    /// What its flashlight lights.
    pub fn cone(&self) -> Cone {
        Cone {
            origin: self.position,
            direction: self.facing(),
            half_angle: FLASHLIGHT_HALF_ANGLE,
            range: FLASHLIGHT_RANGE,
        }
    }

    /// Walks `step` seconds of `patrol`: to its next point, where it stops and turns to the one
    /// after.
    fn walk(&mut self, patrol: &Patrol, step: f32) {
        let points = &patrol.waypoints;
        let mut time = step;
        while time > 0.0 {
            let target = points[self.next];
            if self.pause > 0.0 {
                let used = self.pause.min(time);
                self.pause -= used;
                time -= used;
                self.heading = turn_towards(
                    self.heading,
                    angle_of(target - self.position),
                    GUARD_TURN_SPEED * used,
                );
                continue;
            }
            let offset = target - self.position;
            let distance = offset.length();
            let reach = GUARD_SPEED * time;
            if reach < distance {
                self.position += offset * (reach / distance);
                self.heading = angle_of(offset);
                return;
            }
            self.position = target;
            time -= distance / GUARD_SPEED;
            self.next = (self.next + 1) % points.len();
            self.pause = GUARD_PAUSE;
        }
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

/// A game in progress.
pub struct Game {
    courtyard: Courtyard,
    /// The player's place on the ground: x and z, in metres.
    player: Vec2,
    guards: Vec<Guard>,
    /// Where the player and the guards were before the last step, so that what moves is drawn
    /// between its last two steps.
    previous_player: Vec2,
    previous_guards: Vec<Guard>,
    /// Whether each floodlight is on.
    floodlights_on: Vec<bool>,
    state: State,
}

impl Game {
    /// A new game in `courtyard`: the player at its entrance, the guards at the start of their
    /// rounds, every floodlight on.
    pub fn new(courtyard: Courtyard) -> Self {
        let mut game = Self {
            courtyard,
            player: Vec2::ZERO,
            guards: Vec::new(),
            previous_player: Vec2::ZERO,
            previous_guards: Vec::new(),
            floodlights_on: Vec::new(),
            state: State::Playing,
        };
        game.restart();
        game
    }

    /// Starts a new game in the same courtyard, as [`Game::new`] does.
    pub fn restart(&mut self) {
        self.player = self.courtyard.entrance;
        self.guards = self.courtyard.patrols.iter().map(Guard::new).collect();
        self.floodlights_on = vec![true; self.courtyard.floodlights.len()];
        self.state = State::Playing;
        self.previous_player = self.player;
        self.previous_guards.clone_from(&self.guards);
    }

    /// Where the player is drawn `alpha` of the way, from 0 to 1, between where the last step
    /// started and where it ended: the share of a step that the frame comes after it
    /// (`Context::fixed_step_alpha`), so that it moves smoothly at any frame rate.
    pub fn player_between(&self, alpha: f32) -> Vec2 {
        self.previous_player.lerp(self.player, alpha)
    }

    /// The guards as they are drawn `alpha` of the way between their last two steps, as for
    /// [`Game::player_between`].
    pub fn guards_between(&self, alpha: f32) -> impl Iterator<Item = Guard> + '_ {
        self.previous_guards
            .iter()
            .zip(&self.guards)
            .map(move |(before, after)| Guard {
                position: before.position.lerp(after.position, alpha),
                heading: before.heading + shortest_turn(before.heading, after.heading) * alpha,
                ..*after
            })
    }

    pub fn courtyard(&self) -> &Courtyard {
        &self.courtyard
    }

    /// The player's place on the ground after the last step: x and z, in metres. The game draws
    /// [`Game::player_between`] instead; the tests read this.
    #[cfg(test)]
    pub fn player(&self) -> Vec2 {
        self.player
    }

    /// The guards after the last step; the game draws [`Game::guards_between`] instead.
    #[cfg(test)]
    pub fn guards(&self) -> &[Guard] {
        &self.guards
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// Whether the floodlight of index `index` is on.
    pub fn floodlight_on(&self, index: usize) -> bool {
        self.floodlights_on[index]
    }

    /// What the floodlight of index `index` lights, when it is on.
    pub fn floodlight_cone(&self, index: usize) -> Cone {
        let floodlight = &self.courtyard.floodlights[index];
        Cone {
            origin: floodlight.position,
            direction: floodlight.direction,
            half_angle: FLOODLIGHT_HALF_ANGLE,
            range: FLOODLIGHT_RANGE,
        }
    }

    /// The index of the switch within the player's reach whose floodlight is still on, if any.
    pub fn switch_in_reach(&self) -> Option<usize> {
        let switches = &self.courtyard.switches;
        (0..switches.len()).find(|&index| {
            let switch = &switches[index];
            self.state == State::Playing
                && self.floodlights_on[switch.floodlight]
                && self.player.distance(switch.position) <= SWITCH_REACH
        })
    }

    /// Uses the switch within the player's reach, if any: its floodlight goes off for the rest of
    /// the game.
    pub fn interact(&mut self) {
        if let Some(index) = self.switch_in_reach() {
            let floodlight = self.courtyard.switches[index].floodlight;
            self.floodlights_on[floodlight] = false;
        }
    }

    /// Plays `step` seconds. While the game is on, the player moves towards `direction`, on the
    /// ground, at the share of its speed that the direction's length gives, at most 1, stopped by
    /// the blocks and sliding along them; the guards walk their rounds; the player wins at the
    /// exit and loses when a light sees it or a guard comes too near. A game won or lost shows
    /// for a moment, then a new game starts.
    pub fn step(&mut self, direction: Vec2, step: f32) {
        self.previous_player = self.player;
        self.previous_guards.clone_from(&self.guards);
        match &mut self.state {
            State::Playing => {}
            State::Caught { time, .. } => {
                *time += step;
                if *time >= CAUGHT_PAUSE {
                    self.restart();
                }
                return;
            }
            State::Won { time } => {
                *time += step;
                if *time >= WON_PAUSE {
                    self.restart();
                }
                return;
            }
        }
        self.move_player(direction, step);
        for (guard, patrol) in self.guards.iter_mut().zip(&self.courtyard.patrols) {
            guard.walk(patrol, step);
        }
        if self.player.distance(self.courtyard.exit) <= EXIT_REACH {
            self.state = State::Won { time: 0.0 };
        } else if let Some(by) = self.watcher() {
            self.state = State::Caught { by, time: 0.0 };
        }
    }

    /// What sees the player now, if anything.
    fn watcher(&self) -> Option<Watcher> {
        let blocks = &self.courtyard.blocks;
        let guard = self.guards.iter().position(|guard| {
            guard.position.distance(self.player) < GUARD_RADIUS + PLAYER_RADIUS
                || guard.cone().sees(self.player, blocks)
        });
        let floodlight = || {
            (0..self.floodlights_on.len()).find(|&index| {
                self.floodlights_on[index] && self.floodlight_cone(index).sees(self.player, blocks)
            })
        };
        guard
            .map(Watcher::Guard)
            .or_else(|| floodlight().map(Watcher::Floodlight))
    }

    fn move_player(&mut self, direction: Vec2, step: f32) {
        let travel = direction.clamp_length_max(1.0) * PLAYER_SPEED * step;
        // In moves of at most half the radius, so that no move carries the player past a corner.
        let parts = (travel.length() / (PLAYER_RADIUS / 2.0)).ceil().max(1.0);
        let part = travel / parts;
        for _ in 0..parts as u32 {
            self.player = push_out(self.player + part, PLAYER_RADIUS, &self.courtyard.blocks);
        }
    }
}

/// `position` moved out of every block that the circle of `radius` around it overlaps: along the
/// line from the block's nearest point, which leaves the part of a move along the block's side, so
/// that the player slides.
fn push_out(mut position: Vec2, radius: f32, blocks: &[Block]) -> Vec2 {
    // Pushed out of one block, the circle can enter another, in a corner between two: a few rounds
    // settle it.
    for _ in 0..4 {
        let mut pushed = false;
        for block in blocks {
            let offset = position - block.center;
            let away = offset - offset.clamp(-block.half_size, block.half_size);
            let distance = away.length();
            if distance >= radius {
                continue;
            }
            pushed = true;
            if distance > 1e-6 {
                position += away * ((radius - distance) / distance);
            } else {
                // The center is inside the block: out through its nearest side.
                let depth = block.half_size - offset.abs();
                if depth.x < depth.y {
                    position.x = block.center.x + offset.x.signum() * (block.half_size.x + radius);
                } else {
                    position.y = block.center.y + offset.y.signum() * (block.half_size.y + radius);
                }
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
    use crate::courtyard::{BlockKind, Floodlight, Switch};

    /// The fixed step the game runs at: 1/60 s.
    const STEP: f32 = 1.0 / 60.0;

    /// A courtyard of the given blocks, without guards or lights, with the player starting at
    /// `entrance` and the exit far away.
    fn courtyard(entrance: [f32; 2], blocks: &[([f32; 2], [f32; 2])]) -> Courtyard {
        Courtyard {
            half_size: Vec2::splat(50.0),
            blocks: blocks
                .iter()
                .map(|&(center, size)| Block {
                    kind: BlockKind::Wall,
                    center: Vec2::from(center),
                    half_size: Vec2::from(size) / 2.0,
                    height: 2.0,
                })
                .collect(),
            patrols: Vec::new(),
            floodlights: Vec::new(),
            switches: Vec::new(),
            entrance: Vec2::from(entrance),
            exit: Vec2::splat(1000.0),
        }
    }

    /// Plays `seconds` of holding `direction`.
    fn hold(game: &mut Game, direction: Vec2, seconds: f32) {
        for _ in 0..(seconds / STEP).round() as u32 {
            game.step(direction, STEP);
        }
    }

    #[test]
    fn the_player_moves_at_its_speed_and_a_short_direction_slows_it() {
        let mut game = Game::new(courtyard([0.0, 0.0], &[]));
        hold(&mut game, Vec2::X, 1.0);
        assert!(
            (game.player().x - PLAYER_SPEED).abs() < 1e-4,
            "{}",
            game.player()
        );
        // A diagonal of length √2 moves at the same speed, a direction of length 0.5 at half.
        let mut game = Game::new(courtyard([0.0, 0.0], &[]));
        hold(&mut game, Vec2::ONE, 1.0);
        assert!((game.player().length() - PLAYER_SPEED).abs() < 1e-4);
        let mut game = Game::new(courtyard([0.0, 0.0], &[]));
        hold(&mut game, Vec2::new(0.0, 0.5), 1.0);
        assert!((game.player().y - PLAYER_SPEED / 2.0).abs() < 1e-4);
    }

    #[test]
    fn a_wall_stops_the_player_at_its_radius() {
        // A wall whose west side is at x = 2.
        let mut game = Game::new(courtyard([0.0, 0.0], &[([2.5, 0.0], [1.0, 10.0])]));
        hold(&mut game, Vec2::X, 3.0);
        assert!(
            (game.player().x - (2.0 - PLAYER_RADIUS)).abs() < 1e-4,
            "{}",
            game.player()
        );
    }

    #[test]
    fn the_player_slides_along_a_wall_it_moves_into_at_an_angle() {
        let mut game = Game::new(courtyard([0.0, 0.0], &[([2.5, 0.0], [1.0, 100.0])]));
        hold(&mut game, Vec2::new(1.0, 1.0).normalize(), 2.0);
        let player = game.player();
        assert!((player.x - (2.0 - PLAYER_RADIUS)).abs() < 1e-4, "{player}");
        // Along the wall at the share of the speed that the direction gives along it.
        let along = PLAYER_SPEED * 2.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert!((player.y - along).abs() < 0.05, "{player}");
    }

    #[test]
    fn the_player_goes_round_a_corner_without_crossing_it() {
        // A block from (1, 1) to (3, 3); the player starts south-west of it and moves north-east,
        // straight at its corner, then along its sides.
        let block = ([2.0, 2.0], [2.0, 2.0]);
        for direction in [Vec2::ONE, Vec2::new(1.0, 0.9), Vec2::new(0.9, 1.0)] {
            let mut game = Game::new(courtyard([0.0, 0.0], &[block]));
            for _ in 0..240 {
                game.step(direction, STEP);
                let offset = (game.player() - Vec2::splat(2.0)).abs() - Vec2::ONE;
                let distance = offset.max(Vec2::ZERO).length();
                assert!(
                    distance > PLAYER_RADIUS - 1e-3,
                    "{direction}: {} overlaps the block",
                    game.player()
                );
            }
        }
    }

    #[test]
    fn the_player_does_not_pass_a_gap_narrower_than_itself() {
        // Two blocks whose facing sides are 0.5 apart, less than the player's diameter of 0.7.
        let blocks = [([-1.25, 2.0], [2.0, 1.0]), ([1.25, 2.0], [2.0, 1.0])];
        for start in [0.0, 0.1, -0.2] {
            let mut game = Game::new(courtyard([start, 0.0], &blocks));
            hold(&mut game, Vec2::Y, 3.0);
            assert!(game.player().y < 1.5, "from x = {start}: {}", game.player());
        }
        // A gap of 1 m lets it through.
        let blocks = [([-1.5, 2.0], [2.0, 1.0]), ([1.5, 2.0], [2.0, 1.0])];
        let mut game = Game::new(courtyard([0.0, 0.0], &blocks));
        hold(&mut game, Vec2::Y, 3.0);
        assert!(game.player().y > 3.0, "{}", game.player());
    }

    #[test]
    fn a_fast_move_does_not_cross_a_thin_wall() {
        // A step of a tenth of a second at full speed is 0.3 m, a wall 0.1 m thick.
        let mut game = Game::new(courtyard([0.0, 0.0], &[([1.0, 0.0], [0.1, 10.0])]));
        for _ in 0..30 {
            game.step(Vec2::X, 0.1);
        }
        assert!(game.player().x < 1.0, "{}", game.player());
    }

    #[test]
    fn the_player_stays_inside_the_nightwatch_courtyard() {
        // Without its guards and lights, which would catch the player and start again.
        let mut game = Game::new(Courtyard {
            patrols: Vec::new(),
            floodlights: Vec::new(),
            switches: Vec::new(),
            ..Courtyard::nightwatch()
        });
        let half_size = game.courtyard().half_size;
        for direction in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y, Vec2::ONE, -Vec2::ONE] {
            hold(&mut game, direction, 15.0);
            let inside = half_size - Vec2::splat(PLAYER_RADIUS - 1e-3);
            assert!(
                game.player().abs().cmple(inside).all(),
                "{direction}: {}",
                game.player()
            );
        }
    }

    /// A guard walking along +x from the origin, far.
    fn walking_east() -> Patrol {
        Patrol {
            waypoints: vec![Vec2::ZERO, Vec2::new(50.0, 0.0)],
        }
    }

    #[test]
    fn a_guard_walks_its_round_and_stops_at_each_point() {
        let mut game = Game::new(Courtyard {
            patrols: vec![Patrol {
                waypoints: vec![Vec2::ZERO, Vec2::new(2.0 * GUARD_SPEED, 0.0)],
            }],
            ..courtyard([0.0, 40.0], &[])
        });
        hold(&mut game, Vec2::ZERO, 1.0);
        let guard = game.guards()[0];
        assert!((guard.position.x - GUARD_SPEED).abs() < 1e-3, "{guard:?}");
        assert!(guard.facing().abs_diff_eq(Vec2::X, 1e-5));
        // At the far point after 2 s, it stops there, turning back.
        hold(&mut game, Vec2::ZERO, 1.0 + GUARD_PAUSE / 2.0);
        let guard = game.guards()[0];
        assert!(
            (guard.position.x - 2.0 * GUARD_SPEED).abs() < 1e-3,
            "{guard:?}"
        );
        hold(&mut game, Vec2::ZERO, GUARD_PAUSE / 2.0 + 1.0);
        let guard = game.guards()[0];
        assert!((guard.position.x - GUARD_SPEED).abs() < 1e-3, "{guard:?}");
        assert!(guard.facing().abs_diff_eq(-Vec2::X, 1e-5), "{guard:?}");
    }

    #[test]
    fn a_guard_sees_the_player_in_its_light_and_a_new_game_starts_after_a_pause() {
        let mut game = Game::new(Courtyard {
            patrols: vec![walking_east()],
            ..courtyard([4.0, 0.5], &[])
        });
        game.step(Vec2::ZERO, STEP);
        assert_eq!(
            game.state(),
            State::Caught {
                by: Watcher::Guard(0),
                time: 0.0
            }
        );
        // The player does not move while the lost game shows.
        hold(&mut game, Vec2::X, CAUGHT_PAUSE - 0.1);
        assert!(matches!(game.state(), State::Caught { .. }));
        assert_eq!(game.player(), Vec2::new(4.0, 0.5));
        // The step that ends the pause starts the new game, as it began.
        for _ in 0..12 {
            game.step(Vec2::ZERO, STEP);
            if game.state() == State::Playing {
                break;
            }
        }
        assert_eq!(game.state(), State::Playing);
        assert_eq!(game.player(), Vec2::new(4.0, 0.5));
        assert_eq!(
            game.guards()[0].position,
            Vec2::ZERO,
            "the guards start again"
        );
    }

    #[test]
    fn a_crate_hides_the_player_and_a_guard_does_not_see_behind_it() {
        // The player behind a crate at x = 2, in the guard's light.
        let mut game = Game::new(Courtyard {
            patrols: vec![walking_east()],
            ..courtyard([3.2, 0.0], &[([2.0, 0.0], [1.0, 1.0])])
        });
        hold(&mut game, Vec2::ZERO, 0.5);
        assert_eq!(game.state(), State::Playing);
        // Beside it, the player is seen; and behind the guard it is not.
        let mut game = Game::new(Courtyard {
            patrols: vec![walking_east()],
            ..courtyard([4.0, 1.5], &[([2.0, 0.0], [1.0, 1.0])])
        });
        game.step(Vec2::ZERO, STEP);
        assert!(matches!(game.state(), State::Caught { .. }));
        let mut game = Game::new(Courtyard {
            patrols: vec![walking_east()],
            ..courtyard([-3.0, 0.0], &[])
        });
        hold(&mut game, Vec2::ZERO, 0.5);
        assert_eq!(game.state(), State::Playing);
    }

    #[test]
    fn a_guard_that_comes_too_near_catches_the_player_whichever_way_it_looks() {
        // Right behind the guard, which walks away from it.
        let mut game = Game::new(Courtyard {
            patrols: vec![walking_east()],
            ..courtyard([-0.5, 0.0], &[])
        });
        game.step(Vec2::ZERO, STEP);
        assert!(matches!(
            game.state(),
            State::Caught {
                by: Watcher::Guard(0),
                ..
            }
        ));
    }

    #[test]
    fn a_floodlight_sees_the_player_until_its_switch_turns_it_off() {
        let lit = || Courtyard {
            floodlights: vec![Floodlight {
                position: Vec2::ZERO,
                direction: Vec2::X,
            }],
            switches: vec![Switch {
                position: Vec2::new(0.0, 2.0),
                floodlight: 0,
            }],
            ..courtyard([0.0, 1.5], &[])
        };
        // Walking into the light is seen.
        let mut game = Game::new(lit());
        hold(&mut game, Vec2::new(1.0, -0.4), 1.5);
        assert!(matches!(
            game.state(),
            State::Caught {
                by: Watcher::Floodlight(0),
                ..
            }
        ));
        // Out of reach of the switch, using it does nothing; within reach, it turns the light off.
        let mut game = Game::new(Courtyard {
            entrance: Vec2::new(0.0, 4.0),
            ..lit()
        });
        assert_eq!(game.switch_in_reach(), None);
        game.interact();
        assert!(game.floodlight_on(0));
        let mut game = Game::new(lit());
        assert_eq!(game.switch_in_reach(), Some(0));
        game.interact();
        assert!(!game.floodlight_on(0));
        assert_eq!(game.switch_in_reach(), None, "a switch is used once");
        hold(&mut game, Vec2::new(1.0, -0.4), 1.5);
        assert_eq!(game.state(), State::Playing);
    }

    #[test]
    fn reaching_the_exit_wins_and_a_new_game_starts_after_a_pause() {
        let mut game = Game::new(Courtyard {
            exit: Vec2::new(3.0, 0.0),
            ..courtyard([0.0, 0.0], &[])
        });
        hold(&mut game, Vec2::X, 1.0);
        assert!(
            matches!(game.state(), State::Won { .. }),
            "{:?}",
            game.state()
        );
        hold(&mut game, Vec2::ZERO, WON_PAUSE);
        assert_eq!(game.state(), State::Playing);
        assert_eq!(game.player(), Vec2::ZERO);
    }

    /// What a scripted game does next.
    enum Do {
        /// Walk straight to this point.
        Go([f32; 2]),
        /// Stand still until the game has run this many seconds.
        WaitUntil(f32),
        /// Use the switch within reach.
        Use,
    }

    /// Plays `script` from the start of `game`, and returns the state it ends in, and when.
    fn play(game: &mut Game, script: &[Do]) -> (State, f32) {
        let mut time = 0.0;
        for action in script {
            match *action {
                Do::Go(target) => {
                    let target = Vec2::from(target);
                    for _ in 0..1200 {
                        let offset = target - game.player();
                        if offset.length() < 0.05 || game.state() != State::Playing {
                            break;
                        }
                        // The last step slows down so as not to overshoot.
                        let direction = offset / (PLAYER_SPEED * STEP).max(offset.length());
                        game.step(direction, STEP);
                        time += STEP;
                    }
                }
                Do::WaitUntil(until) => {
                    while time < until && game.state() == State::Playing {
                        game.step(Vec2::ZERO, STEP);
                        time += STEP;
                    }
                }
                Do::Use => game.interact(),
            }
            if game.state() != State::Playing {
                break;
            }
        }
        (game.state(), time)
    }

    /// A way through Nightwatch's courtyard: wait for the west part's guard to walk north, slip
    /// behind it to the opening, cross the middle part along its east side while its guard is on
    /// the far side, follow the east part's guard along the north wall to the switch, and once its
    /// floodlight is off, follow the guard south to the gate.
    const WAY_THROUGH: &[Do] = &[
        Do::WaitUntil(10.5),
        Do::Go([-9.0, 6.5]),
        Do::Go([-3.0, 6.5]),
        Do::Go([3.3, 6.5]),
        Do::Go([3.3, -4.0]),
        Do::WaitUntil(23.5),
        Do::Go([4.8, -7.2]),
        Do::WaitUntil(25.6),
        Do::Go([7.6, -7.2]),
        Do::Use,
        Do::WaitUntil(29.5),
        Do::Go([10.8, -4.0]),
        Do::WaitUntil(32.8),
        Do::Go([11.2, 0.0]),
    ];

    #[test]
    fn a_scripted_game_crosses_the_courtyard_to_the_gate() {
        let mut game = Game::new(Courtyard::nightwatch());
        let (state, time) = play(&mut game, WAY_THROUGH);
        assert!(
            matches!(state, State::Won { .. }),
            "{state:?} after {time} s, at {}",
            game.player()
        );
    }

    #[test]
    fn a_player_who_walks_straight_for_the_way_out_is_seen() {
        // Without waiting for the west part's guard to turn its back.
        let mut game = Game::new(Courtyard::nightwatch());
        let (state, _) = play(&mut game, &WAY_THROUGH[1..]);
        assert!(matches!(state, State::Caught { .. }), "{state:?}");
    }

    #[test]
    fn a_new_game_starts_from_the_state_of_the_first() {
        let first = Game::new(Courtyard::nightwatch());
        let mut game = Game::new(Courtyard::nightwatch());
        play(&mut game, &WAY_THROUGH[..10]);
        assert!(!game.floodlight_on(1), "the script used the switch");
        game.restart();
        assert_eq!(game.player(), first.player());
        assert_eq!(game.guards(), first.guards());
        assert_eq!(game.floodlights_on, first.floodlights_on);
        assert_eq!(game.state(), first.state());
    }

    #[test]
    fn what_moves_is_drawn_between_its_last_two_steps() {
        let mut game = Game::new(Courtyard {
            patrols: vec![walking_east()],
            ..courtyard([0.0, 40.0], &[])
        });
        game.step(Vec2::X, 0.1);
        let middle = game.player_between(0.5);
        assert!((middle.x - PLAYER_SPEED * 0.05).abs() < 1e-4, "{middle}");
        let guard = game.guards_between(0.5).next().expect("one guard");
        assert!(
            (guard.position.x - GUARD_SPEED * 0.05).abs() < 1e-4,
            "{guard:?}"
        );
        // A new game is drawn where it starts, whatever the share.
        game.restart();
        assert_eq!(game.player_between(0.3), game.courtyard().entrance);
    }

    #[test]
    fn a_turn_goes_the_shorter_way_across_a_half_turn() {
        // From just below π to just above −π is a small turn, not a whole one.
        let turn = shortest_turn(3.0, -3.0);
        assert!((turn - (TAU - 6.0)).abs() < 1e-5, "{turn}");
        assert!((shortest_turn(-3.0, 3.0) + (TAU - 6.0)).abs() < 1e-5);
        assert_eq!(turn_towards(0.0, 1.0, 0.25), 0.25);
    }
}
