//! The file that a timing of a reload saves and watches, shared by `style_reload.rs` and
//! `layout_reload.rs`. The save writes a new file and renames it over the old one, as an author's
//! editor does. The watcher stands for the development tool's: it looks at the file's time of
//! change every millisecond, with the standard library alone, and reads it once it changed. A tool
//! is told by the system instead (inotify, `ReadDirectoryChangesW`, FSEvents), and gathers files
//! saved together over an interval, which the timings leave out. A save shows only by its time of
//! change, which needs a file system whose grain is finer than the runs' gap, as Linux's are, not
//! FAT's 2 s.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime};

use super::signals::HostWake;

/// How long a run waits for its change before it fails.
pub(crate) const LIMIT: Duration = Duration::from_secs(10);
/// How often the watcher looks at the file.
pub(crate) const LOOK: Duration = Duration::from_millis(1);

/// A change that the watcher read.
pub(crate) struct Read {
    /// The file's text.
    pub(crate) text: String,
    /// When the watcher saw the change.
    pub(crate) noticed: Instant,
    /// When it had read the file.
    pub(crate) read: Instant,
}

/// Writes `text` beside `path`, then renames it over `path`, as an editor saves; the time once it
/// is saved.
pub(crate) fn save(path: &Path, text: &str) -> Result<Instant, String> {
    let beside = path.with_extension("saving");
    std::fs::write(&beside, text).map_err(|error| format!("{}: {error}", beside.display()))?;
    std::fs::rename(&beside, path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Instant::now())
}

/// The file's time of change, if it can be read.
pub(crate) fn changed_at(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// Watches `path` on a thread of its own until `stop`, and hands each change's text to the host,
/// which it wakes.
pub(crate) fn watch(path: PathBuf, host: Arc<HostWake>, stop: Arc<AtomicBool>) -> Receiver<Read> {
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
