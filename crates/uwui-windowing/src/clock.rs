//! The host's clock (`docs/adr/0227-the-time-that-motion-reads.md`): one that counts the time that
//! the machine spends suspended, so that what the interface waits ten minutes for is due ten minutes
//! later, sleep or not; and the time suspended between two readings, from a clock that stops during
//! a suspend, which tells a resume.
//!
//! - Linux: `CLOCK_BOOTTIME`, against `CLOCK_MONOTONIC`, which stops; and the times of the clock
//!   that the compositor names for its presentations, placed on the host's.
//! - Windows: the performance counter, which counts sleep and hibernation, against the unbiased
//!   interrupt time, which does not; both read from the same counter of the processor. Taken from
//!   the engine's own clock (`crates/slime-app/src/time.rs`), under uwui's licence, the owner's go
//!   (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`).

use std::time::Duration;

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

/// Both clocks read together, in nanoseconds: the host's, then the one that stops.
fn readings() -> (u64, u64) {
    (platform::host(), platform::running())
}

/// The host's clock now.
pub(crate) fn now() -> Instant {
    Instant::from_nanos(platform::host())
}

#[cfg(target_os = "linux")]
mod platform {
    use rustix::time::{ClockId, Timespec, clock_gettime};

    /// Nanoseconds of a reading; both clocks count from the machine's start, never below zero.
    pub(super) fn nanos(time: Timespec) -> u64 {
        let seconds = u64::try_from(time.tv_sec).unwrap_or(0);
        let nanos = u64::try_from(time.tv_nsec).unwrap_or(0);
        seconds.saturating_mul(1_000_000_000).saturating_add(nanos)
    }

    /// `CLOCK_BOOTTIME`, served by the vDSO without a system call since Linux 5.3.
    pub(super) fn host() -> u64 {
        nanos(clock_gettime(ClockId::Boottime))
    }

    /// `CLOCK_MONOTONIC`, which stops during a suspend.
    pub(super) fn running() -> u64 {
        nanos(clock_gettime(ClockId::Monotonic))
    }
}

#[cfg(windows)]
mod platform {
    use std::sync::OnceLock;

    use windows_sys::Win32::System::Performance::{
        QueryPerformanceCounter, QueryPerformanceFrequency,
    };
    use windows_sys::Win32::System::WindowsProgramming::QueryUnbiasedInterruptTimePrecise;

    /// The performance counter's ticks a second, fixed at the system's start.
    fn frequency() -> u64 {
        static FREQUENCY: OnceLock<u64> = OnceLock::new();
        *FREQUENCY.get_or_init(|| {
            let mut frequency = 0_i64;
            #[expect(unsafe_code, reason = "a call to Windows' API")]
            // SAFETY: the pointer is to a live, aligned `i64` that the call writes; the call never
            // fails on Windows XP and later.
            unsafe {
                QueryPerformanceFrequency(&raw mut frequency);
            }
            u64::try_from(frequency).unwrap_or(1).max(1)
        })
    }

    /// The performance counter's `ticks` in nanoseconds.
    fn nanos(ticks: i64) -> u64 {
        let ticks = u128::from(u64::try_from(ticks).unwrap_or(0));
        u64::try_from(ticks * 1_000_000_000 / u128::from(frequency())).unwrap_or(u64::MAX)
    }

    /// The performance counter, which counts sleep and hibernation.
    pub(super) fn host() -> u64 {
        let mut ticks = 0_i64;
        #[expect(unsafe_code, reason = "a call to Windows' API")]
        // SAFETY: the pointer is to a live, aligned `i64` that the call writes; the call never
        // fails on Windows XP and later.
        unsafe {
            QueryPerformanceCounter(&raw mut ticks);
        }
        nanos(ticks)
    }

    /// The unbiased interrupt time, in units of 100 ns, which stops during sleep and hibernation.
    pub(super) fn running() -> u64 {
        let mut hundreds = 0_u64;
        #[expect(unsafe_code, reason = "a call to Windows' API")]
        // SAFETY: the pointer is to a live, aligned `u64` that the call writes; the call exists
        // from Windows 10, which Direct3D 12 needs too.
        unsafe {
            QueryUnbiasedInterruptTimePrecise(&raw mut hundreds);
        }
        hundreds.saturating_mul(100)
    }
}

/// A platform's clock, by Linux's number of it, as `wp_presentation` names the one of its times.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clock(rustix::time::ClockId);

#[cfg(target_os = "linux")]
impl Clock {
    /// The clock of Linux's number `raw`; none for a number that names no clock that every Linux
    /// gives.
    pub(crate) fn from_raw(raw: u32) -> Option<Self> {
        let raw = i32::try_from(raw).ok()?;
        rustix::time::ClockId::try_from(raw).ok().map(Self)
    }

    /// The time `nanos` of this clock, placed on the host's: both clocks read together now, the
    /// time moved by how far the host's runs ahead of this one. Off by the nanoseconds between the
    /// two readings; a time of `CLOCK_MONOTONIC` before a suspend is placed as if none came between.
    pub(crate) fn on_host(self, nanos_of_clock: u64) -> Instant {
        let host = platform::host();
        let this = platform::nanos(rustix::time::clock_gettime(self.0));
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

    #[cfg(target_os = "linux")]
    #[test]
    fn a_time_of_another_clock_is_placed_on_the_hosts() {
        use rustix::time::{ClockId, clock_gettime};

        use super::platform::nanos;

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
