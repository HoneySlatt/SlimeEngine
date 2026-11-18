//! What the host keeps of the runs that did not end well, in the application's crash directory,
//! `<state>/<app id>/crashes`: a *crash report* for each panic that ends a run, a *session's
//! marker* for each run while it lives, and the *tally* of the hours and crashes
//! (`docs/adr/0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md`,
//! `docs/adr/0306-uwuis-crash-reports-a-marker-held-by-a-lock-hours-counted-a-panic-ends-with-101.md`).
//! Part of the host's core, tested without a window. Nothing leaves the machine.
//!
//! A run holds the lock of its marker, `session-<start>-<pid>`, from its start to its end, and
//! removes it at its end; the system lets go of the lock when the process ends, however it ends.
//! The next start takes the lock of every marker that no live run holds: each is a run that did not
//! end, a panic, an abort, the system's killer of memory, a power cut, which it tells and counts
//! as a crash. The panic hook only keeps what a report needs of each panic on the host's thread;
//! the host writes the report, `crash-<time>-<pid>.txt`, once it caught a panic that ends the run,
//! and names it in the marker with the time the run lasted. The tally, `hours`, is changed under
//! the lock of `hours.lock`, which the start's scan and each end take, so that two runs never
//! count a marker twice.
//!
//! What fails here is told and the run goes on: a crash directory is never a reason to stop.

use std::cell::RefCell;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read as _, Seek as _, Write as _};
use std::panic;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// The crash reports kept, the newest; the older are removed at a start.
const REPORTS_KEPT: usize = 20;

/// The tally's first line, which names its version.
const TALLY_HEADER: &str = "# hours 1";

/// A marker's first line, which names its version.
const MARKER_HEADER: &str = "# session 1";

/// How long a start or an end waits for the tally's lock, which another run holds for a few
/// milliseconds; past it, a run stopped while it held the lock, and this one goes on without it.
const TALLY_WAIT: Duration = Duration::from_secs(1);

/// The host's version, written in each report.
const HOST_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tells `line` on the standard error, which may be closed or full: never panics, unlike
/// `eprintln!`.
pub(crate) fn tell(line: &str) {
    let _ = writeln!(io::stderr(), "{line}");
}

/// What the runs before this one left, found at its start: one that did not end.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Unended {
    /// When it started, in seconds since the Unix epoch, when its marker says.
    pub(crate) started: Option<u64>,
    /// How it ended, as its marker says.
    pub(crate) how: How,
}

/// How a run that did not end ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum How {
    /// A panic, after `lasted`, its report written to `report`.
    Panicked { lasted: Duration, report: PathBuf },
    /// Without a word: killed, aborted, out of memory, the power cut, or ended at once by a second
    /// termination signal.
    Unknown,
}

impl Unended {
    /// What the host tells of it, with where the system may keep its own report.
    pub(crate) fn told(&self) -> String {
        let started = self.started.map_or_else(
            || "at a time unknown".to_owned(),
            |at| format!("at {}", shown(at)),
        );
        match &self.how {
            How::Panicked { lasted, report } => format!(
                "the run started {started} ended by a panic after {} s; its report: {}",
                lasted.as_secs(),
                report.display()
            ),
            How::Unknown => format!(
                "the run started {started} did not end: it was killed or aborted, ran out of \
                 memory, lost its power, or was ended at once by a second termination signal; \
                 {SYSTEMS_REPORT}"
            ),
        }
    }
}

/// Where the system keeps its own report of a process that crashed, where it keeps one.
const SYSTEMS_REPORT: &str = if cfg!(target_os = "linux") {
    "where systemd keeps them, `coredumpctl list` shows its own report"
} else if cfg!(windows) {
    "the Event Viewer's Application log may hold the system's own report"
} else if cfg!(target_os = "macos") {
    "~/Library/Logs/DiagnosticReports may hold the system's own report"
} else {
    "the system may keep its own report"
};

/// The hours of use and the crashes counted, in the tally.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Tally {
    /// The seconds that the runs lasted, those ended by a panic included; a run that ended without
    /// a word counts none, its length unknown.
    pub(crate) seconds: u64,
    /// The runs ended, however they ended.
    pub(crate) sessions: u64,
    /// The runs that did not end: a panic or no word.
    pub(crate) crashes: u64,
}

impl Tally {
    /// The tally's text.
    fn text(self) -> String {
        format!(
            "{TALLY_HEADER}\nseconds {}\nsessions {}\ncrashes {}\n",
            self.seconds, self.sessions, self.crashes
        )
    }

