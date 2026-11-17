# 0298. A panic ends a run without `stop`: the same rule in both hosts

- Status: Accepted
- Date: 2026-11-17

## Context

Two hosts run a program's code at each frame: the engine's, `slime-app`, which calls a game's
`Simulation` and `Presentation`, and uwui's desktop host, `uwui-desktop`, which calls an `App`.
Read-only reviews of both, on 2026-11-17, found that neither says what follows a panic:

- `slime-app` catches no panic: one in `fixed_step`, `update` or `apply` leaves `run` without
  `Simulation::stop`, where *Hunna Wood* saves its run (`games/hunnawood/src/simulation.rs`).
- `uwui-desktop` ends a run without `App::stop` after a panic in `App::step`, which its rustdoc does
  not say ([`debt.md`](../debt.md)).
- Calling `stop` on a state that panicked could write it over the last good save or document.
- Since Rust 1.81, a panic that leaves an `extern "C"` function aborts the process: Win32's and
  AppKit's callbacks are such functions, and the engine moves onto uwui's windowing layer at a port
  ([ADR-0279](0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md)).

## Decision

The owner's decision of 2026-11-17, one rule for both hosts:

**A panic in a program's code ends its run without `stop`,** nor any other call of the program:
the state that panicked is never handed back to be kept. The host logs the panic through its hook,
leaves the crash report of [ADR-0176](0176-fuzzing-and-crash-reports.md) once it exists, lets go of
what it holds, and `run` returns an error, or the process exits with one where `run` never returns.

**A panic inside a platform's callback is caught there,** before it leaves the callback, and follows
the same path, rather than aborting the process.

**What survives is what was kept before:** a game's last save written whole, the editor's journal
([ADR-0134](0134-the-editors-core.md)); no save of the moment of the panic is written.

**Each host's rustdoc says so,** beside `stop`, and a test of each makes its program panic and
checks that `stop` is not called and that the run ends with its error.

**Built** in `uwui-desktop` by M6d.7a2b, track B's, and its callbacks of Win32 and AppKit as they are
written, M6d.7c and M6d.7d; in `slime-app` by [S1](../stage-6d.md#s1--saves-migrated-from-layout-5),
which already changes how `slime-app` keeps files and how a game's saves are kept.

## Alternatives considered

- **Each host deciding alone,** uwui at M6d.7a2b, the engine with its crash reports: nothing to
  coordinate now, but two rules that may part while the engine moves onto uwui's layer, and the
  engine without one meanwhile.
- **A call of its own after a panic, `crashed`, where a program may write a save set aside:** a
  chance of keeping a run, but a state that may be broken, an API more for every program, and a
  second panic there to handle; worth weighing once crash reports show players losing runs.

## Consequences

- A player loses what they did since their last save, as after a crash of the process.
- Neither host calls a program's code on a state that panicked.
- A program that wants more keeps its own copy as it goes, as the editor's journal does.

## Validation and reconsideration

Read: both hosts' runs and their `stop`; *Hunna Wood*'s save at its stop; the editor's journal.
Assumed: that a platform's callback can catch a panic and end the run cleanly on Win32 and AppKit,
checked as M6d.7c and M6d.7d write them. Proposing another decision is justified if crash reports
show runs lost that a save set aside after a panic would have kept.
