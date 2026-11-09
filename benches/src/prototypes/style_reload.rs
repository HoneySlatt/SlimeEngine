//! A style sheet's change seen while the application runs (M6d.1d1). Slatty's library, on
//! signals, is played to a game's page. Then, at each run, the bench saves the screen's style
//! sheet with its title's size changed, as an author's editor writes a new file and renames it
//! over the old one. A thread that watches the file, which stands for the development tool's
//! watcher, reads its text once it changed and hands it to the host, which it wakes. The host
//! styles the interface again with it between two frames, its theme kept, and runs the frame that
//! shows it. A run fails if the frame does not show the new size with the state kept: the scroll,
//! the selection, the filter, the field's text and the page open.
//!
//! Timed on the system's monotonic clock, from the save to the end of the frame that shows the
//! change, and its parts: the change noticed and the file read, on the watcher's thread; the
//! sheets and themes read and the interface's rules made again (`Ui::set_themes`); the frame,
//! which styles every element again, lays out and draws.
//!
//! The watcher looks at the file's time of change every millisecond, with the standard library
//! alone; a tool's watcher is told by the system instead (inotify, `ReadDirectoryChangesW`,
//! FSEvents), and gathers files saved together over an interval, which the bench leaves out. The
//! sizes written keep the file's length: only its time of change shows a save, which needs a file
//! system whose grain is finer than the runs' gap, as Linux's are, not FAT's 2 s.
//!
//! ```sh
//! nix develop
//! cargo run --release -p slime-benches -- --prototypes --style --runs 21
//! cargo run -p slime-benches -- --prototypes --style --runs 21    # as an author runs it
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant, SystemTime};

use uwui::prelude::ElementId;

use super::host::{Find, Host, found};
use super::reload::{kept, played, settled};
use super::signals::{HostWake, LIBRARY, Signals};
use super::{SETTLE, median};

/// The style sheet as the screen holds it, and the size of the title that each run changes.
const SHEET: &str = include_str!("sheets/library.style");
const TITLE: &str = "style.text.style = { size = 52, weight = 700 }";
/// The title's sizes that the runs write in turn, the sheet's own first.
const SIZES: [f32; 2] = [52.0, 44.0];

/// How long a run waits for its change before it fails.
const LIMIT: Duration = Duration::from_secs(10);
/// How often the watcher looks at the file.
const LOOK: Duration = Duration::from_millis(1);

/// A change that the watcher read.
struct Read {
    /// The file's text.
    text: String,
    /// When the watcher saw the change.
    noticed: Instant,
    /// When it had read the file.
    read: Instant,
}

/// The sheet with the title's size `size`.
fn sheet(size: f32) -> String {
    SHEET.replace(
        TITLE,
        &format!("style.text.style = {{ size = {size}, weight = 700 }}"),
    )
}

/// Writes `text` beside `path`, then renames it over `path`, as an editor saves; the time once it
/// is saved.
fn save(path: &Path, text: &str) -> Result<Instant, String> {
    let beside = path.with_extension("saving");
    std::fs::write(&beside, text).map_err(|error| format!("{}: {error}", beside.display()))?;
    std::fs::rename(&beside, path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Instant::now())
}

/// The file's time of change, if it can be read.
fn changed_at(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Watches `path` on a thread of its own until `stop`, and hands each change's text to the host,
/// which it wakes.
fn watch(path: PathBuf, host: Arc<HostWake>, stop: Arc<AtomicBool>) -> Receiver<Read> {
    let (sender, receiver) = mpsc::channel();
    // Read before the thread starts, so that a save made before its first look is a change.
    let mut last = changed_at(&path);
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(LOOK);
            let now = changed_at(&path);
            if now == last {
                continue;
            }
            last = now;
            let noticed = Instant::now();
            // A file renamed over the old one, as the bench saves it, is read whole.
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let read = Read {
                text,
                noticed,
                read: Instant::now(),
            };
            if sender.send(read).is_err() {
                return;
            }
            host.wake();
        }
    });
    receiver
}

/// A run: a change saved, to the frame that shows it.
struct Run {
    /// From the save to the watcher's sight of it.
    noticed: Duration,
    /// From the save to the file read.
    read: Duration,
    /// The style sheets and themes read, the rules made again.
    restyled: Duration,
    /// The frame that shows it.
    frame: Duration,
    /// From the save to the end of that frame.
    whole: Duration,
}

/// The title's size that the frame shows, on the game's page.
fn title_size(host: &Host<Signals<()>>) -> Option<f32> {
    let title: ElementId = found(&host.ui, host.root, Find::Class("title"))?;
    Some(host.ui.style(title)?.text.style.size)
}