    /// The tally that `text` holds.
    ///
    /// # Errors
    ///
    /// The tally of a later version, or a text that does not read, said.
    fn read(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        match lines.next() {
            Some(TALLY_HEADER) => {}
            Some(header) if header.starts_with("# hours ") => {
                return Err(format!("its version, `{header}`, is not this host's, 1"));
            }
            _ => return Err("its first line names no version".to_owned()),
        }
        let mut tally = Self::default();
        for line in lines {
            let (name, value) = line
                .split_once(' ')
                .ok_or_else(|| format!("its line `{line}` does not read"))?;
            let value: u64 = value
                .parse()
                .map_err(|_| format!("its line `{line}` holds no count"))?;
            match name {
                "seconds" => tally.seconds = value,
                "sessions" => tally.sessions = value,
                "crashes" => tally.crashes = value,
                _ => return Err(format!("its line `{line}` names nothing that it counts")),
            }
        }
        Ok(tally)
    }

    /// The tally with a run more that lasted `seconds`, a crash if `crashed`; each count stops at
    /// its largest value.
    fn with_run(self, seconds: u64, crashed: bool) -> Self {
        Self {
            seconds: self.seconds.saturating_add(seconds),
            sessions: self.sessions.saturating_add(1),
            crashes: self.crashes.saturating_add(u64::from(crashed)),
        }
    }
}

/// The run's crash directory, from its start to its end: its marker, held while the run lives, and
/// what a report says of it.
pub(crate) struct Crashes {
    directory: PathBuf,
    /// The marker's path and its file, whose lock the run holds; none where it could not be made.
    marker: Option<(PathBuf, File)>,
    /// When the run started, on a clock that does not count a suspend where the system's does not.
    started: Instant,
    facts: Facts,
}

impl Crashes {
    /// Starts a run of the application `app_id`, at `version` if given, in the user's state
    /// directory, at `now`: finds what the runs before left, counts it, removes the older reports,
    /// and holds this run's marker; tells on the standard error what it found and what failed.
    /// None, told, where the environment names no state directory or the crash directory cannot be
    /// made: the run goes on without reports.
    pub(crate) fn start(app_id: &str, version: Option<&str>, now: SystemTime) -> Option<Self> {
        let Some(state) = state_directory_in(cfg!(windows), |name| std::env::var_os(name)) else {
            warn("no state directory: the host keeps no crash report of this run");
            return None;
        };
        let directory = state.join(directory_name(app_id)).join("crashes");
        if let Err(error) = std::fs::create_dir_all(&directory) {
            warn(&format!(
                "the host keeps no crash report of this run: {}: {error}",
                directory.display()
            ));
            return None;
        }
        let facts = Facts::of_this_run(app_id, version, now);
        let (crashes, unended) = Self::start_in(directory, facts, now);
        for run in unended {
            let told = run.told();
            tracing::warn!("{told}");
            tell(&format!("uwui's desktop host: {told}"));
        }
        Some(crashes)
    }

    /// Starts a run in the crash directory `directory`, which exists, as [`start`](Self::start)
    /// says: the runs found that did not end.
    fn start_in(directory: PathBuf, facts: Facts, now: SystemTime) -> (Self, Vec<Unended>) {
        let mut crashes = Self {
            directory,
            marker: None,
            started: Instant::now(),
            facts,
        };
        let Some(_counting) = hold_tally(&crashes.directory) else {
            return (crashes, Vec::new());
        };
        let found = held_markers(&crashes.directory);
        let unended: Vec<Unended> = found
            .iter()
            .map(|(_, _, text)| read_marker(text, &crashes.directory))
            .collect();
        // Counted before the markers go: a run stopped between the two counts a crash twice, never
        // none, so that the share of hours without a crash is never overstated.
        if !unended.is_empty()
            && let Some(tally) = read_tally(&crashes.directory)
        {
            let tally = unended.iter().fold(tally, |tally, run| {
                let lasted = match run.how {
                    How::Panicked { lasted, .. } => lasted.as_secs(),
                    How::Unknown => 0,
                };
                tally.with_run(lasted, true)
            });
            if let Err(error) = write_tally(&crashes.directory, tally) {
                warn(&format!("the crashes found are not counted: {error}"));
            }
        }
        for (path, marker, _) in found {
            drop(marker);
            if let Err(error) = std::fs::remove_file(&path) {
                warn(&format!("a marker stays, {}: {error}", path.display()));
            }
        }
        remove_older_reports(&crashes.directory);
        crashes.marker = make_marker(&crashes.directory, now);
        (crashes, unended)
    }

