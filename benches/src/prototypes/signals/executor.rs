//! The interface's executor, written by hand for the prototypes on the standard library: tasks
//! that run on the interface's thread, each owned by the scope that started it and cancelled when
//! the scope is disposed; timers on the frame's clock, which the bench moves; and the host's
//! waker, which a task woken outside a frame, from another thread, asks a frame through.
//!
//! The executor runs within the frame, never between frames: `advance` gives it the frame's time
//! and wakes the timers that are due; the build polls each task woken, once a frame at most. The
//! host waits for its next deadline itself, as winit's `WaitUntil` does: no thread of the
//! executor's own sleeps until a timer.
//!
//! A frame at rest, no task woken and no timer due, polls nothing and allocates nothing: an
//! empty channel read and a map's first key.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

/// A task's future.
type TaskFuture = Pin<Box<dyn Future<Output = ()>>>;

/// A task: its place among the executor's, and the generation of that place, so that the waker
/// of a task that ended wakes no other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TaskId {
    index: u32,
    generation: u32,
}

/// What wakes the host from any thread, as the engine's `slime_app::Waker` does: a frame asked
/// for, which the host takes, and a wait that ends when one is.
#[derive(Debug, Default)]
pub(crate) struct HostWake {
    asked: Mutex<bool>,
    woken: Condvar,
    /// The wakes since the host was opened.
    wakes: AtomicUsize,
}

impl HostWake {
    /// Asks the host for a frame.
    pub(crate) fn wake(&self) {
        self.wakes.fetch_add(1, Ordering::Relaxed);
        *self.asked.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.woken.notify_all();
    }

    /// Waits until a frame is asked for, then forgets the ask.
    pub(crate) fn wait(&self) {
        let asked = self.asked.lock().unwrap_or_else(PoisonError::into_inner);
        let mut asked = self
            .woken
            .wait_while(asked, |asked| !*asked)
            .unwrap_or_else(PoisonError::into_inner);
        *asked = false;
    }

    /// The wakes since the host was opened.
    pub(crate) fn wakes(&self) -> usize {
        self.wakes.load(Ordering::Relaxed)
    }
}

/// What the wakers of every task share with the executor, on any thread.
struct Woken {
    /// The tasks woken, which the executor polls at its frame: a channel whose sends and reads
    /// never wait for another thread, so that the interface's thread never waits.
    ready: mpsc::Sender<TaskId>,
    /// Whether a frame runs, or the host was asked for one since the executor's last frame
    /// ended: a task woken then asks none.
    asked: AtomicBool,
    host: Arc<HostWake>,
}

/// A task's waker: its id, and whether it waits in the channel already, so that a task woken a
/// thousand times between two frames is sent once.
struct TaskWaker {
    id: TaskId,
    scheduled: AtomicBool,
    woken: Arc<Woken>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if self.scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        // The receiver lives as long as the executor; a send to one dropped wakes nothing.
        if self.woken.ready.send(self.id).is_ok() && !self.woken.asked.swap(true, Ordering::AcqRel)
        {
            self.woken.host.wake();
        }
    }
}

struct TaskSlot {
    generation: u32,
    /// The task's future, out of its slot while it is polled.
    future: Option<TaskFuture>,
    /// Its waker, made once as it is spawned: a poll allocates none.
    waker: Option<(Arc<TaskWaker>, Waker)>,
    /// The frame that last polled it.
    polled: u64,
}

/// The timers waiting, by their deadlines and the order they were made in.
type Timers = BTreeMap<(Instant, u64), Waker>;

