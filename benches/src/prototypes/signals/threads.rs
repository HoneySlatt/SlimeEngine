//! The interface's executor and other threads' changes, on the bench's host: a task's timer on
//! the frame's clock, which the host waits for at rest; a frame at rest while tasks wait, which
//! runs and allocates nothing; a burst from another thread, shown whole in the next frame; a
//! sender kept valid across a reload, the receiver being the application's; a channel made within
//! a view, closed with it; senders on several threads while frames run; and the executor's own
//! rules, a budget of values a poll, a task polled once a frame, a task cancelled in its own poll,
//! and a waker of a task gone, which wakes no other.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use super::Signals;
use super::channel::Closed;
use super::downloads::{Progress, downloads};
use super::executor::{Executor, HostWake, MESSAGES_PER_POLL, TaskId};
use super::kept::{labels, open_view, opened, settled};
use super::prelude::*;
use crate::prototypes::host::{Host, Model};
use crate::prototypes::threads::waiting;

/// Slatty's downloads of `games` games, their receiver the application's, made before the view.
fn downloads_opened(games: usize) -> (Host<Signals<()>>, Sender<Progress>, Rc<Receiver<Progress>>) {
    let (sender, receiver) = channel();
    let receiver = Rc::new(receiver);
    let held = Rc::clone(&receiver);
    let host = opened(move || ((), downloads(games, Rc::clone(&held))));
    (host, sender, receiver)
}

/// Sends `updates` from another thread, which ends before this returns.
fn sent_from_another_thread(sender: &Sender<Progress>, updates: Vec<Progress>) {
    let sender = sender.clone();
    std::thread::spawn(move || {
        for update in updates {
            sender
                .send(update)
                .expect("the receiver is the application's");
        }
    })
    .join()
    .expect("the core's thread sent");
}

#[test]
fn a_task_sleeps_on_the_frames_clock_which_the_host_waits_for() {
    let mut host = opened(|| {
        let text = RwSignal::new("waiting");
        spawn(async move {
            sleep(Duration::from_secs(5)).await;
            text.set("woken");
        });
        ((), label(move || text.get().to_owned()))
    });
    let started = host.now();
    assert!(host.settle(600).is_empty(), "no frame while the task waits");
    assert_eq!(labels(&host), ["waiting"]);
    let deadline = host.program.deadline().expect("the timer's deadline");
    assert!(deadline > started + Duration::from_millis(4_990));
    let frame = host.wait_for_deadline().expect("a deadline waits");
    assert_eq!(labels(&host), ["woken"], "shown in the deadline's frame");
    assert_eq!(frame.runs, 2, "the task's poll, and its label's binding");
    assert_eq!(host.program.deadline(), None);
    assert_eq!(host.program.tasks(), 0, "the task ended");
    assert_eq!(host.program.host_wake().wakes(), 0, "a timer wakes no host");
}

#[test]
fn a_frame_at_rest_while_tasks_wait_runs_and_allocates_nothing() {
    let (senders, receivers): (Vec<_>, Vec<_>) = (0..500).map(|_| channel::<()>()).unzip();
    let receivers = Rc::new(receivers);
    let mut host = opened(move || ((), waiting(500, &receivers)));
    assert_eq!(host.program.tasks(), 1_000);
    assert!(host.settle(600).is_empty(), "nothing asks a frame");
    let frame = host.frame(&[]);
    assert_eq!(frame.runs, 0, "no task polled, no binding run");
    assert_eq!(
        frame.costs[1].allocations.calls, 0,
        "the application allocates nothing"
    );
    assert!(!frame.builds_asked);
    assert_eq!(host.program.host_wake().wakes(), 0);
    let first = host.program.deadline().expect("the first timer");
    assert!(first >= host.now() + Duration::from_secs(3_599));
    drop(senders);
}

#[test]
fn a_burst_from_another_thread_shows_whole_in_the_next_frame() {
    let games = 10_000;
    let (mut host, sender, _receiver) = downloads_opened(games);
    let wake = host.program.host_wake();
    let burst = (0..games)
        .map(|game| Progress::One(game, (game % 100) as u8))
        .collect();
    sent_from_another_thread(&sender, burst);
    assert_eq!(wake.wakes(), 1, "a burst wakes the host once");
    wake.wait();
    let frame = host.frame(&[]);
    let shown = labels(&host);
    assert_eq!(shown[0], "10000 updates");
    assert_eq!(shown[1..4], ["Game 0: 0%", "Game 1: 1%", "Game 2: 2%"]);
    assert!(!frame.builds_asked, "nothing left for another frame");
    // The task's one poll, the count's label, and each row that shows but the first, whose
    // progress stayed 0.
    assert_eq!(frame.runs, 1 + 1 + (shown.len() - 2));
}

