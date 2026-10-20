//! The lane of the loads' tasks on the engine's pool: at most [`LOADS_AT_ONCE`] run at once,
//! whatever the pool's threads, those of the loads started first before the others.

use std::collections::BTreeMap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use slime_tasks::Priority;

/// How many tasks of the process's loads run at once. Measured on the owner's RX 6900 XT with
/// `loading.textures` and `loading.zone` (ADR-0118, `docs/measurements.md`, M6a.2): a load's tasks
/// read cooked files, create GPU objects and write staging buffers, and more of them at once
/// compete for the memory's bandwidth and for the locks of wgpu and the driver with the thread that
/// draws: with two, no frame took more than 13 ms and both loads ended soonest; with four, frames
/// took up to 68 ms, with fourteen up to 41 ms on the zone, and the loads ended later. The pool's
/// other background work, which touches neither the GPU nor much memory, keeps its threads.
const LOADS_AT_ONCE: usize = 2;

/// A task of a load.
type Task = Box<dyn FnOnce() + Send>;

/// The tasks waiting for the lane, by their load's number, then in the order they were given, and
/// how many of the pool's threads run its tasks.
struct Lane {
    waiting: BTreeMap<(u64, u64), Task>,
    given: u64,
    draining: usize,
}

static LANE: Mutex<Lane> = Mutex::new(Lane {
    waiting: BTreeMap::new(),
    given: 0,
    draining: 0,
});

/// A number for a load that starts: its tasks run before those of the loads that start after it,
/// so that a game that asks for its hero, then for its zone, has its hero first.
pub(crate) fn number() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Runs `task`, of the load numbered `load` ([`number`]), as a background task of the engine's
/// pool, once fewer than [`LOADS_AT_ONCE`] tasks of loads run: before the tasks of the loads
/// started after it, in the order the tasks of one load were given. Load 0 comes first: the
/// completion of a load, which ends it, so that a load with nothing else left does not wait for the
/// tasks of other loads.
pub(crate) fn run(load: u64, task: impl FnOnce() + Send + 'static) {
    let mut lane = LANE.lock().unwrap_or_else(PoisonError::into_inner);
    let given = lane.given;
    lane.given += 1;
    lane.waiting.insert((load, given), Box::new(task));
    if lane.draining < LOADS_AT_ONCE {
        lane.draining += 1;
        drop(lane);
        slime_tasks::run(Priority::Background, drain);
    }
}

/// Runs the lane's first task, then queues itself again as a background task of the pool, until
/// none waits: between two tasks of loads, the thread goes back to the pool, which gives it a frame
/// task first.
fn drain() {
    let task = {
        let mut lane = LANE.lock().unwrap_or_else(PoisonError::into_inner);
        match lane.waiting.pop_first() {
            Some((_, task)) => task,
            None => {
                lane.draining -= 1;
                return;
            }
        }
    };
    // A load's task catches its own panic, to report it to its load; this one keeps the lane going
    // whatever comes through it.
    let _ = panic::catch_unwind(AssertUnwindSafe(task));
    slime_tasks::run(Priority::Background, drain);
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    /// How long the test waits for what must happen.
    const PATIENCE: Duration = Duration::from_secs(10);

    /// Taken by the tests that hold threads of the engine's pool until something happens, which
    /// would otherwise wait for each other's threads.
    pub(crate) static HOLDING_THE_POOL: Mutex<()> = Mutex::new(());

    #[test]
    fn two_tasks_of_loads_run_at_once_at_most_completions_first_then_the_earlier_loads() {
        let _holding = HOLDING_THE_POOL
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Two tasks that hold the lane take two of the pool's background threads, which the pool
        // gives from three threads on (`slime_tasks::threads`).
        if slime_tasks::threads() < 3 {
            return;
        }
        let holders = number();
        let (started, both_started) = mpsc::channel();
        let releases: Vec<_> = (0..LOADS_AT_ONCE)
            .map(|_| {
                let (release, released) = mpsc::channel::<()>();
                let started = started.clone();
                run(holders, move || {
                    started.send(()).expect("the test waits");
                    let _ = released.recv();
                });
                release
            })
            .collect();
        for _ in 0..LOADS_AT_ONCE {
            both_started
                .recv_timeout(PATIENCE)
                .expect("both holders run");
        }
        // Given in this order, the later load's tasks first.
        let (earlier, later) = (number(), number());
        let (ran, order) = mpsc::channel();
        for (load, name) in [
            (later, "later 1"),
            (later, "later 2"),
            (earlier, "earlier 1"),
            (earlier, "earlier 2"),
            (0, "completion"),
        ] {
            let ran = ran.clone();
            run(load, move || ran.send(name).expect("the test waits"));
        }
        assert!(
            order.recv_timeout(Duration::from_millis(50)).is_err(),
            "a third task of loads ran"
        );
        // One holder ends: the tasks run one at a time, through the lane left free.
        releases[0].send(()).expect("the holder waits");
        let names: Vec<_> = (0..5)
            .map(|_| order.recv_timeout(PATIENCE).expect("the tasks run"))
            .collect();
        assert_eq!(
            names,
            ["completion", "earlier 1", "earlier 2", "later 1", "later 2"]
        );
        releases[1].send(()).expect("the holder waits");
    }
}