    /// The GPU and its driver, which the report says from now on.
    pub(crate) fn set_gpu(&mut self, gpu: &str) {
        self.facts.gpu = Some(gpu.to_owned());
    }

    /// The run ended cleanly: its time counted, its marker removed. A failure is told.
    pub(crate) fn end(mut self) {
        let counting = hold_tally(&self.directory);
        if counting.is_some()
            && let Some(tally) = read_tally(&self.directory)
        {
            let tally = tally.with_run(self.started.elapsed().as_secs(), false);
            if let Err(error) = write_tally(&self.directory, tally) {
                warn(&format!("this run is not counted: {error}"));
            }
        }
        // Under the tally's lock where it was taken, so that no start takes the marker between its
        // lock let go and its removal, where Windows refuses to remove a file open.
        if let Some((path, marker)) = self.marker.take() {
            drop(marker);
            if let Err(error) = std::fs::remove_file(&path) {
                warn(&format!(
                    "the marker stays, and the next start counts this run as a crash, {}: {error}",
                    path.display()
                ));
            }
        }
        drop(counting);
    }

    /// A panic of `message` ended the run: writes its report from what the panic hook kept of the
    /// last panic on the host's thread, names it in the marker, which stays for the next start, and
    /// tells where it lies. A failure is told.
    pub(crate) fn panicked(mut self, message: &str) {
        let recorded = LAST_PANIC
            .try_with(|last| last.try_borrow_mut().ok()?.take())
            .ok()
            .flatten();
        let recorded = recorded.unwrap_or_else(|| Recorded {
            message: message.to_owned(),
            location: "unknown".to_owned(),
            thread: "unknown".to_owned(),
            backtrace: "none kept".to_owned(),
            at: SystemTime::now(),
            when: Instant::now(),
        });
        let lasted = recorded.when.saturating_duration_since(self.started);
        let at = since_epoch(recorded.at);
        let panic = Panic {
            message: &recorded.message,
            location: &recorded.location,
            thread: &recorded.thread,
            backtrace: &recorded.backtrace,
            at,
            lasted,
        };
        let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok();
        let text = report(&self.facts, &panic, home.as_deref());
        let name = format!("crash-{}-{}.txt", stamp(at), std::process::id());
        let path = self.directory.join(&name);
        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .and_then(|mut file| {
                file.write_all(text.as_bytes())?;
                file.sync_all()
            });
        if let Err(error) = written {
            warn(&format!(
                "no crash report is written, {}: {error}",
                path.display()
            ));
            return;
        }
        tell(&format!(
            "uwui's desktop host wrote a crash report: {}",
            path.display()
        ));
        // Through the file that holds the lock: Windows refuses a write through another handle of
        // a file locked whole.
        if let Some((_, marker)) = &mut self.marker {
            let named = marker.seek(io::SeekFrom::End(0)).and_then(|_| {
                writeln!(marker, "panicked {} {name}", lasted.as_secs())?;
                marker.sync_all()
            });
            if let Err(error) = named {
                warn(&format!("the marker does not name the report: {error}"));
            }
        }
    }
}

/// Tells `warning` in the log and on the standard error, which an application without a log
/// subscriber still shows.
fn warn(warning: &str) {
    tracing::warn!("{warning}");
    tell(&format!("uwui's desktop host: {warning}"));
}

/// Makes and holds this run's marker in `directory`, started at `now`; none, told, where it cannot
/// be, the file left removed.
fn make_marker(directory: &Path, now: SystemTime) -> Option<(PathBuf, File)> {
    let at = since_epoch(now);
    let path = directory.join(format!("session-{}-{}", stamp(at), std::process::id()));
    let made = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut marker| {
            match marker.try_lock() {
                Ok(()) => {}
                Err(TryLockError::WouldBlock) => {
                    return Err(io::Error::other("another run holds it"));
                }
                Err(TryLockError::Error(error)) => return Err(error),
            }
            // Not synced, which would cost the start a write to the disk: a power cut within the
            // seconds that the system takes to write it loses the marker, and that crash is not
            // found.
            marker.write_all(format!("{MARKER_HEADER}\nstarted {at}\n").as_bytes())?;
            Ok(marker)
        });
    match made {
        Ok(marker) => Some((path, marker)),
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            warn(&format!(
                "this run has no marker, {}: {error}",
                path.display()
            ));
            None
        }
    }
}

