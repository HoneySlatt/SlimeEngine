//! The host's clock on Linux (`docs/adr/0227-the-time-that-motion-reads.md`): `CLOCK_BOOTTIME`,
//! which counts the time that the machine spends suspended, so that what the interface waits ten
//! minutes for is due ten minutes later, sleep or not; and the time suspended between two
//! readings, from `CLOCK_MONOTONIC`, which stops during a suspend.

use std::time::Duration;

use rustix::time::{ClockId, Timespec, clock_gettime};
use uwui_time::Instant;

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

impl Suspends {
    pub(crate) fn new() -> Self {
        Self { last: readings() }
    }

    /// The time that the machine spent suspended since the last call, or since the clock was made:
    /// how much more the host's clock went than the one that stops, off by the nanoseconds between
    /// the two readings.
    pub(crate) fn take(&mut self) -> Duration {
        let now = readings();
        let host = now.0.saturating_sub(self.last.0);
        let running = now.1.saturating_sub(self.last.1);
        self.last = now;
        Duration::from_nanos(host.saturating_sub(running))
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
        assert!(suspends.take() < Duration::from_millis(1));
    }
}
