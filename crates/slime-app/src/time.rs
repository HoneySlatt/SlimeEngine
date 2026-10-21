//! Frame timing: how much time each frame represents, and how many fixed steps it contains; and
//! the two times that a frame gives the phases that run in it, real time and game time ([`Time`]).
//!
//! The clocks are fed durations instead of reading the system clock, so the code that drives
//! them decides what time is: real time measured between frames, or a simulated, constant
//! duration that makes a run deterministic. Durations are kept as [`Duration`]s, whole
//! nanoseconds, so accumulating them never drifts the way floating-point seconds do.
//!
//! ```
//! use std::time::Duration;
//! use slime_app::time::{FixedStep, FrameClock};
//!
//! let mut clock = FrameClock::new(Duration::from_millis(250));
//! let mut fixed = FixedStep::new(Duration::from_millis(10), 8);
//!
//! // A frame that took 25 ms holds two 10 ms steps; 5 ms are carried over to the next frame.
//! let frame = clock.advance(Duration::from_millis(25));
//! assert_eq!(fixed.advance(frame.delta()), 2);
//! assert_eq!(fixed.alpha(), 0.5);
//! ```
//!
//! The reasoning behind these rules is recorded in `docs/adr/0009-frame-timing.md`.

use std::time::Duration;

/// The two times of a frame (`docs/adr/0107-the-execution-model.md`): *real time*, the time between
/// frames, which the camera, the UI and the dev overlay run on, and *game time*, the simulation's,
/// which pauses, slows down and steps. Each phase of a frame is given both, and reads the one it
/// lives on. Before the first frame, both are zero.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Time {
    pub(crate) real: FrameTime,
    pub(crate) game: GameTime,
}

impl Time {
    /// Real time: the frame's index, the time it represents and the sum of those times.
    pub fn real(&self) -> &FrameTime {
        &self.real
    }

    /// Game time: the fixed steps run, the time the frame added and the fraction of a step left.
    pub fn game(&self) -> &GameTime {
        &self.game
    }
}

/// The time of a game's simulation, at a point of a frame (`docs/adr/0107-the-execution-model.md`).
///
/// It advances by the frames' real time times a speed, 1 by default, and not while it is paused
/// ([`Context::set_speed`](crate::Context::set_speed), [`Context::pause`](crate::Context::pause));
/// the fixed steps that it holds keep their duration at every speed, so that a simulation gives the
/// same results at every speed. At a quarter of the speed, at 60 frames per second with steps of
/// 1/60 s, a step runs at every fourth frame, and what moves is drawn between its last two steps, a
/// quarter of a step further at each frame ([`GameTime::alpha`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GameTime {
    pub(crate) step: Duration,
    pub(crate) steps: u64,
    pub(crate) delta: Duration,
    pub(crate) alpha: f64,
}

impl GameTime {
    /// The duration of a fixed step, the same for the whole run
    /// ([`AppConfig::fixed_step`](crate::AppConfig::fixed_step)).
    pub fn step(&self) -> Duration {
        self.step
    }

    /// The fixed steps that ran before this point of the frame: in a fixed step, those before it,
    /// its index from 0; once the frame's steps ran, all of them.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// The game time that the [`steps`](Self::steps) covered: that many steps, saturating at
    /// [`Duration::MAX`].
    pub fn elapsed(&self) -> Duration {
        let nanos = self.step.as_nanos().saturating_mul(u128::from(self.steps));
        match u64::try_from(nanos / 1_000_000_000) {
            // Below a second: it fits.
            Ok(seconds) => Duration::new(seconds, (nanos % 1_000_000_000) as u32),
            Err(_) => Duration::MAX,
        }
    }

    /// The game time that the frame added, which its fixed steps ran from with what earlier frames
    /// left: its real time times the speed, rounded to the nanosecond; none while game time is
    /// paused.
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// The fraction of a fixed step that game time accumulated but did not run, at least 0.0 and
    /// below 1.0: the interpolation factor ([`FixedStep::alpha`]), at which the world draws what
    /// moved during the last step, between where it was before and after it
    /// (`slime_world::World::interpolated_transform`). It is set once per frame, after the frame's
    /// time is added and before its fixed steps run, and reads the same in its steps, its update
    /// and its extraction. While game time is paused, it stays as it was, a step asked for
    /// included: each step moves what is drawn by one step.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
}

