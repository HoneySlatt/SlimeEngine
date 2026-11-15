//! What other threads ask of the host's loop, its wakes and its end: part of the host's core.
//!
//! The interface's waker ([`Ui::set_waker`](uwui::tree::Ui::set_waker)) and the handler of the
//! termination signals, SIGTERM, SIGINT and SIGHUP (`ctrlc`), set a flag and *notify* the loop
//! once, the platform's way: an eventfd written on Linux. Wakes that come before the loop takes
//! them notify it once, so that a thread that wakes it a thousand times while it draws costs one
//! step.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Wake, Waker};

/// What the loop is asked from other threads, and how they notify it.
pub(crate) struct Asks {
    /// A wake asked since the loop last took them.
    woken: AtomicBool,
    /// A termination signal came since the loop last took it.
    terminated: AtomicBool,
    /// Wakes the loop, from any thread, without blocking.
    notify: Box<dyn Fn() + Send + Sync>,
}

impl Asks {
    /// What other threads ask, which `notify` tells the loop.
    pub(crate) fn new(notify: impl Fn() + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            woken: AtomicBool::new(false),
            terminated: AtomicBool::new(false),
            notify: Box::new(notify),
        })
    }

    /// The waker that the interface gives the program's threads.
    pub(crate) fn waker(self: &Arc<Self>) -> Waker {
        Waker::from(Arc::clone(self))
    }

    /// A termination signal came: the loop asks the application whether it closes.
    pub(crate) fn terminate(&self) {
        if !self.terminated.swap(true, Ordering::AcqRel) {
            (self.notify)();
        }
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
/// it closes. A process has one handler of them: when another part of the program set it first,
/// the host says so, and the signals end the process as their default does.
pub(crate) fn on_termination(asks: &Arc<Asks>) {
    let asks = Arc::clone(asks);
    if let Err(error) = ctrlc::set_handler(move || asks.terminate()) {
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
        asks.terminate();
        asks.terminate();
        assert_eq!(count.load(Ordering::Relaxed), 1);
        assert!(!asks.take_woken());
        assert!(asks.take_terminated());
        assert!(!asks.take_terminated());
    }
}