/// Takes the tally's lock, waiting for another run's for [`TALLY_WAIT`] at most; none, told,
/// where it cannot be taken.
fn hold_tally(directory: &Path) -> Option<File> {
    let path = directory.join("hours.lock");
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path);
    let file = match file {
        Ok(file) => file,
        Err(error) => {
            warn(&format!(
                "the runs are not counted, {}: {error}",
                path.display()
            ));
            return None;
        }
    };
    let deadline = Instant::now() + TALLY_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Some(file),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(TryLockError::WouldBlock) => {
                warn(&format!(
                    "the runs are not counted: another run holds {}",
                    path.display()
                ));
                return None;
            }
            Err(TryLockError::Error(error)) => {
                warn(&format!(
                    "the runs are not counted, {}: {error}",
                    path.display()
                ));
                return None;
            }
        }
    }
}

/// The markers in `directory` that no live run holds, each with its path, its file, whose lock is
/// now this run's, and its text: those of a later version left as they are.
fn held_markers(directory: &Path) -> Vec<(PathBuf, File, String)> {
    let names = match names_in(directory, "session-") {
        Ok(names) => names,
        Err(error) => {
            warn(&format!("the runs before are not read: {error}"));
            return Vec::new();
        }
    };
    let mut held = Vec::new();
    for name in names {
        let path = directory.join(&name);
        let Ok(mut marker) = OpenOptions::new().read(true).write(true).open(&path) else {
            continue;
        };
        // A live run holds it; one that cannot be locked is left, as a live run's would be.
        if marker.try_lock().is_err() {
            continue;
        }
        let mut text = String::new();
        // A marker that does not read is a run that did not end all the same.
        let _ = marker.read_to_string(&mut text);
        if text
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("# session ") && line != MARKER_HEADER)
        {
            continue;
        }
        held.push((path, marker, text));
    }
    held
}

/// The names of the files in `directory` that start with `start`, sorted, which sorts them by time.
fn names_in(directory: &Path, start: &str) -> io::Result<Vec<OsString>> {
    let mut names: Vec<OsString> = std::fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name())
        .filter(|name| name.to_string_lossy().starts_with(start))
        .collect();
    names.sort();
    Ok(names)
}

/// What the marker `text` says of its run, in `directory`; a marker that does not read, a run of
/// which nothing is known.
fn read_marker(text: &str, directory: &Path) -> Unended {
    let mut started = None;
    let mut how = How::Unknown;
    for line in text.lines() {
        let mut words = line.splitn(3, ' ');
        match (words.next(), words.next(), words.next()) {
            (Some("started"), Some(at), None) => started = at.parse().ok(),
            (Some("panicked"), Some(lasted), Some(report)) => {
                if let Ok(lasted) = lasted.parse() {
                    how = How::Panicked {
                        lasted: Duration::from_secs(lasted),
                        report: directory.join(report),
                    };
                }
            }
            _ => {}
        }
    }
    Unended { started, how }
}

/// The tally in `directory`: zero where it has none; none, told, where it does not read, which is
/// then left as it is, never written over.
fn read_tally(directory: &Path) -> Option<Tally> {
    let path = directory.join("hours");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Some(Tally::default()),
        Err(error) => {
            warn(&format!(
                "the runs are not counted, {}: {error}",
                path.display()
            ));
            return None;
        }
    };
    match Tally::read(&text) {
        Ok(tally) => Some(tally),
        Err(error) => {
            warn(&format!(
                "the runs are not counted, {}: {error}",
                path.display()
            ));
            None
        }
    }
}