/// Timing information about one frame, produced by [`FrameClock::advance`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameTime {
    index: u64,
    delta: Duration,
    elapsed: Duration,
}

impl FrameTime {
    /// The index of the frame: 0 for the first frame, then one more for each frame.
    pub fn index(&self) -> u64 {
        self.index
    }

    /// The time this frame represents: the time since the previous frame, limited to the clock's
    /// maximum delta.
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// The sum of the deltas of all frames so far, this one included.
    ///
    /// This is application time: when a delta is limited, the time cut off is not counted, so
    /// after a long pause it is behind wall-clock time.
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

/// Turns the time between frames into [`FrameTime`]s, limiting how much time one frame can
/// represent.
///
/// The limit protects the rest of the engine from huge steps: after the process was paused (by
/// a debugger, or a suspended laptop) or one frame took very long, a frame represents at most
/// the maximum delta instead of the whole gap.
#[derive(Clone, Debug)]
pub struct FrameClock {
    max_delta: Duration,
    next_index: u64,
    elapsed: Duration,
}

impl FrameClock {
    /// Creates a clock whose frames never represent more than `max_delta`.
    ///
    /// # Panics
    ///
    /// If `max_delta` is zero: time would never advance.
    pub fn new(max_delta: Duration) -> Self {
        assert!(
            !max_delta.is_zero(),
            "the maximum frame delta must not be zero"
        );
        Self {
            max_delta,
            next_index: 0,
            elapsed: Duration::ZERO,
        }
    }

    /// The longest time a single frame can represent.
    pub fn max_delta(&self) -> Duration {
        self.max_delta
    }

    /// Starts a new frame that represents `raw_delta`, limited to the maximum delta. The elapsed
    /// time saturates at [`Duration::MAX`] instead of overflowing.
    pub fn advance(&mut self, raw_delta: Duration) -> FrameTime {
        let delta = raw_delta.min(self.max_delta);
        self.elapsed = self.elapsed.saturating_add(delta);
        let frame = FrameTime {
            index: self.next_index,
            delta,
            elapsed: self.elapsed,
        };
        self.next_index += 1;
        frame
    }
}

/// Splits frame time into steps of a fixed duration.
///
/// Code that must not depend on the frame rate, such as physics or gameplay that has to be
/// reproducible, runs in fixed steps. Each frame adds its delta to an accumulator;
/// [`advance`](Self::advance) returns how many whole steps fit in it and keeps the remainder for
/// the next frame.
///
/// At most `max_steps_per_frame` steps run in one frame. If more are due, the rest of the backlog
/// is dropped: the simulation then runs slower than real time, instead of falling further behind
/// at each frame because catching up takes longer than the time it catches up on (the "spiral of
/// death").
#[derive(Clone, Debug)]
pub struct FixedStep {
    step: Duration,
    max_steps_per_frame: u32,
    accumulator: Duration,
}

impl FixedStep {
    /// Creates an accumulator for steps of `step`, running at most `max_steps_per_frame` of them
    /// per frame.
    ///
    /// # Panics
    ///
    /// If `step` is zero or `max_steps_per_frame` is zero: no step could ever run.
    pub fn new(step: Duration, max_steps_per_frame: u32) -> Self {
        assert!(!step.is_zero(), "the fixed step must not be zero");
        assert!(
            max_steps_per_frame > 0,
            "at least one fixed step per frame must be allowed"
        );
        Self {
            step,
            max_steps_per_frame,
            accumulator: Duration::ZERO,
        }
    }

    /// The duration of one step.
    pub fn step(&self) -> Duration {
        self.step
    }

    /// The largest number of steps that [`advance`](Self::advance) returns.
    pub fn max_steps_per_frame(&self) -> u32 {
        self.max_steps_per_frame
    }

