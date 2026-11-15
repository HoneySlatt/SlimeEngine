//! The loop's wait: one `poll` on the compositor's socket, an eventfd that other threads write to
//! wake it, and a timerfd on `CLOCK_BOOTTIME`, the host's clock, for its deadline. A deadline
//! armed before a suspend ends on time after it, which the timer's clock counts.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

use rustix::event::{EventfdFlags, PollFd, PollFlags, eventfd, poll};
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};
use uwui::time::Instant;

/// The eventfd that wakes the loop, and its timer.
pub(super) struct Waits {
    wake: Arc<OwnedFd>,
    timer: OwnedFd,
    /// The deadline that the timer holds, if it is armed.
    armed: Option<Instant>,
}

impl Waits {
    pub(super) fn new() -> io::Result<Self> {
        Ok(Self {
            wake: Arc::new(eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?),
            timer: timerfd_create(
                TimerfdClockId::Boottime,
                TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
            )?,
            armed: None,
        })
    }

    /// What wakes the loop from any thread, without blocking: one write to the eventfd.
    pub(super) fn notifier(&self) -> impl Fn() + Send + Sync + 'static {
        let wake = Arc::clone(&self.wake);
        move || {
            // Its counter full, at 2⁶⁴ − 2 writes not read, the loop is woken already.
            let _ = rustix::io::write(&*wake, &1_u64.to_ne_bytes());
        }
    }

    /// Waits until `wayland` can be read, the loop is woken, or `deadline` passes; none for no
    /// deadline. Answers whether `wayland` can be read, or ended.
    pub(super) fn wait(
        &mut self,
        wayland: BorrowedFd<'_>,
        deadline: Option<Instant>,
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
            PollFd::new(&wayland, PollFlags::IN),
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
        let [wayland, wake, timer] = fds.map(|fd| fd.revents());
        let mut count = [0_u8; 8];
        if wake.contains(PollFlags::IN) {
            let _ = rustix::io::read(self.wake.as_fd(), &mut count);
        }
        if timer.contains(PollFlags::IN) {
            let _ = rustix::io::read(self.timer.as_fd(), &mut count);
            // A timer that fired is disarmed.
            self.armed = None;
        }
        Ok(!wayland.is_empty())
    }
}
