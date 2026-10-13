//! Frame timing: how much time each frame represents, and how many fixed steps it contains.
//!
//! Both types are fed durations instead of reading the system clock, so the code that drives
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

/// Timing information about one frame, produced by [`FrameClock::advance`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}