/// The interface's executor, of one window's application. It lives across reloads: a waker of a
/// task that a reload cancelled names a generation that no task holds.
pub(crate) struct Executor {
    slots: RefCell<Vec<TaskSlot>>,
    free: RefCell<Vec<u32>>,
    woken: Arc<Woken>,
    ready: mpsc::Receiver<TaskId>,
    /// The tasks taken from the channel that wait for their poll.
    queue: RefCell<VecDeque<TaskId>>,
    /// The tasks woken again in the frame that polled them, for the next frame.
    later: RefCell<Vec<TaskId>>,
    frame: Cell<u64>,
    now: Cell<Instant>,
    timers: RefCell<Timers>,
    next_timer: Cell<u64>,
    /// The tasks polled, since last counted.
    pub(crate) polls: Cell<usize>,
}

impl Executor {
    /// An executor whose first frame is at `now`, which asks `host` for the frames that tasks
    /// woken outside a frame need.
    pub(crate) fn new(now: Instant, host: Arc<HostWake>) -> Rc<Self> {
        let (sender, ready) = mpsc::channel();
        Rc::new(Self {
            slots: RefCell::new(Vec::new()),
            free: RefCell::new(Vec::new()),
            woken: Arc::new(Woken {
                ready: sender,
                asked: AtomicBool::new(false),
                host,
            }),
            ready,
            queue: RefCell::new(VecDeque::new()),
            later: RefCell::new(Vec::new()),
            frame: Cell::new(0),
            now: Cell::new(now),
            timers: RefCell::new(BTreeMap::new()),
            next_timer: Cell::new(0),
            polls: Cell::new(0),
        })
    }

    /// Starts `future` as a task, polled first in the frame that runs now, or the next.
    pub(crate) fn spawn(&self, future: TaskFuture) -> TaskId {
        let mut slots = self.slots.borrow_mut();
        let index = match self.free.borrow_mut().pop() {
            Some(index) => index,
            None => {
                slots.push(TaskSlot {
                    generation: 0,
                    future: None,
                    waker: None,
                    polled: u64::MAX,
                });
                (slots.len() - 1) as u32
            }
        };
        let slot = &mut slots[index as usize];
        let id = TaskId {
            index,
            generation: slot.generation,
        };
        let task_waker = Arc::new(TaskWaker {
            id,
            scheduled: AtomicBool::new(true),
            woken: Arc::clone(&self.woken),
        });
        let waker = Waker::from(Arc::clone(&task_waker));
        slot.future = Some(future);
        slot.waker = Some((task_waker, waker));
        slot.polled = u64::MAX;
        self.queue.borrow_mut().push_back(id);
        id
    }

    /// Cancels `id`: its future dropped, and what it holds with it. A task that ended or was
    /// cancelled already is left alone.
    pub(crate) fn cancel(&self, id: TaskId) {
        let future = {
            let mut slots = self.slots.borrow_mut();
            let Some(slot) = slots.get_mut(id.index as usize) else {
                return;
            };
            if slot.generation != id.generation {
                return;
            }
            slot.generation += 1;
            slot.waker = None;
            self.free.borrow_mut().push(id.index);
            slot.future.take()
        };
        // Dropped once the slots are free: what it holds may reach the executor as it goes, a
        // timer forgetting its deadline.
        drop(future);
    }

    /// Starts the frame at `now`: the tasks woken again in the frame before, and those whose
    /// timers are due, wait for their poll.
    pub(crate) fn advance(&self, now: Instant) {
        // The frame runs: a task woken until it ends needs no frame of the host's.
        self.woken.asked.store(true, Ordering::Release);
        self.frame.set(self.frame.get() + 1);
        self.now.set(now);
        self.queue
            .borrow_mut()
            .extend(self.later.borrow_mut().drain(..));
        loop {
            let due = {
                let mut timers = self.timers.borrow_mut();
                match timers.first_key_value() {
                    Some((&(at, _), _)) if at <= now => timers.pop_first(),
                    _ => None,
                }
            };
            let Some((_, waker)) = due else {
                break;
            };
            waker.wake();
        }
    }

