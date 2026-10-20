#![doc = include_str!("../README.md")]

use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;

/// How soon a task runs, against the others of the pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Priority {
    /// Work that the frame being made waits for, such as a sky's environment: a free thread takes
    /// a frame task before any background task.
    Frame,
    /// Work that no frame waits for, such as loads and cooking. Background tasks never hold every
    /// thread of a pool that has more than one, so that a frame task always finds a thread.
    Background,
}

/// Runs `task` on a thread of the engine's pool, as soon as one is free for its priority: a frame
/// task before every background task queued, background tasks in the order they were given, on
/// every thread but one. The first call starts the pool's threads, [`threads`] of them, which live
/// as long as the process and wait for tasks while there are none.
///
/// A task must not wait for another task, which could be queued behind it; [`map`] waits, and
/// takes its own items while it does. A task that panics ends there, not its thread: the panic is
/// reported as any other, and whoever waits for the task's result hears of it as it arranges, as
/// [`map`] does.
///
/// # Panics
///
/// If the operating system cannot start the pool's threads.
pub fn run(priority: Priority, task: impl FnOnce() + Send + 'static) {
    pool().run(priority, Box::new(task));
}

/// `item(i)` for each `i` from 0 to `count`, in order, computed by tasks of `priority` on the
/// engine's pool and by the calling thread, which takes items too while it waits for the others:
/// called from a task of the pool, it finishes even when no other thread is free. Each item is
/// computed once, by whichever thread takes it first; an item worth less than a few microseconds
/// of work is best grouped with others into one.
///
/// # Panics
///
/// If an item panics, once every item has been computed or has panicked: the panic of the first
/// item that panicked, again. If the operating system cannot start the pool's threads.
pub fn map<T, F>(priority: Priority, count: usize, item: F) -> Vec<T>
where
    T: Send + 'static,
    F: Fn(usize) -> T + Send + Sync + 'static,
{
    pool().map(priority, count, item)
}

/// How many threads the engine's pool has: the machine's logical cores but one, at least one,
/// since a core stays for the main thread (ADR-0107).
pub fn threads() -> usize {
    pool().threads
}

/// A task: work that a thread of a pool runs once.
type Task = Box<dyn FnOnce() + Send>;

/// The engine's pool, started by the first task.
fn pool() -> &'static Pool {
    static POOL: OnceLock<Arc<Pool>> = OnceLock::new();
    POOL.get_or_init(|| {
        let cores = thread::available_parallelism().map_or(1, usize::from);
        Pool::start(cores.saturating_sub(1).max(1))
    })
}

/// Threads that run tasks, and the tasks waiting for them.
struct Pool {
    queues: Mutex<Queues>,
    /// Told when a task is queued, or when a background task ends.
    ready: Condvar,
    threads: usize,
    /// The most threads that run background tasks at once.
    background_threads: usize,
}

/// The tasks waiting for a thread, by priority, in the order they were given.
#[derive(Default)]
struct Queues {
    frame: VecDeque<Task>,
    background: VecDeque<Task>,
    /// How many background tasks are running.
    running_background: usize,
}

impl Pool {
    /// Starts a pool of `threads` threads, at least one.
    fn start(threads: usize) -> Arc<Self> {
        let threads = threads.max(1);
        let pool = Arc::new(Self {
            queues: Mutex::default(),
            ready: Condvar::new(),
            threads,
            // One thread stays for frame tasks when there are two or more.
            background_threads: if threads > 1 { threads - 1 } else { 1 },
        });
        for index in 0..threads {
            let pool = Arc::clone(&pool);
            thread::Builder::new()
                .name(format!("slime-tasks {index}"))
                .spawn(move || pool.work())
                .expect("the operating system starts the pool's threads");
        }
        pool
    }

    fn lock(&self) -> MutexGuard<'_, Queues> {
        // Tasks run outside the lock, and what runs inside it does not panic: a lock poisoned is as
        // good as whole.
        self.queues.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn run(&self, priority: Priority, task: Task) {
        let mut queues = self.lock();
        match priority {
            Priority::Frame => queues.frame.push_back(task),
            Priority::Background => queues.background.push_back(task),
        }
        drop(queues);
        self.ready.notify_one();
    }

