//! With `--threads`, the bench times the signals' executor beside other threads, in the program,
//! without a window: a change sent from another thread to the frame that shows it, the host woken
//! from its wait as a window's loop would be; a burst and a batch, sent whole before the frame;
//! and a frame at rest while tasks wait on timers and channels. On Slatty's downloads, 10,000
//! games whose progress the core sends from its own thread.

use std::rc::Rc;
use std::time::{Duration, Instant};

use uwui::tree::Kind;

use super::host::{Cost, Frame, Host, Look, Model};
use super::screens::small;
use super::signals::Signals;
use super::signals::downloads::{Progress, downloads};
use super::signals::prelude::*;
use super::{SETTLE, kilobytes, median};

/// The games of the downloads' screen.
const GAMES: usize = 10_000;

/// A view of `timers` tasks that wait on timers, the first in an hour, and of a task waiting on
/// each of `channels`.
pub(crate) fn waiting(timers: usize, channels: &Rc<Vec<Receiver<()>>>) -> impl View + use<> {
    for timer in 0..timers {
        spawn(async move {
            sleep(Duration::from_secs(3_600 + timer as u64)).await;
        });
    }
    for at in 0..channels.len() {
        let channels = Rc::clone(channels);
        spawn(async move { while channels[at].recv().await.is_some() {} });
    }
    label("At rest")
}

/// The view that `start` makes, opened on the small cases' sheet and settled.
fn opened<V: View>(start: impl Fn() -> ((), V) + 'static) -> Host<Signals<()>> {
    let (_, (sheet, text)) = small("Slatty's downloads");
    let look = Look::new(sheet, text);
    let now = Instant::now();
    let ui = look.interface().expect("the look reads");
    let mut host = Host::open(ui, look, Signals::open(now, start), now);
    host.frame(&[]);
    host.settle(SETTLE);
    host
}

/// Whether `host` shows a label of `text`.
fn shows(host: &Host<Signals<()>>, text: &str) -> bool {
    let mut stack = vec![host.root];
    while let Some(id) = stack.pop() {
        if let Some(element) = host.ui.element(id)
            && matches!(&element.kind, Kind::Label(held) if held == text)
        {
            return true;
        }
        stack.extend(host.ui.children(id).iter().copied());
    }
    false
}

/// A change from another thread, shown.
struct Sent {
    /// From the send to the end of the frame that shows it, the host woken from its wait; none
    /// for a burst, sent whole before the frame.
    delay: Option<Duration>,
    /// The frames until the interface is still, the first of which shows it.
    frames: usize,
    /// What those frames cost, their phases together.
    cost: Cost,
    /// The units of the application's code that they ran: the task's polls and the bindings.
    runs: usize,
    /// The host's wakes.
    wakes: usize,
}

/// The frames after `host` was woken, until it is still: what they cost and ran.
fn shown_frames(host: &mut Host<Signals<()>>) -> (Vec<Frame>, Instant) {
    let mut frames = vec![host.frame(&[])];
    let shown = Instant::now();
    frames.extend(host.settle(SETTLE));
    (frames, shown)
}

fn sent(frames: &[Frame], delay: Option<Duration>, wakes: usize) -> Sent {
    Sent {
        delay,
        frames: frames.len(),
        cost: frames
            .iter()
            .flat_map(|frame| frame.costs)
            .fold(Cost::default(), Cost::plus),
        runs: frames.iter().map(|frame| frame.runs).sum(),
        wakes,
    }
}

/// One game's progress, sent while the host waits: its wake is in the delay. A run whose host
/// did not wait yet as the change was sent is run again.
fn one_change() -> Result<Sent, String> {
    for _ in 0..5 {
        if let Some(sent) = one_change_once()? {
            return Ok(sent);
        }
    }
    Err("the host never waited before the send".to_owned())
}

/// One game's progress, sent while the host waits, if it did.
fn one_change_once() -> Result<Option<Sent>, String> {
    let (sender, receiver) = channel();
    let receiver = Rc::new(receiver);
    let mut host = opened(move || ((), downloads(GAMES, Rc::clone(&receiver))));
    let wake = host.program.host_wake();
    let core = std::thread::spawn(move || {
        // Long enough for the host to wait first.
        std::thread::sleep(Duration::from_millis(2));
        let sent = Instant::now();
        sender.send(Progress::One(1, 50)).map(|()| (sent, sender))
    });
    wake.wait();
    let (frames, shown) = shown_frames(&mut host);
    let (at, _sender) = core
        .join()
        .map_err(|_| "the core's thread panicked")?
        .map_err(|_| "the channel closed")?;
    if !shows(&host, "Game 1: 50%") {
        return Err("the change not shown".to_owned());
    }
    if wake.found_waiting() != 1 {
        return Ok(None);
    }
    Ok(Some(sent(&frames, Some(shown - at), wake.wakes())))
}

