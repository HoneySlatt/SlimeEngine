# 0306. uwui's crash reports: a marker held by a lock, the hours counted; a panic ends with 101

- Status: Accepted; amends [0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)
  (no lines of the log in a crash report while the host keeps no log)
- Date: 2026-11-18

## Context

M6d.7a2b3, the last part of [ADR-0302](0302-m6d7a2b-cut-in-three-the-platform-layer-runs-the-loop-waylands-mouse-and-touch.md)'s
cut, builds in uwui's desktop host, `uwui-desktop`, the crash report and the session's marker of
[ADR-0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md),
[ADR-0298](0298-a-panic-ends-a-run-without-stop-the-same-rule-in-both-hosts.md)'s rule of panics,
and a termination asked twice that ends the host.

ADR-0270 decided what a report holds and what a marker is for, not where they lie, how a marker is
told from a live run's, nor how the hours are counted. The facts, read on 2026-11-18:

- No program of the repository writes a crash report yet: the engine's is planned for M6c.Q4
  ([ADR-0176](0176-fuzzing-and-crash-reports.md)); `slime-app`'s hook only logs.
- `uwui-desktop` keeps no log: it emits `tracing`'s events, and the application chooses where they
  go, or installs no subscriber, as the example does. A report cannot hold "the last lines of the
  log" without a log of the host's own, or a layer that each application adds.
- `slime-files` holds the engine's safe writes, its held files and the user's directories
  ([ADR-0297](0297-one-crate-of-safe-files-and-the-editors-journal-versioned-built-by-s1.md)), under
  the engine's licence; uwui is under MIT OR Apache-2.0 and leaves the engine's repository at M6d.11.
- Rust 1.89 gives a file's lock, `File::lock` and `File::try_lock`, which the system lets go when the
  process ends, however it ends. `slime-files`' `HeldFile` stands on it.
- Several runs of one application may live at once; a process's id is reused.
- The engine ends at a second termination signal half a second or more after the first, with
  status 130 (`crates/slime-app/src/signal.rs`); uwui's host ended only when killed.
- S1c builds ADR-0298 in `slime-app` at the same time, catching a panic around each callback of its
  loops; its `run` borrows the game, and returns `RunError::Panicked` with the panic's text.

## Decision

The owner's choices of 2026-11-18: the hours counted now; no lines of the log in a report until uwui
has a log. The rest follows from ADR-0270 and ADR-0298, written here since a newcomer would ask.

**Where.** Each application has a crash directory, `crashes` in a directory named by its identity,
the window's app id, in the user's state directory: `$XDG_STATE_HOME`, else `~/.local/state`;
`%LOCALAPPDATA%` on Windows. A character of the identity other than an ASCII letter, a digit, `.`,
`-` and `_` is written `_`, and a name of a Windows device, `CON` or `NUL`, gets a `_` before it, so
that it never names a path elsewhere. Nothing leaves the machine. What fails there is told on the
standard error, and the run goes on: a crash directory is never a reason to stop.

**A marker held by a lock.** A run makes its marker, `session-<start>-<pid>`, before its window
opens, and holds the file's lock until its end, when it removes it. A start takes the lock of every
marker that no live run holds: each is a run that did not end, which it tells on the standard error
with where the system may keep its own report, counts, and removes once counted. A marker of a later
version is left as it is. The marker is not synced at the start, which would cost a write to the
disk: a power cut in the seconds after a start loses it.

**The report.** The host installs a panic hook, then runs the one before it. The hook writes
nothing, since a panic in a hook aborts the process and a write may fail: it keeps what a report
needs of each panic on the host's thread, the next replacing it. Once the host caught a panic that
ends the run, it writes `crash-<time>-<pid>.txt` from the last one kept: the message, where it came,
the backtrace, the application's identity and version (`WindowSettings::version`), the host's
version, the executable, the system, the GPU and its driver, the start and the time of the panic,
the running user's home directory written `~`; it says that no log is kept. It names the report in
the marker, through the file whose lock it holds, with the time the run lasted. A panic that the
application catches itself, or one on another thread, writes none: it ends no run. The 20 newest
reports are kept. The paths of the machine that built the program stay in the backtrace: a program
built for others hides them with `--remap-path-prefix`.

