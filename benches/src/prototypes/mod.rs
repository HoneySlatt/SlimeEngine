//! M6d.1b's bench, on the control: each screen of ADR-0202 prototyped from its ideal code and
//! played through its scripted interactions without a window, on a clock that the bench moves;
//! each phase of each frame timed and its allocations counted; the items of the display list
//! changed, out of all; the delay from an input to the display list that shows it; a frame where
//! nothing changed; the memory held while the screen is open and after it closes; and, after each
//! interaction, the display list compared with that of a new interface built from the same state
//! (`docs/adr/0202-how-m6d1-compares-ways-of-writing-an-application.md`). On a branch, thrown away
//! once M6d.1b ends.
//!
//! ```sh
//! cargo run --release -p slime-benches -- --prototypes            # every screen, 21 runs
//! cargo run --release -p slime-benches -- --prototypes library    # one screen
//! cargo run --release -p slime-benches -- --prototypes --runs 5   # fewer runs
//! ```
//!
//! It prints Markdown, which uwui's `docs/measurements.md` takes. With `PROTOTYPES_DUMP` set, each
//! comparison with a new interface prints the state it copied and the first items that differ.

mod control;
mod data;
mod host;
mod screens;

use std::time::{Duration, Instant};

use uwui::prelude::Program;

use crate::allocations::{Allocations, counted};
use host::{Cost, Frame, Host, Look, PHASES};
use screens::{DOCUMENT, HELLO, LIBRARY, MISTAKE, SONGS, Scale, Screen};

/// The runs of each screen: enough for a median that a few slow runs do not move.
const RUNS: usize = 21;

/// The frames that an interaction may take to settle, past its inputs.
const SETTLE: usize = 600;

/// Runs the bench on the screens that `arguments` name, or on all, and prints its figures.
pub fn run(arguments: &[String]) -> Result<(), String> {
    let mut runs = RUNS;
    let mut names = Vec::new();
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        if argument == "--runs" {
            runs = arguments
                .next()
                .and_then(|runs| runs.parse().ok())
                .filter(|&runs| runs >= 2)
                .ok_or("--runs takes a number of 2 or more")?;
        } else {
            names.push(argument.as_str());
        }
    }
    let chosen = |name: &str| names.is_empty() || names.contains(&name);
    println!(
        "Build: {}; {runs} runs a screen.\n",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    if chosen("hello") {
        print(&HELLO, &measure(&HELLO, Scale::Whole, runs)?);
    }
    if chosen("library") {
        print(&LIBRARY, &measure(&LIBRARY, Scale::Whole, runs)?);
    }
    if chosen("songs") {
        print(&SONGS, &measure(&SONGS, Scale::Whole, runs)?);
    }
    if chosen("document") {
        print(&DOCUMENT, &measure(&DOCUMENT, Scale::Whole, runs)?);
    }
    if chosen("mistake") {
        print(&MISTAKE, &measure(&MISTAKE, Scale::Whole, runs)?);
    }
    Ok(())
}

/// What a run of a screen's script did.
struct Played {
    interactions: Vec<Interacted>,
    /// A frame where nothing changed, once the screen opened and settled.
    still: Frame,
    /// The memory held, when it was measured.
    memory: Option<Memory>,
}

/// What an interaction did: its frames, and the items of its last display list that differ from a
/// new interface's, of all, when they were compared.
struct Interacted {
    frames: Vec<Frame>,
    wrong: Option<(usize, usize)>,
}

/// The bytes held, against what was held before the screen opened.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Memory {
    /// The screen's data alone, made.
    data: i64,
    /// Once open and still: the data, the program and the interface's elements.
    open: i64,
    /// After the script ended and the screen closed: its root removed and its program dropped.
    closed: i64,
}

/// The bytes held more at the end of a span than at its start.
fn held(allocations: Allocations) -> i64 {
    // A thread takes less than 2⁶³ bytes over its life.
    allocations.taken as i64 - allocations.freed as i64
}

