//! The host's clock on Linux (`docs/adr/0227-the-time-that-motion-reads.md`): `CLOCK_BOOTTIME`,
//! which counts the time that the machine spends suspended, so that what the interface waits ten
//! minutes for is due ten minutes later, sleep or not; and the time suspended between two
//! readings, from `CLOCK_MONOTONIC`, which stops during a suspend, which tells a resume; and the
//! times of the clock that the compositor names for its presentations, placed on the host's.

use std::time::Duration;

use rustix::time::{ClockId, Timespec, clock_gettime};
use uwui_time::Instant;

/// The shortest time suspended that tells a resume: what parts the two clocks while the machine is
/// awake, the nanoseconds between two readings, is far below it.
const RESUMED: Duration = Duration::from_secs(1);

/// The time that the machine spends suspended, measured between two readings of both clocks.
#[derive(Debug)]
pub(crate) struct Suspends {
    /// Both clocks at the last measure, in nanoseconds: the host's, then the one that stops.
    last: (u64, u64),
}

/// Nanoseconds of a reading; both clocks count from the machine's start, never below zero.
fn nanos(time: Timespec) -> u64 {
    let seconds = u64::try_from(time.tv_sec).unwrap_or(0);
    let nanos = u64::try_from(time.tv_nsec).unwrap_or(0);
    seconds.saturating_mul(1_000_000_000).saturating_add(nanos)
}

/// `CLOCK_BOOTTIME` and `CLOCK_MONOTONIC` read together, each served by the vDSO without a system
/// call since Linux 5.3.
fn readings() -> (u64, u64) {
    (
        nanos(clock_gettime(ClockId::Boottime)),
        nanos(clock_gettime(ClockId::Monotonic)),
    )
}

/// The host's clock now.
pub(crate) fn now() -> Instant {
    Instant::from_nanos(nanos(clock_gettime(ClockId::Boottime)))
}

/// A platform's clock, by Linux's number of it, as `wp_presentation` names the one of its times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clock(ClockId);

impl Clock {
    /// The clock of Linux's number `raw`; none for a number that names no clock that every Linux
    /// gives.
    pub(crate) fn from_raw(raw: u32) -> Option<Self> {
        let raw = i32::try_from(raw).ok()?;
        ClockId::try_from(raw).ok().map(Self)
    }

    /// The time `nanos` of this clock, placed on the host's: both clocks read together now, the
    /// time moved by how far the host's runs ahead of this one. Off by the nanoseconds between the
    /// two readings; a time of `CLOCK_MONOTONIC` before a suspend is placed as if none came between.
    pub(crate) fn on_host(self, nanos_of_clock: u64) -> Instant {
        let host = nanos(clock_gettime(ClockId::Boottime));
        let this = nanos(clock_gettime(self.0));
        Instant::from_nanos(
            host.saturating_sub(this.saturating_sub(nanos_of_clock))
                .saturating_add(nanos_of_clock.saturating_sub(this)),
        )
    }
}

impl Suspends {
    pub(crate) fn new() -> Self {
        Self { last: readings() }
    }

    /// Whether the machine resumed since the last call, or since the clock was made, and how long
    /// it was suspended: [`RESUMED`] or more. A shorter time is not told, and counts no more.
    pub(crate) fn resumed(&mut self) -> Option<Duration> {
        self.resumed_at(readings())
    }

    /// [`Self::resumed`] at the readings `now`: how much more the host's clock went than the one
    /// that stops, off by the nanoseconds between the two readings.
    fn resumed_at(&mut self, now: (u64, u64)) -> Option<Duration> {
        let host = now.0.saturating_sub(self.last.0);
        let running = now.1.saturating_sub(self.last.1);
        self.last = now;
        let suspended = Duration::from_nanos(host.saturating_sub(running));
        (suspended >= RESUMED).then_some(suspended)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_goes_on_and_counts_no_suspend_while_the_machine_is_awake() {
        let mut suspends = Suspends::new();
        let first = now();
        std::thread::sleep(Duration::from_millis(2));
        assert!(now() - first >= Duration::from_millis(2));
        assert_eq!(suspends.resumed(), None);
    }

    #[test]
    fn a_resume_is_told_once_from_a_second_suspended() {
        let mut suspends = Suspends { last: (100, 100) };
        // Awake: both clocks went alike.
        assert_eq!(suspends.resumed_at((5_000_000_100, 5_000_000_100)), None);
        // Just under a second suspended: not told, and not counted again later.
        assert_eq!(suspends.resumed_at((5_999_999_200, 5_000_000_200)), None);
        assert_eq!(suspends.resumed_at((6_000_000_200, 5_000_000_200)), None);
        // A second: told once, with its length.
        assert_eq!(
            suspends.resumed_at((7_000_000_200, 5_000_000_200)),
            Some(Duration::from_secs(1))
        );
        assert_eq!(suspends.resumed_at((7_000_000_300, 5_000_000_300)), None);
        // Eight hours.
        assert_eq!(
            suspends.resumed_at((28_807_000_000_300, 5_000_000_300)),
            Some(Duration::from_secs(8 * 3600))
        );
    }

    #[test]
    fn a_time_of_another_clock_is_placed_on_the_hosts() {
        // `CLOCK_MONOTONIC`, which compositors give: a time 3 ms ago is 3 ms ago on the host's.
        let monotonic = Clock::from_raw(1).expect("Linux's monotonic clock");
        let then = nanos(clock_gettime(ClockId::Monotonic)) - 3_000_000;
        let placed = monotonic.on_host(then);
        let ago = now() - placed;
        assert!(
            ago >= Duration::from_millis(3) && ago < Duration::from_millis(50),
            "{ago:?}"
        );
        // The host's own clock moves nothing.
        let host = Clock::from_raw(7).expect("Linux's boot time");
        let at = nanos(clock_gettime(ClockId::Boottime));
        assert_eq!(host.on_host(at).as_nanos().abs_diff(at) / 1_000_000, 0);
        // A number that names no clock.
        assert_eq!(Clock::from_raw(99), None);
        assert_eq!(Clock::from_raw(u32::MAX), None);
    }
}
