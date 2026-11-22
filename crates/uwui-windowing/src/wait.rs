//! The loop's wait on Linux, which Wayland's and X11's layers share: one `poll` on the display's
//! socket, an eventfd that other threads write to wake it, a timerfd on `CLOCK_BOOTTIME`, the
//! host's clock, for its deadline, and the kernel's notice of a resume. A deadline armed before a
//! suspend ends on time after it, which the timer's clock counts.
//!
//! The notice is a timerfd on `CLOCK_REALTIME` that never ends, armed with
//! `TFD_TIMER_CANCEL_ON_SET`: the kernel cancels it, which makes it readable, whenever the wall
//! clock moves against `CLOCK_MONOTONIC`, as it does at each resume, which the kernel tells such
//! timers as it tells a clock set (`timekeeping_resume` calls `timerfd_resume`), and when someone
//! sets the clock. The wait ends then, and the layer's measure of the time suspended
//! (`crate::clock`) tells a resume from a clock set
//! (`docs/adr/0315-a-resume-told-by-the-kernels-own-notice-and-given-to-the-program-as-an-action.md`).
//! A resume is told so only where the kernel measures the sleep: a suspend to idle whose
//! timekeeping keeps running, as without a driver of the processor's idle states, moves neither the
//! clocks apart nor the timer.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};
use uwui_time::Instant;

/// The eventfd that wakes the loop, its timer, and the kernel's notice of a resume.
pub(crate) struct Waits {
    wake: Arc<OwnedFd>,
    timer: OwnedFd,
    /// The timer that the kernel cancels as the system resumes or the wall clock is set.
    clock_set: OwnedFd,
    /// The deadline that the timer holds, if it is armed.
    armed: Option<Instant>,
    /// Whether the last wait ended for a wake, the deadline or the kernel's notice, beside the
    /// display, if at all.
    interrupted: bool,
}

impl Waits {
    pub(crate) fn new() -> io::Result<Self> {
        let clock_set = timerfd_create(
            TimerfdClockId::Realtime,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        watch_clock(&clock_set);
        Ok(Self {
            wake: Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?),
            timer: timerfd_create(
                TimerfdClockId::Boottime,
                TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
            )?,
            clock_set,
            armed: None,
            interrupted: false,
        })
    }

    /// What wakes the loop from any thread, without blocking: one write to the eventfd.
    pub(crate) fn notifier(&self) -> impl Fn() + Send + Sync + 'static {
        let wake = Arc::clone(&self.wake);
        move || {
            // Its counter full, at 2⁶⁴ − 2 writes not read, the loop is woken already.
            let _ = rustix::io::write(&*wake, &1_u64.to_ne_bytes());
        }
    }

    /// Waits until `display` can be read, the loop is woken, `deadline` passes, none for no
    /// deadline, or the kernel tells a resume or a clock set; and, when `write`, until `display`
    /// can be written to, its socket full before. Answers whether `display` can be read, or ended.
    pub(crate) fn wait(
        &mut self,
        display: BorrowedFd<'_>,
        deadline: Option<Instant>,
        write: bool,
    ) -> io::Result<bool> {
        if deadline != self.armed {
            // An absolute time of zero disarms the timer: a deadline is one nanosecond at least.
            let at = deadline.map_or(0, |deadline| deadline.as_nanos().max(1));
            let value = Timespec {
                tv_sec: i64::try_from(at / 1_000_000_000).unwrap_or(i64::MAX),
                tv_nsec: i64::try_from(at % 1_000_000_000).unwrap_or(0),
            };
            arm(&self.timer, TimerfdTimerFlags::empty(), value)?;
            self.armed = deadline;
        }
        let mut fds = [
            PollFd::new(
                &display,
                if write {
                    PollFlags::IN | PollFlags::OUT
                } else {
                    PollFlags::IN
                },
            ),
            PollFd::new(&*self.wake, PollFlags::IN),
            PollFd::new(&self.timer, PollFlags::IN),
            PollFd::new(&self.clock_set, PollFlags::IN),
        ];
        loop {
            match poll(&mut fds, None) {
                Ok(_) => break,
                // A termination signal's handler ran on this thread: its wake is the eventfd's.
                Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let [display, wake, timer, clock_set] = fds.map(|fd| fd.revents());
        let mut count = [0_u8; 8];
        if wake.contains(PollFlags::IN) {
            let _ = rustix::io::read(self.wake.as_fd(), &mut count);
        }
        if timer.contains(PollFlags::IN) {
            let _ = rustix::io::read(self.timer.as_fd(), &mut count);
            // A timer that fired is disarmed.
            self.armed = None;
        }
        if clock_set.contains(PollFlags::IN) {
            // A cancel's read fails with `ECANCELED` and leaves the timer watching from the clock's
            // new place; one that expired, as a test arms it, is disarmed: armed again either way.
            let _ = rustix::io::read(self.clock_set.as_fd(), &mut count);
            watch_clock(&self.clock_set);
        }
        self.interrupted = wake.contains(PollFlags::IN)
            || timer.contains(PollFlags::IN)
            || clock_set.contains(PollFlags::IN);
        Ok(display.intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP))
    }

    /// Whether the last wait ended for a wake from another thread or a termination signal, for its
    /// deadline, or for a resume or a clock set, whether or not the display could be read too: the
    /// loop must turn, even where what it read from the display tells nothing.
    pub(crate) fn interrupted(&self) -> bool {
        self.interrupted
    }
}