    /// Adds a frame's delta and returns how many steps to run for that frame.
    ///
    /// The result is at most [`max_steps_per_frame`](Self::max_steps_per_frame). When it is
    /// limited, the whole backlog is dropped, the fraction of a step already accumulated included:
    /// the next frame starts from an empty accumulator, and [`alpha`](Self::alpha) reads 0.0.
    pub fn advance(&mut self, delta: Duration) -> u32 {
        self.accumulator = self.accumulator.saturating_add(delta);
        let mut steps = 0;
        while self.accumulator >= self.step && steps < self.max_steps_per_frame {
            self.accumulator -= self.step;
            steps += 1;
        }
        if self.accumulator >= self.step {
            self.accumulator = Duration::ZERO;
        }
        steps
    }

    /// The fraction of a step that is accumulated but has not run yet: 0.0 right after a step,
    /// approaching 1.0 just before the next one.
    ///
    /// A renderer uses it to interpolate between the last two simulation states, so that motion
    /// looks smooth when the frame rate and the step rate differ. The value is at least 0.0 and
    /// below 1.0, up to floating-point rounding.
    pub fn alpha(&self) -> f64 {
        self.accumulator.div_duration_f64(self.step)
    }
}

/// The clock of game time: the fixed steps it holds, its speed, whether it is paused, and the steps
/// asked for while it is. The host counts the steps run ([`GameTime::steps`]).
#[derive(Clone, Debug)]
pub(crate) struct GameClock {
    fixed: FixedStep,
    speed: f64,
    paused: bool,
    /// Whether the program holds game time, apart from the pause.
    held: bool,
    /// The steps asked for while paused, which the next frames run.
    asked: u32,
}

impl GameClock {
    pub(crate) fn new(fixed: FixedStep) -> Self {
        Self {
            fixed,
            speed: 1.0,
            paused: false,
            held: false,
            asked: 0,
        }
    }

    /// The duration of a fixed step.
    pub(crate) fn step(&self) -> Duration {
        self.fixed.step()
    }

    pub(crate) fn speed(&self) -> f64 {
        self.speed
    }

    /// Sets the speed, a finite number above zero.
    pub(crate) fn set_speed(&mut self, speed: f64) {
        assert!(
            speed.is_finite() && speed > 0.0,
            "the speed of game time is a finite number above zero, not {speed}: pausing stops it"
        );
        self.speed = speed;
    }

    pub(crate) fn paused(&self) -> bool {
        self.paused
    }

    /// Pauses game time, or lets it run again, which drops the steps asked for and not run.
    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if !paused {
            self.asked = 0;
        }
    }

    pub(crate) fn held(&self) -> bool {
        self.held
    }

    /// Holds game time, or lets it go, apart from the pause: held and not paused, it stands still.
    pub(crate) fn set_held(&mut self, held: bool) {
        self.held = held;
    }

    /// The steps asked for and not run yet.
    #[cfg(feature = "development")]
    pub(crate) fn asked(&self) -> u32 {
        self.asked
    }

    /// Asks for `steps` more steps, which the next frames run, while game time is paused; nothing
    /// otherwise.
    pub(crate) fn ask_steps(&mut self, steps: u32) {
        if self.paused {
            self.asked = self.asked.saturating_add(steps);
        }
    }

