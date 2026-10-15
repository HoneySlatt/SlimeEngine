//! The renderer's task pool: threads of its own that run the tasks of scene loads, private to the
//! crate (ADR-0072). Stage 7's job system is to replace it.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

/// A task: work that a thread of the pool runs once.
type Task = Box<dyn FnOnce() + Send>;

/// Runs `task` on a thread of the pool, as soon as one is free. The first call starts the pool's
/// threads, as many as the machine's cores but one, at least 1 and at most 4: they live as long as
/// the process, and wait for tasks while there are none. A task must not wait for another, which
/// could be queued behind it.
///
/// # Panics
///
/// If the operating system cannot start the pool's threads. A task that panics ends its thread;
/// the tasks of this crate catch their panics, to report them to whoever waits for them.
pub(crate) fn run(task: impl FnOnce() + Send + 'static) {
    static TASKS: OnceLock<Sender<Task>> = OnceLock::new();
    TASKS
        .get_or_init(start)
        .send(Box::new(task))
        .expect("the pool's threads wait for tasks as long as the process lives");
}

/// Starts the pool's threads and returns the sender of their tasks.
fn start() -> Sender<Task> {
    let (sender, receiver) = mpsc::channel::<Task>();
    let receiver = Arc::new(Mutex::new(receiver));
    let count = thread::available_parallelism()
        .map_or(1, |cores| cores.get().saturating_sub(1))
        .clamp(1, 4);
    for index in 0..count {
        let receiver = Arc::clone(&receiver);
        thread::Builder::new()
            .name(format!("slime-render loader {index}"))
            .spawn(move || work(&receiver))
            .expect("the operating system starts the pool's threads");
    }
    sender
}

/// Runs the tasks that `receiver` gives, one at a time, for as long as the process lives.
fn work(receiver: &Mutex<Receiver<Task>>) {
    loop {
        // The lock is released before the task runs, so that the other threads take the next ones.
        let task = receiver
            .lock()
            .expect("a thread of the pool never panics while it holds the lock")
            .recv();
        match task {
            Ok(task) => task(),
            // The sender lives in a static: it is never dropped.
            Err(_) => return,
        }
    }
}