    /// Polls each task woken that this frame has not polled yet; gives how many it polled. A task
    /// woken again in this frame waits for the next.
    pub(crate) fn run_ready(&self) -> usize {
        while let Ok(id) = self.ready.try_recv() {
            self.queue.borrow_mut().push_back(id);
        }
        let frame = self.frame.get();
        let mut polled = 0;
        loop {
            let Some(id) = self.queue.borrow_mut().pop_front() else {
                break;
            };
            let taken = {
                let mut slots = self.slots.borrow_mut();
                match slots.get_mut(id.index as usize) {
                    Some(slot) if slot.generation == id.generation => {
                        let (task_waker, waker) = slot.waker.as_ref().expect("a task's waker");
                        if slot.polled == frame {
                            self.later.borrow_mut().push(id);
                            None
                        } else {
                            slot.polled = frame;
                            // Woken from now on again, for the next poll.
                            task_waker.scheduled.store(false, Ordering::Release);
                            Some((slot.future.take(), waker.clone()))
                        }
                    }
                    _ => None,
                }
            };
            let Some((Some(mut future), waker)) = taken else {
                continue;
            };
            polled += 1;
            let done = future
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_ready();
            let mut slots = self.slots.borrow_mut();
            let slot = &mut slots[id.index as usize];
            // A task that its own poll cancelled, closing its own page, is dropped here.
            if slot.generation != id.generation {
                drop(slots);
                drop(future);
                continue;
            }
            if done {
                slot.generation += 1;
                slot.waker = None;
                self.free.borrow_mut().push(id.index);
                drop(slots);
                drop(future);
            } else {
                slot.future = Some(future);
            }
        }
        self.polls.set(self.polls.get() + polled);
        polled
    }

    /// Ends the frame: a task woken from now on asks the host for a frame. Whether work waits for
    /// the next frame: a task woken again in this one, or woken as it ended.
    pub(crate) fn end_frame(&self) -> bool {
        self.woken.asked.store(false, Ordering::Release);
        while let Ok(id) = self.ready.try_recv() {
            self.queue.borrow_mut().push_back(id);
        }
        self.waits()
    }

    /// Whether tasks wait for their poll in the next frame.
    pub(crate) fn waits(&self) -> bool {
        !self.queue.borrow().is_empty() || !self.later.borrow().is_empty()
    }

    /// The deadline of the first timer: when the host must run a frame, though nothing asks one.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.timers
            .borrow()
            .first_key_value()
            .map(|(&(at, _), _)| at)
    }

    /// The tasks alive.
    #[cfg(test)]
    pub(crate) fn tasks(&self) -> usize {
        self.slots.borrow().len() - self.free.borrow().len()
    }
}

/// A future that ends once the frame's clock reaches its deadline.
pub(crate) struct Sleep {
    at: Instant,
    /// Its place among the timers, once it waits.
    key: Option<(Instant, u64)>,
    executor: Rc<Executor>,
}

impl Sleep {
    /// Ends `duration` after the frame that runs now, on the frame's clock.
    pub(crate) fn new(executor: Rc<Executor>, duration: Duration) -> Self {
        Self {
            at: executor.now.get() + duration,
            key: None,
            executor,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.executor.now.get() >= self.at {
            if let Some(key) = self.key.take() {
                self.executor.timers.borrow_mut().remove(&key);
            }
            return Poll::Ready(());
        }
        let mut timers = self.executor.timers.borrow_mut();
        match self.key {
            // The executor's wakers do not change: a timer that waits keeps its own.
            Some(key) if timers.contains_key(&key) => {}
            _ => {
                let order = self.executor.next_timer.get();
                self.executor.next_timer.set(order + 1);
                let key = (self.at, order);
                timers.insert(key, context.waker().clone());
                drop(timers);
                self.key = Some(key);
            }
        }
        Poll::Pending
    }
}

/// A timer dropped with its task forgets its deadline: the host is not woken for it.
impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            self.executor.timers.borrow_mut().remove(&key);
        }
    }
}
