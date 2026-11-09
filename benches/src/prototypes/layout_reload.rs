//! A layout's change seen while the application runs (M6d.1d2a). Slatty's library written as
//! layouts, on signals (`signals/laid.rs`), is played to a game's page. Then, at each run, the
//! bench saves the layouts' file with a subtitle added before the grid, or taken away again, as an
//! author's editor writes a new file and renames it over the old one. A thread that watches the
//! file, which stands for the development tool's watcher, reads its text once it changed and hands
//! it to the host, which it wakes. The host gives it to the application (`Signals::relayout`),
//! which reads it into layouts and sets their signal, then runs the frame that shows it: the
//! library's top layout made again with its bindings, the covers of the cells that show and the
//! game's page too. A run fails if the frame does not show the subtitle added or taken away, if
//! the state was not kept, the scroll, the selection, the filter, the field's text and the page
//! open, or if the application noted a binding whose name the file lost.
//!
//! Timed on the system's monotonic clock, from the save to the end of the frame that shows the
//! change, and its parts: the change noticed and the file read, on the watcher's thread; the text
//! read into layouts and their signal set (`Signals::relayout`); the frame, which makes the
//! layouts' elements again, mounts them over the tree, styles, lays out and draws. The units of the
//! application's code that the frame ran are counted too (`Frame::runs`), as a reload's are: the
//! top layout's view run again and the cells' views made again. The bindings made anew and the
//! page's view, run as they are made, are not counted.
//!
//! The save and the watcher are `style_reload.rs`'s, in `watched.rs`.
//!
//! ```sh
//! nix develop
//! cargo run --release -p slime-benches -- --prototypes --layout --runs 21
//! cargo run -p slime-benches -- --prototypes --layout --runs 21    # as an author runs it
//! ```

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use uwui::tree::Kind;

use super::host::{Find, Host, found};
use super::reload::{kept, played, settled};
use super::signals::layout::Layouts;
use super::signals::{LAID_LIBRARY, LIBRARY_LAYOUTS, RwSignal, Signals};
use super::watched::{LIMIT, Read, save, watch};
use super::{SETTLE, median};

/// The laid library's program, its handle the layouts' signal.
type Laid = Signals<RwSignal<Layouts>>;

/// The grid's line in the layouts' file, before which the subtitle goes.
const GRID: &str = "            grid covers class=covers";
/// The subtitle's text.
const SUBTITLE: &str = "Your games";

/// The layouts' file with a subtitle, a label added before the grid, which shifts the grid's
/// place among its siblings. The frozen tests (`signals/layout/tests.rs`) keep a copy of their
/// own, `with_subtitle`, so that the contract stays as it was frozen.
pub(crate) fn layouts_with_subtitle() -> String {
    LIBRARY_LAYOUTS.replace(
        GRID,
        &format!("            label subtitle text=\"{SUBTITLE}\" class=subtitle\n{GRID}"),
    )
}

/// The layouts' file, with its subtitle where `subtitle`.
fn layouts(subtitle: bool) -> String {
    if subtitle {
        layouts_with_subtitle()
    } else {
        LIBRARY_LAYOUTS.to_owned()
    }
}

/// A run: a change saved, to the frame that shows it.
struct Run {
    /// From the save to the watcher's sight of it.
    noticed: Duration,
    /// From the save to the file read.
    read: Duration,
    /// The text read into layouts and their signal set.
    relaid: Duration,
    /// The frame that shows it.
    frame: Duration,
    /// From the save to the end of that frame.
    whole: Duration,
    /// The units of the application's code that the frame ran.
    runs: usize,
}

/// Whether the frame shows the subtitle, its text the file's.
fn subtitled(host: &Host<Laid>) -> Result<bool, String> {
    let Some(subtitle) = found(&host.ui, host.root, Find::Class("subtitle")) else {
        return Ok(false);
    };
    match &host.ui.element(subtitle).map(|element| &element.kind) {
        Some(Kind::Label(text)) if text == SUBTITLE => Ok(true),
        kind => Err(format!("the subtitle shows {kind:?}")),
    }
}

