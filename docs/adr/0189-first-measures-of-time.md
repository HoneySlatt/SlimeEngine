# 0189. First measures of time

- Status: Accepted; amends [0104](0104-the-build-ci-and-measurements.md) (the trace holds the
  intervals from an input to the image after it), [0175](0175-the-quality-bar.md) (the first
  useful frame; a frame of input to present is its tier's) and
  [0186](0186-counters-and-ratios-in-ci.md) (M6c.Q2d cut in two, memory and power in M6c.Q2e)
- Date: 2026-11-06

## Context

[ADR-0175](0175-the-quality-bar.md) set two budgets of time that nothing measured: the editor and
applications start, warm, to their first useful frame within 150 ms; and from an input's arrival to
the image's present, measured inside the engine, at most one frame at the 99th percentile for the
editor and applications, two for games. A level opened, a game to control and cold starts were to
be measured first, their figures proposed after. [ADR-0186](0186-counters-and-ratios-in-ci.md) put
these measures in M6c.Q2d, with memory, power, variable refresh rates, `hyperfine` and `heaptrack`.

At the milestone's start:

- Nothing timed a start. The editor runs its frames on demand and had no option to quit after one;
  its log said when it opened its level, not when its window showed it.
- The trace that `SLIME_TRACE` asks for ([ADR-0104](0104-the-build-ci-and-measurements.md)) held
  spans only, and nothing timed an input.
- The editor's frames run at most 60 a second, `AppConfig::max_frame_rate`'s default: while the
  pointer moves, an input may wait up to 16.7 ms for the next frame of the schedule.
- The milestone held two designs with code in the engine, two sessions of measures by hand and
  about six figures to propose.

Measured on 2026-11-06 in a headless Weston with lavapipe, the editor in a release build on the
examples' yard, the machine not at rest: its first image presented 440 to 450 ms after the engine
started, of which 31 ms to open the GPU and 23 ms to read the system's fonts; 500 ms for the
process whole.

The owner decided on 2026-11-06, one question at a time: the milestone's cut, what the editor's
first useful frame is, how a start is timed and which of its figures the budget weighs, and how
long a frame of input to present is.

## Decision

**The milestone cut in two.** M6c.Q2d measures time: the starts, input to present, variable refresh
rates, with `hyperfine`. A new milestone, M6c.Q2e, measures resources: memory over 100 undos and
redos and 10 runs of Play, the editor's memory and video memory on the zone, the GPU's power at rest
and of a game at 60 frames a second, with `heaptrack`. Each proposes the figures of its budgets that
have none.

**The first useful frame.** For the editor and applications, the first image presented: the
window, its panels and menus, the level's hierarchy; the viewport may still load its meshes, as the
best editors show their interface first. A level opened is measured apart, until its viewports draw
it whole, every mesh loaded; its textures may still arrive at their full resolution
([ADR-0074](0074-uploading-textures-without-holding-up-frames.md)).

**A start timed by the program and by `hyperfine`.**

- `slime_app::since_start` gives the time since the engine first saw the program:
  `init_diagnostics`, which `run` calls first.
- A windowed run logs at `info` when its first image is presented, the present returned, in
  milliseconds after the start. The editor logs when its viewports first draw its level whole, and
  Hunna Wood when a frame first draws its zone, the player's control: each once the frame is handed
  to the GPU, before its present.
- The editor takes `--frames <count>`, as Hunna Wood and the examples do: it runs that many frames
  on its schedule, not on demand, then quits. It counts every frame, where Hunna Wood counts those
  that draw its zone. `hyperfine` times a run of `--frames 1` whole, from the process's launch to
  its exit, and the launch alone, a run of `--help`, which loads the same program and quits as it
  reads its options.
- The figure that the budget weighs is the log's time to the first image plus the launch: from
  the process's launch to its first useful frame, without the exit, which the user does not wait
  for.
- `scripts/start-times.sh` runs them, for the editor on the yard, the game's editor on its zone
  and the game, after three runs that fill the system's caches: a *warm* start; the editors are
  run apart until they draw their level whole, then stopped. In a headless Weston of its own, on
  copies of the levels and a state of their own, with lavapipe, or the machine's GPU through
  Weston's GL renderer (`--gpu`).
- A *cold* start follows the system's caches dropped, which needs root: the script's `--cold`
  asks for it through `sudo` before each run, and only the owner runs it.
- `uwui`'s span `system_fonts` shows in a trace the read of the system's fonts.

**Input to present in the trace.** A windowed run notes when the oldest input arrives that no image
has followed yet: a key, a mouse button, the pointer, its raw motion, the wheel, the input method.
When the next image's present returns, the trace holds the interval, an event `input to present` on
a track of its own. Without a trace, nothing is written; the run reads the clock once for each
image that inputs wait for. `scripts/frame-pacing.py` gives the intervals' median, 99th percentile
and longest. The interval ends at the first image after the input, which may not show its effect
yet if the program needs another frame for it; the compositor and the display add time that is not
measured. An input that waits for the run's first image, or over a new GPU device's opening, waits
for more than frames: it is forgotten.

**A frame of input to present is its tier's.** One frame for the editor and applications is
4.17 ms, a frame at 240 a second on the high tier; two frames for a game are twice its tier's frame
time, 33.3 ms at 60 a second. The figures do not change with the display.

**Variable refresh rates, checked by hand.** On the owner's display, variable refresh on, the
`window` example's frames are traced and read by `scripts/frame-pacing.py`, as in
[ADR-0187](0187-a-hidden-window-at-rest.md): the roadmap then states them supported, or not.

**`hyperfine` in the development shell**, and `heaptrack` with M6c.Q2e.

## Alternatives considered

- **One milestone, as planned**: one review and one report, but two designs, two sessions by hand
  and six figures in one session, which might stop half way.
- **Three milestones**, the start, input and variable refresh, then resources: smaller, but three
  reviews and three runs of CI for work that is mostly measuring.
- **The level drawn whole as the first useful frame**: closer to "I can work", but the budget would
  depend on the level's size, and the zone's 384 MiB of textures do not load in 150 ms.
- **An environment variable that ends any windowed run after a number of frames**: every program
  at once, but a second way to do what `--frames` does, and a behaviour hidden.
- **The program's log alone**: no option in the editor, but `hyperfine` would have nothing to time,
  and the launch would go unseen.
- **`hyperfine`'s whole run as the budget's figure**: the simplest and a bound from above, but the
  exit, closing the GPU and the threads, would count against the start.
- **The process's start read from the system**: `/proc/self/stat` counts in ticks of 10 ms on
  Linux, and Windows needs a system call through `unsafe` code.
- **The display's refresh as the frame of input to present**: what a person feels, but a figure
  that changes with each display, and a headless Weston at 60 Hz compares with no desk's display.
- **The program's own schedule as the frame**: the easiest to meet, but an editor capped at 60
  frames a second would meet one frame with 16.7 ms on a display at 240 Hz.
- **Input to present summed up in the log as the run ends**: no trace to read, but every run keeps
  its intervals, and a log line says less than a trace that shows each interval among the frames.
- **Input to present through the development channel**: a tool's request, but a tool's input does
  not cross the window, and nothing gives a headless Weston a person's input.

## Consequences

- Every windowed program logs one line more at its start.
- `check (windowed)` runs the editor with `--frames 1` and checks its log, about a second.
- Input to present is not checked in CI: no input reaches a headless Weston. It is measured by
  hand, on the owner's desktop.
- The editor's frames, capped at 60 a second, are measured against a frame of 4.17 ms
  ([`measurements.md`](../measurements.md)); what it misses is debt.
- Applications on uwui are measured once they exist, at Stage 6d.
- The roadmap's budgets of memory and power name M6c.Q2e.