/// Writes `tally` in `directory`, whole or not at all: a file beside it, synced, then renamed,
/// the file beside removed if that fails. The directory is not synced: a power cut just after may
/// bring the tally before back, a run uncounted.
fn write_tally(directory: &Path, tally: Tally) -> io::Result<()> {
    let beside = directory.join(format!("hours.{}", std::process::id()));
    let written = File::create(&beside)
        .and_then(|mut file| {
            file.write_all(tally.text().as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&beside, directory.join("hours")));
    if written.is_err() {
        let _ = std::fs::remove_file(&beside);
    }
    written
}

/// Removes the reports in `directory` but the [`REPORTS_KEPT`] newest, by their names, which start
/// with their time; one that cannot be removed is told and left.
fn remove_older_reports(directory: &Path) {
    let reports = match names_in(directory, "crash-") {
        Ok(reports) => reports,
        Err(error) => {
            warn(&format!("the older reports are not removed: {error}"));
            return;
        }
    };
    let older = reports.len().saturating_sub(REPORTS_KEPT);
    for name in &reports[..older] {
        let path = directory.join(name);
        if let Err(error) = std::fs::remove_file(&path) {
            warn(&format!(
                "an older report stays, {}: {error}",
                path.display()
            ));
        }
    }
}

/// The user's state directory: `%LOCALAPPDATA%` on Windows as `windows` says; elsewhere
/// `$XDG_STATE_HOME`, else `$HOME/.local/state`, as the XDG base directories say; none where `var`
/// gives none, or only relative paths, which they say to ignore. The rule of `slime-files`, which
/// is under the engine's licence (`docs/debt.md`).
fn state_directory_in(windows: bool, var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name: &str| {
        var(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    if windows {
        return absolute("LOCALAPPDATA");
    }
    absolute("XDG_STATE_HOME").or_else(|| Some(absolute("HOME")?.join(".local").join("state")))
}

/// The application's directory's name for its identity: its ASCII letters and digits, `.`, `-`
/// and `_`, any other character `_`, so that an identity never names a path elsewhere, nor a
/// device of Windows, `CON` or `NUL`, which a `_` before it keeps from being one.
fn directory_name(app_id: &str) -> String {
    let name: String = app_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.ends_with(|c: char| c.is_ascii_digit()));
    if name.is_empty() || name.chars().all(|c| c == '.') || device {
        format!("_{name}")
    } else {
        name
    }
}

/// The seconds of `at` since the Unix epoch; zero before it.
fn since_epoch(at: SystemTime) -> u64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The date and time in UTC of `secs` since the Unix epoch: year, month, day, hour, minute, second.
fn utc(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    // Howard Hinnant's `civil_from_days`, on days since 1970-01-01, which are never negative here.
    let days = secs / 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    let time = secs % 86_400;
    (year, month, day, time / 3_600, time / 60 % 60, time % 60)
}

/// `secs` since the Unix epoch in a file's name, which sorts as its time: `20261118T143005Z`.
fn stamp(secs: u64) -> String {
    let (year, month, day, hour, minute, second) = utc(secs);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

/// `secs` since the Unix epoch as a person reads it: `2026-11-18 14:30:05 UTC`.
fn shown(secs: u64) -> String {
    let (year, month, day, hour, minute, second) = utc(secs);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

/// What a crash report says of the run that panicked, besides the panic.
#[derive(Clone)]
pub(crate) struct Facts {
    /// The application's identity and version, if it gave one.
    pub(crate) application: String,
    /// The executable's path.
    pub(crate) executable: String,
    /// The operating system, its version and the processor's architecture.
    pub(crate) system: String,
    /// The GPU and its driver, once the device opened.
    pub(crate) gpu: Option<String>,
    /// When the run started, in seconds since the Unix epoch.
    pub(crate) started: u64,
}

impl Facts {
    /// The facts of this run of `app_id`, at `version` if given, started at `now`.
    fn of_this_run(app_id: &str, version: Option<&str>, now: SystemTime) -> Self {
        let application = match version {
            Some(version) => format!("{app_id} {version}"),
            None => format!("{app_id}, its version not given"),
        };
        let executable = std::env::current_exe().map_or_else(
            |error| format!("unknown: {error}"),
            |path| path.display().to_string(),
        );
        Self {
            application,
            executable,
            system: system(),
            gpu: None,
            started: since_epoch(now),
        }
    }
}

/// The operating system, its version where it says it, and the processor's architecture.
fn system() -> String {
    let mut system = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    if cfg!(target_os = "linux") {
        let release = std::fs::read_to_string("/etc/os-release").ok();
        let name = release.as_deref().and_then(|release| {
            release.lines().find_map(|line| {
                Some(
                    line.strip_prefix("PRETTY_NAME=")?
                        .trim_matches('"')
                        .to_owned(),
                )
            })
        });
        if let Some(name) = name {
            let _ = write!(system, ", {name}");
        }
        if let Ok(kernel) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
            let _ = write!(system, ", kernel {}", kernel.trim());
        }
    }
    system
}

/// A panic as its report tells it.
pub(crate) struct Panic<'a> {
    pub(crate) message: &'a str,
    /// Its file, line and column.
    pub(crate) location: &'a str,
    pub(crate) thread: &'a str,
    pub(crate) backtrace: &'a str,
    /// When it came, in seconds since the Unix epoch.
    pub(crate) at: u64,
    pub(crate) lasted: Duration,
}

/// The report of `panic`, in a run of `facts`, the home directory `home` written `~`.
pub(crate) fn report(facts: &Facts, panic: &Panic<'_>, home: Option<&str>) -> String {
    let Facts {
        application,
        executable,
        system,
        gpu,
        started,
    } = facts;
    let gpu = gpu.as_deref().unwrap_or("none opened yet");
    let text = format!(
        "uwui's desktop host's crash report 1\n\
         \n\
         application: {application}\n\
         host: uwui-desktop {HOST_VERSION}\n\
         executable: {executable}\n\
         system: {system}\n\
         GPU: {gpu}\n\
         started: {}\n\
         panicked: {}, after {} s\n\
         thread: {}\n\
         at: {}\n\
         log: none kept, the host keeping no log of its own\n\
         \n\
         message:\n{}\n\
         \n\
         backtrace:\n{}\n",
        shown(*started),
        shown(panic.at),
        panic.lasted.as_secs(),
        panic.thread,
        panic.location,
        panic.message,
        panic.backtrace.trim_end(),
    );
    match home.map(|home| home.trim_end_matches(['/', '\\'])) {
        Some(home) if !home.is_empty() => text.replace(home, "~"),
        _ => text,
    }
}

/// The panic's message, as its payload gives it.
pub(crate) fn message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "without a message".to_owned())
}