    /// Adds a frame of `real` time, and returns how many fixed steps to run in it, the game time it
    /// added, and the fraction of a step accumulated and not run ([`GameTime::alpha`]). Paused, the
    /// frame runs the steps asked for, at most as many as a frame runs, and the rest wait for the
    /// next ones. Held and not paused, it runs none.
    pub(crate) fn advance(&mut self, real: Duration) -> (u32, Duration, f64) {
        let (steps, delta) = if self.paused {
            let steps = self.asked.min(self.fixed.max_steps_per_frame());
            self.asked -= steps;
            (steps, Duration::ZERO)
        } else if self.held {
            (0, Duration::ZERO)
        } else {
            // Exact for a speed of 1, a delta being far below the 2^53 nanoseconds that f64 holds.
            let nanos = (real.as_nanos() as f64 * self.speed).round();
            let delta = Duration::from_nanos(nanos as u64);
            (self.fixed.advance(delta), delta)
        };
        (steps, delta, self.fixed.alpha())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn clock_numbers_frames_from_zero_and_sums_their_deltas() {
        let mut clock = FrameClock::new(100 * MS);
        let first = clock.advance(10 * MS);
        let second = clock.advance(20 * MS);
        assert_eq!(
            (first.index(), first.delta(), first.elapsed()),
            (0, 10 * MS, 10 * MS)
        );
        assert_eq!(
            (second.index(), second.delta(), second.elapsed()),
            (1, 20 * MS, 30 * MS)
        );
    }

    #[test]
    fn clock_limits_long_frames_and_does_not_count_the_cut_time() {
        let mut clock = FrameClock::new(100 * MS);
        let paused = clock.advance(Duration::from_secs(30));
        assert_eq!(paused.delta(), 100 * MS);
        assert_eq!(paused.elapsed(), 100 * MS);
    }

    #[test]
    fn clock_elapsed_saturates_instead_of_overflowing() {
        let mut clock = FrameClock::new(Duration::MAX);
        clock.advance(Duration::MAX);
        assert_eq!(clock.advance(MS).elapsed(), Duration::MAX);
    }

    #[test]
    #[should_panic(expected = "maximum frame delta")]
    fn clock_rejects_a_zero_maximum_delta() {
        FrameClock::new(Duration::ZERO);
    }

    #[test]
    fn fixed_step_runs_whole_steps_and_carries_the_remainder() {
        let mut fixed = FixedStep::new(10 * MS, 8);
        assert_eq!(fixed.advance(25 * MS), 2);
        assert_eq!(fixed.alpha(), 0.5);
        assert_eq!(fixed.advance(5 * MS), 1);
        assert_eq!(fixed.alpha(), 0.0);
    }

    #[test]
    fn fixed_step_accumulates_frames_shorter_than_a_step() {
        let mut fixed = FixedStep::new(10 * MS, 8);
        assert_eq!(fixed.advance(4 * MS), 0);
        assert_eq!(fixed.advance(4 * MS), 0);
        assert_eq!(fixed.advance(4 * MS), 1);
        assert_eq!(fixed.alpha(), 0.2);
    }

    #[test]
    fn fixed_step_runs_no_step_for_a_zero_delta() {
        let mut fixed = FixedStep::new(10 * MS, 8);
        assert_eq!(fixed.advance(Duration::ZERO), 0);
    }

    #[test]
    fn fixed_step_limits_steps_and_drops_the_backlog() {
        let mut fixed = FixedStep::new(10 * MS, 3);
        assert_eq!(fixed.advance(100 * MS), 3);
        assert_eq!(fixed.alpha(), 0.0);
        // The 70 ms that did not run are gone: the next frame does not try to catch up.
        assert_eq!(fixed.advance(10 * MS), 1);

        // A fraction of a step left over by a limited frame is dropped with the rest.
        let mut fixed = FixedStep::new(10 * MS, 3);
        assert_eq!(fixed.advance(105 * MS), 3);
        assert_eq!(fixed.alpha(), 0.0);
        assert_eq!(fixed.advance(5 * MS), 0);
    }

    #[test]
    fn fixed_step_loses_no_time_when_it_is_not_limited() {
        let deltas = [3, 17, 9, 1, 26, 14, 8, 33, 2, 11].map(|ms| ms * MS);
        let mut fixed = FixedStep::new(7 * MS, 8);
        let mut steps = 0;
        for delta in deltas {
            steps += fixed.advance(delta);
            assert!((0.0..1.0).contains(&fixed.alpha()));
        }
        let total: Duration = deltas.iter().sum();
        assert_eq!(fixed.step() * steps + fixed.accumulator, total);
    }

    #[test]
    fn a_delta_equal_to_the_step_runs_exactly_one_step_per_frame() {
        let step = Duration::from_nanos(16_666_667);
        let mut clock = FrameClock::new(250 * MS);
        let mut fixed = FixedStep::new(step, 8);
        for _ in 0..1000 {
            let frame = clock.advance(step);
            assert_eq!(fixed.advance(frame.delta()), 1);
        }
        assert_eq!(clock.advance(step).elapsed(), step * 1001);
    }

    #[test]
    #[should_panic(expected = "fixed step")]
    fn fixed_step_rejects_a_zero_step() {
        FixedStep::new(Duration::ZERO, 8);
    }

    #[test]
    #[should_panic(expected = "at least one fixed step")]
    fn fixed_step_rejects_zero_steps_per_frame() {
        FixedStep::new(MS, 0);
    }

    /// A clock of steps of 10 ms, as long as the frames of [`run`].
    fn clock() -> GameClock {
        GameClock::new(FixedStep::new(10 * MS, 8))
    }

    /// Runs `frames` frames of 10 ms: how many steps each ran, and its fraction of a step.
    fn run(clock: &mut GameClock, frames: usize) -> Vec<(u32, f64)> {
        (0..frames)
            .map(|_| {
                let (steps, _, alpha) = clock.advance(10 * MS);
                (steps, alpha)
            })
            .collect()
    }

    #[test]
    fn at_a_quarter_of_the_speed_a_step_runs_at_every_fourth_frame() {
        let step = Duration::from_nanos(16_666_667);
        let mut clock = GameClock::new(FixedStep::new(step, 8));
        clock.set_speed(0.25);
        let frames: Vec<(u32, f64)> = (0..12)
            .map(|_| {
                let (steps, _, alpha) = clock.advance(step);
                (steps, alpha)
            })
            .collect();
        let steps: Vec<u32> = frames.iter().map(|&(steps, _)| steps).collect();
        assert_eq!(steps, [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1]);
        // A quarter of a step further at each frame, but for the nanosecond that rounding adds.
        for (frame, &(_, alpha)) in frames.iter().enumerate() {
            let expected = ((frame + 1) % 4) as f64 / 4.0;
            assert!((alpha - expected).abs() < 1e-6, "{frame}: {alpha}");
        }
    }

    #[test]
    fn paused_no_step_runs_and_a_step_asked_for_runs_exactly_one() {
        let mut clock = clock();
        clock.set_speed(0.5);
        run(&mut clock, 1);
        clock.set_paused(true);
        assert_eq!(
            run(&mut clock, 3),
            [(0, 0.5); 3],
            "nothing runs, alpha stays"
        );
        clock.ask_steps(1);
        assert_eq!(run(&mut clock, 2), [(1, 0.5), (0, 0.5)]);
        // Running again, at the speed it had.
        clock.set_paused(false);
        assert_eq!(run(&mut clock, 1), [(1, 0.0)]);
    }

    #[test]
    fn held_game_time_stands_still_but_for_the_steps_asked_while_paused() {
        let mut clock = clock();
        clock.set_held(true);
        assert_eq!(run(&mut clock, 2), [(0, 0.0); 2]);
        clock.set_paused(true);
        clock.ask_steps(1);
        assert_eq!(
            run(&mut clock, 1),
            [(1, 0.0)],
            "a step asked for runs all the same"
        );
        clock.set_paused(false);
        assert_eq!(run(&mut clock, 1), [(0, 0.0)], "still held");
        clock.set_held(false);
        assert_eq!(run(&mut clock, 1), [(1, 0.0)]);
    }

    #[test]
    fn a_step_asked_for_while_game_time_runs_changes_nothing() {
        let mut clock = clock();
        clock.ask_steps(1);
        assert_eq!(run(&mut clock, 2), [(1, 0.0); 2]);
        clock.set_paused(true);
        clock.ask_steps(2);
        // Dropped when game time runs again.
        clock.set_paused(false);
        clock.set_paused(true);
        assert_eq!(run(&mut clock, 1), [(0, 0.0)]);
    }

    #[test]
    fn game_time_counts_the_steps_it_ran() {
        let time = GameTime {
            step: 10 * MS,
            steps: 250,
            ..GameTime::default()
        };
        assert_eq!(time.elapsed(), 2500 * MS);
        let longest = GameTime {
            step: Duration::MAX,
            steps: u64::MAX,
            ..GameTime::default()
        };
        assert_eq!(longest.elapsed(), Duration::MAX);
    }

    #[test]
    #[should_panic(expected = "speed of game time")]
    fn a_speed_of_zero_is_refused() {
        clock().set_speed(0.0);
    }
}
