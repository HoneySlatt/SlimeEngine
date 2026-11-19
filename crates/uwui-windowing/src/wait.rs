//! The loop's wait on Linux, which Wayland's and X11's layers share: one `poll` on the display's
//! socket, an eventfd that other threads write to wake it, and a timerfd on `CLOCK_BOOTTIME`, the
//! host's clock, for its deadline. A deadline armed before a suspend ends on time after it, which
//! the timer's clock counts.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};
use uwui_time::Instant;

/// The eventfd that wakes the loop, and its timer.
pub(crate) struct Waits {
    wake: Arc<OwnedFd>,
    timer: OwnedFd,
    /// The deadline that the timer holds, if it is armed.
    armed: Option<Instant>,
    /// Whether the last wait ended for a wake or the deadline, beside the display, if at all.
    interrupted: bool,
}

impl Waits {
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            wake: Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?),
            timer: timerfd_create(
                TimerfdClockId::Boottime,
                TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
            )?,
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
        ];
        loop {
            match poll(&mut fds, None) {
                Ok(_) => break,
                // A termination signal's handler ran on this thread: its wake is the eventfd's.
                Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let [display, wake, timer] = fds.map(|fd| fd.revents());
        let mut count = [0_u8; 8];
        if wake.contains(PollFlags::IN) {
            let _ = rustix::io::read(self.wake.as_fd(), &mut count);
        }
        if timer.contains(PollFlags::IN) {
            let _ = rustix::io::read(self.timer.as_fd(), &mut count);
            // A timer that fired is disarmed.
            self.armed = None;
        }
        self.interrupted = wake.contains(PollFlags::IN) || timer.contains(PollFlags::IN);
        Ok(display.intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP))
    }

    /// Whether the last wait ended for a wake from another thread or a termination signal, or
    /// for its deadline, whether or not the display could be read too: the loop must turn, even
    /// where what it read from the display tells nothing.
    pub(crate) fn interrupted(&self) -> bool {
        self.interrupted
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
}
