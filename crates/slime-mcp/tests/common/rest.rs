//! Whether a program is at rest (`docs/adr/0185-rest-checked-in-ci.md`): none of its threads leaves
//! its wait over 10 seconds, as the context switches that Linux counts for each thread, in
//! `/proc/<pid>/task/<tid>/status`, show. Every thread counts, the drivers' and the libraries'
//! with the program's own, and both kinds of switches: a thread that waits switches out once each
//! time it wakes and waits again (`voluntary_ctxt_switches`), one taken off its core while it runs
//! (`nonvoluntary_ctxt_switches`). A thread that runs without ever waiting, which may not switch
//! on a core left to it, shows in its time on the CPU (`/proc/<pid>/task/<tid>/stat`). A thread
//! that starts or ends meanwhile counts too, and a process that ended is not at rest. Reading
//! these files does not wake the program. A frame wakes the program's main thread: a program at
//! rest draws none.
//!
//! It holds only while nothing outside asks the program anything: the tests that use it send it
//! nothing while they watch it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long a program is watched: the quality bar's window (`docs/ROADMAP.md`, section 4).
const REST: Duration = Duration::from_secs(10);

/// How long a program must stay still before it is watched: what it was asked last, a frame of the
/// zone on a CPU driver, may take seconds to end.
const STILL: Duration = Duration::from_secs(1);

/// A thread's name, its context switches of both kinds, and its time on the CPU, in the system's
/// ticks, in and out of the kernel.
#[derive(PartialEq)]
struct Thread {
    name: String,
    switches: u64,
    ticks: u64,
}

/// Each thread of a process, by its id.
type Threads = BTreeMap<u32, Thread>;

/// The fields of a `stat` file of `/proc` after the program's name, which may hold spaces and
/// parentheses: the state first.
pub fn stat_fields(stat: &str) -> Vec<&str> {
    stat.rsplit_once(')')
        .map_or_else(Vec::new, |(_, fields)| fields.split_whitespace().collect())
}

/// Reads the threads of `pid`; an error if it ended, a zombie that its parent has not waited for
/// yet included. A thread that ends while it is read is left out, as one that ended before.
fn threads(pid: u32) -> Result<Threads, String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|error| format!("the process {pid} cannot be read: {error}"))?;
    if matches!(stat_fields(&stat).first(), Some(&("Z" | "X"))) {
        return Err(format!("the process {pid} ended"));
    }
    let tasks = fs::read_dir(format!("/proc/{pid}/task"))
        .map_err(|error| format!("the process {pid} cannot be read: {error}"))?;
    let mut threads = Threads::new();
    for task in tasks.flatten() {
        let Some(tid) = task.file_name().to_str().and_then(|tid| tid.parse().ok()) else {
            continue;
        };
        let (Ok(status), Ok(stat)) = (
            fs::read_to_string(task.path().join("status")),
            fs::read_to_string(task.path().join("stat")),
        ) else {
            continue;
        };
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
                .map(str::trim)
        };
        let switches = ["voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"]
            .map(|name| field(name).and_then(|count| count.parse::<u64>().ok()));
        // `utime` and `stime`, the 14th and 15th fields, the 12th and 13th after the name.
        let fields = stat_fields(&stat);
        let ticks = [11, 12].map(|at| fields.get(at).and_then(|ticks| ticks.parse::<u64>().ok()));
        let (Some(name), [Some(voluntary), Some(involuntary)], [Some(user), Some(kernel)]) =
            (field("Name"), switches, ticks)
        else {
            return Err(format!(
                "thread {tid} of {pid}: unreadable {status}\n{stat}"
            ));
        };
        threads.insert(
            tid,
            Thread {
                name: name.to_owned(),
                switches: voluntary + involuntary,
                ticks: user + kernel,
            },
        );
    }
    Ok(threads)
}

/// What changed between two readings of the same process: one line for each thread that
/// switched, ran, started or ended; empty when none did.
fn changes(before: &Threads, after: &Threads) -> String {
    let mut changes = String::new();
    for (tid, thread) in after {
        let name = &thread.name;
        match before.get(tid) {
            Some(earlier) if earlier == thread => {}
            // A thread that ended, its id given to a new one, differs too.
            Some(earlier) => {
                let _ = writeln!(
                    changes,
                    "  thread {tid} `{name}`: {} switches, {} ticks on the CPU",
                    thread.switches.abs_diff(earlier.switches),
                    thread.ticks.abs_diff(earlier.ticks)
                );
            }
            None => {
                let _ = writeln!(changes, "  thread {tid} `{name}`: started");
            }
        }
    }
    for (tid, thread) in before {
        if !after.contains_key(tid) {
            let _ = writeln!(changes, "  thread {tid} `{}`: ended", thread.name);
        }
    }
    changes
}

/// Waits until no thread of `pid` switches or runs for a second, for at most `settle`, then watches
/// it for 10 seconds: `Ok` if none of its threads switched, ran, started or ended meanwhile, and it
/// did not end; or what did.
pub fn at_rest(pid: u32, settle: Duration) -> Result<(), String> {
    let deadline = Instant::now() + settle;
    let mut before = threads(pid)?;
    loop {
        std::thread::sleep(STILL);
        let after = threads(pid)?;
        let changed = changes(&before, &after);
        before = after;
        if changed.is_empty() {
            break;
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "the process {pid} was never still for {STILL:?} within {settle:?}; in the last:\n\
                 {changed}"
            ));
        }
    }
    std::thread::sleep(REST);
    let changed = changes(&before, &threads(pid)?);
    if changed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the process {pid} woke in {REST:?} alone:\n{changed}"
        ))
    }
}

/// The process that `start_game` says it started, whatever it runs.
pub fn launched(started: &str) -> u32 {
    started
        .split("runs as process ")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|pid| pid.parse().ok())
        .unwrap_or_else(|| panic!("no process in {started:?}"))
}

/// The process that `start_game` says it started, which must run `program`: with `cargo run`, the
/// process would be cargo's, at rest whatever the game does.
pub fn pid(started: &str, program: &Path) -> u32 {
    let pid = launched(started);
    let runs = fs::read_link(format!("/proc/{pid}/exe")).expect("the process's program");
    let program = fs::canonicalize(program).expect("the program's path");
    assert_eq!(runs, program, "the process {pid} runs another program");
    pid
}
