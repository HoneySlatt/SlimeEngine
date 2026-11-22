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
    /// Whether the last wait ended for a wake or the deadline, beside the display, if at all.
    interrupted: bool,
}

impl Waits {
    pub(crate) fn new() -> io::Result<Self> {
        let clock_set = timerfd_create(
            TimerfdClockId::Realtime,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        watch_clock(&clock_set)?;
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

    /// Waits until `display` can be read, the loop is woken, or `deadline` passes; none for no
    /// deadline; and, when `write`, until `display` can be written to, its socket full before.
    /// Answers whether `display` can be read, or ended.
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
            timerfd_settime(
                &self.timer,
                TimerfdTimerFlags::ABSTIME,
                &Itimerspec {
                    it_interval: Timespec {
                        tv_sec: 0,
                        tv_nsec: 0,
                    },
                    it_value: value,
                },
            )?;
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
            // Its read fails with `ECANCELED`, which leaves it watching from the clock's new
            // place; armed again all the same.
            let _ = rustix::io::read(self.clock_set.as_fd(), &mut count);
            watch_clock(&self.clock_set)?;
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

/// Arms `timer`, on `CLOCK_REALTIME`, at the clock's last time, which it never reaches, so that the
/// kernel cancels it when the wall clock moves against `CLOCK_MONOTONIC`.
fn watch_clock(timer: &OwnedFd) -> io::Result<()> {
    let never = Timespec {
        tv_sec: i64::MAX,
        tv_nsec: 0,
    };
    timerfd_settime(
        timer,
        TimerfdTimerFlags::ABSTIME | TimerfdTimerFlags::CANCEL_ON_SET,
        &Itimerspec {
            it_interval: Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: never,
        },
    )?;
    Ok(())
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

    #[test]
    fn the_notice_of_a_resume_watches_the_wall_clock_and_stays_armed_after_a_wait() {
        let mut waits = Waits::new().expect("the waits");
        let watched = |waits: &Waits| {
            let info = fdinfo(&waits.clock_set);
            // `CLOCK_REALTIME`, armed absolute and cancelled when the clock is set: the kernel's
            // `TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET`, at a time that it never reaches.
            assert!(info.contains("clockid: 0\n"), "{info}");
            assert!(info.contains("settime flags: 03\n"), "{info}");
            assert!(!info.contains("it_value: (0, 0)"), "{info}");
        };
        watched(&waits);
        // A wait that the deadline ends leaves it armed, and is not told as a clock set.
        let display = display();
        assert!(
            !waits
                .wait(display.as_fd(), Some(Instant::from_nanos(1)), false)
                .expect("waited")
        );
        watched(&waits);
    }
}