#[test]
fn a_batch_by_key_runs_the_rows_that_it_changes_and_show() {
    let (mut host, sender, _receiver) = downloads_opened(10_000);
    // Two rows that show, and one far below that does not.
    let batch = vec![(1, 40), (3, 60), (9_000, 80)];
    sent_from_another_thread(&sender, vec![Progress::Batch(batch)]);
    host.program.host_wake().wait();
    let frame = host.frame(&[]);
    let shown = labels(&host);
    assert_eq!(shown[0], "1 updates");
    assert_eq!((&*shown[2], &*shown[4]), ("Game 1: 40%", "Game 3: 60%"));
    assert_eq!(frame.runs, 1 + 1 + 2, "the poll, the count, two rows");
}

#[test]
fn another_threads_sender_stays_valid_across_a_reload() {
    let (mut host, sender, receiver) = downloads_opened(100);
    sent_from_another_thread(
        &sender,
        (0..3).map(|game| Progress::One(game, 10)).collect(),
    );
    host.program.host_wake().wait();
    settled(&mut host);
    assert_eq!(labels(&host)[0], "3 updates");
    // One sent before the reload, which the view before never received.
    sent_from_another_thread(&sender, vec![Progress::One(3, 10)]);
    host.program
        .reload(move || ((), downloads(100, Rc::clone(&receiver))));
    // One sent while the view is reloaded, before its new task first runs.
    sent_from_another_thread(&sender, vec![Progress::One(4, 10)]);
    settled(&mut host);
    assert_eq!(
        labels(&host)[0],
        "5 updates",
        "nothing lost, the count kept"
    );
    assert_eq!(host.program.notes(), Vec::<String>::new());
    assert_eq!(host.program.tasks(), 1, "the view before's task cancelled");
    assert!(sender.send(Progress::One(5, 10)).is_ok());
}

#[test]
fn a_channel_made_within_a_view_closes_with_it() {
    let handed: Rc<RefCell<Option<Sender<u8>>>> = Rc::default();
    let into = Rc::clone(&handed);
    let view = move || {
        let (sender, receiver) = channel();
        *into.borrow_mut() = Some(sender);
        let shown = RwSignal::new(0);
        spawn(async move {
            while let Some(value) = receiver.recv().await {
                shown.set(value);
            }
        });
        ((), label(move || shown.get().to_string()))
    };
    let mut host = opened(view.clone());
    let before = handed
        .borrow_mut()
        .take()
        .expect("the view handed its sender");
    host.program.reload(view);
    settled(&mut host);
    assert_eq!(
        before.send(7),
        Err(Closed(7)),
        "the send fails, not silently"
    );
}

#[test]
fn the_last_sender_dropped_ends_its_receivers_task() {
    let (mut host, sender, _receiver) = downloads_opened(10);
    assert_eq!(host.program.tasks(), 1);
    std::thread::spawn(move || drop(sender))
        .join()
        .expect("the core's thread ended");
    host.program.host_wake().wait();
    settled(&mut host);
    assert_eq!(host.program.tasks(), 0);
}

/// A task that wakes itself at each poll, which it counts: work that runs at every frame.
struct Busy(Rc<Cell<usize>>);

impl Future for Busy {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        self.0.set(self.0.get() + 1);
        context.waker().wake_by_ref();
        Poll::Pending
    }
}

#[test]
fn a_task_woken_again_in_its_frame_waits_for_the_next() {
    let polls = Rc::new(Cell::new(0));
    let counted = Rc::clone(&polls);
    let mut host = opened(move || {
        spawn(Busy(Rc::clone(&counted)));
        ((), label("Busy"))
    });
    let before = polls.get();
    for _ in 0..3 {
        let frame = host.frame(&[]);
        assert!(frame.builds_asked, "it asks for the next frame");
    }
    assert_eq!(polls.get() - before, 3, "once a frame");
}

/// A task that keeps its waker, and counts its polls.
struct Keeps(Rc<RefCell<Option<Waker>>>, Rc<Cell<usize>>);

impl Future for Keeps {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        *self.0.borrow_mut() = Some(context.waker().clone());
        self.1.set(self.1.get() + 1);
        Poll::Pending
    }
}