/// Plays `screen`'s script once: its interactions' display lists compared with a new interface's
/// where `compare`, its items changed counted where `counting`, its memory measured where `memory`.
fn play<P: Program + Clone>(
    screen: &Screen<P>,
    scale: Scale,
    counting: bool,
    compare: bool,
    memory: bool,
) -> Result<Played, String> {
    let look = Look::new(screen.sheet.0, screen.sheet.1);
    let ui = look.interface()?;
    let script = (screen.script)(scale);
    let start = Instant::now();
    let data = if memory {
        held(counted(|| (screen.make)(start)).1)
    } else {
        0
    };
    // Made before the spans measured, as the frames recorded are taken off what they hold.
    let mut interactions = Vec::with_capacity(script.len());
    let mut opened = 0;
    let mut still = None;
    let (played, life) = counted(|| -> Result<_, String> {
        let (mut host, open) = counted(|| {
            let mut host = Host::open(ui, look, (screen.make)(start), start);
            host.counting = counting;
            host
        });
        opened = held(open);
        for interaction in &script {
            let (mut frames, settled) = counted(|| {
                let mut frames: Vec<Frame> = interaction
                    .frames
                    .iter()
                    .map(|inputs| host.frame(inputs))
                    .collect();
                frames.extend(host.settle(SETTLE));
                frames
            });
            frames.shrink_to_fit();
            if interaction.name == "opened" {
                opened += held(settled) - recorded(&frames);
            }
            let wrong = compare.then(|| host.wrong_items()).transpose()?;
            interactions.push(Interacted { frames, wrong });
            if still.is_none() {
                still = Some(host.frame(&[]));
            }
        }
        Ok(host.close())
    });
    let ui = played?;
    let still = still.ok_or("a script opens its screen")?;
    drop(ui);
    let closed = held(life) - played_frames(&interactions);
    Ok(Played {
        interactions,
        still,
        memory: memory.then_some(Memory {
            data,
            open: opened,
            closed,
        }),
    })
}

/// The bytes that the record of `frames` holds, which the bench keeps, not the screen.
fn recorded(frames: &Vec<Frame>) -> i64 {
    (frames.capacity() * size_of::<Frame>()) as i64
}

/// The bytes that the records of every interaction's frames hold.
fn played_frames(interactions: &[Interacted]) -> i64 {
    interactions
        .iter()
        .map(|interaction| recorded(&interaction.frames))
        .sum()
}

/// What every run of a screen gave: the counts of the first, and each run's times.
struct Measured {
    first: Played,
    /// Whether a second run counted the same.
    exact: bool,
    times: Vec<Times>,
}

/// A run's times: each interaction's by phase, and its delay's processor time; the still frame's
/// by phase.
struct Times {
    interactions: Vec<([Duration; 5], Duration)>,
    still: [Duration; 5],
}

/// Plays `screen` `runs` times: the first compares and counts, the second counts and measures the
/// memory, every run is timed.
fn measure<P: Program + Clone>(
    screen: &Screen<P>,
    scale: Scale,
    runs: usize,
) -> Result<Measured, String> {
    let first = play(screen, scale, true, true, false)?;
    let mut second = play(screen, scale, true, false, true)?;
    let exact = counts(&first) == counts(&second);
    let mut times = vec![times_of(&first), times_of(&second)];
    for _ in 2..runs {
        times.push(times_of(&play(screen, scale, false, false, false)?));
    }
    let mut first = first;
    first.memory = second.memory.take();
    Ok(Measured {
        first,
        exact,
        times,
    })
}

/// Everything a run counted, its times apart.
fn counts(played: &Played) -> Vec<(Vec<Allocations>, FrameCounts)> {
    played
        .interactions
        .iter()
        .flat_map(|interaction| interaction.frames.iter())
        .chain([&played.still])
        .map(|frame| {
            (
                frame.costs.iter().map(|cost| cost.allocations).collect(),
                FrameCounts::of(frame),
            )
        })
        .collect()
}

/// What a frame counted.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FrameCounts {
    built: usize,
    styled: usize,
    laid_out: usize,
    painted: usize,
    visited: usize,
    items: usize,
    changed: usize,
    delay: bool,
}

impl FrameCounts {
    fn of(frame: &Frame) -> Self {
        Self {
            built: frame.stats.built,
            styled: frame.stats.styled,
            laid_out: frame.stats.layouts + frame.stats.lists,
            painted: frame.stats.painted,
            visited: frame.stats.visited,
            items: frame.items,
            changed: frame.changed,
            delay: frame.builds_asked,
        }
    }
}

