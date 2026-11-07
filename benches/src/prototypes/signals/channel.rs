//! A channel from any thread to the interface's tasks: what other threads change signals
//! through. They hold its [`Sender`], never a signal; a task of a view receives, and sets the
//! signals. A burst sent between two frames is received in one poll of the task, in the next
//! frame, up to the executor's budget of values a poll, and its bindings run once.
//!
//! The application makes the channel before its view, and gives the view an `Rc` of its
//! [`Receiver`], which reads through `&self`: a reload disposes the view and its task, the new
//! view's task reads the same receiver, and what was sent meanwhile waits for it. A channel made
//! within a view closes with it, and its senders' sends fail. One task receives at a time: a
//! second one's waker takes the first's place.
//!
//! The interface's thread never waits to receive. The values go through the standard library's
//! channel, which takes no lock, though a read may spin briefly behind a send caught midway. The
//! receiver takes its waker's lock with `try_lock`: a task that finds it held, by a sender waking
//! it, wakes itself for the next frame. A sender takes that lock to wake the waker, which a send
//! from the interface's thread may wait for, behind another sender. The queue is unbounded: while
//! the host runs no frame, what is sent waits in memory.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering, fence};
use std::sync::{Arc, Mutex, TryLockError, mpsc};
use std::task::{Context, Poll, Waker};

use super::executor::spend_budget;

/// The receiver's waker, which a send wakes.
#[derive(Default)]
struct Wakes {
    waker: Mutex<Option<Waker>>,
    /// Whether a send woke the receiver since it last found the channel empty: a burst wakes it
    /// once.
    notified: AtomicBool,
    /// The senders alive: the last one dropped wakes the receiver, which finds the channel closed.
    senders: AtomicUsize,
}

impl Wakes {
    /// Wakes the receiver after a value was sent, or the last sender dropped.
    fn wake(&self) {
        // Paired with `Recv::poll`'s: either the receiver reads what was sent, or this reads the
        // flag that it cleared.
        fence(Ordering::SeqCst);
        if self.notified.swap(true, Ordering::SeqCst) {
            return;
        }
        let held = self.waker.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(waker) = &*held {
            waker.wake_by_ref();
        }
    }
}

/// Sends values to the interface's tasks, from any thread. Clones send to the same receiver.
pub struct Sender<T> {
    /// Taken as it is dropped, before the receiver is woken: it finds the channel closed.
    values: Option<mpsc::Sender<T>>,
    wakes: Arc<Wakes>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.wakes.senders.fetch_add(1, Ordering::Relaxed);
        Self {
            values: self.values.clone(),
            wakes: Arc::clone(&self.wakes),
        }
    }
}

/// The value that a closed channel did not take: its receiver was dropped.
#[derive(Debug, PartialEq, Eq)]
pub struct Closed<T>(pub T);

impl<T> Sender<T> {
    /// Sends `value`, which the receiver's task takes in the next frame; waits only behind
    /// another sender waking the receiver.
    ///
    /// # Errors
    ///
    /// [`Closed`], with `value`, once the receiver was dropped.
    pub fn send(&self, value: T) -> Result<(), Closed<T>> {
        let values = self.values.as_ref().expect("taken only as it is dropped");
        values.send(value).map_err(|error| Closed(error.0))?;
        self.wakes.wake();
        Ok(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        drop(self.values.take());
        // The last sender gone, the receiver's task learns that the channel closed.
        if self.wakes.senders.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.wakes.notified.store(false, Ordering::SeqCst);
            self.wakes.wake();
        }
    }
}

/// Receives on the interface's thread what other threads sent.
pub struct Receiver<T> {
    values: mpsc::Receiver<T>,
    wakes: Arc<Wakes>,
}

/// A channel whose values go from any thread to the interface's tasks.
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (sender, values) = mpsc::channel();
    let wakes = Arc::new(Wakes {
        senders: AtomicUsize::new(1),
        ..Wakes::default()
    });
    (
        Sender {
            values: Some(sender),
            wakes: Arc::clone(&wakes),
        },
        Receiver { values, wakes },
    )
}

impl<T> Receiver<T> {
    /// The next value sent, or `None` once every sender is dropped and none waits. A value taken
    /// is the task's: one held across another `.await` is lost if the task is cancelled.
    pub fn recv(&self) -> Recv<'_, T> {
        Recv { receiver: self }
    }

    /// The next value waiting now, without waiting.
    fn try_take(&self) -> Option<Poll<Option<T>>> {
        match self.values.try_recv() {
            Ok(value) => Some(Poll::Ready(Some(value))),
            Err(mpsc::TryRecvError::Disconnected) => Some(Poll::Ready(None)),
            Err(mpsc::TryRecvError::Empty) => None,
        }
    }
}

/// The future of [`Receiver::recv`].
pub struct Recv<'r, T> {
    receiver: &'r Receiver<T>,
}

impl<T> Future for Recv<'_, T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<T>> {
        // The task's budget of values spent, the rest waits for the next frame.
        if !spend_budget() {
            context.waker().wake_by_ref();
            return Poll::Pending;
        }
        let receiver = self.receiver;
        if let Some(taken) = receiver.try_take() {
            return taken;
        }
        match receiver.wakes.waker.try_lock() {
            Ok(mut held) => {
                if !held
                    .as_ref()
                    .is_some_and(|held| held.will_wake(context.waker()))
                {
                    *held = Some(context.waker().clone());
                }
            }
            Err(TryLockError::Poisoned(error)) => {
                *error.into_inner() = Some(context.waker().clone());
            }
            // A sender holds it to wake the waker before: this task wakes itself instead, for
            // the next frame, rather than wait for that thread.
            Err(TryLockError::WouldBlock) => context.waker().wake_by_ref(),
        }
        // A send from now on wakes the waker; one before is read below.
        receiver.wakes.notified.store(false, Ordering::SeqCst);
        fence(Ordering::SeqCst);
        receiver.try_take().unwrap_or(Poll::Pending)
    }
}
