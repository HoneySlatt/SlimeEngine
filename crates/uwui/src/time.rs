//! The times that a host gives uwui (`docs/adr/0227-the-time-that-motion-reads.md`): when each frame
//! shows, when each input event came, and each root's clock.
//!
//! Two types of time, in whole nanoseconds, that cannot be mixed:
//!
//! - [`Instant`], a time of the *host's clock*: one clock for every time that the host gives, which
//!   counts the time that the machine spends suspended, its origin the clock's own zero;
//! - [`RootTime`], a time of a root's clock, which a game pauses, slows or hastens ([`RootClock`]).
//!
//! A gap between two times of one clock is a [`Duration`]. A float comes only from a `Duration`,
//! never from an origin, whose distance would cost it its precision: `f32` seconds step by 7.8 ms
//! after a day.
//!
//! ```
//! use std::time::Duration;
//! use uwui::time::{Instant, RootTime};
//!
//! let shown = Instant::from_nanos(1_000_000_000);
//! let later = shown + Duration::from_millis(16);
//! assert_eq!(later - shown, Duration::from_millis(16));
//! assert_eq!(later.as_nanos(), 1_016_000_000);
//!
//! // A root's time is another clock's: subtracting it from the host's does not compile.
//! let game = RootTime::from_nanos(5);
//! assert_eq!(game + Duration::from_nanos(5), RootTime::from_nanos(10));
//! ```
//!
//! ```compile_fail
//! use uwui::time::{Instant, RootTime};
//!
//! let _ = Instant::from_nanos(10) - RootTime::from_nanos(5);
//! ```

use std::ops::{Add, AddAssign, Sub};
use std::time::Duration;

/// Defines a time of one clock in whole nanoseconds, with the arithmetic that keeps it to its
/// clock: a time plus a gap, and the gap between two times, which saturates at zero.
macro_rules! time_of_a_clock {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            /// The clock's zero.
            pub const ZERO: Self = Self(0);

            /// The time `nanos` nanoseconds after the clock's zero.
            pub const fn from_nanos(nanos: u64) -> Self {
                Self(nanos)
            }

            /// The nanoseconds since the clock's zero.
            pub const fn as_nanos(self) -> u64 {
                self.0
            }

            /// The gap from `earlier` to this time; zero if `earlier` is later.
            pub fn saturating_duration_since(self, earlier: Self) -> Duration {
                Duration::from_nanos(self.0.saturating_sub(earlier.0))
            }
        }

        impl Add<Duration> for $name {
            type Output = Self;

            /// The time `gap` later, saturating at the clock's last nanosecond, some 584 years
            /// after its zero.
            fn add(self, gap: Duration) -> Self {
                let gap = u64::try_from(gap.as_nanos()).unwrap_or(u64::MAX);
                Self(self.0.saturating_add(gap))
            }
        }

        impl AddAssign<Duration> for $name {
            fn add_assign(&mut self, gap: Duration) {
                *self = *self + gap;
            }
        }

        impl Sub for $name {
            type Output = Duration;

            /// The gap from `earlier` to this time; zero if `earlier` is later, as
            /// [`saturating_duration_since`](Self::saturating_duration_since).
            fn sub(self, earlier: Self) -> Duration {
                self.saturating_duration_since(earlier)
            }
        }
    };
}

time_of_a_clock! {
    /// A time of the host's clock, in whole nanoseconds from the clock's own zero: the time at which
    /// a frame shows, at which an input event came, at which something waits.
    ///
    /// The host reads its platform's clock itself, one that counts the time the machine sleeps
    /// (`CLOCK_BOOTTIME` on Linux, the performance counter on Windows), so that a platform's time
    /// converts by its unit alone. Not [`std::time::Instant`]: it cannot be made from a platform's
    /// time nor by a test, it panics on the web, and it stops during a suspend on Linux and macOS.
    Instant
}

time_of_a_clock! {
    /// A time of a root's clock, in whole nanoseconds: the game's time that a frame shows, for a
    /// root that follows its game ([`RootClock`]).
    RootTime
}

