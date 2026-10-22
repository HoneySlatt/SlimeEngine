//! The lane of the loads' tasks on the engine's pool: at most [`LOADS_AT_ONCE`] run at once,
//! whatever the pool's threads, those of the loads of the lowest rank before the others.

use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, PoisonError};

use slime_gfx::UploadRank;
use slime_tasks::Priority;

/// How many tasks of the process's loads run at once. Measured on the owner's RX 6900 XT with
/// `loading.textures` and `loading.zone` (ADR-0118, `docs/measurements.md`, M6a.2): a load's tasks
/// read cooked files, create GPU objects and write staging buffers, and more of them at once
/// compete for the memory's bandwidth and for the locks of wgpu and the driver with the thread that
/// draws: with two, no frame took more than 13 ms and both loads ended soonest; with four, frames
/// took up to 68 ms, with fourteen up to 41 ms on the zone, and the loads ended later. The pool's
/// other background work, which touches neither the GPU nor much memory, keeps its threads.
pub(crate) const LOADS_AT_ONCE: usize = 2;

/// A task of a load.
type Task = Box<dyn FnOnce() + Send>;

/// The tasks waiting for the lane, each with its load's rank and the order it was given in, and
/// how many of the pool's threads run its tasks.
struct Lane {
    waiting: Vec<(UploadRank, u64, Task)>,
    given: u64,
    draining: usize,
}

static LANE: Mutex<Lane> = Mutex::new(Lane {
    waiting: Vec::new(),
    given: 0,
    draining: 0,
});

/// The rank of the completions of loads, which come before every other task.
pub(crate) static COMPLETION: LazyLock<UploadRank> = LazyLock::new(|| UploadRank::new(0));

/// A number for a load that starts, from 1: of two loads of one class, the one started first runs
/// its tasks and has its copies submitted first, so that a game that asks for its hero, then for
/// its zone, has its hero first.
pub(crate) fn number() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The rank of the load numbered `number` ([`number`]) in the class `class`, from 0, the most
/// urgent: the classes in their order, then the loads in the order they started.
pub(crate) fn rank(class: u64, number: u64) -> u64 {
    class << 56 | number
}

/// Runs `task`, of a load of rank `rank`, as a background task of the engine's pool, once fewer
/// than [`LOADS_AT_ONCE`] tasks of loads run: before the tasks of the loads of higher ranks, read
/// when a thread takes the next task, so that a load whose rank changes moves among the others, and
/// in the order they were given among the tasks of one rank. [`COMPLETION`] comes first: the
/// completion of a load, which ends it, so that a load with nothing else left does not wait for the
/// tasks of other loads.
pub(crate) fn run(rank: &UploadRank, task: impl FnOnce() + Send + 'static) {
    let mut lane = LANE.lock().unwrap_or_else(PoisonError::into_inner);
    let given = lane.given;
    lane.given += 1;
    lane.waiting.push((rank.clone(), given, Box::new(task)));
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
        let first = (0..lane.waiting.len()).min_by_key(|&index| {
            let (rank, given, _) = &lane.waiting[index];
            (rank.get(), *given)
        });
        match first {
            Some(index) => lane.waiting.swap_remove(index).2,
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
    fn two_tasks_of_loads_run_at_once_at_most_completions_first_then_by_rank() {
        let _holding = HOLDING_THE_POOL
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Two tasks that hold the lane take two of the pool's background threads, which the pool
        // gives from three threads on (`slime_tasks::threads`).
        if slime_tasks::threads() < 3 {
            return;
        }
        let holders = UploadRank::new(number());
        let (started, both_started) = mpsc::channel();
        let releases: Vec<_> = (0..LOADS_AT_ONCE)
            .map(|_| {
                let (release, released) = mpsc::channel::<()>();
                let started = started.clone();
                run(&holders, move || {
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
        // Given in this order, the later load's tasks first; the last load's rank is raised above
        // both while its task waits.
        let [earlier, later, raised] = [number(), number(), number()].map(UploadRank::new);
        let (ran, order) = mpsc::channel();
        for (load, name) in [
            (&later, "later 1"),
            (&later, "later 2"),
            (&earlier, "earlier 1"),
            (&earlier, "earlier 2"),
            (&raised, "raised"),
            (&*COMPLETION, "completion"),
        ] {
            let ran = ran.clone();
            run(load, move || ran.send(name).expect("the test waits"));
        }
        raised.set(earlier.get() - 1);
        assert!(
            order.recv_timeout(Duration::from_millis(50)).is_err(),
            "a third task of loads ran"
        );
        // One holder ends: the tasks run one at a time, through the lane left free.
        releases[0].send(()).expect("the holder waits");
        let names: Vec<_> = (0..6)
            .map(|_| order.recv_timeout(PATIENCE).expect("the tasks run"))
            .collect();
        assert_eq!(
            names,
            [
                "completion",
                "raised",
                "earlier 1",
                "earlier 2",
                "later 1",
                "later 2"
            ]
        );
        releases[1].send(()).expect("the holder waits");
    }
}