/// Every game's progress, sent whole before the frame: as `GAMES` messages, or as one batch.
fn burst(batch: bool) -> Result<Sent, String> {
    let (sender, receiver) = channel();
    let receiver = Rc::new(receiver);
    let mut host = opened(move || ((), downloads(GAMES, Rc::clone(&receiver))));
    let wake = host.program.host_wake();
    let core = std::thread::spawn(move || {
        let progress = |game: usize| (game, (game % 99 + 1) as u8);
        let sending = if batch {
            sender.send(Progress::Batch((0..GAMES).map(progress).collect()))
        } else {
            (0..GAMES).try_for_each(|game| {
                let (game, percent) = progress(game);
                sender.send(Progress::One(game, percent))
            })
        };
        sending.map(|()| sender)
    });
    let _sender = core
        .join()
        .map_err(|_| "the core's thread panicked")?
        .map_err(|_| "the channel closed")?;
    wake.wait();
    let (frames, _) = shown_frames(&mut host);
    let count = if batch { 1 } else { GAMES };
    if !shows(&host, &format!("{count} updates")) || !shows(&host, "Game 2: 3%") {
        return Err("the burst not shown whole".to_owned());
    }
    Ok(sent(&frames, None, wake.wakes()))
}

/// A frame at rest, the screen still: its phases' cost, and its application's.
struct Rest {
    frame: Cost,
    application: Cost,
    runs: usize,
    wakes: usize,
    deadline: Option<Duration>,
}

/// A frame at rest while 500 tasks wait on timers and 500 on channels, or none.
fn at_rest(tasks: bool) -> Result<Rest, String> {
    let half = if tasks { 500 } else { 0 };
    let (senders, receivers): (Vec<_>, Vec<_>) = (0..half).map(|_| channel::<()>()).unzip();
    let receivers = Rc::new(receivers);
    let mut host = opened(move || ((), waiting(half, &receivers)));
    if !host.settle(SETTLE).is_empty() {
        return Err("a frame asked at rest".to_owned());
    }
    let now = host.now();
    let frame = host.frame(&[]);
    let rest = Rest {
        frame: frame.costs.into_iter().fold(Cost::default(), Cost::plus),
        application: frame.costs[1],
        runs: frame.runs,
        wakes: host.program.host_wake().wakes(),
        deadline: host.program.deadline().map(|at| at - now),
    };
    // Dropped once counted: the last sender of a channel wakes its task, which then ends.
    drop(senders);
    Ok(rest)
}

/// Times each case `runs` times, and prints the medians as Markdown.
pub(crate) fn print_times(runs: usize) -> Result<(), String> {
    println!("### signals: other threads\n");
    println!(
        "| Change | Frames until still | From the send to the frame: ms | Their phases: ms, \
         allocations, kB | Code run | Host's wakes |"
    );
    println!("|---|---|---|---|---|---|");
    type Case = (&'static str, fn() -> Result<Sent, String>);
    let cases: [Case; 3] = [
        ("One game's progress, the host waiting", one_change),
        ("10,000 games', as many messages", || burst(false)),
        ("10,000 games', as one batch", || burst(true)),
    ];
    for (name, case) in cases {
        let all = (0..runs).map(|_| case()).collect::<Result<Vec<_>, _>>()?;
        let first = &all[0];
        let exact = all.iter().all(|run| {
            run.cost.allocations.calls == first.cost.allocations.calls
                && run.frames == first.frames
                && run.runs == first.runs
                && run.wakes == first.wakes
        });
        if !exact {
            return Err(format!("{name}: the runs counted differently"));
        }
        println!(
            "| {name} | {} | {} | {}, {}, {} | {} | {} |",
            first.frames,
            all.iter()
                .map(|run| run.delay)
                .collect::<Option<Vec<_>>>()
                .map_or("—".to_owned(), median),
            median(all.iter().map(|run| run.cost.time).collect()),
            first.cost.allocations.calls,
            kilobytes(first.cost.allocations.taken),
            first.runs,
            first.wakes,
        );
    }
    println!();
    println!(
        "| Tasks waiting | The frame: ms, allocations, kB | The application's part: ms, \
         allocations | Code run | Host's wakes | Next deadline |"
    );
    println!("|---|---|---|---|---|---|");
    for (name, tasks) in [("None", false), ("500 on timers, 500 on channels", true)] {
        let all = (0..runs)
            .map(|_| at_rest(tasks))
            .collect::<Result<Vec<_>, _>>()?;
        let first = &all[0];
        let exact = all.iter().all(|run| {
            run.application.allocations == first.application.allocations
                && run.frame.allocations.calls == first.frame.allocations.calls
                && run.runs == first.runs
                && run.wakes == first.wakes
        });
        if !exact {
            return Err(format!("{name}: the runs counted differently"));
        }
        println!(
            "| {name} | {}, {}, {} | {}, {} | {} | {} | {} |",
            median(all.iter().map(|run| run.frame.time).collect()),
            first.frame.allocations.calls,
            kilobytes(first.frame.allocations.taken),
            median(all.iter().map(|run| run.application.time).collect()),
            first.application.allocations.calls,
            first.runs,
            first.wakes,
            first
                .deadline
                .map_or("none".to_owned(), |at| format!("{} s", at.as_secs())),
        );
    }
    println!();
    Ok(())
}