/// The time at which a frame shows, and where that time comes from: given before each
/// [`Ui::frame`](crate::tree::Ui::frame) by
/// [`Ui::set_frame_shown`](crate::tree::Ui::set_frame_shown). One time serves every root of a
/// `Ui`: each window's own comes with the targets of roots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameShown {
    /// When the frame shows, on the host's clock.
    pub at: Instant,
    /// Where the time comes from, which tells traces and tests how near it is to when the light
    /// leaves the screen.
    pub source: FrameSource,
}

impl FrameShown {
    /// A frame that shows `at`, at the time that a test or a headless run gives.
    pub const fn simulated(at: Instant) -> Self {
        Self {
            at,
            source: FrameSource::Simulated,
        }
    }
}

/// Where a frame's time comes from (`docs/adr/0227-the-time-that-motion-reads.md`).
///
/// The platforms' own times, given by Apple and Android, predicted from past presentations on
/// Wayland, Windows and X11, asked for, ready under a variable refresh, or on the display's grid,
/// come with the hosts that read them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FrameSource {
    /// The time that a test or a headless run gives.
    #[default]
    Simulated,
    /// When the host started the frame: what a host gives where it reads no time of
    /// presentation, as the engine's windowed loop does. Motion then lags by the frames in flight,
    /// and steps with the host's wakes.
    Started,
}

/// The time at which an input event came, and where that time comes from: given before each
/// event by [`Ui::set_event_time`](crate::tree::Ui::set_event_time).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventTime {
    /// When the event came, on the host's clock.
    pub at: Instant,
    /// Where the time comes from.
    pub source: EventSource,
}

impl EventTime {
    /// An event that the host received `at`, stamped as it took it.
    pub const fn received(at: Instant) -> Self {
        Self {
            at,
            source: EventSource::Received,
        }
    }
}

/// Where an input event's time comes from (`docs/adr/0227-the-time-that-motion-reads.md`).
///
/// A platform's own times, in a clock that the host knows or anchored from another clock, come with
/// the hosts that read them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EventSource {
    /// When the host received the event, where its platform gives no time, as winit gives none: a
    /// stamp taken in a batch, or behind a busy frame, comes late.
    #[default]
    Received,
}

/// A root's clock at a frame, as its host gives it
/// ([`Ui::set_root_clock`](crate::tree::Ui::set_root_clock)): its time, and its *speed*, 1 for the
/// host's pace, 0 for a pause, less to slow it, more to hasten it.
///
/// A root that is never given its clock follows the host's at speed 1. A game gives the game time
/// that the frame shows, interpolated between its last two steps as its world is drawn.
///
/// What follows the root's clock: the transitions of its styles but those of the states `Hovered`,
/// `Pressed` and `FocusVisible`, and its sections' growing and shrinking, in the root and in the
/// layers that open over it. What follows the host's clock always, since it answers a person:
/// those three states' transitions, tooltips' and submenus' rests, notifications' times, a bar of
/// progress without a share. What runs while a game is paused, a pause menu or a dialog over the
/// game, is a root of its own, added beside the game's and given no clock, on the host's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RootClock {
    /// The root's time that the frame shows.
    pub time: RootTime,
    /// How fast the root's time goes against the host's: 0 while paused; a finite number, never
    /// below 0. uwui places motions by [`time`](Self::time) alone, and reads the speed only as
    /// whether the clock runs: a host gives a time that moves with its speed, and 0 when it holds,
    /// so that what holds asks for no frame.
    pub speed: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gap_between_two_times_saturates_at_zero() {
        let early = Instant::from_nanos(10);
        let late = Instant::from_nanos(25);
        assert_eq!(late - early, Duration::from_nanos(15));
        assert_eq!(early - late, Duration::ZERO);
    }

    #[test]
    fn a_time_plus_a_huge_gap_saturates_at_the_clocks_end() {
        let time = RootTime::from_nanos(u64::MAX - 1) + Duration::MAX;
        assert_eq!(time.as_nanos(), u64::MAX);
        let mut instant = Instant::ZERO;
        instant += Duration::from_secs(2);
        assert_eq!(instant.as_nanos(), 2_000_000_000);
    }
}