    /// Runs the tasks queued, a frame task first, for as long as the process lives.
    fn work(&self) {
        let mut queues = self.lock();
        loop {
            let (task, background) = if let Some(task) = queues.frame.pop_front() {
                (task, false)
            } else if queues.running_background < self.background_threads
                && let Some(task) = queues.background.pop_front()
            {
                queues.running_background += 1;
                (task, true)
            } else {
                queues = self
                    .ready
                    .wait(queues)
                    .unwrap_or_else(PoisonError::into_inner);
                continue;
            };
            drop(queues);
            // A task that panics ends there, not its thread; the panic hook has reported it.
            let _ = panic::catch_unwind(AssertUnwindSafe(task));
            queues = self.lock();
            if background {
                queues.running_background -= 1;
                // A thread that waited while background tasks held their share takes the next.
                if !queues.background.is_empty() {
                    self.ready.notify_one();
                }
            }
        }
    }

    fn map<T, F>(&self, priority: Priority, count: usize, item: F) -> Vec<T>
    where
        T: Send + 'static,
        F: Fn(usize) -> T + Send + Sync + 'static,
    {
        let job = Arc::new(Job {
            item,
            count,
            next: AtomicUsize::new(0),
            results: Mutex::new((0..count).map(|_| None).collect()),
            left: Mutex::new(count),
            done: Condvar::new(),
        });
        // The calling thread takes items too: one task fewer.
        for _ in 0..count.saturating_sub(1).min(self.threads) {
            let job = Arc::clone(&job);
            self.run(priority, Box::new(move || job.work()));
        }
        job.work();
        let mut left = job.left.lock().unwrap_or_else(PoisonError::into_inner);
        while *left > 0 {
            left = job.done.wait(left).unwrap_or_else(PoisonError::into_inner);
        }
        drop(left);
        let results =
            std::mem::take(&mut *job.results.lock().unwrap_or_else(PoisonError::into_inner));
        results
            .into_iter()
            .map(|result| match result.expect("every item is computed") {
                Ok(value) => value,
                Err(panic) => panic::resume_unwind(panic),
            })
            .collect()
    }
}

/// The items of one [`map`], which the calling thread and the pool's tasks take one at a time.
struct Job<T, F> {
    item: F,
    count: usize,
    /// The next item to take.
    next: AtomicUsize,
    /// Each item's value, or its panic, once computed.
    results: Mutex<Vec<Option<thread::Result<T>>>>,
    /// How many items are not computed yet, and the condition that tells when none are.
    left: Mutex<usize>,
    done: Condvar,
}