/// What the panic hook kept of a panic on the host's thread, for its report.
struct Recorded {
    message: String,
    location: String,
    thread: String,
    backtrace: String,
    at: SystemTime,
    when: Instant,
}

thread_local! {
    /// The last panic on the host's thread: the hook keeps each, the next replacing it, since a
    /// panic that the application catches itself ends no run; the host takes the one that did.
    static LAST_PANIC: RefCell<Option<Recorded>> = const { RefCell::new(None) };
}

/// Installs the panic hook that keeps what a report needs of each panic on this thread, the
/// host's, then runs the hook installed before it; a panic of another thread goes to that hook
/// alone. The hook stays the process's until it ends. It writes nothing: a panic in a panic's hook
/// aborts the process, and a write may fail.
pub(crate) fn install_hook() {
    let host = std::thread::current().id();
    let before = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        if thread.id() == host {
            let recorded = Recorded {
                message: info
                    .payload_as_str()
                    .unwrap_or("without a message")
                    .to_owned(),
                location: info.location().map_or_else(
                    || "unknown".to_owned(),
                    |at| format!("{}:{}:{}", at.file(), at.line(), at.column()),
                ),
                thread: thread.name().unwrap_or("unnamed").to_owned(),
                backtrace: std::backtrace::Backtrace::force_capture().to_string(),
                at: SystemTime::now(),
                when: Instant::now(),
            };
            let _ = LAST_PANIC.try_with(|last| {
                if let Ok(mut last) = last.try_borrow_mut() {
                    *last = Some(recorded);
                }
            });
        }
        before(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A crash directory of the test's own, made empty; removed by [`Removed`].
    fn directory(name: &str) -> (PathBuf, Removed) {
        let directory = std::env::temp_dir()
            .join(format!("uwui-desktop-crash-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("its directory");
        (directory.clone(), Removed(directory))
    }

    /// Removes its directory as the test ends.
    struct Removed(PathBuf);

    impl Drop for Removed {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn facts() -> Facts {
        Facts {
            application: "dev.uwui.Test 1.2.3".to_owned(),
            executable: "/home/someone/bin/test".to_owned(),
            system: "linux x86_64".to_owned(),
            gpu: None,
            started: 1_795_012_205,
        }
    }

    fn start(directory: &Path, at: u64) -> (Crashes, Vec<Unended>) {
        Crashes::start_in(
            directory.to_owned(),
            facts(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(at),
        )
    }

    fn tally(directory: &Path) -> Tally {
        Tally::read(&std::fs::read_to_string(directory.join("hours")).expect("a tally"))
            .expect("it reads")
    }

    fn marker(run: &Crashes) -> PathBuf {
        run.marker.as_ref().expect("a marker").0.clone()
    }

    #[test]
    fn dates_are_told_in_utc() {
        assert_eq!(shown(0), "1970-01-01 00:00:00 UTC");
        // 2026-11-18 14:30:05 UTC.
        assert_eq!(shown(1_795_012_205), "2026-11-18 14:30:05 UTC");
        assert_eq!(stamp(1_795_012_205), "20261118T143005Z");
        // A leap day, and the last second of a year.
        assert_eq!(shown(951_782_400), "2000-02-29 00:00:00 UTC");
        assert_eq!(shown(1_798_761_599), "2026-12-31 23:59:59 UTC");
    }

    #[test]
    fn an_identity_never_names_a_path_elsewhere_nor_a_device() {
        assert_eq!(
            directory_name("dev.uwui.Application"),
            "dev.uwui.Application"
        );
        assert_eq!(directory_name("../a/b"), ".._a_b");
        assert_eq!(directory_name(".."), "_..");
        assert_eq!(directory_name(""), "_");
        assert_eq!(directory_name("C:\\x"), "C__x");
        assert_eq!(directory_name("con"), "_con");
        assert_eq!(directory_name("NUL.app"), "_NUL.app");
        assert_eq!(directory_name("COM1"), "_COM1");
        assert_eq!(directory_name("COMPANY"), "COMPANY");
    }

    #[test]
    fn the_state_directory_is_the_one_the_environment_gives() {
        let (state, home) = if cfg!(windows) {
            ("C:/state", "C:/home")
        } else {
            ("/state", "/home")
        };
        let environment = |given: &[(&str, &str)]| {
            let given: Vec<(String, OsString)> = given
                .iter()
                .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
                .collect();
            move |name: &str| {
                given
                    .iter()
                    .find(|(variable, _)| variable == name)
                    .map(|(_, value)| value.clone())
            }
        };
        let linux = |given: &[(&str, &str)]| state_directory_in(false, environment(given));
        assert_eq!(
            linux(&[("XDG_STATE_HOME", state), ("HOME", home)]),
            Some(PathBuf::from(state))
        );
        // A relative path is ignored, as the XDG base directories say.
        assert_eq!(
            linux(&[("XDG_STATE_HOME", "state"), ("HOME", home)]),
            Some(PathBuf::from(home).join(".local").join("state"))
        );
        assert_eq!(linux(&[]), None);
        assert_eq!(
            state_directory_in(
                true,
                environment(&[("LOCALAPPDATA", state), ("HOME", home)])
            ),
            Some(PathBuf::from(state))
        );
    }

    #[test]
    fn a_tally_of_a_later_version_or_that_does_not_read_is_refused() {
        let tally = Tally {
            seconds: 7,
            sessions: 2,
            crashes: 1,
        };
        assert_eq!(Tally::read(&tally.text()), Ok(tally));
        assert!(
            Tally::read("# hours 2\n")
                .expect_err("later")
                .contains("version")
        );
        assert!(Tally::read("seconds 1\n").is_err());
        assert!(Tally::read("# hours 1\nseconds many\n").is_err());
        assert!(Tally::read("# hours 1\nminutes 1\n").is_err());
    }

    #[test]
    fn a_tally_at_its_largest_counts_no_further() {
        let full = Tally {
            seconds: u64::MAX,
            sessions: u64::MAX,
            crashes: u64::MAX,
        };
        assert_eq!(full.with_run(9, true), full);
    }

    #[test]
    fn a_run_holds_its_marker_and_removes_it_at_its_end() {
        let (directory, _removed) = directory("clean");
        let (run, unended) = start(&directory, 100);
        assert!(unended.is_empty());
        let marker = marker(&run);
        let name = marker
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        assert!(name.starts_with("session-19700101T000140Z-"), "{name}");
        // A second run finds the first live, and leaves its marker.
        let (second, unended) = start(&directory, 101);
        assert!(unended.is_empty());
        assert!(marker.exists());
        run.end();
        assert!(!marker.exists());
        second.end();
        assert_eq!(tally(&directory).sessions, 2);
        assert_eq!(tally(&directory).crashes, 0);
    }

    #[test]
    fn the_next_start_finds_a_run_that_did_not_end() {
        let (directory, _removed) = directory("crashed");
        let (mut run, _) = start(&directory, 1_795_012_205);
        let marker = marker(&run);
        // A run that ends without its end: its lock let go as its process's would be.
        run.marker = None;
        // One that panicked, as the host leaves its marker; and one of a later version, left.
        std::fs::write(
            directory.join("session-20261118T150000Z-1"),
            format!("{MARKER_HEADER}\nstarted 1795014000\npanicked 42 crash-x.txt\n"),
        )
        .expect("a marker");
        let later = directory.join("session-20261118T160000Z-2");
        std::fs::write(&later, "# session 2\n").expect("a marker");
        let (next, unended) = start(&directory, 1_795_020_000);
        assert_eq!(
            unended,
            [
                Unended {
                    started: Some(1_795_012_205),
                    how: How::Unknown
                },
                Unended {
                    started: Some(1_795_014_000),
                    how: How::Panicked {
                        lasted: Duration::from_secs(42),
                        report: directory.join("crash-x.txt")
                    }
                },
            ]
        );
        assert!(!marker.exists());
        assert!(later.exists(), "a later version's marker was removed");
        assert!(
            unended[0]
                .told()
                .contains("2026-11-18 14:30:05 UTC did not end")
        );
        assert!(unended[0].told().contains("second termination signal"));
        assert!(unended[1].told().contains("ended by a panic after 42 s"));
        assert_eq!(
            tally(&directory),
            Tally {
                seconds: 42,
                sessions: 2,
                crashes: 2
            }
        );
        next.end();
        assert_eq!(tally(&directory).sessions, 3);
    }

    #[test]
    fn a_panic_writes_its_report_and_names_it_in_the_marker_that_stays() {
        let (directory, _removed) = directory("panicked");
        let (mut run, _) = start(&directory, 1_795_012_205);
        run.set_gpu("llvmpipe, driver llvmpipe, Vulkan");
        let marker = marker(&run);
        // Two panics kept by the hook: the last, which ended the run, is the one reported.
        install_hook();
        for message in ["caught by the application", "the one that ends the run"] {
            let _ = panic::catch_unwind(|| panic::panic_any(message));
        }
        run.panicked("the one that ends the run");
        let reports = names_in(&directory, "crash-").expect("the directory reads");
        assert_eq!(reports.len(), 1);
        let report = std::fs::read_to_string(directory.join(&reports[0])).expect("the report");
        assert!(
            report.contains("message:\nthe one that ends the run\n"),
            "{report}"
        );
        assert!(
            report.contains("GPU: llvmpipe, driver llvmpipe, Vulkan"),
            "{report}"
        );
        assert!(
            report.contains("at: crates/uwui-desktop/src/crash.rs:"),
            "{report}"
        );
        let text = std::fs::read_to_string(&marker).expect("the marker stays");
        let name = reports[0].to_string_lossy();
        assert!(text.ends_with(&format!(" {name}\n")), "{text}");
        let (next, unended) = start(&directory, 1_795_020_000);
        assert!(matches!(
            unended[..],
            [Unended {
                how: How::Panicked { .. },
                ..
            }]
        ));
        next.end();
    }

    #[test]
    fn a_tally_that_does_not_read_is_left_as_it_is() {
        let (directory, _removed) = directory("unread");
        std::fs::write(directory.join("hours"), "# hours 2\nweeks 1\n").expect("a tally");
        let (run, _) = start(&directory, 0);
        run.end();
        assert_eq!(
            std::fs::read_to_string(directory.join("hours")).expect("the tally"),
            "# hours 2\nweeks 1\n"
        );
    }

    #[test]
    fn only_the_newest_reports_are_kept() {
        let (directory, _removed) = directory("reports");
        for n in 0..REPORTS_KEPT + 3 {
            std::fs::write(directory.join(format!("crash-{n:03}.txt")), "").expect("a report");
        }
        std::fs::write(directory.join("other.txt"), "").expect("another file");
        let (run, _) = start(&directory, 0);
        run.end();
        assert!(!directory.join("crash-002.txt").exists());
        assert!(directory.join("crash-003.txt").exists());
        assert!(directory.join("other.txt").exists());
    }

    #[test]
    fn a_report_tells_the_panic_and_its_run_the_home_hidden() {
        let mut facts = facts();
        facts.gpu = Some("llvmpipe, driver llvmpipe, Vulkan".to_owned());
        let panic = Panic {
            message: "made to panic",
            location: "src/main.rs:3:5",
            thread: "main",
            backtrace: "   0: main\n             at /home/someone/src/main.rs:3:5\n",
            at: 1_795_012_209,
            lasted: Duration::from_secs(4),
        };
        let text = report(&facts, &panic, Some("/home/someone/"));
        assert!(!text.contains("/home/someone"), "{text}");
        for told in [
            "application: dev.uwui.Test 1.2.3",
            "executable: ~/bin/test",
            "GPU: llvmpipe, driver llvmpipe, Vulkan",
            "started: 2026-11-18 14:30:05 UTC",
            "panicked: 2026-11-18 14:30:09 UTC, after 4 s",
            "at: src/main.rs:3:5",
            "message:\nmade to panic\n",
            "at ~/src/main.rs:3:5",
        ] {
            assert!(text.contains(told), "{told} not in {text}");
        }
        // A home that is the root alone hides nothing.
        assert_eq!(
            report(&facts, &panic, Some("//")),
            report(&facts, &panic, None)
        );
    }

    #[test]
    fn a_panics_message_is_its_payloads_text() {
        assert_eq!(message(&"static"), "static");
        assert_eq!(message(&String::from("owned")), "owned");
        assert_eq!(message(&7_u32), "without a message");
    }
}
