# 0185. Rest, checked in CI

- Status: Accepted; amends [0021](0021-frame-pacing.md) (a hidden window draws on the compositor's
  signal, from M6c.Q2c), [0156](0156-the-editors-window-and-its-operations.md) (a program sets
  its frames on demand while it runs) and [0175](0175-the-quality-bar.md) (rest read for each
  thread; a hidden game checked from M6c.Q2c); amended by
  [0195](0195-a-series-of-headless-compositors.md) (rest also checked on each compositor of the
  series)
- Date: 2026-11-05

## Context

[ADR-0175](0175-the-quality-bar.md) set the budget of a program *at rest*: the editor and its
floating panels draw no frame and have no *own wake-up*, and a game paused or hidden draws nothing.
CI was to check it from the context switches that Linux counts in `/proc`, over 10 seconds; which
threads count was left to this milestone.

Until now, the editor's check asked the editor for its frame number before and after 10 seconds,
through the MCP server. Each request woke the editor for a frame, so the check allowed 2 frames,
and it counted no wake-up.

Measured on 2026-11-05 in a headless Weston with lavapipe:

- The editor at rest has 53 threads: its own, winit's, wgpu's, lavapipe's and Mesa's. In 10
  seconds, 52 of them do not switch once. One wakes twice a second: `blocking-1`, from the
  `blocking` crate's thread pool. AccessKit's adapter, which gives the interface to screen
  readers, connects to D-Bus through `zbus`, whose connection runs on that pool. `blocking` 1.7.0
  keeps an idle thread alive forever, waking every 500 ms; 1.6.2 ends it after 500 ms. It wakes on
  the owner's desktop too, where the connection succeeds.
- Hunna Wood, paused, runs its 60 frames a second: its 16 threads of lavapipe each switch about 50
  times a second. A game keeps its scheduled frames whether anything moves or not.
- winit 0.30 reports no hidden window on Wayland or on Windows, and
  [ADR-0021](0021-frame-pacing.md) chose that a hidden window keeps drawing at its schedule.

The owner decided on 2026-11-05, one question at a time, how a paused game rests, how a hidden
window is found, the milestone's cut and the fix of `blocking`.

## Decision

**What counts.** Every thread of the process counts: the program's, its libraries' and the
drivers', lavapipe's among them. Each thread's voluntary and involuntary context switches are
added: a thread that wakes and waits again switches once, and one taken off its core while it runs
switches too. A thread that runs without ever waiting may not switch on a core left to it: its
time on the CPU counts as well. A thread that starts or ends counts as a wake-up. A program is *at
rest* when, in 10 seconds, no thread switches, runs, starts or ends; a program that ended is not. A frame wakes the main thread, so a program at rest
draws none: the frames need no count of their own. No thread is excused: the measure found none
that needs it, and a driver's thread woken by the program's work is the program's cost.

**How it is read.** A helper of the MCP server's tests (`crates/slime-mcp/tests/common/rest.rs`)
reads `/proc/<pid>/task/<tid>/status` and `stat`, and checks that the process runs the program
that the test started. It waits until the program has been still for a second,
which the last frame asked of it may take seconds to reach on a CPU driver, within a limit that
each test gives. Then it reads the threads again after 10 seconds. Reading these files wakes
nothing. The test sends the program nothing meanwhile, and nothing else reaches it in a headless
Weston: no input, no output that changes. The editor runs without the developer's session bus of
D-Bus, as in CI, so that its adapter for screen readers reaches no service that could speak to it.

**Where it runs.** In the tests that already bring the programs to the states the budget names,
through the MCP server, which now answers `start_game` with the process it started:

- `edit_window`, in `check (windowed)`: the editor, a panel floating, after the first run's hint
  ended. This replaces the check of at most 2 frames.
- `drive`, in `check (game)`: Hunna Wood paused, its zone drawn, its overlay's panels open.
- `rest`, in `cargo test`, on Linux: the helper finds a program that waits at rest, and fails on a
  program made to wake itself, five times a second or once every 2 seconds, on one that spins and
  on one that ended.

**A game rests by its own choice.** `Context::set_on_demand` lets a program with a window turn
its frames on demand, as the editor's are from the start (ADR-0156), or back to their schedule,
while it runs. Hunna Wood turns them on demand while it is paused, once a frame drew its zone and
logged its loading, nothing loading, unless it counts its frames (`--frames`) or watches its files
(`--watch`). Its rules ask for the next frame while an arrow turns where the player looks, at real
time (`SimulationContext::request_frame`), and its screen for the frame that draws its view at a
new size, once that size has held still. Steps asked for while paused run in frames of their own.

**The frame after a rest.** The frame that ends a wait represents the time since what asked for
it, not the wait. Otherwise what moves at real time would jump by the longest time that a frame
represents, 250 ms, at the first frame after a rest.

**`blocking` held at 1.6.2.** The lock file holds `blocking` at 1.6.2, whose idle threads end,
until a release of `blocking` fixes 1.7.0's. `deny.toml` bans 1.7.0 and later, so that a
`cargo update` that raises it fails `cargo deny` with the reason, before the editor's check would.
The debt is in [`debt.md`](../debt.md).

**A hidden window, from M6c.Q2c.** On Wayland, a window draws when the compositor signals that
it can be shown (its frame callbacks, which winit gives through `pre_present_notify`), and the loop
stops waking on its own schedule while it waits for that signal. On Windows, a minimized window,
whose size is zero, draws nothing and does not wake. M6c.Q2c builds it, with the check of a hidden
game, measured on the owner's compositor and in the Windows virtual machine.

## Alternatives considered

- **Counting the program's own threads, not the drivers'**: a driver that polls would hide there,
  and the measure found none to excuse.
- **Counting frames through the MCP server**: each request wakes the frame it asks about, which is
  why the old check allowed 2.
- **Tracing wake-ups with `perf` or eBPF**: tells why each thread woke, but needs privileges or a
  kernel setting that shared runners may not give, and a new tool.
- **A script of `scripts/check.sh` that starts the programs itself**: it would need its own way to
  float a panel and to pause the game, which the MCP tests already have.
- **A paused game resting by a rule of the engine**: every game would rest without a line of its
  own, but a game that moves something at real time while paused would freeze without a word. The
  owner chose that a game decides.
- **Keeping `blocking` 1.7.0 with a patch of our own**: the latest version, but another's code kept
  in the repository.
- **`blocking` held by the lock file alone**: a `cargo update` would fail only the editor's check,
  which does not say why.
- **A hidden window kept drawing (ADR-0021 unchanged)**: no change to pacing, but a hidden game
  would stay over its budget. Frame callbacks are what GTK, Qt and Firefox use.
- **Hidden windows within this milestone**: one milestone instead of two, but a change to every
  game's pacing that only the owner's compositor and the Windows virtual machine can check.

## Consequences

- The budget's checks cost two watches of about 11 seconds, one in `check (windowed)`, which
  replaces the old check's 11 seconds, and one in `check (game)`; the helper's own test, about 12
  seconds of `cargo test`.
- Not checked on Windows: the helper reads Linux's `/proc`. A paused game's frames on demand run
  the same code there.
- A library that adds a thread that polls fails CI, wherever the thread comes from.
- A game whose frames run on demand asks for the frames of what it moves at real time.
- A paused game's dev overlay shows the costs of its last frame until the next.
- Hunna Wood with `--watch` keeps its frames while paused: watching its files polls them.
- A hidden game keeps drawing until M6c.Q2c; on Wayland, a compositor that sends a hidden window a
  frame callback each second, as niri did (ADR-0021), will see it draw once a second.