**The hours.** `hours` holds a version's line, the seconds that the runs lasted, the runs ended and
the crashes. A clean end adds its time and a run; a marker found adds a run, a crash, and the time
of a run ended by a panic; a run killed counts no time, its length unknown, so that the share of
hours without a crash is never overstated. The markers found are counted before they are removed,
for the same reason. Its lock, `hours.lock`, is held by the start's scan and by each end, so that two
runs never count a marker twice; a run waits a second for it at most, then goes on without
counting. It is written beside, synced, then renamed; one of a later version, or that does not
read, is left as it is and nothing counted. Each count stops at its largest value.

**A panic ends with 101.** A panic in any call of the layer to the host, the application's code,
uwui's or the host's, and in `App::stop`, is caught before it leaves the call. The application is
called no more and its value never dropped, since its `Drop` is its code too: the host holds it in a
guard that a panic's unwinding forgets, a panic of the layer's own code included. The device and
the window are still let go of; the process ends with `EXIT_PANICKED`, 101, Rust's status of a
panic, the panic told on the standard error, and the marker left. A failure of the host, or of the
application's start, ends cleanly: the host told why. A panic after a failure replaces its status,
the failure told.

Not dropping the program is uwui's host's alone. Its `run` owns the application and never
returns; the engine's `run` borrows the game, which its caller owns and drops as it chooses after
`run` returns `RunError::Panicked`. ADR-0298 asks both hosts not to call the program after a panic,
which both do.

**A second termination ends at once.** As in the engine: a second signal, half a second or more
after the first, ends the process with `EXIT_TERMINATED`, 130, without a call of the application;
its marker stays, and the next start counts the run as a crash.

## Alternatives considered

- **One marker per application, written at start and removed at the end:** simpler, but a second
  window of the same application would find the first's marker and count a crash.
- **A process's id checked alive, rather than a lock:** an id is reused, and the check is the
  platform's own; the lock is the system's and lets go however the process ends.
- **`slime-files` given to uwui:** one way to keep files, but an engine crate under uwui, which
  leaves the repository. The host writes again, in some lines each, the state directory's rule, a
  file written beside and renamed, and a file held by a lock; the panic's message and the second
  signal's rule are the engine's again too ([`debt.md`](../debt.md)).
- **The report written by the hook:** one place for every panic, one that aborts the process
  included; but a write that fails in a hook aborts the process, and a panic that the application
  catches itself would leave a report of a run that went on.
- **A tracing layer that the application adds, for the log's lines:** a dependency more for uwui,
  `tracing-subscriber`, and lines only where the application thinks of it; the owner chose none
  until uwui keeps a log ([`uwui-needs.md`](../uwui-needs.md)).
- **The hours left until uwui's quality bar measures them:** less code now, but no figures from the
  first applications on uwui.
- **The panic's run counted at once by the host that caught it:** one start less to know of it, but
  a run killed, whose marker only the next start finds, would follow another path.
- **The application dropped after a panic:** what unwinding does by default; but its `Drop` is its
  code, which may write the state that panicked.

## Consequences

- A person who runs an application on uwui finds its crashes in their state directory, and is told
  at the next start that the run before did not end.
- Each start reads the crash directory and makes a file, nothing synced unless it found a run that
  did not end; each clean end writes the tally, synced
  ([`measurements.md`](../measurements.md#m6d7a2b3-crashes-and-panics-in-uwuis-desktop-host)).
- A panic that aborts the process, a panic during another's unwinding, writes no report: the next
  start finds its marker without one.
- Tests and tools that run an application give it its own `XDG_STATE_HOME`, or it writes in the
  user's.
- The code written twice, `slime-files`' and the engine's beside the host's, one per licence, until
  the engine stands on uwui's host.

## Validation and reconsideration

Read: the host, the engine's signal handling and `slime-files`. Checked by the host's tests: a
panic's report and marker, in the step, the start and the stop, a killed run's marker found, the
tally, a second termination (`crates/uwui-desktop/tests/rest.rs`). Assumed, checked by reading
Microsoft's documentation alone: that Windows lets a process write a file through the handle that
holds its lock, and refuses it through another. Assumed: that `File::lock` holds on the file systems that a
state directory lies on; where it fails, the run goes on without a marker. Proposing another
decision is justified if a lock proves unreliable on a platform, or once uwui keeps a log.