/// Arms `timer` once, at the time `at` of its clock, with `flags` beside `TFD_TIMER_ABSTIME`; a
/// time of zero disarms it.
fn arm(timer: &OwnedFd, flags: TimerfdTimerFlags, at: Timespec) -> rustix::io::Result<()> {
    let once = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    timerfd_settime(
        timer,
        TimerfdTimerFlags::ABSTIME | flags,
        &Itimerspec {
            it_interval: once,
            it_value: at,
        },
    )
    .map(drop)
}

/// Arms the notice, `timer` on `CLOCK_REALTIME`, at the clock's last time, which it never reaches,
/// so that the kernel cancels it when the wall clock moves against `CLOCK_MONOTONIC`. It never ends
/// the run: a clock set as it is armed answers `ECANCELED`, the timer armed all the same and
/// watching from the clock's new place (`fs/timerfd.c`), the wait already ended for that set; any
/// other failure, which no kernel since 3.0 gives but a sandbox might, leaves the notice unarmed,
/// and a resume is then told at the next turn that something else wakes.
fn watch_clock(timer: &OwnedFd) {
    let never = Timespec {
        tv_sec: i64::MAX,
        tv_nsec: 0,
    };
    match arm(timer, TimerfdTimerFlags::CANCEL_ON_SET, never) {
        Ok(()) | Err(rustix::io::Errno::CANCELED) => {}
        Err(error) => tracing::warn!(
            %error,
            "the kernel's notice of a resume cannot be armed: a resume is told at the next input"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A display played by an eventfd: readable once written to.
    fn display() -> OwnedFd {
        eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK).expect("an eventfd")
    }

    fn write(fd: &OwnedFd) {
        rustix::io::write(fd, &1_u64.to_ne_bytes()).expect("written");
    }

    #[test]
    fn a_wait_tells_what_ended_it() {
        let mut waits = Waits::new().expect("the waits");
        let display = display();
        // The display alone.
        write(&display);
        assert!(waits.wait(display.as_fd(), None, false).expect("waited"));
        assert!(!waits.interrupted());
        let mut count = [0_u8; 8];
        rustix::io::read(&display, &mut count).expect("read");
        // A wake from another thread.
        waits.notifier()();
        assert!(!waits.wait(display.as_fd(), None, false).expect("waited"));
        assert!(waits.interrupted());
        // A deadline passed.
        let past = Instant::from_nanos(1);
        assert!(
            !waits
                .wait(display.as_fd(), Some(past), false)
                .expect("waited")
        );
        assert!(waits.interrupted());
        // The display and a wake together: both told.
        write(&display);
        waits.notifier()();
        assert!(waits.wait(display.as_fd(), None, false).expect("waited"));
        assert!(waits.interrupted());
    }

    /// What the kernel tells of a timerfd, `/proc/self/fdinfo`'s lines.
    fn fdinfo(fd: &OwnedFd) -> String {
        use std::os::fd::AsRawFd;
        std::fs::read_to_string(format!("/proc/self/fdinfo/{}", fd.as_raw_fd())).expect("fdinfo")
    }

    /// Checks that the notice watches the wall clock, nothing left to read: `CLOCK_REALTIME`,
    /// armed absolute and cancelled when the clock is set, the kernel's
    /// `TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET`, at a time that it never reaches.
    fn watching(waits: &Waits) {
        let info = fdinfo(&waits.clock_set);
        assert!(info.contains("clockid: 0\n"), "{info}");
        assert!(info.contains("ticks: 0\n"), "{info}");
        assert!(info.contains("settime flags: 03\n"), "{info}");
        assert!(!info.contains("it_value: (0, 0)"), "{info}");
    }

    #[test]
    fn the_notice_of_a_resume_watches_the_wall_clock_from_the_start() {
        watching(&Waits::new().expect("the waits"));
    }

    #[test]
    fn a_readable_notice_ends_the_wait_and_is_armed_again() {
        let mut waits = Waits::new().expect("the waits");
        // Armed at a past time of the wall clock, the notice fires at once, readable as a cancel
        // makes it.
        let past = Timespec {
            tv_sec: 1,
            tv_nsec: 0,
        };
        arm(&waits.clock_set, TimerfdTimerFlags::CANCEL_ON_SET, past).expect("armed in the past");
        let display = display();
        let start = crate::clock::now();
        let deadline = start + std::time::Duration::from_secs(2);
        assert!(
            !waits
                .wait(display.as_fd(), Some(deadline), false)
                .expect("waited")
        );
        assert!(waits.interrupted());
        assert!(crate::clock::now() - start < std::time::Duration::from_secs(1));
        watching(&waits);
    }

    /// Sets the wall clock to the time it shows, through `sudo -n`, which CI's runners allow: the
    /// kernel cancels the notice as it does at a resume, by the few microseconds the clock steps.
    fn set_the_clock_to_now() {
        let set = std::process::Command::new("sudo")
            .args(["-n", "sh", "-c", r#"date -s "@$(date +%s.%N)" > /dev/null"#])
            .status()
            .expect("sudo runs");
        assert!(set.success(), "the clock was not set: {set}");
    }

    /// The kernel's own cancel, which only a privileged clock set gives short of a suspend: run in
    /// CI alone, on a runner thrown away after it
    /// (`docs/adr/0315-a-resume-told-by-the-kernels-own-notice-and-given-to-the-program-as-an-action.md`).
    #[test]
    #[ignore = "sets the system's clock through sudo: CI's runners alone, with UWUI_TEST_SETS_CLOCK=1"]
    fn a_clock_set_ends_the_wait_and_leaves_the_notice_watching() {
        assert_eq!(
            std::env::var("UWUI_TEST_SETS_CLOCK").as_deref(),
            Ok("1"),
            "this test sets the system's clock: run it on a throwaway machine, with \
             UWUI_TEST_SETS_CLOCK=1"
        );
        let mut waits = Waits::new().expect("the waits");
        let display = display();
        // Three sets in a row, each read and watched again.
        for _ in 0..3 {
            let start = crate::clock::now();
            let setter = std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(100));
                set_the_clock_to_now();
            });
            let deadline = start + std::time::Duration::from_secs(10);
            let readable = waits
                .wait(display.as_fd(), Some(deadline), false)
                .expect("waited");
            setter.join().expect("the clock set");
            assert!(!readable);
            assert!(waits.interrupted());
            assert!(crate::clock::now() - start < std::time::Duration::from_secs(5));
            watching(&waits);
        }
    }
}