/// The frames of an interaction's delay: from its first, which takes its first input, to the first
/// after which no builder must run again.
fn delay(frames: &[Frame]) -> &[Frame] {
    let end = frames
        .iter()
        .position(|frame| !frame.builds_asked)
        .map_or(frames.len(), |at| at + 1);
    &frames[..end]
}

/// The cost of `frames`, phase by phase.
fn cost(frames: &[Frame]) -> [Cost; 5] {
    let mut costs = [Cost::default(); 5];
    for frame in frames {
        for (sum, cost) in costs.iter_mut().zip(frame.costs) {
            *sum = sum.plus(cost);
        }
    }
    costs
}

/// A run's times: each interaction's by phase and its delay's, and the still frame's by phase.
fn times_of(played: &Played) -> Times {
    let interactions = played
        .interactions
        .iter()
        .map(|Interacted { frames, .. }| {
            let delay = cost(delay(frames)).iter().map(|cost| cost.time).sum();
            (cost(frames).map(|cost| cost.time), delay)
        })
        .collect();
    Times {
        interactions,
        still: played.still.costs.map(|cost| cost.time),
    }
}

/// The median of `times`, and their range, in milliseconds.
fn median(mut times: Vec<Duration>) -> String {
    times.sort();
    let ms = |time: Duration| time.as_secs_f64() * 1e3;
    format!(
        "{:.3} ({:.3}–{:.3})",
        ms(times[times.len() / 2]),
        ms(times[0]),
        ms(times[times.len() - 1])
    )
}

/// `bytes` in kB of 10³ bytes, as uwui's budgets count.
fn kilobytes(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1e3)
}

