//! What other threads ask of the layer's loop, its wakes and its end: part of the layer's core.
//!
//! The waker that the host gives its interface ([`crate::Window::waker`]) and the handler of the
//! termination signals, SIGTERM, SIGINT and SIGHUP (`ctrlc`), set a flag and *notify* the loop
//! once, the platform's way: an eventfd written on Linux. Wakes that come before the loop takes
//! them notify it once, so that a thread that wakes it a thousand times while it draws costs one
//! step. A second termination signal, half a second or more after the first, ends the process at
//! once, with [`EXIT_TERMINATED`](crate::EXIT_TERMINATED).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Wake, Waker};
use std::time::{Duration, Instant};

/// Signals that come within this time of the first repeat it rather than insist: a terminal that
/// closes sends SIGHUP twice, from the shell and then from the kernel, less than a millisecond
/// apart; the engine's own host waits as long (`crates/slime-app/src/signal.rs`).
const REPEAT_WINDOW: Duration = Duration::from_millis(500);

/// What a termination signal asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Termination {
    /// The first: the loop asks the application whether it closes.
    Ask,
    /// One that repeats the first, within half a second of it: nothing more.
    Repeat,
    /// One that insists: the process ends at once.
    End,
}

/// What the loop is asked from other threads, and how they notify it.
pub(crate) struct Asks {
    /// A wake asked since the loop last took them.
    woken: AtomicBool,
    /// A termination signal came since the loop last took it.
    terminated: AtomicBool,
    /// When the first termination signal came.
    first_termination: OnceLock<Instant>,
    /// Wakes the loop, from any thread, without blocking.
    notify: Box<dyn Fn() + Send + Sync>,
}

impl Asks {
    /// What other threads ask, which `notify` tells the loop.
    pub(crate) fn new(notify: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            woken: AtomicBool::new(false),
            terminated: AtomicBool::new(false),
            first_termination: OnceLock::new(),
            notify: Box::new(notify),
        })
    }

    /// The waker that the interface gives the program's threads.
    pub(crate) fn waker(self: &Arc<Self>) -> Waker {
        Waker::from(Arc::clone(self))
    }

    /// A termination signal came at `now`: the first asks the loop to ask the application whether
    /// it closes; what a later one asks is answered.
    pub(crate) fn terminate(&self, now: Instant) -> Termination {
        if let Some(first) = self.first_termination.get() {
            return if now.saturating_duration_since(*first) >= REPEAT_WINDOW {
                Termination::End
            } else {
                Termination::Repeat
            };
        }
        // The signals come on one thread, so nothing sets it between the two calls.
        let _ = self.first_termination.set(now);
        if !self.terminated.swap(true, Ordering::AcqRel) {
            (self.notify)();
        }
        Termination::Ask
    }

    /// Whether a wake was asked since the last call.
    pub(crate) fn take_woken(&self) -> bool {
        self.woken.swap(false, Ordering::AcqRel)
    }

    /// Whether a termination signal came since the last call.
    pub(crate) fn take_terminated(&self) -> bool {
        self.terminated.swap(false, Ordering::AcqRel)
    }
}

impl Wake for Asks {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if !self.woken.swap(true, Ordering::AcqRel) {
            (self.notify)();
        }
    }
}

/// Ends the run on SIGTERM, SIGINT and SIGHUP through `asks`, which asks the application whether
/// it closes; a second signal, half a second or more after the first, ends the process at once, for
/// an application that keeps its window or no longer returns to the loop. A process has one
/// handler of them: when another part of the program set it first, the host says so, and the
/// signals end the process as their default does.
pub(crate) fn on_termination(asks: &Arc<Asks>) {
    let asks = Arc::clone(asks);
    let handled = ctrlc::set_handler(move || {
        if asks.terminate(Instant::now()) == Termination::End {
            // Nothing written first: a standard error that nobody reads could block the exit.
            std::process::exit(crate::EXIT_TERMINATED);
        }
    });
    if let Err(error) = handled {
        tracing::warn!(%error, "the termination signals end the program without asking it");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    fn counted() -> (Arc<Asks>, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let notified = Arc::clone(&count);
        let asks = Asks::new(move || {
            notified.fetch_add(1, Ordering::Relaxed);
        });
        (asks, count)
    }

    #[test]
    fn wakes_before_the_loop_takes_them_notify_it_once() {
        let (asks, count) = counted();
        let waker = asks.waker();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let waker = waker.clone();
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        waker.wake_by_ref();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().expect("the thread ends");
        }
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert!(asks.take_woken());
        assert!(!asks.take_woken());
        // Taken, the next wake notifies again.
        waker.wake();
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn a_termination_is_told_apart_from_a_wake() {
        let (asks, count) = counted();
        let now = Instant::now();
        assert_eq!(asks.terminate(now), Termination::Ask);
        assert_eq!(asks.terminate(now), Termination::Repeat);
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert!(!asks.take_woken());
        assert!(asks.take_terminated());
        assert!(!asks.take_terminated());
    }

    #[test]
    fn a_second_termination_ends_the_process_once_it_insists() {
        let (asks, _) = counted();
        let first = Instant::now();
        assert_eq!(asks.terminate(first), Termination::Ask);
        assert_eq!(
            asks.terminate(first + REPEAT_WINDOW - Duration::from_millis(1)),
            Termination::Repeat
        );
        assert_eq!(asks.terminate(first + REPEAT_WINDOW), Termination::End);
        // Once taken by the loop, a later signal still insists.
        assert!(asks.take_terminated());
        assert_eq!(
            asks.terminate(first + Duration::from_secs(9)),
            Termination::End
        );
    }
}
