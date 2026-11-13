# 0251. The engine's host clock and its times: rustix and windows-sys, the times given before each frame and event, the wake waiting for the executor

- Status: Accepted; amends [0227](0227-the-time-that-motion-reads.md) (the next wake and a wake
  that draws nothing come with uwui's executor; a layer follows the clock of the root that it opens
  over) and [0249](0249-the-hosts-api-its-items-and-their-table.md) (the rows "Wake" and "Frame and
  wake"); amended by [0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md) (an event's time carried
  by the event)
- Date: 2026-11-13

## Context

M6d.5b builds the host's side of the times
([`stage-6d.md`](../stage-6d.md#m6d5b--the-times)), as
[ADR-0227](0227-the-time-that-motion-reads.md) decides them and
[ADR-0249](0249-the-hosts-api-its-items-and-their-table.md)'s column "Built by" assigns them:
uwui's two types of time, each frame's time and its source, each root's clock, each event's time,
the next frame due and the next wake, a wake that runs uwui's timers without drawing, the host's
waker, and the host's clock counting a suspend. ADR-0227 puts each host's clock's dependency to the
owner when that host is built; the engine's host is `slime-app`.

The facts, read on 2026-11-13 in the code and, for the clocks, in the platforms' and crates'
current documentation and sources:

- uwui had one clock, a `Duration` from zero that `Ui::advance(elapsed)` moved on, shared by every
  root; `Ui::next_change` gave one deadline. `slime-app` gave it the time since its last call at
  each event and each frame's start, read from `std::time::Instant`.
- uwui has no executor nor tasks: [ADR-0211](0211-the-executor-and-other-threads-through-channels.md)
  decided them on prototypes that were not kept, and uwui's first stage builds them. Its timers,
  a tooltip's and a submenu's rests and a notification's end, all change the drawing, which
  ADR-0227 applies at the next frame, and wake nothing while their root does not show.
- `std::time::Instant` reads `CLOCK_MONOTONIC` on Linux and `CLOCK_UPTIME_RAW` on macOS, which
  stop during a suspend, and the performance counter on Windows, which counts sleep and
  hibernation. A change of `Instant` to `CLOCK_BOOTTIME` passed Rust's final comment period in
  2021 and was closed unmerged in 2022: Linux's waits (`poll`, futexes) follow `CLOCK_MONOTONIC`.
- rustix 1.1.6 reads `CLOCK_BOOTTIME` and `CLOCK_MONOTONIC` through a safe `clock_gettime`, by the
  vDSO without a system call since Linux 5.3; on Apple targets it offers no clock that counts a
  sleep but `CLOCK_MONOTONIC`, which Apple's C library derives from the wall clock. libc 0.2.190
  binds `CLOCK_BOOTTIME` and an `unsafe` `clock_gettime`, not `mach_continuous_time`. windows-sys
  0.61.2 binds the performance counter and `QueryUnbiasedInterruptTimePrecise`, which stops during
  sleep and hibernation, from Windows 10. Both crates were in the lock file already, for Wayland's
  crates and for arboard, ctrlc and tokio; rustix at 1.1.5, which the lock moves to 1.1.6 for
  RUSTSEC-2026-0486, an unsound function of `rustix::fs` that the engine does not call.
- winit 0.30.13 waits for `WaitUntil` on `std::time::Instant` on Linux, and by a relative time on
  Windows, which since Windows 8 does not count a sleep: on both, a wait armed before a suspend
  ends late by its length.
- uwui's layers, menus, tooltips and floating panels, are roots of their own, which take the scale
  and direction of the root that they open over.
- Giving a time with each input event changes uwui's input methods, called at about 600 places,
  most of them tests; M6d.5c reshapes those methods into events that carry their time, their
  target and their answer.
- No game's root runs on uwui: Hunna Wood uses uwui for its dev overlay alone, which lives on real
  time, and the editor follows the host's clock.

## Decision

The owner's decisions of 2026-11-13: the clock's dependency, the wake, a layer's clock; then the
lead's within them.

**The host's clock reads rustix on Linux and windows-sys on Windows.** `slime-app`'s `HostClock`
reads `CLOCK_BOOTTIME` through rustix's safe `clock_gettime`, with its `time` feature, and the
performance counter through windows-sys. Beside it, a clock that stops during a suspend,
`CLOCK_MONOTONIC` and the unbiased interrupt time, measures the time spent suspended: how much more
the host's clock went than the other since the last reading, read each time the loop is about to
wait again.
The three calls to Windows' API are `unsafe`, each on its smallest item with its `SAFETY` comment
([ADR-0005](0005-lints-and-unsafe-policy.md)); `slime-app` had none. Other systems, which the
engine does not support yet, read `std::time::Instant`; macOS's clock comes with uwui's desktop
host (M6d.7).

**The next wake and a wake that draws nothing wait for uwui's executor.** M6d.5b builds the next
frame due, `Ui::next_frame_due`, in place of `Ui::next_change`, and the waker that the host gives
uwui, `Ui::set_waker`, which the agents' sessions call from M6d.9a1b3. `Ui::next_wake` and a wake
that runs tasks without drawing would run nothing today: they come with the executor, at uwui's
first stage, which they serve. Until then, each wake of the host's waker asks the engine for a
frame, as `slime_app::Waker` always did, and a hidden window holds it as
[ADR-0187](0187-a-hidden-window-at-rest.md) says. ADR-0249's row "Wake" is built by its reader, the
executor, and the next wake leaves the output "Frame and wake" until then.

**The times are given before each frame and each event.** `Ui::set_frame_shown` gives the time at
which the next frame shows, with its source; `Ui::set_event_time` the time at which the next event
came, with its source. The interface's time is the latest given and never goes back; until a host
gives one, it is the clock's zero, and a host gives one before it adds what waits. One frame's time
serves every root of a `Ui`: each target's comes with the targets of M6d.5c. M6d.5c's events carry
their time in place of the second call. `Ui::advance` is gone. The type is named `FrameShown`, not
"frame time", which ADR-0227 refused for hiding that the time is when the frame shows, and which
`slime_app::time::FrameTime`, a frame's real time, already holds.

**The sources built are those that a host gives today.** A frame's: `Simulated`, given by tests
and headless runs, and `Started`, the frame's start, which the engine gives. An event's:
`Received`, the engine's stamp as winit hands the event over. ADR-0227's other sources come with
the desktop host that reads them (M6d.7); the enums are `#[non_exhaustive]`. A frame tells its time
and source back (`FrameStats::shown`).

**Which clock each motion follows.** A style's transition follows the host's clock when one of the
states that answer a person, `Hovered`, `Pressed` or `FocusVisible`, changed since its element was
last styled; any other, and a section's growing or shrinking, follows its root's. A section's
starts at the first frame after what opened or folded it, so that its first step shows. Tooltips,
submenus, notifications and the waiting bar follow the host's clock; the waiting bar's phase is
taken from the clock's whole nanoseconds. A root that the host gives no clock follows the host's at
speed 1. A layer follows the clock of the root that it opens over, as it takes its scale: a menu of
a paused game's interface holds with the game, and a pause menu, added as a root of its own, does
not. A root's first clock, which replaces the host's, and a root's clock that goes back, a save
loaded, start its transitions again from the new time with the time that each had gone, so that
each keeps its place; a later time moves them on, as a late frame does (ADR-0227's "no cap after a
late frame"). uwui places motions by the time given, and reads the speed only as whether the clock
runs: while it is 0, a root's transitions hold, are neither drawn nor laid out again, and ask for
no frame.

**The engine gives the frame's start and each event's reception on the host's clock, and no
root's clock.** The windowed loop reads `HostClock` for each frame's start and each event's
reception, and gives uwui the run's waker as a `std::task::Waker`. Its own pacing, and winit's
`WaitUntil`, keep `std::time::Instant`: the interface's next frame due converts from the host's
clock by the two clocks read together at the frame's start. The game's clock reaches a root when a
game's interface runs on uwui, through the layer of games' interfaces
([`uwui-needs.md`](../uwui-needs.md#slimeengines-own)). The time suspended is logged; winit 0.30
tells no resume on the desktop, so it reaches uwui with the system's notice that M6d.7 finds.

## Alternatives considered

On the clock's dependency:

- **libc on Linux**, the crate that macOS's host will need: an `unsafe` call on Linux too, through
  the system's C library rather than rustix's direct reading of the vDSO.
- **rustix on Linux, `std::time::Instant` on Windows**, which counts a suspend there: no
  dependency on Windows, but the time suspended is not measured there, and the clock's origin is
  the program's start, so that Windows' input times (`POINTER_INFO.PerformanceCount`) would not
  convert at M6d.7 without changing it.
- **No dependency, the debt written down**: a timer of ten minutes ends late by the time the
  machine slept, on Linux, against ADR-0227.

On the wake:

- **`Ui::wake` and `Ui::next_wake` built now, empty of tasks**: the host's loop shaped once for
  M6d.5e and M6d.7; but `next_wake` would never give anything, code without a reader against
  [ADR-0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md), and its shape would guess
  the executor's needs.

On giving the times:

- **A time with each input method now**: the end shape; but about 600 calls changed twice, now and
  at M6d.5c.
- **`Ui::frame` taking its time**: one call; but about 700 calls, most of them tests that read no
  time.

On a root's clock that goes back:

- **Each start moved back by the gap**: one value; but a start before the clock's zero cannot be
  written in `RootTime`, so a root that takes the game's clock from zero lost its transitions'
  progress.
- **Transitions ended at once**: simple; but a save loaded mid-fade jumps to its end.
- **Every jump forward re-anchored too**: a save loaded ahead keeps its fades; but telling a jump
  from time that passed needs the speed and the host's time, and a late frame would then lose the
  motion that ADR-0227 places where the clock puts it.

On a layer's clock:

- **The host's clock, unless the host gives the layer one**: no rule more; but the layer of games'
  interfaces must give every menu that it opens the game's clock, and a menu forgotten moves
  during the pause.

## Consequences

- uwui gains `uwui::time` (`Instant`, `RootTime`, `FrameShown` and `FrameSource`, `EventTime` and
  `EventSource`, `RootClock`) and `Ui::set_frame_shown`, `Ui::set_event_time`,
  `Ui::set_root_clock`, `Ui::next_frame_due`, `Ui::set_waker` and `Ui::waker`; `Ui::advance` and
  `Ui::next_change` are gone.
- `slime-app` depends on rustix on Linux and windows-sys on Windows, and holds `unsafe` code on
  Windows, which the owner accepted with windows-sys (ADR-0005); `docs/ARCHITECTURE.md` lists it.
  Miri gives the performance counter and its frequency, not the unbiased interrupt time; no job of
  CI runs Miri.
- Debt, in [`debt.md`](../debt.md): the frame's start and the stamps at reception until M6d.7; the
  loop's waits, which end late by a suspend on Linux and on Windows until M6d.7 finds the notice
  that the system resumed; macOS's clock; one frame's time for every root and the times given
  beside the events until M6d.5c; the wake until the executor.
- A check by hand: the machine suspended while the editor waits for a notification's end, on
  Linux and in the Windows virtual machine.
- The time suspended is measured from two clocks read one after the other, off by the
  nanoseconds between them; whether the performance counter and the unbiased interrupt time drift
  apart on Windows over a long wait was not read.
