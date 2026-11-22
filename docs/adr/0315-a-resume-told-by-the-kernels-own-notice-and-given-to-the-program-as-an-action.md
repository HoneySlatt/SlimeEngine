# 0315. A resume told by the kernel's own notice, and given to the program as an action

- Status: Accepted; supersedes in part
  [0249](0249-the-hosts-api-its-items-and-their-table.md) (the system's resume: its door, an
  action, and Linux's source, the kernel's notice) and
  [0270](0270-uwuis-desktop-host-on-a-windowing-layer-of-its-own-cut-by-platform-m6d7-cut-in-six.md)
  (Linux's notice of a resume)
- Date: 2026-11-22

## Context

M6d.7b3, "A resume's notice on Linux"
([`stage-6d.md`](../stage-6d.md#m6d7b3--a-resumes-notice-on-linux)), is the last part of M6d.7b
([ADR-0311](0311-m6d7b-cut-in-three-x11s-window-frames-and-pointer-first-on-x11rb.md)). Its card
asks that uwui learn that the system resumed, on Wayland and on X11, with the time suspended; that
the source be weighed at its start, logind's `PrepareForSleep` or the kernel's own; and that the
notice reach uwui's lifecycle
([ADR-0264](0264-the-lifecycle-before-each-targets-size-a-lost-surface-made-again-within-a-budget-m6d5e-cut-in-two.md)),
which left the resume waiting for its reader at M6d.7. ADR-0249's table put the resume among the
host's events, each with its target and its time, and named logind's signal on Linux; ADR-0270
named that signal for M6d.7b.

The facts, read on 2026-11-22:

- Both layers already measure the time suspended (`crates/uwui-windowing/src/clock.rs`): how much
  more `CLOCK_BOOTTIME`, the host's clock, went than `CLOCK_MONOTONIC`, which stops during a
  suspend. They read it at each turn and make the events' anchor again after a second or more. A
  window at rest turns only when something wakes it: a resume went unseen until the next input.
- The loop's deadline is a timerfd on `CLOCK_BOOTTIME` (`wait.rs`): a wait armed before a suspend
  already ends on time after it ([ADR-0227](0227-the-time-that-motion-reads.md)).
- The kernel tells a resume to every timerfd armed on `CLOCK_REALTIME` with
  `TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET`: `timekeeping_resume` calls `timerfd_resume`,
  "as resume is equivalent to clock_was_set()", whose work cancels each such timer whose offset
  between the wall clock and `CLOCK_MONOTONIC` changed; the timer becomes readable, and its read
  fails with `ECANCELED` (Linux's `kernel/time/timekeeping.c` and `fs/timerfd.c`, read on
  2026-11-22). The same happens when someone sets the clock. It needs no privilege, no thread and
  no bus; `rustix`'s `time` feature, which the workspace already enables, gives it. A suspend to
  idle tells it only where it freezes timekeeping, through a driver of the processor's idle
  states; one that idles with timekeeping running, as in many virtual machines, moves neither the
  clocks apart nor the timer.
- logind sends `PrepareForSleep(false)` on the system bus as the system resumes. It needs a D-Bus
  client: `zbus` 5, in the lockfile through `accesskit_unix`, which brings an executor and its
  thread, or one written for the purpose. It gives nothing where logind does not run, as without
  systemd or elogind, nor in a Flatpak without the permission to talk to `login1`; and it does not
  tell the time suspended.
- uwui has a door for each thing a host gives: the program reads what happened from its queue of
  actions; a clipboard's answer that the program asked for, `ActionKind::ClipboardText`, is an
  action without an element.

## Decision

The owner's decisions of 2026-11-22, one at a time: the kernel's own notice; the resume given to
the program through uwui, as an action; then, after the closing review, the kernel's cancel tested
in CI through a clock set. Then the lead's within them.

**The kernel's notice turns the loop; the clock's measure tells the resume.** The loop's wait
(`wait.rs`), which both layers share, polls a third timerfd, on `CLOCK_REALTIME`, armed at a time
that it never reaches with `TFD_TIMER_ABSTIME | TFD_TIMER_CANCEL_ON_SET`. When the kernel cancels
it, the wait ends, the timer is read and armed again, and the loop turns, X11's too, which turns
otherwise only for events that tell the host something. At each turn, as before, the layer reads
the time suspended since the last; a second or more tells a resume, told once in
`Happened::resumed` with its length, and makes the events' anchor again. A clock set without a
suspend turns the loop once and tells nothing. A resume that some other wake reaches first is told
at that turn, the notice's own turn then telling nothing. The threshold of a second, the one the
anchor already used, has one home, `clock.rs`. The notice never ends the run: a clock set while it
is armed again answers `ECANCELED` with the timer armed all the same, and a failure to arm it, as
in a sandbox without the flag, leaves it unarmed with a warning, a resume then told at the next
turn that something else wakes.

**uwui hears it as an action.** `Ui::resumed(suspended)` is the host's door: it puts
`ActionKind::Resumed { suspended }`, without an element, in the program's queue. The interface
itself changes nothing: what it waits for counts on the host's clock. `Ui::act` leaves out a resume
that an agent or the program sends: only a host tells one.

**uwui's desktop host gives it before the turn's inputs and the application's step**, so that the
program learns that what it holds may be old before it acts on any input; logs it; and counts it
as asked from outside, so that a window that holds its frames draws once its signal is late: a
signal asked before a suspend may never come. The example application writes `resumed after <seconds> s`, which
a real suspend checks by hand.

**The kernel's own cancel is tested in CI**, the owner's choice: a test of the wait, ignored
elsewhere, sets the clock to the time it shows through `sudo -n` on the throwaway runner of CI's
headless job, three times, and checks that each set ends the wait and leaves the notice watching.
It never runs on a person's machine: it asks for `UWUI_TEST_SETS_CLOCK=1`.

## Alternatives considered

On the source:

- **logind's `PrepareForSleep`**, as ADR-0270 wrote: a test could play logind on a bus of its own,
  and it tells a suspend before it starts too. But it needs a D-Bus client, `zbus`'s executor in
  uwui or a client of the project's own to keep; it gives nothing without logind; and the time
  suspended still comes from the clocks.
- **The clocks' measure alone**, as before: no code. But a window at rest learns of a resume only
  at its next input, which may be hours later.
- **A timer that wakes the loop now and then to measure**: no notice needed; but the loop wakes at
  rest, against the bar.

On the door into uwui:

- **A method of uwui-desktop's `App`**, `resumed`, empty by default: no change in `uwui`. But it is
  this host's alone: the engine and the hosts to come would each tell it their own way, and the
  program would read a resume elsewhere than its other actions.
- **An `Input` of the host's events**: an event names a target and a time and is answered as taken
  or not; a resume is the system's, for no target, and nothing in the interface takes it.

On the test of the kernel's cancel:

- **By hand alone**: nothing privileged in CI; but the kernel's path would be checked only when the
  owner suspends the machine, and a change that breaks it would pass every check.

## Consequences

- No dependency is added: `rustix`'s `time` feature was already on.
- A resume reaches a program on Linux at once, its window at rest too, where the kernel measures
  the sleep. The kernel's cancel is tested through a clock set in CI; the timer's arming from what
  the kernel tells of it (`/proc/self/fdinfo`); the measure, the door and the action on their own.
  The kernel's call at a resume itself, `timerfd_resume`, reaches no test: a real suspend, deep and
  to idle, on Wayland and under Xwayland, is a check by hand
  ([`PLAN.md`](../PLAN.md#checks-by-hand-waiting-for-the-owner)).
- CI's headless job runs one step through `sudo`, on its own throwaway runner.
- No notice comes before a suspend: nothing reads one yet; logind gives one, with a lock that delays
  the sleep, if a program ever needs to save before it.
- A clock set turns the loop once. The time of day's timer, when uwui's first stage builds it, may
  read the same notice to be armed again (ADR-0227).
- Windows (M6d.7c) and macOS (M6d.7d) give their own notices through the same door.
- [`debt.md`](../debt.md)'s item "Of the window's lifecycle, uwui hears the focus alone" now names
  the resume as given by uwui's desktop host on Linux.

## Validation and reconsideration

- Clock sets in a row: each turns the loop once and tells nothing; a set while the notice is armed
  again leaves it armed (reviewed in `fs/timerfd.c`; tested in CI).
- A suspend under a second: not told, and not counted later (tested).
- A suspend to idle that keeps timekeeping running: not told (reviewed in the kernel's source).
- A kernel or sandbox without `TFD_TIMER_CANCEL_ON_SET`: a resume told at the next input
  (reviewed).
- A real resume on the owner's machine: assumed until the check by hand. A machine where it is not
  told, or a program that needs to save before a suspend, would justify reading logind's
  `PrepareForSleep` beside the kernel's notice.
