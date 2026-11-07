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
//!
//! A task takes at most [`MESSAGES_PER_POLL`] values from channels in one poll, then waits for
//! the next frame: a sender faster than the interface slows the frames, never stops them.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering, fence};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

// What the task that runs may still receive in its poll, held apart from the bench's crate so that
// dx's patch of it reads the running program's (ADR-0212).
use slime_bench_locals::{budget, set_budget};

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
    state: Mutex<HostState>,
    woken: Condvar,
    /// The wakes since the host was opened.
    wakes: AtomicUsize,
}

#[derive(Debug, Default)]
struct HostState {
    /// Whether a frame was asked for since the host last took the ask.
    asked: bool,
    /// Whether the host waits.
    waiting: bool,
    /// The wakes that found the host waiting.
    found_waiting: usize,
}

impl HostWake {
    /// Asks the host for a frame.
    pub(crate) fn wake(&self) {
        self.wakes.fetch_add(1, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.asked = true;
        if state.waiting {
            state.found_waiting += 1;
        }
        self.woken.notify_all();
    }

    /// Waits until a frame is asked for, then forgets the ask.
    pub(crate) fn wait(&self) {
        self.wait_at_most(Duration::MAX);
    }

    /// Waits until a frame is asked for, at most `limit`; whether one was, which it forgets.
    pub(crate) fn wait_at_most(&self, limit: Duration) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.waiting = true;
        let (mut state, _) = self
            .woken
            .wait_timeout_while(state, limit, |state| !state.asked)
            .unwrap_or_else(PoisonError::into_inner);
        state.waiting = false;
        std::mem::take(&mut state.asked)
    }

    /// The wakes since the host was opened.
    pub(crate) fn wakes(&self) -> usize {
        self.wakes.load(Ordering::Relaxed)
    }

    /// The wakes that found the host waiting, rather than working.
    pub(crate) fn found_waiting(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .found_waiting
    }
}

/// The values that a task may take from channels in one poll: a budget that each poll starts
/// with, so that a poll never runs as long as a sender sends.
pub(crate) const MESSAGES_PER_POLL: usize = 16_384;

/// Spends one value of the budget of the task that runs: false when none is left, and the task
/// must wait for the next frame.
pub(crate) fn spend_budget() -> bool {
    match budget() {
        0 => false,
        left => {
            set_budget(left - 1);
            true
        }
    }
}

/// What the wakers of every task share with the executor, on any thread.
struct Woken {
    /// The tasks woken, which the executor polls at its frame: the standard library's channel,
    /// whose sends and reads take no lock. A read may spin briefly behind a send caught midway.
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
        if self.woken.ready.send(self.id).is_err() {
            return;
        }
        // Paired with `end_frame`'s: either it reads this id, or this reads the flag it cleared.
        fence(Ordering::SeqCst);
        if !self.woken.asked.swap(true, Ordering::SeqCst) {
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

/// The frame's clock, and the timers waiting on it by their deadlines and the order they were
/// made in: apart from the executor, which the timers' futures, held by its tasks, must not hold.
pub(crate) struct Clock {
    now: Cell<Instant>,
    timers: RefCell<BTreeMap<(Instant, u64), Waker>>,
    next_timer: Cell<u64>,
}

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
    clock: Rc<Clock>,
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
            clock: Rc::new(Clock {
                now: Cell::new(now),
                timers: RefCell::new(BTreeMap::new()),
                next_timer: Cell::new(0),
            }),
            polls: Cell::new(0),
        })
    }

    /// The frame's clock, which timers wait on.
    pub(crate) fn clock(&self) -> Rc<Clock> {
        Rc::clone(&self.clock)
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
            slot.generation = slot.generation.wrapping_add(1);
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
        self.clock.now.set(now);
        self.queue
            .borrow_mut()
            .extend(self.later.borrow_mut().drain(..));
        loop {
            let due = {
                let mut timers = self.clock.timers.borrow_mut();
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
            set_budget(MESSAGES_PER_POLL);
            let done = future
                .as_mut()
                .poll(&mut Context::from_waker(&waker))
                .is_ready();
            set_budget(usize::MAX);
            let mut slots = self.slots.borrow_mut();
            let slot = &mut slots[id.index as usize];
            // A task that its own poll cancelled, closing its own page, is dropped here.
            if slot.generation != id.generation {
                drop(slots);
                drop(future);
                continue;
            }
            if done {
                slot.generation = slot.generation.wrapping_add(1);
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
        self.woken.asked.store(false, Ordering::SeqCst);
        // Paired with the wakers': either a task woken now asks the host, or this reads its id.
        fence(Ordering::SeqCst);
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
    /// In uwui, it joins `Ui::next_change`, which hosts already wait for.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.clock
            .timers
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
    clock: Rc<Clock>,
}

impl Sleep {
    /// Ends `duration` after the frame that runs now, on `clock`.
    pub(crate) fn new(clock: Rc<Clock>, duration: Duration) -> Self {
        Self {
            at: clock.now.get() + duration,
            key: None,
            clock,
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        let this = &mut *self;
        let mut timers = this.clock.timers.borrow_mut();
        if this.clock.now.get() >= this.at {
            if let Some(key) = this.key.take() {
                timers.remove(&key);
            }
            return Poll::Ready(());
        }
        match this.key.and_then(|key| timers.get_mut(&key)) {
            Some(held) => {
                if !held.will_wake(context.waker()) {
                    *held = context.waker().clone();
                }
            }
            None => {
                let order = this.clock.next_timer.get();
                this.clock.next_timer.set(order + 1);
                let key = (this.at, order);
                timers.insert(key, context.waker().clone());
                this.key = Some(key);
            }
        }
        Poll::Pending
    }
}

/// A timer dropped with its task forgets its deadline: the host is not woken for it.
impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            self.clock.timers.borrow_mut().remove(&key);
        }
    }
}