/// Saves the sheet with the title's size `size`, waits for the watcher's text, styles the
/// interface again with it and runs the frame that shows it; an error if the state was not kept
/// or the size did not show.
fn run(
    host: &mut Host<Signals<()>>,
    path: &Path,
    changes: &Receiver<Read>,
    size: f32,
) -> Result<Run, String> {
    let before = kept(host);
    if !before.page || before.field.as_deref() != Some("star") {
        return Err(format!("the library not played to its page: {before:?}"));
    }
    let wake = host.program.host_wake();
    let saved = save(path, &sheet(size))?;
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
    let restyling = Instant::now();
    host.restyle("library.style", &change.text)?;
    let restyled = restyling.elapsed();
    let framing = Instant::now();
    let frame = host.frame(&[]);
    let shown = Instant::now();
    if title_size(host) != Some(size) {
        return Err(format!(
            "the title's size {size} not shown: {:?}",
            title_size(host)
        ));
    }
    // A style's change runs none of the application's code: no view, no binding.
    if frame.runs != 0 {
        return Err(format!("the change ran {} units of code", frame.runs));
    }
    host.settle(SETTLE);
    let after = kept(host);
    if after != before {
        return Err(format!("the state not kept: {before:?}, then {after:?}"));
    }
    Ok(Run {
        noticed: change.noticed.saturating_duration_since(saved),
        read: change.read.saturating_duration_since(saved),
        restyled,
        frame: shown - framing,
        whole: shown - saved,
    })
}

/// Plays the library, then changes its style sheet `runs` times, and prints the medians as
/// Markdown, the first change apart.
pub(crate) fn print_times(runs: usize) -> Result<(), String> {
    let mut host = played(&LIBRARY);
    settled(&mut host);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/style-reload/library.style");
    let folder = path.parent().ok_or("the sheet has a folder")?;
    std::fs::create_dir_all(folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    save(&path, &sheet(SIZES[0]))?;
    let stop = Arc::new(AtomicBool::new(false));
    let changes = watch(path.clone(), host.program.host_wake(), Arc::clone(&stop));
    // The watcher's first look before the first save, and times of change apart by more than the
    // file system's grain.
    std::thread::sleep(Duration::from_millis(50));
    let mut all = Vec::with_capacity(runs + 1);
    let mut failed = None;
    for count in 0..=runs {
        // Each run changes the size from the one that the last showed.
        let size = SIZES[(count + 1) % 2];
        match run(&mut host, &path, &changes, size) {
            Ok(run) => {
                eprintln!(
                    "run {count}: {:.1} ms from the save to its frame",
                    run.whole.as_secs_f64() * 1e3
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
    println!("### signals: a style sheet's change seen\n");
    println!(
        "| Change | Noticed | Read | Rules made again | Its frame | From the save to the frame |"
    );
    println!("|---|---|---|---|---|---|");
    let ms = |time: Duration| format!("{:.3}", time.as_secs_f64() * 1e3);
    println!(
        "| The session's first | {} | {} | {} | {} | {} |",
        ms(first.noticed),
        ms(first.read),
        ms(first.restyled),
        ms(first.frame),
        ms(first.whole),
    );
    let times = |of: fn(&Run) -> Duration| median(all.iter().map(of).collect());
    println!(
        "| The {} after it | {} | {} | {} | {} | {} |",
        all.len(),
        times(|run| run.noticed),
        times(|run| run.read),
        times(|run| run.restyled),
        times(|run| run.frame),
        times(|run| run.whole),
    );
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_holds_the_title_once() {
        assert_eq!(SHEET.matches(TITLE).count(), 1);
        assert!(sheet(44.0).contains("size = 44, weight = 700"));
    }

    #[test]
    fn a_change_saved_shows_with_the_state_kept() {
        let mut host = played(&LIBRARY);
        settled(&mut host);
        let before = kept(&host);
        assert_eq!(title_size(&host), Some(52.0));
        host.restyle("library.style", &sheet(44.0))
            .expect("the sheet reads");
        settled(&mut host);
        assert_eq!(title_size(&host), Some(44.0));
        assert_eq!(kept(&host), before);
    }

    #[test]
    fn a_change_keeps_the_theme_shown() {
        let mut host = played(&LIBRARY);
        settled(&mut host);
        host.ui.set_theme("light");
        settled(&mut host);
        host.restyle("library.style", &sheet(44.0))
            .expect("the sheet reads");
        let frame = host.frame(&[]);
        assert_eq!(host.ui.theme(), Some("light"));
        assert_eq!(title_size(&host), Some(44.0));
        assert_eq!(
            frame.runs, 0,
            "a style's change runs no code of the application"
        );
    }

    #[test]
    fn a_sheet_that_does_not_read_keeps_the_style() {
        let mut host = played(&LIBRARY);
        settled(&mut host);
        assert!(host.restyle("library.style", "[title\n").is_err());
        settled(&mut host);
        assert_eq!(title_size(&host), Some(52.0));
    }
}