impl<T, F: Fn(usize) -> T> Job<T, F> {
    /// Computes items until none is left to take.
    fn work(&self) {
        loop {
            let index = self.next.fetch_add(1, Ordering::Relaxed);
            if index >= self.count {
                return;
            }
            let result = panic::catch_unwind(AssertUnwindSafe(|| (self.item)(index)));
            self.results.lock().unwrap_or_else(PoisonError::into_inner)[index] = Some(result);
            let mut left = self.left.lock().unwrap_or_else(PoisonError::into_inner);
            *left -= 1;
            if *left == 0 {
                self.done.notify_all();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    /// How long a test waits for what should come at once, before it says it did not.
    const PATIENCE: Duration = Duration::from_secs(10);

    #[test]
    fn a_frame_task_finds_a_thread_while_background_tasks_fill_the_pool() {
        let pool = Pool::start(3);
        let release = Arc::new(Barrier::new(pool.background_threads + 1));
        // More background tasks than the pool has threads, each waiting for the test: twice what
        // background tasks may hold at once.
        for _ in 0..2 * pool.background_threads {
            let release = Arc::clone(&release);
            pool.run(
                Priority::Background,
                Box::new(move || {
                    release.wait();
                }),
            );
        }
        let (ran, frame_ran) = mpsc::channel();
        pool.run(
            Priority::Frame,
            Box::new(move || ran.send(()).expect("the test waits")),
        );
        assert!(
            frame_ran.recv_timeout(PATIENCE).is_ok(),
            "the frame task ran while background tasks waited"
        );
        // The background tasks that held their share end; the others then run, and end too.
        release.wait();
        release.wait();
    }

    #[test]
    fn a_free_thread_takes_a_frame_task_before_every_background_task() {
        let pool = Pool::start(2);
        // Both threads held by frame tasks while tasks of both priorities are queued, then one
        // thread let go: it takes the tasks one after another, the frame task first.
        let holds = [Arc::new(Barrier::new(2)), Arc::new(Barrier::new(2))];
        let (held, holding) = mpsc::channel();
        for hold in &holds {
            let (hold, held) = (Arc::clone(hold), held.clone());
            pool.run(
                Priority::Frame,
                Box::new(move || {
                    held.send(()).expect("the test waits");
                    hold.wait();
                }),
            );
        }
        for _ in &holds {
            holding
                .recv_timeout(PATIENCE)
                .expect("both threads are held");
        }
        let (order, ran) = mpsc::channel();
        for (priority, name) in [
            (Priority::Background, "background 1"),
            (Priority::Background, "background 2"),
            (Priority::Frame, "frame"),
        ] {
            let order = order.clone();
            pool.run(
                priority,
                Box::new(move || order.send(name).expect("the test waits")),
            );
        }
        holds[0].wait();
        let names: Vec<&str> = (0..3)
            .map(|_| ran.recv_timeout(PATIENCE).expect("a task ran"))
            .collect();
        assert_eq!(names, ["frame", "background 1", "background 2"]);
        holds[1].wait();
    }

    #[test]
    fn a_task_that_panics_leaves_its_thread_to_the_next() {
        let pool = Pool::start(1);
        pool.run(Priority::Background, Box::new(|| panic!("a task's panic")));
        let (ran, after) = mpsc::channel();
        pool.run(
            Priority::Background,
            Box::new(move || ran.send(()).expect("the test waits")),
        );
        assert!(after.recv_timeout(PATIENCE).is_ok());
    }

    #[test]
    fn map_gives_each_item_in_order_and_finishes_inside_a_task_of_a_busy_pool() {
        let pool = Pool::start(2);
        let squares = pool.map(Priority::Frame, 100, |index| index * index);
        assert_eq!(
            squares,
            (0..100).map(|index| index * index).collect::<Vec<_>>()
        );
        assert!(pool.map(Priority::Frame, 0, |index| index).is_empty());
        // Inside a task, with the pool's other thread held: the task takes every item itself.
        let hold = Arc::new(Barrier::new(2));
        {
            let hold = Arc::clone(&hold);
            pool.run(
                Priority::Frame,
                Box::new(move || {
                    hold.wait();
                }),
            );
        }
        let (sums, summed) = mpsc::channel();
        let inner = Arc::clone(&pool);
        pool.run(
            Priority::Frame,
            Box::new(move || {
                let values = inner.map(Priority::Background, 10, |index| index + 1);
                sums.send(values.iter().sum::<usize>())
                    .expect("the test waits");
            }),
        );
        assert_eq!(summed.recv_timeout(PATIENCE), Ok(55));
        hold.wait();
    }

    #[test]
    fn map_panics_again_with_the_panic_of_an_item_once_every_item_is_done() {
        let pool = Pool::start(2);
        let done = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&done);
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            pool.map(Priority::Frame, 8, move |index| {
                assert!(index != 3, "item 3 panics");
                counted.fetch_add(1, Ordering::Relaxed);
            })
        }));
        let panic = result.expect_err("the panic of item 3 comes back");
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"item 3 panics"));
        assert_eq!(done.load(Ordering::Relaxed), 7, "the others were computed");
    }

    #[test]
    fn the_engine_s_pool_keeps_a_core_for_the_main_thread() {
        let cores = thread::available_parallelism().map_or(1, usize::from);
        assert_eq!(threads(), cores.saturating_sub(1).max(1));
        let values = map(Priority::Frame, 4, |index| index * 2);
        assert_eq!(values, [0, 2, 4, 6]);
        let (ran, after) = mpsc::channel();
        run(Priority::Background, move || {
            ran.send(()).expect("the test waits")
        });
        assert!(after.recv_timeout(PATIENCE).is_ok());
    }
}