/// Prints what `measured` found of `screen`, as Markdown.
fn print<P>(screen: &Screen<P>, measured: &Measured) {
    let first = &measured.first;
    let names: Vec<&str> = (screen.script)(Scale::Short)
        .iter()
        .map(|interaction| interaction.name)
        .collect();
    println!("### {}\n", screen.name);
    if !measured.exact {
        println!("**A second run counted otherwise: the counts below are not exact.**\n");
    }
    println!(
        "| Interaction | Frames | Delay: frames, CPU ms | Builders run | Elements styled | Layouts | \
         Elements drawn again | Items changed / all | Items wrong |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for (at, Interacted { frames, wrong }) in first.interactions.iter().enumerate() {
        let sum = |count: fn(&Frame) -> usize| frames.iter().map(count).sum::<usize>();
        let delays: Vec<Duration> = measured
            .times
            .iter()
            .map(|run| run.interactions[at].1)
            .collect();
        let changed = sum(|frame| frame.changed);
        let items = sum(|frame| frame.items);
        let wrong = wrong.map_or("—".to_owned(), |(wrong, all)| format!("{wrong} of {all}"));
        println!(
            "| {} | {} | {}, {} | {} | {} | {} | {} | {changed} / {items} ({:.1} %) | {wrong} |",
            names[at],
            frames.len(),
            delay(frames).len(),
            median(delays),
            sum(|frame| frame.stats.built),
            sum(|frame| frame.stats.styled),
            sum(|frame| frame.stats.layouts + frame.stats.lists),
            sum(|frame| frame.stats.painted),
            100.0 * changed as f64 / items.max(1) as f64,
        );
    }
    let still = &first.still;
    println!(
        "| A frame where nothing changed | 1 | — | {} | {} | {} | {} | {} / {} | — |\n",
        still.stats.built,
        still.stats.styled,
        still.stats.layouts + still.stats.lists,
        still.stats.painted,
        still.changed,
        still.items,
    );
    println!(
        "Processor time of each phase, in ms, the median of {} runs (range):\n",
        measured.times.len()
    );
    println!("| Interaction | {} | Whole |", PHASES.join(" | "));
    println!("|---|---|---|---|---|---|---|");
    let row = |name: &str, phases: &dyn Fn(usize) -> Vec<Duration>| {
        let cells: Vec<String> = (0..5).map(|phase| median(phases(phase))).collect();
        let whole = (0..measured.times.len())
            .map(|run| (0..5).map(|phase| phases(phase)[run]).sum())
            .collect();
        println!("| {name} | {} | {} |", cells.join(" | "), median(whole));
    };
    for (at, name) in names.iter().enumerate() {
        row(name, &|phase| {
            measured
                .times
                .iter()
                .map(|run| run.interactions[at].0[phase])
                .collect()
        });
    }
    row("A frame where nothing changed", &|phase| {
        measured.times.iter().map(|run| run.still[phase]).collect()
    });
    println!("\nAllocations of each phase, and their kB:\n");
    println!("| Interaction | {} |", PHASES.join(" | "));
    println!("|---|---|---|---|---|---|");
    let allocations = |name: &str, costs: [Cost; 5]| {
        let cells: Vec<String> = costs
            .iter()
            .map(|cost| {
                format!(
                    "{}, {}",
                    cost.allocations.calls,
                    kilobytes(cost.allocations.taken)
                )
            })
            .collect();
        println!("| {name} | {} |", cells.join(" | "));
    };
    for (at, Interacted { frames, .. }) in first.interactions.iter().enumerate() {
        allocations(names[at], cost(frames));
    }
    allocations("A frame where nothing changed", still.costs);
    if let Some(memory) = first.memory {
        println!(
            "\nMemory held, against before the screen opened: its data alone, {:.2} MB; open and \
             still, {:.2} MB; after the script and the screen closed, {:.2} MB.\n",
            memory.data as f64 / 1e6,
            memory.open as f64 / 1e6,
            memory.closed as f64 / 1e6,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays `screen` twice, short, and gives the first run's items wrong after each interaction,
    /// checking that the second counted the same, but for the application's allocations where
    /// it `prints`: the test captures what a program prints in a buffer that grows.
    fn wrong<P: Program + Clone>(screen: &Screen<P>, prints: bool) -> Vec<usize> {
        let first = play(screen, Scale::Short, true, true, false).expect("the screen plays");
        let second = play(screen, Scale::Short, true, false, true).expect("the screen plays");
        let counted = |played: &Played| {
            let mut counted = counts(played);
            if prints {
                for (allocations, _) in &mut counted {
                    allocations[1] = Allocations::default();
                }
            }
            counted
        };
        assert!(
            counted(&first) == counted(&second),
            "a second run counts the same"
        );
        first
            .interactions
            .iter()
            .map(|interaction| interaction.wrong.expect("compared").0)
            .collect()
    }

    #[test]
    fn the_control_renders_each_screen_as_a_new_interface_would() {
        assert!(wrong(&HELLO, true).iter().all(|&wrong| wrong == 0));
        assert!(wrong(&LIBRARY, false).iter().all(|&wrong| wrong == 0));
        assert!(wrong(&SONGS, false).iter().all(|&wrong| wrong == 0));
        assert!(wrong(&DOCUMENT, false).iter().all(|&wrong| wrong == 0));
    }

    #[test]
    fn the_screens_do_what_their_scripts_say() {
        let played = |screen: &Screen<_>| play(screen, Scale::Short, true, false, false);
        let library = played(&LIBRARY).expect("the library plays");
        let frames = |at: usize| &library.interactions[at].frames;
        assert!(
            frames(1).iter().any(|frame| frame.stats.built > 0),
            "the grid builds the cells that come as it scrolls"
        );
        assert!(
            frames(5)[0].changed > 0,
            "the selection moves, its cell drawn otherwise"
        );
        assert!(
            frames(6).len() > 43,
            "the key art fades in over 180 ms, 43 frames at 240 Hz"
        );
        let theme = frames(8)[0].stats;
        assert_eq!(
            theme.styled, theme.elements,
            "a theme styles every element again"
        );
        let songs = play(&SONGS, Scale::Short, true, false, false).expect("the songs play");
        let wave = &songs.interactions[7].frames;
        assert!(
            wave.iter().all(|frame| frame.stats.built == 1),
            "the wave's builder runs alone at each frame while a song plays"
        );
        let document = play(&DOCUMENT, Scale::Short, true, false, false).expect("it plays");
        let shown = document.interactions[1].frames[0];
        let hidden = document.interactions[2].frames[0];
        assert_eq!(shown.changed, 1, "the line that shows changes alone");
        assert_eq!(
            (hidden.changed, hidden.stats.painted),
            (0, 0),
            "a line that does not show draws nothing"
        );
    }

    #[test]
    fn a_revision_not_moved_shows_as_a_wrong_render() {
        let wrong = wrong(&MISTAKE, false);
        assert_eq!(wrong[..2], [0, 0], "right until the selection moves");
        assert!(wrong[2] > 0, "the row selected before still shows");
    }
}
