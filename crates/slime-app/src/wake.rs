//! What wakes a run whose frames run on demand ([`AppConfig::on_demand`]): a [`Waker`], which any
//! thread holds, and the run's own, which a termination signal and the end of the tool that started
//! the program wake (`docs/adr/0156-the-editors-window-and-its-operations.md`).
//!
//! [`AppConfig::on_demand`]: crate::AppConfig::on_demand

use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};

/// Asks a run for a frame from any thread: a load or a cook that ended, a request that came. A run
/// whose frames run on demand ([`AppConfig::on_demand`](crate::AppConfig::on_demand)) runs one
/// soon after, paced as its frames are; any other run runs its frames anyway, and a wake changes
/// nothing for it. Clones wake the same run; one that ended wakes for nothing.
/// [`Context::waker`](crate::Context::waker) gives the run's.
#[derive(Clone, Debug, Default)]
pub struct Waker {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// Whether a frame was asked for since the run last took the asks.
    asked: Mutex<bool>,
    /// Notified at each wake, which a run without a window waits on.
    woken: Condvar,
    /// The window's event loop, which a windowed run waits in.
    window: OnceLock<slime_platform::Waker>,
}

impl Waker {
    /// Asks the run for a frame.
    pub fn wake(&self) {
        *self
            .inner
            .asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
        self.inner.woken.notify_all();
        if let Some(window) = self.inner.window.get() {
            window.wake();
        }
    }

    /// Whether a frame was asked for since the last call, which forgets it.
    pub(crate) fn take(&self) -> bool {
        std::mem::take(
            &mut *self
                .inner
                .asked
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// Waits until a frame is asked for, which it leaves asked, unless one was already.
    pub(crate) fn wait(&self) {
        let asked = self
            .inner
            .asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        drop(
            self.inner
                .woken
                .wait_while(asked, |asked| !*asked)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// Wakes the event loop of a windowed run too, from now on.
    pub(crate) fn set_window(&self, window: slime_platform::Waker) {
        let _ = self.inner.window.set(window);
    }
}

/// The waker of the run in progress, which what the whole process hears wakes: a termination signal
/// and the end of the tool that started the program.
static CURRENT: Mutex<Option<Waker>> = Mutex::new(None);

/// Makes `waker` the run's, or none once the run ends.
pub(crate) fn set_current(waker: Option<Waker>) {
    *CURRENT.lock().unwrap_or_else(PoisonError::into_inner) = waker;
}

/// Wakes the run in progress, if one runs, so that it notices what the whole process heard.
pub(crate) fn wake_current() {
    if let Some(waker) = &*CURRENT.lock().unwrap_or_else(PoisonError::into_inner) {
        waker.wake();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_wake_from_another_thread_ends_the_wait_and_is_taken_once() {
        let waker = Waker::default();
        assert!(!waker.take());
        let other = waker.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            other.wake();
        });
        waker.wait();
        thread.join().expect("the thread woke the run");
        assert!(waker.take());
        assert!(!waker.take());
        // A wake before the wait ends it at once.
        waker.wake();
        waker.wait();
        assert!(waker.take());
    }
}