/// Saves the layouts' file with its subtitle where `subtitle`, waits for the watcher's text, gives
/// it to the application and runs the frame that shows it; an error if the frame does not show the
/// change, if the state was not kept, or if the application noted a binding left unused.
fn run(
    host: &mut Host<Laid>,
    path: &Path,
    changes: &Receiver<Read>,
    subtitle: bool,
) -> Result<Run, String> {
    let before = kept(host);
    if !before.page || before.field.as_deref() != Some("star") {
        return Err(format!("the library not played to its page: {before:?}"));
    }
    let wake = host.program.host_wake();
    let saved = save(path, &layouts(subtitle))?;
    let change = loop {
        match changes.try_recv() {
            Ok(change) => break change,
            Err(TryRecvError::Disconnected) => return Err("the watcher ended".to_owned()),
            Err(TryRecvError::Empty) => {
                if saved.elapsed() > LIMIT {
                    return Err(format!("no change within {LIMIT:?}"));
                }
                // As a window's host waits for its proxy, which the watcher wakes.
                wake.wait_at_most(LIMIT);
            }
        }
    };
    let relaying = Instant::now();
    host.program.relayout(&change.text)?;
    let relaid = relaying.elapsed();
    let framing = Instant::now();
    let frame = host.frame(&[]);
    let shown = Instant::now();
    if subtitled(host)? != subtitle {
        let shows = if subtitle {
            "not added"
        } else {
            "not taken away"
        };
        return Err(format!("the subtitle {shows} by the frame"));
    }
    host.settle(SETTLE);
    let after = kept(host);
    if after != before {
        return Err(format!("the state not kept: {before:?}, then {after:?}"));
    }
    let notes = host.program.notes();
    if !notes.is_empty() {
        return Err(format!("the application noted: {notes:?}"));
    }
    Ok(Run {
        noticed: change.noticed.saturating_duration_since(saved),
        read: change.read.saturating_duration_since(saved),
        relaid,
        frame: shown - framing,
        whole: shown - saved,
        runs: frame.runs,
    })
}

/// The units of code that `runs` counted: one figure, or their range.
fn counted(runs: &[Run]) -> String {
    let least = runs.iter().map(|run| run.runs).min().unwrap_or(0);
    let most = runs.iter().map(|run| run.runs).max().unwrap_or(0);
    if least == most {
        least.to_string()
    } else {
        format!("{least}–{most}")
    }
}

/// Plays the laid library, then changes its layouts' file `runs` times, a subtitle added then
/// taken away in turn, and prints the medians as Markdown, the first change apart.
pub(crate) fn print_times(runs: usize) -> Result<(), String> {
    let mut host = played(&LAID_LIBRARY);
    settled(&mut host);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/layout-reload/library.kdl");
    let folder = path.parent().ok_or("the file has a folder")?;
    std::fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    save(&path, &layouts(false))?;
    let stop = Arc::new(AtomicBool::new(false));
    let changes = watch(path.clone(), host.program.host_wake(), Arc::clone(&stop));
    // The watcher's first look before the first save, and times of change apart by more than the
    // file system's grain.
    std::thread::sleep(Duration::from_millis(50));
    let mut all = Vec::with_capacity(runs + 1);
    let mut failed = None;
    for count in 0..=runs {
        // Each run changes the file from the one that the last showed.
        let subtitle = count % 2 == 0;
        match run(&mut host, &path, &changes, subtitle) {
            Ok(run) => {
                eprintln!(
                    "run {count}: {:.1} ms from the save to its frame, {} units of code run",
                    run.whole.as_secs_f64() * 1e3,
                    run.runs
                );
                all.push(run);
            }
            Err(error) => {
                failed = Some(error);
                break;
            }
        }
        // The machine back at rest before the next save.
        std::thread::sleep(Duration::from_millis(200));
    }
    stop.store(true, Ordering::Relaxed);
    if let Some(error) = failed {
        return Err(error);
    }
    let first = all.remove(0);
    println!("### signals: a layout's change seen\n");
    println!(
        "| Change | Noticed | Read | Relaid | Its frame | From the save to the frame | Code run |"
    );
    println!("|---|---|---|---|---|---|---|");
    let ms = |time: Duration| format!("{:.3}", time.as_secs_f64() * 1e3);
    println!(
        "| The session's first, a subtitle added | {} | {} | {} | {} | {} | {} |",
        ms(first.noticed),
        ms(first.read),
        ms(first.relaid),
        ms(first.frame),
        ms(first.whole),
        first.runs,
    );
    let times = |of: fn(&Run) -> Duration| median(all.iter().map(of).collect());
    println!(
        "| The {} after it, taken away and added in turn | {} | {} | {} | {} | {} | {} |",
        all.len(),
        times(|run| run.noticed),
        times(|run| run.read),
        times(|run| run.relaid),
        times(|run| run.frame),
        times(|run| run.whole),
        counted(&all),
    );
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_holds_the_grid_once() {
        assert_eq!(LIBRARY_LAYOUTS.matches(GRID).count(), 1);
        assert!(layouts_with_subtitle().contains("label subtitle"));
    }

    #[test]
    fn a_change_saved_shows_with_the_state_kept_and_back() {
        let mut host = played(&LAID_LIBRARY);
        settled(&mut host);
        let before = kept(&host);
        for subtitle in [true, false] {
            host.program
                .relayout(&layouts(subtitle))
                .expect("the file reads");
            host.frame(&[]);
            assert_eq!(subtitled(&host), Ok(subtitle));
            settled(&mut host);
            assert_eq!(kept(&host), before);
            assert!(host.program.notes().is_empty());
        }
    }
}