#[test]
fn a_waker_of_a_task_cancelled_wakes_no_other() {
    let now = Instant::now();
    let executor = Executor::new(now, Arc::new(HostWake::default()));
    let (kept, first) = (Rc::default(), Rc::new(Cell::new(0)));
    let gone = executor.spawn(Box::pin(Keeps(Rc::clone(&kept), Rc::clone(&first))));
    executor.advance(now);
    assert_eq!(executor.run_ready(), 1);
    executor.cancel(gone);
    let second = Rc::new(Cell::new(0));
    executor.spawn(Box::pin(Keeps(Rc::default(), Rc::clone(&second))));
    executor.advance(now);
    assert_eq!(executor.run_ready(), 1, "the new task's first poll");
    let waker = kept.borrow_mut().take().expect("the task kept its waker");
    waker.wake();
    executor.advance(now);
    assert_eq!(executor.run_ready(), 0, "its place's new task not woken");
    assert_eq!((first.get(), second.get()), (1, 1));
}

/// A view that counts what it receives on `receiver`.
fn counter(receiver: &Rc<Receiver<u32>>) -> impl View + use<> {
    let count = RwSignal::new(0_usize);
    let receiver = Rc::clone(receiver);
    spawn(async move {
        while receiver.recv().await.is_some() {
            count.update(|count| *count += 1);
        }
    });
    label(move || count.get().to_string())
}

#[test]
fn senders_on_several_threads_while_frames_run_lose_nothing() {
    let (sender, receiver) = channel::<u32>();
    let receiver = Rc::new(receiver);
    let mut host = opened(move || ((), counter(&receiver)));
    let wake = host.program.host_wake();
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let sender = sender.clone();
            std::thread::spawn(move || {
                for value in 0..5_000 {
                    sender
                        .send(value)
                        .expect("the receiver is the application's");
                    if value % 64 == 0 {
                        std::thread::yield_now();
                    }
                }
            })
        })
        .collect();
    drop(sender);
    let all = ["20000".to_owned()];
    loop {
        let frame = host.frame(&[]);
        if labels(&host) == all {
            break;
        }
        // At rest, the host waits for a wake: one lost would leave it waiting.
        if !frame.builds_asked {
            assert!(
                wake.wait_at_most(Duration::from_secs(5)),
                "a wake lost, {:?} received",
                labels(&host)
            );
        }
    }
    for thread in threads {
        thread.join().expect("each sender ended");
    }
}

#[test]
fn a_task_takes_at_most_its_budget_of_values_a_frame() {
    let (sender, receiver) = channel::<u32>();
    let sent = 2 * MESSAGES_PER_POLL + 1_000;
    for value in 0..sent {
        sender.send(value as u32).expect("the receiver waits");
    }
    let receiver = Rc::new(receiver);
    let mut host = open_view(move || ((), counter(&receiver)));
    let mut received = Vec::new();
    loop {
        let frame = host.frame(&[]);
        received.push(labels(&host)[0].clone());
        if !frame.builds_asked {
            break;
        }
    }
    let budget = MESSAGES_PER_POLL;
    assert_eq!(
        received,
        [budget, 2 * budget, sent].map(|count| count.to_string()),
        "a frame for each budget, the rest in the next"
    );
}

/// A task that cancels itself as it is polled, holding `held`.
struct CancelsItself {
    executor: Rc<Executor>,
    id: Rc<Cell<Option<TaskId>>>,
    _held: Rc<()>,
}

impl Future for CancelsItself {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        self.executor.cancel(self.id.get().expect("spawned"));
        Poll::Pending
    }
}

#[test]
fn a_task_cancelled_in_its_own_poll_is_dropped_after_it() {
    let now = Instant::now();
    let executor = Executor::new(now, Arc::new(HostWake::default()));
    let (id, held) = (Rc::new(Cell::new(None)), Rc::new(()));
    let task = CancelsItself {
        executor: Rc::clone(&executor),
        id: Rc::clone(&id),
        _held: Rc::clone(&held),
    };
    id.set(Some(executor.spawn(Box::pin(task))));
    executor.advance(now);
    assert_eq!(executor.run_ready(), 1);
    assert_eq!(executor.tasks(), 0);
    assert_eq!(Rc::strong_count(&held), 1, "its future dropped");
}

#[test]
fn the_last_sender_dropped_during_a_reload_ends_the_new_task_after_what_was_sent() {
    let (mut host, sender, receiver) = downloads_opened(100);
    sent_from_another_thread(
        &sender,
        (0..2).map(|game| Progress::One(game, 10)).collect(),
    );
    host.program.host_wake().wait();
    settled(&mut host);
    sent_from_another_thread(&sender, vec![Progress::One(2, 10)]);
    host.program
        .reload(move || ((), downloads(100, Rc::clone(&receiver))));
    drop(sender);
    settled(&mut host);
    assert_eq!(labels(&host)[0], "3 updates");
    assert_eq!(
        host.program.tasks(),
        0,
        "the channel closed, its task ended"
    );
}
